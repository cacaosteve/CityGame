//! Bond rate storage, issue pricing, and budget-row synchronization.
//!
//! This is the shared bond data model used by budget controls, scenario cash
//! checks, city valuation, and annual budget settlement.

use super::*;

pub(in crate::app) fn outstanding_bond_count_for_city(city: &City) -> u32 {
    city.pe_dw_city_bonds.min(BOND_DATA_MAX_COUNT as u32)
}

/// PE scenario cash check: `dwCityFunds - dwCityBonds` (`0x4CA444 - 0x4CC4E8`).
///
/// `dwCityBonds` / MISC `0x0018` is the outstanding bond *count* (incremented on
/// issue, decremented on repay), not principal dollars. PE still subtracts that
/// count from funds before comparing `dwScenarioCashGoal` (`0x413968`).
pub(in crate::app) fn scenario_net_city_funds(city: &City) -> i32 {
    // The goal evaluator subtracts the raw DWORD; the separate 50-entry
    // clamp is only for safely indexing the bond-rate table.
    city.city_funds().wrapping_sub(city.pe_dw_city_bonds as i32)
}

pub(in crate::app) fn bond_rates_scaled_for_city(city: &City) -> Vec<u32> {
    let count = outstanding_bond_count_for_city(city) as usize;
    if count == 0 {
        return Vec::new();
    }
    // Both native loaders copy each MISC dword's low WORD into a 50-entry
    // WORD table. Scale only in memory so budget/cashflow arithmetic can
    // retain its fractional-percent units.
    let mut rates: Vec<u32> = city
        .misc_u32_values(BOND_DATA_MISC_OFFSET, count)
        .into_iter()
        .map(|rate| u32::from(rate as u16).saturating_mul(BOND_RATE_SCALE))
        .collect();
    if rates.len() < count {
        rates.resize(count, bond_issue_rate_scaled_for_city(city));
    }
    rates.truncate(count);
    rates
}

/// In-memory budget scale for a Win95 whole-percent rate (4% → 40_000).
pub(in crate::app) fn bond_issue_rate_scaled_for_city(city: &City) -> u32 {
    bond_issue_rate_percent_for_city(city).saturating_mul(BOND_RATE_SCALE)
}

/// PE `0x41b29e..0x41b2a7`: pass the national-tax WORD plus one to
/// `FUN_00473A40`, with 16-bit increment wrap.
pub(in crate::app) fn bond_issue_rate_percent_for_city(city: &City) -> u32 {
    u32::from(city.pe_w_national_tax.wrapping_add(1))
}

/// Annual interest dollars for outstanding bonds (`rate_scaled / 100` each).
pub(in crate::app) fn annual_bond_interest_for_city(city: &City) -> i64 {
    bond_rates_scaled_for_city(city)
        .into_iter()
        .fold(0_i64, |total, rate_scaled| {
            total.saturating_add(i64::from(rate_scaled / 100))
        })
}

pub(in crate::app) fn sync_bond_budget_row_for_city(city: &mut City, _month_index: usize) {
    let rates = bond_rates_scaled_for_city(city);
    let count = rates.len() as u32;
    let sum_rate = rates
        .iter()
        .fold(0_u64, |total, rate| total.saturating_add(u64::from(*rate)));
    let average_rate = if count == 0 {
        0
    } else {
        (sum_rate / u64::from(count)).min(u64::from(u32::MAX)) as u32
    };
    let Some(offset) = budget_row_misc_offset_for_city(city, BUDGET_BOND_ROW, 0) else {
        return;
    };
    let mut values = city.budget_misc_u32_values(offset, BUDGET_SERVICE_MISC_STRIDE);
    if values.len() < BUDGET_SERVICE_MISC_STRIDE {
        values.resize(BUDGET_SERVICE_MISC_STRIDE, 0);
    }
    values[BUDGET_SERVICE_CURRENT_FIELD] = count;
    values[BUDGET_SERVICE_FUNDING_FIELD] = average_rate;
    city.set_budget_misc_u32_values(offset, &values);
}

pub(in crate::app) fn append_bond_rate_for_city(city: &mut City, rate_scaled: u32) -> bool {
    let current = outstanding_bond_count_for_city(city) as usize;
    if current >= BOND_DATA_MAX_COUNT {
        return false;
    }
    city.set_misc_u32_at(
        BOND_DATA_MISC_OFFSET + current * 4,
        rate_scaled / BOND_RATE_SCALE,
    );
    city.set_misc_u32_at(BUDGET_BONDS_MISC_OFFSET, (current + 1) as u32);
    true
}

/// PE `FUN_0042E460`: Hard starts with one $10,000 bond at the national rate
/// (3%). The base city setup already supplies the $10,000 starting funds.
pub(in crate::app) fn initialize_hard_start_bond(city: &mut City) {
    if city.pe_w_city_difficulty == 3 && append_bond_rate_for_city(city, 3 * BOND_RATE_SCALE) {
        sync_bond_budget_row_for_city(city, 0);
        // PE Hard setup seeds row `+0x68` with count × average rate (one month).
        if let Some(offset) = budget_row_misc_offset_for_city(
            city,
            BUDGET_BOND_ROW,
            BUDGET_SERVICE_YEAR_TO_DATE_FIELD,
        ) {
            city.set_misc_u32_at(offset, 3 * BOND_RATE_SCALE);
        }
    }
}

pub(in crate::app) fn remove_oldest_bond_rate_for_city(city: &mut City) -> bool {
    let current = outstanding_bond_count_for_city(city) as usize;
    if current == 0 {
        return false;
    }
    let mut rates = bond_rates_scaled_for_city(city);
    if rates.len() < current {
        rates.resize(current, bond_issue_rate_scaled_for_city(city));
    }
    rates.remove(0);
    if rates.is_empty() {
        city.set_misc_u32_at(BOND_DATA_MISC_OFFSET, 0);
    } else {
        let raw_rates: Vec<u32> = rates.iter().map(|rate| rate / BOND_RATE_SCALE).collect();
        city.set_misc_u32_values(BOND_DATA_MISC_OFFSET, &raw_rates);
    }
    city.set_misc_u32_at(BUDGET_BONDS_MISC_OFFSET, rates.len() as u32);
    true
}
