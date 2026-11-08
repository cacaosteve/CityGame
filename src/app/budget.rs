//! Budget service rows, tax rates, funding, and year-end cashflow helpers.

use super::*;

mod accounting;
mod bonds;
mod mac_building_removal;
mod mac_growth;
mod mac_monthly_scan;
mod mac_transit_route;
mod mac_transport_maintenance;
mod mac_zone_growth;
mod options;
mod rows;
pub(super) use super::mac_terrain_editing::mac_classify_terrain_tile;
pub(super) use accounting::*;
pub(super) use bonds::*;
pub(super) use mac_building_removal::*;
pub(super) use mac_growth::*;
pub(super) use mac_monthly_scan::*;
pub(super) use mac_transit_route::*;
pub(super) use mac_transport_maintenance::*;
pub(super) use mac_zone_growth::*;
pub(super) use options::*;
pub(super) use rows::*;

pub(super) const BUDGET_SERVICE_MISC_STRIDE: usize = 27;
pub(super) const BUDGET_SERVICE_CURRENT_FIELD: usize = 0;
pub(super) const BUDGET_SERVICE_FUNDING_FIELD: usize = 1;
pub(super) const BUDGET_SERVICE_YEAR_TO_DATE_FIELD: usize = 2;
pub(super) const BUDGET_SERVICE_COUNT: usize = 11;
pub(super) const BUDGET_TOTAL_COUNT: usize = 16;
pub(super) const BUDGET_TAX_COUNT: usize = 3;
/// MISC budget row 3 = ordinances book; row 4 = bond book (between taxes and services).
pub(super) const BUDGET_ORDINANCE_ROW: usize = 3;
pub(super) const BUDGET_BOND_ROW: usize = 4;
pub(super) const BUDGET_SERVICE_ROW_START: usize = 5;
pub(super) const TRANSPORT_BUDGET_INDICES: [usize; 6] = [5, 6, 7, 8, 9, 10];
pub(super) const TAX_MISC_OFFSET: usize = 0x077c;
/// Mac save cursor slot 479 starts its 16×27-dword budget row block.
const MAC_BUDGET_MISC_OFFSET: usize = 0x077c;
/// Mac MISC cursor slot 380 stores the eight CODE 14 RCI scan longwords.
pub(super) const MAC_RCI_COUNTERS_MISC_OFFSET: usize = 0x05f0;
/// Mac CODE 4 slot 915 persists `ffffe0c2`, also shared with military
/// capacity and phase-21 Dispatch Military availability.
const MAC_SPECIAL_RCI_EVENT_MISC_OFFSET: usize = 915 * 4;
pub(super) const TAX_MISC_STRIDE: usize = 27;
pub(super) const TAX_RATE_FIELD: usize = 1;
pub(super) const DEFAULT_TAX_RATE: u32 = 7;
pub(super) const MAX_TAX_RATE: u32 = 20;
/// Win95 `FUN_0040E250` reset table at `0x4e6730`.
/// Mac CODE 3 `export_143` reset table at `A5-0x51dc`; recovered values match PE.
/// Win95 budget cash divisor table `0x4E6AD8` tax-row entry (also used for ordinance $).
pub(super) const TAX_REVENUE_DIVISOR: u32 = 75;
/// Win95 signed WORD divisor table @ `0x004E6AD8` (`FUN_004189C0` summary / row walk).
/// `FUN_004734A0` reads a separate table at `0x004E9EB0` with the same values.
pub(super) const PE_BUDGET_ROW_DIVISORS: [i16; BUDGET_TOTAL_COUNT] = [
    75, 75, 75, 75, -100, -1, -1, -2, -4, -1, -1000, -500, -400, -250, -250, -250,
];
/// Mac CODE 4 `export_227` annual cash divisors. The recovered signed WORD
/// table matches the PE table, but Mac applies it to every row's persisted
/// signed raw accumulator, including ordinances and bonds.
const MAC_BUDGET_ROW_DIVISORS: [i16; BUDGET_TOTAL_COUNT] = [
    75, 75, 75, 75, -100, -1, -1, -2, -4, -1, -1000, -500, -400, -250, -250, -250,
];
pub(super) const BUDGET_TAX_STEP: i32 = 1;
pub(super) const BUDGET_FUNDING_STEP: i32 = 5;
pub(super) const BUDGET_BONDS_MISC_OFFSET: usize = 0x0018;
pub(super) const BUDGET_BOND_AMOUNT: i32 = 10_000;
/// Shared MISC `0610..06D7`: 50 per-bond rates. Both native loaders consume
/// the low WORD of each dword; Mac CODE 4 serializes its 50 signed rates here.
pub(super) const BOND_DATA_MISC_OFFSET: usize = 0x0610;
pub(super) const BOND_DATA_MAX_COUNT: usize = 50;
pub(super) const BOND_RATE_SCALE: u32 = 10_000;
/// Mac CODE 14 `export_210` phase 0 clears pointer slots 1–7 after the CODE 4
/// budget publisher and CODE 3 XMIC updater consume them. These are the six
/// weighted RCI counters plus the class-2 growth accumulator; slot 0 is not
/// part of that clear loop.
pub(super) fn mac_budget_clear_monthly_rci_scan_counters(city: &mut City) {
    for slot in 1..=7 {
        city.set_mac_rci_counter(slot, 0);
    }
}

pub(super) fn saturating_i32_from_i64(value: i64) -> i32 {
    value.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32
}

pub(super) fn budget_service_row_needs_live_defaults(
    city: &City,
    budget_index: usize,
    live_current: u32,
) -> bool {
    let Some(offset) = budget_service_misc_offset_for_city(city, budget_index, 0) else {
        return false;
    };
    let row = city.budget_misc_u32_values(offset, BUDGET_SERVICE_MISC_STRIDE);
    budget_row_values_are_unset(&row)
        || (live_current > 0
            && row.len() >= BUDGET_SERVICE_MISC_STRIDE
            && row[BUDGET_SERVICE_CURRENT_FIELD] == 0
            && row[BUDGET_SERVICE_YEAR_TO_DATE_FIELD] == 0)
}
pub(super) fn coverage_for_budget_service_index(
    coverage: CityServiceCoverage,
    index: usize,
) -> Option<ServiceCoverage> {
    match index {
        0 => Some(coverage.police),
        1 => Some(coverage.fire),
        2 => Some(coverage.health),
        3 | 4 => Some(coverage.education),
        index if TRANSPORT_BUDGET_INDICES.contains(&index) => Some(coverage.transit),
        _ => None,
    }
}
#[cfg(test)]
mod mac_growth_acceptance_tests {
    use super::*;

    #[test]
    fn mac_scan_acceptance_route_result_is_a_wrapping_signed_word() {
        let mut base = City::new_blank("/tmp/mac-route-word.city", "word route").unwrap();
        base.kind = CityFileKind::MacCity;
        base.set_building(40, 40, 0x70);
        base.set_zone(40, 40, 0x81);
        base.set_tile_bits(40, 40, TileBits::new(0xc0));
        base.set_building(40, 41, 0x20);
        base.set_zone(40, 42, 3);
        let seed = (1..1000)
            .find(|seed| {
                let mut city = base.clone();
                mac_budget_try_transit_route_at(
                    &mut city,
                    40,
                    40,
                    1,
                    1,
                    true,
                    &mut MacToolboxRng::new(*seed),
                ) == 2000
            })
            .expect("native route reaches an adjacent destination");
        for demand in [0_i16, -2000, 30767, 30768, i16::MAX, i16::MIN] {
            let mut city = base.clone();
            city.set_mac_state_misc_u32_at(
                super::super::RCI_DEMAND_MISC_OFFSET,
                u32::from(demand as u16),
            );
            let actual = mac_budget_try_transit_route_at(
                &mut city,
                40,
                40,
                1,
                1,
                true,
                &mut MacToolboxRng::new(seed),
            );
            assert_eq!(
                actual,
                i32::from(demand.wrapping_add(2000)),
                "demand {demand}"
            );
        }
    }

    #[test]
    fn mac_scan_acceptance_matches_original_instruction_fixture() {
        let fixture = include_str!("fixtures/mac_growth_acceptance_20261023.csv");
        let mut count = 0;
        for (line, row) in fixture.lines().enumerate().skip(1) {
            let v: Vec<i64> = row.split(',').map(|s| s.parse().unwrap()).collect();
            assert_eq!(v.len(), 14);
            let (class, coefficient, category) = (v[0] as u8, v[1] as u16, v[3] as usize);
            let mut rng = MacToolboxRng::new(v[6] as u32);
            let branch = mac_budget_choose_growth_branch(
                class,
                coefficient,
                category,
                v[2] as i16,
                v[5] as u8,
                v[4] != 0,
                &mut rng,
            );
            let actual = match branch {
                MacGrowthBranch::None => (0, -1),
                MacGrowthBranch::ClassZero { orientation } => (1, i64::from(orientation)),
                MacGrowthBranch::Church => (2, -1),
                MacGrowthBranch::Pattern => (3, ((category - 1) / 2) as i64),
                MacGrowthBranch::Generic => (4, category as i64),
            };
            assert_eq!(actual, (v[7], v[8]), "native CSV line {}", line + 1);
            assert_eq!(rng.seed(), v[10] as u32, "native CSV line {}", line + 1);
            let mut expected_rng = MacToolboxRng::new(v[6] as u32);
            for _ in 0..v[9] {
                expected_rng.random_i16();
            }
            assert_eq!(rng.seed(), expected_rng.seed());
            let mut counters = [0x7fff_fffe_u32; 8];
            if let Some(slot) = mac_growth_counter_slot(class, coefficient, category) {
                counters[slot] =
                    counters[slot].wrapping_add(mac_growth_counter_delta(coefficient) as u32);
            }
            assert_eq!(
                counters[category],
                v[11] as u32,
                "native category counter line {}",
                line + 1
            );
            assert_eq!(counters[7], v[12] as u32, "native slot 7 line {}", line + 1);
            count += 1;
        }
        assert_eq!(count, 20_130);
    }

    #[test]
    fn mac_scan_acceptance_church_writer_has_native_bounds_and_rotation_markers() {
        for rotation in 0..4 {
            let mut city = City::new_blank("/tmp/mac-church-write.city", "church").unwrap();
            city.saved_rotation = rotation as u8;
            // Use the saved rotation field, not the renderer's view compass.
            city.set_view_rotation(((rotation + 1) & 3) as u8);
            for x in 40..=41 {
                for y in 39..=40 {
                    city.set_zone(x, y, 0x75);
                    city.set_tile_bits(x, y, TileBits::new(0x09));
                }
            }
            mac_budget_place_church(&mut city, 40, 40);
            let corners = [0x10_u8, 0x20, 0x40, 0x80];
            for (i, (x, y)) in [(40, 39), (41, 39), (41, 40), (40, 40)]
                .into_iter()
                .enumerate()
            {
                assert_eq!(city.building_at(x, y), 0xf7);
                assert_eq!(
                    city.zone_at(x, y).unwrap().raw_bits(),
                    corners[(i + rotation as usize) & 3]
                );
                assert_eq!(city.tile_bits_at(x, y).unwrap().raw_bits(), 0xe9);
            }
        }
        for (x, y, allowed) in [
            (0, 40, false),
            (40, 0, false),
            (127, 40, false),
            (40, 127, false),
            (1, 1, true),
            (126, 126, true),
        ] {
            let mut city = City::new_blank("/tmp/mac-church-bounds.city", "bounds").unwrap();
            mac_budget_place_church(&mut city, x, y);
            assert_eq!(city.building_at(x, y) == 0xf7, allowed, "{x},{y}");
        }
    }
}

#[cfg(test)]
mod mac_growth_xval_tests {
    use super::*;

    #[test]
    fn mac_growth_xval_generic_gate_uses_land_value_at_the_candidate_cell() {
        for (land_value, old_traffic, expected_draws) in [(0x60, 0, 2), (0, 255, 1)] {
            let mut city =
                super::super::tests::blank_city_layout_fixture("mac-growth-xval.city", "XVAL gate");
            city.set_zone(64, 64, 0x82);
            city.set_building(64, 64, 0x8c);
            let mut land = vec![0; CITY_TILES];
            land[64 * CITY_HEIGHT + 64] = land_value;
            land[32 * CITY_HEIGHT + 32] = 255 - land_value;
            city.sync_land_value_from_slice(&land);
            city.set_traffic_density(32, 32, old_traffic);
            let traffic = city.traffic_density_slice().unwrap().to_vec();
            let seed = (1..1000)
                .find(|seed| MacToolboxRng::new(*seed).random_u16() >= 2000)
                .unwrap();
            let mut expected = MacToolboxRng::new(seed);
            for _ in 0..expected_draws {
                expected.random_i16();
            }
            let mut rng = MacToolboxRng::new(seed);
            mac_budget_accumulate_monthly_rci_scan_partition(&mut city, 0, 0, &mut rng);
            assert_eq!(
                rng.seed(),
                expected.seed(),
                "land={land_value}, traffic={old_traffic}"
            );
            assert_eq!(city.building_at(64, 64), 0x8c);
            assert_eq!(city.traffic_density_slice().unwrap(), traffic);
        }
    }

    #[test]
    fn mac_growth_xval_small_residential_variant_uses_aligned_land_value() {
        for value in [0_u8, 63, 64, 127, 128, 255] {
            let mut city = super::super::tests::blank_city_layout_fixture(
                "mac-growth-xval-variant.city",
                "XVAL variant",
            );
            let mut land = vec![0; CITY_TILES];
            land[64 * CITY_HEIGHT + 66] = value;
            // Conflicting values at the unfurled odd tile and old /2 position
            // distinguish the native coarse cell from two coordinate mistakes.
            land[65 * CITY_HEIGHT + 67] = 255 - value;
            land[32 * CITY_HEIGHT + 33] = 255 - value;
            city.sync_land_value_from_slice(&land);
            city.set_traffic_density(32, 33, 255 - value);
            city.set_zone(65, 67, 0x72);
            let traffic = city.traffic_density_slice().unwrap().to_vec();
            let mut expected_rng = MacToolboxRng::new(12345);
            let draw = expected_rng.random_u16();
            let expected_tile = 0x70 + (value / 64).min(2) * 4 + (draw & 3) as u8;
            let mut rng = MacToolboxRng::new(12345);
            mac_budget_place_growth_pattern(&mut city, 65, 67, 1, 0, &mut rng);
            assert_eq!(
                city.building_at(65, 67),
                expected_tile,
                "land value {value}"
            );
            assert_eq!(city.zone_at(65, 67).unwrap().raw_bits(), 0xf2);
            assert_eq!(rng.seed(), expected_rng.seed());
            assert_eq!(city.traffic_density_slice().unwrap(), traffic);
        }
    }
}
