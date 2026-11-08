//! Industry mix ratios, taxes, and demand rows.

use super::*;

/// Win95 MISC after EQ_LE (`FUN_0043153a`, dword index `0x1f + 60`).
/// Packed `[demand, tax, population] × 11` (`wArrIndustrialDemands` `0x4ca3f4`,
/// `wArrIndustrialTaxRates` `0x4cb3e0`, `dwArrIndustrialPopulations` `0x4cb3f0`).
/// Demand and tax are WORDs stored in MISC dwords; tile counts follow at `0x1F0`.
/// Day 21 `SimulationEQ_LE_Processing` (`0x46910a`) owns demand and population;
/// the Industry dialog ratio is live-only from those population shares.
pub(super) const INDUSTRY_MISC_OFFSET: usize = 0x016c;
pub(super) const INDUSTRY_COUNT: usize = 11;
pub(super) const INDUSTRY_FIELD_COUNT: usize = 3;
pub(super) const INDUSTRY_VALUES: usize = INDUSTRY_COUNT * INDUSTRY_FIELD_COUNT;
pub(super) const INDUSTRY_DEMAND_VARIATION: [i32; INDUSTRY_COUNT] =
    [0, -40, -60, 80, 40, -20, 20, 60, 50, 70, 30];
pub(super) const INDUSTRY_LABELS: [&str; INDUSTRY_COUNT] = [
    "Steel / Mining",
    "Textiles",
    "Petrochemical",
    "Food",
    "Construction",
    "Automotive",
    "Aerospace",
    "Finance",
    "Media",
    "Electronics",
    "Tourism",
];
pub(super) fn industry_tax_misc_offset(industry_index: usize) -> Option<usize> {
    if industry_index >= INDUSTRY_COUNT {
        return None;
    }
    industry_index
        .checked_mul(INDUSTRY_FIELD_COUNT)
        .and_then(|base| base.checked_add(1))
        .and_then(|field| field.checked_mul(4))
        .and_then(|bytes| INDUSTRY_MISC_OFFSET.checked_add(bytes))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct IndustryStats {
    /// Display share from `dwArrIndustrialPopulations`; not a MISC field.
    pub(super) ratio: i32,
    pub(super) tax: i32,
    pub(super) demand: i32,
    pub(super) population: i32,
}

pub(super) fn industry_stats_for_city(city: &City) -> [IndustryStats; INDUSTRY_COUNT] {
    let fallback_tax = tax_rate_for_city(city, 2).min(i32::MAX as u32) as i32;
    let mut rows = [IndustryStats {
        ratio: 0,
        tax: fallback_tax,
        demand: 0,
        population: 0,
    }; INDUSTRY_COUNT];
    if city.kind == CityFileKind::MacCity {
        // The save layouts share these 11 demand/tax/population rows. Mac
        // CODE 4 stores the first two lanes as WORDs widened to MISC dwords;
        // decode those saved words while leaving engine-specific producers
        // and formulas to their selected simulation paths.
        let mut mac_rows = mac_industry_stats_for_city(city);
        if mac_rows
            .iter()
            .all(|row| row.demand == 0 && row.tax == 0 && row.population == 0)
        {
            for row in &mut mac_rows {
                row.tax = fallback_tax;
            }
        }
        return mac_rows;
    }
    let values = city.misc_i32_values(INDUSTRY_MISC_OFFSET, INDUSTRY_VALUES);
    if values.len() < INDUSTRY_VALUES || values.iter().all(|value| *value == 0) {
        return rows;
    }
    for (index, row) in rows.iter_mut().enumerate() {
        let base = index * INDUSTRY_FIELD_COUNT;
        if let Some(demand) = values.get(base) {
            row.demand = *demand;
        }
        if let Some(tax) = values.get(base + 1) {
            row.tax = *tax;
        }
        if let Some(population) = values.get(base + 2) {
            row.population = *population;
        }
    }
    let counts = rows.map(|row| row.population.max(0) as u32);
    let ratios = industry_ratio_values(counts);
    for (row, ratio) in rows.iter_mut().zip(ratios) {
        row.ratio = ratio;
    }
    rows
}

/// CODE 4 serializes each Mac industry row as demand WORD, tax WORD, and
/// population LONG, widened to three MISC dwords. Interpret the WORD lanes
/// as the native load does, rather than treating them as PE state.
pub(super) fn mac_industry_stats_for_city(city: &City) -> [IndustryStats; INDUSTRY_COUNT] {
    let mut rows = [IndustryStats {
        ratio: 0,
        tax: 0,
        demand: 0,
        population: 0,
    }; INDUSTRY_COUNT];
    for (index, row) in rows.iter_mut().enumerate() {
        let base = INDUSTRY_MISC_OFFSET + index * INDUSTRY_FIELD_COUNT * 4;
        row.demand = city.mac_state_misc_u32_at(base).unwrap_or(0) as u16 as i16 as i32;
        row.tax = city.mac_state_misc_u32_at(base + 4).unwrap_or(0) as u16 as i16 as i32;
        row.population = city.mac_state_misc_u32_at(base + 8).unwrap_or(0) as i32;
    }
    let ratios = industry_ratio_values(rows.map(|row| row.population.max(0) as u32));
    for (row, ratio) in rows.iter_mut().zip(ratios) {
        row.ratio = ratio;
    }
    rows
}

pub(super) fn live_industry_stats_for_city(
    city: &City,
    population: PopulationStats,
    outcomes: CityServiceOutcomes,
) -> [IndustryStats; INDUSTRY_COUNT] {
    let mut rows = industry_stats_for_city(city);
    let mut counts = [0_u32; INDUSTRY_COUNT];
    for y in 0..CITY_HEIGHT {
        for x in 0..CITY_WIDTH {
            let Some(index) = industrial_building_industry_index(city.building_at(x, y)) else {
                continue;
            };
            counts[index] = counts[index].saturating_add(1);
        }
    }

    let industrial_demand =
        rci_demand_values_for_city(city, population, outcomes)[rci_kind_index(RciKind::Industrial)];
    let ratios = industry_ratio_values(counts);
    let average_tax = average_industry_tax_rate_for_rows(&rows);
    let average_delta = pe_rci_tax_demand_delta(average_tax);
    for (index, row) in rows.iter_mut().enumerate() {
        row.ratio = ratios[index];
        row.population = counts[index] as i32;
        let tax_penalty =
            average_delta - pe_rci_tax_demand_delta(row.tax.clamp(0, MAX_TAX_RATE as i32));
        row.demand = industrial_demand
            .saturating_add(INDUSTRY_DEMAND_VARIATION[index])
            .saturating_sub(tax_penalty)
            .clamp(RCI_DEMAND_MIN, RCI_DEMAND_MAX);
    }
    rows
}

pub(super) fn average_industry_tax_rate_for_rows(rows: &[IndustryStats; INDUSTRY_COUNT]) -> i32 {
    rows.iter()
        .map(|row| row.tax.clamp(0, MAX_TAX_RATE as i32))
        .sum::<i32>()
        / INDUSTRY_COUNT as i32
}

pub(super) fn industry_ratio_values(counts: [u32; INDUSTRY_COUNT]) -> [i32; INDUSTRY_COUNT] {
    let mut ratios = [0_i32; INDUSTRY_COUNT];
    let total = counts.iter().copied().sum::<u32>();
    if total == 0 {
        return ratios;
    }

    let mut assigned = 0_i32;
    for (index, count) in counts.iter().enumerate() {
        ratios[index] = (count.saturating_mul(100) / total) as i32;
        assigned += ratios[index];
    }
    let mut remainder = 100_i32.saturating_sub(assigned);
    while remainder > 0 {
        let Some((index, _)) = counts.iter().enumerate().max_by_key(|(_, count)| *count) else {
            break;
        };
        ratios[index] += 1;
        remainder -= 1;
    }
    ratios
}

pub(super) fn industrial_building_industry_index(building: u8) -> Option<usize> {
    match building {
        0x84 | 0xBC => Some(0),
        0x85 => Some(1),
        0x86 | 0xBD => Some(2),
        0x87 => Some(3),
        0x9E | 0xBE => Some(4),
        0x9F | 0xBF => Some(5),
        0xA0 | 0xC0 => Some(6),
        0xA1 => Some(7),
        0xA2 => Some(8),
        0xA3 | 0xC1 => Some(9),
        0xA4 | 0xA5 => Some(10),
        _ => None,
    }
}

pub(super) fn industry_stats_to_misc_values(rows: [IndustryStats; INDUSTRY_COUNT]) -> Vec<u32> {
    rows.into_iter()
        .flat_map(|stats| {
            [
                stats.demand as u32,
                stats.tax as u32,
                stats.population as u32,
            ]
        })
        .collect()
}
pub(super) fn ensure_industry_misc_defaults_before_extension(city: &mut City) {
    let values = city.misc_u32_values(INDUSTRY_MISC_OFFSET, INDUSTRY_VALUES);
    if values.len() >= INDUSTRY_VALUES && values.iter().any(|value| *value != 0) {
        return;
    }
    let defaults = industry_stats_to_misc_values(industry_stats_for_city(city));
    city.set_misc_u32_values(INDUSTRY_MISC_OFFSET, &defaults);
}
