use super::*;

use crate::mac_random::MacToolboxRng;
use crate::paths::{self, test_mac_city, test_win_city, test_win_populated_city};
use crate::win95_random::Win95CrtRand;

#[test]
fn mac_save_rebuilds_cached_bond_rate_sum_without_using_pe_tail() {
    let mut mac =
        City::new_blank("/tmp/sc2000-mac-bond-cache.city", "MacBondCache").expect("blank city");
    mac.kind = CityFileKind::MacCity;
    mac.set_mac_state_misc_u32_at(MISC_CITY_BONDS_OFFSET, 2);
    mac.set_mac_state_misc_u32_at(MISC_BOND_RATE_TABLE_OFFSET, 0xfeed_0005);
    mac.set_mac_state_misc_u32_at(MISC_BOND_RATE_TABLE_OFFSET + 4, 0xdead_0003);
    mac.set_mac_state_misc_u32_at(MAC_BOND_RATE_SUM_MISC_OFFSET, 0xdead_beef);

    let mac_path =
        std::env::temp_dir().join(format!("sc2000-mac-bond-cache-{}.city", std::process::id()));
    mac.save(&mac_path).expect("save Mac-layout bond cache");
    let mac = City::load(&mac_path).expect("reload Mac-layout bond cache");
    assert_eq!(
        mac.mac_state_misc_u32_at(MAC_BOND_RATE_SUM_MISC_OFFSET),
        Some(8),
        "Mac slot 1070 is derived from the active low-WORD rate entries"
    );
    let _ = fs::remove_file(mac_path);

    let mut pe = City::new_blank("/tmp/sc2000-pe-bond-cache.sc2", "PeBondCache")
        .expect("blank Win-layout city");
    pe.set_misc_u32_at(MISC_CITY_BONDS_OFFSET, 1);
    pe.set_misc_u32_at(MISC_BOND_RATE_TABLE_OFFSET, 7);
    let pe_path =
        std::env::temp_dir().join(format!("sc2000-pe-bond-cache-{}.sc2", std::process::id()));
    pe.save(&pe_path).expect("save Win-layout bond table");
    let pe = City::load(&pe_path).expect("reload Win-layout bond table");
    assert_eq!(
        pe.misc_u32_at(MAC_BOND_RATE_SUM_MISC_OFFSET),
        Some(0),
        "PE's zero-filled MISC tail must not become Mac bond state"
    );
    let _ = fs::remove_file(pe_path);
}

#[test]
fn mac_industry_scale_matches_independent_extended80_implementation() {
    let fixture = include_str!("../../../tests/fixtures/simulation/mac/mac_industry.csv");
    let factors = [
        (0xcccc_cccc_cccc_cccd, true),
        (0xe666_6666_6666_6666, true),
        (0x8ccc_cccc_cccc_cccd, false),
        (0x9999_9999_9999_999a, false),
    ];
    let mut count = 0;
    for line in fixture.lines().skip(1) {
        let mut fields = line.split(',');
        let value = fields.next().unwrap().parse().unwrap();
        let index: usize = fields.next().unwrap().parse().unwrap();
        let expected = fields.next().unwrap().parse().unwrap();
        assert!(fields.next().is_none());
        let (significand, below_one) = factors[index];
        assert_eq!(
            mac_sane_industry_scale(value, significand, below_one),
            Some(expected),
            "{line}"
        );
        count += 1;
    }
    assert_eq!(count, 2212);
}

#[test]
fn mac_industry_modified_chain_matches_68020_with_modeled_sane() {
    let fixture =
        include_str!("../../../tests/fixtures/simulation/mac/mac_industry_modified_chain.csv");
    let mut count = 0;
    for line in fixture.lines().skip(1) {
        let c: Vec<_> = line.split(',').collect();
        assert_eq!(c.len(), 74);
        let pollution = c[0] == "1";
        let growth = c[1] == "1";
        let education = c[2].parse().unwrap();
        let target = c[3].parse().unwrap();
        let prior = std::array::from_fn(|i| c[4 + i].parse().unwrap());
        let taxes = std::array::from_fn(|i| c[15 + i].parse().unwrap());
        let rows = std::array::from_fn(|i| c[26 + i].parse().unwrap());
        let draws: [u16; 11] = std::array::from_fn(|i| c[37 + i].parse().unwrap());
        let expected_demand: [i16; 11] = std::array::from_fn(|i| c[48 + i].parse().unwrap());
        let expected_rows: [u32; 11] = std::array::from_fn(|i| c[59 + i].parse().unwrap());
        let expected_sum: u32 = c[70].parse().unwrap();
        let expected_mix = (c[71].parse().unwrap(), c[72].parse().unwrap());
        let expected_used: usize = c[73].parse().unwrap();
        let (demand, weights, sum) =
            mac_industry_modified_residuals(prior, taxes, pollution, growth, education).unwrap();
        let mut used = 0;
        let rows = mac_industry_rebalance(target, rows, weights, sum, || {
            let value = draws[used];
            used += 1;
            value
        });
        assert_eq!(demand, expected_demand, "{line}");
        assert_eq!(sum, expected_sum, "{line}");
        assert_eq!(rows, expected_rows, "{line}");
        assert_eq!(
            mac_industry_mix_bonuses(target, rows),
            Some(expected_mix),
            "{line}"
        );
        assert_eq!(used, expected_used, "{line}");
        count += 1;
    }
    assert_eq!(count, 160);
}

#[test]
fn mac_industry_modifiers_match_68020_with_modeled_sane() {
    let fixture = include_str!("../../../tests/fixtures/simulation/mac/mac_industry_modifiers.csv");
    let mut count = 0;
    let mut differing_totals = 0;
    for line in fixture.lines().skip(1) {
        let c: Vec<_> = line.split(',').collect();
        assert_eq!(c.len(), 49);
        let pollution = c[0] == "1";
        let growth = c[1] == "1";
        let education = c[2].parse().unwrap();
        let prior = std::array::from_fn(|i| c[3 + i].parse().unwrap());
        let taxes = std::array::from_fn(|i| c[14 + i].parse().unwrap());
        let updated: [i16; 11] = std::array::from_fn(|i| c[25 + i].parse().unwrap());
        let weights: [u32; 11] = std::array::from_fn(|i| c[36 + i].parse().unwrap());
        let sum: u32 = c[47].parse().unwrap();
        assert_eq!(
            mac_industry_modified_residuals(prior, taxes, pollution, growth, education),
            Some((updated, weights, sum)),
            "{line}"
        );
        if weights
            .iter()
            .fold(0_u32, |acc, &value| acc.wrapping_add(value))
            != sum
        {
            differing_totals += 1;
        }
        count += 1;
    }
    assert_eq!(count, 160);
    assert_eq!(differing_totals, 139);
}

#[test]
fn mac_industry_neutral_chain_matches_original_68020() {
    let fixture = include_str!("../../../tests/fixtures/simulation/mac/mac_industry_chain.csv");
    let mut count = 0;
    for line in fixture.lines().skip(1) {
        let c: Vec<_> = line.split(',').collect();
        assert_eq!(c.len(), 71);
        let target = c[0].parse().unwrap();
        let prior = std::array::from_fn(|i| c[1 + i].parse().unwrap());
        let taxes = std::array::from_fn(|i| c[12 + i].parse().unwrap());
        let rows = std::array::from_fn(|i| c[23 + i].parse().unwrap());
        let draws: [u16; 11] = std::array::from_fn(|i| c[34 + i].parse().unwrap());
        let expected_demand: [i16; 11] = std::array::from_fn(|i| c[45 + i].parse().unwrap());
        let expected_rows: [u32; 11] = std::array::from_fn(|i| c[56 + i].parse().unwrap());
        let expected_bonus = (c[67].parse().unwrap(), c[68].parse().unwrap());
        let (demand, weights, weight_sum) = mac_industry_neutral_residuals(prior, taxes);
        let mut used = 0;
        let rows = mac_industry_rebalance(target, rows, weights, weight_sum, || {
            let draw = draws[used];
            used += 1;
            draw
        });
        assert_eq!(demand, expected_demand, "{line}");
        assert_eq!(rows, expected_rows, "{line}");
        assert_eq!(
            mac_industry_mix_bonuses(target, rows),
            Some(expected_bonus),
            "{line}"
        );
        assert_eq!(used, c[69].parse::<usize>().unwrap(), "{line}");
        assert_eq!(c[70].parse::<usize>().unwrap(), 44, "{line}");
        count += 1;
    }
    assert_eq!(count, 100);
}

#[test]
fn mac_export238_growth_modifier_join_matches_68020_case() {
    // One CODE 8 export_238 entry-to-cohort-boundary case; see the evidence
    // note for the synthetic pointers, profile tables, and scripted draws.
    let prior = [20, 20, 10, 10, 15, 5, 0, 15, 8, 0, 10];
    let taxes = [0; PE_INDUSTRY_COUNT];
    let rows = [1; PE_INDUSTRY_COUNT];
    let (demand, weights, weight_sum) =
        mac_industry_modified_residuals(prior, taxes, false, true, 60).unwrap();
    assert_eq!(demand, [15, 15, 7, 7, 11, 3, 0, 11, 6, 0, 7]);
    assert_eq!(weights, [15, 15, 7, 7, 18, 3, 0, 11, 6, 0, 7]);
    // export_238 accumulates this denominator as the rows are visited. Its
    // later growth-modifier writes change row 4's final weight to 18 without
    // revisiting the already accumulated total.
    assert_eq!(weight_sum, 83);

    let mut draw_count = 0;
    let rows = mac_industry_rebalance(20, rows, weights, weight_sum, || {
        draw_count += 1;
        1
    });
    assert_eq!(rows, [3, 3, 2, 2, 3, 2, 1, 3, 2, 1, 2]);
    assert_eq!(draw_count, 9);
    assert_eq!(mac_industry_mix_bonuses(20, rows), Some((0, 0)));
}

#[test]
fn mac_export238_balance_matches_initgraf_quickdraw_sequence() {
    let prior = [20, 20, 10, 10, 15, 5, 0, 15, 8, 0, 10];
    let taxes = [0; PE_INDUSTRY_COUNT];
    let rows = [1; PE_INDUSTRY_COUNT];
    let (_, weights, weight_sum) =
        mac_industry_modified_residuals(prior, taxes, false, true, 60).unwrap();
    let mut rng = MacToolboxRng::new(MacToolboxRng::QUICKDRAW_INIT_GRAF_SEED);
    // The first two native _Random calls precede the row balancing loop.
    assert_eq!(rng.random_i16(), 16_807);
    assert_eq!(rng.random_i16(), 15_089);
    let mut draws = 0;
    let rows = mac_industry_rebalance(20, rows, weights, weight_sum, || {
        draws += 1;
        rng.random_u16()
    });
    assert_eq!(rows, [3, 3, 1, 2, 3, 1, 1, 2, 2, 1, 2]);
    assert_eq!(draws, 9);
}

#[test]
fn mac_export238_balance_matches_native_quickdraw_seed_matrix() {
    let fixture = include_str!(
        "../../../tests/fixtures/simulation/mac/mac_phase21_balance_quickdraw_seed_matrix.csv"
    );
    let prior = [20, 20, 10, 10, 15, 5, 0, 15, 8, 0, 10];
    let taxes = [0; PE_INDUSTRY_COUNT];
    let initial_rows = [1; PE_INDUSTRY_COUNT];
    let (_, weights, weight_sum) =
        mac_industry_modified_residuals(prior, taxes, false, true, 60).unwrap();
    let mut cases = 0;

    for line in fixture.lines().skip(1) {
        let mut fields = line.split(',');
        let seed: u32 = fields.next().unwrap().parse().unwrap();
        let expected_draws: Vec<i16> = fields
            .next()
            .unwrap()
            .split(':')
            .map(|value| value.parse().unwrap())
            .collect();
        let expected_rows: Vec<u32> = fields
            .next()
            .unwrap()
            .split(':')
            .map(|value| value.parse().unwrap())
            .collect();
        assert!(fields.next().is_none());
        let expected_rows: [u32; PE_INDUSTRY_COUNT] = expected_rows.try_into().unwrap();

        let mut sequence_rng = MacToolboxRng::new(seed);
        let actual_draws: Vec<i16> = (0..expected_draws.len())
            .map(|_| sequence_rng.random_i16())
            .collect();
        assert_eq!(actual_draws, expected_draws, "seed {seed}");

        // The native function consumes two words before row balancing.
        let mut rng = MacToolboxRng::new(seed);
        rng.random_i16();
        rng.random_i16();
        let mut used = 0;
        let actual_rows = mac_industry_rebalance(20, initial_rows, weights, weight_sum, || {
            used += 1;
            rng.random_u16()
        });
        assert_eq!(actual_rows, expected_rows, "seed {seed}");
        assert_eq!(used, 9, "seed {seed}");
        cases += 1;
    }
    assert_eq!(cases, 8);
}

#[test]
fn mac_industry_neutral_residuals_match_68020_loop() {
    let fixture = include_str!("../../../tests/fixtures/simulation/mac/mac_industry_neutral.csv");
    let mut count = 0;
    for line in fixture.lines().skip(1) {
        let c: Vec<_> = line.split(',').collect();
        assert_eq!(c.len(), 46);
        let prior = std::array::from_fn(|i| c[i].parse().unwrap());
        let taxes = std::array::from_fn(|i| c[11 + i].parse().unwrap());
        let updated: [i16; 11] = std::array::from_fn(|i| c[22 + i].parse().unwrap());
        let weights: [u32; 11] = std::array::from_fn(|i| c[33 + i].parse().unwrap());
        let sum = c[44].parse().unwrap();
        assert_eq!(
            mac_industry_neutral_residuals(prior, taxes),
            (updated, weights, sum),
            "{line}"
        );
        assert_eq!(c[45].parse::<usize>().unwrap(), 44, "{line}");
        count += 1;
    }
    assert_eq!(count, 100);
}

#[test]
fn mac_industry_balancing_matches_68020_rows_and_draw_order() {
    let fixture = include_str!("../../../tests/fixtures/simulation/mac/mac_industry_balance.csv");
    let mut cases = 0;
    for line in fixture.lines().skip(1) {
        let c: Vec<_> = line.split(',').collect();
        assert_eq!(c.len(), 47);
        let target = c[0].parse().unwrap();
        let rows = std::array::from_fn(|i| c[1 + i].parse().unwrap());
        let weights = std::array::from_fn(|i| c[12 + i].parse().unwrap());
        let weight_sum = c[23].parse().unwrap();
        let draws: [u16; 11] = std::array::from_fn(|i| c[24 + i].parse().unwrap());
        let expected_used: usize = c[35].parse().unwrap();
        let expected: [u32; 11] = std::array::from_fn(|i| c[36 + i].parse().unwrap());
        let mut used = 0;
        let actual = mac_industry_rebalance(target, rows, weights, weight_sum, || {
            let draw = draws[used];
            used += 1;
            draw
        });
        assert_eq!(actual, expected, "{line}");
        assert_eq!(used, expected_used, "{line}");
        cases += 1;
    }
    assert_eq!(cases, 120);
}

#[test]
fn mac_industry_mix_outputs_match_original_68020_instructions() {
    let fixture = include_str!("../../../tests/fixtures/simulation/mac/mac_industry_mix.csv");
    let mut cases = 0;
    for line in fixture.lines().skip(1) {
        let columns: Vec<_> = line.split(',').collect();
        assert_eq!(columns.len(), 14);
        let total: u32 = columns[0].parse().unwrap();
        let rows: [u32; 11] = std::array::from_fn(|i| columns[1 + i].parse().unwrap());
        let expected = (columns[12].parse().unwrap(), columns[13].parse().unwrap());
        assert_eq!(
            mac_industry_mix_bonuses(total, rows),
            Some(expected),
            "{line}"
        );
        cases += 1;
    }
    assert_eq!(cases, 112);
    assert_eq!(mac_industry_mix_bonuses(u32::MAX, [0; 11]), None);
}

#[test]
fn scenario_countdown_writes_stock_raw_chunk_and_preserves_optional_fields() {
    for length in [52, 56] {
        for legacy_compressed in [false, true] {
            let mut city = City::new_blank("/tmp/scenario-chunk.scn", "Scenario").unwrap();
            let mut record = vec![0; length];
            record[0] = 0x80;
            record[8..10].copy_from_slice(&60_u16.to_be_bytes());
            record[12] = 0x42;
            if length == 56 {
                record[52..56].copy_from_slice(&[0, 16, 0, 9]);
            }
            let data = if legacy_compressed {
                sc2_rle_encode(&record)
            } else {
                record.clone()
            };
            city.chunks.push(CityChunk {
                name: "SCEN".into(),
                offset: 0,
                raw_len: data.len(),
                data,
            });
            assert_eq!(city.scenario_time_limit_months(), Some(60));
            city.set_scenario_time_limit_months(59);
            record[8..10].copy_from_slice(&59_u16.to_be_bytes());
            assert_eq!(city.scenario_time_limit_months(), Some(59));
            let file = encode_scdh_file(&city.chunks).unwrap();
            let chunks = parse_scdh_chunks(&file).unwrap();
            let scen = chunks.iter().find(|chunk| chunk.name == "SCEN").unwrap();
            assert_eq!(
                scen.data, record,
                "length={length}, legacy={legacy_compressed}"
            );
        }
    }
}

fn stamp_complete_building(
    city: &mut City,
    origin_x: usize,
    origin_y: usize,
    building: u8,
    size: usize,
) {
    for y in origin_y..origin_y + size {
        for x in origin_x..origin_x + size {
            city.set_building(x, y, building);
        }
    }
    city.pe_rebuild_dw_tile_count_434f80();
}

fn set_military_base_type(city: &mut City, base_type: u32) {
    city.set_misc_u32_at(MISC_MILITARY_BASE_TYPE_OFFSET, base_type);
}

fn load_mac() -> Option<City> {
    test_mac_city().and_then(|path| City::load(path).ok())
}

fn load_win() -> Option<City> {
    test_win_city().and_then(|path| City::load(path).ok())
}

#[test]
fn mac_building_edits_use_zone7_class_census_instead_of_general_xbld_census() {
    let mut city =
        City::new_blank("/tmp/sc2000-mac-xbld-census.sc2", "census").expect("blank city");
    city.kind = CityFileKind::MacCity;
    city.set_mac_state_misc_u32_at(MAC_XBLD_CENSUS_MISC_OFFSET, CITY_TILES as u32);

    city.set_building(10, 20, 0xd2);
    assert_eq!(city.mac_xbld_tile_count(0), (CITY_TILES - 1) as u16);
    assert_eq!(city.mac_xbld_tile_count(0xd2), 1);

    city.set_zone(11, 20, 7);
    city.set_building(11, 20, 0xdd);
    assert_eq!(city.mac_xbld_tile_count(0), (CITY_TILES - 1) as u16);
    assert_eq!(city.mac_xbld_tile_count(0xdd), 0);
    assert_eq!(city.mac_zone7_census_count(0), u16::MAX);
    assert_eq!(city.mac_zone7_census_count(1), 1);

    city.set_building(11, 20, 0xe3);
    assert_eq!(city.mac_zone7_census_count(1), 0);
    assert_eq!(city.mac_zone7_census_count(6), 1);

    city.set_building(11, 20, 0);
    assert_eq!(city.mac_zone7_census_count(6), 0);
    assert_eq!(city.mac_zone7_census_count(0), 0);

    city.set_zone(10, 20, 0);
    city.set_building(10, 20, 0);
    assert_eq!(city.mac_xbld_tile_count(0), CITY_TILES as u16);
    assert_eq!(city.mac_xbld_tile_count(0xd2), 0);
}

#[test]
fn mac_military_marker_bypasses_placetile_and_moves_censuses() {
    let mut city = City::new_blank("/tmp/sc2000-mac-military-marker.sc2", "census").expect("blank");
    city.kind = CityFileKind::MacCity;
    city.set_building(40, 30, 0x08);
    city.set_zone(40, 30, 0xa3);
    let xbld_offset = MAC_XBLD_CENSUS_MISC_OFFSET + 0x08 * 4;
    city.set_mac_state_misc_u32_at(xbld_offset, 0);
    city.set_mac_state_misc_u32_at(MAC_ZONE7_CENSUS_MISC_OFFSET, u16::MAX as u32);

    assert!(city.mac_mark_military_zone_candidate(40, 30));
    assert_eq!(
        city.building_at(40, 30),
        0x08,
        "the native marker keeps XBLD"
    );
    assert_eq!(city.zone_at(40, 30).unwrap().raw_bits(), 0xa7);
    assert_eq!(city.mac_xbld_tile_count(0x08), u16::MAX);
    assert_eq!(city.mac_zone7_census_count(0), 0);
    assert!(
        !city.mac_mark_military_zone_candidate(40, 30),
        "a second visit must not double-count an already-marked military tile"
    );
    assert_eq!(city.mac_zone7_census_count(0), 0);

    city.set_zone(41, 30, 0);
    city.set_building(41, 30, 0x0d);
    assert!(!city.mac_mark_military_zone_candidate(41, 30));
    assert!(!city.mac_mark_military_zone_candidate(CITY_WIDTH, 30));
}

#[test]
fn mac_zone7_class_census_roundtrips_in_native_misc_slots() {
    let path = std::env::temp_dir().join(format!(
        "sc2000-mac-zone7-census-{}.city",
        std::process::id()
    ));
    let mut city = City::new_blank(&path, "Zone7Census").expect("blank city");
    city.kind = CityFileKind::MacCity;
    city.set_mac_state_misc_u32_at(MAC_ZONE7_CENSUS_MISC_OFFSET + 3 * 4, 0x1234);
    city.save(&path).expect("save Mac city");
    let loaded = City::load(&path).expect("reload Mac city");
    assert_eq!(loaded.kind, CityFileKind::MacCity);
    assert_eq!(loaded.mac_zone7_census_count(3), 0x1234);
    let _ = fs::remove_file(path);
}

#[test]
fn mac_load_preserves_saved_xbld_census_when_it_disagrees_with_map() {
    let path = std::env::temp_dir().join(format!(
        "sc2000-mac-stale-xbld-census-{}.city",
        std::process::id()
    ));
    let mut city = City::new_blank(&path, "MacSavedCensus").expect("blank city");
    city.kind = CityFileKind::MacCity;
    // Mac simulation reads this persisted PlaceTile census. Preserve its
    // saved values even though this synthetic map contains no ID-6 tiles.
    city.set_mac_state_misc_u32_at(MAC_XBLD_CENSUS_MISC_OFFSET, 14_944);
    city.set_mac_state_misc_u32_at(MAC_XBLD_CENSUS_MISC_OFFSET + 6 * 4, 65_462);
    city.save(&path).expect("save Mac city");

    let mut loaded = City::load(&path).expect("reload Mac city");
    assert_eq!(loaded.mac_xbld_tile_count(0), 14_944);
    assert_eq!(loaded.mac_xbld_tile_count(6), 65_462);

    // A later native-style PlaceTile update applies a delta to the saved
    // table; it does not first repair the table from the map.
    loaded.set_building(40, 40, 6);
    assert_eq!(loaded.mac_xbld_tile_count(0), 14_943);
    assert_eq!(loaded.mac_xbld_tile_count(6), 65_463);
    let _ = fs::remove_file(path);
}

#[test]
fn mac_underground_edits_update_export106_xund_count_with_word_wrap() {
    let mut city =
        City::new_blank("/tmp/sc2000-mac-selector-xund.sc2", "selector").expect("blank city");
    city.kind = CityFileKind::MacCity;
    city.set_mac_state_misc_u32_at(MAC_XUND_CLASS_COUNT_MISC_OFFSET, 0xffff);
    city.set_zone(10, 20, 7);

    // The class count is maintained from XUND bytes without an XZON gate.
    // The WORD increment wraps, and replacing one member by another is neutral.
    city.set_underground(10, 20, 0x1f);
    assert_eq!(
        city.mac_state_misc_u32_at(MAC_XUND_CLASS_COUNT_MISC_OFFSET),
        Some(0)
    );
    city.set_underground(10, 20, 0x23);
    assert_eq!(
        city.mac_state_misc_u32_at(MAC_XUND_CLASS_COUNT_MISC_OFFSET),
        Some(0)
    );

    city.set_underground(10, 20, 0);
    assert_eq!(
        city.mac_state_misc_u32_at(MAC_XUND_CLASS_COUNT_MISC_OFFSET),
        Some(0xffff)
    );
    city.set_underground(10, 20, 0x10); // Not in export 106's member set.
    assert_eq!(
        city.mac_state_misc_u32_at(MAC_XUND_CLASS_COUNT_MISC_OFFSET),
        Some(0xffff)
    );

    // XBLD edits do not alter this XUND-derived counter.
    city.set_building(10, 20, 0x1f);
    assert_eq!(
        city.mac_state_misc_u32_at(MAC_XUND_CLASS_COUNT_MISC_OFFSET),
        Some(0xffff)
    );

    // The game's MISC word is persisted alongside XUND. Keep an existing
    // count and verify an edit updates the same word that save/load carries.
    city.set_mac_state_misc_u32_at(MAC_XUND_CLASS_COUNT_MISC_OFFSET, 7);
    city.set_underground(10, 20, 0x22);
    assert_eq!(
        city.mac_state_misc_u32_at(MAC_XUND_CLASS_COUNT_MISC_OFFSET),
        Some(8)
    );
    let path = std::env::temp_dir().join(format!(
        "sc2000-mac-selector-xund-{}.city",
        std::process::id()
    ));
    city.save(&path).expect("save Mac XUND count");
    let loaded = City::load(&path).expect("reload Mac XUND count");
    assert_eq!(loaded.kind, CityFileKind::MacCity);
    assert_eq!(loaded.underground_at(10, 20), 0x22);
    assert_eq!(
        loaded.mac_state_misc_u32_at(MAC_XUND_CLASS_COUNT_MISC_OFFSET),
        Some(8)
    );
    let _ = fs::remove_file(path);
}

fn downloaded_maxis_land() -> Option<City> {
    paths::test_city_from_env("SC2000_TEST_DOWNLOADED_MAC_CITY")
        .and_then(|path| City::load(path).ok())
}

#[test]
fn detects_city_kinds() {
    let Some(mac_city) = test_mac_city() else {
        return;
    };
    let Some(win_city) = test_win_city() else {
        return;
    };
    let mac = fs::read(&mac_city).expect("mac city");
    let win = fs::read(&win_city).expect("win city");
    let Some(scenario_path) = paths::test_win_scenario() else {
        return;
    };
    let scenario = fs::read(&scenario_path).expect("scenario");

    assert_eq!(
        detect_city_kind(mac_city.as_path(), &mac).unwrap(),
        CityFileKind::MacCity
    );
    assert_eq!(
        detect_city_kind(win_city.as_path(), &win).unwrap(),
        CityFileKind::WinSc2
    );
    assert_eq!(
        detect_city_kind(scenario_path.as_path(), &scenario).unwrap(),
        CityFileKind::WinScenario
    );
}

#[test]
fn parses_mac_and_windows_chunks() {
    let Some(mac_city) = test_mac_city() else {
        return;
    };
    let Some(win_city) = test_win_city() else {
        return;
    };
    let mac = City::load(mac_city).expect("mac city");
    let win = City::load(win_city).expect("win city");

    assert!(mac.chunk("ALTM").is_some());
    assert!(mac.chunk("XTER").is_some());
    assert!(mac.chunk("XBLD").is_some());
    assert!(win.chunk("CNAM").is_some());
    assert!(win.chunk("ALTM").is_some());
    assert!(win.chunk("XTER").is_some());
}

#[test]
fn win_default_city_is_blank_new_city_template() {
    let Some(path) = test_win_city() else {
        return;
    };
    let city = City::load(path).expect("load default");
    assert!(
        city.name.starts_with("New City"),
        "Win95 DEFAULT.SC2 is the blank New City template, got {:?}",
        city.name
    );
    assert!(city.terrain.is_some());
    assert!(city.buildings.is_some());
    assert!(city.altm.is_some());
    for y in 0..CITY_HEIGHT {
        for x in 0..CITY_WIDTH {
            assert_eq!(city.terrain_at(x, y), 0, "blank map at ({x},{y})");
            assert_eq!(city.building_at(x, y), 0);
        }
    }
}

#[test]
fn win_populated_city_decodes_rle_layers() {
    let Some(path) = test_win_populated_city() else {
        return;
    };
    let city = City::load(path).expect("load populated city");
    let mut heights = std::collections::BTreeSet::new();
    let mut building_tiles = 0usize;
    let mut terrain_kinds = std::collections::BTreeSet::new();
    for y in 0..CITY_HEIGHT {
        for x in 0..CITY_WIDTH {
            heights.insert(city.altitude_at(x, y));
            terrain_kinds.insert(city.terrain_at(x, y));
            if city.building_at(x, y) != 0 {
                building_tiles += 1;
            }
        }
    }
    assert!(
        building_tiles > 100,
        "Bay View should contain roads/buildings, got {building_tiles}"
    );
    assert!(
        heights.len() > 1,
        "Bay View should decode ALTM height variation, got {:?}",
        heights
    );
    assert!(
        terrain_kinds.len() > 5,
        "Bay View should have varied terrain, got {terrain_kinds:?}"
    );
}

#[test]
fn decodes_known_rle_chunks() {
    let Some(win) = load_win() else {
        return;
    };
    let Some(mac) = load_mac() else {
        return;
    };
    let xter = decode_expected_chunk(&win.chunks, "XTER", CITY_TILES).expect("xter decode");
    let xbld = decode_expected_chunk(&win.chunks, "XBLD", CITY_TILES).expect("xbld decode");
    let mac_xter = decode_expected_chunk(&mac.chunks, "XTER", CITY_TILES).expect("mac xter decode");
    assert_eq!(xter.len(), CITY_TILES);
    assert_eq!(xbld.len(), CITY_TILES);
    assert_eq!(mac_xter.len(), CITY_TILES);
}

#[test]
fn city_layers_use_x_major_order() {
    assert_eq!(city_index(0, 0), 0);
    assert_eq!(city_index(0, 1), 1);
    assert_eq!(city_index(1, 0), CITY_HEIGHT);
    assert_eq!(city_index(127, 127), CITY_TILES - 1);
}

#[test]
fn text_overlay_layer_round_trips_through_save() {
    let mut path = std::env::temp_dir();
    path.push(format!("sc2000-text-overlay-{}.sc2", std::process::id()));
    let mut city = City::new_blank(&path, "test").expect("blank city");
    city.set_text_overlay(10, 12, 1);
    city.set_label(1, "Main Street");
    city.save(&path).expect("save city");

    let loaded = City::load(&path).expect("reload city");
    assert_eq!(loaded.text_overlay_at(10, 12), Some(1));
    assert_eq!(loaded.label_at(1), Some("Main Street"));
    let _ = std::fs::remove_file(path);
}

#[test]
fn mac_pending_action_round_trips_on_win_layout_without_overwriting_pe_trigger() {
    let path = std::env::temp_dir().join(format!(
        "sc2000-mac-pending-action-cross-layout-{}.sc2",
        std::process::id()
    ));
    let mut city = City::new_blank(&path, "MacPendingAction").expect("blank Win-layout city");
    city.set_mac_pending_action(7);
    city.pe_w_trigger_disaster = 5;

    city.save(&path).expect("save Win-layout city");
    let loaded = City::load(&path).expect("reload Win-layout city");

    assert_eq!(loaded.mac_pending_action(), 7);
    assert_eq!(loaded.pe_w_trigger_disaster, 5);
    assert_eq!(
        loaded.misc_u32_at(MISC_TRIGGER_DISASTER_OFFSET),
        Some(5),
        "Mac CODE 4 +0x70 selector must not alias Win95's disaster trigger"
    );
    let extension = decode_citygame_extension(&loaded.chunks).expect("decode CityGame extension");
    assert_eq!(extension.version, 1);
    assert_eq!(extension.mac_pending_action, 7);
    assert_eq!(extension.mac_saved_state, None);
    let _ = std::fs::remove_file(path);
}

#[test]
fn mac_cross_layout_saved_state_round_trips_without_using_pe_misc_slots() {
    let path = std::env::temp_dir().join(format!(
        "sc2000-mac-runtime-state-cross-layout-{}.sc2",
        std::process::id()
    ));
    let mut city = City::new_blank(&path, "MacRuntimeState").expect("blank Win-layout city");
    city.mac_cross_layout_active = true;
    city.mac_pending_action_runtime = 0x1234_5678;
    city.mac_rci_counters_runtime = [11, 22, 33, 44, 55, 66, 77, 88];
    city.mac_transit_accumulators_runtime = [101, 202, 303];
    for (index, count) in city.mac_xbld_census_runtime.iter_mut().enumerate() {
        *count = (index as u16).wrapping_mul(17);
    }
    city.mac_kind2_hotspot_runtime = Some((0x1234, 0xabcd));
    city.mac_arcology_total_runtime = Some(0x1020_3040);
    city.mac_police_gate_runtime = -321;
    city.mac_power_summary_runtime = 0x5060_7080;
    city.mac_water_summary_runtime = 0x90a0_b0c0;
    city.mac_treatment_gate_runtime = Some(-1234);
    city.mac_developed_cell_snapshot = Some(0x9876);
    city.set_misc_u32_at(MISC_TRIGGER_DISASTER_OFFSET, 5);

    city.save(&path).expect("save Win-layout city");
    let mut loaded = City::load(&path).expect("reload Win-layout city");
    let extension = decode_citygame_extension(&loaded.chunks).expect("decode Mac state extension");
    assert_eq!(extension.version, 15);
    assert_eq!(extension.mac_pending_action, 0x1234_5678);
    assert_eq!(
        extension
            .mac_saved_state
            .as_ref()
            .and_then(|state| state.mac_scenario_active),
        Some(false)
    );
    assert!(!loaded.mac_cross_layout_active);
    assert!(loaded.mac_cross_layout_state_restored);
    loaded.prepare_mac_fidelity_from_saved_city();

    assert_eq!(
        loaded.mac_rci_counters_runtime,
        [11, 22, 33, 44, 55, 66, 77, 88]
    );
    assert_eq!(loaded.mac_transit_accumulators_runtime, [101, 202, 303]);
    assert_eq!(loaded.mac_xbld_census_runtime, city.mac_xbld_census_runtime);
    assert_eq!(loaded.mac_kind2_hotspot_runtime, Some((0x1234, 0xabcd)));
    assert_eq!(loaded.mac_arcology_total_runtime, Some(0x1020_3040));
    assert_eq!(loaded.pe_dw_arcology_population(), 0x1020_3040);
    assert_eq!(
        loaded.misc_u32_at(MISC_ARCOLOGY_POPULATION_OFFSET),
        Some(0x1020_3040)
    );
    assert_eq!(loaded.mac_police_gate_runtime, -321);
    assert_eq!(loaded.mac_power_summary_runtime, 0x5060_7080);
    assert_eq!(loaded.mac_water_summary_runtime, 0x90a0_b0c0);
    assert_eq!(loaded.mac_treatment_gate_runtime, Some(-1234));
    assert_eq!(loaded.mac_developed_cell_snapshot, Some(0x9876));
    assert_eq!(loaded.misc_u32_at(MISC_TRIGGER_DISASTER_OFFSET), Some(5));
    assert_eq!(loaded.mac_pending_action_runtime, 0x1234_5678);

    // A Win95 save/reload leaves the extension snapshot intact unless Mac
    // fidelity is active and has updated the runtime fields.
    loaded.mac_cross_layout_active = false;
    loaded.save(&path).expect("resave Win-layout city");
    let reloaded = City::load(&path).expect("reload resaved Win-layout city");
    assert_eq!(
        reloaded.mac_rci_counters_runtime,
        [11, 22, 33, 44, 55, 66, 77, 88]
    );
    assert_eq!(
        reloaded.mac_xbld_census_runtime,
        city.mac_xbld_census_runtime
    );
    assert_eq!(reloaded.misc_u32_at(MISC_TRIGGER_DISASTER_OFFSET), Some(5));
    let _ = std::fs::remove_file(path);
}

#[test]
fn pe_cross_layout_history_and_trigger_round_trip_without_overwriting_mac_slots() {
    let path = std::env::temp_dir().join(format!(
        "sc2000-pe-res-pop-mac-layout-{}.city",
        std::process::id()
    ));
    let mut city = City::new_blank(&path, "PeResidentialHistory").expect("blank city");
    city.kind = CityFileKind::MacCity;
    city.pe_dw_residential_population = 0x1234_5678;
    city.pe_w_trigger_disaster = 7;
    city.set_mac_state_misc_u32_at(MISC_TRIGGER_DISASTER_OFFSET, 0xfeed_0077);
    city.set_mac_state_misc_u32_at(MISC_RESIDENTIAL_POPULATION_OFFSET, 0xfeed_beef);

    city.save(&path).expect("save Mac-layout city");

    let mut loaded = City::load(&path).expect("reload Mac-layout city");
    assert_eq!(loaded.kind, CityFileKind::MacCity);
    assert_eq!(loaded.pe_dw_residential_population, 0x1234_5678);
    assert_eq!(loaded.pe_w_trigger_disaster, 7);
    assert_eq!(
        loaded.mac_state_misc_u32_at(MISC_TRIGGER_DISASTER_OFFSET),
        Some(0xfeed_0077),
        "Win95's pending trigger must not replace Mac's native +0x70 slot"
    );
    assert_eq!(
        loaded.mac_state_misc_u32_at(MISC_RESIDENTIAL_POPULATION_OFFSET),
        Some(0xfeed_beef),
        "Win95's previous-row mirror must not replace Mac's native +0x74 slot"
    );
    let extension = decode_citygame_extension(&loaded.chunks).expect("decode CGEX v4");
    assert_eq!(extension.version, 4);
    assert_eq!(extension.pe_residential_population, Some(0x1234_5678));
    assert_eq!(extension.pe_trigger_disaster, Some(7));
    assert_eq!(extension.mac_saved_state, None);

    // The decoded selected-engine cache remains available through another save.
    loaded.save(&path).expect("save reloaded city");
    let reloaded = City::load(&path).expect("reload second save");
    assert_eq!(reloaded.pe_dw_residential_population, 0x1234_5678);
    assert_eq!(reloaded.pe_w_trigger_disaster, 7);
    assert_eq!(
        reloaded.mac_state_misc_u32_at(MISC_TRIGGER_DISASTER_OFFSET),
        Some(0xfeed_0077)
    );
    assert_eq!(
        reloaded.mac_state_misc_u32_at(MISC_RESIDENTIAL_POPULATION_OFFSET),
        Some(0xfeed_beef)
    );
    let _ = std::fs::remove_file(path);
}

#[test]
fn pe_cross_layout_runtime_state_round_trips_without_overwriting_mac_slots() {
    let path = std::env::temp_dir().join(format!(
        "citygame-pe-disaster-mac-layout-{}.city",
        std::process::id()
    ));
    let mut source = City::new_blank(&path, "MacDisasterState").expect("blank city");
    source.kind = CityFileKind::MacCity;
    source.set_mac_state_misc_u32_at(MISC_CURRENT_DISASTER_TYPE_OFFSET, 7);
    source.set_mac_state_misc_u32_at(MISC_DISASTER_ACTIVE_OFFSET, 1);
    source.set_mac_state_misc_u32_at(MISC_OLD_ARRESTS_OFFSET, 31);
    source.set_mac_state_misc_u32_at(MISC_PRISON_BONUS_OFFSET, 1);
    source.set_mac_state_misc_u32_at(MISC_SEWER_BONUS_OFFSET, 1);
    source.save(&path).expect("save unextended Mac-layout city");

    let mut source = City::load(&path).expect("load unextended Mac-layout city");
    assert_eq!(source.pe_w_current_disaster_type(), 0);
    assert_eq!(source.pe_dw_disaster_active(), 0);
    assert_eq!(source.pe_w_old_arrests, 0);
    assert_eq!(source.pe_w_prison_bonus, 0);
    assert_eq!(source.pe_w_sewer_bonus, 0);
    source.prepare_pe_fidelity_from_saved_city();
    assert_eq!(source.pe_w_current_disaster_type(), 7);
    assert_eq!(source.pe_dw_disaster_active(), 1);
    assert_eq!(source.pe_w_old_arrests, 31);
    assert_eq!(source.pe_w_prison_bonus, 1);
    assert_eq!(source.pe_w_sewer_bonus, 1);
    source.pe_dw_residential_population = 0x1234_5678;
    source.pe_w_trigger_disaster = 9;
    source.prepare_pe_fidelity_from_saved_city();
    source.set_pe_w_neighbor_fame_rank(0, 0x8123);
    source.set_pe_w_current_disaster_type(0x12);
    source.set_pe_dw_disaster_active(2);
    source.pe_w_old_arrests = 0x2345;
    source.pe_w_prison_bonus = 2;
    source.pe_w_sewer_bonus = 0;
    source.prepare_pe_fidelity_from_saved_city();
    assert_eq!(source.pe_w_current_disaster_type(), 0x12);
    assert_eq!(source.pe_dw_disaster_active(), 2);
    source.save(&path).expect("save Mac-layout city");

    let mut loaded = City::load(&path).expect("reload Mac-layout city");
    assert_eq!(loaded.kind, CityFileKind::MacCity);
    assert_eq!(loaded.pe_w_current_disaster_type(), 0);
    assert_eq!(loaded.pe_dw_disaster_active(), 0);
    loaded.prepare_pe_fidelity_from_saved_city();
    assert_eq!(loaded.pe_w_current_disaster_type(), 0x12);
    assert_eq!(loaded.pe_dw_disaster_active(), 2);
    assert_eq!(loaded.pe_w_old_arrests, 0x2345);
    assert_eq!(loaded.pe_w_prison_bonus, 2);
    assert_eq!(loaded.pe_w_sewer_bonus, 0);
    assert_eq!(
        loaded.mac_state_misc_u32_at(MISC_CURRENT_DISASTER_TYPE_OFFSET),
        Some(7),
        "PE current-disaster type must not replace the Mac-owned MISC slot"
    );
    assert_eq!(
        loaded.mac_state_misc_u32_at(MISC_DISASTER_ACTIVE_OFFSET),
        Some(1),
        "PE active-disaster flag must not replace the Mac-owned MISC slot"
    );
    let extension = decode_citygame_extension(&loaded.chunks).expect("decode CGEX v16");
    assert_eq!(extension.version, 16);
    assert_eq!(extension.pe_residential_population, Some(0x1234_5678));
    assert_eq!(extension.pe_trigger_disaster, Some(9));
    assert_eq!(extension.pe_neighbor_fame_ranks.unwrap()[0], 0x8123);
    assert_eq!(extension.pe_current_disaster_type, Some(0x12));
    assert_eq!(extension.pe_disaster_active, Some(2));
    assert_eq!(extension.pe_old_arrests, Some(0x2345));
    assert_eq!(extension.pe_prison_bonus, Some(2));
    assert_eq!(extension.pe_sewer_bonus, Some(0));
    assert_eq!(extension.pe_city_has_ocean, Some(0));
    assert_eq!(extension.pe_city_has_river, Some(0));
    assert_eq!(extension.pe_xund_occupancy, Some(0));

    loaded.prepare_pe_fidelity_from_saved_city();
    loaded.set_pe_w_current_disaster_type(0);
    loaded.set_pe_dw_disaster_active(0);
    loaded.pe_w_old_arrests = 0;
    loaded.pe_w_prison_bonus = 0;
    loaded.pe_w_sewer_bonus = 1;
    loaded.save(&path).expect("save cleared PE disaster state");
    let mut reloaded = City::load(&path).expect("reload cleared PE disaster state");
    assert_eq!(reloaded.pe_w_current_disaster_type(), 0);
    assert_eq!(reloaded.pe_dw_disaster_active(), 0);
    assert_eq!(reloaded.pe_w_old_arrests, 0);
    assert_eq!(reloaded.pe_w_prison_bonus, 0);
    reloaded.prepare_pe_fidelity_from_saved_city();
    assert_eq!(reloaded.pe_w_sewer_bonus, 1);
    assert_eq!(
        reloaded.mac_state_misc_u32_at(MISC_CURRENT_DISASTER_TYPE_OFFSET),
        Some(7)
    );
    assert_eq!(
        reloaded.mac_state_misc_u32_at(MISC_DISASTER_ACTIVE_OFFSET),
        Some(1)
    );
    assert_eq!(
        reloaded.mac_state_misc_u32_at(MISC_OLD_ARRESTS_OFFSET),
        Some(31)
    );
    assert_eq!(
        reloaded.mac_state_misc_u32_at(MISC_PRISON_BONUS_OFFSET),
        Some(1)
    );
    assert_eq!(
        reloaded.mac_state_misc_u32_at(MISC_SEWER_BONUS_OFFSET),
        Some(1)
    );
    let _ = std::fs::remove_file(path);
}

#[test]
fn pe_prior_residential_population_v3_remains_readable_on_mac_layout() {
    let path = std::env::temp_dir().join(format!(
        "sc2000-pe-res-pop-v3-mac-layout-{}.city",
        std::process::id()
    ));
    let mut city = City::new_blank(&path, "PeResidentialHistoryV3").expect("blank city");
    city.kind = CityFileKind::MacCity;
    city.pe_dw_residential_population = 0x1234_5678;
    city.set_mac_state_misc_u32_at(MISC_RESIDENTIAL_POPULATION_OFFSET, 0xfeed_beef);

    city.save(&path).expect("save Mac-layout city");
    let loaded = City::load(&path).expect("reload Mac-layout city");

    let extension = decode_citygame_extension(&loaded.chunks).expect("decode CGEX v3");
    assert_eq!(extension.version, 3);
    assert_eq!(extension.pe_residential_population, Some(0x1234_5678));
    assert_eq!(extension.pe_trigger_disaster, None);
    assert_eq!(loaded.pe_dw_residential_population, 0x1234_5678);
    assert_eq!(
        loaded.mac_state_misc_u32_at(MISC_RESIDENTIAL_POPULATION_OFFSET),
        Some(0xfeed_beef)
    );
    let _ = std::fs::remove_file(path);
}

#[test]
fn first_mac_open_of_win_city_uses_native_raw_saved_slots() {
    let path = std::env::temp_dir().join(format!(
        "sc2000-mac-service-summaries-cross-layout-{}.sc2",
        std::process::id()
    ));
    let power_misc = 1048 * MISC_FIELD_LEN;
    let water_misc = 1049 * MISC_FIELD_LEN;
    let police_gate_misc = 1039 * MISC_FIELD_LEN;
    let treatment_gate_misc = 1043 * MISC_FIELD_LEN;
    let hotspot_x_misc = 1030 * MISC_FIELD_LEN;
    let hotspot_y_misc = 1031 * MISC_FIELD_LEN;
    let mut city = City::new_blank(&path, "MacServiceSummaries").expect("blank Win-layout city");
    city.set_misc_u32_at(power_misc, 0x1234_5678);
    city.set_misc_u32_at(water_misc, 0x8765_4321);
    city.set_misc_u32_at(police_gate_misc, 0xffff_fffe);
    // The Win95 save owns this overlapping slot as wPrisonBonus. Preserve the
    // intended raw word through the PE save path before opening it as Mac.
    city.set_pe_w_prison_bonus(0xfffe);
    city.set_misc_u32_at(treatment_gate_misc, 3);
    city.set_misc_u32_at(hotspot_x_misc, 41);
    city.set_misc_u32_at(hotspot_y_misc, 73);
    city.save(&path).expect("save Win-layout fixture");

    let mut loaded = City::load(&path).expect("load Win-layout fixture");
    loaded.prepare_mac_fidelity_from_saved_city();
    assert_eq!(loaded.mac_power_service_summary(), 0x1234_5678);
    assert_eq!(loaded.mac_water_service_summary(), 0x8765_4321);
    assert_eq!(loaded.mac_police_coverage_gate(), -2);
    assert_eq!(loaded.mac_treatment_gate(), 3);
    assert_eq!(loaded.mac_kind2_hotspot(), Some((41, 73)));
    assert_eq!(loaded.misc_u32_at(power_misc), Some(0x1234_5678));
    assert_eq!(loaded.misc_u32_at(water_misc), Some(0x8765_4321));

    loaded
        .save(&path)
        .expect("save Mac-fidelity Win-layout fixture");
    let mut reloaded = City::load(&path).expect("reload Mac-fidelity Win-layout fixture");
    reloaded.prepare_mac_fidelity_from_saved_city();
    assert_eq!(reloaded.mac_power_service_summary(), 0x1234_5678);
    assert_eq!(reloaded.mac_water_service_summary(), 0x8765_4321);
    assert_eq!(reloaded.mac_police_coverage_gate(), -2);
    assert_eq!(reloaded.mac_treatment_gate(), 3);
    assert_eq!(reloaded.mac_kind2_hotspot(), Some((41, 73)));
    assert_eq!(reloaded.misc_u32_at(power_misc), Some(0x1234_5678));
    assert_eq!(reloaded.misc_u32_at(water_misc), Some(0x8765_4321));
    let _ = std::fs::remove_file(path);
}

#[test]
fn first_mac_open_of_win_city_uses_native_misc_xbld_census_and_keeps_pe_misc() {
    let path = std::env::temp_dir().join(format!(
        "sc2000-mac-xbld-census-cross-layout-{}.sc2",
        std::process::id()
    ));
    let census_d2 = MAC_XBLD_CENSUS_MISC_OFFSET + 0xd2 * MISC_FIELD_LEN;
    let census_empty = MAC_XBLD_CENSUS_MISC_OFFSET;
    let mut city = City::new_blank(&path, "MacXbldCensus").expect("blank Win-layout city");
    city.set_building(4, 4, 0xd2);
    city.set_zone(4, 5, 7);
    city.set_building(4, 5, 0xd2);
    city.set_misc_u32_at(census_empty, 0xaabb_1111);
    city.set_misc_u32_at(census_d2, 0x1234_5678);
    city.save(&path).expect("save Win-layout fixture");

    let mut loaded = City::load(&path).expect("load Win-layout fixture");
    assert_eq!(loaded.pe_dw_tile_count[0], (CITY_TILES - 2) as u16);
    assert_eq!(loaded.pe_dw_tile_count[0xd2], 1);
    let saved_pe_misc = loaded.misc_u32_at(census_d2);
    assert_eq!(saved_pe_misc, Some(0x1234_5678));
    loaded.prepare_mac_fidelity_from_saved_city();
    assert_eq!(loaded.mac_xbld_tile_count(0xd2), 0x5678);
    assert_eq!(
        loaded.mac_xbld_tile_count(0),
        0x1111,
        "Mac's loader keeps the low WORD saved at its native MISC offset"
    );
    assert_eq!(loaded.misc_u32_at(census_d2), saved_pe_misc);

    loaded.set_building(4, 4, 0xf5);
    assert_eq!(loaded.mac_xbld_tile_count(0xd2), 0x5677);
    assert_eq!(loaded.mac_xbld_tile_count(0xf5), 1);
    assert_eq!(
        loaded.misc_u32_at(census_d2),
        saved_pe_misc,
        "Mac runtime census edits must not be written into the Win95 MISC range"
    );
    loaded
        .save(&path)
        .expect("save Mac-fidelity Win-layout city");

    let mut reloaded = City::load(&path).expect("reload Mac-fidelity Win-layout city");
    reloaded.prepare_mac_fidelity_from_saved_city();
    assert_eq!(reloaded.mac_xbld_tile_count(0xd2), 0x5677);
    assert_eq!(reloaded.mac_xbld_tile_count(0xf5), 1);
    assert_eq!(reloaded.misc_u32_at(census_d2), saved_pe_misc);
    let _ = std::fs::remove_file(path);
}

#[test]
fn win_xlab_save_uses_length_prefixed_23_byte_rows() {
    let mut path = std::env::temp_dir();
    path.push(format!("sc2000-xlab-limit-{}.sc2", std::process::id()));
    let mut city = City::new_blank(&path, "test").expect("blank city");
    let input = "abcdefghijklmnopqrstuvwx";
    city.set_label(7, input);
    city.save(&path).expect("save city");

    let raw = decode_expected_chunk(
        &city.chunks,
        "XLAB",
        CITY_LABEL_COUNT * CITY_LABEL_RECORD_LEN,
    )
    .expect("decode saved XLAB");
    let row = 7 * CITY_LABEL_RECORD_LEN;
    assert_eq!(raw.len(), CITY_LABEL_COUNT * CITY_LABEL_RECORD_LEN);
    assert_eq!(raw[row], CITY_LABEL_TEXT_MAX as u8);
    assert_eq!(
        &raw[row + 1..row + 1 + CITY_LABEL_TEXT_MAX],
        b"abcdefghijklmnopqrstuvw"
    );
    assert_eq!(raw[row + CITY_LABEL_RECORD_LEN - 1], 0);

    let loaded = City::load(&path).expect("reload city");
    assert_eq!(loaded.label_at(7), Some("abcdefghijklmnopqrstuvw"));
    let _ = std::fs::remove_file(path);
}

#[test]
fn pe_national_prosperity_and_trend_thresholds() {
    assert_eq!(pe_national_prosperity(3_000, 10_000), 29);
    // The PE divides in extended x87 precision, multiplies by 100, then
    // chops. This exact 53 boundary stays just below 53 in that sequence.
    assert_eq!(pe_national_prosperity(2_650_000, 4_999_999), 52);
    assert_eq!(pe_neighbor_prosperity(2_650_000, 5_000_000), 52);
    assert_eq!(pe_national_trend_from_prosperity(44), 0);
    assert_eq!(pe_national_trend_from_prosperity(45), 1);
    assert_eq!(pe_national_trend_from_prosperity(59), 1);
    assert_eq!(pe_national_trend_from_prosperity(60), 2);
    assert_eq!(pe_national_trend_from_prosperity(74), 2);
    assert_eq!(pe_national_trend_from_prosperity(75), 3);
}

#[test]
fn pe_neighbor_value_drift_uses_ftol_low_dword_for_large_signed_speed() {
    // PE `_ftol` stores a signed qword, but the following `sub ecx,eax`
    // consumes only the low dword. A signed MISC tax word can make this
    // intermediate exceed i32; a saturating float-to-i32 cast is wrong.
    assert_eq!(
        pe_national_apply_drift(4_000_000_000, PE_NATIONAL_VALUE_CAP, -32_768),
        1_557_516_970
    );
}

#[test]
fn pe_national_prosperity_x87_boundary_controls_tax_roll() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "nat53").expect("blank");
    city.set_misc_u32_at(MISC_NATIONAL_POPULATION_OFFSET, 4_999_999);
    city.set_misc_u32_at(MISC_NATIONAL_VALUE_OFFSET, 2_636_816);
    city.set_misc_u32_at(MISC_NATIONAL_TAX_OFFSET, 3);
    city.set_misc_u32_at(MISC_NATIONAL_TREND_OFFSET, 0);

    // Drift first produces pop=4,999,999 and value=2,650,000. CRT draws
    // are 4770, 17360, 19702, 5026, 12465; the first tax roll is 52.
    // PE prosperity is 52, so the strict `roll < prosperity` gate holds.
    let mut rng = Win95CrtRand::new(1449);
    city.advance_pe_national_economy_month(&mut rng);

    assert_eq!(
        city.misc_u32_at(MISC_NATIONAL_POPULATION_OFFSET),
        Some(4_999_999)
    );
    assert_eq!(
        city.misc_u32_at(MISC_NATIONAL_VALUE_OFFSET),
        Some(2_650_000)
    );
    assert_eq!(city.misc_u32_at(MISC_NATIONAL_TAX_OFFSET), Some(3));
    assert_eq!(city.misc_u32_at(MISC_NATIONAL_TREND_OFFSET), Some(1));
    assert_eq!(rng.seed(), 0xb0b1_a10c);
}

#[test]
fn mac_national_word_fields_load_low_word_and_sign_extend_on_save() {
    // CODE 4 load reads MISC+0x5a/+0x5e as WORDs. Saveback uses
    // MOVEA.W+MOVE.L for each, so each is sign-extended to its dword slot.
    let path = std::env::temp_dir().join(format!(
        "sc2000_mac_national_word_fields_{}.mac",
        std::process::id()
    ));
    let mut city = City::new_blank(&path, "MacNationalWords").expect("blank city");
    city.kind = CityFileKind::MacCity;
    city.set_misc_u32_at(MISC_NATIONAL_TAX_OFFSET, 0xcafe_0003);
    city.set_misc_u32_at(MISC_NATIONAL_TREND_OFFSET, 0xbeef_0002);
    assert_eq!(city.pe_w_national_tax, 3);
    assert_eq!(city.pe_w_national_trend, 2);

    city.pe_w_national_tax = 0x8001;
    city.pe_w_national_trend = 0x8002;
    city.save(&path).expect("save Mac city");
    assert_eq!(
        city.misc_u32_at(MISC_NATIONAL_TAX_OFFSET),
        Some(0xffff_8001)
    );
    assert_eq!(
        city.misc_u32_at(MISC_NATIONAL_TREND_OFFSET),
        Some(0xffff_8002)
    );
    let loaded = City::load(&path).expect("reload Mac city");
    assert_eq!(loaded.pe_w_national_tax, 0x8001);
    assert_eq!(loaded.pe_w_national_trend, 0x8002);
    let _ = std::fs::remove_file(path);
}

#[test]
fn mac_national_event_matches_native_replay_fixture() {
    let fixture = include_str!("../../../tests/fixtures/simulation/mac/mac_national_event.csv");
    for line in fixture.lines().skip(1) {
        let parts: Vec<_> = line.split('"').collect();
        assert_eq!(parts.len(), 5, "fixture row: {line}");
        let input: Vec<u16> = parts[0]
            .trim_end_matches(',')
            .split(',')
            .map(|value| value.parse().unwrap())
            .collect();
        let draws: Vec<u16> = parts[1]
            .trim_matches(['[', ']'])
            .split(',')
            .map(|value| value.trim().parse().unwrap())
            .collect();
        let expected: Vec<u16> = parts[2]
            .trim_matches(',')
            .split(',')
            .map(|value| value.parse().unwrap())
            .collect();
        let stories: Vec<(u16, u16)> = if parts[3].is_empty() {
            Vec::new()
        } else {
            parts[3]
                .split(';')
                .map(|pair| {
                    let (selector, value) = pair.split_once(':').unwrap();
                    (selector.parse().unwrap(), value.parse().unwrap())
                })
                .collect()
        };
        let mut iter = draws.iter().copied();
        // With population 99, value == score in the recovered arithmetic.
        let result = mac_national_event(u32::from(input[0]), 99, input[1], input[2], || {
            iter.next().expect("unexpected native draw")
        })
        .expect("supported fixture");
        assert_eq!(
            (result.tax, result.trend),
            (expected[0], expected[1]),
            "{line}"
        );
        assert_eq!(result.stories, stories, "{line}");
        assert_eq!(iter.next(), None, "unused native draw: {line}");
        assert_eq!(
            draws.len(),
            parts[4].trim_start_matches(',').parse::<usize>().unwrap()
        );
    }
}

#[test]
fn mac_national_event_preserves_native_draw_and_story_order() {
    let mut draws = [10, 0, 1, 200, 0].into_iter();
    let result = mac_national_event(2_650_000, 4_999_999, 4, 3, || {
        draws.next().expect("unexpected draw")
    })
    .expect("supported event");
    assert_eq!(result.tax, 4);
    assert_eq!(result.trend, 1);
    assert_eq!(result.stories, vec![(9, 5), (10, 4), (7, 1)]);
    assert_eq!(draws.next(), None);

    let mut draws = [10, 0, 0, 1, 1].into_iter();
    let result = mac_national_event(0, 0, 1, 2, || draws.next().expect("unexpected draw"))
        .expect("supported event");
    assert_eq!((result.tax, result.trend), (1, 2));
    assert!(
        result.stories.is_empty(),
        "flooring tax to one emits no story"
    );
    assert_eq!(draws.next(), None);

    let mut draws = [1, 99].into_iter();
    let result = mac_national_event(0, u32::MAX, 0, 2, || draws.next().expect("unexpected draw"))
        .expect("event skipped");
    assert_eq!((result.tax, result.trend), (0, 2));
    assert_eq!(
        draws.next(),
        Some(99),
        "closed event consumes just one draw"
    );

    let mut draws = [0, 0, 4].into_iter();
    assert!(mac_national_event(100, 10, 0, 0, || draws.next().expect("unexpected draw")).is_none());
    assert_eq!(draws.next(), None, "zero divisor is checked after the draw");

    for (score, expected) in [(44, 0), (45, 1), (59, 1), (60, 2), (74, 2), (75, 3)] {
        let mut draws = [0, 2, 0].into_iter();
        let result = mac_national_event(score, 99, 1, 9, || draws.next().expect("unexpected draw"))
            .expect("supported event");
        assert_eq!(result.trend, expected);
        assert_eq!(result.stories, vec![(7, expected)]);
        assert_eq!(draws.next(), None);
    }
}

#[test]
fn mac_national_prosperity_multiplies_before_dividing() {
    assert_eq!(mac_national_prosperity(2_650_000, 4_999_999), Some(53));
    assert_eq!(pe_national_prosperity(2_650_000, 4_999_999), 52);
    assert_eq!(mac_national_prosperity(0, 0), Some(0));
    assert_eq!(mac_national_prosperity(3_000, 9_999), Some(30));
    assert_eq!(mac_national_prosperity(1, u32::MAX), None);
    assert_eq!(mac_national_prosperity(u32::MAX, 0), None);
}

#[test]
fn mac_national_drift_matches_independent_extended80_samples() {
    let fixture = include_str!("../../../tests/fixtures/simulation/mac/mac_national_drift.csv");
    let mut count = 0;
    for line in fixture.lines().skip(1) {
        let mut fields = line.split(',');
        let current = fields.next().unwrap().parse().unwrap();
        let cap = fields.next().unwrap().parse().unwrap();
        let speed = fields.next().unwrap().parse().unwrap();
        let expected: u32 = fields.next().unwrap().parse().unwrap();
        assert!(fields.next().is_none());
        assert_eq!(
            mac_sane_national_apply_drift(current, cap, speed),
            expected,
            "{line}"
        );
        count += 1;
    }
    assert_eq!(count, 3228);
}

#[test]
fn mac_national_drift_rounds_single_delta_then_truncates_total() {
    // CODE 8 zero-extends each saved LONG into Comp, uses unsigned BLS for
    // the cap comparison, and stores the low LONG of the converted result.
    assert_eq!(
        mac_sane_national_apply_drift(0x8000_0000, MAC_NATIONAL_POP_CAP, 3),
        0x7fae_147b
    );
    assert_eq!(
        mac_sane_national_apply_drift(u32::MAX, MAC_NATIONAL_VALUE_CAP, -3),
        0x00a3_d709
    );
    // Mac converts the drift to SANE single before subtracting. FINTRZ
    // then truncates the full result, which differs from PE's truncation
    // of the drift before subtraction for this above-cap value.
    assert_eq!(
        mac_sane_national_apply_drift(5_000_001, MAC_NATIONAL_POP_CAP, 1),
        4_995_834
    );
    assert_eq!(
        pe_national_apply_drift(5_000_001, PE_NATIONAL_POP_CAP, 1),
        4_995_835
    );

    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "macnatdrift").expect("blank");
    city.kind = CityFileKind::MacCity;
    city.set_misc_u32_at(MISC_NATIONAL_POPULATION_OFFSET, 5_000_001);
    city.set_misc_u32_at(MISC_NATIONAL_VALUE_OFFSET, 3_499_999);
    // Mac reads the low saved WORD; the opaque high half is retained.
    city.set_misc_u32_at(MISC_NATIONAL_TREND_OFFSET, 0xbeef_0003);
    city.advance_mac_national_metric_drift_month();
    assert_eq!(
        city.misc_u32_at(MISC_NATIONAL_POPULATION_OFFSET),
        Some(4_987_500)
    );
    assert_eq!(
        city.misc_u32_at(MISC_NATIONAL_VALUE_OFFSET),
        Some(3_491_249)
    );
    assert_eq!(
        city.misc_u32_at(MISC_NATIONAL_TREND_OFFSET),
        Some(0xbeef_0003)
    );
}

#[test]
fn mac_national_drift_accumulates_for_both_city_layouts() {
    for kind in [CityFileKind::MacCity, CityFileKind::WinSc2] {
        let mut city = City::new_blank("/tmp/mac-national-drift.sc2", "Drift").unwrap();
        city.kind = kind;
        let mut population = 4_900_000;
        let mut value = 3_400_000;
        city.set_misc_u32_at(MISC_NATIONAL_POPULATION_OFFSET, population);
        city.set_misc_u32_at(MISC_NATIONAL_VALUE_OFFSET, value);
        city.set_misc_u32_at(MISC_NATIONAL_TREND_OFFSET, 1);
        for _ in 0..3 {
            population = mac_sane_national_apply_drift(population, MAC_NATIONAL_POP_CAP, 1);
            value = mac_sane_national_apply_drift(value, MAC_NATIONAL_VALUE_CAP, 3);
            city.advance_mac_national_metric_drift_month();
            assert_eq!(city.pe_dw_national_population, population, "{kind:?}");
            assert_eq!(city.pe_dw_national_value, value, "{kind:?}");
            assert_eq!(
                city.misc_u32_at(MISC_NATIONAL_POPULATION_OFFSET),
                Some(population)
            );
            assert_eq!(city.misc_u32_at(MISC_NATIONAL_VALUE_OFFSET), Some(value));
        }
    }
}

#[test]
fn mac_difficulty_cache_reads_native_low_word() {
    let path = std::env::temp_dir().join(format!(
        "sc2000_mac_difficulty_word_{}.mac",
        std::process::id()
    ));
    let mut city = City::new_blank(&path, "MacDifficulty").expect("blank city");
    city.kind = CityFileKind::MacCity;
    city.set_misc_u32_at(MISC_DIFFICULTY_OFFSET, 0xcafe_0002);
    assert_eq!(city.pe_w_city_difficulty, 2);
    city.save(&path).expect("save Mac city");
    let loaded = City::load(&path).expect("reload Mac city");
    assert_eq!(loaded.pe_w_city_difficulty, 2);
    assert_eq!(
        loaded.misc_u32_at(MISC_DIFFICULTY_OFFSET),
        Some(0xcafe_0002)
    );
    let _ = std::fs::remove_file(path);
}

#[test]
fn pe_national_economy_mean_reverts_pop_and_uses_trend_table_for_value() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "nat").expect("blank");
    city.set_misc_u32_at(MISC_NATIONAL_POPULATION_OFFSET, 12_000);
    city.set_misc_u32_at(MISC_NATIONAL_VALUE_OFFSET, 3_600);
    city.set_misc_u32_at(MISC_NATIONAL_TAX_OFFSET, 1);
    city.set_misc_u32_at(MISC_NATIONAL_TREND_OFFSET, 1);
    // First CRT from seed 1 is 41 % 10 != 0: drift only.
    city.advance_pe_national_economy_month(&mut Win95CrtRand::new(1));
    assert_eq!(
        city.misc_u32_at(MISC_NATIONAL_POPULATION_OFFSET),
        Some(12_010)
    );
    assert_eq!(city.misc_u32_at(MISC_NATIONAL_VALUE_OFFSET), Some(3_609));
    assert_eq!(city.misc_u32_at(MISC_NATIONAL_TREND_OFFSET), Some(1));
    assert_eq!(city.misc_u32_at(MISC_NATIONAL_TAX_OFFSET), Some(1));
}

#[test]
fn pe_national_economy_shrinks_pop_above_five_million() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "natcap").expect("blank");
    city.set_misc_u32_at(MISC_NATIONAL_POPULATION_OFFSET, 5_000_100);
    city.set_misc_u32_at(MISC_NATIONAL_VALUE_OFFSET, 4_000_000);
    city.set_misc_u32_at(MISC_NATIONAL_TREND_OFFSET, 1);
    city.advance_pe_national_economy_month(&mut Win95CrtRand::new(1));
    assert_eq!(
        city.misc_u32_at(MISC_NATIONAL_POPULATION_OFFSET),
        Some(5_000_100 - 4_166)
    );
    assert_eq!(
        city.misc_u32_at(MISC_NATIONAL_VALUE_OFFSET),
        Some(4_000_000 - 10_000)
    );
}

#[test]
fn pe_national_economy_trend_zero_freezes_population() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "nat0").expect("blank");
    city.set_misc_u32_at(MISC_NATIONAL_POPULATION_OFFSET, 10_000);
    city.set_misc_u32_at(MISC_NATIONAL_VALUE_OFFSET, 3_000);
    city.set_misc_u32_at(MISC_NATIONAL_TREND_OFFSET, 0);
    city.advance_pe_national_economy_month(&mut Win95CrtRand::new(1));
    assert_eq!(
        city.misc_u32_at(MISC_NATIONAL_POPULATION_OFFSET),
        Some(10_000)
    );
    // Trend 0 still drifts value with table speed 6.
    assert_eq!(city.misc_u32_at(MISC_NATIONAL_VALUE_OFFSET), Some(3_015));
}

#[test]
fn pe_national_economy_may_queue_tax_and_trend_stories() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "natstory").expect("blank");
    city.set_misc_u32_at(MISC_NATIONAL_POPULATION_OFFSET, 10_000);
    city.set_misc_u32_at(MISC_NATIONAL_VALUE_OFFSET, 8_000);
    city.set_misc_u32_at(MISC_NATIONAL_TAX_OFFSET, 1);
    city.set_misc_u32_at(MISC_NATIONAL_TREND_OFFSET, 0);
    city.advance_pe_national_economy_month(&mut Win95CrtRand::new(19));
    assert_eq!(city.misc_u32_at(MISC_NATIONAL_VALUE_OFFSET), Some(8_040));
    assert_eq!(city.misc_u32_at(MISC_NATIONAL_TAX_OFFSET), Some(2));
    assert_eq!(city.pe_newspaper_story_id_at(0), Some(9));
    assert_eq!(city.misc_u32_at(MISC_NATIONAL_TREND_OFFSET), Some(0));
}

#[test]
fn pe_newspaper_queue_uses_priority_and_monthly_decay_tables() {
    let mut city =
        City::new_blank("/tmp/sc2000-pe-story-aging.sc2", "StoryAging").expect("blank city");
    city.queue_pe_newspaper_story(8, 4);
    city.queue_pe_newspaper_story(2, 7);
    city.queue_pe_newspaper_story(4, 3);

    assert_eq!(city.pe_newspaper_story_id_at(0), Some(4));
    assert_eq!(city.pe_newspaper_story_priority_at(0), Some(1000));
    assert_eq!(city.pe_newspaper_story_id_at(1), Some(2));
    assert_eq!(city.pe_newspaper_story_priority_at(1), Some(1000));
    assert_eq!(city.pe_newspaper_story_id_at(2), Some(8));
    assert_eq!(city.pe_newspaper_story_priority_at(2), Some(200));

    city.age_pe_newspaper_story_queue();

    assert_eq!(city.pe_newspaper_story_id_at(0), Some(4));
    assert_eq!(city.pe_newspaper_story_priority_at(0), Some(750));
    assert_eq!(city.pe_newspaper_story_param_at(0), Some(3));
    assert_eq!(city.pe_newspaper_story_id_at(1), Some(2));
    assert_eq!(city.pe_newspaper_story_priority_at(1), Some(500));
    assert_eq!(city.pe_newspaper_story_param_at(1), Some(7));
    assert_eq!(city.pe_newspaper_story_id_at(2), Some(8));
    assert_eq!(city.pe_newspaper_story_priority_at(2), Some(150));
    assert_eq!(city.pe_newspaper_story_param_at(2), Some(4));
    assert_eq!(city.pe_newspaper_story_id_at(7), Some(18));
    assert_eq!(city.pe_newspaper_story_id_at(8), Some(19));
}

#[test]
fn pe_newspaper_queue_aging_preserves_mac_owned_misc() {
    let mut city =
        City::new_blank("/tmp/sc2000-mac-story-aging.sc2", "MacStoryAging").expect("blank city");
    city.kind = CityFileKind::MacCity;
    let sentinels: Vec<u32> = (0..NEWSPAPER_STORY_LIST_MISC_DWORDS as u32)
        .map(|index| 0xa500_0000 | index)
        .collect();
    city.set_misc_u32_values(MISC_NEWSPAPER_STORY_LIST_OFFSET, &sentinels);

    city.age_pe_newspaper_story_queue();

    assert_eq!(
        city.misc_u32_values(
            MISC_NEWSPAPER_STORY_LIST_OFFSET,
            NEWSPAPER_STORY_LIST_MISC_DWORDS,
        ),
        sentinels
    );
}

fn seed_pe_neighbor(city: &mut City, index: usize, name: u32, pop: u32, value: u32, fame: u32) {
    city.set_misc_u32_at(
        pe_neighbor_field_offset(index, PE_NEIGHBOR_NAME_FIELD),
        name,
    );
    city.set_misc_u32_at(pe_neighbor_field_offset(index, PE_NEIGHBOR_POP_FIELD), pop);
    city.set_misc_u32_at(
        pe_neighbor_field_offset(index, PE_NEIGHBOR_VALUE_FIELD),
        value,
    );
    city.set_misc_u32_at(
        pe_neighbor_field_offset(index, PE_NEIGHBOR_FAME_FIELD),
        fame,
    );
}

#[test]
fn pe_new_city_neighbor_initialization_matches_reset_draws() {
    let mut city =
        City::new_blank("/tmp/sc2000-neighbor-init.sc2", "Neighbors").expect("blank city");
    let mut crt = Win95CrtRand::new(2);
    city.pe_initialize_new_city_neighbors_40e250(&mut crt);

    let expected = [
        (10, 2_098, 2_098),
        (31, 3_405, 1_702),
        (14, 706, 235),
        (6, 867, 289),
    ];
    for (index, (name, population, value)) in expected.into_iter().enumerate() {
        let base = pe_neighbor_field_offset(index, PE_NEIGHBOR_NAME_FIELD);
        assert_eq!(city.misc_u32_at(base), Some(name));
        assert_eq!(
            city.misc_u32_at(base + PE_NEIGHBOR_POP_FIELD),
            Some(population)
        );
        assert_eq!(
            city.misc_u32_at(base + PE_NEIGHBOR_VALUE_FIELD),
            Some(value)
        );
        assert_eq!(city.misc_u32_at(base + PE_NEIGHBOR_FAME_FIELD), Some(0));
    }
    // Seed 2 forces the fourth name to retry once against an earlier row.
    assert_eq!(crt.seed(), 723_476_577);
}

#[test]
fn pe_neighbor_economy_skips_zero_pop_cities() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "nskip").expect("blank");
    city.set_misc_u32_at(MISC_NATIONAL_TREND_OFFSET, 1);
    city.set_misc_u32_at(MISC_NATIONAL_TAX_OFFSET, 1);
    seed_pe_neighbor(&mut city, 0, 7, 0, 8_000, 42);
    seed_pe_neighbor(&mut city, 2, 3, 12_000, 3_600, 9);
    let mut rng = Win95CrtRand::new(1);
    city.advance_pe_neighbor_economy_month(&mut rng);
    assert_eq!(
        city.misc_u32_at(pe_neighbor_field_offset(0, PE_NEIGHBOR_POP_FIELD)),
        Some(0)
    );
    assert_eq!(
        city.misc_u32_at(pe_neighbor_field_offset(0, PE_NEIGHBOR_VALUE_FIELD)),
        Some(8_000)
    );
    assert_eq!(
        city.misc_u32_at(pe_neighbor_field_offset(0, PE_NEIGHBOR_NAME_FIELD)),
        Some(7)
    );
    assert_eq!(
        city.misc_u32_at(pe_neighbor_field_offset(0, PE_NEIGHBOR_FAME_FIELD)),
        Some(42)
    );
    // Seed 1: rand%3=2 so speed=trend+2=3 → +30 pop, prosperity 29, value speed 4 → +12.
    assert_eq!(
        city.misc_u32_at(pe_neighbor_field_offset(2, PE_NEIGHBOR_POP_FIELD)),
        Some(12_030)
    );
    assert_eq!(
        city.misc_u32_at(pe_neighbor_field_offset(2, PE_NEIGHBOR_VALUE_FIELD)),
        Some(3_612)
    );
    assert_eq!(
        city.misc_u32_at(pe_neighbor_field_offset(2, PE_NEIGHBOR_FAME_FIELD)),
        Some(9)
    );
    // The three draws are live-row speed, live-row value speed, and the
    // post-loop crash gate. Each zero-pop row consumes none.
    assert_eq!(rng.seed(), 0x18be_873a);
}

#[test]
fn pe_neighbor_economy_adds_one_when_pop_delta_is_zero() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "njit").expect("blank");
    city.set_misc_u32_at(MISC_NATIONAL_TREND_OFFSET, 0);
    city.set_misc_u32_at(MISC_NATIONAL_TAX_OFFSET, 1);
    seed_pe_neighbor(&mut city, 0, 1, 10_000, 3_000, 0);
    // Seed 4: rand%3=0 → speed 0, delta 0, then rand&1=1.
    city.advance_pe_neighbor_economy_month(&mut Win95CrtRand::new(4));
    assert_eq!(
        city.misc_u32_at(pe_neighbor_field_offset(0, PE_NEIGHBOR_POP_FIELD)),
        Some(10_001)
    );
    assert_eq!(
        city.misc_u32_at(pe_neighbor_field_offset(0, PE_NEIGHBOR_VALUE_FIELD)),
        Some(3_022)
    );
}

#[test]
fn pe_neighbor_economy_shrinks_pop_above_five_million() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "ncap").expect("blank");
    city.set_misc_u32_at(MISC_NATIONAL_TREND_OFFSET, 1);
    city.set_misc_u32_at(MISC_NATIONAL_TAX_OFFSET, 1);
    seed_pe_neighbor(&mut city, 0, 0, 5_000_100, 4_000_000, 0);
    city.advance_pe_neighbor_economy_month(&mut Win95CrtRand::new(1));
    assert_eq!(
        city.misc_u32_at(pe_neighbor_field_offset(0, PE_NEIGHBOR_POP_FIELD)),
        Some(5_000_100 - 12_500)
    );
}

#[test]
fn pe_neighbor_economy_may_crash_one_city() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "ncrash").expect("blank");
    city.set_misc_u32_at(MISC_NATIONAL_TREND_OFFSET, 1);
    city.set_misc_u32_at(MISC_NATIONAL_TAX_OFFSET, 1);
    seed_pe_neighbor(&mut city, 0, 4, 10_000, 8_000, 11);
    // Seed 385: two drift rands, then `rand & 0x3f == 0` and `rand & 3 == 0`.
    city.advance_pe_neighbor_economy_month(&mut Win95CrtRand::new(385));
    assert_eq!(
        city.misc_u32_at(pe_neighbor_field_offset(0, PE_NEIGHBOR_POP_FIELD)),
        Some(7_518)
    );
    assert_eq!(
        city.misc_u32_at(pe_neighbor_field_offset(0, PE_NEIGHBOR_VALUE_FIELD)),
        Some(4_006)
    );
    assert_eq!(
        city.misc_u32_at(pe_neighbor_field_offset(0, PE_NEIGHBOR_NAME_FIELD)),
        Some(4)
    );
    assert_eq!(
        city.misc_u32_at(pe_neighbor_field_offset(0, PE_NEIGHBOR_FAME_FIELD)),
        Some(11)
    );
}

#[test]
fn pe_eq_le_calendar_month_runs_national_then_neighbors() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "ncal").expect("blank");
    city.set_misc_u32_at(MISC_NATIONAL_POPULATION_OFFSET, 12_000);
    city.set_misc_u32_at(MISC_NATIONAL_VALUE_OFFSET, 3_600);
    city.set_misc_u32_at(MISC_NATIONAL_TAX_OFFSET, 1);
    city.set_misc_u32_at(MISC_NATIONAL_TREND_OFFSET, 1);
    seed_pe_neighbor(&mut city, 0, 2, 12_000, 3_600, 5);
    city.advance_pe_eq_le_calendar_month(
        0,
        100,
        100,
        100,
        0,
        0,
        &mut Win95CrtRand::new(1),
        &mut Win95Lfsr::new(1),
    );
    assert_eq!(
        city.misc_u32_at(MISC_NATIONAL_POPULATION_OFFSET),
        Some(12_010)
    );
    assert_eq!(city.misc_u32_at(MISC_NATIONAL_VALUE_OFFSET), Some(3_609));
    // National consumes the first CRT call; neighbor speed still +3, value speed 6.
    assert_eq!(
        city.misc_u32_at(pe_neighbor_field_offset(0, PE_NEIGHBOR_POP_FIELD)),
        Some(12_030)
    );
    assert_eq!(
        city.misc_u32_at(pe_neighbor_field_offset(0, PE_NEIGHBOR_VALUE_FIELD)),
        Some(3_618)
    );
    assert_eq!(
        city.misc_u32_at(pe_neighbor_field_offset(0, PE_NEIGHBOR_FAME_FIELD)),
        Some(5)
    );
}

#[test]
fn day22_population_delta_immigrates_into_empty_eq_le_next_month() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "inj").expect("blank");
    city.persist_pe_day22_population_leftover(10_000);
    assert_eq!(city.pe_dw_city_population(), 10_000);
    assert_eq!(
        city.misc_u32_at(MISC_NORMAL_POPULATION_OFFSET),
        Some(10_000),
        "MISC 0x102c mirrors dwCityPopulation for save/load"
    );
    assert_eq!(city.pe_eq_leftover_deaths(), 10_000);
    assert_eq!(city.pe_eq_leftover_births(), 0);
    assert_eq!(city.pe_eq_cohort_population(), 0);

    city.advance_pe_eq_le_month(10_000, 0, 0, 0, 100, 100, 100, &mut Win95CrtRand::new(1));
    assert!(
        city.pe_eq_cohort_population() > 0,
        "leftover deaths fill working-age buckets when tables were empty"
    );
    assert!(city.pe_age_cohort_at(4) + city.pe_age_cohort_at(5) > 0);
    assert!(city.pe_eq_averages_ready());
    assert_eq!(city.pe_eq_leftover_deaths(), 0);
}

#[test]
fn dw_city_population_hydrates_from_misc_so_loaded_tables_are_not_wiped() {
    let path = std::env::temp_dir().join(format!("sc2000-eq-le-pop-{}.sc2", std::process::id()));
    let mut city = City::new_blank(&path, "hyd").expect("blank");
    city.test_seed_pe_age_cohorts_from_pop(8_000);
    city.set_misc_u32_at(MISC_NORMAL_POPULATION_OFFSET, 8_000);
    city.save(&path).expect("save");
    let mut loaded = City::load(&path).expect("reload");
    assert_eq!(loaded.pe_dw_city_population(), 8_000);
    loaded.advance_pe_eq_le_calendar_month(
        0,
        100,
        100,
        100,
        0,
        0,
        &mut Win95CrtRand::new(1),
        &mut Win95Lfsr::new(1),
    );
    assert!(
        loaded.pe_eq_cohort_population() > 0,
        "hydrated 0x4caa74 must keep a loaded pyramid out of the pop-0 wipe"
    );
    let _ = std::fs::remove_file(&path);
}

fn seed_pe_industry(city: &mut City, index: usize, demand: i32, tax: i32, pop: u32) {
    city.set_misc_u32_at(
        pe_industry_field_offset(index, PE_INDUSTRY_DEMAND_FIELD),
        demand as u32,
    );
    city.set_misc_u32_at(
        pe_industry_field_offset(index, PE_INDUSTRY_TAX_FIELD),
        tax as u32,
    );
    city.set_misc_u32_at(pe_industry_field_offset(index, PE_INDUSTRY_POP_FIELD), pop);
}

#[test]
fn pe_industry_era_weights_blend_adjacent_50_year_rows() {
    assert_eq!(
        pe_industry_era_weights(1900, 0)[0],
        20,
        "1900 uses row 0 steel weight"
    );
    assert_eq!(pe_industry_era_weights(1900, 25)[0], 25);
    assert_eq!(pe_industry_era_weights(1900, 200)[9], 80);
}

#[test]
fn pe_industry_demand_leaks_three_quarters_plus_era_jitter() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "indd").expect("blank");
    city.set_misc_u32_at(MISC_CITY_EDUCATION_QUOTIENT_OFFSET, 100);
    for index in 0..PE_INDUSTRY_COUNT {
        seed_pe_industry(&mut city, index, 0, 50, 0);
    }
    let mut lfsr = Win95Lfsr::new(1);
    city.advance_pe_industry_mix_month(0, 0, 0, &mut Win95CrtRand::new(1), &mut lfsr);
    let expected = [0, 1, 0, 1, 3, 1, 0, 3, 1, 0, 2];
    for (index, want) in expected.into_iter().enumerate() {
        assert_eq!(
            city.pe_industry_demand_at(index),
            want,
            "industry {index} demand"
        );
        assert_eq!(city.pe_industry_tax_at(index), 50);
        assert_eq!(city.pe_industry_pop_at(index), 0);
    }
    assert_eq!(city.misc_u32_at(MISC_INDUSTRIAL_MIX_BONUS_OFFSET), Some(0));
    assert_eq!(
        city.misc_i32_at(MISC_INDUSTRIAL_MIX_POLLUTION_BONUS_OFFSET),
        Some(-1)
    );
}

#[test]
fn pe_industry_leftover_grows_pops_toward_industrial_zone() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "indg").expect("blank");
    city.set_misc_u32_at(MISC_CITY_EDUCATION_QUOTIENT_OFFSET, 100);
    seed_pe_industry(&mut city, 0, 100, 0, 0);
    for index in 1..PE_INDUSTRY_COUNT {
        seed_pe_industry(&mut city, index, 0, 50, 0);
    }
    let mut lfsr = Win95Lfsr::new(1);
    city.advance_pe_industry_mix_month(10, 0, 0, &mut Win95CrtRand::new(1), &mut lfsr);
    assert_eq!(city.pe_industry_demand_at(0), 75);
    assert_eq!(city.pe_industry_pop_at(0), 10);
    assert_eq!(city.pe_industry_tax_at(0), 0);
    for index in 1..PE_INDUSTRY_COUNT {
        assert_eq!(city.pe_industry_pop_at(index), 0);
    }
}

#[test]
fn pe_industry_shrinks_pops_when_mix_exceeds_zone() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "inds").expect("blank");
    city.set_misc_u32_at(MISC_CITY_EDUCATION_QUOTIENT_OFFSET, 100);
    seed_pe_industry(&mut city, 0, 0, 50, 20);
    for index in 1..PE_INDUSTRY_COUNT {
        seed_pe_industry(&mut city, index, 0, 50, 0);
    }
    let mut lfsr = Win95Lfsr::new(1);
    city.advance_pe_industry_mix_month(10, 0, 0, &mut Win95CrtRand::new(1), &mut lfsr);
    assert_eq!(city.pe_industry_pop_at(0), 10);
}

#[test]
fn pe_industry_population_shrink_uses_wrapped_dword_product() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "indw").expect("blank");
    city.set_misc_u32_at(MISC_CITY_EDUCATION_QUOTIENT_OFFSET, 100);
    for index in 0..PE_INDUSTRY_COUNT {
        seed_pe_industry(&mut city, index, 0, 50, u32::from(index < 2) * 50_000);
    }

    // PE computes ((100_000 - 90_000) * 100 * 50_000) as a low dword,
    // divides that unsigned value by 100_000, then divides the quotient
    // by 100. The native row reduction is 275 plus its 53% remainder roll.
    let mut crt = Win95CrtRand::new(1);
    let mut expected_crt = crt;
    let expected = (0..2)
        .map(|_| {
            let decrement = 275 + i32::from(expected_crt.random_u15() % 100 < 53);
            50_000 - decrement as u32
        })
        .collect::<Vec<_>>();
    for _ in 2..PE_INDUSTRY_COUNT {
        expected_crt.random_u15();
    }
    let mut lfsr = Win95Lfsr::new(1);
    city.advance_pe_industry_mix_month(90_000, 0, 0, &mut crt, &mut lfsr);

    assert_eq!(city.pe_industry_pop_at(0), expected[0]);
    assert_eq!(city.pe_industry_pop_at(1), expected[1]);
    assert_eq!(crt.seed(), expected_crt.seed());
}

#[test]
fn pe_industry_population_growth_uses_wrapped_dword_product() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "indwg").expect("blank");
    for index in 0..PE_INDUSTRY_COUNT {
        seed_pe_industry(&mut city, index, 0, 50, 0);
    }
    let mut crt = Win95CrtRand::new(1);
    let mut expected_crt = crt;
    let expected = (0..2)
        .map(|_| {
            let increment = 21_366 + i32::from(expected_crt.random_u15() % 100 < 88);
            increment as u32
        })
        .collect::<Vec<_>>();
    city.pe_industry_rebalance_pops(
        100_000,
        0,
        [750, 750, 0, 0, 0, 0, 0, 0, 0, 0, 0],
        1_500,
        &mut crt,
    );

    assert_eq!(city.pe_industry_pop_at(0), expected[0]);
    assert_eq!(city.pe_industry_pop_at(1), expected[1]);
    assert_eq!(crt.seed(), expected_crt.seed());
}

#[test]
fn pe_industry_education_modifier_rescales_prior_rows_after_sum() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "indx").expect("blank");
    city.set_misc_u32_at(MISC_CITY_EDUCATION_QUOTIENT_OFFSET, 0);
    for index in 0..PE_INDUSTRY_COUNT {
        seed_pe_industry(&mut city, index, 0, 100, 0);
    }
    seed_pe_industry(&mut city, 6, 1_000, 0, 0);

    let mut crt = Win95CrtRand::new(1);
    let mut expected_crt = crt;
    let grew_extra = expected_crt.random_u15() % 100 < 33;
    let mut lfsr = Win95Lfsr::new(1);
    city.advance_pe_industry_mix_month(100_000, 0, 0, &mut crt, &mut lfsr);

    // Education below 60 scales row 6 by 0.8 five times: once when row 6
    // is produced and again for rows 7..=10. Its already-summed value
    // remains 600, while its final share is 245. PE distributes
    // 10,000,000 * 245 / 600, then applies the 33% rounding remainder.
    assert_eq!(city.pe_industry_demand_at(6), 750);
    assert_eq!(city.pe_industry_pop_at(6), 40_833 + u32::from(grew_extra));
    assert_eq!(crt.seed(), expected_crt.seed());
    for index in (0..PE_INDUSTRY_COUNT).filter(|&index| index != 6) {
        assert_eq!(city.pe_industry_pop_at(index), 0, "industry {index}");
    }
}

#[test]
fn pe_industry_mix_bonuses_use_wrapped_dword_arithmetic() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "indb").expect("blank");
    for index in 0..PE_INDUSTRY_COUNT {
        seed_pe_industry(&mut city, index, 0, 50, 0);
    }
    for index in [0, 1, 2, 5] {
        city.set_misc_u32_at(
            pe_industry_field_offset(index, PE_INDUSTRY_POP_FIELD),
            20_000_000,
        );
    }

    city.pe_industry_write_mix_bonuses(50_000_000);

    assert_eq!(city.misc_u32_at(MISC_INDUSTRIAL_MIX_BONUS_OFFSET), Some(3));
    assert_eq!(
        city.misc_i32_at(MISC_INDUSTRIAL_MIX_POLLUTION_BONUS_OFFSET),
        Some(1)
    );
}

#[test]
fn pe_industry_mix_bonus_thresholds_use_signed_comparisons() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "indsg").expect("blank");
    for index in 0..PE_INDUSTRY_COUNT {
        seed_pe_industry(&mut city, index, 0, 50, 0);
    }
    city.set_misc_u32_at(
        pe_industry_field_offset(0, PE_INDUSTRY_POP_FIELD),
        30_000_000,
    );

    // 30,000,000 * 100 wraps to 3,000,000,000. The PE's JGE threshold
    // treats that quotient as signed, so both bonus arms fall below 20.
    city.pe_industry_write_mix_bonuses(0);

    assert_eq!(city.misc_u32_at(MISC_INDUSTRIAL_MIX_BONUS_OFFSET), Some(0));
    assert_eq!(
        city.misc_i32_at(MISC_INDUSTRIAL_MIX_POLLUTION_BONUS_OFFSET),
        Some(-1)
    );
}

#[test]
fn pe_industry_construction_boost_when_rci_exceeds_city_population() {
    // PE leftover[4] *= 1.1 when rci_zone_people > city_population
    // (`dwCityPopulation` / `0x4CAA74`).
    fn construction_pop(city_population: u32) -> u32 {
        let mut city = City::new_blank("/tmp/sc2000-test.sc2", "indc").expect("blank");
        city.set_misc_u32_at(MISC_CITY_EDUCATION_QUOTIENT_OFFSET, 100);
        for index in 0..PE_INDUSTRY_COUNT {
            seed_pe_industry(&mut city, index, 0, 50, 0);
        }
        seed_pe_industry(&mut city, 4, 100, 0, 0);
        let mut lfsr = Win95Lfsr::new(1);
        city.advance_pe_industry_mix_month(
            100,
            200,
            city_population,
            &mut Win95CrtRand::new(1),
            &mut lfsr,
        );
        city.pe_industry_pop_at(4)
    }
    assert!(
        construction_pop(100) > construction_pop(300),
        "construction overflow boost should grow slot-4 pop when rci people exceed city pop"
    );
}

#[test]
fn eq_le_tables_round_trip_through_misc_0x7c() {
    let path = std::env::temp_dir().join(format!("sc2000-eq-le-{}.sc2", std::process::id()));
    let mut city = City::new_blank(&path, "eqle").expect("blank city");
    city.test_seed_pe_age_cohorts_from_pop(10_000);
    let school = city.pe_age_cohort_at(1) + city.pe_age_cohort_at(2);
    let college = city.pe_age_cohort_at(3);
    let life1 = city.pe_eq_life_points_at(1);
    assert_eq!(school, 1_200);
    city.save(&path).expect("save");

    let loaded = City::load(&path).expect("reload");
    assert_eq!(
        loaded.pe_age_cohort_at(1) + loaded.pe_age_cohort_at(2),
        school,
        "MISC 0x7c cohorts must survive FORM SCDH"
    );
    assert_eq!(loaded.pe_age_cohort_at(3), college);
    assert_eq!(loaded.pe_eq_life_points_at(1), life1);
    let _ = std::fs::remove_file(path);
}

#[test]
fn pe_eq_le_age_forward_matches_pe_draw_order_and_dword_arithmetic() {
    let mut city =
        City::new_blank("/tmp/sc2000-eq-le-age-forward.sc2", "eqle").expect("blank city");
    city.pe_age_cohorts.fill(0);
    city.pe_eq_aux_points.fill(0);
    city.pe_eq_life_points.fill(0);
    city.pe_age_cohorts[4] = 60;
    city.pe_eq_aux_points[4] = u32::MAX;
    city.pe_eq_aux_points[5] = u32::MAX;
    city.pe_eq_life_points[4] = 1;
    city.pe_eq_life_points[5] = u32::MAX;
    let mut bare = city.clone();

    let params = PeEqLeMonthParams {
        life_expectancy: 0,
        hospital_birth_param: 0,
        school_aux_param: 0,
        college_aux_param: 0,
        education_demand: 1,
        city_population: 0,
        growth_slot7: 0,
        pro_reading: true,
    };
    let bare_params = PeEqLeMonthParams {
        pro_reading: false,
        ..params
    };
    let mut actual_rng = Win95CrtRand::new(1);
    let mut bare_rng = Win95CrtRand::new(1);
    let mut expected_rng = Win95CrtRand::new(1);
    let _pe_rounding_draw = expected_rng.random_u16();

    city.pe_age_cohorts_age_forward_one_month(&params, &mut actual_rng);
    bare.pe_age_cohorts_age_forward_one_month(&bare_params, &mut bare_rng);

    assert_eq!(city.pe_age_cohorts[4], 59);
    assert_eq!(city.pe_age_cohorts[5], 1);
    let aux_moved = u32::MAX / 60;
    assert_eq!(city.pe_eq_aux_points[4], u32::MAX - aux_moved);
    assert_eq!(city.pe_eq_aux_points[5], u32::MAX.wrapping_add(aux_moved));
    assert_eq!(
        city.pe_eq_aux_points[5].wrapping_sub(bare.pe_eq_aux_points[5]),
        1,
        "Pro Reading skips exactly the destination aux -= moved instruction"
    );
    assert_eq!(city.pe_eq_life_points[4], 1);
    assert_eq!(
        city.pe_eq_life_points[5],
        u32::MAX.wrapping_add(u32::MAX),
        "PE subtracts the one-point penalty from a zero-point transfer with dword wrap"
    );
    let next_expected = expected_rng.random_u16();
    assert_eq!(
        actual_rng.random_u16(),
        next_expected,
        "PE consumes one CRT rand draw for the nonempty exact-multiple source bucket"
    );
    assert_eq!(bare_rng.random_u16(), next_expected);
}

#[test]
fn pe_eq_mortality_matches_wrapped_pe_widths_and_signed_score() {
    let mut city = City::new_blank("/tmp/sc2000-eq-le-mortality.sc2", "eqm").expect("blank city");
    city.pe_age_cohorts.fill(0);
    city.pe_eq_aux_points.fill(0);
    city.pe_eq_life_points.fill(0);
    city.pe_age_cohorts[1] = 96;
    city.pe_eq_aux_points[1] = 0x4000_0000;

    let mut actual_rng = Win95CrtRand::new(1);
    let mut expected_rng = actual_rng;
    expected_rng.random_u16();
    city.pe_age_cohorts_deaths_one_month(&mut actual_rng);

    assert_eq!(city.pe_eq_deaths, 4, "96 * 100 / 24 / 100");
    assert_eq!(city.pe_age_cohorts[1], 92);
    assert_eq!(
        city.pe_eq_aux_points[1], 0x4000_0000,
        "the PE low-dword multiply wraps 0x40000000 * 4 to zero"
    );
    assert_eq!(actual_rng.random_u16(), expected_rng.random_u16());

    assert_eq!(
        pe_eq_survival_score(30_000_000, 1, 5),
        -258_993_459,
        "PE wraps 3,000,000,000 to a signed dword before IDIV"
    );
    let mut overflow_city =
        City::new_blank("/tmp/sc2000-eq-le-score-wrap.sc2", "eqw").expect("blank city");
    overflow_city.pe_age_cohorts.fill(0);
    overflow_city.pe_eq_aux_points.fill(0);
    overflow_city.pe_eq_life_points.fill(0);
    overflow_city.pe_age_cohorts[1] = 1;
    overflow_city.pe_eq_aux_points[1] = 1;
    overflow_city.pe_eq_life_points[1] = 30_000_000;
    overflow_city.pe_age_cohorts_deaths_one_month(&mut Win95CrtRand::new(1));
    assert_eq!(
        overflow_city.pe_eq_deaths, 1,
        "the negative signed survival score enters the mortality arm"
    );
    assert_eq!(overflow_city.pe_age_cohorts[1], 0);
}

#[test]
fn pe_eq_deficit_immigration_matches_bucket_order_and_wrapping_dwords() {
    let mut city =
        City::new_blank("/tmp/sc2000-eq-le-immigrate-wrap.sc2", "eqi").expect("blank city");
    city.pe_age_cohorts.fill(0);
    city.pe_eq_aux_points.fill(0);
    city.pe_eq_life_points.fill(0);
    // PE chunk is (33 >> 4) + 1 = 3. Four working-age buckets receive
    // three people first, then buckets 0..=6 receive the remaining 21.
    city.pe_age_cohorts[4] = u32::MAX - 2;
    city.pe_eq_life_points[4] = u32::MAX - 100;
    city.pe_eq_aux_points[4] = u32::MAX - 1_000;

    city.pe_age_cohorts_immigrate_deficit(33);

    assert_eq!(city.pe_age_cohorts[4], 3, "bucket 4 receives two chunks");
    assert_eq!(
        city.pe_eq_life_points[4], 265,
        "life additions wrap as dwords"
    );
    assert_eq!(
        city.pe_eq_aux_points[4],
        u32::MAX - 484,
        "aux additions wrap as dwords"
    );
    assert_eq!(city.pe_age_cohorts[0], 3);
    assert_eq!(city.pe_eq_life_points[0], 195);
    assert_eq!(city.pe_eq_aux_points[0], 51);
    assert_eq!(city.pe_age_cohorts[6], 6, "bucket 6 appears in both ranges");
    assert_eq!(city.pe_eq_life_points[6], 354);
    assert_eq!(city.pe_eq_aux_points[6], 504);
    assert_eq!(
        city.pe_age_cohorts[7], 3,
        "first range is visited before age 0"
    );
    assert_eq!(
        city.pe_age_cohorts[8], 0,
        "the 33-person deficit is exhausted at age 6"
    );
}

#[test]
fn pe_eq_month_death_birth_and_leftover_counters_wrap_like_pe_dwords() {
    let params = PeEqLeMonthParams {
        life_expectancy: 85,
        hospital_birth_param: 0,
        school_aux_param: 0,
        college_aux_param: 0,
        education_demand: 0,
        city_population: 100,
        growth_slot7: 0,
        pro_reading: false,
    };

    let mut death_counter_wrap =
        City::new_blank("/tmp/sc2000-eq-le-death-counter-wrap.sc2", "eqdc").expect("blank city");
    death_counter_wrap.pe_age_cohorts.fill(0);
    death_counter_wrap.pe_eq_aux_points.fill(0);
    death_counter_wrap.pe_eq_life_points.fill(0);
    death_counter_wrap.pe_age_cohorts[1] = 24;
    death_counter_wrap.pe_eq_deaths = u32::MAX;
    death_counter_wrap.pe_eq_le_run_one_month(&params, &mut Win95CrtRand::new(1));
    assert_eq!(death_counter_wrap.pe_eq_deaths, 0);
    assert_eq!(death_counter_wrap.pe_age_cohorts[1], 23);
    assert_eq!(
        death_counter_wrap.pe_age_cohorts[4], 1,
        "wrapping the death counter still recovers the one-person monthly delta"
    );

    let mut leftover_counter_wrap =
        City::new_blank("/tmp/sc2000-eq-le-leftover-counter-wrap.sc2", "eqlw").expect("blank city");
    leftover_counter_wrap.pe_age_cohorts.fill(0);
    leftover_counter_wrap.pe_eq_aux_points.fill(0);
    leftover_counter_wrap.pe_eq_life_points.fill(0);
    leftover_counter_wrap.pe_age_cohorts[1] = 24;
    leftover_counter_wrap.pe_eq_leftover_deaths = u32::MAX;
    leftover_counter_wrap.pe_eq_le_run_one_month(&params, &mut Win95CrtRand::new(1));
    assert_eq!(leftover_counter_wrap.pe_eq_deaths, 1);
    assert_eq!(
        leftover_counter_wrap.pe_age_cohorts[4], 0,
        "prior day-22 0xffffffff plus one new death wraps to zero"
    );
    assert_eq!(leftover_counter_wrap.pe_eq_leftover_deaths, 0);

    let mut birth_counter_wrap =
        City::new_blank("/tmp/sc2000-eq-le-birth-counter-wrap.sc2", "eqbc").expect("blank city");
    birth_counter_wrap.pe_age_cohorts.fill(0);
    birth_counter_wrap.pe_age_cohorts[4] = 300;
    birth_counter_wrap.pe_eq_births = u32::MAX;
    assert_eq!(
        birth_counter_wrap.pe_age_cohorts_births_one_month(&params, &mut Win95CrtRand::new(1)),
        1
    );
    assert_eq!(birth_counter_wrap.pe_eq_births, 0);
}

#[test]
fn pe_eq_surplus_emigration_uses_dw_population_and_repeats_native_passes() {
    let mut city = City::new_blank("/tmp/sc2000-eq-le-emigrate.sc2", "eqe").expect("blank city");
    city.pe_age_cohorts.fill(0);
    city.pe_eq_aux_points.fill(0);
    city.pe_eq_life_points.fill(0);
    city.pe_age_cohorts[0] = 300;
    city.pe_age_cohorts[1] = 100;

    let mut actual_rng = Win95CrtRand::new(1);
    city.pe_age_cohorts_emigrate_surplus(50, 200, &mut actual_rng);
    assert_eq!(city.pe_age_cohorts[0], 250);
    assert_eq!(
        city.pe_age_cohorts[1], 100,
        "the PE denominator is leftover + dwCityPopulation, so bucket 0 consumes the clamp"
    );
    assert_eq!(
        actual_rng.seed(),
        1,
        "positive `_ftol` take consumes no draw"
    );

    let mut retry_city =
        City::new_blank("/tmp/sc2000-eq-le-emigrate-retry.sc2", "eqr").expect("blank city");
    retry_city.pe_age_cohorts.fill(0);
    retry_city.pe_eq_aux_points.fill(0);
    retry_city.pe_eq_life_points.fill(0);
    retry_city.pe_age_cohorts[4] = 1;
    let mut retry_rng = Win95CrtRand::new(1);
    let mut expected_rng = retry_rng;
    for _ in 0..4 {
        expected_rng.random_u16();
    }
    retry_city.pe_age_cohorts_emigrate_surplus(1, 100, &mut retry_rng);
    assert_eq!(retry_city.pe_age_cohorts[4], 0);
    assert_eq!(
        retry_rng.seed(),
        expected_rng.seed(),
        "native repeats all 20 buckets until a zero-take rand&3 gate succeeds"
    );
}

#[test]
fn decodes_xtrf_from_windows_city() {
    let Some(win) = load_win() else {
        return;
    };
    assert!(win.has_traffic_density_map());
    assert!(win.traffic_density_at(0, 0).is_some());
}

#[test]
fn decodes_xtrf_from_mac_expanded_city() {
    let Some(mac) = load_mac() else {
        return;
    };
    assert!(mac.has_traffic_density_map());
    assert!(mac.chunk("XTRF").is_some());
    let congested = (0..CITY_WIDTH)
        .flat_map(|x| (0..CITY_HEIGHT).map(move |y| (x, y)))
        .filter(|&(x, y)| mac.traffic_density_at(x, y).is_some_and(|v| v >= 86))
        .count();
    assert!(
        congested > 0,
        "expected some congested tiles in Maxis Land XTRF"
    );
}

#[test]
fn mac_budget_rows_use_serialized_slot_479_without_aliasing_ordinance_state() {
    let Some(mut city) = load_mac() else {
        return;
    };
    let serialized = decode_misc_bytes(&city.chunks).expect("Mac MISC chunk");
    let start = MAC_MISC_BUDGET_BOOK_OFFSET;
    let len = MISC_BUDGET_ROW_DWORDS * MISC_BUDGET_ROW_COUNT * MISC_FIELD_LEN;
    let raw_book = serialized
        .get(start..start + len)
        .expect("full Mac budget row book");
    let decoded_book =
        city.budget_misc_u32_values(start, MISC_BUDGET_ROW_DWORDS * MISC_BUDGET_ROW_COUNT);
    assert_eq!(
        decoded_book,
        raw_book
            .chunks_exact(MISC_FIELD_LEN)
            .map(|bytes| u32::from_be_bytes(bytes.try_into().expect("dword")))
            .collect::<Vec<_>>(),
        "cursor-table slot 479 maps to serialized MISC byte 0x77c"
    );

    let ordinance_mask = city.mac_state_misc_u32_at(0x0fa0);
    let first_zone7_count = city.mac_state_misc_u32_at(0x0fa8);
    let energy_zones = city.mac_state_misc_u32_at(0x102c);
    let police_funding =
        MAC_MISC_BUDGET_BOOK_OFFSET + 5 * MISC_BUDGET_ROW_DWORDS * MISC_FIELD_LEN + MISC_FIELD_LEN;
    let before = city
        .budget_misc_u32_at(police_funding)
        .expect("serialized police funding");
    city.set_budget_misc_u32_values(police_funding, &[before.wrapping_add(1)]);
    assert_eq!(city.mac_state_misc_u32_at(0x0fa0), ordinance_mask);
    assert_eq!(city.mac_state_misc_u32_at(0x0fa8), first_zone7_count);
    assert_eq!(city.mac_state_misc_u32_at(0x102c), energy_zones);

    let path = std::env::temp_dir().join(format!(
        "sc2000-mac-fixture-budget-{}.city",
        std::process::id()
    ));
    city.save(&path).expect("save fixture copy");
    let loaded = City::load(&path).expect("reload fixture copy");
    assert_eq!(
        loaded.budget_misc_u32_at(police_funding),
        Some(before.wrapping_add(1))
    );
    assert_eq!(loaded.mac_state_misc_u32_at(0x0fa0), ordinance_mask);
    assert_eq!(loaded.mac_state_misc_u32_at(0x0fa8), first_zone7_count);
    assert_eq!(loaded.mac_state_misc_u32_at(0x102c), energy_zones);
    let _ = fs::remove_file(path);
}

#[test]
fn mac_power_summary_slot_1048_round_trips_through_city_save() {
    const MAC_POWER_SUMMARY_MISC_OFFSET: usize = 1048 * MISC_FIELD_LEN;
    let path = std::env::temp_dir().join(format!(
        "sc2000-mac-power-summary-{}.mac",
        std::process::id()
    ));
    let mut city = City::new_blank(&path, "MacPowerSave").expect("blank city");
    city.kind = CityFileKind::MacCity;
    let summary = 0x5a12_34ab;
    city.set_mac_state_misc_u32_at(MAC_POWER_SUMMARY_MISC_OFFSET, summary);

    city.save(&path).expect("save Mac city copy");
    let loaded = City::load(&path).expect("reload Mac city copy");
    assert_eq!(
        loaded.mac_state_misc_u32_at(MAC_POWER_SUMMARY_MISC_OFFSET),
        Some(summary),
        "fffff00a is serialized at MISC dword 1048"
    );
    let _ = fs::remove_file(path);
}

#[test]
fn mac_treatment_gate_does_not_read_win_misc_before_first_update() {
    let path = std::env::temp_dir().join(format!("mac-treatment-win-{}.sc2", std::process::id()));
    let mut city = City::new_blank(&path, "Mixed").expect("blank city");
    city.set_misc_u32_at(1043 * MISC_FIELD_LEN, 0x1234_0001);
    assert_eq!(city.mac_treatment_gate(), 0);
    city.set_mac_treatment_gate(1);
    assert_eq!(city.mac_treatment_gate(), 1);
    assert_eq!(city.misc_u32_at(1043 * MISC_FIELD_LEN), Some(0x1234_0001));
    city.save(&path).expect("save Win city");
    let loaded = City::load(&path).expect("reload Win city");
    assert_eq!(loaded.mac_treatment_gate(), 0);
    assert_eq!(loaded.misc_u32_at(1043 * MISC_FIELD_LEN), Some(0x1234_0001));
    std::fs::remove_file(path).unwrap();
}

#[test]
fn mac_pollution_adjustment_uses_shared_win_industry_bonus() {
    let mut city = City::new_blank("/tmp/mac-pollution-win.sc2", "Mixed").unwrap();
    city.set_misc_u32_at(1037 * MISC_FIELD_LEN, 0x1234_0007);
    assert_eq!(city.mac_pollution_adjustment(), 7);
    city.set_mac_pollution_adjustment_for_test(-2);
    assert_eq!(city.mac_pollution_adjustment(), -2);
    assert_eq!(
        city.misc_u32_at(1037 * MISC_FIELD_LEN),
        Some((-2_i32) as u32)
    );
}

#[test]
fn industry_bonus_words_cross_layout_and_survive_save_reload() {
    for (extension, kind) in [
        ("city", CityFileKind::MacCity),
        ("sc2", CityFileKind::WinSc2),
    ] {
        let path = std::env::temp_dir().join(format!(
            "citygame-shared-industry-{}.{extension}",
            std::process::id()
        ));
        let mut city = City::new_blank(&path, "Shared Industry").unwrap();
        city.kind = kind;
        city.set_mac_state_misc_u32_at(MISC_INDUSTRIAL_MIX_BONUS_OFFSET, 0x1234_5678);
        city.set_mac_state_misc_u32_at(MISC_INDUSTRIAL_MIX_POLLUTION_BONUS_OFFSET, 0xabcd_fffe);
        assert_eq!(city.mac_pollution_adjustment(), -2);
        if kind == CityFileKind::MacCity {
            city.prepare_pe_fidelity_from_saved_city();
        }
        city.pe_industry_write_mix_bonuses(0);
        assert_eq!(city.pe_w_industrial_mix_bonus(), 0);
        assert_eq!(city.pe_w_industrial_mix_pollution_bonus(), -1);
        assert_eq!(
            city.mac_pollution_adjustment(),
            if kind == CityFileKind::MacCity {
                -2
            } else {
                -1
            }
        );
        assert_eq!(
            city.misc_u32_at(MISC_INDUSTRIAL_MIX_POLLUTION_BONUS_OFFSET),
            Some(if kind == CityFileKind::MacCity {
                0xabcd_fffe
            } else {
                u32::MAX
            })
        );
        city.save(&path).unwrap();
        let mut loaded = City::load(&path).unwrap();
        assert_eq!(loaded.kind, kind);
        if kind == CityFileKind::MacCity {
            loaded.prepare_pe_fidelity_from_saved_city();
        }
        assert_eq!(loaded.pe_w_industrial_mix_bonus(), 0);
        assert_eq!(loaded.pe_w_industrial_mix_pollution_bonus(), -1);
        assert_eq!(
            loaded.mac_pollution_adjustment(),
            if kind == CityFileKind::MacCity {
                -2
            } else {
                -1
            }
        );
        assert_eq!(
            loaded.misc_u32_at(MISC_INDUSTRIAL_MIX_POLLUTION_BONUS_OFFSET),
            Some(if kind == CityFileKind::MacCity {
                0xabcd_fffe
            } else {
                u32::MAX
            })
        );
        if kind == CityFileKind::MacCity {
            loaded.set_misc_u32_at(MISC_INDUSTRIAL_MIX_BONUS_OFFSET, 0xffff_8001);
            assert_eq!(loaded.pe_w_industrial_mix_bonus(), 0);
            loaded.save(&path).unwrap();
            let mut reopened = City::load(&path).unwrap();
            reopened.prepare_pe_fidelity_from_saved_city();
            assert_eq!(reopened.pe_w_industrial_mix_bonus(), 0);
        }
        std::fs::remove_file(path).unwrap();
    }
}

#[test]
fn pe_traffic_count_uses_extension_when_mac_owns_misc_traffic_total() {
    let path = std::env::temp_dir().join(format!(
        "sc2000-pe-traffic-cross-layout-{}.city",
        std::process::id()
    ));
    let mac_traffic_total = 0x1234_5678;
    let mut city = City::new_blank(&path, "Traffic Cross Layout").expect("blank city");
    city.kind = CityFileKind::MacCity;
    city.set_mac_state_misc_u32_at(MISC_CITY_TRAFFIC_OFFSET, mac_traffic_total);
    city.prepare_pe_fidelity_from_saved_city();
    assert_eq!(city.pe_traffic_count(), mac_traffic_total);

    // PE's later runtime traffic value must not replace Mac's XTRF total.
    city.set_pe_traffic_count(9_001);
    city.save(&path).expect("save PE state on Mac container");
    assert_eq!(
        city.mac_state_misc_u32_at(MISC_CITY_TRAFFIC_OFFSET),
        Some(mac_traffic_total)
    );
    let extension = decode_citygame_extension(&city.chunks).expect("decode CGEX v16");
    assert_eq!(extension.version, 16);
    assert_eq!(extension.pe_traffic_count, Some(9_001));

    let mut reloaded = City::load(&path).expect("reload PE state");
    reloaded.prepare_pe_fidelity_from_saved_city();
    assert_eq!(reloaded.pe_traffic_count(), 9_001);
    assert_eq!(
        reloaded.mac_state_misc_u32_at(MISC_CITY_TRAFFIC_OFFSET),
        Some(mac_traffic_total)
    );

    reloaded.set_pe_traffic_count(0);
    reloaded.save(&path).expect("save cleared PE traffic state");
    let mut reopened = City::load(&path).expect("reload cleared PE traffic state");
    reopened.prepare_pe_fidelity_from_saved_city();
    assert_eq!(reopened.pe_traffic_count(), 0);
    assert_eq!(
        reopened.mac_state_misc_u32_at(MISC_CITY_TRAFFIC_OFFSET),
        Some(mac_traffic_total)
    );
    let _ = std::fs::remove_file(path);
}

#[test]
fn pe_monster_xthg_index_uses_extension_when_mac_owns_misc_slot() {
    let path = std::env::temp_dir().join(format!(
        "sc2000-pe-monster-index-cross-layout-{}.city",
        std::process::id()
    ));
    let mac_slot_value = 0x0000_5678;
    let mut native_mac_city =
        City::new_blank(&path, "Monster Index Cross Layout").expect("blank city");
    native_mac_city.kind = CityFileKind::MacCity;
    native_mac_city.set_mac_state_misc_u32_at(MISC_MONSTER_XTHG_INDEX_OFFSET, mac_slot_value);
    native_mac_city.save(&path).expect("save native Mac slot");
    let mut city = City::load(&path).expect("open Mac file under PE fidelity");
    assert_eq!(city.kind, CityFileKind::MacCity);
    assert_eq!(
        city.misc_u32_at(MISC_MONSTER_XTHG_INDEX_OFFSET),
        Some(mac_slot_value)
    );

    // The first Win95-fidelity open uses the raw slot as the native PE loader
    // would. Subsequent PE updates must not overwrite Mac's state-vector word.
    city.prepare_pe_fidelity_from_saved_city();
    assert_eq!(city.pe_w_monster_xthg_index(), 0x5678);
    city.set_pe_w_monster_xthg_index(0xffff);
    city.save(&path).expect("save cleared PE monster index");
    assert_eq!(
        city.mac_state_misc_u32_at(MISC_MONSTER_XTHG_INDEX_OFFSET),
        Some(mac_slot_value)
    );
    let extension = decode_citygame_extension(&city.chunks).expect("decode CGEX v16");
    assert_eq!(extension.version, 16);
    assert_eq!(extension.pe_monster_xthg_index, Some(0xffff));

    let mut reloaded = City::load(&path).expect("reload PE monster index");
    reloaded.prepare_pe_fidelity_from_saved_city();
    assert_eq!(reloaded.pe_w_monster_xthg_index(), 0xffff);
    assert_eq!(
        reloaded.mac_state_misc_u32_at(MISC_MONSTER_XTHG_INDEX_OFFSET),
        Some(mac_slot_value)
    );

    // The Maxis Man constructor reads the global again for a later spawn, so
    // verify that a new active XTHG index also survives another continuation.
    reloaded.set_pe_w_monster_xthg_index(7);
    reloaded.save(&path).expect("save later PE monster index");
    let mut reopened = City::load(&path).expect("reload later PE monster index");
    reopened.prepare_pe_fidelity_from_saved_city();
    assert_eq!(reopened.pe_w_monster_xthg_index(), 7);
    assert_eq!(
        reopened.mac_state_misc_u32_at(MISC_MONSTER_XTHG_INDEX_OFFSET),
        Some(mac_slot_value)
    );
    let _ = std::fs::remove_file(path);
}

#[test]
fn mac_scenario_active_state_byte_round_trips_through_city_save() {
    let path = std::env::temp_dir().join(format!(
        "sc2000-mac-scenario-active-{}.mac",
        std::process::id()
    ));
    let mut city = City::new_blank(&path, "MacScenarioState").expect("blank city");
    city.kind = CityFileKind::MacCity;
    city.set_mac_state_misc_u32_at(MAC_SCENARIO_ACTIVE_STATE_OFFSET, 0x1234_5600);
    city.set_mac_scenario_active(true);
    assert!(city.mac_scenario_active());

    city.save(&path).expect("save Mac city copy");
    let mut loaded = City::load(&path).expect("reload Mac city copy");
    assert_eq!(
        loaded.mac_state_misc_u32_at(MAC_SCENARIO_ACTIVE_STATE_OFFSET),
        Some(0x1234_5601),
        "fffffdf0 is the low byte of saved state slot 1050"
    );
    assert!(loaded.mac_scenario_active());

    loaded.set_mac_scenario_active(false);
    assert!(!loaded.mac_scenario_active());
    assert_eq!(
        loaded.mac_state_misc_u32_at(MAC_SCENARIO_ACTIVE_STATE_OFFSET),
        Some(0x1234_5600),
        "the simulation write preserves unrelated bytes in the slot"
    );
    let _ = fs::remove_file(path);
}

#[test]
fn mac_simulation_state_slots_survive_pe_save_mirror_pass() {
    let path =
        std::env::temp_dir().join(format!("sc2000-mac-sim-state-{}.mac", std::process::id()));
    let mut city = City::new_blank(&path, "MacSimulationState").expect("blank city");
    city.kind = CityFileKind::MacCity;

    // Mac CODE 4 uses these serialized state slots for XCRM, XTRF, XPLT,
    // its monthly growth scalar, and its pending action. Slot 1020 is
    // opaque to us but must also survive PE mirror serialization.
    let mac_values = [
        (0x0004, 0x1010_1010),
        (0x0008, 0x1234_5603),
        (0x0010, 0x2020_2020),
        (0x001c, 0x3030_3030),
        (0x0020, 0x4040_4040),
        (0x002c, 0x1020_3040),
        (0x0030, 0x2030_4050),
        (0x0034, 0x3040_5060),
        (0x0038, 0x3840_5060),
        (0x003c, 0x3c40_5060),
        (0x0040, 0x4050_6070),
        (0x0044, 0x4450_6070),
        (0x0048, 0x4850_6070),
        (0x004c, 0x4c50_6070),
        (0x0070, 0x5060_7080),
        (0x0074, 0x5470_8090),
        (0x0078, 0x5870_8090),
        (0x0ff0, 0xa17b_2c4d),
    ];
    for (offset, value) in mac_values {
        city.set_mac_state_misc_u32_at(offset, value);
    }

    // Deliberately stale PE runtime values expose any accidental mirror.
    city.pe_dw_city_crime = 1;
    city.set_pe_traffic_count(2);
    city.pe_dw_city_pollution = 3;
    city.pe_w_city_garbage = 4;
    city.pe_dw_city_age = 1;
    city.pe_w_city_difficulty = 2;
    city.pe_w_city_mode = 3;
    city.pe_w_city_rotation = 2;
    city.pe_w_city_progression = 4;
    city.saved_rotation = 1;
    city.pe_eq_workforce_percent = 5;
    city.pe_eq_life_average = 6;
    city.pe_eq_education_average = 7;
    city.pe_w_trigger_disaster = 5;
    city.pe_dw_residential_population = 6;
    city.pe_dw_rewards_available = 7;

    city.save(&path).expect("save Mac city copy");
    let loaded = City::load(&path).expect("reload Mac city copy");
    for (offset, expected) in mac_values {
        assert_eq!(
            loaded.mac_state_misc_u32_at(offset),
            Some(expected),
            "Mac state at MISC {offset:#x} must not be overwritten by a PE mirror"
        );
    }
    assert_eq!(
        loaded.saved_rotation(),
        3,
        "Mac source-scan selector must hydrate from preserved MISC +0x08"
    );
    let _ = fs::remove_file(path);
}

#[test]
fn win95_pe_mirror_fields_still_save_to_misc() {
    let path =
        std::env::temp_dir().join(format!("sc2000-pe-misc-mirrors-{}.sc2", std::process::id()));
    let mut city = City::new_blank(&path, "PeMiscMirrors").expect("blank city");
    city.kind = CityFileKind::WinSc2;
    city.pe_dw_city_crime = 0x1122;
    city.set_pe_traffic_count(0x2233);
    city.pe_dw_city_pollution = 0x3344;
    city.pe_w_city_garbage = 0x4455;
    city.pe_eq_workforce_percent = 66;
    city.pe_eq_life_average = 77;
    city.pe_eq_education_average = 88;
    city.pe_w_trigger_disaster = 9;

    city.save(&path).expect("save Win95 city");
    let loaded = City::load(&path).expect("reload Win95 city");
    for (offset, expected) in [
        (MISC_CITY_CRIME_OFFSET, 0x1122),
        (MISC_CITY_TRAFFIC_OFFSET, 0x2233),
        (MISC_CITY_POLLUTION_OFFSET, 0x3344),
        (MISC_GARBAGE_OFFSET, 0x4455),
        (MISC_WORK_FORCE_PERCENT_OFFSET, 66),
        (MISC_CITY_LIFE_EXPECTANCY_OFFSET, 77),
        (MISC_CITY_EDUCATION_QUOTIENT_OFFSET, 88),
        (MISC_TRIGGER_DISASTER_OFFSET, 9),
    ] {
        assert_eq!(loaded.misc_u32_at(offset), Some(expected));
    }
    let _ = fs::remove_file(path);
}

#[test]
fn pe_eq_le_cache_hydration_respects_city_file_layout() {
    let mut packed = vec![0; PE_EQ_LE_PACKED_DWORDS];
    packed[..3].copy_from_slice(&[1_000, 2_000, 3_000]);

    let mac_path = std::env::temp_dir().join(format!(
        "sc2000-mac-pe-eq-le-cache-{}.city",
        std::process::id()
    ));
    let mut mac = City::new_blank(&mac_path, "MacEqLeOwnership").expect("blank Mac city");
    mac.kind = CityFileKind::MacCity;
    for (offset, value) in [
        (MISC_WORK_FORCE_PERCENT_OFFSET, 73),
        (MISC_CITY_LIFE_EXPECTANCY_OFFSET, 177),
        (MISC_CITY_EDUCATION_QUOTIENT_OFFSET, 244),
    ] {
        mac.set_mac_state_misc_u32_at(offset, value);
    }
    mac.set_budget_misc_u32_values(MISC_EQ_LE_TABLES_OFFSET, &packed);
    mac.save(&mac_path).expect("save Mac-layout city");

    let loaded_mac = City::load(&mac_path).expect("reload Mac-layout city");
    assert_eq!(loaded_mac.kind, CityFileKind::MacCity);
    assert!(!loaded_mac.pe_eq_averages_ready());
    assert_eq!(loaded_mac.pe_eq_workforce_percent(), 0);
    assert_eq!(loaded_mac.pe_eq_life_average(), 0);
    assert_eq!(loaded_mac.pe_eq_education_average(), 0);
    assert_eq!(loaded_mac.pe_age_cohort_at(0), 0);
    assert_eq!(loaded_mac.pe_eq_life_points[0], 0);
    assert_eq!(loaded_mac.pe_eq_aux_points[0], 0);
    assert_eq!(loaded_mac.pe_city_pollution, 0);
    assert_eq!(loaded_mac.pe_city_crime, 0);
    for (offset, expected) in [
        (MISC_WORK_FORCE_PERCENT_OFFSET, 73),
        (MISC_CITY_LIFE_EXPECTANCY_OFFSET, 177),
        (MISC_CITY_EDUCATION_QUOTIENT_OFFSET, 244),
        (MISC_EQ_LE_TABLES_OFFSET, 1_000),
    ] {
        assert_eq!(
            loaded_mac.mac_state_misc_u32_at(offset),
            Some(expected),
            "Mac-owned MISC state at {offset:#x} must be preserved"
        );
    }
    let _ = fs::remove_file(mac_path);

    let win_path = std::env::temp_dir().join(format!(
        "sc2000-win-pe-eq-le-cache-{}.sc2",
        std::process::id()
    ));
    let mut win = City::new_blank(&win_path, "WinEqLeHydration").expect("blank Win95 city");
    win.pe_eq_workforce_percent = 73;
    win.pe_eq_life_average = 177;
    win.pe_eq_education_average = 244;
    win.pe_eq_averages_ready = true;
    win.pe_age_cohorts[0] = packed[0];
    win.pe_eq_aux_points[0] = packed[1];
    win.pe_eq_life_points[0] = packed[2];
    win.save(&win_path).expect("save Win95-layout city");

    let loaded_win = City::load(&win_path).expect("reload Win95-layout city");
    assert_eq!(loaded_win.kind, CityFileKind::WinSc2);
    assert!(loaded_win.pe_eq_averages_ready());
    assert_eq!(loaded_win.pe_eq_workforce_percent(), 73);
    assert_eq!(loaded_win.pe_eq_life_average(), 177);
    assert_eq!(loaded_win.pe_eq_education_average(), 244);
    assert_eq!(loaded_win.pe_age_cohort_at(0), 1_000);
    assert_eq!(loaded_win.pe_eq_aux_points[0], 2_000);
    assert_eq!(loaded_win.pe_eq_life_points[0], 3_000);
    assert_eq!(loaded_win.pe_city_pollution, 177);
    assert_eq!(loaded_win.pe_city_crime, 244);
    let _ = fs::remove_file(win_path);
}

#[test]
fn pe_eq_le_calendar_month_preserves_mac_owned_misc_state() {
    let mut city =
        City::new_blank("/tmp/sc2000-pe-month-mac-layout.city", "MacPEMonth").expect("blank city");
    city.kind = CityFileKind::MacCity;
    let sentinels = [
        (MISC_ORDINANCE_FLAGS_OFFSET, 0xfa65_4321),
        (MISC_PRISON_BONUS_OFFSET, 0x7c11_2233),
        (MISC_NEIGHBOR_TABLE_OFFSET, 0x6d87_8899),
        (
            MISC_NEIGHBOR_TABLE_OFFSET + PE_NEIGHBOR_RECORD_BYTES,
            2_000_000,
        ),
        (
            MISC_NEIGHBOR_TABLE_OFFSET + PE_NEIGHBOR_RECORD_BYTES + 4,
            1_000_000,
        ),
    ];
    for (offset, value) in sentinels {
        city.set_mac_state_misc_u32_at(offset, value);
    }
    for index in 0..PE_INDUSTRY_COUNT {
        let offset = MISC_INDUSTRY_TABLE_OFFSET + index * PE_INDUSTRY_RECORD_BYTES;
        city.set_mac_state_misc_u32_at(offset, 100);
        city.set_mac_state_misc_u32_at(offset + 4, 10);
        city.set_mac_state_misc_u32_at(offset + 8, 20);
    }

    let mut crt = Win95CrtRand::new(17);
    let mut lfsr = Win95Lfsr::new(29);
    let mut expected_national = city.clone();
    let mut expected_crt = crt;
    expected_national.advance_pe_national_economy_month(&mut expected_crt);
    expected_national.advance_pe_neighbor_economy_month(&mut expected_crt);
    let mut expected_lfsr = lfsr;
    expected_national.advance_pe_industry_mix_month(
        1_000,
        2_000,
        10_000,
        &mut expected_crt,
        &mut expected_lfsr,
    );
    city.advance_pe_eq_le_calendar_month(10_000, 100, 100, 100, 1_000, 2_000, &mut crt, &mut lfsr);

    for (offset, expected) in sentinels {
        assert_eq!(
            city.mac_state_misc_u32_at(offset),
            if offset == MISC_NEIGHBOR_TABLE_OFFSET + PE_NEIGHBOR_RECORD_BYTES + 4 {
                expected_national.misc_u32_at(offset)
            } else {
                Some(expected)
            },
            "Mac-owned MISC state at {offset:#x} must survive the PE month wrapper"
        );
    }
    for index in 0..PE_INDUSTRY_COUNT {
        let offset = MISC_INDUSTRY_TABLE_OFFSET + index * PE_INDUSTRY_RECORD_BYTES;
        assert_eq!(
            city.mac_state_misc_u32_at(offset),
            expected_national.mac_state_misc_u32_at(offset)
        );
        assert_eq!(city.mac_state_misc_u32_at(offset + 4), Some(10));
        assert_eq!(
            city.mac_state_misc_u32_at(offset + 8),
            expected_national.mac_state_misc_u32_at(offset + 8)
        );
    }
    assert_eq!(
        crt.seed(),
        expected_crt.seed(),
        "the shared economy workers consume their native CRT draws"
    );
    assert_eq!(
        lfsr.state(),
        expected_lfsr.state(),
        "PE industry demand consumes its native LFSR draws"
    );
}

#[test]
fn pe_month_national_economy_runs_on_mac_layout_without_touching_other_state() {
    for seed in [1, 17, 1234, 0xdead_beef] {
        let path = std::env::temp_dir().join(format!(
            "citygame-pe-national-{}-{seed}.city",
            std::process::id()
        ));
        let mut city = City::new_blank(&path, "National").unwrap();
        city.kind = CityFileKind::MacCity;
        city.set_misc_u32_at(MISC_NATIONAL_POPULATION_OFFSET, 4_900_000);
        city.set_misc_u32_at(MISC_NATIONAL_VALUE_OFFSET, 3_400_000);
        city.set_misc_u32_at(MISC_NATIONAL_TAX_OFFSET, 3);
        city.set_misc_u32_at(MISC_NATIONAL_TREND_OFFSET, 1);
        city.set_mac_state_misc_u32_at(MISC_NEIGHBOR_TABLE_OFFSET, 0x1234_5678);
        city.set_mac_state_misc_u32_at(MISC_PRISON_BONUS_OFFSET, 0x8765_4321);
        city.save(&path).unwrap();
        let mut city = City::load(&path).unwrap();
        assert_eq!(city.kind, CityFileKind::MacCity);
        let mut expected = city.clone();
        let mut expected_rng = Win95CrtRand::new(seed);
        expected.advance_pe_national_economy_month(&mut expected_rng);
        expected.advance_pe_neighbor_economy_month(&mut expected_rng);
        let mut expected_lfsr = Win95Lfsr::new(29);
        expected.advance_pe_industry_mix_month(
            1000,
            2000,
            10_000,
            &mut expected_rng,
            &mut expected_lfsr,
        );
        let mut rng = Win95CrtRand::new(seed);
        let mut lfsr = Win95Lfsr::new(29);
        city.advance_pe_eq_le_calendar_month(
            10_000, 100, 100, 100, 1000, 2000, &mut rng, &mut lfsr,
        );
        for offset in [
            MISC_NATIONAL_POPULATION_OFFSET,
            MISC_NATIONAL_VALUE_OFFSET,
            MISC_NATIONAL_TAX_OFFSET,
            MISC_NATIONAL_TREND_OFFSET,
        ] {
            assert_eq!(
                city.misc_u32_at(offset),
                expected.misc_u32_at(offset),
                "seed {seed} offset {offset:#x}"
            );
        }
        assert_eq!(rng.seed(), expected_rng.seed());
        assert_eq!(lfsr.state(), expected_lfsr.state());
        assert_eq!(
            city.misc_u32_at(MISC_NEIGHBOR_TABLE_OFFSET),
            Some(0x1234_5678)
        );
        assert_eq!(
            city.misc_u32_at(MISC_PRISON_BONUS_OFFSET),
            Some(0x8765_4321)
        );
        city.save(&path).unwrap();
        let loaded = City::load(&path).unwrap();
        assert_eq!(loaded.kind, CityFileKind::MacCity);
        for offset in [
            MISC_NATIONAL_POPULATION_OFFSET,
            MISC_NATIONAL_VALUE_OFFSET,
            MISC_NATIONAL_TAX_OFFSET,
            MISC_NATIONAL_TREND_OFFSET,
        ] {
            assert_eq!(loaded.misc_u32_at(offset), expected.misc_u32_at(offset));
        }
        std::fs::remove_file(path).unwrap();
    }
}

#[test]
fn pe_neighbor_economy_on_mac_layout_matches_pe_state_and_saves() {
    for seed in [1_u32, 17, 385, 0xdead_beef] {
        let path = std::env::temp_dir().join(format!(
            "citygame-pe-neighbors-{}-{seed}.city",
            std::process::id()
        ));
        let mut city = City::new_blank(&path, "Neighbors").unwrap();
        city.kind = CityFileKind::MacCity;
        city.set_misc_u32_at(MISC_NATIONAL_POPULATION_OFFSET, 4_900_000);
        city.set_misc_u32_at(MISC_NATIONAL_VALUE_OFFSET, 3_400_000);
        city.set_misc_u32_at(MISC_NATIONAL_TAX_OFFSET, 3);
        city.set_misc_u32_at(MISC_NATIONAL_TREND_OFFSET, 1);
        for (row, (population, value)) in [(12_000, 3_600), (0, 0), (5_000_001, 3_500_001), (1, 1)]
            .into_iter()
            .enumerate()
        {
            seed_pe_neighbor(
                &mut city,
                row,
                row as u32 + 1,
                population,
                value,
                100 + row as u32,
            );
        }
        city.save(&path).unwrap();
        let mut city = City::load(&path).unwrap();
        assert_eq!(city.kind, CityFileKind::MacCity);
        let mut expected = city.clone();
        let mut expected_rng = Win95CrtRand::new(seed);
        expected.advance_pe_national_economy_month(&mut expected_rng);
        expected.advance_pe_neighbor_economy_month(&mut expected_rng);
        let mut expected_lfsr = Win95Lfsr::new(29);
        expected.advance_pe_industry_mix_month(
            1000,
            2000,
            1000,
            &mut expected_rng,
            &mut expected_lfsr,
        );
        let mut rng = Win95CrtRand::new(seed);
        let mut lfsr = Win95Lfsr::new(29);
        city.advance_pe_eq_le_calendar_month(1000, 100, 100, 100, 1000, 2000, &mut rng, &mut lfsr);
        assert_eq!(rng.seed(), expected_rng.seed());
        for row in 0..PE_NEIGHBOR_COUNT {
            for field in [
                PE_NEIGHBOR_NAME_FIELD,
                PE_NEIGHBOR_POP_FIELD,
                PE_NEIGHBOR_VALUE_FIELD,
                PE_NEIGHBOR_FAME_FIELD,
            ] {
                let offset = pe_neighbor_field_offset(row, field);
                assert_eq!(
                    city.misc_u32_at(offset),
                    expected.misc_u32_at(offset),
                    "seed {seed}, row {row}, field {field}"
                );
            }
        }
        city.save(&path).unwrap();
        let loaded = City::load(&path).unwrap();
        assert_eq!(loaded.kind, CityFileKind::MacCity);
        for row in 0..PE_NEIGHBOR_COUNT {
            for field in [
                PE_NEIGHBOR_NAME_FIELD,
                PE_NEIGHBOR_POP_FIELD,
                PE_NEIGHBOR_VALUE_FIELD,
                PE_NEIGHBOR_FAME_FIELD,
            ] {
                let offset = pe_neighbor_field_offset(row, field);
                assert_eq!(
                    loaded.misc_u32_at(offset),
                    expected.misc_u32_at(offset),
                    "reload seed {seed}, row {row}, field {field}"
                );
            }
        }
        std::fs::remove_file(path).unwrap();
    }
}

#[test]
fn pe_industry_mix_on_mac_layout_matches_full_pe_worker_and_saves() {
    for seed in [1_u16, 29, 0x1234, 0xffff] {
        let path = std::env::temp_dir().join(format!(
            "citygame-pe-industry-{}-{seed}.city",
            std::process::id()
        ));
        let mut city = City::new_blank(&path, "Industry").unwrap();
        city.kind = CityFileKind::MacCity;
        for index in 0..PE_INDUSTRY_COUNT {
            let offset = MISC_INDUSTRY_TABLE_OFFSET + index * PE_INDUSTRY_RECORD_BYTES;
            city.set_misc_u32_at(offset, (index as i32 * 1000 - 5000) as u32);
            city.set_misc_u32_at(offset + 4, 7 + index as u32);
            city.set_misc_u32_at(offset + 8, 100 + index as u32);
        }
        city.set_misc_u32_at(MISC_NEIGHBOR_TABLE_OFFSET, 0x1234_5678);
        city.set_mac_state_misc_u32_at(MISC_INDUSTRIAL_MIX_BONUS_OFFSET, 0x1234_5678);
        city.set_mac_state_misc_u32_at(MISC_INDUSTRIAL_MIX_POLLUTION_BONUS_OFFSET, 0x5678_1234);
        city.save(&path).unwrap();
        let mut city = City::load(&path).unwrap();
        assert_eq!(city.kind, CityFileKind::MacCity);
        assert_eq!(city.pe_w_industrial_mix_bonus(), 0);
        city.prepare_pe_fidelity_from_saved_city();
        assert_eq!(city.pe_w_industrial_mix_bonus(), 0x5678);
        assert_eq!(city.pe_w_industrial_mix_pollution_bonus(), 0x1234);
        let mut reference_crt = Win95CrtRand::new(17);
        let mut reference = city.clone();
        reference.advance_pe_national_economy_month(&mut reference_crt);
        reference.advance_pe_neighbor_economy_month(&mut reference_crt);
        reference.kind = CityFileKind::WinSc2;
        let mut reference_lfsr = Win95Lfsr::new(seed);
        reference.advance_pe_industry_mix_month(
            1000,
            2000,
            1000,
            &mut reference_crt,
            &mut reference_lfsr,
        );
        let mut crt = Win95CrtRand::new(17);
        let mut lfsr = Win95Lfsr::new(seed);
        city.advance_pe_eq_le_calendar_month(1000, 100, 100, 100, 1000, 2000, &mut crt, &mut lfsr);
        assert_eq!(crt.seed(), reference_crt.seed());
        assert_eq!(lfsr.state(), reference_lfsr.state());
        assert_eq!(
            city.pe_w_industrial_mix_bonus(),
            reference.pe_w_industrial_mix_bonus()
        );
        assert_eq!(
            city.pe_w_industrial_mix_pollution_bonus(),
            reference.pe_w_industrial_mix_pollution_bonus()
        );
        assert_eq!(
            city.misc_u32_at(MISC_INDUSTRIAL_MIX_BONUS_OFFSET),
            Some(0x1234_5678)
        );
        assert_eq!(
            city.misc_u32_at(MISC_INDUSTRIAL_MIX_POLLUTION_BONUS_OFFSET),
            Some(0x5678_1234)
        );
        for index in 0..PE_INDUSTRY_COUNT {
            let offset = MISC_INDUSTRY_TABLE_OFFSET + index * PE_INDUSTRY_RECORD_BYTES;
            assert_eq!(
                city.misc_u32_at(offset),
                reference.misc_u32_at(offset),
                "seed {seed} row {index}"
            );
            assert_eq!(city.misc_u32_at(offset + 4), Some(7 + index as u32));
            assert_eq!(
                city.misc_u32_at(offset + 8),
                reference.misc_u32_at(offset + 8)
            );
        }
        assert_eq!(
            city.misc_u32_at(MISC_NEIGHBOR_TABLE_OFFSET),
            Some(0x1234_5678)
        );
        city.save(&path).unwrap();
        let mut loaded = City::load(&path).unwrap();
        assert_eq!(loaded.kind, CityFileKind::MacCity);
        assert_eq!(
            loaded.misc_u32_at(MISC_INDUSTRIAL_MIX_BONUS_OFFSET),
            Some(0x1234_5678)
        );
        assert_eq!(
            loaded.misc_u32_at(MISC_INDUSTRIAL_MIX_POLLUTION_BONUS_OFFSET),
            Some(0x5678_1234)
        );
        loaded.prepare_pe_fidelity_from_saved_city();
        assert_eq!(
            loaded.pe_w_industrial_mix_bonus(),
            reference.pe_w_industrial_mix_bonus()
        );
        assert_eq!(
            loaded.pe_w_industrial_mix_pollution_bonus(),
            reference.pe_w_industrial_mix_pollution_bonus()
        );
        for index in 0..PE_INDUSTRY_COUNT {
            let offset = MISC_INDUSTRY_TABLE_OFFSET + index * PE_INDUSTRY_RECORD_BYTES;
            assert_eq!(loaded.misc_u32_at(offset), reference.misc_u32_at(offset));
            assert_eq!(
                loaded.misc_u32_at(offset + 8),
                reference.misc_u32_at(offset + 8)
            );
        }
        std::fs::remove_file(path).unwrap();
    }
}

#[test]
fn pe_demographics_restore_and_advance_mac_layout_without_losing_other_state() {
    let path =
        std::env::temp_dir().join(format!("citygame-pe-cohorts-{}.city", std::process::id()));
    let mut city = City::new_blank(&path, "Cohorts").unwrap();
    city.kind = CityFileKind::MacCity;
    let mut packed = [0_u32; PE_EQ_LE_PACKED_DWORDS];
    for index in 0..PE_EQ_LE_BUCKETS {
        let count = (index as u32 + 1) * 100;
        packed[index * 3] = count;
        packed[index * 3 + 1] = count * 41;
        packed[index * 3 + 2] = count * 31;
    }
    city.set_misc_u32_values(MISC_EQ_LE_TABLES_OFFSET, &packed);
    city.set_mac_state_misc_u32_at(MISC_WORK_FORCE_PERCENT_OFFSET, 45);
    city.set_mac_state_misc_u32_at(MISC_CITY_LIFE_EXPECTANCY_OFFSET, 70);
    city.set_mac_state_misc_u32_at(MISC_CITY_EDUCATION_QUOTIENT_OFFSET, 80);
    city.set_mac_state_misc_u32_at(MISC_NORMAL_POPULATION_OFFSET, 8_000);
    city.set_mac_state_misc_u32_at(MISC_INDUSTRY_TABLE_OFFSET, 0x1234_5678);
    city.set_mac_state_misc_u32_at(MISC_PRISON_BONUS_OFFSET, 0x8765_4321);
    city.save(&path).unwrap();
    let mut city = City::load(&path).unwrap();
    assert_eq!(city.kind, CityFileKind::MacCity);
    assert_eq!(
        city.pe_dw_city_population(),
        0,
        "Mac session has not selected PE state"
    );
    city.prepare_pe_fidelity_from_saved_city();
    assert_eq!(city.pe_dw_city_population(), 8_000);
    assert!(city.pe_eq_averages_ready());
    assert_eq!(
        (
            city.pe_eq_workforce_percent(),
            city.pe_eq_life_average(),
            city.pe_eq_education_average()
        ),
        (45, 70, 80)
    );
    for index in 0..PE_EQ_LE_BUCKETS {
        assert_eq!(
            (
                city.pe_age_cohorts[index],
                city.pe_eq_aux_points[index],
                city.pe_eq_life_points[index]
            ),
            (
                packed[index * 3],
                packed[index * 3 + 1],
                packed[index * 3 + 2]
            )
        );
    }
    let mut reference = city.clone();
    reference.kind = CityFileKind::WinSc2;
    let mut rng = Win95CrtRand::new(17);
    let mut reference_rng = rng;
    city.advance_pe_eq_le_month(8_000, 2, 3, 1, 100, 100, 100, &mut rng);
    reference.advance_pe_eq_le_month(8_000, 2, 3, 1, 100, 100, 100, &mut reference_rng);
    assert_eq!(rng.seed(), reference_rng.seed());
    assert_eq!(city.pe_age_cohorts, reference.pe_age_cohorts);
    assert_eq!(city.pe_eq_aux_points, reference.pe_eq_aux_points);
    assert_eq!(city.pe_eq_life_points, reference.pe_eq_life_points);
    assert_eq!(
        city.misc_u32_values(MISC_EQ_LE_TABLES_OFFSET, PE_EQ_LE_PACKED_DWORDS),
        reference.misc_u32_values(MISC_EQ_LE_TABLES_OFFSET, PE_EQ_LE_PACKED_DWORDS)
    );
    assert_eq!(
        city.misc_u32_at(MISC_INDUSTRY_TABLE_OFFSET),
        Some(0x1234_5678)
    );
    assert_eq!(
        city.misc_u32_at(MISC_PRISON_BONUS_OFFSET),
        Some(0x8765_4321)
    );
    city.save(&path).unwrap();
    let mut loaded = City::load(&path).unwrap();
    loaded.prepare_pe_fidelity_from_saved_city();
    assert_eq!(loaded.pe_age_cohorts, city.pe_age_cohorts);
    assert_eq!(loaded.pe_eq_aux_points, city.pe_eq_aux_points);
    assert_eq!(loaded.pe_eq_life_points, city.pe_eq_life_points);
    assert_eq!(loaded.pe_eq_life_average(), city.pe_eq_life_average());
    assert_eq!(
        loaded.pe_eq_education_average(),
        city.pe_eq_education_average()
    );
    assert_eq!(
        loaded.misc_u32_at(MISC_WORK_FORCE_PERCENT_OFFSET),
        Some(city.pe_eq_workforce_percent())
    );
    assert_eq!(
        loaded.misc_u32_at(MISC_CITY_LIFE_EXPECTANCY_OFFSET),
        Some(city.pe_eq_life_average())
    );
    assert_eq!(
        loaded.misc_u32_at(MISC_CITY_EDUCATION_QUOTIENT_OFFSET),
        Some(city.pe_eq_education_average())
    );
    assert_eq!(
        loaded.misc_u32_at(MISC_INDUSTRY_TABLE_OFFSET),
        Some(0x1234_5678)
    );
    assert_eq!(
        loaded.misc_u32_at(MISC_PRISON_BONUS_OFFSET),
        Some(0x8765_4321)
    );
    std::fs::remove_file(path).unwrap();
}

#[test]
fn pe_misc_cache_hydration_respects_city_file_layout() {
    let mac_path = std::env::temp_dir().join(format!(
        "sc2000-mac-pe-pop-cache-{}.city",
        std::process::id()
    ));
    let mut mac = City::new_blank(&mac_path, "MacPopOwnership").expect("blank Mac city");
    mac.kind = CityFileKind::MacCity;
    mac.set_mac_state_misc_u32_at(MISC_CITY_MODE_OFFSET, 1);
    mac.set_mac_state_misc_u32_at(MISC_YEAR_FOUNDED_OFFSET, 2000);
    mac.set_mac_state_misc_u32_at(
        MISC_CITY_AGE_OFFSET,
        2 * MAC_CALENDAR_PHASES_PER_YEAR + 4 * MAC_CALENDAR_PHASES_PER_MONTH + 7,
    );
    mac.set_mac_state_misc_u32_at(MISC_DIFFICULTY_OFFSET, 2);
    mac.set_mac_state_misc_u32_at(MISC_CITY_PROGRESSION_OFFSET, 4);
    mac.set_mac_state_misc_u32_at(MISC_GARBAGE_OFFSET, 37);
    mac.set_mac_state_misc_u32_at(MISC_CITY_FAME_OFFSET, 0x3840_5060);
    mac.set_mac_state_misc_u32_at(MISC_CITY_ADVERTISING_OFFSET, 0x3c40_5060);
    for (index, technology) in Technology::ALL.into_iter().enumerate() {
        mac.set_mac_state_misc_u32_at(technology.misc_offset(), 1900 + index as u32);
    }
    mac.set_mac_state_misc_u32_at(MISC_SPORTS_TEAMS_OFFSET, 11);
    // Mac CODE 4 owns these fields as demand and pollution-blur modifiers.
    mac.set_mac_state_misc_u32_at(MISC_INDUSTRIAL_MIX_BONUS_OFFSET, 22);
    mac.set_mac_state_misc_u32_at(MISC_INDUSTRIAL_MIX_POLLUTION_BONUS_OFFSET, 0x1234);
    mac.set_mac_state_misc_u32_at(MISC_OLD_ARRESTS_OFFSET, 44);
    mac.set_mac_state_misc_u32_at(MISC_PRISON_BONUS_OFFSET, 55);
    mac.set_mac_state_misc_u32_at(MISC_MONSTER_XTHG_INDEX_OFFSET, 66);
    mac.set_mac_state_misc_u32_at(MISC_CURRENT_DISASTER_TYPE_OFFSET, 77);
    mac.set_mac_state_misc_u32_at(MISC_DISASTER_ACTIVE_OFFSET, 0x8888_9999);
    mac.set_mac_state_misc_u32_at(MISC_SEWER_BONUS_OFFSET, 88);
    mac.set_mac_state_misc_u32_at(MISC_CITY_HAS_OCEAN_OFFSET, 0xaaaa_0001);
    mac.set_mac_state_misc_u32_at(MISC_CITY_HAS_RIVER_OFFSET, 0xbbbb_0002);
    mac.set_mac_state_misc_u32_at(MISC_XUND_OCCUPANCY_OFFSET, 0xcccc_0003);
    for index in 0..MILITARY_TILE_COUNT_VALUES {
        mac.set_mac_state_misc_u32_at(
            MISC_MILITARY_TILE_COUNTS_OFFSET + index * 4,
            0xd000_0000 + index as u32,
        );
    }
    mac.set_mac_state_misc_u32_at(MISC_TRIGGER_DISASTER_OFFSET, 5);
    mac.set_mac_state_misc_u32_at(MISC_RESIDENTIAL_POPULATION_OFFSET, 0xfeed_beef);
    mac.set_mac_state_misc_u32_at(MISC_REWARDS_AVAILABLE_OFFSET, 0x1234_5678);
    mac.set_mac_state_misc_u32_at(MISC_ARCOLOGY_POPULATION_OFFSET, 0x0f);
    mac.set_mac_state_misc_u32_at(MISC_NORMAL_POPULATION_OFFSET, 0xfeed_beef);
    mac.set_mac_state_misc_u32_at(MISC_BUS_PASSENGERS_OFFSET, 0x1111_2222);
    mac.set_mac_state_misc_u32_at(MISC_RAIL_PASSENGERS_OFFSET, 0x3333_4444);
    mac.set_mac_state_misc_u32_at(MISC_SUBWAY_PASSENGERS_OFFSET, 0x5555_6666);
    mac.set_mac_state_misc_u32_at(MISC_POWER_USED_PERCENT_OFFSET, 0x7777_0088);
    mac.set_mac_state_misc_u32_at(0x019c, 600);
    assert_eq!(mac.pe_w_city_garbage, 0);
    assert_eq!(mac.pe_w_sports_teams, 0);
    assert_eq!(mac.pe_w_neighbor_fame_rank(0), Some(0));
    assert_eq!(mac.pe_w_industrial_mix_bonus, 0);
    assert_eq!(mac.pe_w_industrial_mix_pollution_bonus, 0);
    assert_eq!(mac.pe_w_old_arrests, 0);
    assert_eq!(mac.pe_w_prison_bonus, 0);
    assert_eq!(mac.pe_w_monster_xthg_index, 0xffff);
    assert_eq!(mac.pe_w_current_disaster_type, 0);
    assert_eq!(mac.pe_dw_disaster_active, 0);
    assert_eq!(mac.pe_dw_city_has_ocean, 0);
    assert_eq!(mac.pe_dw_city_has_river, 0);
    assert_eq!(mac.pe_xund_occupancy, 0);
    assert_eq!(mac.pe_military_tile_count, [0; MILITARY_TILE_COUNT_VALUES]);
    mac.save(&mac_path).expect("save Mac-layout city");

    let loaded_mac = City::load(&mac_path).expect("reload Mac-layout city");
    assert_eq!(loaded_mac.kind, CityFileKind::MacCity);
    assert_eq!(loaded_mac.pe_w_city_mode, 0);
    assert_eq!(loaded_mac.pe_dw_city_age, 707);
    assert_eq!(loaded_mac.pe_w_city_difficulty, 2);
    assert_eq!(loaded_mac.pe_w_city_progression, 4);
    assert_eq!(loaded_mac.pe_w_city_garbage, 0);
    assert_eq!(loaded_mac.pe_dw_city_fame, 0);
    assert_eq!(loaded_mac.pe_dw_city_advertising, 0);
    for index in 0..PE_NEIGHBOR_FAME_RANK_COUNT {
        assert_eq!(loaded_mac.pe_w_neighbor_fame_rank(index), Some(0));
    }
    assert_eq!(loaded_mac.pe_w_sports_teams, 0);
    assert_eq!(loaded_mac.pe_w_industrial_mix_bonus, 0);
    assert_eq!(loaded_mac.pe_w_industrial_mix_pollution_bonus, 0);
    let mut loaded_mac = loaded_mac;
    loaded_mac.prepare_pe_fidelity_from_saved_city();
    assert_eq!(loaded_mac.pe_w_industrial_mix_bonus, 22);
    assert_eq!(loaded_mac.pe_w_industrial_mix_pollution_bonus, 0x1234);
    assert_eq!(loaded_mac.pe_dw_rewards_available, 0x1234_5678);
    assert_eq!(loaded_mac.pe_power_used_percent(), 100);
    assert_eq!(
        loaded_mac.saved_date(),
        Some(CityDate {
            year: 2002,
            month: 5
        })
    );
    assert_eq!(
        loaded_mac.mac_state_misc_u32_at(MISC_CITY_MODE_OFFSET),
        Some(1)
    );
    for (offset, expected) in [
        (MISC_CITY_AGE_OFFSET, 707),
        (MISC_DIFFICULTY_OFFSET, 2),
        (MISC_CITY_PROGRESSION_OFFSET, 4),
        (MISC_GARBAGE_OFFSET, 37),
        (MISC_CITY_FAME_OFFSET, 0x3840_5060),
        (MISC_CITY_ADVERTISING_OFFSET, 0x3c40_5060),
        (MISC_SPORTS_TEAMS_OFFSET, 11),
        (MISC_INDUSTRIAL_MIX_BONUS_OFFSET, 22),
        (MISC_INDUSTRIAL_MIX_POLLUTION_BONUS_OFFSET, 0x1234),
        (MISC_OLD_ARRESTS_OFFSET, 44),
        (MISC_PRISON_BONUS_OFFSET, 55),
        (MISC_MONSTER_XTHG_INDEX_OFFSET, 66),
        (MISC_CURRENT_DISASTER_TYPE_OFFSET, 77),
        (MISC_DISASTER_ACTIVE_OFFSET, 0x8888_9999),
        (MISC_SEWER_BONUS_OFFSET, 88),
        (MISC_CITY_HAS_OCEAN_OFFSET, 0xaaaa_0001),
        (MISC_CITY_HAS_RIVER_OFFSET, 0xbbbb_0002),
        (MISC_XUND_OCCUPANCY_OFFSET, 0xcccc_0003),
        (MISC_TRIGGER_DISASTER_OFFSET, 5),
        (MISC_RESIDENTIAL_POPULATION_OFFSET, 0xfeed_beef),
        (MISC_REWARDS_AVAILABLE_OFFSET, 0x1234_5678),
        (MISC_BUS_PASSENGERS_OFFSET, 0x1111_2222),
        (MISC_RAIL_PASSENGERS_OFFSET, 0x3333_4444),
        (MISC_SUBWAY_PASSENGERS_OFFSET, 0x5555_6666),
        (MISC_POWER_USED_PERCENT_OFFSET, 0x7777_0088),
    ] {
        assert_eq!(loaded_mac.mac_state_misc_u32_at(offset), Some(expected));
    }
    for index in 0..MILITARY_TILE_COUNT_VALUES {
        assert_eq!(
            loaded_mac.mac_state_misc_u32_at(MISC_MILITARY_TILE_COUNTS_OFFSET + index * 4),
            Some(0xd000_0000 + index as u32)
        );
    }
    for (index, technology) in Technology::ALL.into_iter().enumerate() {
        assert_eq!(
            loaded_mac.mac_state_misc_u32_at(technology.misc_offset()),
            Some(1900 + index as u32)
        );
    }
    assert_eq!(
        loaded_mac.mac_state_misc_u32_at(MISC_ARCOLOGY_POPULATION_OFFSET),
        Some(0x0f)
    );
    assert_eq!(
        loaded_mac.mac_state_misc_u32_at(MISC_NORMAL_POPULATION_OFFSET),
        Some(0xfeed_beef)
    );
    assert_eq!(loaded_mac.mac_state_misc_u32_at(0x019c), Some(600));
    let _ = fs::remove_file(mac_path);

    let win_path = std::env::temp_dir().join(format!(
        "sc2000-win-pe-pop-cache-{}.sc2",
        std::process::id()
    ));
    let mut win = City::new_blank(&win_path, "WinPopHydration").expect("blank Win95 city");
    win.pe_w_city_mode = 1;
    win.pe_dw_city_has_ocean = 1;
    win.pe_dw_city_has_river = 2;
    win.pe_xund_occupancy = 3;
    for index in 0..MILITARY_TILE_COUNT_VALUES {
        win.set_misc_u32_at(
            MISC_MILITARY_TILE_COUNTS_OFFSET + index * 4,
            100 + index as u32,
        );
    }
    for index in 0..PE_NEIGHBOR_FAME_RANK_COUNT {
        win.set_pe_w_neighbor_fame_rank(index, 1900 + index as u16);
    }
    win.pe_w_sports_teams = 11;
    win.pe_w_industrial_mix_bonus = 22;
    win.pe_w_industrial_mix_pollution_bonus = 0x1234;
    win.pe_w_old_arrests = 44;
    win.pe_w_prison_bonus = 55;
    win.pe_w_monster_xthg_index = 66;
    win.pe_w_current_disaster_type = 77;
    win.pe_dw_disaster_active = 0x8888_9999;
    win.pe_w_sewer_bonus = 88;
    win.pe_w_city_garbage = 37;
    win.pe_w_trigger_disaster = 5;
    win.pe_dw_residential_population = 9_000;
    win.pe_dw_rewards_available = 0x1234_5678;
    win.set_pe_dw_arcology_population(55_000);
    win.set_misc_u32_at(MISC_NORMAL_POPULATION_OFFSET, 8_000);
    win.save(&win_path).expect("save Win95-layout city");

    let loaded_win = City::load(&win_path).expect("reload Win95-layout city");
    assert_eq!(loaded_win.kind, CityFileKind::WinSc2);
    assert_eq!(loaded_win.pe_w_city_mode, 1);
    assert_eq!(loaded_win.pe_dw_city_has_ocean, 1);
    assert_eq!(loaded_win.pe_dw_city_has_river, 2);
    assert_eq!(loaded_win.pe_xund_occupancy, 3);
    assert_eq!(
        loaded_win.pe_military_tile_count,
        std::array::from_fn(|index| 100 + index as u16)
    );
    for index in 0..PE_NEIGHBOR_FAME_RANK_COUNT {
        assert_eq!(
            loaded_win.pe_w_neighbor_fame_rank(index),
            Some(1900 + index as u16)
        );
    }
    assert_eq!(loaded_win.pe_w_sports_teams, 11);
    assert_eq!(loaded_win.pe_w_industrial_mix_bonus, 22);
    assert_eq!(loaded_win.pe_w_industrial_mix_pollution_bonus, 0x1234);
    assert_eq!(loaded_win.pe_w_old_arrests, 44);
    assert_eq!(loaded_win.pe_w_prison_bonus, 55);
    assert_eq!(loaded_win.pe_w_monster_xthg_index, 66);
    assert_eq!(loaded_win.pe_w_current_disaster_type, 77);
    assert_eq!(loaded_win.pe_dw_disaster_active, 0x8888_9999);
    assert_eq!(loaded_win.pe_w_sewer_bonus, 88);
    assert_eq!(loaded_win.pe_w_city_garbage, 37);
    assert_eq!(loaded_win.pe_w_trigger_disaster, 5);
    assert_eq!(loaded_win.pe_dw_residential_population, 9_000);
    assert_eq!(loaded_win.pe_dw_rewards_available, 0x1234_5678);
    assert_eq!(loaded_win.pe_dw_arcology_population(), 55_000);
    assert_eq!(loaded_win.pe_dw_city_population(), 8_000);
    let _ = fs::remove_file(win_path);
}

#[test]
fn citygame_extension_v8_still_decodes_after_geography_state_v9() {
    let payload = encode_citygame_extension(
        0,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        Some(7),
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
    );
    assert_eq!(u32::from_be_bytes(payload[0..4].try_into().unwrap()), 8);
    let chunk = CityChunk {
        name: CITYGAME_EXTENSION_CHUNK.to_owned(),
        offset: 0,
        raw_len: payload.len(),
        data: sc2_rle_encode(&payload),
    };

    let extension = decode_citygame_extension(&[chunk]).expect("decode legacy CGEX v8");
    assert_eq!(extension.version, 8);
    assert_eq!(extension.pe_sewer_bonus, Some(7));
    assert_eq!(extension.pe_city_has_ocean, None);
    assert_eq!(extension.pe_city_has_river, None);
}

#[test]
fn citygame_extension_v13_still_decodes_after_traffic_state_v14() {
    let payload = encode_citygame_extension(
        0,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        Some(0x1234),
        Some(-56),
        None,
        None,
    );
    assert_eq!(u32::from_be_bytes(payload[0..4].try_into().unwrap()), 13);
    let chunk = CityChunk {
        name: CITYGAME_EXTENSION_CHUNK.to_owned(),
        offset: 0,
        raw_len: payload.len(),
        data: sc2_rle_encode(&payload),
    };
    let extension = decode_citygame_extension(&[chunk]).expect("decode CGEX v13");
    assert_eq!(extension.pe_industrial_mix_bonus, Some(0x1234));
    assert_eq!(extension.pe_industrial_mix_pollution_bonus, Some(-56));
    assert_eq!(extension.pe_traffic_count, None);
}

#[test]
fn pe_progression_survives_mac_layout_save_reload_without_replacing_mac_misc() {
    let path = std::env::temp_dir().join(format!(
        "sc2000-pe-progression-mac-layout-{}.city",
        std::process::id()
    ));
    let mut city = City::new_blank(&path, "ProgressionCrossLayout").expect("blank city");
    city.kind = CityFileKind::MacCity;
    city.set_mac_state_misc_u32_at(MISC_CITY_PROGRESSION_OFFSET, 0x1234_5678);
    city.prepare_pe_fidelity_from_saved_city();
    assert_eq!(city.pe_w_city_progression, 0x5678);

    // Model a Win95 milestone update while the native Mac MISC slot remains
    // owned by the Mac container codec.
    city.pe_w_city_progression = 7;
    city.save(&path).expect("save Mac-layout city");
    let mut loaded = City::load(&path).expect("reload Mac-layout city");
    assert_eq!(loaded.kind, CityFileKind::MacCity);
    assert_eq!(
        loaded.mac_state_misc_u32_at(MISC_CITY_PROGRESSION_OFFSET),
        Some(0x1234_5678)
    );
    loaded.prepare_pe_fidelity_from_saved_city();
    assert_eq!(loaded.pe_w_city_progression, 7);

    loaded.pe_w_city_progression = 9;
    loaded.save(&path).expect("save updated PE progression");
    let mut reloaded = City::load(&path).expect("reload updated Mac-layout city");
    assert_eq!(
        reloaded.mac_state_misc_u32_at(MISC_CITY_PROGRESSION_OFFSET),
        Some(0x1234_5678)
    );
    reloaded.prepare_pe_fidelity_from_saved_city();
    assert_eq!(reloaded.pe_w_city_progression, 9);

    let _ = std::fs::remove_file(path);
}

#[test]
fn cross_layout_pe_rewards_and_mac_civic_latch_round_trip_independently() {
    let mac_path = std::env::temp_dir().join(format!(
        "sc2000-pe-rewards-mac-layout-{}.city",
        std::process::id()
    ));
    let mac_latch = 0xcafe_1357;
    let mut mac_city = City::new_blank(&mac_path, "RewardsCrossLayout").expect("blank city");
    mac_city.kind = CityFileKind::MacCity;
    mac_city.set_mac_state_misc_u32_at(MISC_REWARDS_AVAILABLE_OFFSET, mac_latch);
    mac_city.prepare_pe_fidelity_from_saved_city();
    mac_city.set_rewards_available_mask(0x0a);
    mac_city
        .save(&mac_path)
        .expect("save PE on Mac-layout city");

    let mut loaded_mac = City::load(&mac_path).expect("reload Mac-layout city");
    assert_eq!(
        loaded_mac.misc_u32_at(MISC_REWARDS_AVAILABLE_OFFSET),
        Some(mac_latch)
    );
    loaded_mac.prepare_pe_fidelity_from_saved_city();
    assert_eq!(loaded_mac.rewards_available_mask(), 0x0a);
    let pe_extension = decode_citygame_extension(&loaded_mac.chunks).expect("CGEX v12 PE state");
    assert_eq!(pe_extension.pe_rewards_available, Some(0xcafe_134a));
    let _ = std::fs::remove_file(mac_path);

    let win_path = std::env::temp_dir().join(format!(
        "sc2000-mac-latch-win-layout-{}.sc2",
        std::process::id()
    ));
    let pe_rewards = 0xface_0015;
    let mut win_city = City::new_blank(&win_path, "MacLatchCrossLayout").expect("blank city");
    win_city.kind = CityFileKind::WinSc2;
    win_city.set_misc_u32_at(MISC_REWARDS_AVAILABLE_OFFSET, pe_rewards);
    win_city.prepare_mac_fidelity_from_saved_city();
    win_city.set_mac_state_misc_u32_at(MISC_REWARDS_AVAILABLE_OFFSET, pe_rewards | 0x02);
    win_city
        .save(&win_path)
        .expect("save Mac on Win-layout city");

    let mut loaded_win = City::load(&win_path).expect("reload Win-layout city");
    assert_eq!(
        loaded_win.misc_u32_at(MISC_REWARDS_AVAILABLE_OFFSET),
        Some(pe_rewards)
    );
    loaded_win.prepare_mac_fidelity_from_saved_city();
    assert_eq!(
        loaded_win.mac_state_misc_u32_at(MISC_REWARDS_AVAILABLE_OFFSET),
        Some(pe_rewards | 0x02)
    );
    let mac_extension = decode_citygame_extension(&loaded_win.chunks).expect("CGEX v12 Mac state");
    assert_eq!(mac_extension.mac_civic_latch, Some(pe_rewards | 0x02));
    let _ = std::fs::remove_file(win_path);
}

#[test]
fn pe_industry_mix_state_uses_extension_on_mac_layout_without_clobbering_mac_fields() {
    let path = std::env::temp_dir().join(format!(
        "sc2000-pe-industry-mix-cross-layout-{}.city",
        std::process::id()
    ));
    let native_mix = 0xabcd_0042;
    let native_pollution = 0x1234_fffc;
    let mut city = City::new_blank(&path, "Industry Cross Layout").expect("blank city");
    city.kind = CityFileKind::MacCity;
    city.set_mac_state_misc_u32_at(MISC_INDUSTRIAL_MIX_BONUS_OFFSET, native_mix);
    city.set_mac_state_misc_u32_at(MISC_INDUSTRIAL_MIX_POLLUTION_BONUS_OFFSET, native_pollution);
    city.prepare_pe_fidelity_from_saved_city();
    assert_eq!(city.pe_w_industrial_mix_bonus(), 0x42);
    assert_eq!(city.pe_w_industrial_mix_pollution_bonus(), -4);
    city.set_rewards_available_mask(0x0a);

    city.set_pe_w_industrial_mix_bonus(0x789a);
    city.set_pe_w_industrial_mix_pollution_bonus(-123);
    city.save(&path).expect("save PE state on Mac container");
    assert_eq!(
        city.misc_u32_at(MISC_INDUSTRIAL_MIX_BONUS_OFFSET),
        Some(native_mix)
    );
    assert_eq!(
        city.misc_u32_at(MISC_INDUSTRIAL_MIX_POLLUTION_BONUS_OFFSET),
        Some(native_pollution)
    );
    let extension = decode_citygame_extension(&city.chunks).expect("decode CGEX v16");
    assert_eq!(extension.version, 16);
    assert_eq!(extension.pe_rewards_available, Some(0x0a));
    assert_eq!(extension.pe_industrial_mix_bonus, Some(0x789a));
    assert_eq!(extension.pe_industrial_mix_pollution_bonus, Some(-123));

    let mut reloaded = City::load(&path).expect("reload PE state");
    assert_eq!(
        reloaded.misc_u32_at(MISC_INDUSTRIAL_MIX_BONUS_OFFSET),
        Some(native_mix)
    );
    assert_eq!(
        reloaded.misc_u32_at(MISC_INDUSTRIAL_MIX_POLLUTION_BONUS_OFFSET),
        Some(native_pollution)
    );
    reloaded.prepare_pe_fidelity_from_saved_city();
    assert_eq!(reloaded.pe_w_industrial_mix_bonus(), 0x789a);
    assert_eq!(reloaded.pe_w_industrial_mix_pollution_bonus(), -123);

    reloaded.set_pe_w_industrial_mix_bonus(0x4321);
    reloaded.save(&path).expect("save updated PE state");
    let mut reopened = City::load(&path).expect("reload updated PE state");
    reopened.prepare_pe_fidelity_from_saved_city();
    assert_eq!(reopened.pe_w_industrial_mix_bonus(), 0x4321);
    assert_eq!(reopened.pe_w_industrial_mix_pollution_bonus(), -123);
    assert_eq!(
        reopened.misc_u32_at(MISC_INDUSTRIAL_MIX_BONUS_OFFSET),
        Some(native_mix)
    );
    assert_eq!(
        reopened.misc_u32_at(MISC_INDUSTRIAL_MIX_POLLUTION_BONUS_OFFSET),
        Some(native_pollution)
    );
    let _ = std::fs::remove_file(path);
}

#[test]
fn national_economy_state_round_trips_in_both_city_formats() {
    for (kind, extension) in [
        (CityFileKind::MacCity, "mac"),
        (CityFileKind::WinSc2, "sc2"),
    ] {
        let path = std::env::temp_dir().join(format!(
            "sc2000-national-economy-{extension}-{}.{}",
            std::process::id(),
            extension,
        ));
        let mut city = City::new_blank(&path, "NationalEconomy").expect("blank city");
        city.kind = kind;
        city.pe_dw_national_population = 60_000;
        city.pe_dw_national_value = 18_000;
        city.pe_w_national_tax = 3;
        city.pe_w_national_trend = 1;

        city.save(&path).expect("save city");
        let loaded = City::load(&path).expect("reload city");
        assert_eq!(
            [
                loaded.misc_u32_at(MISC_NATIONAL_POPULATION_OFFSET),
                loaded.misc_u32_at(MISC_NATIONAL_VALUE_OFFSET),
                loaded.misc_u32_at(MISC_NATIONAL_TAX_OFFSET),
                loaded.misc_u32_at(MISC_NATIONAL_TREND_OFFSET),
            ],
            [Some(60_000), Some(18_000), Some(3), Some(1)],
            "national economy fields at MISC +0x50..+0x5c are shared by Mac and PE"
        );
        let _ = fs::remove_file(path);
    }
}

#[test]
fn shared_year_and_funds_fields_round_trip_in_both_city_formats() {
    for (kind, extension) in [
        (CityFileKind::MacCity, "mac"),
        (CityFileKind::WinSc2, "sc2"),
    ] {
        let path = std::env::temp_dir().join(format!(
            "sc2000-shared-year-funds-{extension}-{}.{}",
            std::process::id(),
            extension,
        ));
        let mut city = City::new_blank(&path, "SharedYearFunds").expect("blank city");
        city.kind = kind;
        city.initialize_new_city_founding_year(1950);
        city.set_city_funds(75_000);

        city.save(&path).expect("save city");
        let loaded = City::load(&path).expect("reload city");
        assert_eq!(loaded.year_founded(), Some(1950));
        assert_eq!(loaded.misc_u32_at(MISC_YEAR_FOUNDED_OFFSET), Some(1950));
        assert_eq!(loaded.city_funds(), 75_000);
        assert_eq!(loaded.misc_i32_at(MISC_CITY_FUNDS_OFFSET), Some(75_000));
        let _ = fs::remove_file(path);
    }
}

#[test]
fn misc_traffic_count_is_not_xtrf_sum() {
    let Some(mac) = load_mac() else {
        return;
    };
    let Some(traffic_count) = mac.misc_traffic_count() else {
        return;
    };
    let xtrf_sum: u32 = mac
        .traffic_density_slice()
        .expect("mac XTRF")
        .iter()
        .map(|&value| value as u32)
        .sum();

    assert_ne!(
        traffic_count, xtrf_sum,
        "MISC Traffic Count is a separate sim total, not the XTRF byte sum"
    );
}

#[test]
fn density_sum_handles_optional_metric_maps() {
    assert_eq!(density_sum(None), None);
    assert_eq!(density_sum(Some(&[1, 2, 3, 250])), Some(256));
}

#[test]
fn metric_density_layers_sync_and_round_trip() {
    let temp = std::env::temp_dir().join("sc2000_rust_metric_density_test.sc2");
    let mut city = City::new_blank(&temp, "test").expect("blank city");
    let mut land_value = vec![0u8; CITY_TILES];
    let mut crime = vec![0u8; CITY_TILES];
    let mut pollution = vec![0u8; CITY_TILES];
    let mut police = vec![0u8; CITY_TILES];
    let mut fire = vec![0u8; CITY_TILES];
    let mut population = vec![0u8; CITY_TILES];
    let mut growth = vec![0x7fu8; CITY_TILES];
    land_value[city_index(12, 34)] = 180;
    crime[city_index(12, 34)] = 40;
    pollution[city_index(12, 34)] = 90;
    police[city_index(12, 34)] = 150;
    fire[city_index(12, 34)] = 120;
    population[city_index(12, 34)] = 64;
    growth[city_index(12, 34)] = 0x82;
    city.sync_land_value_from_slice(&land_value);
    city.sync_crime_density_from_slice(&crime);
    city.sync_pollution_density_from_slice(&pollution);
    city.sync_police_coverage_from_slice(&police);
    city.sync_fire_coverage_from_slice(&fire);
    city.sync_population_density_from_slice(&population);
    city.sync_rate_of_growth_from_slice(&growth);

    assert_eq!(city.land_value_at(12, 34), Some(180));
    assert_eq!(city.crime_density_at(12, 34), Some(40));
    assert_eq!(city.pollution_density_at(12, 34), Some(90));
    assert_eq!(city.police_coverage_at(12, 34), Some(150));
    assert_eq!(city.fire_coverage_at(12, 34), Some(120));
    assert_eq!(city.population_density_at(12, 34), Some(64));
    assert_eq!(city.rate_of_growth_at(12, 34), Some(0x82));

    city.save(&temp).expect("save city");
    let reloaded = City::load(&temp).expect("reload city");
    assert_eq!(reloaded.land_value_at(12, 34), Some(180));
    assert_eq!(reloaded.crime_density_at(12, 34), Some(40));
    assert_eq!(reloaded.pollution_density_at(12, 34), Some(90));
    assert_eq!(reloaded.police_coverage_at(12, 34), Some(150));
    assert_eq!(reloaded.fire_coverage_at(12, 34), Some(120));
    assert_eq!(reloaded.population_density_at(12, 34), Some(64));
    assert_eq!(reloaded.rate_of_growth_at(12, 34), Some(0x82));
    let _ = std::fs::remove_file(temp);
}

#[test]
fn chunk_u32_values_round_trip_through_save() {
    const XGRP_VALUES: usize = 16 * 52;
    let temp = std::env::temp_dir().join("sc2000_rust_xgrp_chunk_test.sc2");
    let mut city = City::new_blank(&temp, "test").expect("blank city");
    let values = (0..XGRP_VALUES as u32)
        .map(|value| value.saturating_mul(3).saturating_add(7))
        .collect::<Vec<_>>();

    city.set_chunk_u32_values("XGRP", 0, &values);
    city.save(&temp).expect("save city");
    let reloaded = City::load(&temp).expect("reload city");

    assert_eq!(reloaded.chunk_u32_values("XGRP", 0, XGRP_VALUES), values);
    assert_eq!(
        reloaded.chunk_u32_values("MISC", 0, 1),
        [WIN95_MISC_RECORD_VERSION]
    );
    let _ = std::fs::remove_file(temp);
}

#[test]
fn decodes_misc_water_inputs() {
    let mut misc = vec![0u8; 0x0e44];
    misc[MISC_HUMIDITY_OFFSET..MISC_HUMIDITY_OFFSET + MISC_FIELD_LEN]
        .copy_from_slice(&42u32.to_be_bytes());
    misc[MISC_WATER_TABLE_LEVEL_OFFSET..MISC_WATER_TABLE_LEVEL_OFFSET + MISC_FIELD_LEN]
        .copy_from_slice(&7u32.to_be_bytes());
    let chunks = [CityChunk {
        name: "MISC".to_owned(),
        offset: 0,
        raw_len: misc.len(),
        data: misc,
    }];

    assert_eq!(decode_misc_u32(&chunks, MISC_HUMIDITY_OFFSET).unwrap(), 42);
    assert_eq!(
        decode_misc_u32(&chunks, MISC_WATER_TABLE_LEVEL_OFFSET).unwrap(),
        7
    );
}

#[test]
fn misc_water_inputs_prefer_current_misc_chunk_values() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    assert_eq!(city.misc_humidity(), Some(0));
    assert_eq!(
        city.misc_water_table_level(),
        Some(BLANK_MAP_BASE_HEIGHT as u32)
    );

    city.set_misc_u32_at(MISC_HUMIDITY_OFFSET, 42);
    city.set_misc_u32_at(MISC_WATER_TABLE_LEVEL_OFFSET, 7);

    assert_eq!(city.misc_humidity(), Some(42));
    assert_eq!(city.misc_water_table_level(), Some(7));
}

#[test]
fn mac_selected_water_height_reads_packed_misc_slot_912_low_word() {
    let mut city =
        City::new_blank("/tmp/sc2000-mac-water-height.sc2", "MacWater").expect("blank city");
    city.kind = CityFileKind::MacCity;
    city.set_mac_state_misc_u32_at(53 * MISC_FIELD_LEN, 0xdead_beef);
    city.set_mac_state_misc_u32_at(912 * MISC_FIELD_LEN, 0x1234_0015);
    city.set_mac_state_misc_u32_at(MAC_WEATHER_HUMIDITY_STATE_OFFSET, 0x1234_001f);

    assert_eq!(city.mac_selected_water_height(), Some(21));
    assert_eq!(city.mac_weather_humidity(), Some(31));
    assert_eq!(
        city.mac_state_misc_u32_at(912 * MISC_FIELD_LEN),
        Some(0x1234_0015)
    );

    city.set_mac_selected_water_height(4);
    assert_eq!(city.mac_state_misc_u32_at(912 * MISC_FIELD_LEN), Some(4));
    assert_eq!(
        city.mac_state_misc_u32_at(53 * MISC_FIELD_LEN),
        Some(0xdead_beef)
    );

    city.set_mac_state_misc_u32_at(MAC_WEATHER_HUMIDITY_STATE_OFFSET, 0x1234_00ff);
    assert_eq!(city.mac_weather_humidity(), Some(255));

    let mut win95 =
        City::new_blank("/tmp/sc2000-pe-water-height.sc2", "PeWater").expect("blank city");
    win95.set_misc_u32_at(53 * MISC_FIELD_LEN, 27);
    win95.set_misc_u32_at(MISC_WATER_TABLE_LEVEL_OFFSET, 0x1234_0015);
    win95.set_misc_u32_at(MISC_HUMIDITY_OFFSET, 0x1234_001f);
    assert_eq!(win95.mac_selected_water_height(), Some(21));
    assert_eq!(win95.mac_weather_humidity(), Some(31));
    win95.set_mac_selected_water_height(4);
    assert_eq!(win95.mac_selected_water_height(), Some(4));
    assert_eq!(win95.misc_water_table_level(), Some(4));
    assert_eq!(win95.misc_u32_at(53 * MISC_FIELD_LEN), Some(27));
}

#[test]
fn decodes_misc_scenario_status_counts() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");

    city.set_misc_u32_at(MISC_CITY_CRIME_OFFSET, 123);
    city.set_misc_u32_at(MISC_CITY_TRAFFIC_OFFSET, 456);
    city.set_misc_u32_at(MISC_CITY_POLLUTION_OFFSET, 789);

    assert_eq!(city.misc_crime_count(), Some(123));
    assert_eq!(city.misc_traffic_count(), Some(456));
    assert_eq!(city.misc_pollution_count(), Some(789));
}

#[test]
fn public_misc_readers_preserve_big_endian_signed_values_and_bounds() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut misc = vec![0u8; 16];
    misc[0..4].copy_from_slice(&123_456u32.to_be_bytes());
    misc[4..8].copy_from_slice(&(-789i32).to_be_bytes());
    misc[8..12].copy_from_slice(&42u32.to_be_bytes());
    // Replace the valid versioned MISC created by `new_blank`; this test
    // intentionally exercises a short synthetic chunk.
    city.chunks.retain(|chunk| chunk.name != "MISC");
    city.chunks.push(CityChunk {
        name: "MISC".to_owned(),
        offset: 0,
        raw_len: misc.len(),
        data: misc,
    });

    assert_eq!(city.misc_u32_at(0), Some(123_456));
    assert_eq!(city.misc_i32_at(4), Some(-789));
    assert_eq!(
        city.misc_u32_values(0, 3),
        vec![123_456, (-789i32) as u32, 42]
    );
    assert_eq!(city.misc_i32_values(4, 2), vec![-789, 42]);
    assert_eq!(city.misc_u32_at(16), None);
    assert!(city.misc_u32_values(12, 2).is_empty());
}

#[test]
fn public_misc_writer_handles_contiguous_big_endian_values() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");

    city.set_misc_u32_values(8, &[0x0102_0304, 0x1112_1314, 42]);

    assert_eq!(
        city.misc_u32_values(8, 3),
        vec![0x0102_0304, 0x1112_1314, 42]
    );
    let misc = decode_misc_bytes(&city.chunks).expect("MISC chunk");
    assert_eq!(&misc[8..12], &[0x01, 0x02, 0x03, 0x04]);
    assert_eq!(&misc[12..16], &[0x11, 0x12, 0x13, 0x14]);
    assert_eq!(&misc[16..20], &42u32.to_be_bytes());
}

#[test]
fn pe_w_neighbor_fame_ranks_bss_4cb430_hydrate_persist_and_tech_years() {
    // PE MISC↔BSS WORD[17] @ `0x4cb430` / MISC `0x738` (`0x43179f` / `0x433dd8`);
    // same dwords as Technology discovery years.
    let path = "/tmp/sc2000-neighbor-fame-bss.sc2";
    let mut city = City::new_blank(path, "NFameBss").expect("blank");
    city.set_misc_u32_at(MISC_NEIGHBOR_FAME_RANKS_OFFSET, 1955);
    assert_eq!(city.pe_w_neighbor_fame_rank(0), Some(1955));
    assert_eq!(
        city.technology_discovery_year(Technology::GasPower),
        Some(1955)
    );
    city.set_pe_w_neighbor_fame_rank(3, 2001);
    city.set_technology_discovery_year(Technology::NuclearPower, 1960);
    assert_eq!(city.pe_w_neighbor_fame_rank(1), Some(1960));
    city.save(path).expect("save");
    let loaded = City::load(path).expect("load");
    assert_eq!(loaded.pe_w_neighbor_fame_rank(0), Some(1955));
    assert_eq!(loaded.pe_w_neighbor_fame_rank(1), Some(1960));
    assert_eq!(loaded.pe_w_neighbor_fame_rank(3), Some(2001));
    assert_eq!(
        loaded.misc_u32_at(MISC_NEIGHBOR_FAME_RANKS_OFFSET + 3 * 4),
        Some(2001)
    );
    let _ = std::fs::remove_file(path);
}

#[test]
fn pe_neighbor_fame_ranks_on_mac_layout_use_pe_loader_view_and_round_trip_in_cgex() {
    let path = std::env::temp_dir().join(format!(
        "citygame-pe-mac-neighbor-ranks-{}.city",
        std::process::id()
    ));
    let mut source = City::new_blank(&path, "MacRanks").unwrap();
    source.kind = CityFileKind::MacCity;
    let mut saved_words = [0_u32; PE_NEIGHBOR_FAME_RANK_COUNT];
    for (index, word) in saved_words.iter_mut().enumerate() {
        *word = 1900 + index as u32 * 10;
        source.set_mac_state_misc_u32_at(
            MISC_NEIGHBOR_FAME_RANKS_OFFSET + index * MISC_FIELD_LEN,
            *word,
        );
    }
    source.save(&path).unwrap();

    let mut city = City::load(&path).unwrap();
    assert_eq!(city.kind, CityFileKind::MacCity);
    assert_eq!(city.pe_w_neighbor_fame_rank(0), Some(0));
    city.prepare_pe_fidelity_from_saved_city();
    for (index, word) in saved_words.iter().enumerate() {
        assert_eq!(city.pe_w_neighbor_fame_rank(index), Some(*word as u16));
    }

    // PE selected-engine updates change the PE runtime cache, while the Mac
    // container keeps its native MISC bytes and saves the PE view separately.
    city.set_pe_w_neighbor_fame_rank(0, 0);
    city.set_pe_w_neighbor_fame_rank(4, 0x8123);
    city.save(&path).unwrap();
    for (index, word) in saved_words.iter().enumerate() {
        assert_eq!(
            city.mac_state_misc_u32_at(MISC_NEIGHBOR_FAME_RANKS_OFFSET + index * 4),
            Some(*word),
            "Mac-owned MISC slot {index} must remain intact"
        );
    }

    let mut reloaded = City::load(&path).unwrap();
    reloaded.prepare_pe_fidelity_from_saved_city();
    assert_eq!(reloaded.pe_w_neighbor_fame_rank(0), Some(0));
    assert_eq!(reloaded.pe_w_neighbor_fame_rank(4), Some(0x8123));
    assert_eq!(
        reloaded.pe_w_neighbor_fame_rank(1),
        Some(saved_words[1] as u16)
    );
    for (index, word) in saved_words.iter().enumerate() {
        assert_eq!(
            reloaded.mac_state_misc_u32_at(MISC_NEIGHBOR_FAME_RANKS_OFFSET + index * 4),
            Some(*word)
        );
    }
    let _ = std::fs::remove_file(path);
}

#[test]
fn pe_newspaper_and_technology_writers_preserve_mac_state_vector() {
    let mut city = City::new_blank("/tmp/sc2000-mac-pe-newspaper-writers.sc2", "MacNewsWriters")
        .expect("blank");
    city.kind = CityFileKind::MacCity;

    let technology = Technology::GasPower;
    let technology_offset = technology.misc_offset();
    let technology_sentinel = 0x6a5b_4c3d;
    city.set_mac_state_misc_u32_at(technology_offset, technology_sentinel);

    for index in 0..NEWSPAPER_STORY_LIST_MISC_DWORDS {
        let sentinel = 0x5a00_0000 | index as u32;
        city.set_mac_state_misc_u32_at(
            MISC_NEWSPAPER_STORY_LIST_OFFSET + index * MISC_FIELD_LEN,
            sentinel,
        );
    }

    // The PE cache can still change for a Win95-engine/Mac-save run, but
    // these bytes are the Mac CODE 4 vector and must survive saveback.
    city.set_technology_discovery_year(technology, 0);
    assert_eq!(city.technology_discovery_year(technology), Some(0));
    assert_eq!(
        city.mac_state_misc_u32_at(technology_offset),
        Some(technology_sentinel)
    );

    city.queue_pe_newspaper_story(4, 0);
    for index in 0..NEWSPAPER_STORY_LIST_MISC_DWORDS {
        assert_eq!(
            city.mac_state_misc_u32_at(MISC_NEWSPAPER_STORY_LIST_OFFSET + index * MISC_FIELD_LEN),
            Some(0x5a00_0000 | index as u32),
            "PE story queue overwrote Mac CODE 4 state dword {index}"
        );
    }
    assert!(
        !city.take_pe_newspaper_extra_popup_pending(),
        "a PE-only Mac-layout story must not open the Win95 Extra popup"
    );
}

#[test]
fn pe_publish_local_fame_base_from_city_age_41358c() {
    // PE UpdateGraphData @ `0x41358c`: [0x4cb020] = [0x4cae04] / 0x12c.
    let mut city = City::new_blank("/tmp/sc2000-fame-base.sc2", "FameBase").expect("blank");
    city.pe_dw_city_age = 0x12c * 7 + 50;
    city.pe_publish_local_fame_base_from_city_age_41358c();
    assert_eq!(city.pe_w_local_fame_base(), 7);
    city.pe_dw_city_age = 0x12c - 1;
    city.pe_publish_local_fame_base_from_city_age_41358c();
    assert_eq!(city.pe_w_local_fame_base(), 0);
}

#[test]
fn pe_publish_graph_calendar_words_from_city_age_41354e() {
    // PE @ `0x41354e`: month=(age/0x19)%0xc, season=((month+1)%0xc)/3, fame=age/0x12c.
    let mut city = City::new_blank("/tmp/sc2000-graph-cal.sc2", "GraphCal").expect("blank");
    // age=25*14+3 → month 14%12=2, season (3%12)/3=1, fame 353/300=1.
    city.pe_dw_city_age = 25 * 14 + 3;
    city.pe_publish_graph_calendar_words_from_city_age_41354e();
    assert_eq!(city.pe_w_calendar_month_mod(), 2);
    assert_eq!(city.pe_w_season(), 1);
    assert_eq!(city.pe_w_local_fame_base(), 1);
}

#[test]
fn pe_rebuild_tech_unlock_mask_46a0be_ors_zero_rank_bits() {
    // PE @ `0x46a0be`: zero ranks OR into mask starting at 7; index 1 gated by ordinance.
    let mut city = City::new_blank("/tmp/sc2000-tech-unlock.sc2", "TechUnlock").expect("blank");
    // Nonzero ranks skip; leave 0,2,12 zero and fill the rest.
    for i in 0..17 {
        city.set_pe_w_neighbor_fame_rank(i, 1900);
    }
    city.set_pe_w_neighbor_fame_rank(0, 0);
    city.set_pe_w_neighbor_fame_rank(2, 0);
    city.set_pe_w_neighbor_fame_rank(12, 0);
    city.set_pe_w_neighbor_fame_rank(1, 0);
    let mask = city.pe_rebuild_tech_unlock_mask_46a0be(0);
    assert_eq!(mask, 7 | 0x8 | 0x10 | 0x40);
    assert_eq!(city.pe_w_tech_unlock_high_zero_count(), 1);
    // Ordinance bit 0x20000 blocks index-1 OR.
    let masked = city.pe_rebuild_tech_unlock_mask_46a0be(0x20000);
    assert_eq!(masked, 7 | 0x8 | 0x40);
    assert_eq!(city.pe_w_tech_unlock_mask(), masked);
}

#[test]
fn pe_rebuild_tech_unlock_mask_46a0be_ors_ui_globals_arms_6_through_11() {
    // PE jump-table arms 6..=11 @ `0x46a116`..`0x46a16a` atop defaults `0x4e9560`.
    let mut city = City::new_blank("/tmp/sc2000-tech-ui.sc2", "TechUi").expect("blank");
    for i in 0..17 {
        city.set_pe_w_neighbor_fame_rank(i, 1900);
    }
    for i in 6..=11 {
        city.set_pe_w_neighbor_fame_rank(i, 0);
    }
    let _ = city.pe_rebuild_tech_unlock_mask_46a0be(0);
    // Defaults: 4e9a30=1, 4e9a28=5, 4e9a2c=5, 4e9a20=7; then arm ORs.
    assert_eq!(city.pe_dw_tech_ui_4e9a30(), 1 | 2);
    assert_eq!(city.pe_dw_tech_ui_4e9a28(), 5 | 0xa | 0x10);
    assert_eq!(city.pe_dw_tech_ui_4e9a2c(), 5 | 0x1a);
    assert_eq!(city.pe_dw_tech_ui_4e9a20(), 7 | 8 | 0x10);
    // Second rebuild recopies defaults then ORs (not sticky accumulate).
    let _ = city.pe_rebuild_tech_unlock_mask_46a0be(0);
    assert_eq!(city.pe_dw_tech_ui_4e9a30(), 1 | 2);
    assert_eq!(city.pe_dw_tech_ui_4e9a28(), 5 | 0xa | 0x10);
}

#[test]
fn pe_rebuild_tech_unlock_mask_46a0be_epilogue_ors_4e9a18_for_military_base() {
    let mut city = City::new_blank("/tmp/sc2000-tech-epi.sc2", "TechEpi").expect("blank");
    for i in 0..17 {
        city.set_pe_w_neighbor_fame_rank(i, 1900);
    }
    city.set_misc_u32_at(MISC_MILITARY_BASE_TYPE_OFFSET, 2);
    let _ = city.pe_rebuild_tech_unlock_mask_46a0be(0);
    assert_eq!(city.pe_dw_tech_ui_4e9a18(), 0x3 | 4);
    city.set_misc_u32_at(MISC_MILITARY_BASE_TYPE_OFFSET, 1);
    let _ = city.pe_rebuild_tech_unlock_mask_46a0be(0);
    assert_eq!(city.pe_dw_tech_ui_4e9a18(), 0x3);
}

#[test]
fn pe_clear_neighbor_fame_ranks_and_rebuild_411690_zeros_rewards_and_funds() {
    let mut city = City::new_blank("/tmp/sc2000-clear-fame.sc2", "ClearFame").expect("blank");
    for i in 0..17 {
        city.set_pe_w_neighbor_fame_rank(i, 1900 + i as u16);
    }
    city.pe_w_city_progression = 2;
    city.set_city_funds(1_000);
    city.set_rewards_available_mask(0);
    city.pe_clear_neighbor_fame_ranks_and_rebuild_411690();
    for i in 0..17 {
        assert_eq!(city.pe_w_neighbor_fame_rank(i), Some(0));
    }
    assert_eq!(city.pe_w_city_progression, 6);
    // All ranks zero → mask gets every arm bit from 0..=5 plus high-zero count.
    assert_eq!(
        city.pe_w_tech_unlock_mask(),
        7 | 0x8 | 0x10 | 0x40 | 0x20 | 0x80 | 0x100
    );
    assert_eq!(city.pe_w_tech_unlock_high_zero_count(), 4);
    // PE @ `0x4116b6`/`0x4116dd`: reward bits 0..=4 and +0x7a120 funds.
    assert_eq!(city.rewards_available_mask(), 0x1f);
    assert_eq!(city.city_funds(), 1_000 + 0x7a_120);
}

#[test]
fn pe_clear_fame_cheat_preserves_mac_state_and_updates_separate_rewards() {
    let mut city =
        City::new_blank("/tmp/sc2000-clear-fame-mac.sc2", "ClearFameMac").expect("blank");
    city.kind = CityFileKind::MacCity;
    let technology_sentinels: Vec<_> = Technology::ALL
        .into_iter()
        .enumerate()
        .map(|(index, technology)| {
            let value = 0xa100_0000 | index as u32;
            city.set_mac_state_misc_u32_at(technology.misc_offset(), value);
            (technology, value)
        })
        .collect();
    let progression_sentinel = 0xbeef_0002;
    let rewards_sentinel = 0xcafe_1357;
    city.set_mac_state_misc_u32_at(MISC_CITY_PROGRESSION_OFFSET, progression_sentinel);
    city.set_mac_state_misc_u32_at(MISC_REWARDS_AVAILABLE_OFFSET, rewards_sentinel);
    city.prepare_pe_fidelity_from_saved_city();
    city.pe_w_city_progression = 2;
    city.set_city_funds(1_000);

    city.pe_clear_neighbor_fame_ranks_and_rebuild_412290();

    for (technology, expected) in technology_sentinels {
        assert_eq!(city.misc_u32_at(technology.misc_offset()), Some(expected));
        assert_eq!(
            city.pe_w_neighbor_fame_rank(
                Technology::ALL
                    .iter()
                    .position(|candidate| *candidate == technology)
                    .expect("technology index")
            ),
            Some(0)
        );
    }
    assert_eq!(
        city.misc_u32_at(MISC_CITY_PROGRESSION_OFFSET),
        Some(progression_sentinel)
    );
    assert_eq!(city.pe_w_city_progression, 6);
    assert_eq!(
        city.misc_u32_at(MISC_REWARDS_AVAILABLE_OFFSET),
        Some(rewards_sentinel)
    );
    assert_eq!(city.rewards_available_mask(), 0x1f);
    assert_eq!(city.city_funds(), 1_000 + 0x7a_120);
}

#[test]
fn pe_cheat_add_funds_40bf4d_adds_250() {
    let mut city = City::new_blank("/tmp/sc2000-cheat-funds.sc2", "CheatFunds").expect("blank");
    city.set_city_funds(100);
    city.pe_cheat_add_funds_40bf4d();
    assert_eq!(city.city_funds(), 100 + 0xfa);
}

#[test]
fn pe_cheat_set_trigger_disaster_40cb7f_40cba9_writes_4ca420() {
    let mut city = City::new_blank("/tmp/sc2000-cheat-trig.sc2", "CheatTrig").expect("blank");
    city.pe_cheat_set_trigger_disaster_40cb7f();
    assert_eq!(city.pe_w_trigger_disaster, 0xe);
    assert_eq!(city.misc_u32_at(MISC_TRIGGER_DISASTER_OFFSET), Some(0xe));
    city.pe_cheat_set_trigger_disaster_40cba9();
    assert_eq!(city.pe_w_trigger_disaster, 0xc);
    assert_eq!(city.misc_u32_at(MISC_TRIGGER_DISASTER_OFFSET), Some(0xc));
}

#[test]
fn pe_cheat_accept_military_base_4136da_sets_type_1() {
    let mut city = City::new_blank("/tmp/sc2000-cheat-mil.sc2", "CheatMil").expect("blank");
    city.set_misc_u32_at(MISC_MILITARY_BASE_TYPE_OFFSET, 0);
    city.pe_cheat_accept_military_base_4136da();
    assert_eq!(city.misc_u32_at(MISC_MILITARY_BASE_TYPE_OFFSET), Some(1));
}

#[test]
fn pe_cheat_military_dialog_4142c0_only_accepts_on_7() {
    let mut city = City::new_blank("/tmp/sc2000-cheat-mildlg.sc2", "CheatMilDlg").expect("blank");
    city.set_misc_u32_at(MISC_MILITARY_BASE_TYPE_OFFSET, 0);
    city.pe_cheat_military_dialog_4142c0(6);
    assert_eq!(city.misc_u32_at(MISC_MILITARY_BASE_TYPE_OFFSET), Some(0));
    city.pe_cheat_military_dialog_4142c0(7);
    assert_eq!(city.misc_u32_at(MISC_MILITARY_BASE_TYPE_OFFSET), Some(1));
}

#[test]
fn pe_cheat_clear_fame_40cb75_wires_412290() {
    let mut city = City::new_blank("/tmp/sc2000-cheat-fame.sc2", "CheatFame").expect("blank");
    for i in 0..17 {
        city.set_pe_w_neighbor_fame_rank(i, 1900);
    }
    city.set_city_funds(0);
    city.set_rewards_available_mask(0);
    city.pe_cheat_clear_fame_40cb75();
    assert_eq!(city.pe_w_neighbor_fame_rank(0), Some(0));
    assert_eq!(city.pe_w_city_progression, 6);
    assert_eq!(city.rewards_available_mask(), 0x1f);
    assert_eq!(city.city_funds(), 0x7a_120);
}

#[test]
fn pe_cheat_jump_table_40cd08_dispatches_wired_arms() {
    let mut city = City::new_blank("/tmp/sc2000-cheat-jt.sc2", "CheatJT").expect("blank");
    city.set_city_funds(0);
    city.pe_cheat_jump_table_40cd08(0);
    assert_eq!(city.pe_w_last_cheat_sound_id, 0x19);
    city.pe_cheat_jump_table_40cd08(1);
    assert_eq!(city.city_funds(), 0xfa);
    city.pe_cheat_jump_table_40cd08(3);
    assert_eq!(city.pe_w_trigger_disaster, 0xe);
    city.pe_cheat_jump_table_40cd08(4);
    assert_eq!(city.pe_w_trigger_disaster, 0xc);
    city.pe_cheat_jump_table_40cd08(6);
    assert_eq!(city.misc_u32_at(MISC_MILITARY_BASE_TYPE_OFFSET), Some(1));
    city.pe_cheat_jump_table_40cd08(2);
    assert_eq!(city.pe_w_city_progression, 6);
    assert_eq!(city.city_funds(), 0xfa + 0x7a_120);
}

#[test]
fn pe_cheat_sound_40cb0a_records_sound_id_19() {
    let mut city = City::new_blank("/tmp/sc2000-cheat-snd.sc2", "CheatSnd").expect("blank");
    assert_eq!(city.pe_w_last_cheat_sound_id, 0);
    city.pe_cheat_sound_40cb0a();
    assert_eq!(city.pe_w_last_cheat_sound_id, 0x19);
}

#[test]
fn pe_reseed_neighbor_fame_ranks_42e585_adds_noise_and_zeros_before_founded() {
    // PE @ `0x42e585`: invention table + rand%0x14; zero when < year founded.
    let mut city = City::new_blank("/tmp/sc2000-fame-reseed.sc2", "FameReseed").expect("blank");
    city.pe_w_year_founded = 2000;
    let mut crt = Win95CrtRand::new(1);
    city.pe_reseed_neighbor_fame_ranks_42e585(&mut crt);
    // Index 9 base 1900 always < 2000 → zero.
    assert_eq!(city.pe_w_neighbor_fame_rank(9), Some(0));
    // Index 0 base 1940 < 2000 → zero.
    assert_eq!(city.pe_w_neighbor_fame_rank(0), Some(0));
    // PE @ `0x42e5bf` rebuilds tech-unlock mask after reseed.
    assert_ne!(city.pe_w_tech_unlock_mask(), 0);
    // Index 4 base 2020 + noise stays >= 2000.
    let rank4 = city.pe_w_neighbor_fame_rank(4).expect("rank");
    assert!(
        (2020..2040).contains(&rank4),
        "expected 2020..2040 got {rank4}"
    );
}

#[test]
fn pe_fame_reseed_keeps_mac_technology_slots_while_updating_runtime_cache() {
    let mut city = City::new_blank("/tmp/sc2000-fame-reseed-mac.sc2", "FameMac").expect("blank");
    city.kind = CityFileKind::MacCity;
    city.pe_w_year_founded = 1900;
    let sentinels: Vec<_> = Technology::ALL
        .into_iter()
        .enumerate()
        .map(|(index, technology)| {
            let value = 0xd200_0000 | index as u32;
            city.set_mac_state_misc_u32_at(technology.misc_offset(), value);
            value
        })
        .collect();
    let mut pe = city.clone();
    pe.kind = CityFileKind::WinSc2;

    city.pe_reseed_neighbor_fame_ranks_42e585(&mut Win95CrtRand::new(1));
    pe.pe_reseed_neighbor_fame_ranks_42e585(&mut Win95CrtRand::new(1));

    for (index, technology) in Technology::ALL.into_iter().enumerate() {
        assert_eq!(
            city.pe_w_neighbor_fame_rank(index),
            pe.pe_w_neighbor_fame_rank(index),
            "runtime PE rank {technology:?}"
        );
        assert_eq!(
            city.misc_u32_at(technology.misc_offset()),
            Some(sentinels[index]),
            "Mac slot {technology:?}"
        );
    }
}

#[test]
fn pe_scale_national_population_from_fame_484d1e_threshold_bands() {
    // PE @ `0x484d1e`: fame+year cuts at 0x79e / 0x7d0 / 0x802.
    let mut city = City::new_blank("/tmp/sc2000-nat-fame.sc2", "NatFame").expect("blank");
    city.pe_w_year_founded = 1900;
    city.set_pe_w_local_fame_base(0);
    city.pe_scale_national_population_from_fame_484d1e();
    assert_eq!(city.pe_dw_national_population, 10_000);
    assert_eq!(city.pe_dw_national_value, 3_000);

    city.set_pe_w_local_fame_base(0);
    city.pe_w_year_founded = 0x79f; // just above 0x79e
    city.pe_scale_national_population_from_fame_484d1e();
    assert_eq!(city.pe_dw_national_population, 25_000);

    city.pe_w_year_founded = 0x7d1;
    city.pe_scale_national_population_from_fame_484d1e();
    assert_eq!(city.pe_dw_national_population, 60_000);

    city.pe_w_year_founded = 0x803;
    city.pe_scale_national_population_from_fame_484d1e();
    assert_eq!(city.pe_dw_national_population, 150_000);
    assert_eq!(city.pe_dw_national_value, 45_000);
}

#[test]
fn pe_fun_00484cd0_national_from_fame_sets_tax_trend_and_reseeds() {
    let mut city = City::new_blank("/tmp/sc2000-484cd0.sc2", "Fun484cd0").expect("blank");
    city.pe_w_year_founded = 1900;
    city.pe_w_city_difficulty = 2;
    city.set_pe_w_local_fame_base(0);
    let mut crt = Win95CrtRand::new(7);
    city.pe_fun_00484cd0_national_from_fame(&mut crt);
    assert_eq!(city.pe_dw_national_population, 10_000);
    assert_eq!(city.pe_w_national_tax, 3);
    assert_eq!(city.pe_w_national_trend, 1);
    // Dry corners → PE else arm: river=1 ocean=0.
    assert_eq!(city.pe_dw_city_has_ocean(), 0);
    assert_eq!(city.pe_dw_city_has_river(), 1);
    // Index 6 base 1920 + noise with founded 1900 stays nonzero.
    let rank6 = city.pe_w_neighbor_fame_rank(6).expect("rank");
    assert!(
        (1920..1940).contains(&rank6),
        "expected PE Airport band 1920..1940 got {rank6}"
    );
}

#[test]
fn pe_init_ocean_river_new_city_40e173_sets_river_not_ocean() {
    let mut city = City::new_blank("/tmp/sc2000-newcity-ocean.sc2", "NewOcean").expect("blank");
    city.set_pe_dw_city_has_ocean(1);
    city.set_pe_dw_city_has_river(0);
    city.pe_init_ocean_river_new_city_40e173();
    assert_eq!(city.pe_dw_city_has_ocean(), 0);
    assert_eq!(city.pe_dw_city_has_river(), 1);
    assert_eq!(city.misc_u32_at(MISC_CITY_HAS_OCEAN_OFFSET), Some(0));
    assert_eq!(city.misc_u32_at(MISC_CITY_HAS_RIVER_OFFSET), Some(1));
}

#[test]
fn pe_ocean_river_initialization_keeps_mac_coast_and_river_slots() {
    let mut city =
        City::new_blank("/tmp/sc2000-ocean-river-mac.sc2", "OceanRiverMac").expect("blank");
    city.kind = CityFileKind::MacCity;
    city.set_mac_state_misc_u32_at(MISC_CITY_HAS_OCEAN_OFFSET, 0xaabb_ccdd);
    city.set_mac_state_misc_u32_at(MISC_CITY_HAS_RIVER_OFFSET, 0x1122_3344);

    city.pe_init_ocean_river_new_city_40e173();

    assert_eq!(city.pe_dw_city_has_ocean(), 0);
    assert_eq!(city.pe_dw_city_has_river(), 1);
    assert_eq!(
        city.misc_u32_at(MISC_CITY_HAS_OCEAN_OFFSET),
        Some(0xaabb_ccdd)
    );
    assert_eq!(
        city.misc_u32_at(MISC_CITY_HAS_RIVER_OFFSET),
        Some(0x1122_3344)
    );
}

#[test]
fn pe_publish_ocean_river_flags_484cd0_corner_water_sets_ocean() {
    let mut city = City::new_blank("/tmp/sc2000-ocean-flags.sc2", "OceanFlags").expect("blank");
    city.pe_publish_ocean_river_flags_484cd0();
    assert_eq!(city.pe_dw_city_has_ocean(), 0);
    assert_eq!(city.pe_dw_city_has_river(), 1);

    city.set_tile_bits(0, 0, TileBits::new(0x04));
    city.set_tile_bits(0x7f, 0x7f, TileBits::new(0x04));
    city.pe_publish_ocean_river_flags_484cd0();
    assert_eq!(city.pe_dw_city_has_ocean(), 1);
    assert_eq!(city.pe_dw_city_has_river(), 0);
}

#[test]
fn pe_ocean_river_bss_hydrates_and_persists_misc_0e44_0e48() {
    let mut city = City::new_blank("/tmp/sc2000-ocean-misc.sc2", "OceanMisc").expect("blank");
    city.set_misc_u32_at(MISC_CITY_HAS_OCEAN_OFFSET, 1);
    city.set_misc_u32_at(MISC_CITY_HAS_RIVER_OFFSET, 0);
    assert_eq!(city.pe_dw_city_has_ocean(), 1);
    assert_eq!(city.pe_dw_city_has_river(), 0);

    city.set_pe_dw_city_has_ocean(0);
    city.set_pe_dw_city_has_river(1);
    city.persist_pe_dw_city_has_ocean_to_misc_433ed8();
    city.persist_pe_dw_city_has_river_to_misc_433eec();
    assert_eq!(city.misc_u32_at(MISC_CITY_HAS_OCEAN_OFFSET), Some(0));
    assert_eq!(city.misc_u32_at(MISC_CITY_HAS_RIVER_OFFSET), Some(1));
}

#[test]
fn technology_discovery_years_map_to_original_misc_offsets() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");

    for (index, technology) in Technology::ALL.into_iter().enumerate() {
        city.set_technology_discovery_year(technology, 1900 + index as u32);
    }

    let misc = decode_misc_bytes(&city.chunks).expect("MISC chunk");
    for (index, technology) in Technology::ALL.into_iter().enumerate() {
        let offset = 0x0738 + index * MISC_FIELD_LEN;
        let expected = 1900 + index as u32;
        assert_eq!(technology.misc_offset(), offset, "{technology:?}");
        assert_eq!(
            u32::from_be_bytes(misc[offset..offset + MISC_FIELD_LEN].try_into().unwrap()),
            expected,
            "{technology:?}"
        );
        assert_eq!(
            city.technology_discovery_year(technology),
            Some(expected),
            "{technology:?}"
        );
    }
}

#[test]
fn default_new_city_technology_schedule_matches_original_template() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");

    city.apply_default_new_city_technology_schedule();

    for (technology, year) in DEFAULT_NEW_CITY_TECHNOLOGY_DISCOVERY_YEARS {
        assert_eq!(city.technology_discovery_year(technology), Some(year));
    }
}

#[test]
fn mac_new_city_technology_dates_use_quickdraw_and_round_trip_on_both_layouts() {
    for kind in [CityFileKind::WinSc2, CityFileKind::MacCity] {
        let suffix = if kind == CityFileKind::MacCity {
            "city"
        } else {
            "sc2"
        };
        let path = std::env::temp_dir().join(format!(
            "citygame-mac-tech-dates-{}-{suffix}.{suffix}",
            std::process::id()
        ));
        let mut city = City::new_blank(&path, "MacTechDates").expect("blank city");
        city.kind = kind;
        let mut rng = MacToolboxRng::new(0x4321);
        let mut expected_rng = rng;
        let mut expected = [0u16; PE_NEIGHBOR_FAME_RANK_COUNT];
        for (index, base_year) in TECHNOLOGY_BASE_DISCOVERY_YEARS.iter().copied().enumerate() {
            let year = base_year + expected_rng.random_u16() % 20;
            expected[index] = if year < 1950 { 0 } else { year };
        }

        city.mac_reseed_technology_dates_00041564(1950, &mut rng);
        assert_eq!(rng.seed(), expected_rng.seed());
        for (index, technology) in Technology::ALL.into_iter().enumerate() {
            assert_eq!(
                city.mac_state_misc_u32_at(technology.misc_offset()),
                Some(u32::from(expected[index])),
                "saved Mac technology date {index} ({kind:?})"
            );
            if kind == CityFileKind::WinSc2 {
                assert_eq!(city.pe_w_neighbor_fame_rank(index), Some(expected[index]));
            }
        }

        city.save(&path).expect("save scheduled city");
        let loaded = City::load(&path).expect("reload scheduled city");
        for (index, technology) in Technology::ALL.into_iter().enumerate() {
            assert_eq!(
                loaded.mac_state_misc_u32_at(technology.misc_offset()),
                Some(u32::from(expected[index])),
                "reloaded Mac technology date {index} ({kind:?})"
            );
        }
        let _ = std::fs::remove_file(path);
    }
}

#[test]
fn city_funds_round_trip_through_misc_chunk() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_city_funds(20_000);
    city.debit_city_funds(4_000);
    let path = std::env::temp_dir().join("sc2000_rust_city_funds_test.sc2");

    city.save(&path).expect("save city funds");
    let loaded = City::load(&path).expect("reload city funds");

    assert_eq!(loaded.city_funds(), 16_000);
    assert_eq!(loaded.misc_i32_at(0x0014), Some(16_000));
    let _ = fs::remove_file(path);
}

#[test]
fn decodes_reward_availability_mask() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_misc_u32_at(MISC_REWARDS_AVAILABLE_OFFSET, 0xffff_fff5);

    assert_eq!(city.pe_dw_rewards_available, 0xffff_fff5);
    assert_eq!(city.rewards_available_mask(), 0x15);
}

#[test]
fn setting_reward_availability_preserves_other_misc_bits() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_misc_u32_at(MISC_REWARDS_AVAILABLE_OFFSET, 0xabc0_001f);

    city.set_rewards_available_mask(0b0_0101);

    assert_eq!(city.rewards_available_mask(), 0b0_0101);
    assert_eq!(city.pe_dw_rewards_available, 0xabc0_0005);
    assert_eq!(
        city.misc_u32_at(MISC_REWARDS_AVAILABLE_OFFSET),
        Some(0xabc0_0005)
    );
    assert_eq!(
        city.chunks
            .iter()
            .filter(|chunk| chunk.name == "MISC")
            .count(),
        1
    );
}

#[test]
fn reward_availability_round_trips_through_save() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_rewards_available_mask(0b1_0110);
    let path = std::env::temp_dir().join("sc2000_rust_reward_mask_test.sc2");

    city.save(&path).expect("save reward mask");
    let loaded = City::load(&path).expect("reload reward mask");

    assert_eq!(loaded.rewards_available_mask(), 0b1_0110);
    let _ = fs::remove_file(path);
}

#[test]
fn disaster_markers_control_active_disaster_state() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    assert!(!city.has_active_disaster());

    city.set_text_overlay(12, 34, 0xff);
    assert!(city.has_active_disaster());

    city.set_text_overlay(12, 34, 0);
    assert!(!city.has_active_disaster());
}

#[test]
fn pe_xthg_disaster_actor_paints_slot_proxy_and_counts_as_active() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    assert!(!city.has_active_disaster());

    let slot = city
        .place_pe_tornado_actor(12, 34, 0, 0, 0, 0, 0)
        .expect("tornado slot");
    assert_eq!(city.text_overlay_at(12, 34), xthg_proxy_for_slot(slot));
    assert!(city.has_active_disaster());

    assert_eq!(city.clear_pe_tornado_actors(), 1);
    assert_eq!(city.text_overlay_at(12, 34), Some(0));
    assert!(!city.has_active_disaster());
}

#[test]
fn pe_explosion_counts_as_active_only_while_disaster_type_is_current() {
    let mut city = City::new_blank("/tmp/sc2000-explosion-keepalive.sc2", "ExplKeepalive")
        .expect("blank city");
    let slot = city
        .place_pe_explosion_actor(12, 34, 0, 0, 0)
        .expect("explosion slot");
    assert!(
        !city.has_active_disaster(),
        "ordinary explosions do not activate a disaster"
    );

    city.set_pe_w_current_disaster_type(1);
    assert!(
        city.has_active_disaster(),
        "PE kind-6 actor keeps the current disaster alive"
    );

    city.traffic_overlays_mut().unwrap()[slot].kind = 0;
    assert!(!city.has_active_disaster());
}

#[test]
fn pe_disaster_bulk_clear_zeros_xtxt_not_restore_saved_a() {
    // PE tornado/monster/air-crash clear arms call `FUN_00401528` (zero XTXT).
    let mut city = City::new_blank("/tmp/sc2000-xtxt-clear.sc2", "test").expect("blank city");
    city.set_text_overlay(12, 34, 0x22);
    let slot = city
        .place_pe_tornado_actor(12, 34, 0, 0, 0, 0, 0x22)
        .expect("tornado");
    assert_eq!(city.traffic_overlays().unwrap()[slot].saved_a, 0x22);
    assert_eq!(city.text_overlay_at(12, 34), xthg_proxy_for_slot(slot));
    assert_eq!(city.clear_pe_tornado_actors(), 1);
    // Must zero, not restore the saved 0x22 prior label.
    assert_eq!(city.text_overlay_at(12, 34), Some(0));
    let rec = city.traffic_overlays().unwrap()[slot];
    assert_eq!(rec.kind, 0);
    assert_eq!(rec.saved_a, 0x22, "FUN_004507D0 keeps +A");
}

#[test]
fn pe_tornado_spawn_clears_proxy_band_xtxt_via_fun_00401528() {
    let mut city = City::new_blank("/tmp/sc2000-tornado-proxy.sc2", "test").expect("blank city");
    let mut table = empty_overlay_table();
    table[2] = Win95OverlayRecord {
        kind: XTHG_SAILBOAT,
        tile_x: 12,
        tile_y: 34,
        height: 1,
        saved_a: 0x11,
        ..Default::default()
    };
    city.set_traffic_overlays_for_test(table);
    let prior_proxy = xthg_proxy_for_slot(2).expect("slot 2 proxy");
    city.set_text_overlay(12, 34, prior_proxy);
    let slot = city
        .place_pe_tornado_actor(12, 34, 0, 0, 0, 0, prior_proxy)
        .expect("tornado");
    assert_ne!(
        slot, 2,
        "cleared prior slot stays free; tornado takes another"
    );
    assert_eq!(city.traffic_overlays().unwrap()[2].kind, 0);
    assert_eq!(city.traffic_overlays().unwrap()[2].height, 1);
    assert_eq!(city.traffic_overlays().unwrap()[slot].saved_a, 0);
    assert_eq!(city.text_overlay_at(12, 34), xthg_proxy_for_slot(slot));
}

#[test]
fn pe_aircrash_spawn_requires_zero_xtxt() {
    let mut city = City::new_blank("/tmp/sc2000-aircrash-xtxt.sc2", "test").expect("blank city");
    city.set_text_overlay(40, 40, 0x22);
    assert!(city.place_pe_aircrash_actor(40, 40).is_none());
    assert_eq!(city.text_overlay_at(40, 40), Some(0x22));
    city.set_text_overlay(40, 40, 0);
    let slot = city.place_pe_aircrash_actor(40, 40).expect("aircrash");
    assert_eq!(city.traffic_overlays().unwrap()[slot].saved_a, 0);
    assert_eq!(city.text_overlay_at(40, 40), xthg_proxy_for_slot(slot));
}

#[test]
fn place_ambient_train_refuses_when_w_active_trains_ge_5() {
    // PE SpawnTrain @ `0x453603`: `cmp word [0x4e99d8], 5` / `jl` else return 0.
    let mut city = City::new_blank("/tmp/sc2000-train-cap5.sc2", "TrainCap5").expect("blank");
    city.set_traffic_overlays_for_test(empty_overlay_table());
    for i in 0..5 {
        let x = 10 + i * 10;
        city.set_building(x, 10, 45);
        city.set_building(x + 1, 10, 45);
        assert!(
            city.place_ambient_port_actor(x, 10, XTHG_TRAIN_ENGINE, XTHG_DIRECTION_EAST),
            "seed engine {i}"
        );
    }
    assert_eq!(city.ambient_active_train_count(), 5);
    city.set_building(70, 10, 45);
    city.set_building(71, 10, 45);
    assert!(
        !city.place_ambient_train(70, 10, 2),
        "sixth SpawnTrain must refuse at wActiveTrains>=5"
    );
    assert_eq!(city.ambient_active_train_count(), 5);
}

#[test]
fn place_ambient_train_refuses_outside_pe_2_to_7c_bounds() {
    // PE SpawnTrain @ `0x45361b`–`0x453646`: x/y must be in `2..=0x7c`.
    let mut city = City::new_blank("/tmp/sc2000-train-bounds.sc2", "TrainBounds").expect("blank");
    city.set_traffic_overlays_for_test(empty_overlay_table());
    for &(x, y) in &[
        (0usize, 10),
        (1, 10),
        (0x7d, 10),
        (10, 0),
        (10, 1),
        (10, 0x7d),
    ] {
        city.set_building(x, y, 45);
        let next = (x.saturating_add(1).min(CITY_WIDTH - 1), y);
        city.set_building(next.0, next.1, 45);
        assert!(
            !city.place_ambient_train(x, y, 2),
            "SpawnTrain must refuse ({x},{y})"
        );
    }
    city.set_building(2, 2, 45);
    city.set_building(3, 2, 45);
    assert!(city.place_ambient_train(2, 2, 2), "2,2 is the PE minimum");
    city.set_building(0x7c, 0x7c, 45);
    city.set_building(0x7b, 0x7c, 45);
    // Direction west toward in-bounds rail for next-tile probe.
    assert!(
        city.place_ambient_train(0x7c, 0x7c, 6),
        "0x7c,0x7c is the PE maximum"
    );
}

#[test]
fn pe_sailboat_spawn_tile_face_skips_xbld_gate_like_pe_453380() {
    // PE marina scanner @ `0x4533fa`/`0x453407`: water + XTXT only; XBLD is
    // SpawnSailBoat (`0x453491`). A built water face is still selected so Place
    // can refuse and the next cardinal can win.
    let mut city = City::new_blank("/tmp/sc2000-marina-face.sc2", "MarinaFace").expect("blank");
    city.set_tile_bits(30, 29, TileBits::new(0x04));
    city.set_building(30, 29, 45); // rail on water face — PE still selects it
    city.set_text_overlay(30, 29, 0);
    city.set_tile_bits(31, 30, TileBits::new(0x04));
    city.set_building(31, 30, 0);
    city.set_text_overlay(31, 30, 0);
    // N face first in PE order; must return the built face, not skip to E.
    assert_eq!(
        city.pe_sailboat_spawn_tile_from_marina(30, 30),
        Some((30, 29))
    );
    assert!(
        !city.place_ambient_port_actor(30, 29, XTHG_SAILBOAT, 0),
        "SpawnSailBoat must refuse nonzero XBLD"
    );
    assert_eq!(city.pe_sailboat_budget, 0);
}

#[test]
fn pe_train_spawn_track_face_scanner_allows_0_to_7f_like_pe_453590() {
    // PE face scanner @ `0x453590` uses `0..=0x7f`; SpawnTrain bounds are separate.
    let mut city = City::new_blank("/tmp/sc2000-train-face.sc2", "TrainFace").expect("blank");
    // Station NW at (2,2); only north face (2,1) has rail — y=1 is outside SpawnTrain
    // bounds but inside the face scanner.
    stamp_complete_building_for_test_local(&mut city, 2, 2, 0xed, 2);
    city.set_building(2, 1, 45);
    city.set_text_overlay(2, 1, 0);
    assert_eq!(city.pe_train_spawn_track_from_station(2, 2), Some((2, 1)));
    assert!(
        !city.place_ambient_train(2, 1, 2),
        "SpawnTrain must still refuse y=1"
    );
}

fn stamp_complete_building_for_test_local(
    city: &mut City,
    x: usize,
    y: usize,
    building: u8,
    size: usize,
) {
    for dy in 0..size {
        for dx in 0..size {
            city.set_building(x + dx, y + dy, building);
        }
    }
}

#[test]
fn place_ambient_train_refuses_non_rail_xbld() {
    // PE SpawnTrain @ `0x453665`–`0x4536aa`: effective XBLD gate is `0x2c..=0x35`.
    let mut city = City::new_blank("/tmp/sc2000-train-xbld.sc2", "TrainXbld").expect("blank");
    city.set_traffic_overlays_for_test(empty_overlay_table());
    city.set_building(10, 10, 0); // empty
    city.set_building(11, 10, 45);
    assert!(!city.place_ambient_train(10, 10, 2));
    city.set_building(10, 10, 0x1d); // road
    assert!(!city.place_ambient_train(10, 10, 2));
    city.set_building(10, 10, 0x36); // past PE `(xbld-0x2c)<=9`
    assert!(!city.place_ambient_train(10, 10, 2));
    city.set_building(10, 10, 0x2c);
    assert!(city.place_ambient_train(10, 10, 2));
    assert_eq!(city.ambient_active_train_count(), 1);
}

#[test]
fn place_ambient_train_matches_native_slot_zero_allocator_failure() {
    // PE `FUN_00453600` accepts the consist after three unchecked allocations.
    // The allocator returns slot zero when all usable slots 1–39 are occupied;
    // the constructor writes through it, stamps `slot+0xc9`, and increments the
    // active-train count even when no usable engine record survives.
    for free_usable_slots in 0..=2 {
        let mut city = City::new_blank(
            format!("/tmp/sc2000-train-slot-zero-{free_usable_slots}.sc2"),
            "TrainSlotZero",
        )
        .expect("blank");
        let mut table = empty_overlay_table();
        table[0].kind = 0x55; // the reserved slot is overwritten by native failure writes
        for slot in XTHG_FIRST_USABLE_SLOT..XTHG_RECORD_COUNT {
            table[slot].kind = 1;
        }
        for slot in XTHG_FIRST_USABLE_SLOT..XTHG_FIRST_USABLE_SLOT + free_usable_slots {
            table[slot].kind = 0;
        }
        city.set_traffic_overlays_for_test(table);
        city.set_building(10, 10, 45);
        city.set_building(11, 10, 45);

        assert!(city.place_ambient_train(10, 10, 2));
        assert_eq!(city.ambient_active_train_count(), 1);
        assert_eq!(
            city.text_overlay_at(10, 10),
            Some(XTHG_PROXY_BASE + if free_usable_slots == 0 { 0 } else { 1 })
        );
        let overlays = city.traffic_overlays().unwrap();
        assert_eq!(overlays[0].kind, XTHG_TRAIN_CAR);
        assert_eq!(
            overlays[0].saved_b,
            if free_usable_slots == 2 { 2 } else { 0 }
        );
        assert_eq!(city.traffic_overlays_free_slots(), 0);
        if free_usable_slots > 0 {
            assert_eq!(overlays[1].kind, XTHG_TRAIN_ENGINE);
            assert_eq!(
                overlays[1].state,
                if free_usable_slots == 2 { 2 } else { 0 }
            );
        }

        // The native actor sweep rebuilds wActiveTrains from usable records,
        // dropping the slot-zero-only phantom count after its next pass.
        city.clear_pe_active_train_count_override();
        assert_eq!(
            city.ambient_active_train_count(),
            usize::from(free_usable_slots > 0)
        );
    }
}

#[test]
fn dispatch_actor_uses_xthg_slot_without_touching_xbld() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    stamp_complete_building(&mut city, 100, 100, 210, 3);

    assert!(city.place_dispatch_actor(12, 34, DispatchKind::Police));

    assert_eq!(city.building_at(12, 34), 0);
    let (slot, record) = city.dispatch_actor_at(12, 34).expect("police actor");
    assert_eq!(slot, 1);
    assert_eq!(record.kind, DispatchKind::Police.overlay_kind());
    assert_eq!(record.state, 0);
    assert_eq!((record.tile_x, record.tile_y), (12, 34));
    // Kind 7 is a surface dispatch actor; it has no nonzero height entry
    // in the PE XTHG spawn-height table (unlike planes/heli/ships).
    assert_eq!(record.height, xthg_spawn_height(record.kind));
    assert_eq!(record.saved_a, 0);
    assert_eq!(record.saved_b, RUST_DISPATCH_ACTOR_SENTINEL);
    assert_eq!(city.traffic_overlays_free_slot(), Some(0));
    assert_eq!(
        city.traffic_overlays_free_slots(),
        crate::win95_overlay::XTHG_RECORD_COUNT - 1
    );
}

#[test]
fn dispatch_uses_all_available_units_then_relocates_the_oldest() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    stamp_complete_building(&mut city, 100, 100, 211, 3);
    stamp_complete_building(&mut city, 110, 100, 211, 3);

    assert!(city.place_dispatch_actor(10, 20, DispatchKind::Firefighters));
    assert!(city.place_dispatch_actor(30, 40, DispatchKind::Firefighters));
    assert!(city.dispatch_actor_at(10, 20).is_some());
    assert_eq!(city.dispatch_actor_at(30, 40).unwrap().0, 2);

    assert!(city.place_dispatch_actor(50, 60, DispatchKind::Firefighters));

    assert_eq!(city.building_at(10, 20), 0);
    assert!(city.dispatch_actor_at(10, 20).is_none());
    assert!(city.dispatch_actor_at(30, 40).is_some());
    assert_eq!(city.dispatch_actor_at(50, 60).unwrap().0, 1);
}

#[test]
fn stale_dispatch_records_do_not_make_real_buildings_into_actors() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    stamp_complete_building(&mut city, 100, 100, 210, 3);
    let mut table = empty_overlay_table();
    table[1] = Win95OverlayRecord {
        kind: DispatchKind::Police.overlay_kind(),
        tile_x: 10,
        tile_y: 20,
        saved_b: 0,
        ..Default::default()
    };
    city.set_traffic_overlays_for_test(table);
    city.set_building(10, 20, 202);

    assert!(city.dispatch_actor_at(10, 20).is_none());

    assert!(city.place_dispatch_actor(30, 40, DispatchKind::Police));
    assert_eq!(city.building_at(10, 20), 0);
    assert_eq!(city.building_at(30, 40), 0);
    let (slot, record) = city.dispatch_actor_at(30, 40).expect("police actor");
    assert_eq!(slot, 1);
    assert!(record.is_rust_dispatch_actor());
}

#[test]
fn stale_rust_dispatch_records_do_not_override_real_buildings() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut table = empty_overlay_table();
    table[1] = Win95OverlayRecord {
        kind: DispatchKind::Firefighters.overlay_kind(),
        tile_x: 10,
        tile_y: 20,
        saved_b: RUST_DISPATCH_ACTOR_SENTINEL,
        ..Default::default()
    };
    city.set_traffic_overlays_for_test(table);
    city.set_building(10, 20, 202);

    assert!(city.dispatch_actor_at(10, 20).is_none());
}

#[test]
fn dispatch_uses_both_editions_xbit_gate_and_allows_xbld_tiles() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    stamp_complete_building(&mut city, 100, 100, 210, 3);
    city.set_terrain(4, 5, 0x30);
    city.set_tile_bits(4, 5, TileBits::new(0));
    city.set_building(6, 7, 112);
    city.set_tile_bits(8, 9, TileBits::new(0x04));

    // Both native input paths check XBIT bit 2, not an XTER water classification.
    assert!(city.can_place_dispatch_actor(4, 5, DispatchKind::Police));
    assert!(city.can_place_dispatch_actor_for_engine(
        4,
        5,
        DispatchKind::Police,
        EngineFidelity::Mac68k
    ));
    // XBLD is not an occupancy gate; XTXT is. A dispatch marker may overlay a built tile.
    assert!(city.can_place_dispatch_actor(6, 7, DispatchKind::Police));
    assert!(city.can_place_dispatch_actor_for_engine(
        6,
        7,
        DispatchKind::Police,
        EngineFidelity::Mac68k
    ));
    assert!(!city.can_place_dispatch_actor_for_engine(
        8,
        9,
        DispatchKind::Police,
        EngineFidelity::Mac68k
    ));
    assert!(city.place_dispatch_actor_for_engine(
        6,
        7,
        DispatchKind::Police,
        EngineFidelity::Mac68k
    ));
    assert_eq!(city.building_at(6, 7), 112);
    assert!(city.dispatch_actor_at(6, 7).is_some());
    assert!(!city.can_place_dispatch_actor(8, 9, DispatchKind::Police));
}

#[test]
fn pe_dispatch_spawn_requires_zero_xtxt() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    stamp_complete_building(&mut city, 100, 100, 210, 3);
    city.set_text_overlay(20, 21, 0x40);
    assert!(!city.can_place_dispatch_actor(20, 21, DispatchKind::Police));
    assert!(!city.place_dispatch_actor(20, 21, DispatchKind::Police));
    assert_eq!(city.text_overlay_at(20, 21), Some(0x40));
    assert!(city.dispatch_actor_at(20, 21).is_none());
    assert!(!city.can_place_dispatch_actor_for_engine(
        20,
        21,
        DispatchKind::Police,
        EngineFidelity::Mac68k
    ));
}

#[test]
fn dispatch_rejects_tiles_with_existing_dispatch_actor() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    stamp_complete_building(&mut city, 100, 100, 210, 3);
    stamp_complete_building(&mut city, 110, 100, 211, 3);

    assert!(city.place_dispatch_actor(20, 21, DispatchKind::Police));

    assert!(!city.can_place_dispatch_actor(20, 21, DispatchKind::Firefighters));
    assert!(!city.place_dispatch_actor(20, 21, DispatchKind::Firefighters));
    assert_eq!(
        city.dispatch_actor_at(20, 21)
            .and_then(|(_, record)| record.dispatch_kind()),
        Some(DispatchKind::Police)
    );
}

#[test]
fn clear_legacy_dispatch_actor_proxies_restores_real_xbld_tiles() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut table = empty_overlay_table();
    table[1] = Win95OverlayRecord {
        kind: DispatchKind::Police.overlay_kind(),
        tile_x: 12,
        tile_y: 34,
        saved_a: 0,
        saved_b: 0,
        ..Default::default()
    };
    table[2] = Win95OverlayRecord {
        kind: DispatchKind::Firefighters.overlay_kind(),
        tile_x: 56,
        tile_y: 78,
        saved_a: 112,
        saved_b: 0,
        ..Default::default()
    };
    city.set_traffic_overlays_for_test(table);
    city.set_building(12, 34, 202);
    city.set_building(56, 78, 203);

    assert_eq!(city.clear_legacy_dispatch_actor_proxies(), 2);

    let cleared1 = city.traffic_overlays().unwrap()[1];
    let cleared2 = city.traffic_overlays().unwrap()[2];
    assert_eq!(cleared1.kind, 0);
    assert_eq!(cleared2.kind, 0);
    assert_eq!((cleared1.tile_x, cleared1.tile_y), (12, 34));
    assert_eq!((cleared2.tile_x, cleared2.tile_y), (56, 78));
    assert_eq!(cleared2.saved_a, 112);
    assert_eq!(city.building_at(12, 34), 0);
    assert_eq!(city.building_at(56, 78), 112);
}

#[test]
fn removing_dispatch_clears_record_without_touching_xbld() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    assert!(city.place_dispatch_actor(8, 9, DispatchKind::Military));
    let before = city.traffic_overlays().unwrap()[1];
    assert_ne!(before.kind, 0);
    let height = before.height;
    let saved_b = before.saved_b;

    assert!(city.remove_dispatch_actor_at(8, 9));
    assert_eq!(city.building_at(8, 9), 0);
    assert!(city.dispatch_actor_at(8, 9).is_none());
    let cleared = city.traffic_overlays().unwrap()[1];
    assert_eq!(cleared.kind, 0);
    assert_eq!(cleared.height, height);
    assert_eq!(cleared.saved_b, saved_b);
    assert_eq!((cleared.tile_x, cleared.tile_y), (8, 9));
}

#[test]
fn dispatch_actor_round_trips_through_save() {
    let path = std::env::temp_dir().join("sc2000_rust_dispatch_actor_test.sc2");
    let mut city = City::new_blank(&path, "test").expect("blank city");
    stamp_complete_building(&mut city, 100, 100, 210, 3);
    assert!(city.place_dispatch_actor(22, 44, DispatchKind::Police));

    city.save(&path).expect("save dispatch actor");
    let loaded = City::load(&path).expect("reload dispatch actor");

    let (_, record) = loaded.dispatch_actor_at(22, 44).expect("saved actor");
    assert_eq!(record.kind, DispatchKind::Police.overlay_kind());
    assert_eq!(record.saved_b, RUST_DISPATCH_ACTOR_SENTINEL);
    assert_eq!(loaded.building_at(22, 44), 0);
    let _ = fs::remove_file(path);
}

#[test]
fn dispatch_capacity_matches_stations_and_military_base_type() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    stamp_complete_building(&mut city, 10, 10, 210, 3);
    stamp_complete_building(&mut city, 20, 10, 210, 3);
    stamp_complete_building(&mut city, 10, 20, 211, 3);
    city.set_building(30, 30, 211);
    set_military_base_type(&mut city, 2);
    assert_eq!(city.dispatch_capacity(DispatchKind::Police), 2);
    assert_eq!(city.dispatch_capacity(DispatchKind::Firefighters), 1);
    assert_eq!(city.dispatch_capacity(DispatchKind::Military), 5);

    set_military_base_type(&mut city, 3);
    assert_eq!(city.dispatch_capacity(DispatchKind::Military), 2);
    set_military_base_type(&mut city, 4);
    assert_eq!(city.dispatch_capacity(DispatchKind::Military), 3);
    set_military_base_type(&mut city, 5);
    assert_eq!(city.dispatch_capacity(DispatchKind::Military), 0);
}

#[test]
fn military_selector_uses_shared_saved_byte_across_engines_and_layouts() {
    for (kind, extension) in [
        (CityFileKind::WinSc2, "sc2"),
        (CityFileKind::MacCity, "city"),
    ] {
        let path = std::env::temp_dir().join(format!(
            "citygame-military-selector-{}.{}",
            std::process::id(),
            extension
        ));
        let mut city = City::new_blank(&path, "selector").expect("blank city");
        city.kind = kind;
        city.set_mac_state_misc_u32_at(MISC_MILITARY_BASE_TYPE_OFFSET, 0xfeed_be02);
        city.save(&path).expect("save selector");
        let mut city = City::load(&path).expect("load selector");
        assert_eq!(city.kind, kind);
        city.prepare_pe_fidelity_from_saved_city();
        for engine in [EngineFidelity::Win95, EngineFidelity::Mac68k] {
            assert_eq!(
                city.dispatch_capacity_for_engine(DispatchKind::Military, engine),
                5
            );
        }
        city.set_pe_military_base_type(0x103);
        assert_eq!(
            city.mac_state_misc_u32_at(MISC_MILITARY_BASE_TYPE_OFFSET),
            Some(3)
        );
        city.save(&path).expect("save updated selector");
        let mut loaded = City::load(&path).expect("reload updated selector");
        loaded.prepare_pe_fidelity_from_saved_city();
        for engine in [EngineFidelity::Win95, EngineFidelity::Mac68k] {
            assert_eq!(
                loaded.dispatch_capacity_for_engine(DispatchKind::Military, engine),
                2
            );
        }
        loaded.set_mac_state_misc_u32_at(MISC_MILITARY_BASE_TYPE_OFFSET, 0xffff_ff04);
        assert_eq!(
            loaded.dispatch_capacity_for_engine(DispatchKind::Military, EngineFidelity::Win95),
            3
        );
        let _ = fs::remove_file(path);
    }
}

#[test]
fn dispatch_station_capacity_uses_edition_tile_census_shifted_by_three() {
    for (tile_count, capacity) in [(7, 0), (8, 1), (15, 1), (16, 2)] {
        for file_kind in [CityFileKind::WinSc2, CityFileKind::MacCity] {
            let mut city = City::new_blank("/tmp/sc2000-dispatch-tile-census.sc2", "test")
                .expect("blank city");
            city.kind = file_kind;
            for building in [210u8, 211] {
                city.pe_dw_tile_count[usize::from(building)] = tile_count;
                city.set_mac_state_misc_u32_at(
                    MAC_XBLD_CENSUS_MISC_OFFSET + usize::from(building) * 4,
                    u32::from(tile_count),
                );
            }

            for fidelity in [EngineFidelity::Win95, EngineFidelity::Mac68k] {
                assert_eq!(
                    city.dispatch_capacity_for_engine(DispatchKind::Police, fidelity),
                    capacity,
                    "police: {tile_count} tiles, {file_kind:?}, {fidelity:?}"
                );
                assert_eq!(
                    city.dispatch_capacity_for_engine(DispatchKind::Firefighters, fidelity),
                    capacity,
                    "fire: {tile_count} tiles, {file_kind:?}, {fidelity:?}"
                );
            }
        }
    }

    // When a Mac file's saved census disagrees with the current PE census,
    // select by simulation fidelity rather than by city-file format.
    let mut city =
        City::new_blank("/tmp/sc2000-dispatch-edition-census.sc2", "test").expect("blank city");
    city.kind = CityFileKind::MacCity;
    city.pe_dw_tile_count[210] = 8;
    city.pe_dw_tile_count[211] = 15;
    city.set_mac_state_misc_u32_at(MAC_XBLD_CENSUS_MISC_OFFSET + 210 * 4, 7);
    city.set_mac_state_misc_u32_at(MAC_XBLD_CENSUS_MISC_OFFSET + 211 * 4, 16);

    assert_eq!(
        city.dispatch_capacity_for_engine(DispatchKind::Police, EngineFidelity::Win95),
        1
    );
    assert_eq!(
        city.dispatch_capacity_for_engine(DispatchKind::Firefighters, EngineFidelity::Win95),
        1
    );
    assert_eq!(
        city.dispatch_capacity_for_engine(DispatchKind::Police, EngineFidelity::Mac68k),
        0
    );
    assert_eq!(
        city.dispatch_capacity_for_engine(DispatchKind::Firefighters, EngineFidelity::Mac68k),
        2
    );

    // The PE source has no native clamp above 31; its one-based 32-byte
    // coordinate ring would overrun at capacity 32. CityGame keeps the
    // existing bounded port policy for this oversized-city edge.
    city.pe_dw_tile_count[210] = 256;
    assert_eq!(
        city.dispatch_capacity_for_engine(DispatchKind::Police, EngineFidelity::Win95),
        31
    );
    assert_eq!(dispatch_capacity_from_station_tile_count(0x8000), 0);
}

#[test]
fn win95_stadium_mask_on_mac_layout_hydrates_updates_and_round_trips() {
    let path = std::env::temp_dir().join(format!(
        "citygame-pe-mac-stadium-{}.city",
        std::process::id()
    ));
    let mut fixture = City::new_blank(&path, "Stadium").unwrap();
    fixture.kind = CityFileKind::MacCity;
    fixture.set_mac_state_misc_u32_at(MISC_SPORTS_TEAMS_OFFSET, 3);
    let xtxt = fixture.provision_microsim_for_building(0xd7).unwrap();
    fixture.save(&path).unwrap();
    let mut city = City::load(&path).unwrap();
    city.prepare_pe_fidelity_from_saved_city();
    assert_eq!(city.pe_w_sports_teams(), 3);
    let mut crt = Win95CrtRand::new(1);
    assert!(city.pe_newspaper_stadium_sports_reward_4837a5(xtxt, &mut crt));
    assert_eq!(city.pe_w_sports_teams(), 7);
    assert_eq!(city.misc_u32_at(MISC_SPORTS_TEAMS_OFFSET), Some(7));
    city.save(&path).unwrap();
    let mut city = City::load(&path).unwrap();
    city.prepare_pe_fidelity_from_saved_city();
    assert_eq!(city.pe_w_sports_teams(), 7);
    city.pe_destroy_building_clear_sports_team_bit_459fb7(xtxt);
    assert_eq!(city.pe_w_sports_teams(), 3);
    assert_eq!(city.misc_u32_at(MISC_SPORTS_TEAMS_OFFSET), Some(3));
    city.pe_or_w_sports_teams_bit_47ce74(4);
    assert_eq!(city.misc_u32_at(MISC_SPORTS_TEAMS_OFFSET), Some(19));
    city.set_pe_w_sports_teams(0x8002);
    assert_eq!(
        city.misc_u32_at(MISC_SPORTS_TEAMS_OFFSET),
        Some(0xffff_8002)
    );
    city.save(&path).unwrap();
    let mut loaded = City::load(&path).unwrap();
    loaded.prepare_pe_fidelity_from_saved_city();
    assert_eq!(loaded.pe_w_sports_teams(), 0x8002);
    let _ = std::fs::remove_file(path);
}

#[test]
fn mac_stadium_mask_update_on_win_layout_survives_pe_save_mirror() {
    let path = std::env::temp_dir().join(format!(
        "citygame-mac-win-stadium-{}.sc2",
        std::process::id()
    ));
    let mut city = City::new_blank(&path, "Stadium").unwrap();
    city.prepare_mac_fidelity_from_saved_city();
    city.set_mac_state_misc_u32_at(MISC_SPORTS_TEAMS_OFFSET, 0xffff_8003);
    assert_eq!(city.pe_w_sports_teams(), 0x8003);
    city.save(&path).unwrap();
    let loaded = City::load(&path).unwrap();
    assert_eq!(loaded.pe_w_sports_teams(), 0x8003);
    assert_eq!(
        loaded.misc_u32_at(MISC_SPORTS_TEAMS_OFFSET),
        Some(0xffff_8003)
    );
    let _ = std::fs::remove_file(path);
}

#[test]
fn pe_stadium_xmic_word_round_trips_and_uses_x86_shift_counts() {
    for (extension, kind) in [
        ("city", CityFileKind::MacCity),
        ("sc2", CityFileKind::WinSc2),
    ] {
        let path = std::env::temp_dir().join(format!(
            "citygame-stadium-word-{}.{}",
            std::process::id(),
            extension
        ));
        let mut city = City::new_blank(&path, "Stadium").unwrap();
        city.kind = kind;
        let xtxt = city.provision_microsim_for_building(0xd7).unwrap();
        let slot = usize::from(xtxt - MIN_SIM_TEXT_ENTRIES);
        assert!(city.pe_write_sports_team_index_to_microsim_483816(xtxt, 3));
        assert_eq!(city.microsim_at(slot).unwrap().to_bytes()[4..6], [0, 3]);
        city.save(&path).unwrap();
        let mut loaded = City::load(&path).unwrap();
        assert_eq!(loaded.microsim_at(slot).unwrap().stat2, 3);
        for (index, expected) in [(2, 0x1b), (16, 0x1f), (32, 0x1e), (33, 0x1d)] {
            loaded.set_microsim_stat2(slot, index);
            loaded.set_pe_w_sports_teams(0x1f);
            loaded.pe_destroy_building_clear_sports_team_bit_459fb7(xtxt);
            assert_eq!(
                loaded.pe_w_sports_teams(),
                expected,
                "{extension} shift {index}"
            );
        }
        let _ = std::fs::remove_file(path);
    }
}

#[test]
fn pe_military_census_load_uses_low_word_of_each_saved_dword() {
    let path = std::env::temp_dir().join(format!(
        "citygame-military-words-{}.sc2",
        std::process::id()
    ));
    let mut city = City::new_blank(&path, "Military").unwrap();
    for index in 0..MILITARY_TILE_COUNT_VALUES {
        let raw = 0x7abc_8000 + index as u32;
        city.set_misc_u32_at(MISC_MILITARY_TILE_COUNTS_OFFSET + index * 4, raw);
        assert_eq!(city.pe_military_tile_count[index], raw as u16);
    }
    city.save(&path).unwrap();
    let loaded = City::load(&path).unwrap();
    for index in 0..MILITARY_TILE_COUNT_VALUES {
        assert_eq!(loaded.pe_military_tile_count[index], 0x8000 + index as u16);
    }
    let _ = std::fs::remove_file(path);
}

#[test]
fn mac_dispatch_capacity_uses_reconstructed_selector_table() {
    let mut city =
        City::new_blank("/tmp/sc2000-mac-dispatch-capacity.sc2", "test").expect("blank city");
    city.kind = CityFileKind::MacCity;
    stamp_complete_building(&mut city, 10, 10, 210, 3);

    // Selector 6 reads beyond the initialized native local table; keep
    // CityGame's out-of-range lookup deterministic rather than model stack residue.
    for (selector, capacity) in [(0, 0), (1, 0), (2, 5), (3, 2), (4, 3), (5, 0), (6, 0)] {
        city.set_mac_state_misc_u32_at(MAC_DISPATCH_SELECTOR_MISC_OFFSET, selector);
        assert_eq!(
            city.dispatch_capacity_for_engine(DispatchKind::Military, EngineFidelity::Mac68k),
            capacity,
            "Mac selector {selector}"
        );
    }

    // The native all-services-empty fallback remains one deployable military actor.
    // Remove all station tiles: a damaged 8-tile footprint still has one police slot.
    for y in 10..13 {
        for x in 10..13 {
            city.set_building(x, y, 0);
        }
    }
    city.set_mac_state_misc_u32_at(MAC_DISPATCH_SELECTOR_MISC_OFFSET, 5);
    assert_eq!(
        city.dispatch_capacity_for_engine(DispatchKind::Military, EngineFidelity::Mac68k),
        1
    );
}

#[test]
fn dispatch_setup_retains_original_military_fallback() {
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");

    assert_eq!(city.dispatch_capacity(DispatchKind::Police), 0);
    assert_eq!(city.dispatch_capacity(DispatchKind::Firefighters), 0);
    assert_eq!(city.dispatch_capacity(DispatchKind::Military), 1);
}

#[test]
fn emergency_reset_removes_all_dispatch_records_and_proxies() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    stamp_complete_building(&mut city, 100, 100, 210, 3);
    stamp_complete_building(&mut city, 110, 100, 211, 3);
    set_military_base_type(&mut city, 2);
    assert!(city.place_dispatch_actor(10, 20, DispatchKind::Police));
    assert!(city.place_dispatch_actor(30, 40, DispatchKind::Firefighters));
    assert!(city.place_dispatch_actor(50, 60, DispatchKind::Military));

    city.reset_emergency_dispatch();

    for (x, y) in [(10, 20), (30, 40), (50, 60)] {
        assert_eq!(city.building_at(x, y), 0);
        assert!(city.dispatch_actor_at(x, y).is_none());
    }
    assert!(
        city.traffic_overlays()
            .unwrap()
            .iter()
            .all(|record| !record.is_dispatch_actor())
    );
}

#[test]
fn decodes_city_date_from_misc_age() {
    let mut misc = vec![0u8; 0x14];
    misc[0x0c..0x10].copy_from_slice(&1900u32.to_be_bytes());
    misc[0x10..0x14].copy_from_slice(&53488u32.to_be_bytes());
    let date = decode_city_date(
        &[CityChunk {
            name: "MISC".to_owned(),
            offset: 0,
            raw_len: misc.len(),
            data: misc,
        }],
        CityFileKind::WinSc2,
    )
    .expect("date decode");

    assert_eq!(
        date,
        CityDate {
            year: 2078,
            month: 4
        }
    );
}

#[test]
fn decodes_mac_city_date_from_persisted_dispatch_phases() {
    let mut misc = vec![0u8; 0x14];
    // The Mac loader reads the low WORD; unrelated high-word data must not
    // be treated as part of the founding year.
    misc[0x0c..0x10].copy_from_slice(&0xcafe_07d0u32.to_be_bytes());
    // Two elapsed years, four elapsed months, and seven phases into the
    // current month. Mac calendar mapping is 300 phases/year, 25/month.
    misc[0x10..0x14].copy_from_slice(&(2 * 300 + 4 * 25 + 7_u32).to_be_bytes());
    let date = decode_city_date(
        &[CityChunk {
            name: "MISC".to_owned(),
            offset: 0,
            raw_len: misc.len(),
            data: misc,
        }],
        CityFileKind::MacCity,
    )
    .expect("Mac date decode");

    assert_eq!(
        date,
        CityDate {
            year: 2002,
            month: 5
        }
    );
}

#[test]
fn setting_city_date_writes_age_and_round_trips_through_misc() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_saved_date(CityDate {
        year: 1901,
        month: 3,
    });
    let path = std::env::temp_dir().join("sc2000_rust_city_date_test.sc2");

    city.save(&path).expect("save city date");
    let loaded = City::load(&path).expect("reload city date");

    assert_eq!(
        loaded.saved_date(),
        Some(CityDate {
            year: 1901,
            month: 3
        })
    );
    assert_eq!(loaded.misc_u32_at(0x0c), Some(1900));
    assert_eq!(loaded.misc_u32_at(0x10), Some(350));
    assert_eq!(loaded.city_age_days(), Some(350));
    let _ = fs::remove_file(path);
}

#[test]
fn setting_mac_city_date_uses_native_phase_calendar_and_round_trips() {
    let path =
        std::env::temp_dir().join(format!("sc2000_mac_city_date_{}.mac", std::process::id()));
    let mut city = City::new_blank(&path, "MacDate").expect("blank city");
    city.kind = CityFileKind::MacCity;
    // CODE 4 loads the low WORD, then its saveback sign-extends that word
    // into the full dword. Seed an upper-word sentinel to check both sides.
    let founded_slot_with_sentinel = 0xcafe_0000 | 2000;
    city.set_mac_state_misc_u32_at(MISC_YEAR_FOUNDED_OFFSET, founded_slot_with_sentinel);
    assert_eq!(city.year_founded(), Some(2000));
    city.set_mac_state_misc_u32_at(
        MISC_CITY_AGE_OFFSET,
        2 * MAC_CALENDAR_PHASES_PER_YEAR + 2 * MAC_CALENDAR_PHASES_PER_MONTH + 13,
    );

    city.set_saved_date_and_day(
        CityDate {
            year: 2003,
            month: 5,
        },
        7,
    );
    let expected_counter = 3 * MAC_CALENDAR_PHASES_PER_YEAR + 4 * MAC_CALENDAR_PHASES_PER_MONTH + 7;
    assert_eq!(
        city.mac_state_misc_u32_at(MISC_CITY_AGE_OFFSET),
        Some(expected_counter),
        "Mac date stores one native dispatcher tick per in-game day"
    );
    assert_eq!(
        city.saved_date(),
        Some(CityDate {
            year: 2003,
            month: 5
        })
    );
    assert_eq!(
        city.misc_u32_at(MISC_YEAR_FOUNDED_OFFSET),
        Some(2000),
        "Mac date update must sign-extend its low year WORD into the MISC dword"
    );

    city.save(&path).expect("save Mac city");
    let loaded = City::load(&path).expect("reload Mac city");
    assert_eq!(
        loaded.saved_date(),
        Some(CityDate {
            year: 2003,
            month: 5
        })
    );
    assert_eq!(
        loaded.mac_state_misc_u32_at(MISC_CITY_AGE_OFFSET),
        Some(expected_counter)
    );
    assert_eq!(loaded.year_founded(), Some(2000));
    assert_eq!(
        loaded.misc_u32_at(MISC_YEAR_FOUNDED_OFFSET),
        Some(2000),
        "Mac saveback must normalize the year slot from the native WORD"
    );
    assert_eq!(loaded.city_age_days(), Some(expected_counter));
    let _ = fs::remove_file(path);
}

#[test]
fn setting_city_date_with_sim_day_writes_age_remainder() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_saved_date_and_day(
        CityDate {
            year: 1901,
            month: 3,
        },
        13,
    );

    assert_eq!(city.city_age_days(), Some(363));
    assert_eq!(city.year_founded(), Some(1900));
    assert_eq!(
        city.saved_date(),
        Some(CityDate {
            year: 1901,
            month: 3
        })
    );
}

#[test]
fn decodes_win95_saved_camera_tile_and_zoom_from_misc() {
    let mut misc = vec![0u8; 0x1020];
    misc[0x1010..0x1014].copy_from_slice(&0x0000_4246u32.to_be_bytes());
    misc[0x1014..0x1018].copy_from_slice(&2u32.to_be_bytes());
    let view = decode_city_view(
        &[CityChunk {
            name: "MISC".to_owned(),
            offset: 0,
            raw_len: misc.len(),
            data: misc,
        }],
        CityFileKind::WinSc2,
    )
    .expect("view decode");

    assert_eq!(
        view,
        CityView {
            mode: 2,
            x: 70,
            y: 66
        }
    );
}

#[test]
fn win95_saved_camera_setter_round_trips_through_misc_and_file_save() {
    let path = std::env::temp_dir().join("sc2000_saved_camera_round_trip.sc2");
    let mut city = City::new_blank(&path, "camera test").expect("blank Win95 city");
    let view = CityView {
        mode: 1,
        x: 77,
        y: 33,
    };

    assert!(city.set_saved_view(view));
    assert_eq!(city.saved_view(), Some(view));
    city.save(&path).expect("save camera view");
    let loaded = City::load(&path).expect("reload camera view");
    assert_eq!(loaded.saved_view(), Some(view));
    let _ = fs::remove_file(path);
}

#[test]
fn win95_saved_camera_uses_original_no_target_sentinel() {
    let mut misc = vec![0u8; 0x1018];
    misc[0x1010..0x1014].copy_from_slice(&u32::from(u16::MAX).to_be_bytes());
    misc[0x1014..0x1018].copy_from_slice(&1u32.to_be_bytes());
    let view = decode_city_view(
        &[CityChunk {
            name: "MISC".to_owned(),
            offset: 0,
            raw_len: misc.len(),
            data: misc,
        }],
        CityFileKind::WinScenario,
    )
    .expect("sentinel view decode");

    assert_eq!(
        view,
        CityView {
            mode: 1,
            x: 64,
            y: 128
        }
    );
}

#[test]
fn does_not_apply_win95_saved_view_layout_to_mac_misc() {
    let misc = vec![0u8; 0x1018];
    assert!(
        decode_city_view(
            &[CityChunk {
                name: "MISC".to_owned(),
                offset: 0,
                raw_len: misc.len(),
                data: misc,
            }],
            CityFileKind::MacCity,
        )
        .is_err()
    );
}

#[test]
fn downloaded_maxis_land_opens_at_saved_date_and_rotation() {
    let Some(city) = downloaded_maxis_land() else {
        return;
    };
    assert_eq!(
        city.saved_date(),
        Some(CityDate {
            year: 2078,
            month: 4
        })
    );
    assert_eq!(city.saved_rotation(), 3);
    assert_eq!(city.rotation(), 0);
    assert!(
        city.buildings
            .as_ref()
            .is_some_and(|buildings| { buildings.iter().filter(|&&id| id != 0).count() > 4000 })
    );
    assert!(city.has_traffic_density_map());
}

#[test]
fn infers_corner_key_bits_per_mac_save_layout() {
    let Some(original) = load_mac() else {
        return;
    };
    assert_eq!(original.corner_key_bits, [0x40, 0x20, 0x10, 0x80]);

    let Some(edited) = downloaded_maxis_land() else {
        return;
    };
    assert_eq!(edited.corner_key_bits, [0x20, 0x10, 0x80, 0x40]);
}

#[test]
fn decodes_xthg_when_present() {
    let Some(win) = load_win() else {
        return;
    };
    if win.chunk("XTHG").is_none() {
        return;
    }
    assert!(win.has_traffic_overlays());
    let overlays = win.traffic_overlays().expect("overlays");
    assert_eq!(overlays.len(), crate::win95_overlay::XTHG_RECORD_COUNT);
}

#[test]
fn xthg_round_trip_through_save_layers() {
    let Some(mut win) = load_win() else {
        return;
    };
    if !win.has_traffic_overlays() {
        return;
    }
    let before = win.traffic_overlays().unwrap().to_vec();
    win.update_chunks_from_layers();
    let raw = decode_expected_chunk(&win.chunks, "XTHG", XTHG_CHUNK_SIZE).expect("xthg chunk");
    let after = decode_xthg_records(&raw).expect("decode");
    assert_eq!(after, before);
}

#[test]
fn mac_xthg_records_round_trip_without_pe_interpretation() {
    let path = std::env::temp_dir().join(format!(
        "sc2000-mac-xthg-roundtrip-{}.city",
        std::process::id()
    ));
    let mut city = City::new_blank(&path, "MacXthgRoundTrip").expect("blank city");
    city.kind = CityFileKind::MacCity;
    // XTXT is Mac's separately saved per-cell proxy/marker plane at
    // `fffffb44`; keep representative proxy, text, and disaster markers
    // independent of the XTHG record bytes below.
    city.set_text_overlay(12, 34, 0xd0);
    city.set_text_overlay(64, 65, 0xff);
    city.set_text_overlay(127, 127, 0x33);
    let records = (0..XTHG_RECORD_COUNT)
        .map(|slot| {
            let bytes: [u8; crate::win95_overlay::XTHG_RECORD_SIZE] =
                std::array::from_fn(|field| {
                    (slot * crate::win95_overlay::XTHG_RECORD_SIZE + field) as u8
                });
            Win95OverlayRecord::from_bytes(&bytes)
        })
        .collect::<Vec<_>>();
    city.set_traffic_overlays_for_test(records.clone());

    city.save(&path).expect("save synthetic Mac XTHG city");
    let loaded = City::load(&path).expect("reload synthetic Mac XTHG city");
    assert_eq!(loaded.kind, CityFileKind::MacCity);
    assert_eq!(loaded.text_overlay_at(12, 34), Some(0xd0));
    assert_eq!(loaded.text_overlay_at(64, 65), Some(0xff));
    assert_eq!(loaded.text_overlay_at(127, 127), Some(0x33));
    let xtxt = decode_expected_chunk(&loaded.chunks, "XTXT", CITY_TILES)
        .expect("decoded Mac XTXT payload");
    assert_eq!(xtxt[city_index(12, 34)], 0xd0);
    assert_eq!(xtxt[city_index(64, 65)], 0xff);
    assert_eq!(xtxt[city_index(127, 127)], 0x33);
    assert_eq!(
        loaded.traffic_overlays(),
        Some(records.as_slice()),
        "Mac XTHG bytes must round-trip without running PE actor semantics"
    );
    let raw = decode_expected_chunk(&loaded.chunks, "XTHG", XTHG_CHUNK_SIZE)
        .expect("decoded Mac XTHG payload");
    let expected = records
        .iter()
        .flat_map(|record| record.to_bytes())
        .collect::<Vec<_>>();
    assert_eq!(raw, expected);
    let _ = fs::remove_file(path);

    // When a native Mac fixture is configured, also exercise its compressed
    // on-disk XTHG chunk through a real save/reload copy. The default test
    // suite remains self-contained when no external fixture is installed.
    if let Some(mut fixture) = load_mac() {
        let before = fixture
            .traffic_overlays()
            .expect("configured Mac fixture has XTHG records")
            .to_vec();
        let copy = std::env::temp_dir().join(format!(
            "sc2000-mac-xthg-fixture-roundtrip-{}.city",
            std::process::id()
        ));
        fixture.save(&copy).expect("save Mac fixture copy");
        let reloaded = City::load(&copy).expect("reload Mac fixture copy");
        assert_eq!(
            reloaded.traffic_overlays(),
            Some(before.as_slice()),
            "native Mac XTHG records must survive decode/save/reload byte-for-byte"
        );
        let _ = fs::remove_file(copy);
    }
}

#[test]
fn decodes_xbit_when_present() {
    let Some(mut win) = load_win() else {
        return;
    };
    if win.chunk("XBIT").is_some() {
        assert!(win.has_tile_bits_map());
        assert!(win.tile_bits_at(0, 0).is_some());

        let saved_xbit = decode_expected_chunk(&win.chunks, "XBIT", CITY_TILES)
            .expect("decode original XBIT plane");
        let decoded_xbit = win
            .tile_bits
            .as_ref()
            .expect("decoded tile bits")
            .iter()
            .map(|bits| bits.raw_bits())
            .collect::<Vec<_>>();
        assert_eq!(
            decoded_xbit, saved_xbit,
            "loading must preserve every XBIT byte"
        );

        win.update_chunks_from_layers();
        let round_tripped_xbit = decode_expected_chunk(&win.chunks, "XBIT", CITY_TILES)
            .expect("decode round-tripped XBIT plane");
        assert_eq!(
            round_tripped_xbit, saved_xbit,
            "saving must preserve every XBIT byte"
        );
    }
}

#[test]
fn synthetic_win95_xbit_plane_survives_city_load_and_save() {
    let input = std::env::temp_dir().join(format!(
        "sc2000-xbit-roundtrip-input-{}.sc2",
        std::process::id()
    ));
    let output = std::env::temp_dir().join(format!(
        "sc2000-xbit-roundtrip-output-{}.sc2",
        std::process::id()
    ));
    let original = (0..CITY_TILES).map(|index| index as u8).collect::<Vec<_>>();
    let mut bytes = Vec::with_capacity(20 + original.len());
    bytes.extend_from_slice(b"FORM");
    bytes.extend_from_slice(&((12 + original.len()) as u32).to_be_bytes());
    bytes.extend_from_slice(b"SCDH");
    bytes.extend_from_slice(b"XBIT");
    bytes.extend_from_slice(&(original.len() as u32).to_be_bytes());
    bytes.extend_from_slice(&original);
    std::fs::write(&input, bytes).expect("write synthetic XBIT city");

    let loaded = City::load(&input).expect("load synthetic XBIT city");
    let decoded = loaded
        .tile_bits
        .as_ref()
        .expect("XBIT plane present")
        .iter()
        .map(|bits| bits.raw_bits())
        .collect::<Vec<_>>();
    assert_eq!(decoded, original, "load must preserve all XBIT byte values");

    let mut loaded = loaded;
    loaded.save(&output).expect("save synthetic XBIT city");
    let saved_bytes = std::fs::read(&output).expect("read saved XBIT city");
    let saved_chunks = parse_scdh_chunks(&saved_bytes).expect("parse saved XBIT chunks");
    let round_tripped =
        decode_expected_chunk(&saved_chunks, "XBIT", CITY_TILES).expect("decode saved XBIT plane");
    assert_eq!(
        round_tripped, original,
        "save must preserve every XBIT byte"
    );

    let _ = std::fs::remove_file(input);
    let _ = std::fs::remove_file(output);
}

#[test]
fn decodes_rotation_and_corner_flags() {
    let Some(mac) = load_mac() else {
        return;
    };
    assert_eq!(mac.rotation(), 0);

    let zone_bytes = decode_expected_chunk(&mac.chunks, "XZON", CITY_TILES).expect("xzon decode");
    assert_eq!(zone_bytes.len(), CITY_TILES);
    assert!(
        mac.zones
            .as_ref()
            .is_some_and(|zones| zones.len() == CITY_TILES)
    );
    assert_eq!(ZoneFlags::new(0x41).zone_sprite_id(), Some(1291));
    assert_eq!(ZoneFlags::new(0x06).zone_sprite_id(), Some(1296));
    assert_eq!(ZoneFlags::new(0x00).zone_sprite_id(), None);
}

#[test]
fn corner_key_rotation_matches_renderer_projection() {
    assert!(ZoneFlags::new(0x40).key_for_rotation(0));
    assert!(ZoneFlags::new(0x20).key_for_rotation(1));
    assert!(ZoneFlags::new(0x10).key_for_rotation(2));
    assert!(ZoneFlags::new(0x80).key_for_rotation(3));

    assert!(!ZoneFlags::new(0x20).key_for_rotation(0));
    assert!(!ZoneFlags::new(0x20).key_for_rotation(3));
}

#[test]
fn corner_anchored_buildings_only_draw_on_active_corner() {
    let Some(mac) = load_mac() else {
        return;
    };

    let mut church_tiles = 0;
    let mut drawn_church_tiles = 0;
    let mut tree_tiles = 0;
    let mut drawn_tree_tiles = 0;

    for y in 0..CITY_HEIGHT {
        for x in 0..CITY_WIDTH {
            match mac.building_at(x, y) {
                7 => {
                    tree_tiles += 1;
                    if mac.building_should_draw_at(x, y) {
                        drawn_tree_tiles += 1;
                    }
                }
                247 => {
                    church_tiles += 1;
                    if mac.building_should_draw_at(x, y) {
                        drawn_church_tiles += 1;
                    }
                }
                _ => {}
            }
        }
    }

    assert!(tree_tiles > 0);
    assert_eq!(tree_tiles, drawn_tree_tiles);
    assert_eq!(church_tiles, 4);
    assert_eq!(drawn_church_tiles, 1);
}

#[test]
fn cornerless_multitile_building_footprint_draws_once() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    stamp_complete_building(&mut city, 20, 30, 251, 4);

    let drawn = (30..34)
        .flat_map(|y| (20..24).map(move |x| (x, y)))
        .filter(|&(x, y)| city.building_should_draw_at(x, y))
        .collect::<Vec<_>>();

    assert_eq!(drawn.len(), 1);
}

#[test]
fn cornerless_multitile_building_inferred_draw_corner_tracks_rotation() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    stamp_complete_building(&mut city, 20, 30, 251, 4);

    for rotation in 0..4 {
        city.set_view_rotation(rotation);
        let drawn = (30..34)
            .flat_map(|y| (20..24).map(move |x| (x, y)))
            .filter(|&(x, y)| city.building_should_draw_at(x, y))
            .collect::<Vec<_>>();

        assert_eq!(drawn.len(), 1, "rotation {rotation}");
        let [top_left, top_right, bottom_left, bottom_right] = city.footprint_corner_bits();
        let expected = match city.active_corner_key_bit() {
            bit if bit == top_left => (20, 30),
            bit if bit == top_right => (23, 30),
            bit if bit == bottom_left => (20, 33),
            bit if bit == bottom_right => (23, 33),
            _ => (20, 30),
        };
        assert_eq!(drawn[0], expected, "rotation {rotation}");
    }
}

#[test]
fn cornerless_adjacent_multitile_buildings_draw_once_per_footprint() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    stamp_complete_building(&mut city, 20, 30, 251, 4);
    stamp_complete_building(&mut city, 24, 30, 251, 4);

    for rotation in 0..4 {
        city.set_view_rotation(rotation);
        let drawn = (30..34)
            .flat_map(|y| (20..28).map(move |x| (x, y)))
            .filter(|&(x, y)| city.building_should_draw_at(x, y))
            .collect::<Vec<_>>();

        assert_eq!(drawn.len(), 2, "rotation {rotation}");
    }
}

#[test]
fn non_active_corner_bits_do_not_fall_back_to_inferred_draw_tiles() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    stamp_complete_building(&mut city, 20, 30, 251, 4);
    let [top_left, top_right, bottom_left, bottom_right] = city.footprint_corner_bits();
    city.set_zone(20, 30, top_left);
    city.set_zone(23, 30, top_right);
    city.set_zone(20, 33, bottom_left);
    city.set_zone(23, 33, bottom_right);

    city.set_view_rotation(0);

    assert!(!city.building_should_draw_at(20, 30));
    assert!(city.building_should_draw_at(23, 33));
}

#[test]
fn partial_cornered_multitile_footprint_does_not_mix_inferred_anchor() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    stamp_complete_building(&mut city, 20, 30, 251, 4);
    let [top_left, _top_right, _bottom_left, _bottom_right] = city.footprint_corner_bits();
    city.set_zone(20, 30, top_left);

    for rotation in 0..4 {
        city.set_view_rotation(rotation);
        let drawn = (30..34)
            .flat_map(|y| (20..24).map(move |x| (x, y)))
            .filter(|&(x, y)| city.building_should_draw_at(x, y))
            .collect::<Vec<_>>();

        assert!(drawn.is_empty(), "rotation {rotation}: {drawn:?}");
    }
}

#[test]
fn orphaned_cornered_multitile_fragment_does_not_draw() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let [top_left, _top_right, _bottom_left, _bottom_right] = city.footprint_corner_bits();
    city.set_building(20, 30, 251);
    city.set_zone(20, 30, top_left);
    city.set_view_rotation(2);

    assert!(!city.building_should_draw_at(20, 30));
}

#[test]
fn loaded_mac_city_multitile_buildings_draw_at_most_once_per_footprint() {
    let Some(mut city) = load_mac() else {
        return;
    };
    let mut claimed = vec![false; CITY_TILES];
    let mut footprints = Vec::new();

    for y in 0..CITY_HEIGHT {
        for x in 0..CITY_WIDTH {
            let index = city_index(x, y);
            if claimed[index] {
                continue;
            }
            let building = city.building_at(x, y);
            let size = usize::from(multi_tile_building_size(building));
            if building == 0 || size <= 1 {
                continue;
            }
            if !city.complete_building_footprint(x, y, building, size) {
                continue;
            }

            for ty in y..y + size {
                for tx in x..x + size {
                    claimed[city_index(tx, ty)] = true;
                }
            }
            footprints.push((x, y, building, size));
        }
    }

    assert!(
        !footprints.is_empty(),
        "loaded Mac city should contain multi-tile buildings"
    );

    for rotation in 0..4 {
        city.set_view_rotation(rotation);
        let repeated = footprints
            .iter()
            .filter_map(|&(origin_x, origin_y, building, size)| {
                let drawn = (origin_y..origin_y + size)
                    .flat_map(|ty| (origin_x..origin_x + size).map(move |tx| (tx, ty)))
                    .filter(|&(tx, ty)| city.building_should_draw_at(tx, ty))
                    .collect::<Vec<_>>();
                (drawn.len() > 1).then_some((origin_x, origin_y, building, size, drawn))
            })
            .collect::<Vec<_>>();

        assert!(
            repeated.is_empty(),
            "rotation {rotation} drew multi-tile buildings more than once: {repeated:?}"
        );
    }
}

#[test]
fn rle_decoder_handles_literal_and_repeat_runs() {
    let decoded = sc2_rle_decode(&[3, 1, 2, 3, 130, 9]).expect("decode");
    assert_eq!(decoded, vec![1, 2, 3, 9, 9, 9]);
}

#[test]
fn decodes_altm_land_water_and_tunnel_levels() {
    let raw = u16::from_be_bytes([0x7d, 0x42]);
    let sample = AltmSample {
        land: (raw & 0x1f) as u8,
        water: ((raw >> 5) & 0x1f) as u8,
        tunnel_levels: ((raw >> 10) & 0x3f) as u8,
    };
    assert_eq!(
        sample,
        AltmSample {
            land: 2,
            water: 10,
            tunnel_levels: 31,
        }
    );
}

#[test]
fn altm_encoder_preserves_all_upper_bits() {
    let encoded = encode_altm(&[AltmSample {
        land: 2,
        water: 10,
        tunnel_levels: 0x3f,
    }]);
    assert_eq!(encoded, vec![0xfd, 0x42]);
}

#[test]
fn altitude_edits_preserve_tunnel_levels() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_tunnel_levels(12, 34, 7);
    city.set_altitude(12, 34, 9, 11);

    assert_eq!(city.altitude_at(12, 34), 9);
    assert_eq!(city.water_height_at(12, 34), 11);
    assert_eq!(city.tunnel_levels_at(12, 34), 7);
}

#[test]
fn xbit_water_tile_flag() {
    assert!(!TileBits::new(0b0000_0011).water_tile());
    assert!(TileBits::new(0b0000_0100).water_tile());
    assert!(TileBits::new(0b0000_0111).water_tile());
}

#[test]
fn xbit_rotate_uses_the_opensc2k_bit() {
    assert!(!TileBits::new(0b0100_0000).rotate());
    assert!(TileBits::new(0b0000_0010).rotate());
}

#[test]
fn xbit_rotate_updates_preserve_other_flags() {
    let bits = TileBits::new(0b1011_0101).with_rotate(true);
    assert_eq!(bits.raw_bits(), 0b1011_0111);
    assert_eq!(bits.with_rotate(false).raw_bits(), 0b1011_0101);
}

#[test]
fn xbit_pipe_state_flags_match_opensc2k() {
    assert!(!TileBits::new(0b0000_1111).piped());
    assert!(TileBits::new(0b0010_0000).piped());

    assert!(!TileBits::new(0b0010_1111).watered());
    assert!(TileBits::new(0b0001_0000).watered());
}

#[test]
fn xbit_pipe_state_updates_preserve_other_flags() {
    let bits = TileBits::new(0b0100_0101)
        .with_piped(true)
        .with_watered(true);
    assert_eq!(bits.raw_bits(), 0b0111_0101);
    assert_eq!(
        bits.with_piped(false).with_watered(false).raw_bits(),
        0b0100_0101
    );
}

#[test]
fn xbit_power_flags_match_sc2kfix_names() {
    assert!(TileBits::new(0b0100_0000).powered());
    assert!(TileBits::new(0b1000_0000).powerable());
    let bits = TileBits::new(0).with_powered(true).with_powerable(true);
    assert_eq!(bits.raw_bits(), 0b1100_0000);
}

#[test]
fn structure_elevation_uses_water_height_on_water_tiles() {
    let Some(mac) = load_mac() else {
        return;
    };
    let mut water_surface_tiles = 0;
    let mut mismatched = 0;
    for y in 0..CITY_HEIGHT {
        for x in 0..CITY_WIDTH {
            let terrain = mac.terrain_at(x, y);
            let sample = mac.altm_at(x, y);
            if terrain >= 0x10 || mac.tile_bits_at(x, y).is_some_and(TileBits::water_tile) {
                water_surface_tiles += 1;
                assert_eq!(
                    mac.structure_elevation_at(x, y),
                    sample.water,
                    "tile ({x},{y}) xter=0x{terrain:02x}"
                );
            } else if sample.land != sample.water {
                mismatched += 1;
                assert_eq!(mac.structure_elevation_at(x, y), sample.land);
            }
        }
    }
    assert!(
        water_surface_tiles > 0,
        "expected water/coast tiles in Maxis Land"
    );
    assert!(
        mismatched > 0,
        "expected some land tiles where water height differs"
    );
}

#[test]
fn maxis_land_has_distinct_water_heights() {
    let Some(mac) = load_mac() else {
        return;
    };
    let mut distinct_water = std::collections::BTreeSet::new();
    for y in 0..CITY_HEIGHT {
        for x in 0..CITY_WIDTH {
            distinct_water.insert(mac.water_height_at(x, y));
        }
    }
    assert!(
        distinct_water.len() > 1,
        "expected varied ALTM water heights, got {distinct_water:?}"
    );
}

#[test]
fn xtrf_microsim_resize_roundtrip() {
    let mut micro = vec![0u8; 4096];
    for y in 0..64 {
        for x in 0..64 {
            micro[y * 64 + x] = ((x + y) % 256) as u8;
        }
    }
    let expanded = resize_map_64_to_128(&micro);
    assert_eq!(expanded.len(), CITY_TILES);
    let resaved = resize_map_128_to_64(&expanded);
    assert_eq!(resaved, micro);
}

#[test]
fn win95_xtrf_rejects_expanded_128_square_payload() {
    let chunk = CityChunk {
        name: "XTRF".to_owned(),
        offset: 0,
        raw_len: CITY_TILES,
        data: vec![0; CITY_TILES],
    };
    let error = decode_xtrf(&[chunk], CityFileKind::WinSc2)
        .expect_err("Win95 loader requires a decoded 64×64 XTRF plane");
    assert!(error.to_string().contains("expected 4096"));
}

#[test]
fn win95_load_propagates_present_invalid_xtrf() {
    let temp = std::env::temp_dir().join("sc2000-invalid-win95-xtrf.sc2");
    let payload = vec![0; CITY_TILES];
    let mut bytes = Vec::with_capacity(20 + payload.len());
    bytes.extend_from_slice(b"FORM");
    bytes.extend_from_slice(&((12 + payload.len()) as u32).to_be_bytes());
    bytes.extend_from_slice(b"SCDH");
    bytes.extend_from_slice(b"XTRF");
    bytes.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    bytes.extend_from_slice(&payload);
    std::fs::write(&temp, bytes).expect("write malformed Win95 fixture");

    let error = City::load(&temp).expect_err("present invalid XTRF must fail Win95 load");
    assert!(error.to_string().contains("loading XTRF chunk"));
    let _ = std::fs::remove_file(temp);
}

#[test]
fn mac_xtrf_accepts_expanded_128_square_payload() {
    let chunk = CityChunk {
        name: "XTRF".to_owned(),
        offset: 0,
        raw_len: CITY_TILES,
        data: vec![0; CITY_TILES],
    };
    let (decoded, format) =
        decode_xtrf(&[chunk], CityFileKind::MacCity).expect("Mac per-tile XTRF remains supported");
    assert_eq!(decoded.len(), CITY_TILES);
    assert_eq!(format, XtrfStorageFormat::PerTile128);
}

#[test]
fn win95_xplt_rejects_expanded_128_square_payload() {
    let chunk = CityChunk {
        name: "XPLT".to_owned(),
        offset: 0,
        raw_len: CITY_TILES,
        data: vec![0; CITY_TILES],
    };
    let error = decode_microsim64_chunk(&[chunk], "XPLT", CityFileKind::WinSc2)
        .expect_err("Win95 loader requires a decoded 64×64 XPLT plane");
    assert!(error.to_string().contains("expected 4096"));
}

#[test]
fn mac_xplt_accepts_expanded_128_square_payload() {
    let chunk = CityChunk {
        name: "XPLT".to_owned(),
        offset: 0,
        raw_len: CITY_TILES,
        data: vec![0; CITY_TILES],
    };
    let decoded = decode_microsim64_chunk(&[chunk], "XPLT", CityFileKind::MacCity)
        .expect("Mac per-tile XPLT remains supported");
    assert_eq!(decoded.len(), CITY_TILES);
}

#[test]
fn win95_load_propagates_present_invalid_xplt() {
    let temp = std::env::temp_dir().join(format!(
        "sc2000-invalid-win95-xplt-{}.sc2",
        std::process::id()
    ));
    let payload = vec![0; CITY_TILES];
    let mut bytes = Vec::with_capacity(20 + payload.len());
    bytes.extend_from_slice(b"FORM");
    bytes.extend_from_slice(&((12 + payload.len()) as u32).to_be_bytes());
    bytes.extend_from_slice(b"SCDH");
    bytes.extend_from_slice(b"XPLT");
    bytes.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    bytes.extend_from_slice(&payload);
    std::fs::write(&temp, bytes).expect("write malformed Win95 fixture");

    let load_result = City::load(&temp);
    let _ = std::fs::remove_file(temp);
    let error = load_result.expect_err("present invalid XPLT must fail Win95 load");
    assert!(error.to_string().contains("loading XPLT chunk"));
}

#[test]
fn win95_32_square_planes_reject_wrong_decoded_sizes() {
    for name in ["XPLC", "XFIR", "XPOP", "XROG"] {
        let chunk = CityChunk {
            name: name.to_owned(),
            offset: 0,
            raw_len: CITY_TILES,
            data: sc2_rle_encode(&vec![0; CITY_TILES]),
        };
        let error = decode_optional_microsim32_chunk(&[chunk], name, CityFileKind::WinSc2)
            .expect_err("Win95 32×32 plane must contain 1024 decoded bytes");
        assert!(error.to_string().contains(&format!("loading {name} chunk")));
    }
}

#[test]
fn win95_load_propagates_present_invalid_xplc() {
    let temp = std::env::temp_dir().join(format!(
        "sc2000-invalid-win95-xplc-{}.sc2",
        std::process::id()
    ));
    let payload = sc2_rle_encode(&vec![0; 2048]);
    let mut bytes = Vec::with_capacity(20 + payload.len());
    bytes.extend_from_slice(b"FORM");
    bytes.extend_from_slice(&((12 + payload.len()) as u32).to_be_bytes());
    bytes.extend_from_slice(b"SCDH");
    bytes.extend_from_slice(b"XPLC");
    bytes.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    bytes.extend_from_slice(&payload);
    std::fs::write(&temp, bytes).expect("write malformed Win95 fixture");

    let load_result = City::load(&temp);
    let _ = std::fs::remove_file(temp);
    let error = load_result.expect_err("present invalid XPLC must fail Win95 load");
    assert!(error.to_string().contains("loading XPLC chunk"));
}

#[test]
fn win95_load_propagates_present_invalid_fixed_chunks() {
    for name in ["XTER", "XBLD", "XZON", "XBIT", "XVAL", "XCRM", "XTHG"] {
        let temp = std::env::temp_dir().join(format!(
            "sc2000-invalid-win95-{}-{}.sc2",
            name,
            std::process::id()
        ));
        let mut bytes = Vec::with_capacity(21);
        bytes.extend_from_slice(b"FORM");
        bytes.extend_from_slice(&13u32.to_be_bytes());
        bytes.extend_from_slice(b"SCDH");
        bytes.extend_from_slice(name.as_bytes());
        bytes.extend_from_slice(&1u32.to_be_bytes());
        bytes.push(0); // Invalid RLE control byte.
        std::fs::write(&temp, bytes).expect("write malformed Win95 fixture");

        let load_result = City::load(&temp);
        let _ = std::fs::remove_file(temp);
        let error = match load_result {
            Err(error) => error,
            Ok(_) => panic!("present invalid {name} must fail Win95 load"),
        };
        assert!(
            error.to_string().contains(&format!("loading {name} chunk")),
            "unexpected {name} error: {error}"
        );
    }
}

#[test]
fn win95_optional_city_layers_reject_wrong_decoded_sizes() {
    let expected_sizes = [
        ("XMIC", MICROSIM_COUNT * MICROSIM_RECORD_LEN),
        ("XLAB", CITY_LABEL_COUNT * CITY_LABEL_RECORD_LEN),
        ("XTXT", CITY_TILES),
        ("XUND", CITY_TILES),
    ];
    for (name, expected) in expected_sizes {
        let chunk = CityChunk {
            name: name.to_owned(),
            offset: 0,
            raw_len: expected + 1,
            data: sc2_rle_encode(&vec![0; expected + 1]),
        };
        let decode = |chunks: &[CityChunk]| decode_expected_chunk(chunks, name, expected);
        let error =
            decode_optional_chunk_layer(&[chunk.clone()], name, CityFileKind::WinSc2, decode)
                .expect_err("present malformed Win95 layer must fail");
        assert!(error.to_string().contains(&format!("loading {name} chunk")));

        let mac = decode_optional_chunk_layer(&[chunk], name, CityFileKind::MacCity, decode)
            .expect("Mac retains best-effort layer decoding");
        assert!(mac.is_none(), "malformed Mac {name} remains absent");
    }
}

#[test]
fn cnam_reads_nul_terminated_text_after_metadata_byte() {
    assert_eq!(cnam_text(&[1, b'C', b'i', b't', b'y', 0, b'!']), "City");
    assert_eq!(cnam_text(&[0, b'X', b'y']), "Xy");
    assert_eq!(
        cnam_text(&[0, b' ', b'C', b'i', b't', b'y', b' ']),
        " City "
    );
    let path = Path::new("MacTown.city");
    assert_eq!(
        city_name_from_chunks(path, &[], CityFileKind::MacCity),
        "MacTown.city"
    );
    assert_eq!(city_name_from_chunks(path, &[], CityFileKind::WinSc2), "");
}

#[test]
fn win95_load_keeps_empty_name_for_missing_or_zero_length_cnam() {
    for (suffix, cnam) in [("missing", None), ("empty", Some(&[][..]))] {
        let temp = std::env::temp_dir().join(format!(
            "sc2000-win95-cnam-{suffix}-{}.sc2",
            std::process::id()
        ));
        let mut form = b"SCDH".to_vec();
        if let Some(cnam) = cnam {
            form.extend_from_slice(b"CNAM");
            form.extend_from_slice(&(cnam.len() as u32).to_be_bytes());
            form.extend_from_slice(cnam);
        }
        let mut bytes = Vec::with_capacity(8 + form.len());
        bytes.extend_from_slice(b"FORM");
        bytes.extend_from_slice(&(form.len() as u32).to_be_bytes());
        bytes.extend_from_slice(&form);
        std::fs::write(&temp, bytes).expect("write CNAM fixture");

        let load_result = City::load(&temp);
        let _ = std::fs::remove_file(temp);
        let city = load_result.expect("minimal Win95 city fixture should load");
        assert_eq!(
            city.name, "",
            "{suffix} CNAM follows the PE empty-name reset"
        );
    }
}

#[test]
fn win95_altm_misc_and_xgrp_validate_decoded_sizes() {
    for (name, expected) in [
        ("ALTM", CITY_TILES * 2),
        ("MISC", WIN95_MISC_RECORD_SIZE),
        ("XGRP", 0xd00),
    ] {
        let bad_chunk = CityChunk {
            name: name.to_owned(),
            offset: 0,
            raw_len: expected + 1,
            data: sc2_rle_encode(&vec![0; expected + 1]),
        };
        let decode =
            |chunks: &[CityChunk]| decode_expected_chunk(chunks, name, expected).map(|_| ());
        let error =
            decode_optional_chunk_layer(&[bad_chunk.clone()], name, CityFileKind::WinSc2, decode)
                .expect_err("present malformed Win95 chunk must fail");
        assert!(error.to_string().contains(&format!("loading {name} chunk")));

        let mac = decode_optional_chunk_layer(&[bad_chunk], name, CityFileKind::MacCity, decode)
            .expect("Mac retains best-effort chunk decoding");
        assert!(mac.is_none(), "malformed Mac {name} remains absent");
    }

    let compressed_altm = CityChunk {
        name: "ALTM".to_owned(),
        offset: 0,
        raw_len: sc2_rle_encode(&vec![0; CITY_TILES * 2]).len(),
        data: sc2_rle_encode(&vec![0; CITY_TILES * 2]),
    };
    assert_eq!(
        decode_altm(&[compressed_altm])
            .expect("ALTM accepts a valid encoded payload")
            .len(),
        CITY_TILES
    );
}

#[test]
#[ignore = "requires CITYGAME_TEST_VIRTUAL_CITY pointing to a locally imported VIRTUALV.SC2"]
fn imported_win95_city_loads_its_versioned_misc_record() {
    let path = std::env::var_os("CITYGAME_TEST_VIRTUAL_CITY")
        .map(std::path::PathBuf::from)
        .expect("set CITYGAME_TEST_VIRTUAL_CITY to a locally imported VIRTUALV.SC2");
    let mut city = City::load(&path).expect("bundled Win95 city should load");
    assert_eq!(city.kind, CityFileKind::WinSc2);
    assert_eq!(city.chunk_u32_values("MISC", 0, 1), [0x122]);
    assert_eq!(
        city.saved_view(),
        Some(CityView {
            mode: 2,
            x: 34,
            y: 31,
        })
    );

    for (name, expanded) in [
        ("XVAL", city.land_value_density.as_ref()),
        ("XCRM", city.crime_density.as_ref()),
    ] {
        let expanded = expanded.expect("bundled Win95 density chunk");
        assert_eq!(expanded.len(), CITY_TILES, "{name} runtime plane");
        let raw = decode_expected_chunk(&city.chunks, name, 4096).expect("64×64 chunk");
        for (micro_x, micro_y) in [(0, 0), (17, 29), (63, 63)] {
            let value = raw[micro_y * 64 + micro_x];
            for dy in 0..2 {
                for dx in 0..2 {
                    assert_eq!(
                        expanded[city_index(micro_x * 2 + dx, micro_y * 2 + dy)],
                        value,
                        "{name} microsim sample ({micro_x},{micro_y}) expands across 2×2"
                    );
                }
            }
        }
    }

    let temp = std::env::temp_dir().join(format!(
        "sc2000-bundled-win95-density-resave-{}.sc2",
        std::process::id()
    ));
    city.save(&temp).expect("resave bundled Win95 city");
    let saved_bytes = fs::read(&temp).expect("read resaved city");
    let saved_chunks = parse_scdh_chunks(&saved_bytes).expect("parse resaved city");
    for name in ["XVAL", "XCRM"] {
        assert_eq!(
            decode_expected_chunk(&saved_chunks, name, 4096)
                .expect("Win95 density saves as 64×64")
                .len(),
            4096
        );
    }
    let _ = fs::remove_file(temp);
}

#[test]
fn win95_misc_rejects_unknown_record_version() {
    let mut data = vec![0; WIN95_MISC_RECORD_SIZE];
    data[..4].copy_from_slice(&0x121u32.to_be_bytes());
    let chunk = CityChunk {
        name: "MISC".to_owned(),
        offset: 0,
        raw_len: data.len(),
        data,
    };
    let error = decode_win95_misc_record(&[chunk]).expect_err("unknown version must fail");
    assert!(
        error
            .to_string()
            .contains("unsupported Win95 MISC record version")
    );
}

#[test]
fn win95_load_propagates_present_invalid_altm_misc_and_xgrp() {
    for (name, expected) in [
        ("ALTM", CITY_TILES * 2),
        ("MISC", WIN95_MISC_RECORD_SIZE),
        ("XGRP", 0xd00),
    ] {
        let temp = std::env::temp_dir().join(format!(
            "sc2000-invalid-win95-{name}-{}.sc2",
            std::process::id()
        ));
        let payload = sc2_rle_encode(&vec![0; expected + 1]);
        let mut bytes = Vec::with_capacity(20 + payload.len());
        bytes.extend_from_slice(b"FORM");
        bytes.extend_from_slice(&((12 + payload.len()) as u32).to_be_bytes());
        bytes.extend_from_slice(b"SCDH");
        bytes.extend_from_slice(name.as_bytes());
        bytes.extend_from_slice(&(payload.len() as u32).to_be_bytes());
        bytes.extend_from_slice(&payload);
        std::fs::write(&temp, bytes).expect("write malformed Win95 fixture");

        let load_result = City::load(&temp);
        let _ = std::fs::remove_file(temp);
        let error = load_result.expect_err("present invalid Win95 chunk must fail load");
        assert!(error.to_string().contains(&format!("loading {name} chunk")));
    }
}

#[test]
fn win95_load_propagates_present_invalid_xund() {
    let temp = std::env::temp_dir().join(format!(
        "sc2000-invalid-win95-xund-{}.sc2",
        std::process::id()
    ));
    let payload = sc2_rle_encode(&vec![0; CITY_TILES + 1]);
    let mut bytes = Vec::with_capacity(20 + payload.len());
    bytes.extend_from_slice(b"FORM");
    bytes.extend_from_slice(&((12 + payload.len()) as u32).to_be_bytes());
    bytes.extend_from_slice(b"SCDH");
    bytes.extend_from_slice(b"XUND");
    bytes.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    bytes.extend_from_slice(&payload);
    std::fs::write(&temp, bytes).expect("write malformed Win95 fixture");

    let load_result = City::load(&temp);
    let _ = std::fs::remove_file(temp);
    let error = load_result.expect_err("present invalid XUND must fail Win95 load");
    assert!(error.to_string().contains("loading XUND chunk"));
}

#[test]
fn win95_xtrf_loads_as_microsim64() {
    let Some(city) = load_win() else {
        return;
    };
    assert_eq!(city.xtrf_storage_format(), XtrfStorageFormat::Microsim64);
    assert_eq!(
        city.traffic_density_slice().map(|d| d.len()),
        Some(CITY_TILES)
    );
}

#[test]
fn win95_save_emits_reference_chunk_order() {
    let Some(mut city) = load_win() else {
        return;
    };
    let temp = std::env::temp_dir().join("sc2000_rust_chunk_order_test.sc2");
    city.save(&temp).expect("save");
    let bytes = fs::read(&temp).expect("read save");
    let tags = chunk_tags_in_file(&bytes).expect("parse chunks");
    let known_in_file: Vec<&str> = tags
        .iter()
        .filter(|tag| SAVE_CHUNK_ORDER.contains(&tag.as_str()))
        .map(String::as_str)
        .collect();
    let expected: Vec<&str> = SAVE_CHUNK_ORDER
        .iter()
        .copied()
        .filter(|tag| tags.iter().any(|t| t == tag))
        .collect();
    assert_eq!(known_in_file, expected, "chunk order {tags:?}");
    let _ = fs::remove_file(temp);
}

#[test]
#[ignore = "requires CITYGAME_TEST_VIRTUAL_CITY pointing to a locally imported VIRTUALV.SC2"]
fn imported_win95_city_uses_mapped_save_chunk_order() {
    let path = std::env::var_os("CITYGAME_TEST_VIRTUAL_CITY")
        .map(std::path::PathBuf::from)
        .expect("set CITYGAME_TEST_VIRTUAL_CITY to a locally imported VIRTUALV.SC2");
    let bytes = fs::read(&path).expect("bundled Win95 reference city");
    let tags = chunk_tags_in_file(&bytes).expect("parse reference city chunks");
    let expected = [
        "MISC", "ALTM", "XTER", "XBLD", "XZON", "XUND", "XTXT", "XLAB", "XMIC", "XTHG", "XBIT",
        "XTRF", "XPLT", "XVAL", "XCRM", "XPLC", "XFIR", "XPOP", "XROG", "XGRP", "CNAM",
    ];
    let known_in_file: Vec<&str> = tags
        .iter()
        .filter(|tag| SAVE_CHUNK_ORDER.contains(&tag.as_str()))
        .map(String::as_str)
        .collect();
    assert_eq!(known_in_file, expected);
}

#[test]
fn win95_save_xtrf_decodes_to_4096_bytes() {
    let Some(mut city) = load_win() else {
        return;
    };
    let temp = std::env::temp_dir().join("sc2000_rust_xtrf_size_test.sc2");
    city.save(&temp).expect("save");
    let bytes = fs::read(&temp).expect("read save");
    let chunks = parse_scdh_chunks(&bytes).expect("parse");
    let xtrf = chunks.iter().find(|c| c.name == "XTRF").expect("XTRF");
    let decoded = sc2_rle_decode(&xtrf.data).unwrap_or_else(|_| xtrf.data.clone());
    assert_eq!(decoded.len(), 4096, "Win95 XTRF must be 64×64 microsim");
    let _ = fs::remove_file(temp);
}

#[test]
fn win95_resave_matches_original_chunk_payload_sizes() {
    let Some(path) = test_win_city() else {
        return;
    };
    let original_bytes = fs::read(&path).expect("read original");
    assert_eq!(&original_bytes[0..4], b"FORM");
    assert_eq!(&original_bytes[8..12], b"SCDH");
    let original = parse_scdh_chunks(&original_bytes).expect("parse original");

    let mut city = City::load(&path).expect("load");
    let temp = std::env::temp_dir().join("sc2000_rust_win95_qa.sc2");
    city.save(&temp).expect("save");
    let resaved_bytes = fs::read(&temp).expect("read resave");
    assert_eq!(&resaved_bytes[0..4], b"FORM");
    assert_eq!(&resaved_bytes[8..12], b"SCDH");
    let resaved = parse_scdh_chunks(&resaved_bytes).expect("parse resave");

    for orig in &original {
        let new = resaved
            .iter()
            .find(|chunk| chunk.name == orig.name)
            .unwrap_or_else(|| panic!("missing chunk {} after resave", orig.name));
        assert_eq!(
            decoded_chunk_payload_len(orig),
            decoded_chunk_payload_len(new),
            "{} payload size",
            orig.name
        );
    }

    if std::env::var("SC2000_WRITE_QA_SAVE").is_ok() {
        let qa_path = std::env::temp_dir().join("sc2000_rust_manual_qa.sc2");
        fs::copy(&temp, &qa_path).expect("copy qa save");
        eprintln!("Manual Win95 QA save written to {}", qa_path.display());
    }

    let _ = fs::remove_file(temp);
}

#[test]
fn win95_resave_roundtrip_loads_without_error() {
    let Some(path) = test_win_city() else {
        return;
    };
    let mut city = City::load(path).expect("load");
    let temp = std::env::temp_dir().join("sc2000_rust_win95_reload_test.sc2");
    let buildings_before: Vec<_> = (0..CITY_TILES)
        .map(|i| city.building_at(i % CITY_WIDTH, i / CITY_WIDTH))
        .collect();
    city.save(&temp).expect("save");
    let reloaded = City::load(&temp).expect("reload resave");
    assert_eq!(
        reloaded.xtrf_storage_format(),
        XtrfStorageFormat::Microsim64
    );
    for (i, &building_before) in buildings_before.iter().enumerate().take(CITY_TILES) {
        let x = i % CITY_WIDTH;
        let y = i / CITY_WIDTH;
        assert_eq!(
            building_before,
            reloaded.building_at(x, y),
            "building mismatch at ({x},{y})"
        );
    }
    let _ = fs::remove_file(temp);
}

fn decoded_chunk_payload_len(chunk: &CityChunk) -> usize {
    sc2_rle_decode(&chunk.data)
        .map(|data| data.len())
        .unwrap_or_else(|_| chunk.data.len())
}

#[test]
fn save_roundtrip_preserves_core_layers() {
    let Some(mut city) = load_win() else {
        return;
    };
    let temp = std::env::temp_dir().join("sc2000_rust_save_test.sc2");
    let building = city.building_at(64, 64);
    let terrain = city.terrain_at(10, 10);
    let traffic = city.traffic_density_slice().map(|s| s.to_vec());
    let xtrf_format = city.xtrf_storage_format();
    city.save(&temp).expect("save");
    let reloaded = City::load(&temp).expect("reload");
    assert_eq!(building, reloaded.building_at(64, 64));
    assert_eq!(terrain, reloaded.terrain_at(10, 10));
    assert_eq!(
        traffic,
        reloaded.traffic_density_slice().map(|s| s.to_vec())
    );
    assert_eq!(xtrf_format, reloaded.xtrf_storage_format());
    let _ = fs::remove_file(temp);
}

#[test]
fn save_roundtrip_preserves_underground_layer() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    assert!(city.has_underground_map());
    city.set_underground(12, 34, 16);
    city.set_underground(13, 34, 17);
    city.set_tunnel_levels(12, 34, 3);

    let temp = std::env::temp_dir().join("sc2000_rust_xund_save_test.sc2");
    city.save(&temp).expect("save");
    let reloaded = City::load(&temp).expect("reload");

    assert!(reloaded.has_underground_map());
    assert_eq!(reloaded.underground_at(12, 34), 16);
    assert_eq!(reloaded.underground_at(13, 34), 17);
    assert_eq!(reloaded.tunnel_levels_at(12, 34), 3);
    let _ = fs::remove_file(temp);
}

#[test]
fn mac_placetile_zone7_counters_match_native_replay() {
    let csv = include_str!("../../../tests/fixtures/simulation/mac/mac_placetile_zone7.csv");
    let mut city = City::new_blank("/tmp/mac-place-zone7.city", "test").unwrap();
    city.kind = CityFileKind::MacCity;
    for (line, row) in csv.lines().enumerate().skip(1) {
        let fields: Vec<&str> = row.split(',').collect();
        assert_eq!(fields.len(), 7, "fixture line {line}");
        let old: u8 = fields[1].parse().unwrap();
        let new: u8 = fields[2].parse().unwrap();
        city.set_building_without_mac_census(40, 50, old);
        city.set_zone(40, 50, 7);
        for i in 0..16 {
            city.set_mac_state_misc_u32_at(0x0fa8 + i * 4, 100);
        }
        city.set_building(40, 50, new);
        assert_eq!(
            city.building_at(40, 50),
            fields[3].parse::<u8>().unwrap(),
            "tile, fixture line {line}"
        );
        for (i, value) in fields[6].split(';').enumerate() {
            assert_eq!(
                city.mac_zone7_census_count(i),
                value.parse::<u16>().unwrap(),
                "counter {i}, fixture line {line}"
            );
        }
    }
}

#[test]
fn mac_placetile_both_census_paths_match_68020_replay() {
    let fixtures = [
        (
            100,
            include_str!("../../../tests/fixtures/simulation/mac/mac_placetile.csv"),
        ),
        (
            0,
            include_str!("../../../tests/fixtures/simulation/mac/mac_placetile_zero.csv"),
        ),
        (
            65535,
            include_str!("../../../tests/fixtures/simulation/mac/mac_placetile_max.csv"),
        ),
    ];
    let mut city = City::new_blank("/tmp/mac-place-native.city", "test").unwrap();
    city.kind = CityFileKind::MacCity;
    for (seed, csv) in fixtures {
        for (line, row) in csv.lines().enumerate().skip(1) {
            let fields: Vec<&str> = row.split(',').collect();
            assert_eq!(fields.len(), 7, "fixture line {line}");
            let zone: u8 = fields[0].parse().unwrap();
            let old: u8 = fields[1].parse().unwrap();
            let new: u8 = fields[2].parse().unwrap();
            city.set_building_without_mac_census(40, 50, old);
            city.set_zone(40, 50, zone);
            for id in [old, new] {
                city.set_mac_state_misc_u32_at(0x01f0 + usize::from(id) * 4, seed);
            }
            for i in 0..16 {
                city.set_mac_state_misc_u32_at(0x0fa8 + i * 4, seed);
            }
            city.set_building(40, 50, new);
            assert_eq!(
                city.building_at(40, 50),
                fields[3].parse::<u8>().unwrap(),
                "tile, line {line}"
            );
            assert_eq!(
                city.mac_xbld_tile_count(old),
                fields[4].parse::<u16>().unwrap(),
                "old count, line {line}"
            );
            assert_eq!(
                city.mac_xbld_tile_count(new),
                fields[5].parse::<u16>().unwrap(),
                "new count, line {line}"
            );
            for (i, value) in fields[6].split(';').enumerate() {
                assert_eq!(
                    city.mac_zone7_census_count(i),
                    value.parse::<u16>().unwrap(),
                    "special {i}, line {line}"
                );
            }
        }
    }
}

#[test]
fn pe_cohort_update_uses_published_pollution_not_expanded_map_sum() {
    let mut clean_map = City::new_blank("/tmp/pe-cohort-pollution.sc2", "test").unwrap();
    clean_map.pe_age_cohorts[0] = 60;
    clean_map.pe_eq_life_points[0] = 6_000;
    clean_map.pe_dw_city_pollution = 0;
    clean_map.sync_pollution_density_from_slice(&vec![0; CITY_TILES]);
    let mut different_map = clean_map.clone();
    different_map.sync_pollution_density_from_slice(&vec![1; CITY_TILES]);
    let mut rng_a = Win95CrtRand::new(1);
    let mut rng_b = Win95CrtRand::new(1);
    clean_map.advance_pe_eq_le_month(60, 0, 0, 0, 0, 0, 0, &mut rng_a);
    different_map.advance_pe_eq_le_month(60, 0, 0, 0, 0, 0, 0, &mut rng_b);
    assert_eq!(different_map.pe_age_cohorts, clean_map.pe_age_cohorts);
    assert_eq!(
        different_map.pe_eq_life_points, clean_map.pe_eq_life_points,
        "the display plane must not override the published pollution input"
    );
    assert_eq!(different_map.pe_eq_aux_points, clean_map.pe_eq_aux_points);
    assert_eq!(rng_a.seed(), rng_b.seed());
}

#[test]
fn pe_opinion_poll_consumes_latched_scores_and_saved_rates() {
    let mut city = City::new_blank("/tmp/poll-latch.sc2", "Poll").unwrap();
    city.set_pe_attitude_tax_bases([10, 20, 30]);
    city.set_pe_attitude_demand_inputs(8, 300, 200, 100);
    city.set_pe_traffic_count(610);
    city.recompute_pe_attitude_scores(100, 100, 100);
    let scores = city.pe_attitude_scores;
    city.set_pe_attitude_demand_inputs(1, 9_000, 8_000, 7_000);
    city.pe_eq_averages_ready = false;
    city.pe_city_crime = 37;
    city.pe_city_pollution = 49;
    let mut rng = Win95CrtRand::new(123);
    city.run_pe_opinion_poll_with_rng(1_000, 100, 5, 10, &mut rng);
    assert_eq!(city.pe_attitude_scores, scores);
    assert_eq!(city.pe_city_crime, 37);
    assert_eq!(city.pe_city_pollution, 49);
}

#[test]
fn pe_calendar_words_match_original_x86_replay() {
    let csv = include_str!("../../../tests/fixtures/simulation/win95/pe_graph_calendar.csv");
    let mut city = City::new_blank("/tmp/pe-age-words.sc2", "Age").unwrap();
    for (line, row) in csv.lines().enumerate().skip(1) {
        let values: Vec<u32> = row.split(',').map(|v| v.parse().unwrap()).collect();
        assert_eq!(values.len(), 4, "line {line}");
        city.pe_dw_city_age = values[0];
        city.pe_publish_graph_calendar_words_from_city_age_41354e();
        assert_eq!(
            u32::from(city.pe_w_calendar_month_mod()),
            values[1],
            "age {}",
            values[0]
        );
        assert_eq!(
            u32::from(city.pe_w_season()),
            values[2],
            "age {}",
            values[0]
        );
        assert_eq!(
            u32::from(city.pe_w_local_fame_base()),
            values[3],
            "age {}",
            values[0]
        );
    }
}

#[test]
fn pe_unemployment_dword_round_trips_without_overwriting_mac_slot() {
    for (index, kind) in [CityFileKind::WinSc2, CityFileKind::MacCity]
        .into_iter()
        .enumerate()
    {
        let extension = if kind == CityFileKind::MacCity {
            "city"
        } else {
            "sc2"
        };
        let path = std::env::temp_dir().join(format!(
            "citygame-unemployment-{}-{index}.{extension}",
            std::process::id()
        ));
        let mut city = City::new_blank(&path, "Unemployment").unwrap();
        city.kind = kind;
        let saved_mac_value = 0x1234_5678;
        if kind == CityFileKind::MacCity {
            city.set_mac_state_misc_u32_at(MISC_CITY_UNEMPLOYMENT_OFFSET, saved_mac_value);
        }
        city.set_pe_unemployment_percent(0xfedc_ba98);
        city.save(&path).unwrap();
        let loaded = City::load(&path).unwrap();
        assert_eq!(loaded.kind, kind);
        if kind == CityFileKind::MacCity {
            assert_eq!(
                loaded.mac_state_misc_u32_at(MISC_CITY_UNEMPLOYMENT_OFFSET),
                Some(saved_mac_value)
            );
            assert_eq!(loaded.pe_unemployment_percent(), 0);
        } else {
            assert_eq!(loaded.pe_unemployment_percent(), 0xfedc_ba98);
            assert_eq!(
                loaded.misc_u32_at(MISC_CITY_UNEMPLOYMENT_OFFSET),
                Some(0xfedc_ba98)
            );
        }
        std::fs::remove_file(path).unwrap();
    }
}

#[test]
fn mac_zero_scalar_cohort_clear_matches_original_68020() {
    let fixture = include_str!("../../../tests/fixtures/simulation/mac/mac_cohort_zero.csv");
    for line in fixture.lines().skip(1) {
        let c: Vec<_> = line.split(',').collect();
        let scalar: u32 = c[0].parse().unwrap();
        let before: Vec<u32> = c[3].split(':').map(|v| v.parse().unwrap()).collect();
        let after: Vec<u32> = c[4].split(':').map(|v| v.parse().unwrap()).collect();
        for (kind_index, kind) in [CityFileKind::MacCity, CityFileKind::WinSc2]
            .into_iter()
            .enumerate()
        {
            let mut city = City::new_blank("/tmp/mac-zero-cohort.city", "oracle").unwrap();
            city.kind = kind;
            let summary_sentinel = if kind.is_win95() { 77 } else { 12345 };
            city.set_mac_state_misc_u32_at(MISC_NORMAL_POPULATION_OFFSET, scalar);
            for lane in 0..3 {
                for index in 0..20 {
                    city.set_mac_state_misc_u32_at(
                        MISC_EQ_LE_TABLES_OFFSET + (index * 3 + lane) * 4,
                        before[lane * 20 + index],
                    );
                }
            }
            for offset in [0x44, 0x48, 0x4c] {
                city.set_mac_state_misc_u32_at(offset, summary_sentinel);
            }
            if kind.is_win95() {
                // Model a loaded Windows-layout city: PE caches hydrate from
                // these same saved summary slots before Mac fidelity runs.
                city.pe_eq_workforce_percent = summary_sentinel;
                city.pe_eq_life_average = summary_sentinel;
                city.pe_eq_education_average = summary_sentinel;
            }
            assert_eq!(
                city.mac_clear_cohorts_if_zero_growth(),
                scalar == 0,
                "{line}, {kind:?}"
            );
            for lane in 0..3 {
                for index in 0..20 {
                    assert_eq!(
                        city.mac_state_misc_u32_at(
                            MISC_EQ_LE_TABLES_OFFSET + (index * 3 + lane) * 4
                        ),
                        Some(after[lane * 20 + index]),
                        "{line}, {kind:?}"
                    );
                }
            }
            for offset in [0x44, 0x48, 0x4c] {
                assert_eq!(city.mac_state_misc_u32_at(offset), Some(summary_sentinel));
            }
            if scalar == 0 {
                let path = std::env::temp_dir().join(format!(
                    "mac-zero-cohort-{}-{kind_index}.{}",
                    std::process::id(),
                    if kind == CityFileKind::MacCity {
                        "city"
                    } else {
                        "sc2"
                    }
                ));
                city.save(&path).expect("save shared zeroed cohort lanes");
                let loaded = City::load(&path).expect("reload shared zeroed cohort lanes");
                for lane in 0..3 {
                    for index in 0..20 {
                        assert_eq!(
                            loaded.mac_state_misc_u32_at(
                                MISC_EQ_LE_TABLES_OFFSET + (index * 3 + lane) * 4
                            ),
                            Some(0),
                            "{line}, {kind:?} after save/reload"
                        );
                    }
                }
                for offset in [0x44, 0x48, 0x4c] {
                    assert_eq!(
                        loaded.mac_state_misc_u32_at(offset),
                        Some(summary_sentinel),
                        "{line}, {kind:?} summary after save/reload"
                    );
                }
                std::fs::remove_file(path).expect("remove temporary zero-cohort city");
            }
        }
    }
}
