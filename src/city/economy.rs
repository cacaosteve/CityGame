//! Win95 and Mac national economy, neighbor, and industry-mix simulation.

use super::*;

pub(super) const PE_NEIGHBOR_COUNT: usize = 4;

pub(super) const PE_NEIGHBOR_RECORD_BYTES: usize = 16;
pub(super) const PE_NEIGHBOR_NAME_FIELD: usize = 0;
pub(super) const PE_NEIGHBOR_POP_FIELD: usize = 4;
pub(super) const PE_NEIGHBOR_VALUE_FIELD: usize = 8;
pub(super) const PE_NEIGHBOR_FAME_FIELD: usize = 12;

/// Win95 industry triples packed by `FUN_0043153a` (`0x4CA3F4` / `0x4CB3E0` / `0x4CB3F0`).
pub(super) const MISC_INDUSTRY_TABLE_OFFSET: usize = 0x016c;
pub(super) const PE_INDUSTRY_COUNT: usize = 11;
pub(super) const PE_INDUSTRY_RECORD_BYTES: usize = 12;
pub(super) const PE_INDUSTRY_DEMAND_FIELD: usize = 0;
pub(super) const PE_INDUSTRY_TAX_FIELD: usize = 4;
pub(super) const PE_INDUSTRY_POP_FIELD: usize = 8;

/// CODE 8's zero-app-LFSR demand update narrows the product to a signed
/// WORD before dividing by four.
fn mac_industry_smoothed_demand(prior: i16) -> i16 {
    (i32::from(prior.wrapping_mul(3)) / 4) as i16
}

/// Multiply a LONG by a positive SANE extended constant, round the product to
/// a 64-bit significand using ties-to-even, then apply FINTRZ/FX2L. The two
/// supported exponents cover the four CODE 8 modifier constants. SANE
/// exceptions and a changed process floating-point environment are not modeled.
pub(super) fn mac_sane_industry_scale(
    value: i32,
    significand: u64,
    below_one: bool,
) -> Option<i32> {
    let magnitude = i64::from(value).unsigned_abs() as u128;
    let product = magnitude * u128::from(significand);
    let bits = 128 - product.leading_zeros();
    let shift = bits.saturating_sub(64);
    let mut rounded = product >> shift;
    if shift != 0 {
        let remainder = product & ((1_u128 << shift) - 1);
        let halfway = 1_u128 << (shift - 1);
        if remainder > halfway || (remainder == halfway && rounded & 1 != 0) {
            rounded += 1;
        }
    }
    let rounded_product = rounded << shift;
    let integer = rounded_product >> if below_one { 64 } else { 63 };
    let signed = if value < 0 {
        -(integer as i64)
    } else {
        integer as i64
    };
    i32::try_from(signed).ok()
}

/// CODE 8's demand-to-residual loop using the reconstructed zero app-LFSR and
/// standard SANE environment. The returned total is accumulated during the
/// loop and may differ from the final weights after later row transforms.
/// Original SANE runtime and the whole-session random sequence remain unverified.
#[allow(dead_code)]
pub(super) fn mac_industry_modified_residuals(
    prior: [i16; PE_INDUSTRY_COUNT],
    taxes: [i16; PE_INDUSTRY_COUNT],
    pollution_controls: bool,
    growth_modifier: bool,
    education: u32,
) -> Option<([i16; PE_INDUSTRY_COUNT], [u32; PE_INDUSTRY_COUNT], u32)> {
    const F08: u64 = 0xcccc_cccc_cccc_cccd;
    const F09: u64 = 0xe666_6666_6666_6666;
    const F11: u64 = 0x8ccc_cccc_cccc_cccd;
    const F12: u64 = 0x9999_9999_9999_999a;
    let updated = prior.map(mac_industry_smoothed_demand);
    let mut weights = [0_i32; PE_INDUSTRY_COUNT];
    let mut total = 0_u32;
    for i in 0..PE_INDUSTRY_COUNT {
        weights[i] = i32::from(updated[i]);
        if pollution_controls {
            for j in [0, 1, 2, 5] {
                weights[j] = mac_sane_industry_scale(weights[j], F09, true)?;
            }
        }
        if growth_modifier {
            weights[4] = mac_sane_industry_scale(weights[4], F11, false)?;
        }
        if education > 130 {
            for j in [6, 9] {
                weights[j] = mac_sane_industry_scale(weights[j], F12, false)?;
            }
        } else if education > 100 {
            for j in [2, 5, 7, 8, 6, 9] {
                weights[j] = mac_sane_industry_scale(weights[j], F11, false)?;
            }
        } else if education < 60 {
            for j in [6, 9] {
                weights[j] = mac_sane_industry_scale(weights[j], F08, true)?;
            }
        }
        weights[i] = weights[i].wrapping_sub(i32::from(taxes[i])).max(0);
        total = total.wrapping_add(weights[i] as u32);
    }
    Some((updated, weights.map(|value| value as u32), total))
}

/// Demand-to-residual path when pollution controls, the growth modifier, and
/// all education modifiers are inactive. The SANE-modified paths and shared
/// Toolbox chronology are still required before this can be scheduled.
#[allow(dead_code)]
pub(super) fn mac_industry_neutral_residuals(
    prior: [i16; PE_INDUSTRY_COUNT],
    taxes: [i16; PE_INDUSTRY_COUNT],
) -> ([i16; PE_INDUSTRY_COUNT], [u32; PE_INDUSTRY_COUNT], u32) {
    let updated = prior.map(mac_industry_smoothed_demand);
    let weights =
        std::array::from_fn(|i| (i32::from(updated[i]) - i32::from(taxes[i])).max(0) as u32);
    let sum = weights
        .iter()
        .fold(0_u32, |acc, &value| acc.wrapping_add(value));
    (updated, weights, sum)
}

/// CODE 8 export 238's two industry-mix WORD outputs, after its population
/// balancing. The live scheduler cannot call this yet: that earlier balancing
/// and its QuickDraw chronology are not integrated. A zero denominator is an
/// unsupported native divide exception, rather than a fabricated output.
#[allow(dead_code)]
pub(super) fn mac_industry_mix_bonuses(
    industrial_total: u32,
    populations: [u32; PE_INDUSTRY_COUNT],
) -> Option<(i16, i16)> {
    let denominator = industrial_total.wrapping_add(1);
    if denominator == 0 {
        return None;
    }
    let dirty = [0, 1, 2, 5]
        .into_iter()
        .fold(0_u32, |sum, index| sum.wrapping_add(populations[index]));
    let dirty_share = dirty.wrapping_mul(100) / denominator;
    let pollution = if (dirty_share as i32) < 20 {
        -1
    } else {
        ((dirty_share as i32 - 20) / 30) as i16
    };
    let maximum = populations
        .into_iter()
        .map(|value| value.wrapping_mul(100) / denominator)
        .max()
        .unwrap_or(0);
    let mix = if (maximum as i32) < 20 {
        0
    } else {
        ((maximum as i32 - 20) / 5) as i16
    };
    Some((pollution, mix))
}

/// CODE 8 export 238's population balancing, given the already computed
/// nonnegative residual weights, their separately accumulated total, and a
/// stream of Toolbox random words. Later row transforms can change earlier
/// weights after their contributions entered that total. The
/// preceding demand/ordinance transforms and process-wide RNG chronology are
/// still unported, so this bounded worker is not scheduled on its own.
#[allow(dead_code)]
pub(super) fn mac_industry_rebalance<F: FnMut() -> u16>(
    target: u32,
    mut populations: [u32; PE_INDUSTRY_COUNT],
    residual_weights: [u32; PE_INDUSTRY_COUNT],
    weights_total: u32,
    mut random_word: F,
) -> [u32; PE_INDUSTRY_COUNT] {
    let total = populations
        .iter()
        .fold(0_u32, |sum, &v| sum.wrapping_add(v));
    let shrinking = total > target;
    let (scale, divisor) = if shrinking {
        (total.wrapping_sub(target).wrapping_mul(100), total)
    } else if weights_total != 0 {
        (target.wrapping_sub(total).wrapping_mul(100), weights_total)
    } else {
        return populations;
    };
    for (index, population) in populations.iter_mut().enumerate() {
        let weight = if shrinking {
            *population
        } else {
            residual_weights[index]
        };
        if !shrinking && weight == 0 {
            continue;
        }
        let fraction = weight.wrapping_mul(scale) / divisor;
        let whole = (fraction as i32) / 100;
        if shrinking {
            *population = population.wrapping_sub(whole as u32);
        } else {
            *population = population.wrapping_add(whole as u32);
        }
        let remainder = fraction.wrapping_sub((whole as u32).wrapping_mul(100));
        if (remainder as i32) > i32::from(random_word() % 100) {
            if shrinking {
                *population = population.wrapping_sub(1);
            } else {
                *population = population.wrapping_add(1);
            }
        }
    }
    populations
}

/// Win95 Pollution Controls ordinance bit (`0x4caa40` bit `0x80000`).
pub(super) const PE_INDUSTRY_POLLUTION_CONTROLS: u32 = 0x80000;

pub(super) const PE_NATIONAL_POP_CAP: u32 = 5_000_000;
/// Win95 `0x3567E0`: national value mean-reversion cap.
pub(super) const PE_NATIONAL_VALUE_CAP: u32 = 3_500_000;
/// Win95 dword `0x4DAEF0` = `1/1200`.
pub(super) const PE_NATIONAL_DRIFT_SCALE: f64 = 1.0 / 1200.0;
/// Win95 words `0x4E9558`: value-drift speed for trends 0..=3.
pub(super) const PE_NATIONAL_VALUE_TREND_SPEED: [i32; 4] = [6, 3, 0, -3];
/// Mac CODE 8 phase-21 population reference at `0x39172`.
pub(super) const MAC_NATIONAL_POP_CAP: u32 = 5_000_000;
/// Mac CODE 8 phase-21 value reference at `0x3925e`.
pub(super) const MAC_NATIONAL_VALUE_CAP: u32 = 3_500_000;
/// Mac A5-`0x31c4` signed WORD table used by `export_238` for value drift.
pub(super) const MAC_NATIONAL_VALUE_TREND_SPEED: [i32; 4] = [6, 3, 0, -3];
/// Win95 qword `0x4DAF00`: rare neighbor-city pop crash scale.
pub(super) const PE_NEIGHBOR_CRASH_POP_SCALE: f64 = 0.75;
/// Win95 qword `0x4DAF08`: rare neighbor-city value crash scale.
pub(super) const PE_NEIGHBOR_CRASH_VALUE_SCALE: f64 = 0.5;
/// Win95 dwords `0x4E9458`: industry era weights, 50-year rows × 11 industries.
pub(super) const PE_INDUSTRY_ERA_TABLE: [[i32; PE_INDUSTRY_COUNT]; 5] = [
    [20, 20, 10, 10, 15, 5, 0, 15, 8, 0, 10],
    [30, 20, 40, 10, 20, 40, 20, 20, 16, 10, 20],
    [35, 20, 30, 10, 25, 40, 30, 25, 24, 80, 30],
    [20, 50, 20, 10, 20, 30, 40, 30, 40, 80, 40],
    [10, 20, 10, 10, 20, 20, 50, 30, 20, 80, 50],
];

pub(super) fn pe_industry_field_offset(index: usize, field: usize) -> usize {
    MISC_INDUSTRY_TABLE_OFFSET + index * PE_INDUSTRY_RECORD_BYTES + field
}

/// PE `0x4693d3..0x4693d6` / `0x469443..0x469446`: 32-bit IMUL keeps the
/// low dword, then unsigned DIV divides that dword by the supplied denominator.
pub(super) fn pe_industry_wrapped_mul_div(lhs: u32, rhs: u32, denominator: u32) -> i32 {
    (lhs.wrapping_mul(rhs) / denominator) as i32
}

/// Win95 `cdq; and mask; add; sar` truncating signed divide by `2^shift`.
pub(super) fn pe_sar_div(value: i32, shift: u32) -> i32 {
    let mask = (1_i32 << shift).wrapping_sub(1);
    let bias = if value < 0 { mask } else { 0 };
    value.wrapping_add(bias) >> shift
}

pub(super) fn pe_industry_era_weights(
    founded_year: u32,
    elapsed_years: u32,
) -> [i32; PE_INDUSTRY_COUNT] {
    let era = founded_year.saturating_sub(1900) / 50 + elapsed_years / 50;
    let rem = elapsed_years % 50;
    if era >= 4 {
        return PE_INDUSTRY_ERA_TABLE[4];
    }
    let era = era as usize;
    let current = PE_INDUSTRY_ERA_TABLE[era];
    let next = PE_INDUSTRY_ERA_TABLE[era + 1];
    let mut weights = [0_i32; PE_INDUSTRY_COUNT];
    for index in 0..PE_INDUSTRY_COUNT {
        let blended = current[index] * (50 - rem as i32) + next[index] * rem as i32;
        weights[index] = blended / 50;
    }
    weights
}

pub(super) fn pe_industry_scale_leftover_globals(
    leftover: &mut [i32; PE_INDUSTRY_COUNT],
    pollution_controls: bool,
    construction_boost: bool,
    education: u32,
) {
    let scale = |slot: &mut i32, factor: f64| {
        *slot = pe_ftol_chop(f64::from(*slot) * factor);
    };
    if pollution_controls {
        for index in [0, 1, 2, 5] {
            scale(&mut leftover[index], 0.9);
        }
    }
    if construction_boost {
        scale(&mut leftover[4], 1.1);
    }
    if education > 130 {
        scale(&mut leftover[6], 1.2);
        scale(&mut leftover[9], 1.2);
    } else if education > 100 {
        for index in [2, 5, 7, 8, 6, 9] {
            scale(&mut leftover[index], 1.1);
        }
    } else if education < 60 {
        scale(&mut leftover[6], 0.8);
        scale(&mut leftover[9], 0.8);
    }
}

pub(super) fn pe_industry_apply_pop_delta(
    city: &mut City,
    index: usize,
    quota: i32,
    delta_sign: i32,
    rng: &mut Win95CrtRand,
) {
    let integer = quota / 100;
    let remainder = quota - integer.wrapping_mul(100);
    let offset = pe_industry_field_offset(index, PE_INDUSTRY_POP_FIELD);
    let pop = city.pe_industry_pop_at(index);
    let next = if delta_sign < 0 {
        pop.wrapping_sub(integer as u32)
    } else {
        pop.wrapping_add(integer as u32)
    };
    city.set_misc_u32_at(offset, next);
    if (i32::from(rng.random_u15() % 100)) < remainder {
        let pop = city.pe_industry_pop_at(index);
        let next = if delta_sign < 0 {
            pop.wrapping_sub(1)
        } else {
            pop.wrapping_add(1)
        };
        city.set_misc_u32_at(offset, next);
    }
}

pub(super) fn pe_neighbor_field_offset(index: usize, field: usize) -> usize {
    MISC_NEIGHBOR_TABLE_OFFSET + index * PE_NEIGHBOR_RECORD_BYTES + field
}

/// Win95 neighbor prosperity `trunc(value / pop * 100)` (`0x468fd4`, not `pop+1`).
pub(super) fn pe_neighbor_prosperity(value: u32, pop: u32) -> i32 {
    if pop == 0 {
        return 0;
    }
    pe_ftol_x87_divide_then_mul_100(value, pop)
}

/// MSVC `_ftol` chop toward zero (`0x48d48c`, RC=chop).
pub(super) fn pe_ftol_chop(value: f64) -> i32 {
    value.trunc() as i32
}

pub(super) fn pe_national_apply_drift(current: u32, cap: u32, speed: i32) -> u32 {
    // `FUN_00468d30` calls `_ftol` (FISTP qword), then uses EAX in its
    // dword ADD/SUB. Preserve that low-dword result even when a signed saved
    // tax word makes a neighbor value speed large enough to exceed i32.
    let delta = pe_ftol_low_dword((current as f64) * (speed as f64) * PE_NATIONAL_DRIFT_SCALE);
    if current > cap {
        current.wrapping_sub(delta)
    } else {
        current.wrapping_add(delta)
    }
}

/// Mac CODE 8 `export_238`: SANE computes a Comp×integer/1200 drift, converts
/// that delta to single, adds/subtracts it from the Comp value, then `FINTRZ`
/// truncates the final total before the low dword is stored.
pub(super) fn mac_sane_national_apply_drift(current: u32, cap: u32, speed: i32) -> u32 {
    let delta = ((current as f64) * (speed as f64) / 1200.0) as f32;
    let adjusted = if current > cap {
        (current as f64) - f64::from(delta)
    } else {
        (current as f64) + f64::from(delta)
    };
    adjusted.trunc() as i64 as u32
}

/// Low EAX result used by PE drift callers after `_ftol` stores a signed qword.
pub(super) fn pe_ftol_low_dword(value: f64) -> u32 {
    value.trunc() as i64 as u32
}

pub(super) fn pe_national_value_speed(trend: i32) -> i32 {
    usize::try_from(trend)
        .ok()
        .and_then(|index| PE_NATIONAL_VALUE_TREND_SPEED.get(index).copied())
        .unwrap_or(0)
}

impl City {
    /// Win95 national economy prelude of `SimulationEQ_LE_Processing` (`0x468d30`).
    ///
    /// Pop mean-reverts around 5_000_000 with speed `wNationalEconomyTrend`;
    /// value uses word table `0x4e9558` = `{6,3,0,-3}` around 3_500_000. About
    /// 1/10 months (`rand%10`) consider tax (`story 9`/`0xA`) and trend (`story 7`).
    pub fn advance_pe_national_economy_month(&mut self, rng: &mut Win95CrtRand) {
        let trend = self.pe_national_trend_word();
        let pop = self.pe_dw_national_population;
        let value = self.pe_dw_national_value;
        self.set_misc_u32_at(
            MISC_NATIONAL_POPULATION_OFFSET,
            pe_national_apply_drift(pop, PE_NATIONAL_POP_CAP, trend),
        );
        self.set_misc_u32_at(
            MISC_NATIONAL_VALUE_OFFSET,
            pe_national_apply_drift(value, PE_NATIONAL_VALUE_CAP, pe_national_value_speed(trend)),
        );

        if rng.random_u15() % 10 != 0 {
            return;
        }
        let pop = self.pe_dw_national_population;
        let value = self.pe_dw_national_value;
        let prosperity = pe_national_prosperity(value, pop);

        if rng.random_u15() % 5 < 2 {
            let tax = self.pe_national_tax_word();
            let denom = tax.saturating_mul(25).unsigned_abs().max(1);
            let roll = u32::from(rng.random_u15()) % denom;
            if (roll as i32) < prosperity {
                let tax = (tax as i16).wrapping_add(1) as i32;
                self.set_misc_u32_at(MISC_NATIONAL_TAX_OFFSET, tax as u32);
                self.queue_pe_newspaper_story(
                    PE_NEWSPAPER_STORY_NATIONAL_TAX_UP,
                    tax.clamp(0, i32::from(u8::MAX)) as u8,
                );
            }
            let tax = self.pe_national_tax_word();
            let denom = tax.saturating_mul(25).unsigned_abs().max(1);
            let roll = u32::from(rng.random_u15()) % denom;
            if (roll as i32) > prosperity {
                let mut tax = (tax as i16).wrapping_sub(1);
                if tax == 0 {
                    tax = 1;
                }
                self.set_misc_u32_at(MISC_NATIONAL_TAX_OFFSET, tax as u32);
                self.queue_pe_newspaper_story(
                    PE_NEWSPAPER_STORY_NATIONAL_TAX_DOWN,
                    (i32::from(tax)).clamp(0, i32::from(u8::MAX)) as u8,
                );
            }
        }

        if rng.random_u15() % 3 != 0 {
            return;
        }
        let next_trend = pe_national_trend_from_prosperity(prosperity);
        if next_trend != self.pe_national_trend_word() {
            self.set_misc_u32_at(MISC_NATIONAL_TREND_OFFSET, next_trend as u32);
            self.queue_pe_newspaper_story(
                PE_NEWSPAPER_STORY_NATIONAL_TREND,
                next_trend.clamp(0, i32::from(u8::MAX)) as u8,
            );
        }
    }

    /// Mac CODE 8 `export_238` phase-21 deterministic national-population and
    /// value drift. The later QuickDraw-gated tax/trend event arm is separate.
    pub(crate) fn advance_mac_national_metric_drift_month(&mut self) {
        let trend = i32::from(self.pe_w_national_trend as i16);
        let population = self.pe_dw_national_population;
        let value = self.pe_dw_national_value;
        let next_population =
            mac_sane_national_apply_drift(population, MAC_NATIONAL_POP_CAP, trend);
        self.set_mac_state_misc_u32_at(MISC_NATIONAL_POPULATION_OFFSET, next_population);
        // These are shared national figures, including when the loaded save
        // uses the Win95 layout. The Mac-specific MISC writer deliberately
        // avoids synchronizing unrelated PE caches in that layout.
        self.pe_dw_national_population = next_population;

        // Native code indexes A5-0x31c4 by the signed trend WORD. The four
        // ordinary tiers are the rooted [6, 3, 0, -3] table. For malformed
        // trend values, the original indexed read escapes this table; avoid
        // inventing an adjacent-global interpretation here.
        if let Some(value_speed) = usize::try_from(trend)
            .ok()
            .and_then(|index| MAC_NATIONAL_VALUE_TREND_SPEED.get(index).copied())
        {
            let next_value =
                mac_sane_national_apply_drift(value, MAC_NATIONAL_VALUE_CAP, value_speed);
            self.set_mac_state_misc_u32_at(MISC_NATIONAL_VALUE_OFFSET, next_value);
            self.pe_dw_national_value = next_value;
        }
    }

    /// Win95 neighbor pop/value monthly from `SimulationEQ_LE_Processing` (`0x468f34`).
    ///
    /// Packed MISC `0x06d8` is `[name, pop, value, fame] × 4`. Zero-pop neighbors
    /// (ocean) skip the monthly drift. Speed is `trend + (rand % 3)` around the
    /// same 5_000_000 cap, with `pop += rand & 1` when the truncated delta is 0.
    /// Value speed is `table[trend] + (rand % 5) - national_tax - local_class`
    /// around 3_500_000. Local class uses `trunc(value/pop*100)` vs 45/60/75.
    /// After the four neighbors, 1/64 months (`rand & 0x3f == 0`) pick one city
    /// (`rand & 3`) and scale pop by 0.75 / value by 0.5 (`0x4daf00` / `0x4daf08`).
    pub fn advance_pe_neighbor_economy_month(&mut self, rng: &mut Win95CrtRand) {
        let trend = self.pe_national_trend_word();
        let tax = self.pe_national_tax_word();
        for index in 0..PE_NEIGHBOR_COUNT {
            let pop_offset = pe_neighbor_field_offset(index, PE_NEIGHBOR_POP_FIELD);
            let value_offset = pe_neighbor_field_offset(index, PE_NEIGHBOR_VALUE_FIELD);
            let pop = self.misc_u32_at(pop_offset).unwrap_or(0);
            if pop == 0 {
                continue;
            }
            let speed = trend + i32::from(rng.random_u15() % 3);
            let delta = pe_ftol_chop((pop as f64) * (speed as f64) * PE_NATIONAL_DRIFT_SCALE);
            let mut next_pop = pe_national_apply_drift(pop, PE_NATIONAL_POP_CAP, speed);
            if delta == 0 {
                next_pop = pop.wrapping_add(u32::from(rng.random_u15() & 1));
            }
            self.set_misc_u32_at(pop_offset, next_pop);

            let value = self.misc_u32_at(value_offset).unwrap_or(0);
            let local_class =
                pe_national_trend_from_prosperity(pe_neighbor_prosperity(value, next_pop));
            let value_speed = pe_national_value_speed(trend) + i32::from(rng.random_u15() % 5)
                - tax
                - local_class;
            self.set_misc_u32_at(
                value_offset,
                pe_national_apply_drift(value, PE_NATIONAL_VALUE_CAP, value_speed),
            );
        }

        if rng.random_u15() as u8 & 0x3f != 0 {
            return;
        }
        let index = (rng.random_u15() & 3) as usize;
        let pop_offset = pe_neighbor_field_offset(index, PE_NEIGHBOR_POP_FIELD);
        let value_offset = pe_neighbor_field_offset(index, PE_NEIGHBOR_VALUE_FIELD);
        let pop = self.misc_u32_at(pop_offset).unwrap_or(0);
        let value = self.misc_u32_at(value_offset).unwrap_or(0);
        self.set_misc_u32_at(
            pop_offset,
            pe_ftol_chop((pop as f64) * PE_NEIGHBOR_CRASH_POP_SCALE) as u32,
        );
        self.set_misc_u32_at(
            value_offset,
            pe_ftol_chop((value as f64) * PE_NEIGHBOR_CRASH_VALUE_SCALE) as u32,
        );
    }

    /// Win95 fresh-city neighbor rows from `FUN_0040E250` (`0x40e575..0x40e68f`).
    /// Four distinct name indices are selected from 1..=36. Each population is
    /// the minimum of three `100 + rand()%7400` draws; value is population
    /// divided by `1 + rand()%3`, and the initial fame field is zero.
    pub fn pe_initialize_new_city_neighbors_40e250(&mut self, crt: &mut Win95CrtRand) {
        let mut names = [0u16; PE_NEIGHBOR_COUNT];
        for index in 0..PE_NEIGHBOR_COUNT {
            let mut name = crt.random_u15() % 0x24 + 1;
            while names[..index].contains(&name) {
                name = crt.random_u15() % 0x24 + 1;
            }
            names[index] = name;

            let population = (0..3)
                .map(|_| u32::from(crt.random_u15() % 0x1ce8 + 100))
                .min()
                .expect("three population draws");
            let value_divisor = u32::from(crt.random_u15() % 3 + 1);
            let base = pe_neighbor_field_offset(index, PE_NEIGHBOR_NAME_FIELD);
            self.set_misc_u32_at(base, u32::from(name));
            self.set_misc_u32_at(base + PE_NEIGHBOR_POP_FIELD, population);
            self.set_misc_u32_at(base + PE_NEIGHBOR_VALUE_FIELD, population / value_divisor);
            self.set_misc_u32_at(base + PE_NEIGHBOR_FAME_FIELD, 0);
        }
    }

    /// The deterministic demand-word slice of Mac CODE 8 export 238.
    /// The app-owned LFSR starts at zero and is a fixed point, so its four
    /// samples per row contribute zero. MULS.W followed by EXT.L retains the
    /// low signed WORD of the product before the signed division by four.
    /// Mac's subsequent industry balancing remains a separate, unported step.
    pub(crate) fn advance_mac_industry_demand_words_month(&mut self) {
        // Both save layouts carry the same 11 demand/tax/population triples.
        // The selected engine, not the source-file layout, owns this update.
        for index in 0..PE_INDUSTRY_COUNT {
            let offset = MISC_INDUSTRY_TABLE_OFFSET + index * PE_INDUSTRY_RECORD_BYTES;
            let prior = self.mac_state_misc_u32_at(offset).unwrap_or(0) as u16 as i16;
            let next = mac_industry_smoothed_demand(prior);
            self.set_mac_state_misc_u32_at(offset, i32::from(next) as u32);
        }
    }

    /// Advance the PE demand WORD for one industry, including its four
    /// game-LFSR draws. Both city formats store these eleven rows alike.
    fn advance_pe_industry_demand_row(
        &mut self,
        index: usize,
        weight: i32,
        lfsr: &mut Win95Lfsr,
    ) -> i32 {
        let demand = self.pe_industry_demand_at(index);
        let jitter = (0..4).fold(0_i32, |sum, _| sum + i32::from(lfsr.random_mask_7f()));
        let next = pe_sar_div(
            demand
                .wrapping_mul(3)
                .wrapping_add(pe_sar_div(jitter.wrapping_mul(weight), 8)),
            2,
        ) as i16 as i32;
        self.set_misc_u32_at(
            pe_industry_field_offset(index, PE_INDUSTRY_DEMAND_FIELD),
            next as u32,
        );
        next
    }

    /// Standalone demand slice used by focused worker comparisons.
    #[cfg(test)]
    pub(super) fn advance_pe_industry_demand_month(&mut self, lfsr: &mut Win95Lfsr) {
        let weights =
            pe_industry_era_weights(self.year_founded().unwrap_or(1900), self.pe_elapsed_years());
        for (index, weight) in weights.into_iter().enumerate() {
            self.advance_pe_industry_demand_row(index, weight, lfsr);
        }
    }

    /// Win95 industry mix monthly from `SimulationEQ_LE_Processing` (`0x46910a`).
    ///
    /// Demand is a 3/4 leaky update plus four `RandomAnd7F` samples times the era
    /// weight (`0x4e9458`, interpolated by city age / 50). Leftover `demand-tax`
    /// (Pollution Controls / EQ / construction overflow scaled) grows or shrinks
    /// `dwArrIndustrialPopulations` toward `zonePopsBase[2]`. Mix bonuses then
    /// use dirty-industry / max share against that same industrial zone pop.
    pub fn advance_pe_industry_mix_month(
        &mut self,
        industrial_zone_pop: u32,
        rci_zone_people: u32,
        city_population: u32,
        rng: &mut Win95CrtRand,
        lfsr: &mut Win95Lfsr,
    ) {
        let weights =
            pe_industry_era_weights(self.year_founded().unwrap_or(1900), self.pe_elapsed_years());
        let pollution_controls =
            self.ordinance_flags_for_saved_layout() & PE_INDUSTRY_POLLUTION_CONTROLS != 0;
        let construction_boost = rci_zone_people > city_population;
        let education = self
            .misc_u32_at(MISC_CITY_EDUCATION_QUOTIENT_OFFSET)
            .unwrap_or(0);
        let mut leftover = [0_i32; PE_INDUSTRY_COUNT];
        let mut leftover_sum = 0_i32;
        let mut mix_pop = 0_u32;

        for index in 0..PE_INDUSTRY_COUNT {
            mix_pop = mix_pop.wrapping_add(self.pe_industry_pop_at(index));
            let next = self.advance_pe_industry_demand_row(index, weights[index], lfsr);
            leftover[index] = next;
            // PE runs these whole-vector transforms inside the row loop. Later
            // rows can therefore rescale an earlier leftover after it has
            // already been added to `leftover_sum` below.
            pe_industry_scale_leftover_globals(
                &mut leftover,
                pollution_controls,
                construction_boost,
                education,
            );
            let taxed = leftover[index] - self.pe_industry_tax_at(index);
            leftover[index] = taxed.max(0);
            leftover_sum += leftover[index];
        }

        self.pe_industry_rebalance_pops(industrial_zone_pop, mix_pop, leftover, leftover_sum, rng);
        self.pe_industry_write_mix_bonuses(industrial_zone_pop);
    }

    pub(super) fn pe_elapsed_years(&self) -> u32 {
        let current = u32::from(self.saved_date().map(|date| date.year).unwrap_or(1900));
        let founded = self
            .year_founded()
            .filter(|year| *year >= 1900)
            .unwrap_or(current.min(1900));
        current.saturating_sub(founded)
    }

    pub(super) fn pe_industry_demand_at(&self, index: usize) -> i32 {
        (self
            .misc_u32_at(pe_industry_field_offset(index, PE_INDUSTRY_DEMAND_FIELD))
            .unwrap_or(0) as u16) as i16 as i32
    }

    pub(super) fn pe_industry_tax_at(&self, index: usize) -> i32 {
        (self
            .misc_u32_at(pe_industry_field_offset(index, PE_INDUSTRY_TAX_FIELD))
            .unwrap_or(0) as u16) as i16 as i32
    }

    pub(super) fn pe_industry_pop_at(&self, index: usize) -> u32 {
        self.misc_u32_at(pe_industry_field_offset(index, PE_INDUSTRY_POP_FIELD))
            .unwrap_or(0)
    }

    pub(super) fn pe_industry_rebalance_pops(
        &mut self,
        industrial_zone_pop: u32,
        mix_pop: u32,
        leftover: [i32; PE_INDUSTRY_COUNT],
        leftover_sum: i32,
        rng: &mut Win95CrtRand,
    ) {
        if mix_pop > industrial_zone_pop {
            let scale = mix_pop.wrapping_sub(industrial_zone_pop).wrapping_mul(100);
            for index in 0..PE_INDUSTRY_COUNT {
                let pop = self.pe_industry_pop_at(index);
                let quota = pe_industry_wrapped_mul_div(scale, pop, mix_pop);
                pe_industry_apply_pop_delta(self, index, quota, -1, rng);
            }
            return;
        }
        if leftover_sum == 0 {
            return;
        }
        let scale = industrial_zone_pop.wrapping_sub(mix_pop).wrapping_mul(100);
        for index in 0..PE_INDUSTRY_COUNT {
            let share = leftover[index];
            if share == 0 {
                continue;
            }
            let quota = pe_industry_wrapped_mul_div(scale, share as u32, leftover_sum as u32);
            pe_industry_apply_pop_delta(self, index, quota, 1, rng);
        }
    }

    pub(super) fn pe_industry_write_mix_bonuses(&mut self, industrial_zone_pop: u32) {
        // PE `0x4694bc..0x4694c8` uses INC and 32-bit shifts/LEAs before
        // unsigned DIV. Keep the native dword wrap at each arithmetic step.
        let denom = industrial_zone_pop.wrapping_add(1);
        let dirty = self
            .pe_industry_pop_at(0)
            .wrapping_add(self.pe_industry_pop_at(1))
            .wrapping_add(self.pe_industry_pop_at(2))
            .wrapping_add(self.pe_industry_pop_at(5));
        let dirty_share = (dirty.wrapping_mul(100) / denom) as i32;
        let pollution = if dirty_share < 20 {
            -1_i32
        } else {
            (dirty_share - 20) / 30
        };
        let mut max_share = 0_u32;
        for index in 0..PE_INDUSTRY_COUNT {
            max_share = max_share.max(self.pe_industry_pop_at(index).wrapping_mul(100) / denom);
        }
        let max_share = max_share as i32;
        let mix = if max_share < 20 {
            0
        } else {
            ((max_share - 20) / 5) as u32
        };
        // PE `0x46951e` writes BSS `wIndustrialMixBonus` @ `0x4ca1e8`.
        self.pe_w_industrial_mix_bonus = mix.min(u32::from(u16::MAX)) as u16;
        // PE `0x4694d2` writes BSS `wIndustrialMixPollutionBonus` @ `0x4c9428`.
        self.pe_w_industrial_mix_pollution_bonus = pollution as i16;
        if self.kind.is_win95() {
            self.set_misc_u32_at(MISC_INDUSTRIAL_MIX_BONUS_OFFSET, mix);
            self.set_misc_u32_at(MISC_INDUSTRIAL_MIX_POLLUTION_BONUS_OFFSET, pollution as u32);
        } else {
            // Mac owns these offsets as different persistent modifiers.
            // Keep the selected PE engine's state in runtime/CGEX instead.
            self.pe_cross_layout_saved_industrial_mix_bonus = Some(self.pe_w_industrial_mix_bonus);
            self.pe_cross_layout_saved_industrial_mix_pollution_bonus =
                Some(self.pe_w_industrial_mix_pollution_bonus);
        }
    }

    pub(super) fn pe_national_trend_word(&self) -> i32 {
        (self.pe_w_national_trend as i16) as i32
    }

    pub(super) fn pe_national_tax_word(&self) -> i32 {
        (self.pe_w_national_tax as i16) as i32
    }
}

impl City {
    /// Win95 `wIndustrialMixBonus` @ `0x4ca1e8`.
    pub fn set_pe_w_industrial_mix_bonus(&mut self, bonus: u16) {
        self.pe_w_industrial_mix_bonus = bonus;
        if self.kind == CityFileKind::MacCity && self.pe_cross_layout_active {
            self.pe_cross_layout_saved_industrial_mix_bonus = Some(bonus);
        }
    }

    pub fn pe_w_industrial_mix_bonus(&self) -> u16 {
        self.pe_w_industrial_mix_bonus
    }

    /// Win95 `wIndustrialMixPollutionBonus` @ `0x4c9428` (signed).
    pub fn set_pe_w_industrial_mix_pollution_bonus(&mut self, bonus: i16) {
        self.pe_w_industrial_mix_pollution_bonus = bonus;
        if self.kind == CityFileKind::MacCity && self.pe_cross_layout_active {
            self.pe_cross_layout_saved_industrial_mix_pollution_bonus = Some(bonus);
        }
    }

    pub fn pe_w_industrial_mix_pollution_bonus(&self) -> i16 {
        self.pe_w_industrial_mix_pollution_bonus
    }
}

impl City {
    /// MISC offset 0x78 / BSS `0x4e9a24`. The low five bits grant Mayor's House,
    /// City Hall, Statue, Braun Llama Dome, and Arcologies, respectively.
    pub fn rewards_available_mask(&self) -> u32 {
        self.pe_dw_rewards_available & 0x1f
    }

    pub fn city_funds(&self) -> i32 {
        self.pe_i32_city_funds
    }
}

impl City {
    pub fn set_city_funds(&mut self, funds: i32) {
        self.set_misc_u32_at(MISC_CITY_FUNDS_OFFSET, funds as u32);
    }

    pub fn debit_city_funds(&mut self, cost: u32) {
        let cost = i32::try_from(cost).unwrap_or(i32::MAX);
        self.set_city_funds(self.city_funds().saturating_sub(cost));
    }

    pub fn set_rewards_available_mask(&mut self, mask: u32) {
        const REWARD_BITS: u32 = 0x1f;

        let current = self.pe_dw_rewards_available;
        let patched = (current & !REWARD_BITS) | (mask & REWARD_BITS);
        self.pe_dw_rewards_available = patched;
        if self.kind == CityFileKind::MacCity && self.pe_cross_layout_active {
            // Mac CODE 4 owns this slot as its command-strip latch. Keep the
            // selected Win95 engine's rewards mask in CGEX instead.
            self.pe_cross_layout_saved_rewards_available = Some(patched);
        } else {
            self.set_misc_u32_at(MISC_REWARDS_AVAILABLE_OFFSET, patched);
        }
    }
}

/// Win95 `0x4C4B40`: national population mean-reversion cap.
/// Win95 `value / (pop+1) * 100` in x87 (`0x468e22..0x468e50`).
pub fn pe_national_prosperity(value: u32, pop: u32) -> i32 {
    pe_ftol_x87_divide_then_mul_100(value, pop.saturating_add(1))
}

/// Mac CODE 8 `export_238`: multiply the unsigned value by 100 before
/// dividing by the wrapped population-plus-one, then truncate to a LONG.
/// The discretionary event is not scheduled until its QuickDraw chronology
/// is recovered. Unsupported zero-divisor and signed-LONG overflow states are
/// left to the caller instead of guessing SANE exception handling.
#[allow(dead_code)]
pub(crate) fn mac_national_prosperity(value: u32, population: u32) -> Option<i32> {
    let denominator = population.wrapping_add(1);
    if denominator == 0 {
        return None;
    }
    // The integer product is exact in SANE extended precision. For results
    // fitting a signed LONG, the nearest nonintegral ratio is farther from an
    // integer than half an extended-precision ULP; truncation is exact here.
    i32::try_from(u64::from(value) * 100 / u64::from(denominator)).ok()
}

/// Result of CODE 8's discretionary national event. Story selectors are kept
/// in native call order for the eventual scheduler/queue integration.
#[allow(dead_code)]
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct MacNationalEvent {
    pub tax: u16,
    pub trend: u16,
    pub stories: Vec<(u16, u16)>,
}

/// Execute the bounded event with an explicitly supplied QuickDraw stream.
/// This is deliberately unscheduled until the incoming stream position is
/// known. `None` leaves native numeric/divide exceptions unresolved.
#[allow(dead_code)]
pub(crate) fn mac_national_event(
    value: u32,
    population: u32,
    tax: u16,
    trend: u16,
    mut random_word: impl FnMut() -> u16,
) -> Option<MacNationalEvent> {
    let mut result = MacNationalEvent {
        tax,
        trend,
        stories: Vec::new(),
    };
    if random_word() % 10 != 0 {
        return Some(result);
    }
    let score = mac_national_prosperity(value, population)?;
    if random_word() % 5 < 2 {
        let first_draw = random_word();
        let divisor = result.tax.wrapping_mul(25);
        if divisor == 0 {
            return None;
        }
        if i32::from(first_draw % divisor) < score {
            result.tax = result.tax.wrapping_add(1);
            result.stories.push((9, result.tax));
        }
        let second_draw = random_word();
        let divisor = result.tax.wrapping_mul(25);
        if divisor == 0 {
            return None;
        }
        if i32::from(second_draw % divisor) > score {
            result.tax = result.tax.wrapping_sub(1);
            if result.tax == 0 {
                result.tax = 1;
            } else {
                result.stories.push((10, result.tax));
            }
        }
    }
    if random_word() % 3 == 0 {
        let next = if score < 45 {
            0
        } else if score < 60 {
            1
        } else if score < 75 {
            2
        } else {
            3
        };
        if result.trend != next {
            result.trend = next;
            result.stories.push((7, next));
        }
    }
    Some(result)
}

/// Reproduces PE's positive `FILD / FILD; FDIVP; FMUL qword(100); _ftol`
/// sequence at the normal x87 64-bit significand precision. `_ftol` changes
/// only the rounding-control bits while converting, so the intermediate x87
/// precision is observable at exact integer boundaries.
pub(super) fn pe_ftol_x87_divide_then_mul_100(numerator: u32, denominator: u32) -> i32 {
    pe_ftol_x87_divide_then_mul(numerator, denominator, 100)
}

/// Reproduces positive x87 `FILD numerator; FDIV denominator; FMUL multiplier;
/// _ftol` at 64-bit significand precision, returning the PE's low EAX dword.
pub(super) fn pe_ftol_x87_divide_then_mul(
    numerator: u32,
    denominator: u32,
    multiplier: u32,
) -> i32 {
    if numerator == 0 || denominator == 0 || multiplier == 0 {
        return 0;
    }

    // Normalize the exact positive rational to a 64-bit x87 significand,
    // rounding to nearest-even as FDIVP does under the default control word.
    let mut exponent =
        (31 - numerator.leading_zeros() as i32) - (31 - denominator.leading_zeros() as i32);
    if exponent >= 0 {
        if u128::from(numerator) < (u128::from(denominator) << exponent as u32) {
            exponent -= 1;
        }
    } else if (u128::from(numerator) << (-exponent) as u32) < u128::from(denominator) {
        exponent -= 1;
    }

    let numerator_shift = (63 - exponent) as u32;
    let scaled_numerator = u128::from(numerator) << numerator_shift;
    let denominator = u128::from(denominator);
    let quotient = scaled_numerator / denominator;
    let remainder = scaled_numerator % denominator;
    let mut significand = pe_round_unsigned_ratio_nearest_even(quotient, remainder, denominator);
    if significand == (1_u128 << 64) {
        significand >>= 1;
        exponent += 1;
    }

    // FMUL by the exactly representable integer multiplier, then round the
    // product back to the same 64-bit x87 significand before `_ftol` chops.
    let product = significand * u128::from(multiplier);
    let product_bits = 128 - product.leading_zeros() as i32;
    let discarded_bits = (product_bits - 64) as u32;
    let product_quotient = product >> discarded_bits;
    let product_remainder = product & ((1_u128 << discarded_bits) - 1);
    let mut product_significand = pe_round_unsigned_ratio_nearest_even(
        product_quotient,
        product_remainder,
        1_u128 << discarded_bits,
    );
    let mut product_exponent = exponent + discarded_bits as i32;
    if product_significand == (1_u128 << 64) {
        product_significand >>= 1;
        product_exponent += 1;
    }

    let integer = if product_exponent >= 63 {
        product_significand << (product_exponent - 63) as u32
    } else {
        let shift = (63 - product_exponent) as u32;
        if shift >= 64 {
            0
        } else {
            product_significand >> shift
        }
    };
    integer as u32 as i32
}

fn pe_round_unsigned_ratio_nearest_even(
    quotient: u128,
    remainder: u128,
    denominator: u128,
) -> u128 {
    let complement = denominator - remainder;
    if remainder > complement || (remainder == complement && quotient & 1 != 0) {
        quotient + 1
    } else {
        quotient
    }
}

/// Win95 trend class from prosperity at `0x468ef9`.
pub fn pe_national_trend_from_prosperity(prosperity: i32) -> i32 {
    if prosperity < 45 {
        0
    } else if prosperity < 60 {
        1
    } else if prosperity < 75 {
        2
    } else {
        3
    }
}
