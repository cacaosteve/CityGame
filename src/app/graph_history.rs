//! Shared XGRP history layout, capture, cadence and sampling model.
//!
//! Saved series layout, published-state reads, population fallbacks, section
//! rolling and sample ordering retain their existing behavior. RCI owns history
//! publication and calendar dispatch; dialogs own graph drawing and input.

use super::{
    CityServiceOutcomes, PopulationStats, budget_service_built_counts_for_city,
    pe_attitude_developed_tile_count, pe_zone_pop_row_people, pe_zone_pops_scaled_population,
    unemployment_percent_for_city,
};
use crate::city::{City, pe_arco_tile_pop_bonus, pe_occupancy_arco_footprints};

pub(super) const XGRP_CHUNK_NAME: &str = "XGRP";

const XGRP_SERIES: usize = 16;

pub(super) const XGRP_MONTH_SAMPLES: usize = 12;

pub(super) const XGRP_HALF_YEAR_SAMPLES: usize = 20;

pub(super) const XGRP_FIVE_YEAR_SAMPLES: usize = 20;

pub(super) const XGRP_SAMPLES_PER_SERIES: usize =
    XGRP_MONTH_SAMPLES + XGRP_HALF_YEAR_SAMPLES + XGRP_FIVE_YEAR_SAMPLES;

pub(super) const XGRP_VALUES: usize = XGRP_SERIES * XGRP_SAMPLES_PER_SERIES;

pub(super) const XGRP_MONTH_OFFSET: usize = 0;

/// CODE 8 export 237 aggregates six scan counters into the total and three
/// paired values. CODE 14 combines those with the signed arcology total.
fn mac_xgrp_population_samples(total: u32, counters: [u32; 6], arcology: u32) -> [u32; 4] {
    let arcology_signed = arcology as i32;
    [
        total.wrapping_mul(10).wrapping_add(arcology),
        counters[0]
            .wrapping_add(counters[1])
            .wrapping_mul(10)
            .wrapping_add((arcology_signed / 2) as u32),
        counters[2]
            .wrapping_add(counters[3])
            .wrapping_mul(10)
            .wrapping_add((arcology_signed / 4) as u32),
        counters[4]
            .wrapping_add(counters[5])
            .wrapping_mul(10)
            .wrapping_add((arcology_signed / 4) as u32),
    ]
}

/// Publish rows 0..3 only where the Mac growth scan supplies the counters.
pub(super) fn mac_publish_xgrp_population(city: &mut City) {
    let counters = std::array::from_fn(|index| city.mac_rci_counter(index + 1));
    // CODE 8 export 237 derives the total from the six scan counters before
    // CODE 14 publishes the history. Recompute it here instead of relying on
    // saved counter 0: that slot overlaps PE state in Win-layout cities, where
    // Mac-owned counters live separately in runtime state.
    let total = counters.into_iter().fold(0_u32, u32::wrapping_add);
    let latest = mac_xgrp_population_samples(total, counters, city.mac_arcology_resident_total());
    let age = city.city_age_days().unwrap_or(0);
    let mut values = city.chunk_u32_values(XGRP_CHUNK_NAME, 0, XGRP_VALUES);
    values.resize(XGRP_VALUES, 0);
    mac_xgrp_roll_rows(
        &mut values,
        0,
        &latest,
        ((age / 25) % 12) as u16,
        (age / 300) as u16,
    );
    city.set_chunk_u32_values(XGRP_CHUNK_NAME, 0, &values);
}

/// CODE 14 `0x26d18..0x26d90`: newest XGRP traffic, pollution, land and
/// crime values. The WORD count is signed before division; both final
/// divisions are unsigned. A zero divisor represents an original CPU trap,
/// not a value to publish.
pub(super) fn mac_xgrp_environment_samples(
    totals: [u32; 4],
    road_counts: [u32; 3],
    developed_word: u16,
) -> Option<[u32; 4]> {
    let traffic_divisor = road_counts[0]
        .wrapping_add(road_counts[1])
        .wrapping_add(1)
        .wrapping_add(road_counts[2]);
    let environment_divisor = (i32::from(developed_word as i16) / 4 + 1) as u32;
    Some([
        totals[0].checked_div(traffic_divisor)?,
        totals[1].checked_div(environment_divisor)?,
        totals[2].checked_div(environment_divisor)?,
        totals[3].checked_div(environment_divisor)?,
    ])
}

/// Publish only Mac XGRP environment rows 4..7 with CODE 14's monthly,
/// half-year and five-year rollover. Other rows have separate producers.
pub(super) fn mac_xgrp_roll_environment(
    values: &mut [u32],
    latest: [u32; 4],
    month_index: u16,
    elapsed_year: u16,
) -> bool {
    mac_xgrp_roll_rows(values, 4, &latest, month_index, elapsed_year)
}

fn mac_xgrp_roll_rows(
    values: &mut [u32],
    first_series: usize,
    latest: &[u32],
    month_index: u16,
    elapsed_year: u16,
) -> bool {
    if values.len() < (first_series + latest.len()) * XGRP_SAMPLES_PER_SERIES {
        return false;
    }
    for (index, &sample) in latest.iter().enumerate() {
        let start = (index + first_series) * XGRP_SAMPLES_PER_SERIES;
        values.copy_within(start..start + 11, start + 1);
        values[start] = sample;
        if month_index == 0 || month_index == 6 {
            values.copy_within(start + 12..start + 31, start + 13);
            values[start + 12] = sample;
        }
        if month_index == 0 && (elapsed_year as i16) % 5 == 0 {
            values.copy_within(start + 32..start + 51, start + 33);
            values[start + 32] = sample;
        }
    }
    true
}

/// Mac CODE 14 `0x26dd6..0x26df6`: publish service rows as DWORD
/// `100 - native summary`, retaining original wrapping behavior.
pub(super) fn mac_publish_xgrp_services(city: &mut City) {
    let latest = [
        100_u32.wrapping_sub(city.mac_power_service_summary()),
        100_u32.wrapping_sub(city.mac_water_service_summary()),
    ];
    let age = city.city_age_days().unwrap_or(0);
    let mut values = city.chunk_u32_values(XGRP_CHUNK_NAME, 0, XGRP_VALUES);
    values.resize(XGRP_VALUES, 0);
    mac_xgrp_roll_rows(
        &mut values,
        8,
        &latest,
        ((age / 25) % 12) as u16,
        (age / 300) as u16,
    );
    city.set_chunk_u32_values(XGRP_CHUNK_NAME, 0, &values);
}

/// CODE 14 `0x26df6..0x26e0e`: copy the saved Mac cohort metrics into XGRP
/// rows 10 and 11 (`fffff02e` then `fffff02a`) before the shared history roll.
/// Both fields occupy shared MISC slots, so this publication is storage-layout
/// independent when the Mac engine is selected.
pub(super) fn mac_publish_xgrp_cohort_metrics(city: &mut City) {
    let [Some(row_10_metric), Some(row_11_metric)] = [
        city.mac_state_misc_u32_at(0x0048),
        city.mac_state_misc_u32_at(0x004c),
    ] else {
        return;
    };
    let age = city.city_age_days().unwrap_or(0);
    let mut values = city.chunk_u32_values(XGRP_CHUNK_NAME, 0, XGRP_VALUES);
    values.resize(XGRP_VALUES, 0);
    mac_xgrp_roll_rows(
        &mut values,
        10,
        &[row_10_metric, row_11_metric],
        ((age / 25) % 12) as u16,
        (age / 300) as u16,
    );
    city.set_chunk_u32_values(XGRP_CHUNK_NAME, 0, &values);
}

/// CODE 14 `0x26e46..0x26e6c`: national population, value, and the
/// sign-extended national tax WORD, sampled after monthly economy drift.
pub(super) fn mac_publish_xgrp_national(city: &mut City) {
    let latest = [
        city.pe_dw_national_population,
        city.pe_dw_national_value,
        i32::from(city.pe_w_national_tax as i16) as u32,
    ];
    let age = city.city_age_days().unwrap_or(0);
    let mut values = city.chunk_u32_values(XGRP_CHUNK_NAME, 0, XGRP_VALUES);
    values.resize(XGRP_VALUES, 0);
    mac_xgrp_roll_rows(
        &mut values,
        13,
        &latest,
        ((age / 25) % 12) as u16,
        (age / 300) as u16,
    );
    city.set_chunk_u32_values(XGRP_CHUNK_NAME, 0, &values);
}

/// CODE 14 `0x26e0e..0x26e46`: the ratio is also stored as XGRP row 12.
/// The caller only provides a sample when its native counter inputs exist.
pub(super) fn mac_publish_xgrp_riot_ratio(city: &mut City, ratio: u32) {
    let age = city.city_age_days().unwrap_or(0);
    let mut values = city.chunk_u32_values(XGRP_CHUNK_NAME, 0, XGRP_VALUES);
    values.resize(XGRP_VALUES, 0);
    mac_xgrp_roll_rows(
        &mut values,
        12,
        &[ratio],
        ((age / 25) % 12) as u16,
        (age / 300) as u16,
    );
    city.set_chunk_u32_values(XGRP_CHUNK_NAME, 0, &values);
}

/// Publish Mac XGRP environment rows 4..7. The remaining series are published
/// by their separate Mac phase-21 workers; graph maxima are presentation state.
pub(super) fn mac_publish_xgrp_environment(city: &mut City) -> bool {
    let Some(developed) = city.mac_developed_cells() else {
        return false;
    };
    let totals = [
        city.mac_state_misc_u32_at(super::MISC_CITY_TRAFFIC_OFFSET),
        city.mac_state_misc_u32_at(super::MISC_CITY_POLLUTION_OFFSET),
        city.mac_state_misc_u32_at(super::MISC_LAND_VALUE_OFFSET),
        city.mac_state_misc_u32_at(super::MISC_CITY_CRIME_OFFSET),
    ];
    let [Some(traffic), Some(pollution), Some(land), Some(crime)] = totals else {
        return false;
    };
    let mut roads = [0; 3];
    for (index, row) in (10..=12).enumerate() {
        let Some(offset) = super::budget::budget_row_misc_offset_for_city(
            city,
            row,
            super::budget::BUDGET_SERVICE_CURRENT_FIELD,
        ) else {
            return false;
        };
        let Some(value) = city.budget_misc_u32_at(offset) else {
            return false;
        };
        roads[index] = value;
    }
    let Some(latest) =
        mac_xgrp_environment_samples([traffic, pollution, land, crime], roads, developed)
    else {
        return false;
    };
    let age = city.city_age_days().unwrap_or(0);
    let month = ((age / 25) % 12) as u16;
    let year = (age / 300) as u16;
    let mut values = city.chunk_u32_values(XGRP_CHUNK_NAME, 0, XGRP_VALUES);
    values.resize(XGRP_VALUES, 0);
    if !mac_xgrp_roll_environment(&mut values, latest, month, year) {
        return false;
    }
    city.set_chunk_u32_values(XGRP_CHUNK_NAME, 0, &values);
    true
}

/// CODE 14's phase-2 `ffffbb50` census counts each full-map cell whose
/// building is at least 0x1d or whose low zone nibble is nonzero.
#[allow(dead_code)]
fn mac_xgrp_is_developed_cell(building: u8, zone_byte: u8) -> bool {
    building >= 0x1d || zone_byte & 0x0f != 0
}

#[allow(dead_code)]
pub(super) fn mac_xgrp_count_developed_cells(city: &City) -> u16 {
    let mut count = 0_u16;
    for y in 0..crate::city::CITY_HEIGHT {
        for x in 0..crate::city::CITY_WIDTH {
            let zone = city.zone_at(x, y).map_or(0, |zone| zone.zone_id());
            if mac_xgrp_is_developed_cell(city.building_at(x, y), zone) {
                count = count.wrapping_add(1);
            }
        }
    }
    count
}

pub(super) const XGRP_HALF_YEAR_OFFSET: usize = XGRP_MONTH_SAMPLES;

pub(super) const XGRP_FIVE_YEAR_OFFSET: usize = XGRP_MONTH_SAMPLES + XGRP_HALF_YEAR_SAMPLES;

/// Win95 Graphs window radios: one-year / ten-year / hundred-year.
const GRAPH_SCALE_COUNT: usize = 3;

/// Win95 Graphs window time-range radios (`TEXT_USA.DAT` one/ten/hundred-year).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum GraphScale {
    #[default]
    Year1,
    Years10,
    Years100,
}

impl GraphScale {
    pub(super) const ALL: [Self; GRAPH_SCALE_COUNT] = [Self::Year1, Self::Years10, Self::Years100];

    pub(super) fn label(self) -> &'static str {
        match self {
            Self::Year1 => "1 Year",
            Self::Years10 => "10 Years",
            Self::Years100 => "100 Years",
        }
    }

    pub(super) fn xgrp_offset(self) -> usize {
        match self {
            Self::Year1 => XGRP_MONTH_OFFSET,
            Self::Years10 => XGRP_HALF_YEAR_OFFSET,
            Self::Years100 => XGRP_FIVE_YEAR_OFFSET,
        }
    }

    fn sample_count(self) -> usize {
        match self {
            Self::Year1 => XGRP_MONTH_SAMPLES,
            Self::Years10 => XGRP_HALF_YEAR_SAMPLES,
            Self::Years100 => XGRP_FIVE_YEAR_SAMPLES,
        }
    }
}

/// PE `0x413e80..0x413f5a`: growth slot 0 and pZonePops, with
/// the signed arcology WORD census bonus and DWORD wrapping.
pub(super) fn pe_xgrp_population_values(city: &City, slot0: i32) -> [u32; 4] {
    let tile_sum: i32 = (251..=254)
        .map(|tile| i32::from(city.pe_dw_tile_count[tile] as i16))
        .sum();
    let footprints = tile_sum / 16;
    let bonus = if footprints > 140 {
        (footprints - 140).wrapping_mul(20_000)
    } else {
        0
    };
    let arco = (city.pe_dw_arcology_population() as i32).wrapping_add(bonus);
    let rows = city.pe_zone_pops();
    [
        slot0.wrapping_mul(10).wrapping_add(arco) as u32,
        rows[0].wrapping_mul(10).wrapping_add(arco / 2) as u32,
        rows[1].wrapping_mul(10).wrapping_add(arco / 4) as u32,
        rows[2].wrapping_mul(10).wrapping_add(arco / 4) as u32,
    ]
}

pub(super) fn xgrp_history_latest_values(
    city: &City,
    population: PopulationStats,
    _outcomes: CityServiceOutcomes,
) -> [u32; XGRP_SERIES] {
    // Win95 `UpdateGraphData` (`0x413e50`), GRP_CITYSIZE..GRP_FEDRATE.
    let arco_load = city
        .pe_dw_arcology_population()
        .saturating_add(pe_arco_tile_pop_bonus(pe_occupancy_arco_footprints(city)));
    let (city_size, residential, commercial, industrial) = if city.pe_zone_pops_ready() {
        let rows = city.pe_zone_pops();
        (
            city.pe_dw_city_population().saturating_add(arco_load),
            pe_zone_pop_row_people(rows[0]).saturating_add(arco_load / 2),
            pe_zone_pop_row_people(rows[1]).saturating_add(arco_load / 4),
            pe_zone_pop_row_people(rows[2]).saturating_add(arco_load / 4),
        )
    } else {
        let zone_pop = pe_zone_pops_scaled_population(population);
        (
            zone_pop.saturating_add(arco_load),
            population
                .residential_population
                .saturating_add(arco_load / 2),
            population
                .commercial_population
                .saturating_add(arco_load / 4),
            population
                .industrial_population
                .saturating_add(arco_load / 4),
        )
    };
    // Generic fallback estimates for series 4..7. The live Win95 graph
    // publisher replaces them with the exact scores stored in its graph rows.
    let built = budget_service_built_counts_for_city(city);
    let traffic_divisor = built
        .roads
        .saturating_add(built.highways)
        .saturating_add(built.bridges)
        .saturating_add(1);
    let developed_divisor = pe_attitude_developed_tile_count(city) / 4 + 1;
    // `UpdateGraphData` reads the already-published `dwPowerUsedPercentage`
    // BSS at `0x4caa50`; day-1 `FUN_00467860` owns its calculation and CRT use.
    // Graph publication must not recompute the estimate or advance that stream.
    let power_used = u32::from(city.pe_power_used_percent());
    // PE `UpdateGraphData` @ `0x413fa4`/`0x413fd2`/`0x413fdd`/`0x413fee` /
    // `0x414075`/`0x414082` reads traffic/pollution/land/crime/EQ/LE BSS.
    [
        city_size,
        residential,
        commercial,
        industrial,
        city.pe_traffic_count() / traffic_divisor,
        city.pe_dw_city_pollution / developed_divisor,
        city.pe_dw_city_land_value / developed_divisor,
        city.pe_dw_city_crime / developed_divisor,
        100_u32.wrapping_sub(power_used),
        // PE `UpdateGraphData` @ `0x414065`: `100 - [0x4cc4b8]` coverage BSS.
        100_u32.wrapping_sub(city.pe_water_coverage_percent()),
        city.pe_eq_life_average(),
        city.pe_eq_education_average(),
        unemployment_percent_for_city(city, population),
        city.pe_dw_national_value,
        city.pe_dw_national_population,
        u32::from(city.pe_w_national_tax),
    ]
}

pub(super) fn roll_xgrp_history_section(
    values: &mut [u32],
    section_offset: usize,
    section_len: usize,
    latest_values: &[u32; XGRP_SERIES],
) {
    // Win95 `UpdateGraphData` (`0x413e50`): shift samples toward higher
    // indices and write the newest value at the section start.
    for (series_index, latest) in latest_values.iter().copied().enumerate() {
        let start = series_index * XGRP_SAMPLES_PER_SERIES + section_offset;
        let end = start + section_len;
        if end > values.len() || section_len == 0 {
            continue;
        }
        values.copy_within(start..end - 1, start + 1);
        values[start] = latest;
    }
}

/// Win95 graph worker tests the age-derived month WORD for 0 or 6.
pub(super) fn pe_xgrp_rolls_half_year(city: &City) -> bool {
    matches!(city.pe_w_calendar_month_mod(), 0 | 6)
}

/// PE tests age-derived month zero and the signed local-year WORD modulo five.
pub(super) fn pe_xgrp_rolls_five_year(city: &City) -> bool {
    city.pe_w_calendar_month_mod() == 0 && (city.pe_w_local_fame_base() as i16) % 5 == 0
}

pub(super) fn xgrp_samples_oldest_first(
    values: &[u32],
    series_index: usize,
    scale: GraphScale,
) -> Vec<u32> {
    let count = scale.sample_count();
    let start = series_index * XGRP_SAMPLES_PER_SERIES + scale.xgrp_offset();
    let end = start + count;
    if end > values.len() {
        return vec![0; count];
    }
    let mut samples = values[start..end].to_vec();
    samples.reverse();
    samples
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mac_population_history_uses_engine_counters_on_win_layout() {
        let mut city =
            City::new_blank("/tmp/mac-xgrp-win-layout.sc2", "Mixed").expect("blank city");
        assert_eq!(city.kind, crate::city::CityFileKind::WinSc2);
        city.prepare_mac_fidelity_from_saved_city();
        for (index, count) in [11_i32, -4, -3, 1, 2, 5].into_iter().enumerate() {
            city.set_mac_rci_counter(index + 1, count as u32);
        }
        // The stored total overlaps PE-owned data on this layout. The native
        // phase computes the total from counters 1..=6 before chart sampling.
        city.set_mac_rci_counter(0, 0x1234_5678);
        city.set_mac_arcology_resident_total(21);
        for index in 0..6 {
            city.set_misc_u32_at(0x05f4 + index * 4, 1000 + index as u32);
        }
        let untouched = 0x1234_5678;
        let mut history = vec![untouched; XGRP_VALUES];
        city.set_chunk_u32_values(XGRP_CHUNK_NAME, 0, &history);

        mac_publish_xgrp_population(&mut city);
        history = city.chunk_u32_values(XGRP_CHUNK_NAME, 0, XGRP_VALUES);

        assert_eq!(history[0 * XGRP_SAMPLES_PER_SERIES], 141);
        assert_eq!(history[1 * XGRP_SAMPLES_PER_SERIES], 80);
        assert_eq!(history[2 * XGRP_SAMPLES_PER_SERIES], (-15_i32) as u32);
        assert_eq!(history[3 * XGRP_SAMPLES_PER_SERIES], 75);
        assert!(
            history[4 * XGRP_SAMPLES_PER_SERIES..]
                .iter()
                .all(|&value| value == untouched),
            "unexpected row-4+ values: {:?}",
            &history[4 * XGRP_SAMPLES_PER_SERIES..4 * XGRP_SAMPLES_PER_SERIES + 16]
        );
        for index in 0..6 {
            assert_eq!(
                city.misc_u32_at(0x05f4 + index * 4),
                Some(1000 + index as u32)
            );
        }
    }

    #[test]
    fn mac_service_history_matches_original_68020() {
        let fixture = include_str!("../../tests/fixtures/simulation/mac/mac_xgrp_services.csv");
        let mut count = 0;
        for line in fixture.lines().skip(1) {
            let v: Vec<_> = line.split(',').collect();
            let month: u16 = v[0].parse().unwrap();
            let year: u16 = v[1].parse().unwrap();
            let seed: u32 = v[2].parse().unwrap();
            let decode = |hex: &str| -> Vec<u32> {
                (0..hex.len() / 8)
                    .map(|i| u32::from_str_radix(&hex[i * 8..i * 8 + 8], 16).unwrap())
                    .collect()
            };
            let mut values = vec![0xdeadbeef; XGRP_VALUES];
            values[4 * 52..10 * 52].copy_from_slice(&decode(v[3]));
            let environment = mac_xgrp_environment_samples(
                [3000 + seed, 4000 + seed, 5000 + seed, 6000 + seed],
                [3 + seed, 5 + seed, 7 + seed],
                (12 + seed) as u16,
            )
            .unwrap();
            assert!(mac_xgrp_roll_environment(
                &mut values,
                environment,
                month,
                year
            ));
            let power = 25 + seed * 126;
            let water = if seed == 0 { 61 } else { u32::MAX };
            assert!(mac_xgrp_roll_rows(
                &mut values,
                8,
                &[100_u32.wrapping_sub(power), 100_u32.wrapping_sub(water)],
                month,
                year
            ));
            assert_eq!(
                &values[4 * 52..10 * 52],
                decode(v[4]),
                "month {month}, year {year}, seed {seed}"
            );
            assert!(
                values[..4 * 52]
                    .iter()
                    .chain(&values[10 * 52..])
                    .all(|&v| v == 0xdeadbeef)
            );
            count += 1;
        }
        assert_eq!(count, 40);
    }

    #[test]
    fn mac_population_history_matches_original_68020_rollovers() {
        let fixture = include_str!("../../tests/fixtures/simulation/mac/mac_xgrp_population.csv");
        let mut count = 0;
        for line in fixture.lines().skip(1) {
            let v: Vec<_> = line.split(',').collect();
            let month: u16 = v[0].parse().unwrap();
            let year: u16 = v[1].parse().unwrap();
            let seed: u32 = v[2].parse().unwrap();
            let decode = |hex: &str| -> Vec<u32> {
                (0..hex.len() / 8)
                    .map(|i| u32::from_str_radix(&hex[i * 8..i * 8 + 8], 16).unwrap())
                    .collect()
            };
            let counters = if seed == 0 {
                [3, 4, 5, 6, 7, 8]
            } else {
                [u32::MAX, 0x8000_0000, 1, 7, u32::MAX - 1, 5]
            };
            let total = counters.into_iter().fold(0_u32, u32::wrapping_add);
            let arcology = if seed == 0 { 11 } else { (-5_i32) as u32 };
            let mut values = vec![0xdeadbeef; XGRP_VALUES];
            values[..4 * 52].copy_from_slice(&decode(v[3]));
            let latest = mac_xgrp_population_samples(total, counters, arcology);
            assert!(mac_xgrp_roll_rows(&mut values, 0, &latest, month, year));
            assert_eq!(
                &values[..4 * 52],
                decode(v[4]),
                "month {month}, year {year}, seed {seed}"
            );
            assert!(values[4 * 52..].iter().all(|&value| value == 0xdeadbeef));
            count += 1;
        }
        assert_eq!(count, 40);
    }

    #[test]
    fn mac_riot_history_matches_original_68020_rollovers() {
        let fixture = include_str!("../../tests/fixtures/simulation/mac/mac_xgrp_riot.csv");
        let mut count = 0;
        for line in fixture.lines().skip(1) {
            let v: Vec<_> = line.split(',').collect();
            let month: u16 = v[0].parse().unwrap();
            let year: u16 = v[1].parse().unwrap();
            let seed: u32 = v[2].parse().unwrap();
            let decode = |hex: &str| -> Vec<u32> {
                (0..hex.len() / 8)
                    .map(|i| u32::from_str_radix(&hex[i * 8..i * 8 + 8], 16).unwrap())
                    .collect()
            };
            let mut values = vec![0xdeadbeef; XGRP_VALUES];
            values[12 * 52..13 * 52].copy_from_slice(&decode(v[3]));
            let (total, class_two): (u32, u32) = if seed == 0 { (1, 3) } else { (0, 0x4000_0000) };
            let ratio = class_two.wrapping_mul(100) / total.wrapping_add(class_two).wrapping_add(1);
            assert!(mac_xgrp_roll_rows(&mut values, 12, &[ratio], month, year));
            assert_eq!(
                &values[12 * 52..13 * 52],
                decode(v[4]),
                "month {month}, year {year}, seed {seed}"
            );
            assert!(
                values[..12 * 52]
                    .iter()
                    .chain(&values[13 * 52..])
                    .all(|&value| value == 0xdeadbeef)
            );
            count += 1;
        }
        assert_eq!(count, 40);
    }

    #[test]
    fn mac_national_history_matches_original_68020_rollovers() {
        let fixture = include_str!("../../tests/fixtures/simulation/mac/mac_xgrp_national.csv");
        let mut count = 0;
        for line in fixture.lines().skip(1) {
            let v: Vec<_> = line.split(',').collect();
            let month: u16 = v[0].parse().unwrap();
            let year: u16 = v[1].parse().unwrap();
            let seed: u32 = v[2].parse().unwrap();
            let decode = |hex: &str| -> Vec<u32> {
                (0..hex.len() / 8)
                    .map(|i| u32::from_str_radix(&hex[i * 8..i * 8 + 8], 16).unwrap())
                    .collect()
            };
            let mut values = vec![0xdeadbeef; XGRP_VALUES];
            values[13 * 52..16 * 52].copy_from_slice(&decode(v[3]));
            let tax = if seed == 0 { 3 } else { u32::MAX };
            assert!(mac_xgrp_roll_rows(
                &mut values,
                13,
                &[4_000_000 + seed, 3_000_000 + seed, tax],
                month,
                year
            ));
            assert_eq!(
                &values[13 * 52..16 * 52],
                decode(v[4]),
                "month {month}, year {year}, seed {seed}"
            );
            assert!(values[..13 * 52].iter().all(|&value| value == 0xdeadbeef));
            count += 1;
        }
        assert_eq!(count, 40);
    }

    #[test]
    fn mac_national_publisher_works_for_both_city_layouts() {
        for kind in [
            crate::city::CityFileKind::MacCity,
            crate::city::CityFileKind::WinSc2,
        ] {
            let mut city = City::new_blank("/tmp/mac-national-history.sc2", "History").unwrap();
            city.kind = kind;
            city.initialize_new_city_founding_year(1900);
            city.set_saved_date_and_day(
                crate::city::CityDate {
                    year: 1905,
                    month: 1,
                },
                21,
            );
            city.pe_dw_national_population = 4_000_001;
            city.pe_dw_national_value = 3_000_001;
            city.pe_w_national_tax = u16::MAX;
            let before: Vec<u32> = (0..XGRP_VALUES as u32).collect();
            city.set_chunk_u32_values(XGRP_CHUNK_NAME, 0, &before);
            mac_publish_xgrp_national(&mut city);
            let after = city.chunk_u32_values(XGRP_CHUNK_NAME, 0, XGRP_VALUES);
            assert_eq!(&after[..13 * 52], &before[..13 * 52]);
            for (index, sample) in [4_000_001, 3_000_001, u32::MAX].into_iter().enumerate() {
                let start = (13 + index) * 52;
                assert_eq!(
                    (after[start], after[start + 12], after[start + 32]),
                    (sample, sample, sample)
                );
                assert_eq!(
                    (after[start + 1], after[start + 13], after[start + 33]),
                    (before[start], before[start + 12], before[start + 32])
                );
            }
        }
    }

    #[test]
    fn mac_service_publisher_preserves_other_rows_and_win_misc() {
        for kind in [
            crate::city::CityFileKind::MacCity,
            crate::city::CityFileKind::WinSc2,
        ] {
            let mut city = City::new_blank("/tmp/mac-xgrp-services.sc2", "History").unwrap();
            city.kind = kind;
            city.initialize_new_city_founding_year(1900);
            city.set_saved_date_and_day(
                crate::city::CityDate {
                    year: 1905,
                    month: 1,
                },
                21,
            );
            if kind != crate::city::CityFileKind::MacCity {
                city.set_misc_u32_at(0x1060, 0x12345678);
                city.set_misc_u32_at(0x1064, 0x87654321);
            }
            city.set_mac_power_service_summary(151);
            city.set_mac_water_service_summary(u32::MAX);
            let before: Vec<u32> = (0..XGRP_VALUES as u32).collect();
            city.set_chunk_u32_values(XGRP_CHUNK_NAME, 0, &before);
            mac_publish_xgrp_services(&mut city);
            let after = city.chunk_u32_values(XGRP_CHUNK_NAME, 0, XGRP_VALUES);
            for (index, sample) in [100_u32.wrapping_sub(151), 101].into_iter().enumerate() {
                let start = (8 + index) * 52;
                assert_eq!(
                    (after[start], after[start + 12], after[start + 32]),
                    (sample, sample, sample)
                );
                assert_eq!(
                    (after[start + 1], after[start + 13], after[start + 33]),
                    (before[start], before[start + 12], before[start + 32])
                );
            }
            assert_eq!(&after[..8 * 52], &before[..8 * 52]);
            assert_eq!(&after[10 * 52..], &before[10 * 52..]);
            if kind != crate::city::CityFileKind::MacCity {
                assert_eq!(city.misc_u32_at(0x1060), Some(0x12345678));
                assert_eq!(city.misc_u32_at(0x1064), Some(0x87654321));
            }
        }
    }

    #[test]
    fn mac_cohort_metric_history_uses_shared_fields_on_both_layouts() {
        for kind in [
            crate::city::CityFileKind::MacCity,
            crate::city::CityFileKind::WinSc2,
        ] {
            let mut city =
                City::new_blank("/tmp/mac-xgrp-cohort-metrics.sc2", "History").expect("blank city");
            city.kind = kind;
            city.set_mac_state_misc_u32_at(0x0048, 0x1234_5678);
            city.set_mac_state_misc_u32_at(0x004c, 0x8765_4321);
            let before: Vec<u32> = (0..XGRP_VALUES as u32).collect();
            city.set_chunk_u32_values(XGRP_CHUNK_NAME, 0, &before);

            mac_publish_xgrp_cohort_metrics(&mut city);

            let after = city.chunk_u32_values(XGRP_CHUNK_NAME, 0, XGRP_VALUES);
            assert_eq!(&after[..10 * 52], &before[..10 * 52], "{kind:?}");
            for (index, sample) in [0x1234_5678, 0x8765_4321].into_iter().enumerate() {
                let start = (10 + index) * 52;
                assert_eq!(after[start], sample, "{kind:?}");
                assert_eq!(after[start + 1], before[start], "{kind:?}");
            }
            assert_eq!(&after[12 * 52..], &before[12 * 52..], "{kind:?}");
            assert_eq!(city.mac_state_misc_u32_at(0x0048), Some(0x1234_5678));
            assert_eq!(city.mac_state_misc_u32_at(0x004c), Some(0x8765_4321));
        }
    }

    #[test]
    fn mac_environment_publisher_uses_latched_state_for_both_city_layouts() {
        for kind in [
            crate::city::CityFileKind::MacCity,
            crate::city::CityFileKind::WinSc2,
        ] {
            let mut city = City::new_blank("/tmp/mac-xgrp-publish.sc2", "History").unwrap();
            city.kind = kind;
            city.initialize_new_city_founding_year(1900);
            city.set_saved_date_and_day(
                crate::city::CityDate {
                    year: 1905,
                    month: 1,
                },
                21,
            );
            if kind != crate::city::CityFileKind::MacCity {
                city.set_misc_u32_at(0x10ac, 0x12345678);
            }
            city.set_mac_developed_cells(12);
            for (offset, value) in [
                (super::super::MISC_CITY_TRAFFIC_OFFSET, 3000),
                (super::super::MISC_CITY_POLLUTION_OFFSET, 4000),
                (super::super::MISC_LAND_VALUE_OFFSET, 5000),
                (super::super::MISC_CITY_CRIME_OFFSET, 6000),
            ] {
                city.set_mac_state_misc_u32_at(offset, value);
            }
            for (row, value) in [(10, 3), (11, 5), (12, 7)] {
                let offset =
                    super::super::budget::budget_row_misc_offset_for_city(&city, row, 0).unwrap();
                city.set_budget_misc_u32_values(offset, &[value]);
            }
            let before: Vec<u32> = (0..XGRP_VALUES as u32).collect();
            city.set_chunk_u32_values(XGRP_CHUNK_NAME, 0, &before);
            assert!(mac_publish_xgrp_environment(&mut city));
            let after = city.chunk_u32_values(XGRP_CHUNK_NAME, 0, XGRP_VALUES);
            for (index, sample) in [187, 1000, 1250, 1500].into_iter().enumerate() {
                let start = (index + 4) * 52;
                assert_eq!(
                    (after[start], after[start + 12], after[start + 32]),
                    (sample, sample, sample)
                );
                assert_eq!(after[start + 1], before[start]);
                assert_eq!(after[start + 13], before[start + 12]);
                assert_eq!(after[start + 33], before[start + 32]);
            }
            assert_eq!(&after[..4 * 52], &before[..4 * 52]);
            assert_eq!(&after[8 * 52..], &before[8 * 52..]);
            if kind != crate::city::CityFileKind::MacCity {
                assert_eq!(city.misc_u32_at(0x10ac), Some(0x12345678));
            }
        }
    }

    #[test]
    fn mac_environment_history_roll_matches_original_68020() {
        let fixture = include_str!("../../tests/fixtures/simulation/mac/mac_xgrp_roll.csv");
        let mut count = 0;
        for line in fixture.lines().skip(1) {
            let v: Vec<_> = line.split(',').collect();
            let month: u16 = v[0].parse().unwrap();
            let year: u16 = v[1].parse().unwrap();
            let seed: u32 = v[2].parse().unwrap();
            let decode = |hex: &str| -> Vec<u32> {
                (0..hex.len() / 8)
                    .map(|i| u32::from_str_radix(&hex[i * 8..i * 8 + 8], 16).unwrap())
                    .collect()
            };
            let mut values = vec![0xdeadbeef; XGRP_VALUES];
            values[4 * 52..8 * 52].copy_from_slice(&decode(v[3]));
            let samples = mac_xgrp_environment_samples(
                [3000 + seed, 4000 + seed, 5000 + seed, 6000 + seed],
                [3 + seed, 5 + seed, 7 + seed],
                (12 + seed) as u16,
            )
            .unwrap();
            assert!(mac_xgrp_roll_environment(&mut values, samples, month, year));
            assert_eq!(
                &values[4 * 52..8 * 52],
                decode(v[4]),
                "month {month}, year {year}"
            );
            assert!(
                values[..4 * 52]
                    .iter()
                    .chain(&values[8 * 52..])
                    .all(|&v| v == 0xdeadbeef)
            );
            count += 1;
        }
        assert_eq!(count, 40);
    }

    #[test]
    fn mac_developed_cell_matches_original_68020() {
        let fixture = include_str!("../../tests/fixtures/simulation/mac/mac_developed_tile.csv");
        let mut count = 0;
        for line in fixture.lines().skip(1) {
            let values: Vec<u16> = line.split(',').map(|v| v.parse().unwrap()).collect();
            let actual = values[2].wrapping_add(u16::from(mac_xgrp_is_developed_cell(
                values[0] as u8,
                values[1] as u8,
            )));
            assert_eq!(actual, values[3], "{line}");
            count += 1;
        }
        assert_eq!(count, 3072);
    }

    #[test]
    fn mac_developed_census_counts_cells_once() {
        let mut city = City::new_blank("/tmp/mac-developed-xgrp.sc2", "Census").unwrap();
        city.set_building(4, 6, 0x1d);
        city.set_zone(4, 6, 1);
        city.set_zone(5, 6, 1);
        city.set_building(6, 6, 0x1c);
        assert_eq!(mac_xgrp_count_developed_cells(&city), 2);
    }

    #[test]
    fn mac_environment_samples_match_original_68020() {
        let fixture = include_str!("../../tests/fixtures/simulation/mac/mac_xgrp_environment.csv");
        let mut count = 0;
        for line in fixture.lines().skip(1) {
            let values: Vec<u32> = line.split(',').map(|v| v.parse().unwrap()).collect();
            assert_eq!(values.len(), 12);
            let actual = mac_xgrp_environment_samples(
                [values[0], values[1], values[2], values[3]],
                [values[4], values[5], values[6]],
                values[7] as u16,
            );
            assert_eq!(
                actual,
                Some([values[8], values[9], values[10], values[11]]),
                "{line}"
            );
            count += 1;
        }
        assert_eq!(count, 120);
        assert_eq!(
            mac_xgrp_environment_samples([1; 4], [u32::MAX, 0, 0], 0),
            None
        );
        assert_eq!(mac_xgrp_environment_samples([1; 4], [0; 3], 0xfffc), None);
    }
}
