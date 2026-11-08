//! City ordinances: flags, choice catalog, and demand/budget side effects.

use super::*;

pub(super) const ORDINANCE_FLAGS_MISC_OFFSET: usize = 0x0fa0;
/// Mac CODE 4 ordinance-mask slot 1000 (`ffffeff2`), shared with the PE offset.
pub(super) const MAC_ORDINANCE_FLAGS_MISC_OFFSET: usize = 0x0fa0;
pub(super) const ORDINANCE_COUNT: usize = 20;
pub(super) const ORDINANCE_ALL_MASK: u32 = (1 << ORDINANCE_COUNT) - 1;
pub(super) const ORDINANCE_SALES_TAX: usize = 0;
pub(super) const ORDINANCE_INCOME_TAX: usize = 1;
pub(super) const ORDINANCE_LEGALIZED_GAMBLING: usize = 2;
pub(super) const ORDINANCE_PARKING_FINES: usize = 3;
pub(super) const ORDINANCE_VOLUNTEER_FIRE_DEPT: usize = 4;
pub(super) const ORDINANCE_PUBLIC_SMOKING_BAN: usize = 5;
pub(super) const ORDINANCE_FREE_CLINICS: usize = 6;
pub(super) const ORDINANCE_JUNIOR_SPORTS: usize = 7;
pub(super) const ORDINANCE_PRO_READING_CAMPAIGN: usize = 8;
pub(super) const ORDINANCE_ANTI_DRUG_CAMPAIGN: usize = 9;
pub(super) const ORDINANCE_CPR_TRAINING: usize = 10;
pub(super) const ORDINANCE_NEIGHBORHOOD_WATCH: usize = 11;
pub(super) const ORDINANCE_TOURIST_ADVERTISING: usize = 12;
pub(super) const ORDINANCE_BUSINESS_ADVERTISING: usize = 13;
pub(super) const ORDINANCE_CITY_BEAUTIFICATION: usize = 14;
pub(super) const ORDINANCE_ANNUAL_CARNIVAL: usize = 15;
pub(super) const ORDINANCE_ENERGY_CONSERVATION: usize = 16;
pub(super) const ORDINANCE_NUCLEAR_FREE_ZONE: usize = 17;
pub(super) const ORDINANCE_HOMELESS_SHELTER: usize = 18;
pub(super) const ORDINANCE_POLLUTION_CONTROLS: usize = 19;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct OrdinanceChoice {
    pub(super) label: &'static str,
    pub(super) bit: usize,
}

pub(super) const ORDINANCE_CHOICES: [OrdinanceChoice; ORDINANCE_COUNT] = [
    OrdinanceChoice {
        label: "Sales Tax",
        bit: 0,
    },
    OrdinanceChoice {
        label: "Income Tax",
        bit: 1,
    },
    OrdinanceChoice {
        label: "Legalized Gambling",
        bit: 2,
    },
    OrdinanceChoice {
        label: "Parking Fines",
        bit: 3,
    },
    OrdinanceChoice {
        label: "Volunteer Fire Dept",
        bit: 4,
    },
    OrdinanceChoice {
        label: "Public Smoking Ban",
        bit: 5,
    },
    OrdinanceChoice {
        label: "Free Clinics",
        bit: 6,
    },
    OrdinanceChoice {
        label: "Junior Sports",
        bit: 7,
    },
    OrdinanceChoice {
        label: "Pro Reading Campaign",
        bit: 8,
    },
    OrdinanceChoice {
        label: "Anti Drug Campaign",
        bit: 9,
    },
    OrdinanceChoice {
        label: "CPR Training",
        bit: 10,
    },
    OrdinanceChoice {
        label: "Neighborhood Watch",
        bit: 11,
    },
    OrdinanceChoice {
        label: "Tourist Advertising",
        bit: 12,
    },
    OrdinanceChoice {
        label: "Business Advertising",
        bit: 13,
    },
    OrdinanceChoice {
        label: "City Beautification",
        bit: 14,
    },
    OrdinanceChoice {
        label: "Annual Carnival",
        bit: 15,
    },
    OrdinanceChoice {
        label: "Energy Conservation",
        bit: 16,
    },
    OrdinanceChoice {
        label: "Nuclear Free Zone",
        bit: 17,
    },
    OrdinanceChoice {
        label: "Homeless Shelter",
        bit: 18,
    },
    OrdinanceChoice {
        label: "Pollution Controls",
        bit: 19,
    },
];
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct OrdinanceEffects {
    pub(super) police_bonus: i32,
    pub(super) fire_bonus: i32,
    pub(super) health_bonus: i32,
    pub(super) education_bonus: i32,
    pub(super) recreation_bonus: i32,
    pub(super) land_value_bonus: i32,
    pub(super) residential_demand_bonus: i32,
    pub(super) commercial_demand_bonus: i32,
    pub(super) industrial_demand_bonus: i32,
    pub(super) annual_revenue: i64,
    pub(super) annual_expense: i64,
}

/// Win95 `FUN_0042BCA0` / `SimulationCalculateOrdinanceCost` raw signed amounts before ÷75.
///
/// Bases match live budget tax bases (object `0x4CC4CC`: R@+0x60, C@+0xD0, I@+0x140).
/// Energy uses `-(dwArcologyPopulation + dwCityPopulation)`. Integer division truncates toward 0.
pub(super) fn pe_ordinance_raw_cash_amount(
    index: usize,
    residential_tax_base: i32,
    commercial_tax_base: i32,
    industrial_tax_base: i32,
    arcology_population: i32,
    city_population: i32,
) -> i32 {
    let r = residential_tax_base;
    let c = commercial_tax_base;
    let i = industrial_tax_base;
    match index {
        ORDINANCE_SALES_TAX => c,
        ORDINANCE_INCOME_TAX => r,
        ORDINANCE_LEGALIZED_GAMBLING => c.wrapping_mul(2),
        ORDINANCE_PARKING_FINES => r / 2,
        ORDINANCE_VOLUNTEER_FIRE_DEPT => (r / 3).wrapping_neg(),
        ORDINANCE_PUBLIC_SMOKING_BAN => (c / 6).wrapping_neg(),
        ORDINANCE_FREE_CLINICS => (r / 2).wrapping_neg(),
        ORDINANCE_JUNIOR_SPORTS => (r / 4).wrapping_neg(),
        ORDINANCE_PRO_READING_CAMPAIGN => (r / 6).wrapping_neg(),
        ORDINANCE_ANTI_DRUG_CAMPAIGN => (r / 5).wrapping_neg(),
        ORDINANCE_CPR_TRAINING => (r / 6).wrapping_neg(),
        ORDINANCE_NEIGHBORHOOD_WATCH => (r / 3).wrapping_neg(),
        ORDINANCE_TOURIST_ADVERTISING => c.wrapping_neg(),
        ORDINANCE_BUSINESS_ADVERTISING => i.wrapping_neg(),
        ORDINANCE_CITY_BEAUTIFICATION => (r / 4).wrapping_neg(),
        ORDINANCE_ANNUAL_CARNIVAL => (c / 3).wrapping_neg(),
        ORDINANCE_ENERGY_CONSERVATION => arcology_population
            .wrapping_add(city_population)
            .wrapping_neg(),
        ORDINANCE_NUCLEAR_FREE_ZONE => 0,
        ORDINANCE_HOMELESS_SHELTER => (r / 2).wrapping_neg(),
        ORDINANCE_POLLUTION_CONTROLS => i.wrapping_neg(),
        _ => 0,
    }
}

/// Mac CODE 11 helper reached by CODE 4 `export_227`. Unlike PE's Energy
/// Conservation input, Mac subtracts the two saved Mac globals directly.
pub(super) fn mac_ordinance_raw_cash_total(
    city: &City,
    residential_tax_base: i32,
    commercial_tax_base: i32,
    industrial_tax_base: i32,
) -> i32 {
    let flags = city.ordinance_flags_for_saved_layout();
    let energy_zone_total = city.mac_state_misc_u32_at(0x102c).unwrap_or(0) as i32;
    let arcology_total = city.mac_arcology_resident_total() as i32;
    let r = residential_tax_base;
    let c = commercial_tax_base;
    let i = industrial_tax_base;
    (0..ORDINANCE_COUNT).fold(0_i32, |total, index| {
        if !ordinance_flag_enabled(flags, index) {
            return total;
        }
        let raw = match index {
            ORDINANCE_SALES_TAX => c,
            ORDINANCE_INCOME_TAX => r,
            ORDINANCE_LEGALIZED_GAMBLING => c.wrapping_mul(2),
            ORDINANCE_PARKING_FINES => r / 2,
            ORDINANCE_VOLUNTEER_FIRE_DEPT => (r / 3).wrapping_neg(),
            ORDINANCE_PUBLIC_SMOKING_BAN => (c / 6).wrapping_neg(),
            ORDINANCE_FREE_CLINICS => (r / 2).wrapping_neg(),
            ORDINANCE_JUNIOR_SPORTS => (r / 4).wrapping_neg(),
            ORDINANCE_PRO_READING_CAMPAIGN => (r / 6).wrapping_neg(),
            ORDINANCE_ANTI_DRUG_CAMPAIGN => (r / 5).wrapping_neg(),
            ORDINANCE_CPR_TRAINING => (r / 6).wrapping_neg(),
            ORDINANCE_NEIGHBORHOOD_WATCH => (r / 3).wrapping_neg(),
            ORDINANCE_TOURIST_ADVERTISING => c.wrapping_neg(),
            ORDINANCE_BUSINESS_ADVERTISING => i.wrapping_neg(),
            ORDINANCE_CITY_BEAUTIFICATION => (r / 4).wrapping_neg(),
            ORDINANCE_ANNUAL_CARNIVAL => (c / 3).wrapping_neg(),
            ORDINANCE_ENERGY_CONSERVATION => energy_zone_total
                .wrapping_neg()
                .wrapping_sub(arcology_total),
            ORDINANCE_NUCLEAR_FREE_ZONE => 0,
            ORDINANCE_HOMELESS_SHELTER => (r / 2).wrapping_neg(),
            ORDINANCE_POLLUTION_CONTROLS => i.wrapping_neg(),
            _ => 0,
        };
        total.wrapping_add(raw)
    })
}

fn pe_budget_cash_dollars_from_raw(raw: i32) -> i64 {
    i64::from(raw) / i64::from(TAX_REVENUE_DIVISOR)
}

fn pe_ordinance_tax_bases_and_pops(
    city: &City,
    population: PopulationStats,
    fidelity: EngineFidelity,
) -> (i32, i32, i32, i32, i32) {
    let bases = budget_tax_bases_for_city_with_fidelity(city, population, fidelity);
    let r = i32::try_from(bases[0]).unwrap_or(i32::MAX);
    let c = i32::try_from(bases[1]).unwrap_or(i32::MAX);
    let i = i32::try_from(bases[2]).unwrap_or(i32::MAX);
    // PE ordinance energy @ `0x42bdea` reads BSS `0x4c94c4`.
    let uses_pe_population = fidelity.uses_win95();
    let arcology_population = if uses_pe_population {
        city.pe_dw_arcology_population()
    } else {
        population.arcology_population
    };
    let arco = i32::try_from(arcology_population).unwrap_or(i32::MAX);
    // PE ordinance table uses `dwCityPopulation` (`0x4CAA74`) for city_pop terms.
    let city_pop = if uses_pe_population {
        i32::try_from(city.pe_dw_city_population()).unwrap_or(i32::MAX)
    } else {
        i32::try_from(super::pe_zone_pops_scaled_population(population)).unwrap_or(i32::MAX)
    };
    (r, c, i, arco, city_pop)
}

/// Win95 `FUN_004734A0` rebuilds active-document `+0x1b0` from the raw
/// enabled-ordinance results after its monthly budget-row accrual loop. This
/// value becomes row 3's current input for the following monthly accrual.
pub(super) fn pe_ordinance_raw_cash_total_for_city(
    city: &City,
    population: PopulationStats,
) -> i32 {
    let (r, c, i, arco, city_pop) =
        pe_ordinance_tax_bases_and_pops(city, population, EngineFidelity::Win95);
    let flags = ordinance_flags_for_city(city);
    (0..ORDINANCE_COUNT).fold(0_i32, |total, index| {
        if !ordinance_flag_enabled(flags, index) {
            return total;
        }
        total.wrapping_add(pe_ordinance_raw_cash_amount(index, r, c, i, arco, city_pop))
    })
}

/// Year-end / display ordinance cash: sum of enabled raw amounts each ÷75 (trunc toward 0).
pub(super) fn pe_ordinance_annual_cash_for_city(city: &City, population: PopulationStats) -> i64 {
    let (r, c, i, arco, city_pop) =
        pe_ordinance_tax_bases_and_pops(city, population, EngineFidelity::Win95);
    let flags = ordinance_flags_for_city(city);
    (0..ORDINANCE_COUNT).fold(0_i64, |total, index| {
        if !ordinance_flag_enabled(flags, index) {
            return total;
        }
        let raw = pe_ordinance_raw_cash_amount(index, r, c, i, arco, city_pop);
        total.saturating_add(pe_budget_cash_dollars_from_raw(raw))
    })
}

pub(super) fn ordinance_effects_for_city(
    city: &City,
    population: PopulationStats,
) -> OrdinanceEffects {
    ordinance_effects_for_city_with_fidelity(city, population, EngineFidelity::Win95)
}

pub(super) fn ordinance_effects_for_city_with_fidelity(
    city: &City,
    population: PopulationStats,
    fidelity: EngineFidelity,
) -> OrdinanceEffects {
    let mut effects = OrdinanceEffects::default();
    // Select the saved bit source by engine. The shared civic-score
    // coefficients below remain CityGame estimates, not a Mac native formula
    // claim; only rooted Mac effects should be promoted to parity.
    let flags = ordinance_flags_for_city_with_fidelity(city, fidelity);
    let enabled = |index: usize| ordinance_flag_enabled(flags, index);

    if enabled(ORDINANCE_SALES_TAX) {
        effects.residential_demand_bonus -= 30;
        effects.commercial_demand_bonus -= 50;
    }
    if enabled(ORDINANCE_INCOME_TAX) {
        effects.residential_demand_bonus -= 70;
        effects.commercial_demand_bonus -= 20;
    }
    if enabled(ORDINANCE_LEGALIZED_GAMBLING) {
        effects.police_bonus -= 5;
        effects.health_bonus -= 2;
        effects.land_value_bonus -= 6;
        effects.commercial_demand_bonus += 90;
    }
    if enabled(ORDINANCE_PARKING_FINES) {
        effects.commercial_demand_bonus -= 30;
    }
    if enabled(ORDINANCE_VOLUNTEER_FIRE_DEPT) {
        effects.fire_bonus += 15;
    }
    if enabled(ORDINANCE_PUBLIC_SMOKING_BAN) {
        effects.health_bonus += 8;
        effects.land_value_bonus += 2;
        effects.commercial_demand_bonus -= 20;
    }
    if enabled(ORDINANCE_FREE_CLINICS) {
        effects.health_bonus += 15;
    }
    if enabled(ORDINANCE_JUNIOR_SPORTS) {
        effects.education_bonus += 3;
        effects.recreation_bonus += 10;
    }
    if enabled(ORDINANCE_PRO_READING_CAMPAIGN) {
        effects.education_bonus += 15;
    }
    if enabled(ORDINANCE_ANTI_DRUG_CAMPAIGN) {
        effects.police_bonus += 8;
        effects.health_bonus += 4;
        effects.education_bonus += 3;
    }
    if enabled(ORDINANCE_CPR_TRAINING) {
        effects.health_bonus += 8;
    }
    if enabled(ORDINANCE_NEIGHBORHOOD_WATCH) {
        effects.police_bonus += 12;
    }
    if enabled(ORDINANCE_TOURIST_ADVERTISING) {
        effects.commercial_demand_bonus += 120;
    }
    if enabled(ORDINANCE_BUSINESS_ADVERTISING) {
        effects.commercial_demand_bonus += 70;
        effects.industrial_demand_bonus += 50;
    }
    if enabled(ORDINANCE_CITY_BEAUTIFICATION) {
        effects.land_value_bonus += 12;
        effects.recreation_bonus += 8;
    }
    if enabled(ORDINANCE_ANNUAL_CARNIVAL) {
        effects.recreation_bonus += 8;
        effects.commercial_demand_bonus += 60;
    }
    if enabled(ORDINANCE_ENERGY_CONSERVATION) {
        effects.land_value_bonus += 4;
        effects.residential_demand_bonus += 40;
        effects.industrial_demand_bonus += 20;
    }
    if enabled(ORDINANCE_NUCLEAR_FREE_ZONE) {
        effects.land_value_bonus += 6;
        effects.industrial_demand_bonus -= 80;
    }
    if enabled(ORDINANCE_HOMELESS_SHELTER) {
        effects.health_bonus += 5;
        effects.land_value_bonus -= 3;
        effects.residential_demand_bonus += 30;
    }
    if enabled(ORDINANCE_POLLUTION_CONTROLS) {
        effects.health_bonus += 4;
        effects.land_value_bonus += 12;
        effects.industrial_demand_bonus -= 80;
    }

    let (r, c, i, arco, city_pop) = pe_ordinance_tax_bases_and_pops(city, population, fidelity);
    for index in 0..ORDINANCE_COUNT {
        if !enabled(index) {
            continue;
        }
        let dollars = pe_budget_cash_dollars_from_raw(pe_ordinance_raw_cash_amount(
            index, r, c, i, arco, city_pop,
        ));
        if dollars >= 0 {
            effects.annual_revenue = effects.annual_revenue.saturating_add(dollars);
        } else {
            effects.annual_expense = effects.annual_expense.saturating_add(-dollars);
        }
    }

    effects
}

pub(super) fn ordinance_year_end_delta_for_city(city: &City, population: PopulationStats) -> i64 {
    pe_ordinance_annual_cash_for_city(city, population)
}
pub(super) fn ordinance_index_is_valid(index: usize) -> bool {
    index < ORDINANCE_CHOICES.len()
}

pub(super) fn ordinance_flags_for_city(city: &City) -> u32 {
    ordinance_flags_for_city_in_saved_layout(city)
}

pub(super) fn ordinance_flags_for_city_with_fidelity(
    city: &City,
    _fidelity: EngineFidelity,
) -> u32 {
    // Ordinance choices have a known saved representation in each file
    // family. Engine fidelity controls the effects, not which bytes own the
    // current city's flags.
    ordinance_flags_for_city_in_saved_layout(city)
}

fn ordinance_flags_for_city_in_saved_layout(city: &City) -> u32 {
    city.ordinance_flags_for_saved_layout()
}

pub(super) fn ordinance_flag_enabled(flags: u32, index: usize) -> bool {
    if !ordinance_index_is_valid(index) {
        return false;
    }
    let bit = 1_u32 << ORDINANCE_CHOICES[index].bit;
    (flags & bit) != 0
}

pub(super) fn ordinance_enabled_for_city(city: &City, index: usize) -> bool {
    ordinance_flag_enabled(ordinance_flags_for_city(city), index)
}

pub(super) fn ordinance_enabled_for_city_with_fidelity(
    city: &City,
    index: usize,
    fidelity: EngineFidelity,
) -> bool {
    ordinance_flag_enabled(
        ordinance_flags_for_city_with_fidelity(city, fidelity),
        index,
    )
}

pub(super) fn set_city_ordinance_enabled(city: &mut City, index: usize, enabled: bool) -> bool {
    set_city_ordinance_enabled_with_fidelity(city, index, enabled, EngineFidelity::Win95)
}

pub(super) fn set_city_ordinance_enabled_with_fidelity(
    city: &mut City,
    index: usize,
    enabled: bool,
    fidelity: EngineFidelity,
) -> bool {
    let Some(next) = write_city_ordinance_flag(city, index, enabled) else {
        return false;
    };
    if fidelity.uses_win95() {
        // PE ordinance-budget recalc @ `0x419348` → tech-unlock rebuild after flag walk.
        let _ = city.pe_rebuild_tech_unlock_mask_46a0be(next);
    }
    true
}

/// Change the saved-layout-owned raw mask without invoking a manual UI handler.
/// None means invalid index or unchanged mask; upper/unknown bits are retained.
fn write_city_ordinance_flag(city: &mut City, index: usize, enabled: bool) -> Option<u32> {
    if !ordinance_index_is_valid(index) {
        return None;
    }
    let bit = 1_u32 << ORDINANCE_CHOICES[index].bit;
    let (offset, current) = if city.kind.is_win95() {
        (
            ORDINANCE_FLAGS_MISC_OFFSET,
            city.misc_u32_at(ORDINANCE_FLAGS_MISC_OFFSET).unwrap_or(0),
        )
    } else {
        (
            MAC_ORDINANCE_FLAGS_MISC_OFFSET,
            city.mac_state_misc_u32_at(MAC_ORDINANCE_FLAGS_MISC_OFFSET)
                .unwrap_or(0),
        )
    };
    let next = if enabled {
        current | bit
    } else {
        current & !bit
    };
    if next == current {
        return None;
    }
    if city.kind.is_win95() {
        ensure_budget_misc_defaults_before_extension(city);
        city.set_misc_u32_at(offset, next);
    } else {
        city.set_mac_state_misc_u32_at(offset, next);
    }
    Some(next)
}

/// PE `0x4738b6`: conditional process-CRT draws after monthly current rebuild.
fn pe_budget_ordinance_choice(
    no_disasters: bool,
    funds: i32,
    crt: &mut Win95CrtRand,
) -> Option<usize> {
    if no_disasters || crt.random_u15() & 7 != 0 {
        return None;
    }
    // The signed comparison follows the second draw even for negative funds.
    if funds <= 50_000 + i32::from(crt.random_u15()) {
        return None;
    }
    Some(usize::from(crt.random_u15() % 20))
}

pub(super) fn apply_pe_budget_ordinance_tail(
    city: &mut City,
    no_disasters: bool,
    crt: &mut Win95CrtRand,
) {
    let Some(index) = pe_budget_ordinance_choice(no_disasters, city.city_funds(), crt) else {
        return;
    };
    // Unlike the manual handler, native random adoption does not rebuild tech
    // or current costs. An already-enabled choice still inserts story 0x29.
    let _ = write_city_ordinance_flag(city, index, true);
    city.queue_pe_newspaper_story(0x29, index as u8);
}

/// Mac CODE 4 `export_227`'s automatic ordinance selection at the end of
/// phase 0. The selector consumes QuickDraw values, then sets the saved Mac
/// ordinance bit. The separate Mac CODE 7 story callback remains presentation
/// state and must not be routed through the PE newspaper queue.
pub(super) fn apply_mac_budget_ordinance_tail(
    city: &mut City,
    no_disasters: bool,
    quickdraw: &mut MacToolboxRng,
) -> Option<usize> {
    if no_disasters || quickdraw.random_u16() & 7 != 0 {
        return None;
    }

    let candidate_funds = 50_000_u32 + u32::from(quickdraw.random_u16());
    if (candidate_funds as i32) >= city.city_funds() {
        return None;
    }

    let index = usize::from(quickdraw.random_u16() % ORDINANCE_COUNT as u16);
    let _ = write_city_ordinance_flag(city, index, true);
    Some(index)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mac_budget_ordinance_tail_matches_native_gate_and_funds_boundaries() {
        let mut city =
            City::new_blank("/tmp/mac-ordinance-tail.city", "Ordinance").expect("blank Mac city");
        city.kind = CityFileKind::MacCity;

        let mut rng = MacToolboxRng::new(15);
        let mut expected = rng;
        assert_eq!(
            apply_mac_budget_ordinance_tail(&mut city, true, &mut rng),
            None,
            "No Disasters skips the monthly tail"
        );
        assert_eq!(
            rng.seed(),
            expected.seed(),
            "No Disasters consumes no QuickDraw values"
        );

        let equality_seed = (1..100_000)
            .find(|&seed| {
                let mut probe = MacToolboxRng::new(seed);
                probe.random_u16() & 7 == 0
            })
            .expect("find first gate pass");
        expected = MacToolboxRng::new(equality_seed);
        assert_eq!(expected.random_u16() & 7, 0);
        let funds_boundary = 50_000 + i32::from(expected.random_u16());
        let equality_rng = expected;
        city.set_city_funds(funds_boundary);
        let mut rng = MacToolboxRng::new(equality_seed);
        assert_eq!(
            apply_mac_budget_ordinance_tail(&mut city, false, &mut rng),
            None,
            "candidate funds equal to the treasury are rejected"
        );
        assert_eq!(
            rng.seed(),
            equality_rng.seed(),
            "equality consumes exactly two values"
        );

        let acceptance_seed = (1..100_000)
            .find(|&seed| {
                let mut probe = MacToolboxRng::new(seed);
                if probe.random_u16() & 7 != 0 {
                    return false;
                }
                let candidate = 50_000 + i32::from(probe.random_u16());
                candidate < i32::MAX
            })
            .expect("find a seed with a reachable funds threshold");
        expected = MacToolboxRng::new(acceptance_seed);
        assert_eq!(expected.random_u16() & 7, 0);
        let candidate = 50_000 + i32::from(expected.random_u16());
        let ordinal = usize::from(expected.random_u16() % ORDINANCE_COUNT as u16);
        city.set_city_funds(candidate + 1);
        city.set_mac_state_misc_u32_at(MAC_ORDINANCE_FLAGS_MISC_OFFSET, 1 << ordinal);
        let mut rng = MacToolboxRng::new(acceptance_seed);
        assert_eq!(
            apply_mac_budget_ordinance_tail(&mut city, false, &mut rng),
            Some(ordinal),
            "a treasury one unit above the candidate accepts"
        );
        assert_eq!(
            rng.seed(),
            expected.seed(),
            "accepted selection consumes exactly three values"
        );
        assert_eq!(
            city.mac_state_misc_u32_at(MAC_ORDINANCE_FLAGS_MISC_OFFSET),
            Some(1 << ordinal),
            "reselecting an enabled ordinance preserves its bit"
        );
    }

    #[test]
    fn pe_budget_tail_matches_all_original_x86_cases() {
        let mut cases = 0;
        let mut choices = 0_u32;
        let mut draw_counts = [0; 4];
        let mut lines = include_str!("fixtures/pe_budget_tail_native_20261023.csv").lines();
        assert_eq!(
            lines.next(),
            Some(
                "no_disasters,funds,flags_before,seed_before,flags_after,seed_after,draws,story_calls,story_id,story_param,instructions"
            )
        );
        for line in lines {
            let row: Vec<i64> = line
                .split(',')
                .map(|value| value.parse().unwrap())
                .collect();
            assert_eq!(row.len(), 11);
            let mut crt = Win95CrtRand::new(row[3] as u32);
            let choice = pe_budget_ordinance_choice(row[0] != 0, row[1] as i32, &mut crt);
            assert_eq!(crt.seed(), row[5] as u32, "final seed: {line}");
            assert_eq!(
                choice.map_or(-1, |index| index as i64),
                row[9],
                "choice: {line}"
            );
            assert_eq!(i64::from(choice.is_some()), row[7], "story call: {line}");
            assert_eq!(if choice.is_some() { 0x29 } else { -1 }, row[8]);
            let bit = choice.map_or(0, |index| 1_u32 << index);
            assert_eq!(row[2] as u32 | bit, row[4] as u32, "mask: {line}");
            // The native fixture records the draw count independently of Rust.
            let draws = row[6] as usize;
            let mut expected_crt = Win95CrtRand::new(row[3] as u32);
            for _ in 0..draws {
                expected_crt.random_u15();
            }
            assert_eq!(crt, expected_crt, "draw count: {line}");
            draw_counts[draws] += 1;
            choices |= bit;
            cases += 1;
        }
        assert_eq!(cases, 5380);
        assert_eq!(choices, ORDINANCE_ALL_MASK);
        assert_eq!(draw_counts, [2680, 2320, 240, 140]);
    }
}
