//! Budget cashflow, saved-row accounting and monthly current rebuilds.
//!
//! Preserve edition-specific row access, raw units, wrapping arithmetic and
//! snapshot/accrual order. Calendar/modal scheduling, native budget models and
//! interactive tax/funding/bond controls retain their existing owners.

use super::{
    BUDGET_ORDINANCE_ROW, BUDGET_SERVICE_COUNT, BUDGET_SERVICE_CURRENT_FIELD,
    BUDGET_SERVICE_FUNDING_FIELD, BUDGET_SERVICE_MISC_STRIDE, BUDGET_SERVICE_ROW_START,
    BUDGET_SERVICE_YEAR_TO_DATE_FIELD, BUDGET_TOTAL_COUNT, DEFAULT_TAX_RATE, PopulationStats,
    ViewerState, apply_pe_budget_ordinance_tail, budget_row_is_unset,
    budget_row_misc_offset_for_city, budget_service_built_counts_for_city,
    budget_service_funding_percent_for_city, budget_service_funding_raw_for_city,
    budget_service_misc_offset_for_city, budget_tax_bases_for_city_with_fidelity,
    budget_year_end_delta_for_city, mac_budget_year_end_delta_for_city,
    pe_budget_month_history_fields, pe_budget_month_index, pe_ordinance_raw_cash_total_for_city,
    sync_bond_budget_row_for_city, tax_rate_for_city,
};

impl ViewerState {
    pub(super) fn refresh_budget_service_rows_from_live_counts(&mut self) {
        let built_counts = budget_service_built_counts_for_city(&self.city);
        for index in 0..BUDGET_SERVICE_COUNT {
            let live_current = built_counts.by_budget_index(index);
            let Some(offset) = budget_service_misc_offset_for_city(&self.city, index, 0) else {
                continue;
            };
            let mut values = self
                .city
                .budget_misc_u32_values(offset, BUDGET_SERVICE_MISC_STRIDE);
            if values.len() < BUDGET_SERVICE_MISC_STRIDE {
                values.resize(BUDGET_SERVICE_MISC_STRIDE, 0);
            }
            let generated_empty = budget_row_is_unset(&self.city, BUDGET_SERVICE_ROW_START + index);
            let funding = if generated_empty {
                100
            } else if self.engine_fidelity.uses_win95() {
                // FUN_004734A0 updates service current/cost fields, not their
                // +0x64 funding DWORDs. UI clamping must not normalize a save
                // during the monthly current rebuild.
                budget_service_funding_raw_for_city(&self.city, index)
            } else {
                budget_service_funding_percent_for_city(&self.city, index)
            };
            values[BUDGET_SERVICE_CURRENT_FIELD] = live_current;
            values[BUDGET_SERVICE_FUNDING_FIELD] = funding;
            if generated_empty {
                values[BUDGET_SERVICE_YEAR_TO_DATE_FIELD] = 0;
                for month in 0..12 {
                    values[3 + month * 2] = 0;
                    values[4 + month * 2] = funding;
                }
            }
            self.city.set_budget_misc_u32_values(offset, &values);
        }
    }

    pub(super) fn apply_budget_year_end_settlement(&mut self) {
        if self.engine_fidelity.uses_win95() {
            let delta = budget_year_end_delta_for_city(&self.city);
            if delta != 0 {
                // FUN_004734A0 adds each signed row quotient directly to the
                // 32-bit funds DWORD, so the Win95 path wraps on overflow.
                self.city
                    .set_city_funds(self.city.city_funds().wrapping_add(delta as i32));
            }
        } else {
            let delta = mac_budget_year_end_delta_for_city(&self.city);
            if delta != 0 {
                // Mac `export_227` adds each signed row result directly into
                // the 32-bit city-funds longword, preserving machine wrap.
                self.city
                    .set_city_funds(self.city.city_funds().wrapping_add(delta));
            }
        }
    }

    pub(super) fn clear_budget_year_history(&mut self) {
        if !self.engine_fidelity.uses_win95() {
            // Mac export 227 clears only each row's raw year accumulator at the
            // January settlement. Its twelve monthly current/funding snapshots
            // remain in the active document and are overwritten month by month.
            for row_index in 0..BUDGET_TOTAL_COUNT {
                let Some(offset) = budget_row_misc_offset_for_city(
                    &self.city,
                    row_index,
                    BUDGET_SERVICE_YEAR_TO_DATE_FIELD,
                ) else {
                    continue;
                };
                if self.city.budget_misc_u32_at(offset).is_some() {
                    self.city.set_budget_misc_u32_values(offset, &[0]);
                }
            }
            return;
        }
        for row_index in 0..BUDGET_TOTAL_COUNT {
            let Some(offset) = budget_row_misc_offset_for_city(&self.city, row_index, 0) else {
                continue;
            };
            let mut values = self
                .city
                .budget_misc_u32_values(offset, BUDGET_SERVICE_MISC_STRIDE);
            if values.len() < BUDGET_SERVICE_MISC_STRIDE {
                values.resize(BUDGET_SERVICE_MISC_STRIDE, 0);
            }
            // PE FUN_004734A0's January settlement reads and clears only the
            // raw +0x68 year accumulator. It then overwrites this month's
            // +0x60/+0x64 history slots; the other eleven month pairs remain
            // intact. Preserve those snapshots and let year-to-date include
            // only elapsed months.
            values[BUDGET_SERVICE_YEAR_TO_DATE_FIELD] = 0;
            self.city.set_budget_misc_u32_values(offset, &values);
        }
        sync_bond_budget_row_for_city(&mut self.city, 0);
    }

    pub(super) fn refresh_monthly_budget_inputs(&mut self, population: PopulationStats) {
        self.refresh_tax_budget_rows(population);
    }

    pub(super) fn refresh_monthly_budget_rows(&mut self, population: PopulationStats) {
        // PE FUN_004734A0 snapshots and accrues the row values already in the
        // active document before rebuilding service and ordinance currents for
        // the next month. Keep that order so a newly observed service count or
        // ordinance total is not charged retroactively for the month just
        // completed.
        self.ensure_pe_ordinance_budget_defaults();
        self.accrue_current_budget_month();

        let month_index = pe_budget_month_index(self.game_clock.month());
        self.refresh_budget_service_rows_from_live_counts();
        sync_bond_budget_row_for_city(&mut self.city, month_index);
        self.refresh_pe_ordinance_budget_row(population);
        if self.engine_fidelity.uses_win95() {
            apply_pe_budget_ordinance_tail(
                &mut self.city,
                self.no_disasters_enabled,
                &mut self.pe_process_crt,
            );
        }
    }

    pub(super) fn refresh_tax_budget_rows(&mut self, population: PopulationStats) {
        let tax_bases =
            budget_tax_bases_for_city_with_fidelity(&self.city, population, self.engine_fidelity);
        for (index, current) in tax_bases.into_iter().enumerate() {
            let Some(offset) = budget_row_misc_offset_for_city(&self.city, index, 0) else {
                continue;
            };
            let mut values = self
                .city
                .budget_misc_u32_values(offset, BUDGET_SERVICE_MISC_STRIDE);
            if values.len() < BUDGET_SERVICE_MISC_STRIDE {
                values.resize(BUDGET_SERVICE_MISC_STRIDE, 0);
            }
            let generated_empty = budget_row_is_unset(&self.city, index);
            let tax_rate = if generated_empty {
                DEFAULT_TAX_RATE
            } else {
                tax_rate_for_city(&self.city, index)
            };
            values[BUDGET_SERVICE_CURRENT_FIELD] = current;
            values[BUDGET_SERVICE_FUNDING_FIELD] = tax_rate;
            if generated_empty {
                values[BUDGET_SERVICE_YEAR_TO_DATE_FIELD] = 0;
                for month in 0..12 {
                    values[3 + month * 2] = 0;
                    values[4 + month * 2] = tax_rate;
                }
            }
            self.city.set_budget_misc_u32_values(offset, &values);
        }
    }

    pub(super) fn accrue_current_budget_month(&mut self) {
        let month_index = pe_budget_month_index(self.game_clock.month());
        let (count_field, funding_field) = pe_budget_month_history_fields(month_index);
        for row_index in 0..BUDGET_TOTAL_COUNT {
            let Some(offset) = budget_row_misc_offset_for_city(&self.city, row_index, 0) else {
                continue;
            };
            let mut values = self
                .city
                .budget_misc_u32_values(offset, BUDGET_SERVICE_MISC_STRIDE);
            if values.len() < BUDGET_SERVICE_MISC_STRIDE {
                values.resize(BUDGET_SERVICE_MISC_STRIDE, 0);
            }
            values[count_field] = values[BUDGET_SERVICE_CURRENT_FIELD];
            values[funding_field] = values[BUDGET_SERVICE_FUNDING_FIELD];
            let monthly_product = values[BUDGET_SERVICE_CURRENT_FIELD]
                .wrapping_mul(values[BUDGET_SERVICE_FUNDING_FIELD]);
            values[BUDGET_SERVICE_YEAR_TO_DATE_FIELD] =
                values[BUDGET_SERVICE_YEAR_TO_DATE_FIELD].wrapping_add(monthly_product);
            self.city.set_budget_misc_u32_values(offset, &values);
        }
    }

    fn ensure_pe_ordinance_budget_defaults(&mut self) {
        if !budget_row_is_unset(&self.city, BUDGET_ORDINANCE_ROW) {
            return;
        }
        let Some(offset) = budget_row_misc_offset_for_city(&self.city, BUDGET_ORDINANCE_ROW, 0)
        else {
            return;
        };
        // FUN_0040E250's new-city row table initializes both ordinance current
        // and funding to one before the first FUN_004734A0 monthly pass.
        let mut values = [0_u32; BUDGET_SERVICE_MISC_STRIDE];
        values[BUDGET_SERVICE_CURRENT_FIELD] = 1;
        values[BUDGET_SERVICE_FUNDING_FIELD] = 1;
        for month in 0..12 {
            let (_, history_funding) = pe_budget_month_history_fields(month);
            values[history_funding] = 1;
        }
        self.city.set_budget_misc_u32_values(offset, &values);
    }

    fn refresh_pe_ordinance_budget_row(&mut self, population: PopulationStats) {
        let raw_total = pe_ordinance_raw_cash_total_for_city(&self.city, population) as u32;
        if let Some(offset) = budget_row_misc_offset_for_city(&self.city, BUDGET_ORDINANCE_ROW, 0) {
            self.city.set_budget_misc_u32_values(offset, &[raw_total]);
        }
    }
}
