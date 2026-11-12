//! Shared budget-row views, offsets, built counts, and tax-base readers.
//!
//! The MISC layout constants stay in the parent budget module. This owner
//! interprets those rows for PE/Mac city layouts and exposes the common view
//! used by the budget window, funding controls, and simulation summaries.

use super::*;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(in crate::app) struct BudgetServiceBuiltCounts {
    pub(in crate::app) police: u32,
    pub(in crate::app) fire: u32,
    pub(in crate::app) health: u32,
    pub(in crate::app) schools: u32,
    pub(in crate::app) colleges: u32,
    pub(in crate::app) roads: u32,
    pub(in crate::app) highways: u32,
    pub(in crate::app) bridges: u32,
    pub(in crate::app) rail: u32,
    pub(in crate::app) subway: u32,
    pub(in crate::app) tunnel: u32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(in crate::app) struct BudgetServiceRow {
    pub(in crate::app) current: u32,
    pub(in crate::app) funding: u32,
    pub(in crate::app) year_to_date: u32,
}

impl BudgetServiceBuiltCounts {
    pub(in crate::app) fn by_budget_index(self, index: usize) -> u32 {
        match index {
            0 => self.police,
            1 => self.fire,
            2 => self.health,
            3 => self.schools,
            4 => self.colleges,
            5 => self.roads,
            6 => self.highways,
            7 => self.bridges,
            8 => self.rail,
            9 => self.subway,
            10 => self.tunnel,
            _ => 0,
        }
    }
}
pub(in crate::app) fn budget_service_built_counts_for_city(
    city: &City,
) -> BudgetServiceBuiltCounts {
    // PE civic built units from `dwTileCount` cells (`0x4CA4C8`): 3×3 `/9`, college `/16`.
    // Transport rows are independent sums of that same census in FUN_004734A0; keep
    // overlaps (road×rail, bridge×highway, tunnel×road) rather than classifying each
    // map cell into one mutually exclusive transport family.
    let counts = BudgetServiceBuiltCounts {
        police: u32::from(city.pe_dw_tile_count[210]) / 9,
        fire: u32::from(city.pe_dw_tile_count[FIRE_STATION_BUILDING_ID as usize]) / 9,
        health: u32::from(city.pe_dw_tile_count[HOSPITAL_BUILDING_ID as usize]) / 9,
        schools: u32::from(city.pe_dw_tile_count[SCHOOL_BUILDING_ID as usize]) / 9,
        colleges: u32::from(city.pe_dw_tile_count[217]) / 16,
        roads: pe_budget_transport_tile_sum(
            city,
            &[(0x1d, 0x2b), (0x3f, 0x46), (0x4b, 0x4c), (0x5d, 0x60)],
        )
        .saturating_add((u32::from(city.pe_dw_tile_count[236]) / 4).saturating_mul(250)),
        highways: pe_budget_transport_tile_sum(city, &[(0x49, 0x50), (0x61, 0x6b)]),
        bridges: pe_budget_transport_tile_sum(city, &[(0x51, 0x5c), (0x6a, 0x6b)]),
        rail: pe_budget_transport_tile_sum(
            city,
            &[(0x2c, 0x3e), (0x45, 0x48), (0x6c, 0x6f), (0x4d, 0x4e)],
        )
        .saturating_add(u32::from(city.pe_dw_tile_count[237])),
        tunnel: pe_budget_transport_tile_sum(city, &[(0x3f, 0x42)]),
        // PE @ `0x473807`–`0x473815`: subway built = `0x4ca41c` + `dwTileCount[0xe9]`.
        subway: u32::from(city.pe_xund_occupancy).wrapping_add(u32::from(
            city.pe_dw_tile_count[SUBWAY_STATION_BUILDING_ID as usize],
        )),
        ..Default::default()
    };

    counts
}

fn pe_budget_transport_tile_sum(city: &City, ranges: &[(u8, u8)]) -> u32 {
    ranges.iter().fold(0_u32, |total, &(first, last)| {
        (first..=last).fold(total, |sum, building| {
            sum.saturating_add(u32::from(city.pe_dw_tile_count[usize::from(building)]))
        })
    })
}
pub(in crate::app) fn funded_coverage_percent(
    city: &City,
    budget_index: usize,
    coverage: ServiceCoverage,
) -> u32 {
    coverage_percent(coverage)
        .saturating_mul(budget_service_funding_percent_for_city(city, budget_index))
        / 100
}

pub(in crate::app) fn thresholded_funded_service_coverage_percent(
    city: &City,
    budget_index: usize,
    coverage: ServiceCoverage,
    population: PopulationStats,
    population_step: u32,
    built_count: u32,
) -> u32 {
    thresholded_service_coverage_percent(coverage, population, population_step, built_count)
        .saturating_mul(budget_service_funding_percent_for_city(city, budget_index))
        / 100
}
pub(in crate::app) fn budget_row_values_are_unset(values: &[u32]) -> bool {
    values.iter().all(|value| *value == 0)
}

pub(in crate::app) fn budget_row_is_unset(city: &City, row_index: usize) -> bool {
    let Some(start) = budget_row_misc_offset_for_city(city, row_index, 0) else {
        return true;
    };
    let values = city.budget_misc_u32_values(start, BUDGET_SERVICE_MISC_STRIDE);
    if !values.is_empty() {
        return budget_row_values_are_unset(&values);
    }
    // `misc_u32_values` needs the full 27-dword row. A lone saved rate/funding
    // dword still counts as set so zero-fill gaps do not wipe player values.
    let field = if row_index < BUDGET_TAX_COUNT {
        TAX_RATE_FIELD
    } else {
        BUDGET_SERVICE_FUNDING_FIELD
    };
    city.budget_misc_u32_at(
        budget_row_misc_offset_for_city(city, row_index, field).unwrap_or(start),
    )
    .is_none()
}

/// Return the active-document tax DWORD before the UI clamp.
pub(in crate::app) fn raw_tax_rate_for_city(city: &City, tax_index: usize) -> u32 {
    if budget_row_is_unset(city, tax_index) {
        return DEFAULT_TAX_RATE;
    }
    tax_rate_misc_offset_for_city(city, tax_index)
        .and_then(|offset| city.budget_misc_u32_at(offset))
        .unwrap_or(DEFAULT_TAX_RATE)
}

pub(in crate::app) fn tax_rate_for_city(city: &City, tax_index: usize) -> u32 {
    if budget_row_is_unset(city, tax_index) {
        return DEFAULT_TAX_RATE;
    }
    let Some(offset) = tax_rate_misc_offset_for_city(city, tax_index) else {
        return DEFAULT_TAX_RATE;
    };
    city.budget_misc_u32_at(offset)
        .map(|rate| rate.min(MAX_TAX_RATE))
        .unwrap_or(DEFAULT_TAX_RATE)
}

pub(in crate::app) fn tax_rate_misc_offset(tax_index: usize) -> Option<usize> {
    tax_index
        .checked_mul(TAX_MISC_STRIDE)
        .and_then(|base| base.checked_add(TAX_RATE_FIELD))
        .and_then(|field| field.checked_mul(4))
        .and_then(|bytes| TAX_MISC_OFFSET.checked_add(bytes))
}
pub(in crate::app) fn budget_row_misc_offset(row_index: usize, field: usize) -> Option<usize> {
    budget_row_misc_offset_at_base(TAX_MISC_OFFSET, row_index, field)
}

pub(in crate::app) fn budget_row_misc_offset_for_city(
    city: &City,
    row_index: usize,
    field: usize,
) -> Option<usize> {
    let base = match city.kind {
        CityFileKind::MacCity => MAC_BUDGET_MISC_OFFSET,
        CityFileKind::WinSc2 | CityFileKind::WinScenario => TAX_MISC_OFFSET,
    };
    budget_row_misc_offset_at_base(base, row_index, field)
}

fn budget_row_misc_offset_at_base(base: usize, row_index: usize, field: usize) -> Option<usize> {
    row_index
        .checked_mul(BUDGET_SERVICE_MISC_STRIDE)
        .and_then(|base| base.checked_add(field))
        .and_then(|field| field.checked_mul(4))
        .and_then(|bytes| base.checked_add(bytes))
}

pub(in crate::app) fn tax_rate_misc_offset_for_city(
    city: &City,
    tax_index: usize,
) -> Option<usize> {
    budget_row_misc_offset_for_city(city, tax_index, TAX_RATE_FIELD)
}

/// Win95 calendar month word @ `0x004CB01C`: `1..=12` → history index `0..=11`.
pub(in crate::app) fn pe_budget_month_index(clock_month: u8) -> usize {
    usize::from(clock_month.saturating_sub(1)).min(11)
}

/// PE `FUN_004734A0` snapshots row `+0x60/+0x64` into `+month*4` / `+0x30+month*4`.
/// Rust stores the pair in MISC dwords `3+month*2` (count/current) and `4+month*2` (funding).
pub(in crate::app) fn pe_budget_month_history_fields(month_index: usize) -> (usize, usize) {
    let month_index = month_index.min(11);
    (3 + month_index * 2, 4 + month_index * 2)
}

pub(in crate::app) fn budget_service_funding_percent_for_city(
    city: &City,
    budget_index: usize,
) -> u32 {
    budget_service_funding_raw_for_city(city, budget_index).min(100)
}

/// Return the saved active-document funding dword without applying the UI's
/// 0..=100 display/input clamp. PE annual selectors and per-tile upkeep read
/// this raw saved value before applying their instruction-width semantics.
pub(in crate::app) fn budget_service_funding_raw_for_city(city: &City, budget_index: usize) -> u32 {
    if budget_row_is_unset(city, BUDGET_SERVICE_ROW_START.saturating_add(budget_index)) {
        return 100;
    }
    let Some(offset) = budget_service_funding_misc_offset_for_city(city, budget_index) else {
        return 100;
    };
    city.budget_misc_u32_at(offset).unwrap_or(100)
}

pub(in crate::app) fn education_budget_funding_percent_for_city(city: &City) -> u32 {
    average_budget_service_funding_percent_for_city(city, &[3, 4])
}

pub(in crate::app) fn transport_budget_funding_percent_for_city(city: &City) -> u32 {
    average_budget_service_funding_percent_for_city(city, &TRANSPORT_BUDGET_INDICES)
}

pub(in crate::app) fn average_budget_service_funding_percent_for_city(
    city: &City,
    budget_indices: &[usize],
) -> u32 {
    if budget_indices.is_empty() {
        return 0;
    }
    budget_indices
        .iter()
        .map(|&index| budget_service_funding_percent_for_city(city, index))
        .sum::<u32>()
        / budget_indices.len() as u32
}

pub(in crate::app) fn budget_service_funding_misc_offset(budget_index: usize) -> Option<usize> {
    budget_service_misc_offset(budget_index, BUDGET_SERVICE_FUNDING_FIELD)
}

pub(in crate::app) fn budget_service_funding_misc_offset_for_city(
    city: &City,
    budget_index: usize,
) -> Option<usize> {
    budget_service_misc_offset_for_city(city, budget_index, BUDGET_SERVICE_FUNDING_FIELD)
}

pub(in crate::app) fn budget_service_misc_offset(
    budget_index: usize,
    field: usize,
) -> Option<usize> {
    budget_row_misc_offset(BUDGET_SERVICE_ROW_START.checked_add(budget_index)?, field)
}

pub(in crate::app) fn budget_service_misc_offset_for_city(
    city: &City,
    budget_index: usize,
    field: usize,
) -> Option<usize> {
    budget_row_misc_offset_for_city(
        city,
        BUDGET_SERVICE_ROW_START.checked_add(budget_index)?,
        field,
    )
}

pub(in crate::app) fn budget_service_rows_for_city(
    city: &City,
) -> [BudgetServiceRow; BUDGET_SERVICE_COUNT] {
    let built_counts = budget_service_built_counts_for_city(city);
    let mut rows = [BudgetServiceRow {
        current: 0,
        funding: 100,
        year_to_date: 0,
    }; BUDGET_SERVICE_COUNT];

    for (index, row) in rows.iter_mut().enumerate() {
        let live_current = built_counts.by_budget_index(index);
        row.current = live_current;
        row.funding = budget_service_funding_percent_for_city(city, index);
        row.year_to_date = live_current;

        let Some(offset) = budget_service_misc_offset_for_city(city, index, 0) else {
            continue;
        };
        let saved = city.budget_misc_u32_values(offset, BUDGET_SERVICE_MISC_STRIDE);
        if saved.len() < BUDGET_SERVICE_MISC_STRIDE {
            continue;
        }
        let saved_current = saved[BUDGET_SERVICE_CURRENT_FIELD];
        let saved_year_to_date = saved[BUDGET_SERVICE_YEAR_TO_DATE_FIELD];
        let generated_empty = budget_row_is_unset(city, BUDGET_SERVICE_ROW_START + index);
        if generated_empty {
            row.funding = 100;
        } else {
            row.funding = saved
                .get(BUDGET_SERVICE_FUNDING_FIELD)
                .copied()
                .unwrap_or_else(|| budget_service_funding_percent_for_city(city, index))
                .min(100);
        }
        let missing_live_costs = live_current > 0 && saved_current == 0 && saved_year_to_date == 0;
        if generated_empty || missing_live_costs {
            row.current = live_current;
            row.year_to_date = live_current;
        } else {
            row.current = saved_current;
            row.year_to_date = saved_year_to_date;
        }
    }

    rows
}

/// Win95 `SimulationRCIDemandUpdates` (`0x468489`..`0x468534`) tax `iCurrentCosts`:
/// `zonePopsBase[R|C|I] * 10`, then add `dwArcologyPopulation / {6,12,12}`.
///
/// When day-21 `pZonePops` is ready, prefer those folded rows (`* 10`) over the
/// census `*_population` fields so January attitude score 0 and unset tax-fund
/// currents match PE `dwCityTrafficCount / (taxBases + 1)`.
pub(in crate::app) fn budget_tax_bases_for_city(
    city: &City,
    population: PopulationStats,
) -> [u32; BUDGET_TAX_COUNT] {
    budget_tax_bases_for_city_with_fidelity(city, population, EngineFidelity::Win95)
}

pub(in crate::app) fn budget_tax_bases_for_city_with_fidelity(
    city: &City,
    population: PopulationStats,
    fidelity: EngineFidelity,
) -> [u32; BUDGET_TAX_COUNT] {
    // PE `0x4684f1`..`0x468527` reads BSS `dwArcologyPopulation` @ `0x4c94c4`.
    // Choose that runtime state by engine fidelity. The city kind controls
    // where values are serialized, not which simulation is running.
    let uses_pe_population = fidelity.uses_win95();
    let arco = if uses_pe_population {
        city.pe_dw_arcology_population()
    } else {
        city.mac_arcology_resident_total()
    };
    let [residential, commercial, industrial] = if uses_pe_population && city.pe_zone_pops_ready() {
        let rows = city.pe_zone_pops();
        [
            pe_zone_pop_row_people(rows[0]),
            pe_zone_pop_row_people(rows[1]),
            pe_zone_pop_row_people(rows[2]),
        ]
    } else {
        [
            population.residential_population,
            population.commercial_population,
            population.industrial_population,
        ]
    };
    [
        residential.saturating_add(arco / 6),
        commercial.saturating_add(arco / 12),
        industrial.saturating_add(arco / 12),
    ]
}

/// PE `zonePopsBase[i] * 10` for tax / attitude bases; negative rows contribute 0.
pub(in crate::app) fn pe_zone_pop_row_people(row: i32) -> u32 {
    let scaled = i64::from(row).saturating_mul(10);
    if scaled <= 0 {
        0
    } else {
        u32::try_from(scaled).unwrap_or(u32::MAX)
    }
}

/// PE `pBudgetArr[BUDGET_RESFUND/COMFUND/INDFUND].iCurrentCosts` for scenario R/C/I goals.
///
/// Win95 scenarios read the tax-row CURRENT field even when the whole row is
/// zero, as the native evaluator does. Other callers retain their live-base
/// fallback for an unset row.
pub(in crate::app) fn budget_tax_fund_current_costs_for_city(
    city: &City,
    population: PopulationStats,
) -> [u32; BUDGET_TAX_COUNT] {
    let mut costs = if city.kind == CityFileKind::WinScenario {
        [0; BUDGET_TAX_COUNT]
    } else {
        budget_tax_bases_for_city(city, population)
    };
    for (index, cost) in costs.iter_mut().enumerate() {
        if city.kind != CityFileKind::WinScenario && budget_row_is_unset(city, index) {
            continue;
        }
        let Some(offset) =
            budget_row_misc_offset_for_city(city, index, BUDGET_SERVICE_CURRENT_FIELD)
        else {
            continue;
        };
        if let Some(current) = city.budget_misc_u32_at(offset) {
            *cost = current;
        }
    }
    costs
}
