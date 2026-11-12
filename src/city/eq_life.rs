use super::economy::pe_ftol_x87_divide_then_mul;
use super::{
    City, MISC_NORMAL_POPULATION_OFFSET, pe_occupancy_div9_count, pe_occupancy_div16_count,
};
use crate::win95_random::{Win95CrtRand, Win95Lfsr};

impl City {
    pub fn set_pe_life_expectancy(&mut self, years: u32) {
        self.pe_life_expectancy = years.clamp(20, 95) as u8;
    }

    pub fn pe_life_expectancy(&self) -> u8 {
        self.pe_life_expectancy
    }

    /// Day-21 wrapper: national, neighbors, industry mix, then EQ_LE tables.
    pub fn advance_pe_eq_le_calendar_month(
        &mut self,
        school_funding: u32,
        health_funding: u32,
        college_funding: u32,
        industrial_zone_pop: u32,
        rng: &mut Win95CrtRand,
        lfsr: &mut Win95Lfsr,
    ) {
        // National population, value, tax and trend are shared by both save
        // layouts, so this first native worker is independent of file kind.
        self.advance_pe_national_economy_month(rng);
        // CODE 4 loader replay and CODE 8's paired population/value worker
        // establish the same neighbor record layout on Mac files.
        self.advance_pe_neighbor_economy_month(rng);
        // Both editions persist the industry rows, ordinance mask, and the
        // cohort-derived education measure at the same saved positions.
        self.advance_pe_industry_mix_month(industrial_zone_pop, rng, lfsr);
        let school_count = pe_occupancy_div9_count(self, 214).max(0) as u32;
        let hospital_count = pe_occupancy_div9_count(self, 209).max(0) as u32;
        let college_count = pe_occupancy_div16_count(self, 217).max(0) as u32;
        // PE `0x469537` tests `0x4caa74`, the population published by the
        // phase-21 RCI fold. Zero takes the cohort-clear branch.
        // School/hospital `/9` and college `sar 4` come from `dwTileCount`
        // (`0x46957b` / `0x4695f0` / `0x4695b2`), not XMIC rows.
        self.advance_pe_eq_le_month(
            self.pe_dw_city_population,
            school_count,
            hospital_count,
            college_count,
            school_funding,
            health_funding,
            college_funding,
            rng,
        );
    }

    /// Win95 `SimulationEQ_LE_Processing` (`0x468d30`, thunk `0x401262`, SimCalendar day 21).
    ///
    /// The PE function starts with national pop/value/tax/trend (`0x4ca928` /
    /// `0x4cb014` / `0x4ca938` / `0x4ca1bc`), then neighbors and industry mix,
    /// then the age tables. `0x4caa74` is `dwCityPopulation`: if it is 0, PE
    /// zeros the three 20-dword tables and returns (`0x469537`). The calendar
    /// feeds the RCI fold's published population. With nonzero population,
    /// the worker runs life-table deaths, age-forward transfers, births,
    /// population balancing, and fertile averages (`0x469be8`).
    pub fn advance_pe_eq_le_month(
        &mut self,
        pop_load: u32,
        school_count: u32,
        hospital_count: u32,
        college_count: u32,
        school_funding: u32,
        health_funding: u32,
        college_funding: u32,
        rng: &mut Win95CrtRand,
    ) {
        if pop_load == 0 {
            self.wipe_pe_eq_le_tables();
            return;
        }
        let params = self.pe_eq_le_month_params(
            school_count,
            hospital_count,
            college_count,
            school_funding,
            health_funding,
            college_funding,
            pop_load,
        );
        self.pe_eq_deaths = 0;
        self.pe_eq_births = 0;
        self.pe_eq_le_run_one_month(&params, rng);
        self.persist_pe_eq_misc_from_averages();
    }

    /// Test helper: wipe if city pop is 0, otherwise run 12 PE monthly steps.
    pub fn advance_pe_age_cohorts_yearly(
        &mut self,
        pop_load: u32,
        school_count: u32,
        hospital_count: u32,
        college_count: u32,
        school_funding: u32,
        health_funding: u32,
        college_funding: u32,
        rng: &mut Win95CrtRand,
    ) {
        if pop_load == 0 {
            self.wipe_pe_eq_le_tables();
            return;
        }
        let params = self.pe_eq_le_month_params(
            school_count,
            hospital_count,
            college_count,
            school_funding,
            health_funding,
            college_funding,
            pop_load,
        );
        self.pe_eq_deaths = 0;
        self.pe_eq_births = 0;
        for _ in 0..12 {
            self.pe_eq_le_run_one_month(&params, rng);
        }
        self.persist_pe_eq_misc_from_averages();
    }

    pub(super) fn pe_eq_le_tables_empty(&self) -> bool {
        self.pe_age_cohorts[1] == 0 && self.pe_age_cohorts[2] == 0 && self.pe_age_cohorts[3] == 0
    }

    /// PE `0x469537` when `dwCityPopulation` is 0: zero 20 dwords of cohorts, life, aux.
    pub(super) fn wipe_pe_eq_le_tables(&mut self) {
        self.pe_age_cohorts = [0; 20];
        self.pe_eq_life_points = [0; 20];
        self.pe_eq_aux_points = [0; 20];
        self.persist_pe_eq_le_tables_to_misc();
    }

    fn pe_eq_le_month_params(
        &self,
        school_count: u32,
        hospital_count: u32,
        college_count: u32,
        school_funding: u32,
        health_funding: u32,
        college_funding: u32,
        city_population: u32,
    ) -> PeEqLeMonthParams {
        // PE `0x46976f` reads the published 64×64 XPLT aggregate directly.
        // The expanded display plane has four cells per native cell and can
        // also differ in time from the last published simulation total.
        let pollution = self.pe_dw_city_pollution;
        PeEqLeMonthParams {
            life_expectancy: pe_eq_ordinance_life_expectancy(
                self.ordinance_flags_for_saved_layout(),
            ),
            hospital_birth_param: pe_eq_hospital_birth_param(hospital_count, health_funding)
                .wrapping_add(pe_eq_free_clinics_hospital_bump(
                    self.ordinance_flags_for_saved_layout(),
                    self.pe_attitude_tax_bases[0],
                )),
            school_aux_param: pe_eq_school_aux_param(school_count, school_funding),
            college_aux_param: pe_eq_college_aux_param(college_count, college_funding),
            education_demand: pollution,
            city_population,
            growth_slot7: self.pe_eq_growth_slot7,
            pro_reading: pe_eq_ordinance_pro_reading(self.ordinance_flags_for_saved_layout()),
        }
    }

    /// PE EQ/LE month pollution input is the published BSS `0x4cc910`.
    #[cfg(test)]
    pub fn pe_eq_month_pollution_param_for_test(&self) -> u32 {
        self.pe_dw_city_pollution
    }

    pub(super) fn pe_eq_le_run_one_month(
        &mut self,
        params: &PeEqLeMonthParams,
        rng: &mut Win95CrtRand,
    ) {
        let deaths_before = self.pe_eq_deaths;
        self.pe_age_cohorts_deaths_one_month(rng);
        self.pe_age_cohorts_age_forward_one_month(params, rng);
        let month_births = self.pe_age_cohorts_births_one_month(params, rng);
        let month_deaths = self.pe_eq_deaths.wrapping_sub(deaths_before);
        // Day-22 leftover sits in `0x4ca4cc` / `0x4cb1a8` until this compare
        // (`0x469945`). Mortality and births add on top, then leftover is spent.
        self.pe_age_cohorts_rebalance_one_month(
            month_deaths.wrapping_add(self.pe_eq_leftover_deaths),
            month_births.wrapping_add(self.pe_eq_leftover_births),
            params.city_population,
            rng,
        );
        self.pe_eq_leftover_deaths = 0;
        self.pe_eq_leftover_births = 0;
        self.pe_age_cohorts_write_fertile_averages(params.city_population);
    }

    /// Invented pyramid used by tests as a stand-in for a loaded MISC `0x7c` city.
    /// PE never writes this on January occupancy.
    #[cfg(test)]
    pub(super) fn seed_pe_age_cohorts_from_pop(&mut self, pop_load: u32) {
        self.wipe_pe_eq_le_tables();
        // This is a synthetic test fixture, not the PE zero-population path.
        self.pe_eq_deaths = 0;
        self.pe_eq_births = 0;
        self.pe_eq_leftover_deaths = 0;
        self.pe_eq_leftover_births = 0;
        self.pe_eq_life_average = 0;
        self.pe_eq_education_average = 0;
        self.pe_eq_workforce_percent = 0;
        self.pe_eq_averages_ready = false;
        let school = pop_load.saturating_mul(12) / 100;
        let college = pop_load.saturating_mul(5) / 100;
        self.pe_age_cohorts[1] = school / 2;
        self.pe_age_cohorts[2] = school.saturating_sub(self.pe_age_cohorts[1]);
        self.pe_age_cohorts[3] = college;
        let remaining = pop_load.saturating_sub(school.saturating_add(college));
        self.pe_age_cohorts[0] = remaining / 10;
        let after_infant = remaining.saturating_sub(self.pe_age_cohorts[0]);
        let fertile_each = after_infant / 5;
        for index in 4..=8 {
            self.pe_age_cohorts[index] = fertile_each;
        }
        let assigned: u32 = self.pe_age_cohorts.iter().sum();
        if assigned < pop_load {
            self.pe_age_cohorts[5] = self.pe_age_cohorts[5].saturating_add(pop_load - assigned);
        }
        // Same `esp+0x14` ordinance LE used by PE births (`0x469623`). Coverage LE
        // is a hospital-overlay snapshot and does not initialize `0x4caa70`.
        let life_per =
            pe_eq_ordinance_life_expectancy(self.ordinance_flags_for_saved_layout()).max(1);
        for index in 0..20 {
            let pop = self.pe_age_cohorts[index];
            self.pe_eq_life_points[index] = pop.saturating_mul(life_per);
            self.pe_eq_aux_points[index] = pop;
        }
        self.persist_pe_eq_le_tables_to_misc();
    }

    /// Win95 EQ_LE mortality loop at `0x469671` using parallel life points `0x4caa70`.
    pub(super) fn pe_age_cohorts_deaths_one_month(&mut self, rng: &mut Win95CrtRand) {
        let mut age_divisor = 5_u32;
        for index in 1..20 {
            let pop = self.pe_age_cohorts[index];
            if pop == 0 {
                age_divisor = age_divisor.saturating_add(5);
                continue;
            }
            let life = self.pe_eq_life_points[index];
            // score = ((life / pop) * 100) / age_divisor; skip deaths when >= 100.
            let score = pe_eq_survival_score(life, pop, age_divisor);
            if score >= 100 {
                age_divisor = age_divisor.saturating_add(5);
                continue;
            }
            // PE uses a wrapped dword IMUL, then unsigned DIV 24. The score
            // can be negative after its signed IDIV, so SUB 100-score is also
            // the native wrapping dword operation before IMUL.
            let mortality_weight = 100_i32.wrapping_sub(score) as u32;
            let raw = mortality_weight.wrapping_mul(pop) / 24;
            let mut deaths = raw / 100;
            let rem = raw % 100;
            let rounding_roll = u32::from(rng.random_u16()) % 100;
            if rounding_roll < rem {
                deaths = deaths.saturating_add(1);
            }
            let deaths = deaths.min(pop);
            if deaths == 0 {
                age_divisor = age_divisor.saturating_add(5);
                continue;
            }
            let aux = self.pe_eq_aux_points[index];
            let aux_loss = (aux.wrapping_mul(deaths) / pop).min(aux);
            let life_loss = (life.wrapping_mul(deaths) / pop).min(life);
            self.pe_eq_aux_points[index] = aux - aux_loss;
            self.pe_eq_life_points[index] = life - life_loss;
            self.pe_age_cohorts[index] = pop - deaths;
            self.pe_eq_deaths = self.pe_eq_deaths.wrapping_add(deaths);
            age_divisor = age_divisor.saturating_add(5);
        }
    }

    pub(super) fn pe_age_cohorts_age_forward_one_month(
        &mut self,
        params: &PeEqLeMonthParams,
        rng: &mut Win95CrtRand,
    ) {
        let life_penalty = params.life_penalty();
        // PE `0x469790`: from oldest bucket down, move ~prev/60 into the next age.
        // It calls CRT rand once for every nonempty source bucket, even when
        // `prev` is an exact multiple of 60 and the remainder comparison cannot
        // round the quotient up.
        for index in (1..20).rev() {
            let prev = self.pe_age_cohorts[index - 1];
            if prev == 0 {
                continue;
            }
            let mut moving = prev / 60;
            let rem = prev % 60;
            let rounding_roll = u32::from(rng.random_u16()) % 60;
            if rounding_roll < rem {
                moving = moving.wrapping_add(1);
            }
            let moving = moving.min(prev);
            if moving == 0 {
                continue;
            }
            let life_src = self.pe_eq_life_points[index - 1];
            let life_move = life_src.wrapping_mul(moving) / prev;
            // Proportional aux slice leaves the source; bonuses apply only to dest in-flight.
            let aux_src = self.pe_eq_aux_points[index - 1];
            let aux_src_loss = aux_src.wrapping_mul(moving) / prev;
            let mut aux_dest = aux_src_loss;
            // PE school-age dest (index < 3): aux += min(esp+0x1c, moving) * 35.
            if index < 3 {
                let school_bonus = params.school_aux_param.min(moving).wrapping_mul(35);
                aux_dest = aux_dest.wrapping_add(school_bonus);
            } else if index == 3 && moving > 0 {
                // College dest: aux += (min(esp+0x18, moving) * aux_move / moving) / 2.
                let college_cap = params.college_aux_param.min(moving);
                let college_bonus = college_cap.wrapping_mul(aux_src_loss) / moving / 2;
                aux_dest = aux_dest.wrapping_add(college_bonus);
            }
            // PE `test ah,1` (`0x4caa40` bit 0x100 Pro Reading): skip `aux -= moving`.
            if !params.pro_reading {
                aux_dest = aux_dest.wrapping_sub(moving);
            }
            self.pe_age_cohorts[index - 1] = prev - moving;
            self.pe_age_cohorts[index] = self.pe_age_cohorts[index].wrapping_add(moving);
            self.pe_eq_life_points[index - 1] =
                self.pe_eq_life_points[index - 1].wrapping_sub(life_move);
            // PE subtracts the penalty as a dword, so a small life_move can
            // underflow before the destination addition wraps as well.
            self.pe_eq_life_points[index] =
                self.pe_eq_life_points[index].wrapping_add(life_move.wrapping_sub(life_penalty));
            self.pe_eq_aux_points[index - 1] =
                self.pe_eq_aux_points[index - 1].wrapping_sub(aux_src_loss);
            self.pe_eq_aux_points[index] = self.pe_eq_aux_points[index].wrapping_add(aux_dest);
        }
    }

    pub(super) fn pe_age_cohorts_births_one_month(
        &mut self,
        params: &PeEqLeMonthParams,
        rng: &mut Win95CrtRand,
    ) -> u32 {
        // PE `0x4698ae`: newborns ≈ sum(EQ_LE[4..=8]) / 300 into bucket 0.
        let fertile = self.pe_age_cohorts[4..=8]
            .iter()
            .copied()
            .fold(0_u32, u32::wrapping_add);
        // PE obtains the base with signed IDIV 300. At 0x4698d4 it reloads
        // the age-forward life penalty, so the rounding threshold uses
        // fertile - 300 * penalty instead of the division remainder.
        let fertile_signed = fertile as i32;
        let mut births = (fertile_signed / 300) as u32;
        let rounding_threshold = fertile_signed.wrapping_sub((params.life_penalty() as i32) * 300);
        let rounding_roll = u32::from(rng.random_u16()) % 300;
        if (rounding_roll as i32) < rounding_threshold {
            births = births.wrapping_add(1);
        }
        if births == 0 {
            return 0;
        }
        self.pe_age_cohorts[0] = self.pe_age_cohorts[0].wrapping_add(births);
        // PE `0x4698fb`: (esp+0x14 - 0x23) * min(births, esp+0x48) + births*35.
        let life_add =
            pe_eq_birth_life_points(births, params.life_expectancy, params.hospital_birth_param);
        self.pe_eq_life_points[0] = self.pe_eq_life_points[0].wrapping_add(life_add);
        // PE aux[0] += births * `[0x4cc4b4]` / 5; that slot is last month's fertile EQ.
        let aux_add = births.wrapping_mul(self.pe_eq_education_average) / 5;
        self.pe_eq_aux_points[0] = self.pe_eq_aux_points[0].wrapping_add(aux_add);
        self.pe_eq_births = self.pe_eq_births.wrapping_add(births);
        births
    }

    /// Win95 leftover rebalance at `0x469945`: hold EQ_LE pop after deaths/births.
    pub(super) fn pe_age_cohorts_rebalance_one_month(
        &mut self,
        month_deaths: u32,
        month_births: u32,
        city_population: u32,
        rng: &mut Win95CrtRand,
    ) {
        if month_deaths > month_births {
            self.pe_age_cohorts_immigrate_deficit(month_deaths - month_births);
        } else if month_births > month_deaths {
            self.pe_age_cohorts_emigrate_surplus(month_births - month_deaths, city_population, rng);
        }
    }

    /// When deaths exceed births, fill buckets 4..=7 then 0..=11 (`0x469976`).
    pub(super) fn pe_age_cohorts_immigrate_deficit(&mut self, leftover: u32) {
        let mut leftover = leftover;
        // PE computes this once from the original deficit (`shr edx,4; inc edx`)
        // and reuses it while visiting the two bucket ranges.
        let chunk = pe_eq_immigrant_chunk(leftover);
        while leftover > 0 {
            // PE first visits buckets 4..=7. IMUL and ADD write only the low
            // dword, as do the population and remaining-deficit updates.
            for index in 4..=7 {
                let take = leftover.min(chunk);
                if take == 0 {
                    break;
                }
                let ecx = index as u32;
                self.pe_age_cohorts[index] = self.pe_age_cohorts[index].wrapping_add(take);
                self.pe_eq_life_points[index] = self.pe_eq_life_points[index]
                    .wrapping_add(65_u32.wrapping_sub(ecx).wrapping_mul(take));
                self.pe_eq_aux_points[index] = self.pe_eq_aux_points[index]
                    .wrapping_add(90_u32.wrapping_sub(ecx).wrapping_mul(take));
                leftover = leftover.wrapping_sub(take);
            }
            if leftover == 0 {
                break;
            }
            // PE then visits buckets 0..=11; aux for buckets 0..=2 uses
            // ebx=17,52,87, while buckets 3..=11 use 90-index.
            let mut ebx = 17_u32;
            for index in 0..=11 {
                let take = leftover.min(chunk);
                if take == 0 {
                    break;
                }
                let ecx = index as u32;
                self.pe_age_cohorts[index] = self.pe_age_cohorts[index].wrapping_add(take);
                self.pe_eq_life_points[index] = self.pe_eq_life_points[index]
                    .wrapping_add(65_u32.wrapping_sub(ecx).wrapping_mul(take));
                let aux_add = if index < 3 {
                    ebx.wrapping_mul(take)
                } else {
                    90_u32.wrapping_sub(ecx).wrapping_mul(take)
                };
                self.pe_eq_aux_points[index] = self.pe_eq_aux_points[index].wrapping_add(aux_add);
                ebx = ebx.wrapping_add(35);
                leftover = leftover.wrapping_sub(take);
            }
        }
    }

    /// When births exceed deaths, scan all 20 buckets and remove each PE-proportional share.
    pub(super) fn pe_age_cohorts_emigrate_surplus(
        &mut self,
        surplus: u32,
        city_population: u32,
        rng: &mut Win95CrtRand,
    ) {
        let mut leftover = surplus;
        while leftover > 0 {
            // The PE snapshots the numerator at the start of each pass
            // (0x469a77 / 0x469bdc) while the denominator tracks live surplus.
            let pass_surplus = leftover;
            let mut has_cohort = false;
            for index in 0..20 {
                if leftover == 0 {
                    break;
                }
                let pop = self.pe_age_cohorts[index];
                if pop == 0 {
                    continue;
                }
                has_cohort = true;
                let denominator = leftover.wrapping_add(city_population);
                let mut take = pe_ftol_x87_divide_then_mul(pass_surplus, denominator, pop) as u32;
                if take == 0 && (u32::from(rng.random_u16()) & 3) == 0 {
                    take = 1;
                }
                let take = take.min(pop).min(leftover);
                if take == 0 {
                    continue;
                }
                let life = self.pe_eq_life_points[index];
                let aux = self.pe_eq_aux_points[index];
                let life_loss = (pe_ftol_x87_divide_then_mul(life, pop, take) as u32).min(life);
                let aux_loss = (pe_ftol_x87_divide_then_mul(aux, pop, take) as u32).min(aux);
                self.pe_eq_life_points[index] = life.wrapping_sub(life_loss);
                self.pe_eq_aux_points[index] = aux.wrapping_sub(aux_loss);
                self.pe_age_cohorts[index] = pop - take;
                leftover -= take;
            }
            // The PE begins another ascending 20-bucket pass while its saved
            // birth surplus remains. If malformed state requests more than
            // the available cohort population, avoid an endless Rust loop.
            if !has_cohort {
                return;
            }
        }
    }

    pub fn pe_age_cohort_at(&self, index: usize) -> u32 {
        self.pe_age_cohorts.get(index).copied().unwrap_or(0)
    }

    pub fn pe_eq_deaths(&self) -> u32 {
        self.pe_eq_deaths
    }

    pub fn pe_eq_births(&self) -> u32 {
        self.pe_eq_births
    }

    pub fn pe_eq_cohort_population(&self) -> u32 {
        self.pe_age_cohorts.iter().copied().sum()
    }

    pub fn pe_eq_life_average(&self) -> u32 {
        self.pe_eq_life_average
    }

    pub fn pe_eq_education_average(&self) -> u32 {
        self.pe_eq_education_average
    }

    pub fn pe_eq_workforce_percent(&self) -> u32 {
        self.pe_eq_workforce_percent
    }

    /// Whether Win95's three published EQ/LE summary globals have an authoritative
    /// source (loaded MISC, a saved summary write, or the EQ/LE worker). Zero is
    /// a valid published value; this does not mean the cohort tables are populated.
    pub fn pe_eq_averages_ready(&self) -> bool {
        self.pe_eq_averages_ready
    }
}

/// Win95 EQ_LE survival score from `0x469698`: unsigned `life/pop`, wrapped
/// dword `*100`, then signed `IDIV age_divisor`.
pub fn pe_eq_survival_score(life_points: u32, pop: u32, age_divisor: u32) -> i32 {
    if pop == 0 {
        return 100;
    }
    let scaled = (life_points / pop).wrapping_mul(100) as i32;
    scaled / (age_divisor.max(1) as i32)
}

/// Monthly locals for EQ_LE birth/age bonuses (`esp+0x14/0x18/0x1c/0x48`).
#[derive(Clone, Copy)]
pub(super) struct PeEqLeMonthParams {
    pub(super) life_expectancy: u32,
    pub(super) hospital_birth_param: u32,
    pub(super) school_aux_param: u32,
    pub(super) college_aux_param: u32,
    /// Win95 `dwCityPollution` `0x4cc910` (age-forward life penalty numerator).
    pub(super) education_demand: u32,
    /// Win95 `dwCityPopulation` `0x4caa74` (age-forward life penalty denom base).
    pub(super) city_population: u32,
    /// Win95 growth-aggregate slot 7 (`[0x4CA428]+0x1C`) for `10 * slot7` in the denom.
    pub(super) growth_slot7: i32,
    pub(super) pro_reading: bool,
}

impl PeEqLeMonthParams {
    fn life_penalty(&self) -> u32 {
        pe_eq_education_life_penalty(
            self.education_demand,
            self.city_population,
            self.growth_slot7,
        )
    }
}

/// Win95 hospital birth cap `esp+0x48`: wrapped signed dword
/// `(hospital_count * raw_health_funding * 25) / 100`.
/// `hospital_count` is the positive building count (PE `dwTileCount / 9`);
/// the funding dword is consumed raw, with no 100% clamp.
pub fn pe_eq_hospital_birth_param(hospital_count: u32, health_funding: u32) -> u32 {
    let scaled = (hospital_count as i32)
        .wrapping_mul(health_funding as i32)
        .wrapping_mul(25);
    (scaled / 100) as u32
}

/// Win95 Free Clinics (`0x4caa40` bit `0x40`) adds the signed
/// `pBudgetArr[0].iCurrentCosts / 400` (`[0x4cc4cc]+0x60`) to the hospital cap.
pub fn pe_eq_free_clinics_hospital_bump(ordinance_flags: u32, residential_tax_base: u32) -> u32 {
    if ordinance_flags & 0x40 == 0 {
        return 0;
    }
    ((residential_tax_base as i32) / 400) as u32
}

/// Win95 school age-forward aux cap `esp+0x1c`: wrapped signed dword
/// `(schools * raw_school_funding * 15) / 100`.
pub fn pe_eq_school_aux_param(school_count: u32, school_funding: u32) -> u32 {
    let scaled = (school_count as i32)
        .wrapping_mul(school_funding as i32)
        .wrapping_mul(15);
    (scaled / 100) as u32
}

/// Win95 college age-forward aux cap `esp+0x18`: wrapped signed dword
/// `(colleges * raw_college_funding * 50) / 100`.
pub fn pe_eq_college_aux_param(college_count: u32, college_funding: u32) -> u32 {
    let scaled = (college_count as i32)
        .wrapping_mul(college_funding as i32)
        .wrapping_mul(50);
    (scaled / 100) as u32
}

/// Win95 newborn life points at `0x4698fb`, using wrapping dword arithmetic:
/// `(life_expectancy - 35) * min(births, hospital_param) + births * 35`.
pub fn pe_eq_birth_life_points(births: u32, life_expectancy: u32, hospital_param: u32) -> u32 {
    let capped = births.min(hospital_param);
    life_expectancy
        .wrapping_sub(35)
        .wrapping_mul(capped)
        .wrapping_add(births.wrapping_mul(35))
}

/// Win95 age-forward life penalty `esp+0x10` @ `0x469761`:
/// `min(dwCityPollution 0x4cc910 / (dwCityPopulation 0x4caa74 + 10 * slot7 + 1), 3)`.
///
/// Slot 7 is the growth-aggregate special weight (`[base+0x1C]`). PE uses unsigned
/// `div`; negative slot7 follows dword wrapping via the `lea` chain.
pub fn pe_eq_education_life_penalty(
    pollution: u32,
    city_population: u32,
    growth_slot7: i32,
) -> u32 {
    let denom = city_population
        .wrapping_add((growth_slot7 as u32).wrapping_mul(10))
        .wrapping_add(1);
    if denom == 0 {
        return 0;
    }
    (pollution / denom).min(3)
}

/// Win95 EQ_LE newborn LE base `esp+0x14` from ordinance bits on `0x4caa40`.
///
/// Starts at 85; Anti-Drug (`0x200`) raises the floor to 90; CPR (`0x400`) and
/// Smoking Ban (`0x20`) each add 5.
pub fn pe_eq_ordinance_life_expectancy(ordinance_flags: u32) -> u32 {
    let mut life = if ordinance_flags & 0x200 != 0 { 90 } else { 85 };
    if ordinance_flags & 0x400 != 0 {
        life += 5;
    }
    if ordinance_flags & 0x20 != 0 {
        life += 5;
    }
    life
}

/// Win95 Pro Reading campaign (`0x4caa40` bit `0x100`) skips age-forward aux decay.
pub fn pe_eq_ordinance_pro_reading(ordinance_flags: u32) -> bool {
    ordinance_flags & 0x100 != 0
}

/// Win95 fertile buckets 4..=10 averages at `0x469be8`.
/// Returns `(life_avg 0x4cb410, aux_avg 0x4cc4b4, workforce% 0x4cb460)`.
pub fn pe_eq_fertile_averages(
    cohorts: &[u32; 20],
    life: &[u32; 20],
    aux: &[u32; 20],
    city_population: u32,
) -> (u32, u32, u32) {
    let mut life_sum = 0_u32;
    let mut aux_sum = 0_u32;
    let mut pop_sum = 0_u32;
    for index in 4..=10 {
        life_sum = life_sum.wrapping_add(life[index]);
        aux_sum = aux_sum.wrapping_add(aux[index]);
        pop_sum = pop_sum.wrapping_add(cohorts[index]);
    }
    if pop_sum == 0 {
        return (0, 0, 0);
    }
    let workforce = pop_sum.wrapping_mul(100) / city_population.wrapping_add(1);
    (life_sum / pop_sum, aux_sum / pop_sum, workforce)
}

/// Win95 immigrant chunk `leftover/16 + 1` at `0x46996a`.
pub fn pe_eq_immigrant_chunk(leftover: u32) -> u32 {
    leftover / 16 + 1
}

impl City {
    /// Snap growth-aggregate slot 7 for the day-21 EQ_LE life penalty denom.
    pub fn set_pe_eq_growth_slot7(&mut self, slot7: i32) {
        self.pe_eq_growth_slot7 = slot7;
    }

    pub fn pe_eq_growth_slot7(&self) -> i32 {
        self.pe_eq_growth_slot7
    }

    /// Win95 `FUN_00468150` writes folded `(1+2)/(3+4)/(5+6)` into `pZonePops`.
    pub fn set_pe_zone_pops(&mut self, rows: [i32; 3]) {
        self.pe_zone_pops = rows;
        self.pe_zone_pops_ready = true;
    }

    pub fn clear_pe_zone_pops(&mut self) {
        self.pe_zone_pops = [0; 3];
        self.pe_zone_pops_ready = false;
    }

    pub fn pe_zone_pops_ready(&self) -> bool {
        self.pe_zone_pops_ready
    }

    pub fn pe_zone_pops(&self) -> [i32; 3] {
        self.pe_zone_pops
    }

    pub fn pe_eq_leftover_deaths(&self) -> u32 {
        self.pe_eq_leftover_deaths
    }

    pub fn pe_eq_leftover_births(&self) -> u32 {
        self.pe_eq_leftover_births
    }

    /// Win95 day 22 (`0x46818a`): park the difference between the folded
    /// six-counter total times ten and `dwCityPopulation`, then snapshot it.
    /// Growth (new > old) goes into deaths so the next month immigrates.
    pub fn persist_pe_day22_population_leftover(&mut self, zone_scaled_pop: u32) {
        let old = self.pe_dw_city_population;
        self.pe_eq_leftover_births = 0;
        self.pe_eq_leftover_deaths = 0;
        if zone_scaled_pop < old {
            self.pe_eq_leftover_births = old - zone_scaled_pop;
        } else {
            self.pe_eq_leftover_deaths = zone_scaled_pop - old;
        }
        self.pe_dw_city_population = zone_scaled_pop;
        // The selected PE engine reads and writes this saved slot for either
        // file layout; the Mac engine has a distinct producer for the slot.
        self.set_misc_u32_at(MISC_NORMAL_POPULATION_OFFSET, zone_scaled_pop);
    }
}
