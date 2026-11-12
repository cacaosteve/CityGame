//! Shared power/water service helpers and module exports.

use super::*;

mod pipes;
mod power;
mod water;
pub(super) use pipes::*;
pub(super) use power::*;
pub(super) use water::*;

/// PE power-line tool post-place @ `0x43f396`–`0x43f3aa`: subtype `[0x4c9c56]==0` success
/// and `dwCityPopulation < 0xc350` → `FUN_00467860`, including the PE source and queue scans.
pub(super) fn pe_powerline_post_place_power_recalc_43f396(city: &mut City, crt: &mut Win95CrtRand) {
    if city.pe_dw_city_population() >= 0xc350 {
        return;
    }
    pe_fun_00467860(city, crt);
}

/// PE pipe-tool post-place @ `0x43f419`–`0x43f42f`: subtype `[0x4c9c58]==0` success and
/// `dwCityPopulation < 0xc350` → `FUN_00464550`, with the saved-XBIT source and queue scans.
pub(super) fn pe_pipetool_post_place_water_recalc_43f419(city: &mut City) {
    if city.pe_dw_city_population() >= 0xc350 {
        return;
    }
    pe_fun_00464550(city);
}

pub(super) fn service_percent(served: u32, demand: u32) -> u32 {
    if demand == 0 {
        100
    } else {
        served
            .saturating_mul(100)
            .checked_div(demand)
            .unwrap_or(0)
            .min(100)
    }
}

const PE_NETWORK_QUEUE_CAPACITY: usize = 0x200;

fn pe_network_queue_push(
    queue: &mut std::collections::VecDeque<(usize, usize)>,
    coordinate: (usize, usize),
) {
    if queue.len() >= PE_NETWORK_QUEUE_CAPACITY {
        let _ = queue.pop_front();
    }
    queue.push_back(coordinate);
}

/// Win95 `dwWaterUsedPercentage`: watered consumers × 100 / supply capacity (100 if no capacity).
pub(super) fn service_utilization_percent(used: u32, capacity: u32) -> u32 {
    if capacity == 0 {
        100
    } else {
        used.saturating_mul(100)
            .checked_div(capacity)
            .unwrap_or(0)
            .min(100)
    }
}

#[cfg(test)]
mod pe_network_queue_tests {
    use super::*;
    use crate::mac_random::MacToolboxRng;
    use crate::win95_random::Win95CrtRand;
    use std::collections::VecDeque;

    #[test]
    fn pe_power_worker_reads_ordinance_from_saved_file_layout() {
        let powered_line = |native_ordinance_mask: u32, mac_zone7_decoy: u32| {
            let mut city = City::new_blank(
                "/tmp/sc2000-pe-power-ordinance-layout.sc2",
                "PePowerOrdinance",
            )
            .expect("blank city");
            city.kind = crate::city::CityFileKind::MacCity;
            city.set_mac_state_misc_u32_at(0x0fa0, native_ordinance_mask);
            city.set_mac_state_misc_u32_at(0x0fa8, mac_zone7_decoy);
            city.set_building(10, 20, 198); // PE fixed 40-unit coal source.
            city.set_tile_bits(10, 20, TileBits::new(0x80));
            for x in 11..=54 {
                city.set_building(x, 20, 0x70);
                city.set_tile_bits(x, 20, TileBits::new(0x80));
            }

            let mut crt = Win95CrtRand::new(1);
            pe_fun_00467860(&mut city, &mut crt);
            (10..=54)
                .filter(|&x| city.tile_bits_at(x, 20).is_some_and(|bits| bits.powered()))
                .count()
        };

        assert_eq!(
            powered_line(1 << 16, 0),
            43,
            "native Mac Energy Conservation choice contributes three PE units"
        );
        assert_eq!(
            powered_line(0, 1 << 16),
            40,
            "Mac's first zone-7 census word must not activate the PE ordinance"
        );
    }

    #[test]
    fn pe_power_source_scan_revisits_unpowered_source_in_shared_component() {
        let mut city = City::new_blank(
            "/tmp/sc2000-pe-power-repeated-source.sc2",
            "PePowerRepeatedSource",
        )
        .expect("blank city");
        city.pe_b_weather_humid = 30; // Solar span 7: (100 - 30) / 10.

        // The x-major scan starts at the first solar source. The connected
        // component has 16 consumers between that root and the second source.
        // With seed 1, the first two solar draws yield 11 + 6 = 17 units, so
        // the second walk marks the root and consumers but exhausts its budget
        // before it reaches the second source. The later outer scan therefore
        // enters that still-unmarked source and floods the same component again.
        city.set_building(10, 10, 204); // XBLD 0xCC, solar.
        city.set_tile_bits(10, 10, TileBits::new(0x80));
        for y in 11..=26 {
            city.set_building(10, y, 0x70);
            city.set_tile_bits(10, y, TileBits::new(0x80));
        }
        city.set_building(10, 27, 204);
        city.set_tile_bits(10, 27, TileBits::new(0x80));

        let mut crt = Win95CrtRand::new(1);
        pe_fun_00467860(&mut city, &mut crt);

        // The second root re-evaluates both dynamic sources with draws 3 and 4
        // (11 + 10 units). Two source scans therefore consume four CRT draws.
        assert_eq!(crt.seed(), 3_884_216_597);
        assert_eq!(city.pe_power_used_percent(), 84); // 32 served / 38 raw.
        assert!(city.tile_bits_at(10, 10).unwrap().powered());
        assert!(city.tile_bits_at(10, 24).unwrap().powered());
        assert!(city.tile_bits_at(10, 27).unwrap().powered());
    }

    #[test]
    fn pe_power_source_reached_by_prior_scan_is_not_scanned_twice() {
        let mut city = City::new_blank(
            "/tmp/sc2000-pe-power-covered-source.sc2",
            "PePowerCoveredSource",
        )
        .expect("blank city");
        city.pe_b_weather_humid = 30;

        // The first x-major root has enough fixed capacity to publish XBIT 0x40
        // across this whole component. Its first pass also visits the later
        // solar plant, so that plant's one dynamic draw belongs to this scan;
        // the outer source loop must then skip it as already powered.
        for y in 10..=12 {
            for x in 10..=127 {
                city.set_building(x, y, 0x70);
                city.set_tile_bits(x, y, TileBits::new(0x80));
            }
        }
        city.set_building(10, 10, 206); // XBLD 0xCE, fixed 555 PE units.
        city.set_building(10, 11, 204); // XBLD 0xCC, dynamic solar.

        let mut expected_crt = Win95CrtRand::new(1);
        let _ = expected_crt.random_u16();
        let expected_seed = expected_crt.seed();

        let mut crt = Win95CrtRand::new(1);
        pe_fun_00467860(&mut city, &mut crt);

        assert_eq!(
            crt.seed(),
            expected_seed,
            "solar source draws once in the first flood"
        );
        assert_eq!(city.pe_power_used_percent(), 62); // 352 served / 560..566 raw.
        for y in 10..=12 {
            for x in 10..=127 {
                assert!(
                    city.tile_bits_at(x, y).unwrap().powered(),
                    "first source should power the complete component at ({x},{y})"
                );
            }
        }
    }

    #[test]
    fn mac_power_dynamic_sources_follow_native_word_division_order() {
        let mut city =
            City::new_blank("/tmp/sc2000-mac-power-source-terms.sc2", "MacPower").unwrap();
        city.kind = crate::city::CityFileKind::MacCity;
        city.set_mac_state_misc_u32_at(0x64, 15); // F085: (15 >> 3) + 1 = 2
        city.set_mac_state_misc_u32_at(0x68, 15); // F086: (100 - 15) / 10 = 8
        let mut xval = vec![0_u8; CITY_TILES];
        xval[40 * CITY_WIDTH + 40] = 0x9f;
        city.sync_land_value_from_slice(&xval);
        city.set_altitude(40, 40, 27, 0);

        let mut c8_rng = MacToolboxRng::new(1);
        // Native CODE 14 reads ALTM through A5-0xebc, not the XVAL decoy.
        // Random(1) returns 16807, whose low WORD is 16807; 16807 % 2 = 1.
        assert_eq!(mac_power_c8_units(&city, 40, 40, &mut c8_rng), 14);
        assert_eq!(c8_rng.seed(), 16_807);

        let mut cc_rng = MacToolboxRng::new(1);
        assert_eq!(mac_power_cc_units(&city, &mut cc_rng), Some(12));
        assert_eq!(cc_rng.seed(), 16_807);
    }

    #[test]
    fn mac_power_fixed_sources_follow_code14_jump_table() {
        assert_eq!(mac_power_fixed_units_for_building(0xC6), Some(40));
        assert_eq!(mac_power_fixed_units_for_building(0xC7), Some(40));
        assert_eq!(mac_power_fixed_units_for_building(0xC9), Some(11));
        assert_eq!(mac_power_fixed_units_for_building(0xCA), Some(48));
        assert_eq!(mac_power_fixed_units_for_building(0xCB), Some(111));
        assert_eq!(mac_power_fixed_units_for_building(0xCD), Some(355));
        assert_eq!(mac_power_fixed_units_for_building(0xCE), Some(555));
        assert_eq!(mac_power_fixed_units_for_building(0xCF), Some(44));
        assert_eq!(mac_power_fixed_units_for_building(0xC8), None);
        assert_eq!(mac_power_fixed_units_for_building(0xCC), None);
    }

    #[test]
    fn mac_power_load_census_matches_the_full_xbld_byte_domain() {
        for building in u8::MIN..=u8::MAX {
            let expected = building > 0x6f && !(0xc6..=0xcf).contains(&building);
            assert_eq!(
                mac_power_load_xbld(building),
                expected,
                "XBLD {building:#04x} load classification"
            );
        }
    }

    #[test]
    fn mac_power_cc_preserves_native_zero_divisor_boundary() {
        let mut city = City::new_blank("/tmp/sc2000-mac-power-zero-span.sc2", "MacPower").unwrap();
        city.kind = crate::city::CityFileKind::MacCity;
        city.set_mac_state_misc_u32_at(0x68, 95);

        let mut rng = MacToolboxRng::new(1);
        assert_eq!(mac_power_cc_units(&city, &mut rng), None);
        assert_eq!(rng.seed(), 16_807, "the native draw happens before DIVU");
    }

    #[test]
    fn mac_power_worker_rebuilds_capacity_limited_power_bits() {
        let mut city = City::new_blank("/tmp/sc2000-mac-power-worker.sc2", "MacPower").unwrap();
        city.kind = crate::city::CityFileKind::MacCity;
        for (x, building) in [(10, 0xC6), (11, 0x70), (12, 0x71)] {
            city.set_building(x, 20, building);
            city.set_tile_bits(x, 20, TileBits::new(0).with_powerable(true));
        }
        // A disconnected, non-powerable structure is not admitted by the Mac
        // XBIT 0x80 worker even though it is cardinally adjacent.
        city.set_building(13, 20, 0x72);

        let mut rng = MacToolboxRng::new(1);
        assert_eq!(mac_export_218_power_with_rng(&mut city, &mut rng), Some(5));
        for x in 10..=12 {
            let bits = city.tile_bits_at(x, 20).unwrap();
            assert!(bits.powered(), "native second pass marks powered tile {x}");
            assert_eq!(bits.raw_bits() & 0x08, 0, "scratch bit cleared at {x}");
        }
        assert!(!city.tile_bits_at(13, 20).unwrap().powered());
    }

    #[test]
    fn mac_plant_power_uses_xbld_source_without_xmic_row() {
        let mut city =
            City::new_blank("/tmp/sc2000-mac-plant-source-no-xmic.sc2", "MacPlant").unwrap();
        city.kind = crate::city::CityFileKind::MacCity;
        city.set_building(10, 20, 0xC9);
        city.set_tile_bits(10, 20, TileBits::new(0x80));
        for x in 11..=20 {
            city.set_building(x, 20, 0x70);
            city.set_tile_bits(x, 20, TileBits::new(0x80));
        }
        assert!((0..MICROSIM_COUNT).all(|index| {
            city.microsim_at(index)
                .is_none_or(|record| record.tile_id != 0xC9)
        }));

        let mut rng = MacToolboxRng::new(1);
        assert_eq!(mac_export_218_power_with_rng(&mut city, &mut rng), Some(90));
        assert!((10..=20).all(|x| city.tile_bits_at(x, 20).unwrap().powered()));
    }

    #[test]
    fn mac_power_worker_consumes_dynamic_source_draws_in_x_major_component_order() {
        let mut city =
            City::new_blank("/tmp/sc2000-mac-power-draw-order.sc2", "MacPowerDrawOrder").unwrap();
        city.kind = crate::city::CityFileKind::MacCity;
        city.set_mac_state_misc_u32_at(0x68, 20); // F086: CC divisor 8.

        // The first CC component has the smaller x coordinate, so the native
        // x-major root scan consumes the first draw despite its larger y.
        // Its capacity is 12 (16807 % 8); the later component gets capacity 6
        // (15089 % 8). Reversing visitation gives only 7 of 11 consumers
        // service instead of 11, changing the summary from 61% to 38%.
        for (x, y, consumers) in [(10, 40, 10), (20, 10, 1)] {
            city.set_building(x, y, 0xCC);
            city.set_tile_bits(x, y, TileBits::new(0x80));
            for offset in 1..=consumers {
                city.set_building(x + offset, y, 0x70);
                city.set_tile_bits(x + offset, y, TileBits::new(0x80));
            }
        }

        let mut rng = MacToolboxRng::new(1);
        assert_eq!(mac_export_218_power_with_rng(&mut city, &mut rng), Some(61));
        assert_eq!(rng.seed(), 282_475_249, "one draw per dynamic source");
        for x in 11..=20 {
            assert!(city.tile_bits_at(x, 40).unwrap().powered());
        }
        assert!(city.tile_bits_at(21, 10).unwrap().powered());
    }

    #[test]
    fn mac_power_worker_without_sources_clears_transient_bits_and_reports_hundred() {
        let mut city = City::new_blank("/tmp/sc2000-mac-power-no-sources.sc2", "MacPower").unwrap();
        city.kind = crate::city::CityFileKind::MacCity;
        city.set_tile_bits(20, 20, TileBits::new(0x48 | 0x80));
        let mut rng = MacToolboxRng::new(1);

        assert_eq!(
            mac_export_218_power_with_rng(&mut city, &mut rng),
            Some(100)
        );
        assert_eq!(city.tile_bits_at(20, 20).unwrap().raw_bits(), 0x80);
    }

    #[test]
    fn mac_power_summary_keeps_ordinance_bonus_above_one_hundred_percent() {
        let mut city =
            City::new_blank("/tmp/sc2000-mac-power-ordinance-percent.sc2", "MacPower").unwrap();
        city.kind = crate::city::CityFileKind::MacCity;
        city.set_building(10, 20, 0xC6); // 40 raw units, +3 ordinance units.
        city.set_tile_bits(10, 20, TileBits::new(0x80));
        for x in 11..=52 {
            city.set_building(x, 20, 0x70);
            city.set_tile_bits(x, 20, TileBits::new(0x80));
        }
        // Bit 0 is Sales Tax and the old +0x010c read is an unrelated state
        // slot; neither enables the Energy Conservation power bonus.
        city.set_mac_state_misc_u32_at(0x0fa0, 1);
        city.set_mac_state_misc_u32_at(0x010c, 1);
        let mut rng = MacToolboxRng::new(1);
        assert_eq!(
            mac_export_218_power_with_rng(&mut city, &mut rng),
            Some(100)
        );

        // Native BTST #0 at ffff eff3 is bit 16 of the big-endian ordinance
        // mask rooted at ffff eff2 / saved MISC slot 1000.
        city.set_mac_state_misc_u32_at(0x0fa0, 1 << 16);
        let mut rng = MacToolboxRng::new(1);

        assert_eq!(
            mac_export_218_power_with_rng(&mut city, &mut rng),
            Some(105)
        );
    }

    #[test]
    fn mac_power_worker_preserves_native_draw_before_zero_divisor_trap() {
        let mut city =
            City::new_blank("/tmp/sc2000-mac-power-worker-trap.sc2", "MacPower").unwrap();
        city.kind = crate::city::CityFileKind::MacCity;
        city.set_mac_state_misc_u32_at(0x68, 95);
        city.set_building(20, 20, 0xCC);
        city.set_tile_bits(20, 20, TileBits::new(0x48 | 0x80));
        let before = city.tile_bits_at(20, 20).unwrap();
        let mut rng = MacToolboxRng::new(1);

        assert_eq!(mac_export_218_power_with_rng(&mut city, &mut rng), None);
        assert_eq!(city.tile_bits_at(20, 20), Some(before));
        assert_eq!(rng.seed(), 16_807, "the trapping source consumes Random");
    }

    #[test]
    fn pe_network_queue_push_discards_oldest_at_512_pending() {
        // This shared VecDeque helper bounds geometry-derived scans separately
        // from the native PE ring's 511-pending-entry contract.
        let mut queue = VecDeque::new();
        for i in 0..PE_NETWORK_QUEUE_CAPACITY {
            pe_network_queue_push(&mut queue, (i, 0));
        }
        assert_eq!(queue.len(), PE_NETWORK_QUEUE_CAPACITY);
        assert_eq!(queue.front(), Some(&(0, 0)));
        pe_network_queue_push(&mut queue, (0x200, 1));
        assert_eq!(queue.len(), PE_NETWORK_QUEUE_CAPACITY);
        assert_eq!(queue.front(), Some(&(1, 0)));
        assert_eq!(queue.back(), Some(&(0x200, 1)));
    }

    #[test]
    fn power_queue_push_discards_oldest_at_511_pending() {
        let mut queue = VecDeque::new();
        for i in 0..POWER_QUEUE_PENDING_CAPACITY {
            power_queue_push_511(&mut queue, (i, 0));
        }
        assert_eq!(queue.len(), 0x1ff);
        assert_eq!(queue.front(), Some(&(0, 0)));
        power_queue_push_511(&mut queue, (0x200, 1));
        assert_eq!(queue.len(), 0x1ff);
        assert_eq!(queue.front(), Some(&(1, 0)));
        assert_eq!(queue.back(), Some(&(0x200, 1)));
    }

    #[test]
    fn pe_power_first_pass_overflow_drops_reachable_cells_like_native_ring() {
        let mut city =
            City::new_blank("/tmp/pe-power-ring-overflow.sc2", "PePowerRing").expect("blank city");
        for y in 0..0x80 {
            for x in 0..0x80 {
                city.set_building(x, y, 0x70);
                // A deterministic 4/5-connected field, with enough enclosed
                // gaps to keep unvisited coordinates queued more than once.
                let powerable = (x * 17 + y * 31 + x * y * 3) % 5 != 0;
                city.set_tile_bits(x, y, TileBits::new(if powerable { 0x80 } else { 0 }));
            }
        }
        // XBLD 205 is fixed-output, so this walk does not consume CRT.
        city.set_building(64, 64, 205);
        city.set_tile_bits(64, 64, TileBits::new(0x80));

        let mut crt = Win95CrtRand::new(1);
        let (supply, consumers) = pe_power_first_pass(&mut city, (64, 64), &mut crt);

        // The 0x200-slot native ring can hold 511 pending coordinates; its
        // colliding enqueue advances the read cursor and drops the oldest.
        // In this rooted map that loses a small reachable tail, including the
        // asserted coordinate. An unbounded BFS would reach 13,758 cells.
        assert_eq!(supply, 0x163);
        assert_eq!(consumers, 13_741);
        assert!(
            city.tile_bits_at(64, 64)
                .is_some_and(|bits| bits.raw_bits() & 0x08 != 0)
        );
        assert!(
            city.tile_bits_at(67, 122)
                .is_some_and(|bits| bits.raw_bits() & 0x08 == 0)
        );
    }

    #[test]
    fn mac_power_first_pass_overflow_drops_reachable_cells_like_native_ring() {
        let mut city = City::new_blank("/tmp/mac-power-ring-overflow.sc2", "MacPowerRing")
            .expect("blank city");
        for y in 0..0x80 {
            for x in 0..0x80 {
                city.set_building(x, y, 0x70);
                let powerable = (x * 17 + y * 31 + x * y * 3) % 5 != 0;
                city.set_tile_bits(x, y, TileBits::new(if powerable { 0x80 } else { 0 }));
            }
        }
        // Mac XBLD 0xCD is a fixed 355-unit source, so this case leaves the
        // QuickDraw stream untouched while exercising the same native ring.
        city.set_building(64, 64, 0xCD);
        city.set_tile_bits(64, 64, TileBits::new(0x80));

        let mut rng = MacToolboxRng::new(1);
        let (capacity, consumers) =
            mac_power_first_pass(&mut city, (64, 64), &mut rng).expect("fixed source cannot trap");

        assert_eq!(capacity, 355);
        assert_eq!(consumers, 13_741);
        assert_eq!(rng.seed(), 1, "a fixed-capacity source uses no Random draw");
        assert!(
            city.tile_bits_at(64, 64)
                .is_some_and(|bits| bits.raw_bits() & 0x08 != 0)
        );
        assert!(
            city.tile_bits_at(67, 122)
                .is_some_and(|bits| bits.raw_bits() & 0x08 == 0)
        );
    }

    #[test]
    fn pe_water_first_worker_marks_and_skips_later_connected_source() {
        let mut city =
            City::new_blank("/tmp/pe-water-shared-sources.sc2", "PeWaterShared").unwrap();
        city.set_misc_u32_at(0x0e40, 8); // Each dry pump contributes 40 PE units.

        for (x, building) in [(10, 220), (11, 0x70), (12, 220)] {
            city.set_building(x, 20, building);
            city.set_tile_bits(
                x,
                20,
                TileBits::new(0)
                    .with_piped(true)
                    .with_powered(building == 220),
            );
        }

        let maps = water_service_maps_with_entry_seed_and_fidelity(&city, 1, EngineFidelity::Win95);

        assert!(maps.pe_worker_contract_complete);
        assert_eq!(maps.pe_worker_capacity_units, 80);
        assert!(maps.watered_tiles[20 * CITY_WIDTH + 10]);
        assert!(maps.watered_tiles[20 * CITY_WIDTH + 11]);
        assert!(maps.watered_tiles[20 * CITY_WIDTH + 12]);
    }

    #[test]
    fn pe_water_first_pass_overflow_drops_reachable_cells_like_native_ring() {
        let mut city =
            City::new_blank("/tmp/pe-water-ring-overflow.sc2", "PeWaterRing").expect("blank city");
        for y in 0..0x80 {
            for x in 0..0x80 {
                let piped = (x * 17 + y * 31 + x * y * 3) % 2 != 0 || (x, y) == (64, 64);
                city.set_tile_bits(x, y, TileBits::new(if piped { 0x20 } else { 0 }));
            }
        }

        let mut work = vec![false; CITY_TILES];
        let order = saved_xbit_water_first_pass(&city, (64, 64), &mut work);

        // An unbounded connectivity walk confirms the selected tail cell is
        // reachable in the saved plane; its omission is caused by ring loss.
        let mut reachable = vec![false; CITY_TILES];
        let mut pending: VecDeque<(usize, usize)> = VecDeque::from([(64, 64)]);
        reachable[64 * CITY_WIDTH + 64] = true;
        while let Some((x, y)) = pending.pop_front() {
            for (nx, ny) in [
                (x, y.wrapping_sub(1)),
                (x.wrapping_sub(1), y),
                (x, y + 1),
                (x + 1, y),
            ] {
                if nx >= 0x80 || ny >= 0x80 {
                    continue;
                }
                let index = ny * CITY_WIDTH + nx;
                if !reachable[index] && city.tile_bits_at(nx, ny).is_some_and(TileBits::piped) {
                    reachable[index] = true;
                    pending.push_back((nx, ny));
                }
            }
        }

        // This connected saved-XBIT pattern overflows the native 0x200-slot
        // ring. The 511-pending drop-oldest walk loses this reachable tail;
        // an unbounded flood visits 12,289 cells instead of 11,729.
        assert_eq!(order.len(), 11_805);
        assert_eq!(reachable.iter().filter(|&&visited| visited).count(), 12_289);
        assert!(reachable[66 * CITY_WIDTH + 107]);
        assert!(city.tile_bits_at(107, 66).is_some_and(TileBits::piped));
        assert!(!work[66 * CITY_WIDTH + 107]);
        assert!(work[64 * CITY_WIDTH + 64]);

        // The worker then performs a second bounded FIFO over the transient
        // marks. It reaches the same number of marked cells, but its order can
        // differ because pass two enqueues only marked cells while pass one
        // also lets non-piped neighbors occupy the bounded ring. Both PE and
        // Mac wrappers use this shared pass.
        let service_order = saved_xbit_water_second_pass((64, 64), &mut work);
        assert_eq!(service_order.len(), 11_805);
        assert_eq!(service_order.first(), Some(&(64, 64)));
        assert_ne!(service_order, order);
        assert!(!work.iter().any(|&marked| marked));
        assert!(!service_order.contains(&(107, 66)));
    }

    #[test]
    fn pe_water_worker_uses_second_pass_order_after_first_pass_ring_overflow() {
        let mut city = City::new_blank("/tmp/pe-water-second-pass-overflow.sc2", "PeWaterRing")
            .expect("blank city");
        for y in 0..0x80 {
            for x in 0..0x80 {
                let piped = (x * 17 + y * 31 + x * y * 3) % 2 != 0 || (x, y) == (64, 64);
                city.set_building(x, y, if piped { 0x70 } else { 0 });
                city.set_tile_bits(x, y, TileBits::new(0).with_piped(piped));
            }
        }
        // Blank-city water-table height supplies 40 units per Pump. Seventy-
        // two powered Pumps put the budget just beyond the queue-order split:
        // this makes pass two's order visible in the served-cell boundary.
        let mut pumps = 1;
        city.set_building(64, 64, 220);
        city.set_tile_bits(64, 64, TileBits::new(0).with_piped(true).with_powered(true));
        for x in 65..0x80 {
            for y in 0..0x80 {
                let piped = (x * 17 + y * 31 + x * y * 3) % 2 != 0;
                if !piped {
                    continue;
                }
                city.set_building(x, y, 220);
                city.set_tile_bits(x, y, TileBits::new(0).with_piped(true).with_powered(true));
                pumps += 1;
                if pumps == 72 {
                    break;
                }
            }
            if pumps == 72 {
                break;
            }
        }
        assert_eq!(pumps, 72);

        let maps = water_service_maps_with_entry_seed_and_fidelity(&city, 1, EngineFidelity::Win95);
        let served: Vec<_> = maps
            .watered_tiles
            .iter()
            .enumerate()
            .filter_map(|(index, watered)| {
                (*watered
                    && pe_water_generic_counter_tile(
                        city.building_at(index % CITY_WIDTH, index / CITY_WIDTH),
                    ))
                .then_some((index % CITY_WIDTH, index / CITY_WIDTH))
            })
            .collect();

        assert!(maps.pe_worker_contract_complete);
        assert_eq!(maps.pe_worker_capacity_units, 2_880);
        assert_eq!(served.len(), 2_880);
        assert!(served.contains(&(63, 21)));
        assert!(!served.contains(&(91, 81)));
    }

    #[test]
    fn water_overflow_second_root_reads_the_live_tower_marker() {
        for (tower, expected_plane) in
            [(0x30, 0x75f8_5e75_4b54_3c75), (0x70, 0xf452_0fc2_d24e_7035)]
        {
            let mut city = City::new_blank("/tmp/pe-water-second-root.sc2", "Second root").unwrap();
            city.set_mac_state_misc_u32_at(0x0e40, 0);
            city.pe_b_weather_humid = 2;
            for x in 0..128 {
                for y in 0..128 {
                    if (x * 17 + y * 31 + x * y * 3) % 2 != 0 {
                        city.set_building(x, y, 0x70);
                        city.set_tile_bits(x, y, TileBits::new(0x20));
                    }
                }
            }
            for (x, y) in [(64, 64), (107, 66)] {
                city.set_building(x, y, 0xdc);
                city.set_tile_bits(x, y, TileBits::new(0x60));
            }
            city.set_building(65, 64, 0xeb);
            city.set_tile_bits(65, 64, TileBits::new(tower));

            for fidelity in [EngineFidelity::Win95, EngineFidelity::Mac68k] {
                let mut city = city.clone();
                if fidelity.uses_win95() {
                    let maps = pe_fun_00464550(&mut city);
                    assert_eq!(maps.pe_worker_capacity_units, 103);
                    assert_eq!(maps.pe_worker_served_cells, 103);
                    assert_eq!(city.pe_water_coverage_percent(), 100);
                    assert_eq!(city.pe_w_sewer_bonus(), 0);
                } else {
                    city.kind = crate::city::CityFileKind::MacCity;
                    city.set_mac_state_misc_u32_at(0x68, 2);
                    city.set_mac_selected_water_height(0);
                    let maps = water_service_maps_with_entry_seed_and_fidelity(&city, 1, fidelity);
                    assert_eq!(maps.mac_worker_capacity_units, 103);
                    assert_eq!(maps.mac_worker_served_cells, 103);
                    assert_eq!(maps.mac_worker_utilization_percent(), Some(100));
                    crate::app::power_water_refresh::publish_mac_water_result_for_city(
                        &mut city, &maps,
                    );
                }
                // Original output is captured in first-coordinate row-table order.
                let mut fingerprint = 0xcbf2_9ce4_8422_2325u64;
                for x in 0..128 {
                    for y in 0..128 {
                        let byte = city.tile_bits_at(x, y).unwrap().raw_bits();
                        fingerprint =
                            (fingerprint ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3);
                    }
                }
                assert_eq!(fingerprint, expected_plane, "{fidelity:?}");
            }
        }
    }

    #[test]
    fn pe_water_second_pass_drains_a_full_128_plane_without_losing_marks() {
        // Unlike pass one, pass two enqueues only coordinates carrying the
        // transient work mark. Stress its largest possible saved-plane input
        // independently of the first-pass ring-loss pattern.
        let mut work = vec![false; CITY_TILES];
        for y in 0..0x80 {
            for x in 0..0x80 {
                work[y * CITY_WIDTH + x] = true;
            }
        }

        let order = saved_xbit_water_second_pass((64, 64), &mut work);

        assert_eq!(order.len(), 0x80 * 0x80);
        assert_eq!(order.first(), Some(&(64, 64)));
        assert_eq!(&order[1..5], &[(64, 63), (63, 64), (64, 65), (65, 64)]);
        assert!(!work.iter().any(|&marked| marked));
    }

    #[test]
    fn mac_water_fifo_uses_first_powered_pump_in_rotation_scan() {
        // Different X and Y coordinates distinguish the native outer/inner
        // axes. With X as row index and Y as byte offset, export_209 scans
        // `(y,x)`, `(x,reverse-y)`, `(reverse-y,reverse-x)`, then
        // `(reverse-x,y)` for selectors 0..3.
        let pumps = [(40, 42), (41, 40)];
        let component = vec![pumps[0], (40, 41), (41, 41), pumps[1]];
        let expected_roots = [pumps[1], pumps[0], pumps[0], pumps[1]];

        for (rotation, expected_root) in expected_roots.into_iter().enumerate() {
            let mut city =
                City::new_blank("/tmp/sc2000-mac-water-roots.sc2", "water").expect("blank city");
            city.kind = CityFileKind::MacCity;
            city.saved_rotation = rotation as u8;
            // The original selector belongs to the saved Mac map orientation;
            // changing CityGame's view compass must not change its scan root.
            city.set_view_rotation((rotation as u8 + 1) & 3);
            for &(x, y) in &component {
                city.set_tile_bits(
                    x,
                    y,
                    TileBits::new(0)
                        .with_piped(true)
                        .with_powered(pumps.contains(&(x, y))),
                );
            }
            for &(x, y) in &pumps {
                city.set_building(x, y, 220);
            }

            let order = mac_water_allocation_order(&city, &component)
                .expect("exact, powered Pump-only XBIT component");
            assert_eq!(order[0], expected_root, "rotation {rotation}");
            assert_eq!(order.len(), component.len(), "rotation {rotation}");
        }
    }

    #[test]
    fn mac_water_serves_positive_y_before_positive_x_when_capacity_runs_out() {
        let mut city = City::new_blank("/tmp/mac-water-fork.city", "Water").unwrap();
        city.kind = CityFileKind::MacCity;
        city.set_mac_state_misc_u32_at(0x68, 2);
        city.set_mac_selected_water_height(0);
        for (x, y, building, bits) in [
            (10, 10, 0xdc, 0x60),
            (10, 11, 0x70, 0x20),
            (11, 10, 0x71, 0x20),
        ] {
            city.set_building(x, y, building);
            city.set_tile_bits(x, y, TileBits::new(bits));
        }
        let maps =
            water_service_maps_with_entry_seed_and_fidelity(&city, 1, EngineFidelity::Mac68k);
        assert_eq!(maps.mac_worker_capacity_units, 1);
        assert!(maps.watered_tiles[11 * CITY_WIDTH + 10]);
        assert!(!maps.watered_tiles[10 * CITY_WIDTH + 11]);
    }

    #[test]
    fn mac_water_component_ignores_connected_unpowered_source_capacity() {
        let mut city =
            City::new_blank("/tmp/sc2000-mac-water-mixed-source.sc2", "water").expect("blank city");
        city.kind = CityFileKind::MacCity;
        city.set_building(40, 40, 220);
        city.set_building(41, 40, 0x70);
        city.set_building(42, 40, 220);
        for x in 40..=42 {
            city.set_underground(x, 40, 16);
            city.set_tile_bits(
                x,
                40,
                TileBits::new(0).with_piped(true).with_powered(x == 40),
            );
        }
        // The powered Pump contributes humidity / 2 = 2 source units. The
        // connected but unpowered Pump is reached by the flood, but its
        // native source arm skips its capacity contribution.
        city.set_misc_u32_at(0x68, 4);
        city.set_mac_state_misc_u32_at(912 * 4, 0);

        let maps =
            water_service_maps_with_entry_seed_and_fidelity(&city, 1, EngineFidelity::Mac68k);

        assert!(maps.mac_worker_contract_complete);
        assert_eq!(maps.mac_worker_capacity_units, 2);
        assert_eq!(maps.mac_worker_served_cells, 1);
        assert!(maps.watered_tiles[40 * CITY_WIDTH + 41]);
        assert!(!maps.watered_tiles[40 * CITY_WIDTH + 42]);
    }

    #[test]
    fn mac_water_pump_uses_zero_extended_humidity_byte() {
        let mut city = City::new_blank("/tmp/sc2000-mac-water-humidity-byte.sc2", "water")
            .expect("blank city");
        city.kind = CityFileKind::MacCity;
        city.set_mac_selected_water_height(0);
        city.set_building(40, 40, 220);
        city.set_tile_bits(40, 40, TileBits::new(0x60)); // Conductive + powered.
        city.set_building(41, 40, 0x70);
        city.set_tile_bits(41, 40, TileBits::new(0x20)); // Conductive generic cell.

        // CODE 14 starts D0 at zero, loads fffff086 with MOVE.B, and uses
        // DIVS.W #2. The byte is therefore zero-extended before division;
        // high-bit weather values still contribute positive half-values.
        for (humidity, expected_capacity) in [(0x7f, 63), (0x80, 64), (0xff, 127)] {
            city.set_mac_state_misc_u32_at(0x68, humidity);
            let maps =
                water_service_maps_with_entry_seed_and_fidelity(&city, 1, EngineFidelity::Mac68k);
            assert!(maps.mac_worker_contract_complete);
            assert_eq!(
                maps.mac_worker_capacity_units, expected_capacity,
                "humidity byte {humidity:#04x}"
            );
            assert_eq!(maps.mac_worker_served_cells, 1);
        }
    }

    #[test]
    fn mac_water_mixed_sources_combine_capacity_and_keep_special_markers_separate() {
        let mut city = City::new_blank("/tmp/sc2000-mac-water-mixed-workers.sc2", "water")
            .expect("blank city");
        city.kind = CityFileKind::MacCity;
        city.set_mac_selected_water_height(0);
        city.set_mac_state_misc_u32_at(0x68, 2); // Pump contributes humidity / 2 = 1.

        // A 3x3 saline neighborhood makes the surviving Desalination tile
        // contribute 180 units. XBIT's low water-kind bits coexist with the
        // saved 0x20 conductive and 0x40 powered worker bits.
        for y in 39..=41 {
            for x in 42..=44 {
                city.set_tile_bits(x, y, TileBits::new(0x05));
            }
        }

        let row = [
            (220, 0x60),  // Pump: conductive + powered.
            (0x50, 0x20), // Low-XBLD cell: served without using D3.
            (0x70, 0x25), // Developed generic cell and saline XBIT.
            (250, 0x65),  // Desalination: conductive + powered + saline.
            (0x70, 0x25), // Second developed generic cell.
            (235, 0x70),  // Tower: conductive + powered + previously served.
            (244, 0x60),  // Treatment: special powered served-bit arm.
        ];
        for (offset, (building, bits)) in row.into_iter().enumerate() {
            let x = 40 + offset;
            city.set_building(x, 40, building);
            city.set_tile_bits(x, 40, TileBits::new(bits));
        }

        let maps =
            water_service_maps_with_entry_seed_and_fidelity(&city, 1, EngineFidelity::Mac68k);

        // One unit from the Pump, 180 from Desalination, and 100 carried by
        // the previously served tower. The tower's separate refill pool is
        // bounded after two developed generic cells consume D3.
        assert!(maps.mac_worker_contract_complete);
        assert_eq!(maps.mac_worker_capacity_units, 281);
        assert_eq!(maps.mac_worker_served_cells, 2);
        assert_eq!(maps.mac_worker_utilization_percent(), Some(0));
        for x in 40..=46 {
            assert!(maps.watered_tiles[40 * CITY_WIDTH + x], "tile x={x}");
        }
    }

    #[test]
    fn mac_water_unpowered_only_pump_and_desalination_do_not_start_worker() {
        let mut city = City::new_blank("/tmp/sc2000-mac-water-unpowered-only.sc2", "water")
            .expect("blank city");
        city.kind = CityFileKind::MacCity;
        city.set_building(40, 40, 220);
        city.set_building(41, 40, 0x70);
        city.set_building(42, 40, 250);
        for x in 40..=42 {
            city.set_underground(x, 40, 16);
            city.set_tile_bits(x, 40, TileBits::new(0).with_piped(true));
        }

        // export_209 starts FUN_00026106 only from powered 0xDC/0xFA
        // source cells. A conductive component with no powered source has no
        // native worker root, even when it contains both source types.
        let maps =
            water_service_maps_with_entry_seed_and_fidelity(&city, 1, EngineFidelity::Mac68k);

        assert!(maps.mac_worker_contract_complete);
        assert_eq!(maps.mac_worker_capacity_units, 0);
        assert_eq!(maps.mac_worker_served_cells, 0);
        assert!(!maps.watered_tiles[40 * CITY_WIDTH + 40]);
        assert!(!maps.watered_tiles[40 * CITY_WIDTH + 41]);
        assert!(!maps.watered_tiles[40 * CITY_WIDTH + 42]);
        assert_eq!(maps.mac_worker_utilization_percent(), Some(100));
    }

    #[test]
    fn pe_water_worker_follows_saved_xbit_across_geometry_gap() {
        let mut city =
            City::new_blank("/tmp/sc2000-pe-water-xbit-gap.sc2", "water").expect("blank city");
        city.set_building(40, 40, 220);
        city.set_tile_bits(40, 40, TileBits::new(0).with_piped(true).with_powered(true));
        // The saved membership plane joins these cells, although no XUND is
        // present at the middle cell and the geometry map therefore has a gap.
        city.set_tile_bits(41, 40, TileBits::new(0).with_piped(true));
        city.set_building(42, 40, 0x70);
        city.set_tile_bits(42, 40, TileBits::new(0).with_piped(true));

        let (geometry_pipes, geometry_connected) = water_pipe_connection_maps(&city);
        assert!(!geometry_pipes[40 * CITY_WIDTH + 41]);
        assert!(!geometry_connected[40 * CITY_WIDTH + 42]);

        let maps = water_service_maps_with_entry_seed_and_fidelity(&city, 1, EngineFidelity::Win95);
        assert!(maps.pe_worker_contract_complete);
        assert!(maps.watered_tiles[40 * CITY_WIDTH + 42]);

        pe_fun_00464550(&mut city);
        assert!(city.tile_bits_at(42, 40).unwrap().watered());
    }

    #[test]
    fn mac_water_worker_ratio_uses_capped_cells_over_raw_source_units() {
        let maps = WaterServiceMaps {
            pipe_tiles: Vec::new(),
            watered_tiles: Vec::new(),
            pe_worker_capacity_units: 0,
            pe_worker_served_cells: 0,
            pe_worker_pending_tiles: Vec::new(),
            pe_worker_contract_complete: false,
            mac_worker_capacity_units: 2,
            mac_worker_served_cells: 1,
            mac_worker_contract_complete: true,
        };
        assert_eq!(maps.mac_worker_utilization_percent(), Some(50));

        let no_supply = WaterServiceMaps {
            mac_worker_capacity_units: 0,
            mac_worker_served_cells: 0,
            ..maps
        };
        assert_eq!(no_supply.mac_worker_utilization_percent(), Some(100));

        let incomplete = WaterServiceMaps {
            mac_worker_contract_complete: false,
            ..no_supply
        };
        assert_eq!(incomplete.mac_worker_utilization_percent(), None);
    }

    #[test]
    fn mac_water_worker_collects_ratio_from_an_exact_source_component() {
        let mut city =
            City::new_blank("/tmp/sc2000-mac-water-ratio.sc2", "water").expect("blank city");
        city.kind = CityFileKind::MacCity;
        city.set_building(40, 40, 220);
        city.set_building(41, 40, 0x70);
        city.set_underground(40, 40, 16);
        city.set_underground(41, 40, 16);
        city.set_tile_bits(40, 40, TileBits::new(0).with_piped(true).with_powered(true));
        city.set_tile_bits(41, 40, TileBits::new(0).with_piped(true));
        // Humidity 4 contributes two source units; zero selected height and
        // no saved surface-water cells add no other capacity.
        city.set_misc_u32_at(0x68, 4);
        city.set_mac_state_misc_u32_at(912 * 4, 0);

        let maps =
            water_service_maps_with_entry_seed_and_fidelity(&city, 1, EngineFidelity::Mac68k);
        assert!(maps.mac_worker_contract_complete);
        assert_eq!(maps.mac_worker_capacity_units, 2);
        assert_eq!(maps.mac_worker_served_cells, 1);
        assert_eq!(maps.mac_worker_utilization_percent(), Some(50));

        // The Mac worker follows saved XBIT membership directly. Geometry
        // still recognizes the underground connection, but a missing saved
        // XBIT bit leaves the consumer outside the native flood.
        let mut mismatched = city;
        mismatched.set_tile_bits(41, 40, TileBits::new(0));
        let fallback =
            water_service_maps_with_entry_seed_and_fidelity(&mismatched, 1, EngineFidelity::Mac68k);
        assert!(fallback.mac_worker_contract_complete);
        assert_eq!(fallback.mac_worker_capacity_units, 2);
        assert_eq!(fallback.mac_worker_served_cells, 0);
        assert_eq!(fallback.mac_worker_utilization_percent(), Some(0));
        assert!(fallback.watered_tiles[40 * CITY_WIDTH + 40]);
        assert!(!fallback.watered_tiles[40 * CITY_WIDTH + 41]);
    }

    #[test]
    fn mac_water_tower_without_pump_or_desalination_keeps_only_saved_tower_cells() {
        let mut city =
            City::new_blank("/tmp/sc2000-mac-water-tower-only.sc2", "tower").expect("blank city");
        city.kind = CityFileKind::MacCity;
        for y in 10..=11 {
            for x in 10..=11 {
                city.set_building(x, y, 235);
                city.set_underground(x, y, 16);
                city.set_tile_bits(x, y, TileBits::new(0).with_piped(true).with_watered(true));
            }
        }
        city.set_building(12, 10, 0x70);
        city.set_underground(12, 10, 16);
        city.set_tile_bits(12, 10, TileBits::new(0).with_piped(true));

        let maps =
            water_service_maps_with_entry_seed_and_fidelity(&city, 1, EngineFidelity::Mac68k);
        assert!(maps.mac_worker_contract_complete);
        assert_eq!(maps.mac_worker_capacity_units, 0);
        assert_eq!(maps.mac_worker_served_cells, 0);
        assert_eq!(maps.mac_worker_utilization_percent(), Some(100));
        assert!(maps.watered_tiles[10 * CITY_WIDTH + 10]);
        assert!(maps.watered_tiles[11 * CITY_WIDTH + 11]);
        assert!(!maps.watered_tiles[10 * CITY_WIDTH + 12]);
    }
    #[test]
    fn mac_power_worker_matches_original_68020_map_replay() {
        let csv = include_str!("../../tests/fixtures/simulation/mac/mac_power.csv");
        for row in csv.lines().skip(1) {
            let fields: Vec<_> = row.split(',').collect();
            assert_eq!(fields.len(), 9);
            let name = fields[0];
            let value = |i: usize| fields[i].parse::<u32>().unwrap();
            if name == "phase0_statue_ordinance_prefix" {
                // The native phase-1 oracle starts after the four draws
                // composed by mac_annual_phase0_xmic_and_ordinance_draws_feed_phase1_power_stream.
                let mut phase0_rng = MacToolboxRng::new(15);
                for _ in 0..4 {
                    phase0_rng.random_u16();
                }
                assert_eq!(value(3), phase0_rng.seed(), "phase-0 prefix seed");
            }
            let mut city = City::new_blank("/tmp/mac-power-replay.city", "Power").unwrap();
            city.kind = crate::city::CityFileKind::MacCity;
            for x in 0..128 {
                for y in 0..128 {
                    city.set_building(x, y, 0);
                    city.set_tile_bits(x, y, TileBits::new(0));
                }
            }
            city.set_mac_state_misc_u32_at(0x64, value(1));
            city.set_mac_state_misc_u32_at(0x68, value(2));
            city.set_mac_state_misc_u32_at(0x0fa0, value(4) << 16);
            if fields[5] == "@overflow" {
                for x in 0..128 {
                    for y in 0..128 {
                        let root = (x, y) == (64, 64);
                        city.set_building(x, y, if root { 0xcd } else { 0x70 });
                        let powerable = root || (x * 17 + y * 31 + x * y * 3) % 5 != 0;
                        city.set_tile_bits(x, y, TileBits::new(if powerable { 0x80 } else { 0 }));
                    }
                }
            } else if !fields[5].is_empty() {
                for cell in fields[5].split('|') {
                    let v: Vec<usize> = cell.split(':').map(|s| s.parse().unwrap()).collect();
                    assert_eq!(v.len(), 5);
                    city.set_building(v[0], v[1], v[2] as u8);
                    city.set_tile_bits(v[0], v[1], TileBits::new(v[3] as u8));
                    city.set_altitude(v[0], v[1], v[4] as u8, 0);
                }
            }
            let mut rng = MacToolboxRng::new(value(3));
            assert_eq!(
                mac_export_218_power_with_rng(&mut city, &mut rng),
                Some(value(6)),
                "{name} summary"
            );
            assert_eq!(rng.seed(), value(7), "{name} RNG");
            let mut hash = 14695981039346656037_u64;
            for x in 0..128 {
                for y in 0..128 {
                    hash = (hash ^ u64::from(city.tile_bits_at(x, y).unwrap().raw_bits()))
                        .wrapping_mul(1099511628211);
                }
            }
            assert_eq!(hash, fields[8].parse::<u64>().unwrap(), "{name} XBIT map");
        }
    }
    #[test]
    fn mac_water_worker_matches_original_68020_map_replay() {
        let csv = include_str!("../../tests/fixtures/simulation/mac/mac_water.csv");
        for row in csv.lines().skip(1) {
            let fields: Vec<_> = row.split(',').collect();
            assert_eq!(fields.len(), 9);
            let name = fields[0];
            let value = |i: usize| fields[i].parse::<u32>().unwrap();
            let native_xbit: &[u8] = match name {
                "empty" => {
                    include_bytes!("../../tests/fixtures/simulation/mac/water_bitplanes/empty.bin")
                }
                "tower_only" => include_bytes!(
                    "../../tests/fixtures/simulation/mac/water_bitplanes/tower_only.bin"
                ),
                "pump" => {
                    include_bytes!("../../tests/fixtures/simulation/mac/water_bitplanes/pump.bin")
                }
                "mixed" => {
                    include_bytes!("../../tests/fixtures/simulation/mac/water_bitplanes/mixed.bin")
                }
                "tower_fill" => include_bytes!(
                    "../../tests/fixtures/simulation/mac/water_bitplanes/tower_fill.bin"
                ),
                "tower_carried" => include_bytes!(
                    "../../tests/fixtures/simulation/mac/water_bitplanes/tower_carried.bin"
                ),
                "rotation_0" => include_bytes!(
                    "../../tests/fixtures/simulation/mac/water_bitplanes/rotation_0.bin"
                ),
                "rotation_1" => include_bytes!(
                    "../../tests/fixtures/simulation/mac/water_bitplanes/rotation_1.bin"
                ),
                "rotation_2" => include_bytes!(
                    "../../tests/fixtures/simulation/mac/water_bitplanes/rotation_2.bin"
                ),
                "rotation_3" => include_bytes!(
                    "../../tests/fixtures/simulation/mac/water_bitplanes/rotation_3.bin"
                ),
                "ring_overflow" => include_bytes!(
                    "../../tests/fixtures/simulation/mac/water_bitplanes/ring_overflow.bin"
                ),
                _ => panic!("unknown native Mac water replay fixture {name}"),
            };
            let mut city = City::new_blank("/tmp/mac-water-replay.city", "Water").unwrap();
            city.kind = crate::city::CityFileKind::MacCity;
            for x in 0..128 {
                for y in 0..128 {
                    city.set_building(x, y, 0);
                    city.set_tile_bits(x, y, TileBits::new(0));
                }
            }
            city.saved_rotation = value(1) as u8;
            city.set_mac_state_misc_u32_at(0x68, value(2));
            city.set_mac_selected_water_height(value(3) as u8);
            if fields[4] == "@overflow" {
                for x in 0..128 {
                    for y in 0..128 {
                        let root = (x, y) == (64, 64);
                        city.set_building(x, y, if root { 0xdc } else { 0x70 });
                        let piped = (x * 17 + y * 31 + x * y * 3) % 2 != 0;
                        city.set_tile_bits(
                            x,
                            y,
                            TileBits::new(if root {
                                0x60
                            } else if piped {
                                0x20
                            } else {
                                0
                            }),
                        );
                    }
                }
            } else if !fields[4].is_empty() {
                for cell in fields[4].split('|') {
                    let v: Vec<usize> = cell.split(':').map(|s| s.parse().unwrap()).collect();
                    assert_eq!(v.len(), 4);
                    city.set_building(v[0], v[1], v[2] as u8);
                    city.set_tile_bits(v[0], v[1], TileBits::new(v[3] as u8));
                }
            }
            let maps =
                water_service_maps_with_entry_seed_and_fidelity(&city, 1, EngineFidelity::Mac68k);
            assert!(maps.mac_worker_contract_complete, "{name}");
            assert_eq!(
                maps.mac_worker_served_cells,
                value(5),
                "{name} served count"
            );
            assert_eq!(maps.mac_worker_capacity_units, value(6), "{name} capacity");
            assert_eq!(
                maps.mac_worker_utilization_percent(),
                Some(value(7)),
                "{name} summary"
            );
            crate::app::power_water_refresh::publish_mac_water_result_for_city(&mut city, &maps);
            let mut hash = 14695981039346656037_u64;
            for x in 0..128 {
                for y in 0..128 {
                    hash = (hash ^ u64::from(city.tile_bits_at(x, y).unwrap().raw_bits()))
                        .wrapping_mul(1099511628211);
                }
            }
            let mut rust_xbit = Vec::with_capacity(128 * 128);
            for x in 0..128 {
                for y in 0..128 {
                    rust_xbit.push(city.tile_bits_at(x, y).unwrap().raw_bits());
                }
            }
            assert_eq!(rust_xbit.as_slice(), native_xbit, "{name} full XBIT bytes");
            assert_eq!(
                hash,
                fields[8].parse::<u64>().unwrap(),
                "{name} full XBIT map"
            );
        }
    }

    #[test]
    fn mac_water_source_weather_matrix_matches_native_fingerprints() {
        // Extension of the original CODE 14 source/weather matrix. These
        // source-only maps isolate the byte humidity, selected-height, and
        // local-water terms; the native full-plane fingerprints are pinned
        // below as regression oracles.
        let cases: [(&str, u8, u8, u8, usize, u32, u32, u64); 6] = [
            (
                "pump-zero",
                220,
                0,
                0,
                0,
                0,
                100,
                956_091_040_562_241_557_u64,
            ),
            (
                "pump-high",
                220,
                255,
                31,
                0,
                282,
                0,
                956_091_040_562_241_557,
            ),
            (
                "pump-high-local-water",
                220,
                255,
                31,
                8,
                362,
                0,
                9_077_960_052_210_056_117,
            ),
            ("desal-zero", 250, 15, 0, 0, 0, 100, 956_091_040_562_241_557),
            (
                "desal-one-salt-neighbor",
                250,
                15,
                0,
                1,
                20,
                0,
                10_254_596_661_353_056_654,
            ),
            (
                "desal-eight-salt-neighbors",
                250,
                15,
                0,
                8,
                160,
                0,
                4_840_327_164_549_656_109,
            ),
        ];
        let neighbors = [
            (9, 9),
            (9, 10),
            (9, 11),
            (10, 9),
            (10, 11),
            (11, 9),
            (11, 10),
            (11, 11),
        ];

        for (name, source, humidity, height, wet_neighbors, capacity, summary, native_hash) in cases
        {
            let mut city =
                City::new_blank("/tmp/mac-water-source-weather.sc2", name).expect("blank city");
            city.kind = crate::city::CityFileKind::MacCity;
            city.saved_rotation = 0;
            for x in 0..128 {
                for y in 0..128 {
                    city.set_building(x, y, 0);
                    city.set_tile_bits(x, y, TileBits::new(0));
                }
            }
            city.set_mac_state_misc_u32_at(0x68, u32::from(humidity));
            city.set_mac_selected_water_height(height);
            city.set_building(10, 10, source);
            city.set_tile_bits(10, 10, TileBits::new(0x60));
            let water_bits = if source == 220 { 0x04 } else { 0x05 };
            for &(x, y) in neighbors.iter().take(wet_neighbors) {
                city.set_tile_bits(x, y, TileBits::new(water_bits));
            }

            let maps =
                water_service_maps_with_entry_seed_and_fidelity(&city, 1, EngineFidelity::Mac68k);
            assert!(maps.mac_worker_contract_complete, "{name}");
            assert_eq!(maps.mac_worker_served_cells, 0, "{name} served");
            assert_eq!(maps.mac_worker_capacity_units, capacity, "{name} capacity");
            assert_eq!(
                maps.mac_worker_utilization_percent(),
                Some(summary),
                "{name} ratio"
            );
            crate::app::power_water_refresh::publish_mac_water_result_for_city(&mut city, &maps);

            let mut hash = 14_695_981_039_346_656_037_u64;
            for x in 0..128 {
                for y in 0..128 {
                    hash = (hash ^ u64::from(city.tile_bits_at(x, y).unwrap().raw_bits()))
                        .wrapping_mul(1_099_511_628_211);
                }
            }
            assert_eq!(hash, native_hash, "{name} full XBIT fingerprint");
        }
    }

    #[test]
    fn mac_treatment_gate_matches_original_68020_replay() {
        let csv = include_str!("../../tests/fixtures/simulation/mac/mac_treatment_gate.csv");
        for row in csv.lines().skip(1) {
            let values: Vec<u32> = row.split(',').map(|s| s.parse().unwrap()).collect();
            assert_eq!(values.len(), 3);
            assert_eq!(
                mac_water_treatment_gate(values[1], values[0] as u16),
                values[2] as i16,
                "{row}"
            );
        }
    }
}
