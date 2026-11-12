//! Win95 yearly structure microsim refresh and dispatch.

use super::*;

impl City {
    /// January structure-microsim refresh.
    ///
    /// School / hospital / college / stadium / police / fire / prison / zoo /
    /// llama / city-hall / statue / mayor / water / desal / museum / library /
    /// park / marina / bus / rail / subway / power / arco follow Win95 `FUN_0044e390` (thunk `0x402ac7`, year-end
    /// `UpdateBudgetInformation`). Museum/library do not age-forward EQ_LE.
    /// Power-plant newspaper (age>48) queues PE story `0x24`. Aged plants rebuild only
    /// when No Disasters (`0x4CC4D4` / MISC `0x1000`) is on and funds cover cost;
    /// otherwise they demolish. The Mayor's House copies the latched Approval
    /// (`0x4E9904`). School/college occupancy reads
    /// EQ_LE buckets restored from MISC `0x7c` (`[0x4c94b4]+4/+8` and `+0xc`); empty
    /// tables stay empty. Monthly EQ_LE aging is calendar day 21, not this pass.
    ///
    /// Returns total arco residents written into microsims (for MISC sync).
    pub fn refresh_structure_microsim_yearly(
        &mut self,
        normal_population: u32,
        police_funding_raw: u32,
        fire_funding_raw: u32,
        health_funding_raw: u32,
        school_funding_raw: u32,
        college_funding_raw: u32,
    ) -> u32 {
        self.refresh_structure_microsim_yearly_with_rng(
            normal_population,
            police_funding_raw,
            fire_funding_raw,
            health_funding_raw,
            school_funding_raw,
            college_funding_raw,
            &mut Win95CrtRand::new(Win95CrtRand::DEFAULT_SEED),
            &mut Win95Lfsr::new(Win95Lfsr::DEFAULT_STATE),
        )
    }

    /// Offline convenience entry with injectable CRT and Game_Random streams; the separate
    /// zoo game-LCG uses a deterministic seed derived from the CRT stream here.
    pub fn refresh_structure_microsim_yearly_with_rng(
        &mut self,
        normal_population: u32,
        police_funding_raw: u32,
        fire_funding_raw: u32,
        health_funding_raw: u32,
        school_funding_raw: u32,
        college_funding_raw: u32,
        rng: &mut Win95CrtRand,
        lfsr: &mut Win95Lfsr,
    ) -> u32 {
        // The live app uses `with_rngs` to preserve this stream between years.
        // Offline callers get a deterministic local stream without CRT advancement.
        let mut game_rng = Win95GameRand::new(rng.seed());
        self.refresh_structure_microsim_yearly_with_rngs(
            normal_population,
            police_funding_raw,
            fire_funding_raw,
            health_funding_raw,
            school_funding_raw,
            college_funding_raw,
            rng,
            &mut game_rng,
            lfsr,
        )
    }

    /// Yearly updater with the three independent Win95 random streams injectable.
    pub fn refresh_structure_microsim_yearly_with_rngs(
        &mut self,
        normal_population: u32,
        police_funding_raw: u32,
        fire_funding_raw: u32,
        health_funding_raw: u32,
        school_funding_raw: u32,
        college_funding_raw: u32,
        rng: &mut Win95CrtRand,
        game_rng: &mut Win95GameRand,
        lfsr: &mut Win95Lfsr,
    ) -> u32 {
        if self.microsims.is_none() {
            self.reset_pe_transit_passenger_totals();
            return 0;
        }
        let arco_population = self.pe_dw_arcology_population;
        // Win95 `0x4CA1DC` prison-bonus word is snapshotted then cleared at updater entry.
        // PE snapshots the raw WORD and later sign-extends it in the police
        // arm (`MOVSX ECX, word [esp+0x1c]`) before forming 5 - bonus.
        let prior_prison_bonus = i32::from(self.pe_w_prison_bonus as i16);
        self.pe_w_prison_bonus = 0;

        let mut school_indices = self.structure_microsim_indices_for(214);
        let mut college_indices = self.structure_microsim_indices_for(217);
        let mut hospital_indices = self.structure_microsim_indices_for(209);
        let mut police_indices = self.structure_microsim_indices_for(210);
        let mut fire_indices = self.structure_microsim_indices_for(211);
        let mut stadium_indices = self.structure_microsim_indices_for(215);
        let mut prison_indices = self.structure_microsim_indices_for(216);
        let mut zoo_indices = self.structure_microsim_indices_for(218);
        let mut llama_indices = self.structure_microsim_indices_for(255);
        let mut city_hall_indices = self.structure_microsim_indices_for(208);
        let mut statue_indices = self.structure_microsim_indices_for(219);
        let mut mayor_indices = self.structure_microsim_indices_for(243);
        let mut water_indices = self.structure_microsim_indices_for(244);
        let mut desal_indices = self.structure_microsim_indices_for(250);

        // PE `0x44e7ae` reads the live `dwCityCrime` BSS at `0x4cb454`.
        // Do not use `misc_crime_count()`: its display/save compatibility
        // fallback can resurrect an older serialized total when live crime is 0.
        let city_crime = self.pe_dw_city_crime;
        // PE occupancy divisors are `dwTileCount` cell census (`0x4CA4C8`), not microsim
        // rows. 3×3 footprints `/9`, 4×4 `/16`; stadium and police use the raw tile word.
        let police_station_count = pe_occupancy_divisor(pe_occupancy_tile_words(self, &[210]));
        // PE school/college pools are EQ_LE dwords at `[0x4c94b4]+4/+8` and `+0xc`.
        // Day-22 leftover immigration fills those ages; empty tables stay empty.
        let school_student_pool = self.pe_age_cohorts[1].saturating_add(self.pe_age_cohorts[2]);
        let college_student_pool = self.pe_age_cohorts[3];
        let school_count = pe_occupancy_divisor(pe_occupancy_div9_count(self, 214));
        let college_count = pe_occupancy_divisor(pe_occupancy_div16_count(self, 217));
        let hospital_count = pe_occupancy_div9_count(self, 209);
        let stadium_count = pe_occupancy_divisor(pe_occupancy_tile_words(self, &[215]));
        // Raw `dwTileCount` cells: museum 212 (`+0x1a8`), library 245 (`+0x1ea`).
        let museum_tiles = count_building_tiles(self, &[212]);
        let library_tiles = count_building_tiles(self, &[245]);
        // Park tiles 13+213 (`+0x1a` / `+0x1aa`); marina 248 (`+0x1f0`).
        let park_tiles = count_building_tiles(self, &[13, 213]);
        let marina_tiles = count_building_tiles(self, &[248]);
        let bus_tiles = count_building_tiles(self, &[236]);
        let rail_tiles = count_building_tiles(self, &[237]);
        let subway_tiles = count_building_tiles(self, &[233]);
        // PE transit microsims read `dwBusPassengers`/`dwRailPassengers`/`dwSubwayPassengers`
        // @ `0x4c85a0`/`0x4c85a4`/`0x4c8600` (viewer MISC snapshots `+0x1120`..`+0x1128`).
        let bus_passengers = self.pe_dw_bus_passengers;
        let rail_passengers = self.pe_dw_rail_passengers;
        let subway_passengers = self.pe_dw_subway_passengers;
        // PE `wOldArrests` (`0x4CA430`): prior-year arrest accumulator for prison intake.
        let old_arrests = u32::from(self.pe_w_old_arrests);
        // PE `0x44eb90` reads a signed WORD from `[ESP+0x12]`, not from the
        // nearby pre-push `dwTileCount+0x1b0` local. That word aliases the
        // high WORD of the title helper's CString buffer pointer. Its native
        // allocator-derived value is not yet captured, so keep the stable
        // facility-count proxy until a rooted PE run establishes it.
        let prison_arrest_divisor = pe_occupancy_divisor(pe_occupancy_div16_count(self, 216));
        // PE llama short path when city name strcmp equals `"aus"` (`0x4e807c`).
        let llama_short_path = self.name == "aus";
        // PE `0x4CC4B8`: watered×100/demand (100 when demand is 0). App updates via
        // [`Self::set_pe_water_coverage_percent`] after the water service scan.
        let water_efficiency = self.pe_water_coverage_percent as u8;
        // PE `0x4CAA50` power-used%; defaults to 100 when unset (no-demand path).
        let power_used_percent = self.pe_power_used_percent as u16;
        // PE `0x4CC4D4` is the No Disasters option (MISC `0x1000`); default off.
        let no_disasters = self.misc_u32_at(MISC_NO_DISASTERS_OFFSET).unwrap_or(0) != 0;
        let tax_r = self
            .misc_u32_at(MISC_TAX_RATE_R_OFFSET)
            .unwrap_or(DEFAULT_TAX_RATE)
            .min(20) as i32;
        let tax_c = self
            .misc_u32_at(MISC_TAX_RATE_C_OFFSET)
            .unwrap_or(DEFAULT_TAX_RATE)
            .min(20) as i32;
        let tax_i = self
            .misc_u32_at(MISC_TAX_RATE_I_OFFSET)
            .unwrap_or(DEFAULT_TAX_RATE)
            .min(20) as i32;
        // PE `FUN_0044f7d0` / occupancy prologue: arco cells 251..=254 `/16`.
        // Count feeds the >140 bonus; `.max(1)` is only the arco funding cap.
        let arco_building_count_raw = pe_occupancy_arco_footprints(self);
        let arco_building_count = arco_building_count_raw.max(1);
        let pe_capacity_pop = pe_microsim_population_load(
            normal_population,
            arco_population,
            pe_arco_tile_pop_bonus(arco_building_count_raw),
        );
        let mayor_approval = self.pe_mayor_approval;
        // PE marina/arco occupancy use Game_Random LFSR (`0x401415` / `0x401cad`).
        let mut city_funds = self.city_funds();
        let mut pending_power_demolish: Vec<(usize, u8, (usize, usize))> = Vec::new();
        let mut pending_power_aging_news: Vec<u8> = Vec::new();
        let mut pending_fund_education_news = false;
        let mut pending_low_school_score_news = false;
        let mut pending_prison_overcrowded_news = false;

        let mut arco_residents_total = 0_u32;
        let mut changed = false;
        let Some(records) = self.microsims.as_deref() else {
            return 0;
        };
        let rng_schedule = yearly_microsim_rng_schedule(
            self,
            records,
            *rng,
            *lfsr,
            YearlyMicrosimRngInputs {
                normal_population,
                population_load: pe_capacity_pop,
                school_student_pool,
                school_count,
                college_student_pool,
                college_count,
                hospital_count,
                stadium_count,
                old_arrests,
                prison_arrest_divisor,
                marina_tiles,
                llama_short_path,
                no_disasters,
                city_funds,
            },
        );
        let demolition_anchors = &rng_schedule.demolition_anchors;
        let demolition_sites = &rng_schedule.demolition_sites;
        let dispatched_rows = &rng_schedule.dispatched_rows;
        school_indices.retain(|&index| dispatched_rows[index]);
        college_indices.retain(|&index| dispatched_rows[index]);
        hospital_indices.retain(|&index| dispatched_rows[index]);
        police_indices.retain(|&index| dispatched_rows[index]);
        fire_indices.retain(|&index| dispatched_rows[index]);
        stadium_indices.retain(|&index| dispatched_rows[index]);
        prison_indices.retain(|&index| dispatched_rows[index]);
        zoo_indices.retain(|&index| dispatched_rows[index]);
        llama_indices.retain(|&index| dispatched_rows[index]);
        city_hall_indices.retain(|&index| dispatched_rows[index]);
        statue_indices.retain(|&index| dispatched_rows[index]);
        mayor_indices.retain(|&index| dispatched_rows[index]);
        water_indices.retain(|&index| dispatched_rows[index]);
        desal_indices.retain(|&index| dispatched_rows[index]);
        let Some(mut microsims) = self.microsims.take() else {
            return 0;
        };

        for &index in &school_indices {
            *rng = rng_schedule.crt_start_by_slot[index]
                .expect("school microsim consumes the yearly CRT stream");
            let Some(record) = microsims.get_mut(index) else {
                continue;
            };
            // Win95 school case @ `0x44e978`.
            // PE @ 0x44e983..0x44e98c performs signed dword division by four
            // with truncation toward zero, then stores AX. The following
            // unsigned word compare gates news.
            let funding_quarters = pe_school_funding_quarters(school_funding_raw);
            if funding_quarters < 20 {
                pending_fund_education_news = true;
            }
            let mut students = (school_student_pool / school_count)
                .wrapping_add(u32::from(rng.random_u16_mask(0xf)));
            if (students as i32) > 1_500 {
                students = 1_500 + u32::from(rng.random_u16_mask(0xff));
            }
            let students = pe_microsim_scaled_capacity(pe_capacity_pop, 20, students);
            let teachers_raw = pe_school_teachers_raw(
                school_funding_raw,
                i32::from(rng.random_u16_mask(7)),
                record.stat0,
            );
            let teachers = pe_microsim_scaled_capacity(pe_capacity_pop, 100, teachers_raw);
            record.stat0 = pe_school_grade(students, teachers);
            // OpenSC2K annual phase: any school_score < 4 → NEWS_EDUCATION `0x26`.
            if record.stat0 < 4 {
                pending_low_school_score_news = true;
            }
            record.stat1 = students.min(u32::from(u16::MAX)) as u16;
            record.stat2 = teachers.min(u32::from(u16::MAX)) as u16;
            record.stat3 = funding_quarters;
            changed = true;
        }

        for &index in &college_indices {
            *rng = rng_schedule.crt_start_by_slot[index]
                .expect("college microsim consumes the yearly CRT stream");
            let Some(record) = microsims.get_mut(index) else {
                continue;
            };
            // Win95 college case @ `0x44ec8a`.
            // PE adds the jitter as a 32-bit value, then uses signed JLE for
            // the 5,000 threshold (`cmp eax, 0x1388; jle`).
            let mut students = (college_student_pool / college_count)
                .wrapping_add(u32::from(rng.random_u16_mask(0x1f)));
            if (students as i32) > 5_000 {
                students = 5_000 + u32::from(rng.random_u16_mask(0x1ff));
            }
            let students = pe_microsim_scaled_capacity(pe_capacity_pop, 30, students);
            let professors_raw = pe_college_professors_raw(
                college_funding_raw,
                u32::from(rng.random_u16_mask(0xf)),
                record.stat0,
            );
            let professors = pe_microsim_scaled_capacity(pe_capacity_pop, 100, professors_raw);
            record.stat0 = pe_hospital_style_grade(students.saturating_mul(4) / professors.max(1));
            record.stat1 = students.min(u32::from(u16::MAX)) as u16;
            record.stat2 = professors.min(u32::from(u16::MAX)) as u16;
            record.stat3 = college_funding_raw as u16;
            changed = true;
        }

        for &index in &hospital_indices {
            *rng = rng_schedule.crt_start_by_slot[index]
                .expect("hospital microsim consumes the yearly CRT stream");
            let Some(record) = microsims.get_mut(index) else {
                continue;
            };
            // Win95 hospital case @ `0x44e662`: patients = city_pop / max(footprints*25, 1).
            let scaled_hospitals = pe_occupancy_divisor(hospital_count.saturating_mul(25));
            let mut patients = (normal_population / scaled_hospitals)
                .wrapping_add(u32::from(rng.random_u16_mask(0xf)));
            if (patients as i32) > 1_000 {
                patients = 1_000 + u32::from(rng.random_u16_mask(0x7f));
            }
            let patients = pe_microsim_scaled_capacity(pe_capacity_pop, 30, patients);
            let doctors_raw = pe_hospital_doctors_raw(
                health_funding_raw,
                i32::from(rng.random_u16_mask(7)),
                i32::from(record.stat0) * 2 - 0x18,
            );
            let doctors = pe_microsim_scaled_capacity(pe_capacity_pop, 120, doctors_raw);
            record.stat0 = pe_hospital_style_grade(patients.saturating_mul(10) / doctors.max(1));
            record.stat1 = patients.min(u32::from(u16::MAX)) as u16;
            record.stat2 = doctors.min(u32::from(u16::MAX)) as u16;
            // PE `cdq; sub eax, edx; sar eax, 1` implements signed division
            // by two with truncation toward zero, then stores AX.
            record.stat3 = pe_hospital_funding_half(health_funding_raw);
            changed = true;
        }

        let mut new_old_arrests = 0_u32;
        for &index in &police_indices {
            *rng = rng_schedule.crt_start_by_slot[index]
                .expect("police microsim consumes the yearly CRT stream");
            let Some(record) = microsims.get_mut(index) else {
                continue;
            };
            // Win95 `FUN_0044e390` police case @ `0x44e764`.
            let funding_cap = police_funding_raw.wrapping_mul(2);
            let capacity = pe_microsim_scaled_capacity(pe_capacity_pop, 90, funding_cap);
            let crimes = city_crime / police_station_count;
            let arrest_div = (5 - prior_prison_bonus).max(1) as u32;
            let arrests = crimes / arrest_div + u32::from(rng.random_u16_mask(0xf));
            record.stat0 = police_funding_raw as u8;
            record.stat1 = capacity.min(u32::from(u16::MAX)) as u16;
            // PE writes SI/CX directly to the XMIC WORD fields; preserve the
            // low word even when the computed crime/arrest value exceeds it.
            record.stat2 = crimes as u16;
            record.stat3 = arrests as u16;
            new_old_arrests = new_old_arrests.saturating_add(arrests).min(0xffff);
            changed = true;
        }

        for &index in &fire_indices {
            *rng = rng_schedule.crt_start_by_slot[index]
                .expect("fire microsim consumes the yearly CRT stream");
            let Some(record) = microsims.get_mut(index) else {
                continue;
            };
            // Win95 `FUN_0044e390` fire case @ `0x44e81c`.
            let funding_cap = ((fire_funding_raw as i32) >> 1) as u32;
            let capacity = pe_microsim_scaled_capacity(pe_capacity_pop, 70, funding_cap);
            let trucks = (capacity >> 4).saturating_add(1);
            let firefighters = u32::from(rng.random_u15() % 20) + 2;
            record.stat0 = fire_funding_raw as u8;
            record.stat1 = capacity.min(u32::from(u16::MAX)) as u16;
            record.stat2 = trucks.min(u32::from(u16::MAX)) as u16;
            record.stat3 = firefighters.min(u32::from(u16::MAX)) as u16;
            changed = true;
        }

        for &index in &stadium_indices {
            *rng = rng_schedule.crt_start_by_slot[index]
                .expect("stadium microsim consumes the yearly CRT stream");
            let Some(record) = microsims.get_mut(index) else {
                continue;
            };
            // Win95 stadium case @ `0x44eaba`: first ÷ is raw stadium tiles, pop+arco+bonus.
            let mut attendance = pe_capacity_pop / stadium_count;
            // PE's `cmp eax, 0x61a8; jle` is a signed comparison even though
            // the preceding DIV produced an unsigned quotient.
            if (attendance as i32) > 25_000 {
                attendance = 25_000 - u32::from(rng.random_u16_mask(0xff));
            }
            let attendance = (pe_microsim_scaled_capacity(pe_capacity_pop, 5, attendance) as u16)
                .wrapping_add(rng.random_u16_mask(0xff));
            record.stat0 = (u32::from(rng.random_u16_mask(0x1f)) + 9).min(255) as u8;
            record.stat1 = attendance;
            changed = true;
        }

        for &index in &prison_indices {
            *rng = rng_schedule.crt_start_by_slot[index]
                .expect("prison microsim consumes the yearly CRT stream");
            let Some(record) = microsims.get_mut(index) else {
                continue;
            };
            // Win95 prison case @ `0x44eb8e`.
            let prev_inmates = u32::from(record.stat1);
            let mut inmates = prev_inmates
                // PE zero-extends this word before `cdq; and edx, 3; add eax,
                // edx; sar eax, 2`; for the persisted nonnegative input that
                // is floor(previous / 4), not a ceiling division.
                .saturating_sub(prev_inmates / 4)
                .saturating_add(old_arrests / prison_arrest_divisor);
            if inmates > 10_000 {
                inmates = 10_000 + u32::from(rng.random_u16_mask(0x3ff));
            }
            let capacity = pe_microsim_scaled_capacity(pe_capacity_pop, 20, inmates);
            let guards_cap = pe_microsim_scaled_capacity(
                pe_capacity_pop,
                120,
                police_funding_raw.wrapping_mul(3),
            );
            let inmate_hundreds = inmates / 100;
            record.stat1 = capacity.min(u32::from(u16::MAX)) as u16;
            record.stat2 = guards_cap.min(u32::from(u16::MAX)) as u16;
            record.stat3 = inmate_hundreds.min(u32::from(u16::MAX)) as u16;
            // OpenSC2K `services.gd`: prison_stat > 105 → NEWS_PRISON `0x25`.
            if inmate_hundreds > 105 {
                pending_prison_overcrowded_news = true;
            }
            if inmate_hundreds > 90 {
                let grade_span =
                    pe_prison_overcrowding_grade_span(inmate_hundreds, police_funding_raw);
                let roll = u32::from(rng.random_u15());
                // PE unsigned-divides by this span; an invalid saved funding
                // value can produce zero and fault the original executable.
                // Keep CityGame resilient for that corrupt state while
                // matching every nonzero PE divisor bit-for-bit.
                record.stat0 = if grade_span == 0 {
                    0
                } else {
                    (roll % grade_span) as u8
                };
            } else {
                record.stat0 = 0;
            }
            changed = true;
        }

        for &index in &zoo_indices {
            let Some(record) = microsims.get_mut(index) else {
                continue;
            };
            // Win95 zoo case @ `0x44ed8a`: four independent calls to the
            // `0x40116d` game-LCG helper, separate from CRT `rand()` and LFSR.
            record.stat0 = game_rng.random_mod(100) as u8;
            record.stat1 = game_rng.random_mod(100);
            record.stat2 = game_rng.random_mod(100);
            record.stat3 = game_rng.random_mod(100);
            changed = true;
        }

        for &index in &llama_indices {
            *rng = rng_schedule.crt_start_by_slot[index]
                .expect("llama microsim consumes the yearly CRT stream");
            let Some(record) = microsims.get_mut(index) else {
                continue;
            };
            // Win95 llama case @ `0x44f14e` (`0x4caa74` ≈ city pop load).
            // Unlike several neighboring arms, the PE Llama arm loads
            // `dwCityPopulation` directly (`0x4caa74`), excluding arcology
            // residents and the arcology tile-count bonus.
            let pop_div8 = normal_population >> 3;
            if llama_short_path {
                // PE writes AX to the saved WORD at 0x44f18c. This is a
                // truncating 16-bit store, not a saturating capacity clamp.
                record.stat1 = pop_div8.wrapping_add(u32::from(rng.random_u16_mask(0x3ff))) as u16;
                record.stat0 = rng.random_u16_mask(0x7f) as u8;
                record.stat2 = u32::from(rng.random_u16_mask(0x7f)).wrapping_add(10) as u16;
            } else {
                record.stat0 = (rng.random_u15() & 0xff) as u8;
                let visitors = pop_div8.wrapping_add(u32::from(rng.random_u16_mask(0x3ff)));
                record.stat1 = visitors as u16;
                // PE adds 7/15 only when the signed visitor value is negative,
                // to round division toward zero. The shifted population plus
                // a 10-bit draw is nonnegative, so both divisions round down.
                let sightings = u32::from(rng.random_u16_mask(0x7f)).wrapping_add(visitors >> 3);
                record.stat2 = sightings as u16;
                let bungee = u32::from(rng.random_u16_mask(0x3f)).wrapping_add(visitors >> 4);
                record.stat3 = bungee as u16;
            }
            changed = true;
        }

        for &index in &city_hall_indices {
            let Some(record) = microsims.get_mut(index) else {
                continue;
            };
            // Win95 city-hall case @ `0x44e647`: only refreshes `stat1`.
            // Win95 pushes cap 900 then divisor 200 at 0x44e647, but the
            // shared helper reads divisor from the second stack argument.
            record.stat1 = pe_microsim_scaled_capacity(pe_capacity_pop, 900, 200)
                .min(u32::from(u16::MAX)) as u16;
            changed = true;
        }

        for &index in &statue_indices {
            *rng = rng_schedule.crt_start_by_slot[index]
                .expect("statue microsim consumes the yearly CRT stream");
            let Some(record) = microsims.get_mut(index) else {
                continue;
            };
            // Win95 statue case @ `0x44eddb`: `stat2 = rand() % 0x2a`.
            record.stat2 = rng.random_mod(0x2a);
            changed = true;
        }

        if let Some(record) = microsims.get_mut(MICROSIM_SYSTEM_MUSEUM) {
            if record.tile_id == 212 {
                // Win95 museum case @ `0x44e887`. College `iFundingPercent` (`+0x454`).
                let museum_tiles_word = museum_tiles as u16;
                let funding_cap =
                    u32::from(museum_tiles_word).wrapping_mul(college_funding_raw.wrapping_shl(2));
                record.stat1 = pe_microsim_scaled_capacity(pe_capacity_pop, 0x14, funding_cap)
                    .min(u32::from(u16::MAX)) as u16;
                // PE `0x44e8bf..0x44e8dc`: signed DWORD IDIV by 10, then
                // 16-bit `imul ax, WORD[dwTileCount+0x1a8]`; the field wraps.
                let funding_tenths = (college_funding_raw as i32) / 10;
                record.stat2 = (funding_tenths as u16).wrapping_mul(museum_tiles_word);
                changed = true;
            }
        }
        if let Some(record) = microsims.get_mut(MICROSIM_SYSTEM_LIBRARY) {
            if record.tile_id == 245 {
                // Win95 library case @ `0x44ef17`. School `iFundingPercent` (`+0x3e4`).
                // PE shifts the raw DWORD left by two, multiplies AX by CX,
                // then lets FUN_0044f7d0 sign-interpret the low cap word.
                let library_tiles_word = library_tiles as u16;
                let library_tiles_signed = i32::from(library_tiles_word as i16);
                let funding_cap =
                    u32::from(library_tiles_word).wrapping_mul(school_funding_raw.wrapping_shl(2));
                record.stat1 = pe_microsim_scaled_capacity(pe_capacity_pop, 0x12, funding_cap)
                    .min(u32::from(u16::MAX)) as u16;
                // PE performs SUB/IMUL/ADD in 32 bits, then accepts only the
                // signed interval (0, 0x7d00) before storing AX.
                let books = u32::from(record.stat2).wrapping_add(
                    school_funding_raw
                        .wrapping_sub(0x32)
                        .wrapping_mul(library_tiles_signed as u32),
                ) as i32;
                if (1..0x7d00).contains(&books) {
                    record.stat2 = books as u16;
                }
                // Grade uses `0x4caa74` alone (`cmp 1; idiv`), not the arco pop load.
                let grade_pop = (normal_population as i32).max(1);
                let grade_numerator = library_tiles_signed
                    .wrapping_mul(school_funding_raw as i32)
                    .wrapping_mul(300);
                let grade = grade_numerator / grade_pop;
                record.stat0 = grade.min(12) as u8;
                changed = true;
            }
        }

        if let Some(record) = microsims.get_mut(MICROSIM_SYSTEM_PARK) {
            if record.tile_id == 213 {
                // Win95 park case @ `0x44e8e6`. Attendance uses last year's tile word
                // in stat2 (`412×stat2` vs `dwCityPopulation/6`, cap 0xfde8), then
                // stat2 is rewritten as small+big park tiles. Placement of a 3×3
                // (`InitializeMicrosim` `+9`) fills that word before the first January.
                record.stat1 = pe_park_attendance(normal_population, record.stat2)
                    .min(u32::from(u16::MAX)) as u16;
                record.stat2 = park_tiles.min(u32::from(u16::MAX)) as u16;
                let footprints = u32::from(record.stat2) / 9;
                record.stat3 = pe_microsim_scaled_capacity(pe_capacity_pop, 0x78, footprints)
                    .min(u32::from(u16::MAX)) as u16;
                changed = true;
            }
        }
        if let Some(record) = microsims.get_mut(MICROSIM_SYSTEM_MARINA) {
            if record.tile_id == 248 {
                *lfsr = rng_schedule.lfsr_start_by_slot[MICROSIM_SYSTEM_MARINA]
                    .expect("active marina consumes the yearly LFSR");
                // Win95 marina case @ `0x44efd2`: `0x401cad(0x14)` then tiles×8,
                // `FUN_0044f7d0` ÷0x96.
                let funding_cap =
                    u32::from(lfsr.random_mod(0x14)).saturating_add(marina_tiles.saturating_mul(8));
                record.stat1 = pe_microsim_scaled_capacity(pe_capacity_pop, 0x96, funding_cap)
                    .min(u32::from(u16::MAX)) as u16;
                changed = true;
            }
        }
        if let Some(record) = microsims.get_mut(MICROSIM_SYSTEM_BUS) {
            if record.tile_id == 236 {
                // Win95 bus case @ `0x44ee19`: stations = tiles `sar 2`, buses = tiles,
                // passengers = `dwBusPassengers` WORD.
                record.stat1 = pe_transit_depot_count(bus_tiles);
                record.stat2 = bus_tiles.min(u32::from(u16::MAX)) as u16;
                record.stat3 = pe_microsim_passenger_word(bus_passengers);
                changed = true;
            }
        }
        if let Some(record) = microsims.get_mut(MICROSIM_SYSTEM_RAIL) {
            if record.tile_id == 237 {
                // Win95 rail case @ `0x44ee5f`: stations = tiles `sar 2`,
                // passengers = `dwRailPassengers` WORD (no stat2 rewrite).
                record.stat1 = pe_transit_depot_count(rail_tiles);
                record.stat3 = pe_microsim_passenger_word(rail_passengers);
                changed = true;
            }
        }
        if let Some(record) = microsims.get_mut(MICROSIM_SYSTEM_SUBWAY) {
            if record.tile_id == 233 {
                // Win95 subway case @ `0x44edf3`: stations = raw tiles,
                // passengers = `dwSubwayPassengers` WORD.
                record.stat1 = subway_tiles.min(u32::from(u16::MAX)) as u16;
                record.stat3 = pe_microsim_passenger_word(subway_passengers);
                changed = true;
            }
        }

        for &index in &mayor_indices {
            let Some(record) = microsims.get_mut(index) else {
                continue;
            };
            // Win95 mayor case @ `0x44ee8e`.
            record.stat2 = mayor_approval;
            if record.stat3 != 0 {
                record.stat3 = record.stat3.saturating_sub(1);
                // PE `inc byte ptr [ecx + edi + 1]` at `0x44eebf` wraps.
                record.stat0 = record.stat0.wrapping_add(1);
            }
            changed = true;
        }

        // The worker prologue divides the day-22 population DWORD directly.
        let pop_div_50 = normal_population / 50;
        for &index in water_indices.iter().chain(desal_indices.iter()) {
            *rng = rng_schedule.crt_start_by_slot[index]
                .expect("water microsim consumes the yearly CRT stream");
            let Some(record) = microsims.get_mut(index) else {
                continue;
            };
            // Win95 water/desal case @ `0x44eec4`.
            // PE `add al, cl` at `0x44eed1` updates a byte, so values above
            // 248 wrap when the 0..=7 jitter is added.
            record.stat0 = water_efficiency.wrapping_add(rng.random_u16_mask(7) as u8);
            record.stat1 = rng.random_mod(100);
            let capacity = u32::from(rng.random_u16_mask(0x1f))
                .saturating_add(0x87)
                .min(pop_div_50);
            record.stat2 = capacity.min(u32::from(u16::MAX)) as u16;
            changed = true;
        }

        for index in MICROSIM_STRUCTURE_FIRST..microsims.len() {
            if microsims[index].tile_id == 0 {
                continue;
            }
            // Keep the active row out of the vector while DestroyBuilding
            // clears XMIC resources, so those resource releases can mutate
            // any other row without aliasing this one.
            let mut record = std::mem::replace(&mut microsims[index], MicrosimRecord::default());
            if matches!(record.tile_id, 201..=207) {
                *rng = rng_schedule.crt_start_by_slot[index]
                    .expect("power-plant microsim consumes the yearly CRT stream");
                // Win95 power-plant case @ `0x44e541`.
                // Age>48 queues newspaper story `0x24`. Rebuild is gated on No Disasters
                // (`0x4CC4D4`); otherwise age>50 always DestroyStructure.
                // The PE uses `inc byte ptr [esi + 1]`, so 0xff wraps to zero.
                record.stat0 = record.stat0.wrapping_add(1);
                // PE adds the low WORD and jitter in AX, wrapping at 0xffff.
                record.stat2 = power_used_percent.wrapping_add(rng.random_u16_mask(7));
                if record.stat0 > 48 {
                    pending_power_aging_news.push(record.tile_id.saturating_sub(201));
                }
                if record.stat0 > 50 {
                    if let Some(anchor) = demolition_anchors[index] {
                        let cost = power_plant_rebuild_cost(record.tile_id) as i32;
                        if no_disasters && cost > 0 && city_funds >= cost {
                            city_funds -= cost;
                            record.stat0 = 0;
                        } else {
                            let tile_id = record.tile_id;
                            // Both the demolition release and the annual caller
                            // clear only the type byte, leaving the statistics.
                            record.tile_id = 0;
                            if let Some(site) = demolition_sites[index].as_ref() {
                                self.clear_map_structure_for_microsim_pe_plant(
                                    tile_id,
                                    anchor,
                                    &site.cells,
                                    rng,
                                    &mut microsims,
                                );
                            } else {
                                pending_power_demolish.push((index, tile_id, anchor));
                            }
                        }
                    }
                }
                changed = true;
            } else if matches!(record.tile_id, 251..=254) {
                *lfsr = rng_schedule.lfsr_start_by_slot[index]
                    .expect("arcology microsim consumes the yearly LFSR");
                // Win95 arco case @ `0x44f008` (launch 254 shares the body after a counter inc).
                let design = u32::from(record.stat1).saturating_mul(1_000);
                let prev = u32::from(record.stat2);
                // PE pushes `max(arcology_count, 1) * 20` as the divisor and
                // `design / 10` as the cap (`0x44f01f..0x44f037`).
                let divisor = arco_building_count.saturating_mul(20);
                let funding_cap = design / 10;
                let capacity = pe_microsim_scaled_capacity(pe_capacity_pop, divisor, funding_cap);
                let score = (60 - tax_r - tax_c - tax_i) / 6 + i32::from(record.stat0);
                let target = i64::from(score) * 5 - 50;
                let target = target * 5 * 8; // PE lea/shl chain
                let mut residents = i64::from(capacity);
                if target < residents {
                    residents = target;
                }
                residents = residents
                    .saturating_add(i64::from(prev / 50))
                    .saturating_add(i64::from(prev));
                if residents > i64::from(design) {
                    residents = i64::from(design);
                }
                // PE caps the signed running value before its final Game_Random
                // draw, then performs `add ax,bx` and stores the low word. The
                // jitter can therefore take the saved word above design
                // capacity, and negative intermediate values wrap at the word
                // store instead of being clamped to zero.
                let residents_word = (residents as u16).wrapping_add(lfsr.random_mod64());
                let residents_u = u32::from(residents_word);
                record.stat2 = residents_word;
                arco_residents_total = arco_residents_total.saturating_add(residents_u);
                changed = true;
            }
            microsims[index] = record;
        }

        // The category loops above preserve readable case-specific formulas,
        // while the original walks XMIC in slot order. Carry the stream state
        // forward as though each active record ran in that original order.
        *rng = rng_schedule.crt_after_records;
        *lfsr = rng_schedule.lfsr_after_records;
        self.microsims = Some(microsims);

        if changed {
            self.modified = true;
        }
        if city_funds != self.city_funds() {
            self.set_city_funds(city_funds);
        }
        for (_index, tile_id, anchor) in pending_power_demolish {
            // Malformed marker-to-nonplant locations follow a separate
            // DestroyBuilding selector route that has not been modeled.
            self.clear_map_structure_for_microsim(tile_id, anchor, rng);
        }
        for plant_param in pending_power_aging_news {
            self.queue_pe_newspaper_story(PE_NEWSPAPER_STORY_POWER_PLANT_AGING, plant_param);
        }
        if pending_fund_education_news || pending_low_school_score_news {
            self.queue_pe_newspaper_story(PE_NEWSPAPER_STORY_FUND_EDUCATION, 0);
        }
        if pending_prison_overcrowded_news {
            self.queue_pe_newspaper_story(PE_NEWSPAPER_STORY_PRISON_OVERCROWDED, 0);
        }
        // PE @ `0x44f26a`–`0x44f311`: after the yearly structure walk writes arco pop,
        // Launch (`0xfe`) footprints `dwTileCount/16 > 300` and arco pop `> 0x5b8d80`
        // → DestroyBuilding mode 1 on every XBLD `0xfe` tile.
        self.pe_maybe_mass_destroy_launch_arcos(arco_residents_total, rng);
        // PE police epilogue @ `0x44f0fd` writes the arrest accumulator to BSS `0x4ca430`.
        self.pe_w_old_arrests = new_old_arrests as u16;
        if self.kind != CityFileKind::MacCity {
            self.set_misc_u32_at(MISC_OLD_ARRESTS_OFFSET, new_old_arrests);
        }
        // PE prison finalize @ `0x44f125`: average inmates/100 → 1 if `< 0x50`, else 0.
        let prison_bonus = pe_prison_bonus_from_microsims(self);
        self.pe_w_prison_bonus = prison_bonus.min(u32::from(u16::MAX)) as u16;
        if self.kind != CityFileKind::MacCity {
            self.set_misc_u32_at(MISC_PRISON_BONUS_OFFSET, prison_bonus);
        }
        // PE `FUN_0044e390` clears all three dword passenger accumulators at
        // `0x44f10e..0x44f118`, after yearly transit rows consume them.
        self.reset_pe_transit_passenger_totals();
        arco_residents_total
    }
}

/// Win95 park attendance (`0x44e8e6`): `min(412×prev_stat2, dwCityPopulation/6, 0xfde8)`.
/// A 3×3 park's `InitializeMicrosim` `+9` is the first January's previous acres.
pub(super) fn pe_park_attendance(pop_load: u32, prev_stat2: u16) -> u32 {
    const CAP: u32 = 0xfde8;
    let scaled = u32::from(prev_stat2).saturating_mul(412).min(CAP);
    let pop_div6 = (pop_load / 6).min(CAP);
    scaled.min(pop_div6)
}

/// Win95 bus/rail depot count: `cdq; and 3; add; sar 2` on the tile WORD.
pub(super) fn pe_transit_depot_count(tiles: u32) -> u16 {
    pe_sar_div(tiles.min(i32::MAX as u32) as i32, 2).max(0) as u16
}

pub(super) fn pe_microsim_passenger_word(passengers: u32) -> u16 {
    // PE stores DX to the XMIC word, retaining the low 16 bits.
    passengers as u16
}

/// Win95 college professor `LEA`/`JNS` sequence at `0x44ed07..0x44ed21`.
/// Keep the active funding dword and each intermediate at x86 32-bit width;
/// a negative signed result is replaced with zero.
pub(super) fn pe_college_professors_raw(funding: u32, jitter: u32, prior_grade: u8) -> u32 {
    let grade_adjust = u32::from(prior_grade).wrapping_mul(4).wrapping_sub(0x30);
    let candidate = funding
        .wrapping_mul(2)
        .wrapping_add(jitter)
        .wrapping_add(grade_adjust);
    if (candidate as i32) >= 0 {
        candidate
    } else {
        0
    }
}

pub(super) fn pe_hospital_funding_half(funding: u32) -> u16 {
    let signed_funding = funding as i32;
    let sign_correction = if signed_funding < 0 { -1 } else { 0 };
    (signed_funding.wrapping_sub(sign_correction) >> 1) as u16
}

/// Win95 hospital doctor candidate: 32-bit additions, then `JNS` zero clamp.
pub(super) fn pe_hospital_doctors_raw(funding: u32, jitter: i32, grade_adjust: i32) -> u32 {
    let candidate = funding
        .wrapping_add(jitter as u32)
        .wrapping_add(grade_adjust as u32);
    if (candidate as i32) >= 0 {
        candidate
    } else {
        0
    }
}

/// Win95 school funding field at `0x44e983..0x44e98c`.
/// `CDQ; AND EDX,3; ADD EAX,EDX; SAR EAX,2; MOV [stat3],AX` implements signed
/// dword division by four with truncation toward zero, then stores its low word.
pub(super) fn pe_school_funding_quarters(funding: u32) -> u16 {
    let signed_funding = funding as i32;
    let rounded = signed_funding.wrapping_add(if signed_funding < 0 { 3 } else { 0 }) >> 2;
    rounded as u16
}

/// Win95 teacher candidate at `0x44ea19..0x44ea48`.
/// The funding dword is multiplied by six at 32-bit width, divided by ten as
/// a signed dword, then combined with the random jitter and prior grade before
/// the `JNS` zero clamp.
pub(super) fn pe_school_teachers_raw(funding: u32, jitter: i32, prior_grade: u8) -> u32 {
    let funding_term = (funding.wrapping_mul(6) as i32) / 10;
    let candidate = funding_term
        .wrapping_add(jitter)
        .wrapping_add(i32::from(prior_grade))
        .wrapping_sub(12);
    if candidate >= 0 { candidate as u32 } else { 0 }
}

/// Win95 school grade from students/teachers (`0x44ea6f`).
pub(super) fn pe_school_grade(students: u32, teachers: u32) -> u8 {
    let teachers = teachers.max(1);
    let ratio = students / teachers;
    if ratio < 15 {
        12
    } else if ratio > 51 {
        0
    } else {
        ((51 - ratio) / 3) as u8
    }
}

/// Win95 hospital/college grade bands from a scaled occupancy ratio (`0x44e72f` / `0x44ed54`).
pub(super) fn pe_hospital_style_grade(ratio: u32) -> u8 {
    if ratio < 50 {
        12
    } else if ratio > 110 {
        0
    } else {
        ((110 - ratio) / 5) as u8
    }
}

/// Win95 `wOldArrests` input to the prison case: sum of police XMIC `stat3` (arrests)
/// from the prior year-end pass (`0x4CA430` before this updater rewrites police rows).
pub(super) fn pe_old_arrests_from_microsims(city: &City) -> u32 {
    let mut total = 0_u32;
    for index in MICROSIM_STRUCTURE_FIRST..MICROSIM_COUNT {
        let Some(record) = city.microsim_at(index) else {
            continue;
        };
        if record.tile_id != 210 {
            continue;
        }
        total = total.saturating_add(u32::from(record.stat3));
        if total >= 0xffff {
            return 0xffff;
        }
    }
    total
}

/// Win95 prison grade span at `0x44ec3f..0x44ec5d`. The funding subtraction
/// wraps as a dword, is then interpreted signed by IDIV, and the resulting
/// signed quotient is added to `inmate_hundreds - 90` with dword wrap. The
/// caller uses this bit pattern as an unsigned DIV divisor.
pub(super) fn pe_prison_overcrowding_grade_span(inmate_hundreds: u32, police_funding: u32) -> u32 {
    let funding_delta = (100_u32.wrapping_sub(police_funding) as i32) / 10;
    (inmate_hundreds as i32)
        .wrapping_sub(90)
        .wrapping_add(funding_delta) as u32
}

/// Win95 `wPrisonBonus` (`0x4CA1DC`): 1 when a prison exists and average inmates/100 (`stat3`) < 0x50.
pub(super) fn pe_prison_bonus_from_microsims(city: &City) -> u32 {
    let mut count = 0_u32;
    let mut inmate_hundreds = 0_u32;
    for index in MICROSIM_STRUCTURE_FIRST..MICROSIM_COUNT {
        let Some(record) = city.microsim_at(index) else {
            continue;
        };
        if record.tile_id != 216 {
            continue;
        }
        count = count.saturating_add(1);
        inmate_hundreds = inmate_hundreds.saturating_add(u32::from(record.stat3));
    }
    if count == 0 {
        return 0;
    }
    u32::from(inmate_hundreds / count < 0x50)
}
