//! Budget tax/funding controls and bond eligibility/action adapters.
//!
//! Preserve save-layout gates, selected-row edits, finance-check side effects and
//! status/reward/redraw order. Dialog routing, native finance models and monthly
//! accounting retain their existing owners.

use super::{
    BOND_DATA_MAX_COUNT, BUDGET_BOND_AMOUNT, BUDGET_SERVICE_CURRENT_FIELD,
    BUDGET_SERVICE_FUNDING_FIELD, BUDGET_SERVICE_MISC_STRIDE, MAX_TAX_RATE, ViewerState,
    append_bond_rate_for_city, bond_issue_rate_scaled_for_city, budget_row_misc_offset_for_city,
    budget_service_built_counts_for_city, budget_service_funding_misc_offset_for_city,
    budget_service_funding_percent_for_city, budget_service_funding_raw_for_city,
    budget_service_misc_offset_for_city, budget_tax_bases_for_city_with_fidelity,
    ensure_budget_misc_defaults_before_extension, ensure_industry_misc_defaults_before_extension,
    industry_stats_for_city, industry_tax_misc_offset, mac_bond_issue_city_value_allows_3a714,
    mac_bond_issue_city_value_would_allow_3a714, mac_bond_issue_rate_scaled_3a714,
    mac_industry_stats_for_city, outstanding_bond_count_for_city,
    pe_bond_issue_city_value_allows_41b270, pe_bond_issue_city_value_would_allow_41b270,
    remove_oldest_bond_rate_for_city, sync_bond_budget_row_for_city, tax_rate_for_city,
    tax_rate_misc_offset_for_city,
};

impl ViewerState {
    pub(super) fn adjust_budget_tax_rate(&mut self, tax_index: usize, delta: i32) -> bool {
        if tax_rate_misc_offset_for_city(&self.city, tax_index).is_none() {
            return false;
        }
        let current = tax_rate_for_city(&self.city, tax_index);
        let next = (current as i32 + delta).clamp(0, MAX_TAX_RATE as i32) as u32;
        if next == current {
            return false;
        }
        ensure_budget_misc_defaults_before_extension(&mut self.city);
        if let Some(row_offset) = budget_row_misc_offset_for_city(&self.city, tax_index, 0) {
            let population = self.live_population_stats_for_fidelity();
            let tax_bases = budget_tax_bases_for_city_with_fidelity(
                &self.city,
                population,
                self.engine_fidelity,
            );
            let mut values = self
                .city
                .budget_misc_u32_values(row_offset, BUDGET_SERVICE_MISC_STRIDE);
            values.resize(BUDGET_SERVICE_MISC_STRIDE, 0);
            values[BUDGET_SERVICE_CURRENT_FIELD] = tax_bases[tax_index];
            values[BUDGET_SERVICE_FUNDING_FIELD] = next;
            self.city.set_budget_misc_u32_values(row_offset, &values);
        }
        let (population, _) = self.refresh_live_misc_current_values();
        self.refresh_tax_budget_rows(population);
        self.city.mark_all_dirty();
        true
    }

    pub(super) fn adjust_industry_tax_rate(&mut self, industry_index: usize, delta: i32) -> bool {
        if !self.engine_fidelity.uses_win95() {
            if industry_index >= super::INDUSTRY_COUNT {
                return false;
            }
            let mut rows = mac_industry_stats_for_city(&self.city);
            let current = rows[industry_index].tax;
            let next = current.saturating_add(delta).clamp(0, 20);
            if next == current {
                return false;
            }
            rows[industry_index].tax = next;
            // CODE 5 writes the selected WORD, then calls CODE 4 export 228
            // with step zero: it clamps all rows and publishes their average.
            let mut sum = 0;
            for (index, row) in rows.iter().enumerate() {
                let rate = row.tax.clamp(0, 20) as u32;
                sum += rate;
                self.city.set_mac_state_misc_u32_at(
                    industry_tax_misc_offset(index).expect("valid Mac industry row"),
                    rate,
                );
            }
            if let Some(offset) = tax_rate_misc_offset_for_city(&self.city, 2) {
                self.city.set_mac_state_misc_u32_at(offset, (sum + 6) / 11);
            }
            self.city.mark_all_dirty();
            return true;
        }
        let Some(offset) = industry_tax_misc_offset(industry_index) else {
            return false;
        };
        let current = industry_stats_for_city(&self.city)[industry_index].tax;
        let next = (current + delta).clamp(0, MAX_TAX_RATE as i32);
        if next == current {
            return false;
        }
        ensure_industry_misc_defaults_before_extension(&mut self.city);
        let next = next as u32;
        self.city.set_misc_u32_at(offset, next);
        self.refresh_live_misc_current_values();
        self.city.mark_all_dirty();
        true
    }

    pub(super) fn adjust_budget_service_funding(
        &mut self,
        service_index: usize,
        delta: i32,
    ) -> bool {
        let Some(offset) = budget_service_funding_misc_offset_for_city(&self.city, service_index)
        else {
            return false;
        };
        if self.engine_fidelity.uses_win95() {
            let current = budget_service_funding_percent_for_city(&self.city, service_index);
            let next = (current as i32 + delta).clamp(0, 100) as u32;
            if next == current {
                return false;
            }
            ensure_budget_misc_defaults_before_extension(&mut self.city);
            if let Some(row_offset) =
                budget_service_misc_offset_for_city(&self.city, service_index, 0)
            {
                let live_current =
                    budget_service_built_counts_for_city(&self.city).by_budget_index(service_index);
                let mut values = self
                    .city
                    .budget_misc_u32_values(row_offset, BUDGET_SERVICE_MISC_STRIDE);
                values.resize(BUDGET_SERVICE_MISC_STRIDE, 0);
                if values[BUDGET_SERVICE_CURRENT_FIELD] == 0 {
                    values[BUDGET_SERVICE_CURRENT_FIELD] = live_current;
                }
                values[BUDGET_SERVICE_FUNDING_FIELD] = next;
                self.city.set_budget_misc_u32_values(row_offset, &values);
            } else {
                self.city.set_misc_u32_at(offset, next);
            }
            self.refresh_live_misc_current_values();
            self.refresh_budget_service_rows_from_live_counts();
            self.refresh_reward_availability();
        } else {
            // CODE 4's budget action checks the signed saved funding before
            // stepping it and leaves current, accrual and history untouched.
            let current = budget_service_funding_raw_for_city(&self.city, service_index) as i32;
            if delta == 0 || (delta > 0 && current >= 100) || (delta < 0 && current <= 0) {
                return false;
            }
            ensure_budget_misc_defaults_before_extension(&mut self.city);
            self.city
                .set_budget_misc_u32_values(offset, &[current.wrapping_add(delta) as u32]);
        }
        if !self.query_is_inspect {
            self.refresh_status_line();
        }
        self.city.mark_all_dirty();
        true
    }

    pub(super) fn issue_budget_bond(&mut self) -> bool {
        let current_bonds = outstanding_bond_count_for_city(&self.city);
        if current_bonds >= BOND_DATA_MAX_COUNT as u32 {
            return false;
        }
        // The edition-specific issue gate is rooted in each binary's finance path.
        let may_issue = if self.engine_fidelity.uses_win95() {
            pe_bond_issue_city_value_allows_41b270(&mut self.city)
        } else {
            mac_bond_issue_city_value_allows_3a714(&mut self.city)
        };
        if !may_issue {
            return false;
        }
        let rate_scaled = if self.engine_fidelity.uses_win95() {
            bond_issue_rate_scaled_for_city(&self.city)
        } else {
            let Some(rate) = mac_bond_issue_rate_scaled_3a714(&self.city) else {
                return false;
            };
            rate
        };
        if !append_bond_rate_for_city(&mut self.city, rate_scaled) {
            return false;
        }
        // PE `0x473ae5` and Mac CODE 4 `0x2cc38` both use a 32-bit ADD.
        self.city
            .set_city_funds(self.city.city_funds().wrapping_add(BUDGET_BOND_AMOUNT));
        let month_index = usize::from(self.game_clock.month().saturating_sub(1)).min(11);
        sync_bond_budget_row_for_city(&mut self.city, month_index);
        if !self.query_is_inspect {
            self.refresh_status_line();
        }
        self.city.mark_all_dirty();
        true
    }

    pub(super) fn budget_bond_action_enabled(&self, issue: bool) -> bool {
        if issue {
            if outstanding_bond_count_for_city(&self.city) >= BOND_DATA_MAX_COUNT as u32 {
                return false;
            }
            return if self.engine_fidelity.uses_win95() {
                pe_bond_issue_city_value_would_allow_41b270(&self.city)
            } else {
                mac_bond_issue_city_value_would_allow_3a714(&self.city)
            };
        }
        outstanding_bond_count_for_city(&self.city) > 0
            && self.city.city_funds() >= BUDGET_BOND_AMOUNT
    }

    pub(super) fn repay_budget_bond(&mut self) -> bool {
        if outstanding_bond_count_for_city(&self.city) == 0
            || self.city.city_funds() < BUDGET_BOND_AMOUNT
        {
            return false;
        }
        if !remove_oldest_bond_rate_for_city(&mut self.city) {
            return false;
        }
        self.city
            .set_city_funds(self.city.city_funds().saturating_sub(BUDGET_BOND_AMOUNT));
        let month_index = usize::from(self.game_clock.month().saturating_sub(1)).min(11);
        sync_bond_budget_row_for_city(&mut self.city, month_index);
        if !self.query_is_inspect {
            self.refresh_status_line();
        }
        self.city.mark_all_dirty();
        true
    }
}
