//! PE and Mac budget-row settlement, monthly snapshots, and derived currents.
//!
//! Saved row storage and offset calculations remain in the parent. This module
//! owns the arithmetic that consumes those rows, preserving PE signed divisor
//! settlement separately from Mac's signed accumulated row book.

use super::*;

pub(in crate::app) fn budget_row_participates_in_cashflow(row_index: usize) -> bool {
    // The PE January worker settles all sixteen row accumulators, including
    // the ordinance and bond rows. Their current fields have distinct
    // producers, but +0x68 uses the same raw current×funding accumulator.
    row_index < BUDGET_TOTAL_COUNT
}

pub(in crate::app) fn budget_row_year_to_date(
    row_index: usize,
    values: &[u32],
    elapsed_months: usize,
) -> u32 {
    let elapsed_months = elapsed_months.min(12);
    if row_index >= BUDGET_TOTAL_COUNT {
        return 0;
    }
    // FUN_004734A0 stores each raw current×funding product in the history
    // pair and adds it to +0x68 with 32-bit wrap. The resulting accumulator
    // is not dollars: January applies the signed row divisor and /12 once.
    (0..elapsed_months).fold(0_u32, |total, month| {
        let (count_field, funding_field) = pe_budget_month_history_fields(month);
        let product = values
            .get(count_field)
            .copied()
            .unwrap_or(0)
            .wrapping_mul(values.get(funding_field).copied().unwrap_or(0));
        total.wrapping_add(product)
    })
}

pub(in crate::app) fn budget_row_annual_amount(
    row_index: usize,
    current: u32,
    funding: u32,
) -> u32 {
    if row_index < BUDGET_TAX_COUNT {
        return current
            .saturating_mul(funding.min(20))
            .checked_div(TAX_REVENUE_DIVISOR)
            .unwrap_or(0);
    }
    if row_index < BUDGET_SERVICE_ROW_START {
        return 0;
    }

    let Some(divisor) = PE_BUDGET_ROW_DIVISORS
        .get(row_index)
        .copied()
        .filter(|divisor| *divisor < 0)
        .map(i16::unsigned_abs)
    else {
        return 0;
    };
    let numerator = u64::from(current).saturating_mul(u64::from(funding.min(100)));
    (numerator / u64::from(divisor)).min(u64::from(u32::MAX)) as u32
}

pub(in crate::app) fn budget_year_end_delta_for_city(city: &City) -> i32 {
    // PE FUN_004734A0 iterates every 0x70-byte row, signed-divides its raw
    // +0x68 accumulator by (signed divisor × 12), then ADDs the quotient to
    // the 32-bit funds DWORD. Preserve that order and wrap width; do not
    // recompute ordinances from current population or bond costs from today's
    // outstanding-rate table.
    (0..BUDGET_TOTAL_COUNT).fold(0_i32, |total, row_index| {
        let Some(offset) =
            budget_row_misc_offset_for_city(city, row_index, BUDGET_SERVICE_YEAR_TO_DATE_FIELD)
        else {
            return total;
        };
        let raw = city.budget_misc_u32_at(offset).unwrap_or(0) as i32;
        let divisor = i32::from(PE_BUDGET_ROW_DIVISORS[row_index]) * 12;
        total.wrapping_add(raw / divisor)
    })
}

/// Mac CODE 4 `export_227`: settle each row's signed 32-bit accumulated
/// current×funding numerator once, using the edition's shared divisor table.
/// Unlike the PE path, Mac row 3 (ordinances) and row 4 (bonds) are ordinary
/// rows in this 16-row walk; their accumulated fields already contain the
/// signed cash numerator and must not be recalculated from PE population data.
pub(in crate::app) fn mac_budget_year_end_delta_for_city(city: &City) -> i32 {
    (0..BUDGET_TOTAL_COUNT).fold(0_i32, |total, row_index| {
        let Some(offset) =
            budget_row_misc_offset_for_city(city, row_index, BUDGET_SERVICE_YEAR_TO_DATE_FIELD)
        else {
            return total;
        };
        let Some(raw_accumulator) = city.budget_misc_u32_at(offset) else {
            return total;
        };
        let raw_accumulator = raw_accumulator as i32;
        let divisor = i32::from(MAC_BUDGET_ROW_DIVISORS[row_index]) * 12;
        total.wrapping_add(raw_accumulator / divisor)
    })
}

/// Mac CODE 4 `export_227`: snapshot and accrue the active-document budget
/// rows before it refreshes row 3's ordinance current value. The row book uses
/// signed 32-bit longwords, so both multiplication and accumulation wrap.
pub(in crate::app) fn mac_budget_month_snapshot_and_accrue(city: &mut City, month_index: usize) {
    let (history_current, history_funding) = pe_budget_month_history_fields(month_index);
    for row_index in 0..BUDGET_TOTAL_COUNT {
        let Some(offset) = budget_row_misc_offset_for_city(city, row_index, 0) else {
            continue;
        };
        let mut values = city.budget_misc_u32_values(offset, BUDGET_SERVICE_MISC_STRIDE);
        if values.len() < BUDGET_SERVICE_MISC_STRIDE {
            values.resize(BUDGET_SERVICE_MISC_STRIDE, 0);
        }
        values[history_current] = values[BUDGET_SERVICE_CURRENT_FIELD];
        values[history_funding] = values[BUDGET_SERVICE_FUNDING_FIELD];
        let current = values[BUDGET_SERVICE_CURRENT_FIELD] as i32;
        let funding = values[BUDGET_SERVICE_FUNDING_FIELD] as i32;
        let accumulator = values[BUDGET_SERVICE_YEAR_TO_DATE_FIELD] as i32;
        values[BUDGET_SERVICE_YEAR_TO_DATE_FIELD] =
            accumulator.wrapping_add(current.wrapping_mul(funding)) as u32;
        city.set_budget_misc_u32_values(offset, &values);
    }
}

/// CODE 8 export 237 entry publishes the six-counter sum and scalar with
/// wrapping long arithmetic. The +0x102c population scalar is shared by both
/// native layouts; its Mac-only companion accumulator remains layout-gated.
pub(in crate::app) fn mac_publish_monthly_growth_scalar(city: &mut City) {
    let total = (1..=6).fold(0_u32, |sum, index| {
        sum.wrapping_add(city.mac_rci_counter(index))
    });
    let scalar = total.wrapping_mul(10);
    let previous = city.mac_state_misc_u32_at(0x102c).unwrap_or(0);
    city.mac_population_change_balances = if scalar < previous {
        [0, previous - scalar]
    } else {
        [scalar - previous, 0]
    };
    let accumulated = city.mac_state_misc_u32_at(0x0040).unwrap_or(0);
    city.set_mac_rci_counter(0, total);
    city.set_mac_state_misc_u32_at(0x102c, scalar);
    if city.kind == CityFileKind::MacCity {
        city.set_mac_state_misc_u32_at(0x0040, accumulated.wrapping_add(scalar));
    } else {
        // PE loads the same shared saved scalar into dwCityPopulation.
        city.set_pe_dw_city_population(scalar);
    }
}

/// CODE 14 FUN_00026c2a publishes the ratio consumed by Mac riot gates. The
/// native CODE 4 cursor and PE persistence path both place this DWORD at MISC
/// +0x0fa4, and both engines publish the same full-DWORD ratio there.
pub(in crate::app) fn mac_publish_monthly_riot_ratio(city: &mut City) -> Option<u32> {
    let total = city.mac_rci_counter(0);
    let class_two = city.mac_rci_counter(7);
    let denominator = total.wrapping_add(class_two).wrapping_add(1);
    // The original DIVU faults for a wrapped zero divisor; preserve the saved
    // value for malformed state rather than aborting the game.
    let ratio = class_two.wrapping_mul(100).checked_div(denominator)?;
    city.set_mac_state_misc_u32_at(MAC_RIOT_UNEMPLOYMENT_STATE_OFFSET, ratio);
    // On a Win-layout save, City::save mirrors this PE cache back to the same
    // serialized DWORD. Keep it synchronized so the Mac-owned ratio survives
    // a save while Mac fidelity is active.
    if city.kind.is_win95() {
        city.set_pe_unemployment_percent(ratio);
    }
    Some(ratio)
}

/// Mac CODE 8 `export_237` writes the first three budget row currents from the
/// six monthly RCI scan counters and the saved arcology total. The caller
/// selects this worker by engine fidelity; cross-layout cities read the Mac
/// counters and arcology total from their separate runtime state.
pub(in crate::app) fn mac_budget_refresh_rci_budget_currents(city: &mut City) {
    let rci_counts = std::array::from_fn::<_, 6, _>(|index| city.mac_rci_counter(index + 1) as i32);
    let arcology_total = city.mac_arcology_resident_total() as i32;
    for (row, numerator, divisor) in [(0, 0, 6_i32), (1, 2, 12), (2, 4, 12)] {
        let pair_total = rci_counts[numerator].wrapping_add(rci_counts[numerator + 1]);
        let current = pair_total
            .wrapping_mul(10)
            .wrapping_add(arcology_total / divisor);
        mac_budget_set_current(city, row, current as u32);
    }
}

/// Mac CODE 4 `export_227` refreshes these rows after the current month's
/// snapshot. Its inputs are the persisted Mac XBLD census words, not the PE
/// census, which is rebuilt with a different military-zone rule.
pub(in crate::app) fn mac_budget_refresh_derived_currents(city: &mut City) {
    mac_budget_refresh_rci_budget_currents(city);

    let census = std::array::from_fn::<_, 256, _>(|id| city.mac_xbld_tile_count(id as u8));
    let count = |building: u8| census[usize::from(building)];
    let current_by_row = [
        (5, u32::from(count(0xd2) / 9)),
        (6, u32::from(count(0xd3) / 9)),
        (7, u32::from(count(0xd1) / 9)),
        (8, u32::from(count(0xd6) / 9)),
        (9, u32::from(count(0xd9) >> 4)),
    ];
    for (row, current) in current_by_row {
        mac_budget_set_current(city, row, current);
    }

    let sum_ranges = |ranges: &[(u8, u8)]| {
        ranges.iter().fold(0_u32, |total, &(first, last)| {
            (first..=last).fold(total, |sum, id| sum.wrapping_add(u32::from(count(id))))
        })
    };
    let road = sum_ranges(&[(0x1d, 0x2b), (0x3f, 0x46), (0x4b, 0x4c), (0x5d, 0x60)])
        .wrapping_add(u32::from(count(236) >> 2).wrapping_mul(250) as u16 as u32);
    let highway = sum_ranges(&[(0x49, 0x50), (0x61, 0x6b)]);
    let bridge = sum_ranges(&[(0x51, 0x5c), (0x6a, 0x6b)]);
    let rail = sum_ranges(&[(0x2c, 0x3e), (0x45, 0x48), (0x6c, 0x6f), (0x4d, 0x4e)])
        .wrapping_add(u32::from(count(237)));
    let subway = u32::from(city.mac_xund_class_count().wrapping_add(count(0xe9)));
    let tunnel = sum_ranges(&[(0x3f, 0x42)]);
    for (row, current) in [
        (10, road),
        (11, highway),
        (12, bridge),
        (13, rail),
        (14, subway),
        (15, tunnel),
    ] {
        mac_budget_set_current(city, row, current);
    }
}

fn mac_budget_set_current(city: &mut City, row: usize, current: u32) {
    if let Some(offset) = budget_row_misc_offset_for_city(city, row, BUDGET_SERVICE_CURRENT_FIELD) {
        city.set_budget_misc_u32_values(offset, &[current]);
    }
}
