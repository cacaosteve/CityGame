//! Mac CODE 8 monthly RCI scan partition orchestration.
//!
//! This module owns the ordered per-partition worker and its local marina,
//! arcology, and late-XUND effects. Growth placement, transit routing,
//! transport upkeep, and building-removal implementations stay with their
//! dedicated modules and are called through the budget owner.

use super::*;

/// Mac CODE 8 `export_241` work for one CODE 14 phases-3–18 map partition.
/// This ports the recovered transport upkeep, powered transit-route effects,
/// weighted counters, class-0/1/2 and generic growth, and bounded special-zone
/// and XUND edits. Mac's class/coefficient/placement tables remain separate
/// from PE's. Other special/event mutations and month-wide draw interleaving
/// remain incomplete, so this is not whole-worker or original-runtime parity.
/// The caller selects this worker by engine fidelity; saved-city layout does
/// not gate Mac simulation.
pub(in crate::app) fn mac_budget_accumulate_monthly_rci_scan_partition(
    city: &mut City,
    x_residue: usize,
    y_residue: usize,
    rng: &mut MacToolboxRng,
) {
    let mut kind2_sound_deadline = 0;
    mac_budget_accumulate_monthly_rci_scan_partition_inner(
        city,
        x_residue,
        y_residue,
        rng,
        None,
        &mut kind2_sound_deadline,
    );
}

/// Live Mac scan entry. The rail producer's route chooser consumes the same
/// process-owned CODE 13 game LCG as rail-head updates and the monthly zoo row.
pub(in crate::app) fn mac_budget_accumulate_monthly_rci_scan_partition_with_game_rng(
    city: &mut City,
    x_residue: usize,
    y_residue: usize,
    rng: &mut MacToolboxRng,
    game_rng: &mut MacGameLcgRng,
    kind2_sound_deadline: &mut u32,
) {
    mac_budget_accumulate_monthly_rci_scan_partition_inner(
        city,
        x_residue,
        y_residue,
        rng,
        Some(game_rng),
        kind2_sound_deadline,
    );
}

fn mac_budget_accumulate_monthly_rci_scan_partition_inner(
    city: &mut City,
    x_residue: usize,
    y_residue: usize,
    rng: &mut MacToolboxRng,
    mut game_rng: Option<&mut MacGameLcgRng>,
    kind2_sound_deadline: &mut u32,
) {
    if x_residue >= 4 || y_residue >= 4 {
        return;
    }

    let active_corner = city.mac_growth_scan_corner_mask();
    // export_241 captures this once at entry, before any church is placed.
    // It is independent of the per-candidate power/route result.
    let church_gate = u32::from(city.mac_xbld_tile_count(0xf7)) * 2500
        < city.mac_state_misc_u32_at(0x102c).unwrap_or(0);
    for x in (x_residue..CITY_WIDTH).step_by(4) {
        for y in (y_residue..CITY_HEIGHT).step_by(4) {
            // CODE 8 `export_241` `0x35f90..0x360bc`: unzoned XBLD
            // `0x5d..=0x60` road and the following rail selector families
            // reach direct QuickDraw calls and PlaceTile before the ordinary
            // zone/counter path. Export 208's app-LFSR gate is always zero.
            mac_budget_decay_road_or_rail_at(city, x, y, rng);
            mac_budget_decay_bridge_at(city, x, y, rng);
            mac_budget_decay_cardinal_portal_at(city, x, y, rng);
            mac_budget_decay_highway_block_at(city, x, y, rng);
            mac_budget_update_arcology_xmic_offset(city, x, y);
            mac_budget_try_spawn_mac_marina_actor(city, x, y);
            if let Some(game_rng) = game_rng.as_deref_mut() {
                mac_budget_try_spawn_mac_rail_actor(city, x, y, game_rng);
            }

            let Some(zone) = city.zone_at(x, y) else {
                continue;
            };
            let category = usize::from(zone.zone_id());
            if category == 8 {
                mac_budget_process_zone_eight_special_growth(city, x, y, rng, kind2_sound_deadline);
                continue;
            }
            if category == 9 {
                mac_budget_process_zone_nine_special_growth(city, x, y, rng);
                continue;
            }
            if category == 7 {
                mac_budget_process_zone_seven_special_growth(city, x, y, rng);
                continue;
            }
            if !(1..=6).contains(&category) {
                // CODE 8 `export_241` branches from the ordinary RCI path at
                // `0x364c6..0x36744` and the category-0 upkeep path at
                // `0x35fa8..0x364c6` straight to `0x36c48`. Only the
                // non-special zone IDs 10..=15 reach the late XUND gate at
                // `0x36bc6` (the 7/8/9 arms have already continued above).
                if category >= 10 {
                    mac_budget_process_late_xund_subway_gate(city, x, y, rng);
                }
                continue;
            }

            let building = city.building_at(x, y);
            let low_xbld_override = building < 0x1d;
            let (coefficient, local_c) = if low_xbld_override {
                // CODE 8 first calls CODE 14 export 216 for low XBLD. Only a
                // positive network query enters the shared powered-route and
                // generic-growth path with the coefficient forced to zero.
                if !mac_budget_has_nearby_transit(city, x, y) {
                    continue;
                }
                (
                    0,
                    mac_budget_try_transit_route_at(city, x, y, category, 0, false, rng),
                )
            } else {
                // The ordinary XBLD >= 0x70 path requires the selected XZON
                // corner marker. XBIT is used separately for powered adjacency.
                if building < 0x70 || zone.raw_bits() & active_corner == 0 {
                    continue;
                }
                let coefficient = mac_growth_coefficient_for_xbld(building);
                let local_c =
                    mac_budget_try_transit_route_at(city, x, y, category, coefficient, true, rng);
                (coefficient, local_c)
            };
            let counter_delta = mac_growth_counter_delta(coefficient);
            let growth_class = if low_xbld_override {
                0
            } else {
                mac_growth_class_for_xbld(building)
            };
            if let Some(slot) = mac_growth_counter_slot(growth_class, coefficient, category) {
                let prior = city.mac_rci_counter(slot) as i32;
                city.set_mac_rci_counter(slot, prior.wrapping_add(i32::from(counter_delta)) as u32);
            }
            let branch = mac_budget_choose_growth_branch(
                growth_class,
                coefficient,
                category,
                local_c as i16,
                mac_budget_growth_land_value_at(city, x, y),
                church_gate,
                rng,
            );
            match branch {
                MacGrowthBranch::None => {}
                MacGrowthBranch::ClassZero { orientation } => {
                    mac_budget_dispatch_class_zero_footprint(
                        city,
                        x,
                        y,
                        coefficient,
                        orientation,
                        rng,
                    );
                }
                MacGrowthBranch::Church => mac_budget_place_church(city, x, y),
                MacGrowthBranch::Pattern => {
                    mac_budget_place_growth_pattern(
                        city,
                        x,
                        y,
                        coefficient,
                        (category - 1) / 2,
                        rng,
                    );
                }
                MacGrowthBranch::Generic => {
                    mac_budget_place_generic_growth(city, x, y, category, coefficient, rng);
                }
            }
        }
    }
}

/// Mac CODE 8 `export_241` XBLD `0xed` producer. The export-205 A5-2 LFSR is
/// a zero fixed point in the recovered Mac data image. Export 203 supplies the
/// candidate offset, while export 201 uses the app-owned persistent game LCG
/// for route direction. The caller preserves map-scan order and shared slots.
fn mac_budget_try_spawn_mac_rail_actor(
    city: &mut City,
    x: usize,
    y: usize,
    game_rng: &mut MacGameLcgRng,
) -> bool {
    if city.building_at(x, y) != 0xed
        || !city.zone_at(x, y).is_some_and(|zone| zone.zone_id() == 0)
        || !city
            .tile_bits_at(x, y)
            .is_some_and(|bits| bits.raw_bits() & 0x40 != 0)
    {
        return false;
    }

    let mut lfsr = MacAppLfsrRng::new(MacAppLfsrRng::INITIAL_STATE);
    if lfsr.low_two_bits() != 0 || city.mac_xbld_tile_count(0xed) / 4 <= city.mac_rail_actor_counter
    {
        return false;
    }

    super::mac_loop_scheduler::rail_consist::mac_spawn_rail_near_station(
        city,
        x as i16,
        y as i16,
        || lfsr.bounded(4),
        || game_rng.bounded(2),
    )
}

/// Mac CODE 8 `export_241` marina branch → CODE 6 export 118. The reconstructed
/// CODE 13 app LFSR is fixed at zero, so both selector gates admit and each
/// eligible cardinal water neighbor is used. This is Mac XTHG/XTXT
/// behavior, independent of Win95/XTRF road traffic and PE sailboat spawning.
fn mac_budget_try_spawn_mac_marina_actor(city: &mut City, x: usize, y: usize) -> bool {
    if city.building_at(x, y) != 0xf8
        || !city.zone_at(x, y).is_some_and(|zone| zone.zone_id() == 0)
        || !city
            .tile_bits_at(x, y)
            .is_some_and(|bits| bits.raw_bits() & 0x40 != 0)
    {
        return false;
    }

    let mut lfsr = MacAppLfsrRng::new(MacAppLfsrRng::INITIAL_STATE);
    if lfsr.low_two_bits() != 0
        || city.mac_xbld_tile_count(0xf8) / 9 <= city.mac_marina_actor_counter
        || city.mac_marina_actor_counter >= 4
    {
        return false;
    }

    // FUN_0000cbdc probes the initialized CODE 0 offsets in N/E/S/W order.
    let candidates = [
        y.checked_sub(1).map(|ny| (x, ny)),
        x.checked_add(1)
            .filter(|&nx| nx < CITY_WIDTH)
            .map(|nx| (nx, y)),
        y.checked_add(1)
            .filter(|&ny| ny < CITY_HEIGHT)
            .map(|ny| (x, ny)),
        x.checked_sub(1).map(|nx| (nx, y)),
    ];
    let mut spawned = false;
    for (water_x, water_y) in candidates.into_iter().flatten() {
        if !city
            .tile_bits_at(water_x, water_y)
            .is_some_and(|bits| bits.raw_bits() & 0x04 != 0)
            || city.text_overlay_at(water_x, water_y) != Some(0)
        {
            continue;
        }
        let Some(slot) = city
            .mac_object_slot_records_mut()
            .iter()
            .enumerate()
            .skip(1)
            .find_map(|(slot, record)| (record.kind == 0).then_some(slot))
        else {
            break;
        };
        let direction = lfsr.bounded(3) as u8;
        let prior = city.mac_object_slot_records_mut()[slot];
        let prior_xtxt = city.text_overlay_at(water_x, water_y).unwrap_or(0);
        city.set_mac_object_slot_record(
            slot,
            Win95OverlayRecord {
                kind: 9,
                lane: direction,
                state: 0,
                // Native CODE 6 writes its map X/Y pair to +3/+4 and
                // preserves the free slot's +8/+9 and +0xB bytes.
                tile_x: water_x as u8,
                tile_y: water_y as u8,
                sub_x: 4,
                sub_y: 4,
                target_x: prior.target_x,
                target_y: prior.target_y,
                // Native writes the admitted cell's prior XTXT to +0xA; the
                // empty-marker gate above makes this zero, not stale slot data.
                saved_a: prior_xtxt,
                saved_b: prior.saved_b,
                ..Win95OverlayRecord::default()
            },
        );
        city.set_text_overlay(water_x, water_y, (0xc9 + slot) as u8);
        city.mac_marina_actor_counter = city.mac_marina_actor_counter.wrapping_add(1);
        spawned = true;
    }
    spawned
}

/// Mac CODE 8 `export_241 @ 0x364bc`: the phases-3–18 walk rewrites XMIC `+1`
/// for each qualifying arcology tile. Although the value is also used to place
/// object windows, phase 0 reads it as the arcology age input, so this scan
/// mutation changes next month's resident update. The original predicate does
/// not require the XMIC class byte to equal the current XBLD byte.
fn mac_budget_update_arcology_xmic_offset(city: &mut City, x: usize, y: usize) {
    let building = city.building_at(x, y);
    if !(0xfb..=0xfe).contains(&building)
        || !city
            .zone_at(x, y)
            .is_some_and(|zone| zone.raw_bits() & 0xf0 == 0x80)
    {
        return;
    }

    let Some(xtxt @ 0x33..=0xc8) = city.text_overlay_at(x, y) else {
        return;
    };
    let index = usize::from(xtxt - 0x33);
    let Some(record) = city.microsim_at(index) else {
        return;
    };
    if !(0xfb..=0xfe).contains(&record.tile_id) {
        return;
    }

    // The Mac worker's density planes are 64×64. CityGame keeps their current
    // values expanded over 2×2 city tiles, so sample the even/even cell which
    // corresponds to the original x/2,y/2 address.
    let sample_x = x & !1;
    let sample_y = y & !1;
    let land = i32::from(city.land_value_at(sample_x, sample_y).unwrap_or(0) >> 5);
    let crime = i32::from(city.crime_density_at(sample_x, sample_y).unwrap_or(0) >> 5);
    let pollution = i32::from(city.pollution_density_at(sample_x, sample_y).unwrap_or(0) >> 5);
    let mut offset = land + (12 - crime) - pollution;
    let bits = city.tile_bits_at(x, y).unwrap_or_default();
    if !bits.powered() {
        offset /= 2;
    }
    if !bits.watered() {
        offset /= 2;
    }
    let offset = offset.clamp(0, 12) as u8;
    city.set_microsim_stat0(index, offset);
}

/// CODE 8 `export_241` `0x36bc6..0x36c46` late XUND/subway-funding gate.
/// The A5 LFSR helper is at its reconstructed zero fixed point in the Mac
/// data image. Accepted XUND values other than `0x23` only enter
/// TickCount/SystemTask helpers. The XUND-`0x23` cardinal portal arm, stock
/// generic extent-1 selectors, the `0x51..=0x5c` bridge strip, and
/// extent-2 topology selectors `0x49..=0x50`/`0x61..=0x6b`, and the table-
/// backed generic extent-1/2/3/4 bands have bounded zero-render export-80 map
/// edits. Civic latch writes and the `0xd7` XMIC choice-mask decrement are
/// preserved; unknown off-map marker-probe behavior remains open.
fn mac_budget_process_late_xund_subway_gate(
    city: &mut City,
    x: usize,
    y: usize,
    rng: &mut MacToolboxRng,
) {
    let underground = city.underground_at(x, y);
    if !matches!(underground, 0x01..=0x0f | 0x1f | 0x20 | 0x22 | 0x23) {
        return;
    }

    let funding = mac_budget_service_funding_word_for_city(city, 10);
    if funding == 100 || funding > rng.random_u16() % 100 {
        return;
    }

    if underground == 0x23 {
        mac_destroy_building_export80(city, x, y, rng, false);
    }
}

#[cfg(test)]
mod mac_arcology_geometry_tests {
    use super::*;

    #[test]
    fn mac_growth_scan_arcology_effect_is_independent_of_city_storage_layout() {
        let mut result = Vec::new();
        for kind in [CityFileKind::MacCity, CityFileKind::WinSc2] {
            let mut city = City::new_blank("/tmp/mac-arcology-cross-layout.sc2", "Arcology")
                .expect("blank city");
            city.kind = kind;
            if kind == CityFileKind::WinSc2 {
                city.prepare_mac_fidelity_from_saved_city();
            }

            let (x, y, xtxt) = (20, 24, 0x34);
            city.set_building(x, y, 0xfb);
            city.set_zone(x, y, 0x80);
            city.set_text_overlay(x, y, xtxt);
            city.set_tile_bits(x, y, TileBits::new(0));
            let slot = usize::from(xtxt - 0x33);
            assert!(city.set_microsim_tile_id(slot, 0xfc));
            assert!(city.set_microsim_stat0(slot, 99));

            let mut land = vec![0; CITY_TILES];
            let mut crime = vec![0; CITY_TILES];
            let mut pollution = vec![0; CITY_TILES];
            let sample = x * CITY_HEIGHT + y;
            land[sample] = 96;
            crime[sample] = 224;
            pollution[sample] = 32;
            city.sync_land_value_from_slice(&land);
            city.sync_crime_density_from_slice(&crime);
            city.sync_pollution_density_from_slice(&pollution);

            let mut rng = MacToolboxRng::new(0x1357_2468);
            let rng_before = rng.seed();
            mac_budget_accumulate_monthly_rci_scan_partition(&mut city, 0, 0, &mut rng);

            result.push((
                city.microsim_at(slot).map(|row| row.stat0),
                city.building_at(x, y),
                rng.seed(),
                rng_before,
            ));
        }

        assert_eq!(result[0].0, Some(1));
        assert_eq!(result[0].1, 0xfb);
        assert_eq!(result[0].2, result[0].3);
        assert_eq!(result[1], result[0]);
    }

    #[test]
    fn mac_growth_scan_counter_uses_engine_state_for_both_city_layouts() {
        for kind in [CityFileKind::MacCity, CityFileKind::WinSc2] {
            let mut city =
                City::new_blank("/tmp/mac-rci-cross-layout.sc2", "Mac RCI").expect("blank city");
            city.kind = kind;
            if kind == CityFileKind::WinSc2 {
                city.prepare_mac_fidelity_from_saved_city();
            }
            city.set_zone(40, 40, 0x81);
            city.set_building(40, 40, 0xc5);
            let mut rng = MacToolboxRng::new(1);

            mac_budget_accumulate_monthly_rci_scan_partition(&mut city, 0, 0, &mut rng);

            assert_eq!(city.mac_rci_counter(7), 36, "{kind:?}");
            assert_eq!(city.mac_rci_counter(1), 0, "{kind:?}");
        }
    }

    #[test]
    fn mac_growth_scan_counters_round_trip_on_win_layout_without_overwriting_pe_counts() {
        let path = std::env::temp_dir().join(format!(
            "sc2000-mac-growth-cross-layout-{}.sc2",
            std::process::id()
        ));
        let mut city = City::new_blank(&path, "Mac Growth Cross Layout").expect("blank city");
        city.kind = CityFileKind::WinSc2;
        city.prepare_mac_fidelity_from_saved_city();

        // These PE MISC fields overlap Mac's native monthly counters. They
        // must remain the Win-layout values while Mac fidelity uses CGEX.
        let pe_counts = [0x1122_3344, 0x5566_7788, 0x99aa_bbcc];
        city.set_misc_u32_at(0x05f8, pe_counts[0]);
        city.set_misc_u32_at(0x0600, pe_counts[1]);
        city.set_misc_u32_at(0x0608, pe_counts[2]);
        city.set_zone(40, 40, 0x81);
        city.set_building(40, 40, 0xc5);

        let mut rng = MacToolboxRng::new(1);
        mac_budget_accumulate_monthly_rci_scan_partition(&mut city, 0, 0, &mut rng);

        assert_eq!(city.mac_rci_counter(7), 36);
        assert_eq!(
            [
                city.misc_u32_at(0x05f8).unwrap(),
                city.misc_u32_at(0x0600).unwrap(),
                city.misc_u32_at(0x0608).unwrap(),
            ],
            pe_counts
        );
        city.save(&path)
            .expect("save Win-layout city with Mac state");

        let loaded = City::load(&path).expect("reload Win-layout city");
        assert_eq!(loaded.mac_rci_counter(7), 36);
        assert_eq!(
            [
                loaded.misc_u32_at(0x05f8).unwrap(),
                loaded.misc_u32_at(0x0600).unwrap(),
                loaded.misc_u32_at(0x0608).unwrap(),
            ],
            pe_counts
        );
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn mac_growth_scan_updates_arcology_xmic_offset_with_native_gates_and_formula() {
        let mut city =
            City::new_blank("/tmp/mac-arcology-xmic-offset.sc2", "Arcology").expect("blank city");
        city.kind = CityFileKind::MacCity;

        let mut land = vec![0; CITY_TILES];
        let mut crime = vec![0; CITY_TILES];
        let mut pollution = vec![0; CITY_TILES];
        for (x, y) in [(20, 24), (24, 24), (28, 24), (32, 24), (36, 24), (40, 24)] {
            let index = x * CITY_HEIGHT + y;
            // 3 + (12 - 7) - 1 = 7 before XBIT reductions.
            land[index] = 96;
            crime[index] = 224;
            pollution[index] = 32;
        }
        city.sync_land_value_from_slice(&land);
        city.sync_crime_density_from_slice(&crime);
        city.sync_pollution_density_from_slice(&pollution);

        for (x, xtxt, xmic_class, bits, zone, building, initial) in [
            // XMIC class need only be in the arcology range; native code does
            // not require it to equal this tile's XBLD byte.
            (20, 0x34, 0xfc, 0x00, 0x80, 0xfb, 99),
            (24, 0x35, 0xfd, 0x40, 0x80, 0xfc, 99),
            (28, 0x36, 0xfe, 0x50, 0x80, 0xfd, 99),
            // Wrong XMIC class, wrong XZON high nibble, and wrong XBLD class.
            (32, 0x37, 0xfa, 0x50, 0x80, 0xfb, 88),
            (36, 0x38, 0xfb, 0x50, 0x10, 0xfb, 88),
            (40, 0x39, 0xfb, 0x50, 0x80, 0xfa, 88),
        ] {
            city.set_building(x, 24, building);
            city.set_zone(x, 24, zone);
            city.set_text_overlay(x, 24, xtxt);
            city.set_tile_bits(x, 24, TileBits::new(bits));
            let slot = usize::from(xtxt - 0x33);
            assert!(city.set_microsim_tile_id(slot, xmic_class));
            assert!(city.set_microsim_stat0(slot, initial));
        }

        let mut rng = MacToolboxRng::new(0x1357_2468);
        mac_budget_accumulate_monthly_rci_scan_partition(&mut city, 0, 0, &mut rng);

        assert_eq!(city.microsim_at(1).map(|row| row.stat0), Some(1));
        assert_eq!(city.microsim_at(2).map(|row| row.stat0), Some(3));
        assert_eq!(city.microsim_at(3).map(|row| row.stat0), Some(7));
        assert_eq!(city.microsim_at(4).map(|row| row.stat0), Some(88));
        assert_eq!(city.microsim_at(5).map(|row| row.stat0), Some(88));
        assert_eq!(city.microsim_at(6).map(|row| row.stat0), Some(88));
    }
}

#[cfg(test)]
mod mac_rail_scan_tests {
    use super::*;
    use crate::city::CityFileKind;
    use crate::mac_random::MacGameLcgRng;

    #[test]
    fn live_mac_monthly_scan_spawns_rail_with_shared_game_lcg_on_both_layouts() {
        for kind in [CityFileKind::MacCity, CityFileKind::WinSc2] {
            let mut city =
                City::new_blank("/tmp/mac-rail-scan.city", "Rail scan").expect("blank city");
            city.kind = kind;
            if kind == CityFileKind::WinSc2 {
                city.prepare_mac_fidelity_from_saved_city();
            }

            // Two rail stations satisfy the saved XBLD census / 4 producer
            // gate. This one is in residue partition (1, 0); the other only
            // contributes to the census. The west candidate and its route
            // reproduce a positive station-search fixture case.
            city.set_building(59, 64, 0x2c);
            city.set_building(60, 64, 0x2c);
            city.set_building(61, 64, 0xed);
            city.set_building(30, 30, 0xed);
            city.set_building(34, 30, 0xed);
            city.set_building(38, 30, 0xed);
            city.set_zone(61, 64, 0);
            city.set_tile_bits(61, 64, TileBits::new(0x40));
            assert_eq!(city.mac_xbld_tile_count(0xed) / 4, 1, "{kind:?}");

            let mut quickdraw = MacToolboxRng::new(0x1234_5678);
            let mut game_rng = MacGameLcgRng::new(MacGameLcgRng::INITIAL_STATE);
            let mut kind2_sound_deadline = 0;
            mac_budget_accumulate_monthly_rci_scan_partition_with_game_rng(
                &mut city,
                1,
                0,
                &mut quickdraw,
                &mut game_rng,
                &mut kind2_sound_deadline,
            );

            assert_eq!(city.mac_rail_actor_counter, 1, "{kind:?}");
            assert_eq!(game_rng.state(), 0x41c6_7ea6, "{kind:?}");
            assert_eq!(city.text_overlay_at(60, 64), Some(0xca), "{kind:?}");
            let records = city.traffic_overlays().expect("Mac actor records");
            assert_eq!(records[1].kind, 10, "{kind:?}");
            assert_eq!(records[records[1].state as usize].kind, 11, "{kind:?}");
            assert_eq!(
                records[records[records[1].state as usize].state as usize].kind, 11,
                "{kind:?}"
            );
        }
    }
}
