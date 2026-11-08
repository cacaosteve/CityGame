//! Mac structure-XMIC refresh helpers.
//!
//! Most methods here mirror CODE 3 `export_162` selector bodies. Despite the
//! historical `mac_monthly_*` Rust names, that selector sweep runs annually
//! in January; separate producers accumulate some of its inputs monthly.

use super::mac_monthly_ordered_rows::{
    mac_college_professors_raw, mac_education_student_candidate, mac_hospital_funding_half,
    mac_hospital_physicians_raw, mac_police_arrest_subtotal_after_row, mac_prison_guard_candidate,
    mac_prison_overcrowding_field, mac_school_funding_quarters, mac_school_teachers_raw,
    mac_update_library_record, mac_update_marina_record, mac_update_museum_record,
    mac_update_park_record,
};
use super::*;
use crate::mac_random::MacGameLcgRng;

impl City {
    /// Mac CODE 3 `export_162` hydro/wind selectors run during the annual
    /// January XMIC sweep. They read the saved XBLD census and replace the
    /// XMIC usage word with that count. Only already-active fixed system rows
    /// are updated, matching the Mac selector's saved-tile dispatch gate.
    pub fn refresh_mac_monthly_utility_microsims(&mut self) {
        let hydro_count = self
            .mac_xbld_tile_count(198)
            .wrapping_add(self.mac_xbld_tile_count(199));
        let wind_count = self.mac_xbld_tile_count(200);
        let Some(microsims) = self.microsims.as_mut() else {
            return;
        };

        if let Some(row) = microsims.get_mut(MICROSIM_SYSTEM_HYDRO)
            && matches!(row.tile_id, 198 | 199)
        {
            row.stat1 = hydro_count;
            row.stat2 = row.stat1.wrapping_mul(20);
            self.modified = true;
        }
        if let Some(row) = microsims.get_mut(MICROSIM_SYSTEM_WIND)
            && row.tile_id == 200
        {
            row.stat1 = wind_count;
            row.stat2 = row.stat1.wrapping_shl(2);
            self.modified = true;
        }
    }

    /// Mac CODE 3 `export_162` power-plant rows (`0xc9..=0xcf`) run before
    /// later randomized XMIC selectors in the slot walk. Each active row
    /// advances its age byte and consumes one shared QuickDraw value for the
    /// `fffff00c + (Random & 7)` WORD at XMIC `+4`.
    ///
    /// This selector helper covers the age byte and power WORD. Runtime uses
    /// the ordered XMIC dispatcher, which also applies marker-gated renewal
    /// and the rooted age-out map-edit route; the age-over-48 event remains a
    /// separate presentation path.
    pub fn refresh_mac_monthly_power_plant_microsims(&mut self, rng: &mut MacToolboxRng) {
        let power_summary_low_word = self.mac_power_service_summary() as u16;
        let Some(microsims) = self.microsims.as_mut() else {
            return;
        };
        let mut changed = false;
        for record in microsims
            .iter_mut()
            .filter(|record| (0xc9..=0xcf).contains(&record.tile_id))
        {
            record.stat0 = record.stat0.wrapping_add(1);
            record.stat2 = power_summary_low_word.wrapping_add(rng.random_u16() & 7);
            changed = true;
        }
        if changed {
            self.modified = true;
        }
    }

    /// Mac CODE 3 `export_162` hospital rows (`0xd1`). This follows the plant
    /// family in the native XMIC slot walk and shares its QuickDraw stream.
    /// `population_scalar` is the opaque persisted `fffff01a` scan result;
    /// it is not treated as PE city population here.
    pub fn refresh_mac_monthly_hospital_microsims(
        &mut self,
        health_funding: u32,
        rng: &mut MacToolboxRng,
    ) {
        let hospital_tiles = self.mac_xbld_tile_count(0xd1);
        let population_scalar = self.mac_state_misc_u32_at(0x102c).unwrap_or(0);
        let raw_divisor = ((hospital_tiles / 9).wrapping_mul(25)) as i16;
        let patient_divisor = if raw_divisor < 1 {
            1
        } else {
            raw_divisor as u16
        };

        let Some(microsims) = self.microsims.as_mut() else {
            return;
        };
        let mut changed = false;
        for record in microsims.iter_mut().filter(|record| record.tile_id == 0xd1) {
            record.stat3 = mac_hospital_funding_half(health_funding);

            let patient_jitter = u32::from(rng.random_u16() & 0x0f);
            let first_patient_candidate =
                (population_scalar / u32::from(patient_divisor)).wrapping_add(patient_jitter);
            // CMPI.L / BLE treats the wrapped quotient-plus-jitter as signed.
            let patient_candidate = if (first_patient_candidate as i32) > 1_000 {
                1_000 + u32::from(rng.random_u16() & 0x7f)
            } else {
                first_patient_candidate
            } as u16;
            let patient_cap = (population_scalar / 30) as i16;
            let patients = if patient_cap < patient_candidate as i16 {
                patient_cap as u16
            } else {
                patient_candidate
            };
            record.stat1 = patients;

            let physician_jitter = rng.random_u16() & 7;
            let physician_candidate =
                mac_hospital_physicians_raw(health_funding, physician_jitter, record.stat0) as u16;
            let physician_cap = (population_scalar / 120) as i16;
            let physicians = if physician_cap < physician_candidate as i16 {
                physician_cap as u16
            } else {
                physician_candidate
            };
            record.stat2 = physicians;

            let grade_divisor = i32::from(physicians as i16).max(1);
            let ratio = (i32::from(patients as i16) * 10) / grade_divisor;
            record.stat0 = if ratio < 50 {
                12
            } else if ratio < 111 {
                ((110 - ratio) / 5) as u8
            } else {
                0
            };
            changed = true;
        }
        if changed {
            self.modified = true;
        }
    }

    /// Mac CODE 3 `export_162` police/fire XMIC cases (`0xd2`/`0xd3`).
    /// Police reads the previous pass's saved `fffffe00` threshold and the
    /// saved XCRM aggregate, then publishes the phase-local arrest sum for
    /// the later prison selector (`0xd8`) to consume in slot order.
    pub fn refresh_mac_monthly_police_fire_microsims(
        &mut self,
        police_funding: u32,
        fire_funding: u32,
        rng: &mut MacToolboxRng,
    ) {
        let police_tiles = self.mac_xbld_tile_count(0xd2);
        // Mac `ffffeffe` is saved at MISC byte offset 0x2c and refreshed by
        // the prior CODE 14 phase-2 XCRM pass; preserve that phase snapshot.
        let crime_total = self.mac_state_misc_u32_at(0x002c).unwrap_or(0);
        let police_tile_count = u32::from(police_tiles).max(1);
        let prior_police_gate = self.mac_police_coverage_gate();
        let arrests_per_police = crime_total / police_tile_count;
        let arrest_divisor = (5_i16.wrapping_sub(prior_police_gate)).max(1) as u32;
        let police_capacity = ((self.mac_state_misc_u32_at(0x102c).unwrap_or(0) / 90) as i16)
            .min((police_funding as u16).wrapping_mul(2) as i16)
            as u16;
        // CODE 3 loads the funding dword, ASR.Ls it, then passes D0.w to
        // FUN_0001238a. Narrow only after the 32-bit arithmetic shift.
        let fire_funding_half_word = ((fire_funding as i32) >> 1) as i16;
        let fire_capacity = ((self.mac_state_misc_u32_at(0x102c).unwrap_or(0) / 70) as i16)
            .min(fire_funding_half_word) as u16;

        let Some(microsims) = self.microsims.as_mut() else {
            self.mac_monthly_arrest_total = 0;
            return;
        };
        let mut arrest_total = 0_u16;
        let mut changed = false;
        for record in microsims.iter_mut() {
            match record.tile_id {
                0xd2 => {
                    record.stat0 = police_funding as u8;
                    record.stat1 = police_capacity;
                    record.stat2 = (arrests_per_police as i16) as u16;
                    let per_site = ((arrests_per_police as i32) / arrest_divisor as i32)
                        .wrapping_add(i32::from(rng.random_u16() & 0x0f))
                        as u32;
                    record.stat3 = (per_site as i16) as u16;
                    arrest_total = mac_police_arrest_subtotal_after_row(arrest_total, per_site);
                    changed = true;
                }
                0xd3 => {
                    record.stat0 = fire_funding as u8;
                    record.stat1 = fire_capacity;
                    record.stat2 = (fire_capacity >> 4).wrapping_add(1);
                    record.stat3 = (rng.random_u16() % 20).wrapping_add(2);
                    changed = true;
                }
                _ => {}
            }
        }
        self.mac_monthly_arrest_total = arrest_total;
        if changed {
            self.modified = true;
        }
    }

    /// Mac CODE 3 `export_162` stadium (`0xd7`) XMIC row. It uses the
    /// previous month's weighted growth scalar and the saved XBLD tile census;
    /// retain the Mac monthly inputs separately from the PE annual formula.
    pub fn refresh_mac_monthly_stadium_microsims(&mut self, rng: &mut MacToolboxRng) {
        let stadium_tiles = self.mac_xbld_tile_count(0xd7).max(1);
        let growth_scalar = self.mac_state_misc_u32_at(0x102c).unwrap_or(0);
        let Some(microsims) = self.microsims.as_mut() else {
            return;
        };
        let mut changed = false;
        for record in microsims.iter_mut().filter(|record| record.tile_id == 0xd7) {
            let mut candidate = growth_scalar / u32::from(stadium_tiles);
            if (candidate as i32) > 25_000 {
                candidate = 25_000_u32.wrapping_sub(u32::from(rng.random_u16() & 0xff));
            }

            let candidate_word = candidate as u16 as i16;
            let capacity = ((growth_scalar / 5) as i16).min(candidate_word) as u16;
            record.stat1 = capacity.wrapping_add(rng.random_u16() & 0xff);
            record.stat0 = ((rng.random_u16() & 0x1f) + 9) as u8;
            changed = true;
        }
        if changed {
            self.modified = true;
        }
    }

    /// Mac CODE 3 `export_162` school (`0xd6`) XMIC row. The persisted
    /// `fffff062` entries remain opaque counters; MISC stores them interleaved
    /// with two companion vectors, so entries 1 and 2 live at 0x88 and 0x94.
    pub fn refresh_mac_monthly_school_microsims(
        &mut self,
        school_funding: u32,
        rng: &mut MacToolboxRng,
    ) {
        let school_tiles = self.mac_xbld_tile_count(0xd6);
        let growth_scalar = self.mac_state_misc_u32_at(0x102c).unwrap_or(0);
        let school_counter_1 = self.mac_state_misc_u32_at(0x0088).unwrap_or(0);
        let school_counter_2 = self.mac_state_misc_u32_at(0x0094).unwrap_or(0);
        let counter_sum = school_counter_1.wrapping_add(school_counter_2);
        let raw_denominator = ((school_tiles / 9) as i16).max(1);

        let Some(microsims) = self.microsims.as_mut() else {
            return;
        };
        let mut changed = false;
        for record in microsims.iter_mut().filter(|record| record.tile_id == 0xd6) {
            let funding_word = mac_school_funding_quarters(school_funding);
            record.stat3 = funding_word;

            let student_candidate = mac_education_student_candidate(
                counter_sum,
                u32::from(raw_denominator as u16),
                rng,
                0x0f,
                1_500,
                1_500,
                0xff,
            );
            let student_cap = (growth_scalar / 20) as i16;
            let students = student_candidate.min(student_cap) as u16;
            record.stat1 = students;

            let teacher_candidate =
                mac_school_teachers_raw(school_funding, rng.random_u16() & 3, record.stat0);
            let teacher_cap = (growth_scalar / 100) as i16;
            let teachers = teacher_candidate.min(teacher_cap) as u16;
            record.stat2 = teachers;

            let grade_divisor = i32::from((teachers as i16).max(1));
            let ratio = (i32::from(students as i16) * 4) / grade_divisor;
            record.stat0 = if ratio < 50 {
                12
            } else if ratio < 111 {
                ((110 - ratio) / 5) as u8
            } else {
                0
            };
            changed = true;
        }
        if changed {
            self.modified = true;
        }
    }

    /// Mac CODE 3 prison-like selector (`0xd8`). Consumes the current
    /// phase's police subtotal and publishes the one-bit gate for the next
    /// monthly police pass. The PE annual prison path remains separate.
    pub fn refresh_mac_monthly_prison_microsims(
        &mut self,
        police_funding: u32,
        rng: &mut MacToolboxRng,
    ) {
        let raw_prison_divisor = (self.mac_xbld_tile_count(0xd8) >> 4) as i16;
        let inmate_divisor = if raw_prison_divisor == 0 {
            1
        } else {
            raw_prison_divisor
        };
        let arrest_total = u32::from(self.mac_monthly_arrest_total);
        let growth_scalar = self.mac_state_misc_u32_at(0x102c).unwrap_or(0);
        let guard_candidate = mac_prison_guard_candidate(police_funding);
        let guard_capacity = ((growth_scalar / 120) as i16).min(guard_candidate) as u16;
        let Some(microsims) = self.microsims.as_mut() else {
            self.mac_monthly_arrest_total = 0;
            self.set_mac_police_coverage_gate(0);
            return;
        };

        let mut accumulated_gate = 0_i16;
        let mut changed = false;
        for record in microsims.iter_mut().filter(|record| record.tile_id == 0xd8) {
            let prior_inmates = u32::from(record.stat1);
            let raw_candidate =
                arrest_total / inmate_divisor as u32 + (prior_inmates - prior_inmates / 4);
            let mut inmate_candidate = raw_candidate;
            if raw_candidate > 10_000 {
                inmate_candidate = 10_000 + u32::from(rng.random_u16() & 0x03ff);
            }

            let inmate_candidate_word = inmate_candidate as u16 as i16;
            let inmate_capacity = (growth_scalar / 20) as i16;
            record.stat1 = inmate_candidate_word.min(inmate_capacity) as u16;
            record.stat2 = guard_capacity;

            let inmates_per_hundred = (inmate_candidate / 100) as i16;
            record.stat3 = inmates_per_hundred as u16;
            accumulated_gate = accumulated_gate.wrapping_add(inmates_per_hundred);
            if inmates_per_hundred < 91 {
                record.stat0 = 0;
            } else {
                record.stat0 = mac_prison_overcrowding_field(
                    inmates_per_hundred,
                    police_funding,
                    rng.random_u16(),
                );
            }
            changed = true;
        }

        let gate = if raw_prison_divisor > 0 && accumulated_gate / raw_prison_divisor < 80 {
            1
        } else {
            0
        };
        self.mac_monthly_arrest_total = 0;
        self.set_mac_police_coverage_gate(gate as i16);
        if changed {
            self.modified = true;
        }
    }

    /// Mac CODE 3 college selector (`0xd9`). Its numerator is the opaque
    /// saved `fffff062[3]` counter at MISC byte 0xa0; this remains distinct
    /// from PE population and PE EQ/LE inputs.
    pub fn refresh_mac_monthly_college_microsims(
        &mut self,
        college_funding: u32,
        rng: &mut MacToolboxRng,
    ) {
        let college_tiles = self.mac_xbld_tile_count(0xd9);
        let raw_divisor = ((college_tiles >> 4) as i16).max(1);
        let growth_scalar = self.mac_state_misc_u32_at(0x102c).unwrap_or(0);
        let college_counter = self.mac_state_misc_u32_at(0x00a0).unwrap_or(0);
        let Some(microsims) = self.microsims.as_mut() else {
            return;
        };
        let mut changed = false;
        for record in microsims.iter_mut().filter(|record| record.tile_id == 0xd9) {
            record.stat3 = college_funding as u16;

            let student_candidate = mac_education_student_candidate(
                college_counter,
                u32::from(raw_divisor as u16),
                rng,
                0x1f,
                5_000,
                5_000,
                0x01ff,
            );
            let students = student_candidate.min((growth_scalar / 30) as i16) as u16;
            record.stat1 = students;

            let professor_candidate =
                mac_college_professors_raw(college_funding, rng.random_u16() & 0x0f, record.stat0);
            let professors = professor_candidate.min((growth_scalar / 100) as i16) as u16;
            record.stat2 = professors;

            let ratio = (i32::from(students as i16) * 4) / i32::from((professors as i16).max(1));
            record.stat0 = if ratio < 50 {
                12
            } else if ratio < 111 {
                ((110 - ratio) / 5) as u8
            } else {
                0
            };
            changed = true;
        }
        if changed {
            self.modified = true;
        }
    }

    /// Mac CODE 3 Statue (`0xdb`) writes only record `+4`, using one unsigned
    /// QuickDraw word modulo 42. The Win95 annual path retains its CRT stream.
    pub fn refresh_mac_monthly_statue_microsims(&mut self, rng: &mut MacToolboxRng) {
        let Some(microsims) = self.microsims.as_mut() else {
            return;
        };
        let mut changed = false;
        for record in microsims.iter_mut().filter(|record| record.tile_id == 0xdb) {
            record.stat2 = rng.random_u16() % 42;
            changed = true;
        }
        if changed {
            self.modified = true;
        }
    }

    /// Mac CODE 3 water-treatment/desalination selectors (`0xf4`, `0xfa`).
    /// Both share three QuickDraw draws per active row and read the previous
    /// phase-21 water percentage and demand-scan capacity scalar.
    pub fn refresh_mac_monthly_water_microsims(&mut self, rng: &mut MacToolboxRng) {
        let water_percent = self.mac_water_service_summary() as u8;
        let growth_scalar = self.mac_state_misc_u32_at(0x102c).unwrap_or(0);
        let Some(microsims) = self.microsims.as_mut() else {
            return;
        };
        let mut changed = false;
        for record in microsims
            .iter_mut()
            .filter(|record| matches!(record.tile_id, 0xf4 | 0xfa))
        {
            record.stat0 = water_percent.wrapping_add((rng.random_u16() & 7) as u8);
            record.stat1 = rng.random_u16() % 100;
            let capacity_candidate = 0x87_u32 + u32::from(rng.random_u16() & 0x1f);
            record.stat2 = capacity_candidate.min(growth_scalar / 50) as u16;
            changed = true;
        }
        if changed {
            self.modified = true;
        }
    }

    /// Mac CODE 3 Braun Llama Dome (`0xff`) writes all four statistic fields
    /// from four ordered QuickDraw draws and the prior demand-scan scalar.
    pub fn refresh_mac_monthly_llama_dome_microsims(&mut self, rng: &mut MacToolboxRng) {
        let growth_scalar = self.mac_state_misc_u32_at(0x102c).unwrap_or(0);
        let Some(microsims) = self.microsims.as_mut() else {
            return;
        };
        let mut changed = false;
        for record in microsims.iter_mut().filter(|record| record.tile_id == 0xff) {
            record.stat0 = rng.random_u16() as u8;
            let value = u32::from(rng.random_u16() & 0x03ff) + (growth_scalar >> 3);
            record.stat1 = value as u16;
            record.stat2 = (u32::from(rng.random_u16() & 0x007f) + value / 8) as u16;
            record.stat3 = (u32::from(rng.random_u16() & 0x003f) + value / 16) as u16;
            changed = true;
        }
        if changed {
            self.modified = true;
        }
    }

    /// Mac CODE 3 zoo (`0xda`) updates four XMIC words from the app-owned
    /// CODE 13 game-LCG, separately from QuickDraw `_Random`.
    pub fn refresh_mac_monthly_zoo_microsims(&mut self, rng: &mut MacGameLcgRng) {
        for slot in 1..MICROSIM_COUNT {
            self.refresh_mac_monthly_zoo_microsim_at(slot, rng);
        }
    }

    pub(crate) fn refresh_mac_monthly_zoo_microsim_at(
        &mut self,
        slot: usize,
        rng: &mut MacGameLcgRng,
    ) {
        let Some(record) = self.microsims.as_mut().and_then(|rows| rows.get_mut(slot)) else {
            return;
        };
        if record.tile_id != 0xda {
            return;
        }
        record.stat0 = rng.bounded(100) as u8;
        record.stat1 = rng.bounded(100);
        record.stat2 = rng.bounded(100);
        record.stat3 = rng.bounded(100);
        self.modified = true;
    }

    /// Mac CODE 3 arcology rows (`0xfb..=0xfe`) run in the annual January
    /// XMIC sweep. Use the saved XBLD census and water-worker result from that
    /// phase-0 pass.
    pub fn refresh_mac_monthly_arcology_microsims(
        &mut self,
        tax_rates: [u32; 3],
        water_percent: u32,
    ) {
        let count_snapshot = self.mac_arcology_count_snapshot();
        for index in 1..MICROSIM_COUNT {
            self.refresh_mac_monthly_arcology_microsim_at(
                index,
                tax_rates,
                water_percent,
                count_snapshot,
            );
        }
        self.republish_mac_arcology_resident_total();
    }

    /// CODE 3 snapshots the four census WORDs with wrapping additions before
    /// entering the XMIC walk, then logically shifts the WORD by four.
    pub(crate) fn mac_arcology_count_snapshot(&self) -> u16 {
        [0xfb, 0xfc, 0xfd, 0xfe]
            .into_iter()
            .fold(0_u16, |sum, tile| {
                sum.wrapping_add(self.mac_xbld_tile_count(tile))
            })
            >> 4
    }

    /// Update one native arcology slot using the pre-walk census snapshot.
    pub(crate) fn refresh_mac_monthly_arcology_microsim_at(
        &mut self,
        index: usize,
        tax_rates: [u32; 3],
        water_percent: u32,
        count_snapshot: u16,
    ) {
        if index == 0 {
            return;
        }
        let Some(record) = self.microsim_at(index) else {
            return;
        };
        if !(0xfb..=0xfe).contains(&record.tile_id) {
            return;
        }
        let growth = self.mac_state_misc_u32_at(0x102c).unwrap_or(0);
        let divisor_word = count_snapshot.max(1).wrapping_mul(20);
        let divisor = if divisor_word == 0 {
            100_u32
        } else {
            i32::from(divisor_word as i16) as u32
        };
        let cap_param = u32::from(record.stat1);
        let helper_candidate = (cap_param * 100) as u16 as i16;
        let helper_cap = (growth / divisor) as u16 as i16;
        let scaled_capacity = i32::from(helper_candidate.min(helper_cap));
        let tax_delta = 60_u32
            .wrapping_sub(tax_rates[0])
            .wrapping_sub(tax_rates[1])
            .wrapping_sub(tax_rates[2]) as i32;
        let score = i32::from(record.stat0).wrapping_add(tax_delta / 6);
        let target = score
            .wrapping_mul(200)
            .wrapping_sub(2_000)
            .min(scaled_capacity);
        let residents = i32::from(record.stat2);
        let candidate = target
            .wrapping_add(residents / 50)
            .wrapping_add(residents)
            .min((cap_param * 1_000) as i32);
        self.set_microsim_stat2(index, candidate.wrapping_add(water_percent as i32) as u16);
    }

    pub fn has_mac_monthly_arcology_microsims(&self) -> bool {
        self.microsims.as_ref().is_some_and(|rows| {
            rows.iter()
                .skip(1)
                .any(|record| (0xfb..=0xfe).contains(&record.tile_id))
        })
    }

    pub fn republish_mac_arcology_resident_total(&mut self) {
        let total = self.mac_arcology_rows_resident_total();
        self.set_mac_arcology_resident_total(total);
    }

    fn mac_arcology_rows_resident_total(&self) -> u32 {
        self.microsims
            .as_ref()
            .into_iter()
            .flatten()
            .skip(1)
            .filter(|record| (0xfb..=0xfe).contains(&record.tile_id))
            .fold(0_u32, |sum, record| {
                sum.wrapping_add(u32::from(record.stat2))
            })
    }

    pub(crate) fn mac_arcology_resident_total(&self) -> u32 {
        if self.kind == CityFileKind::MacCity {
            self.mac_state_misc_u32_at(1032 * MISC_FIELD_LEN)
                .unwrap_or(0)
        } else {
            self.mac_arcology_total_runtime
                .or_else(|| self.misc_u32_at(MISC_ARCOLOGY_POPULATION_OFFSET))
                .unwrap_or_else(|| self.mac_arcology_rows_resident_total())
        }
    }

    pub(crate) fn set_mac_arcology_resident_total(&mut self, total: u32) {
        if self.kind == CityFileKind::MacCity {
            self.set_mac_state_misc_u32_at(1032 * MISC_FIELD_LEN, total);
        } else {
            self.mac_arcology_total_runtime = Some(total);
            // CODE 4 saves Mac `fffffe0a` at slot 1032 / MISC +0x1020,
            // the same saved role as PE `dwArcologyPopulation`. Keep the
            // selected engine's result in the container's canonical field
            // so switching fidelity after reload sees the latest total.
            self.set_misc_u32_at(MISC_ARCOLOGY_POPULATION_OFFSET, total);
        }
    }

    /// Current CODE 3 police arrest subtotal for the Mac prison XMIC row later
    /// in the same annual January slot sweep.
    pub fn mac_monthly_arrest_total(&self) -> u16 {
        self.mac_monthly_arrest_total
    }

    /// Mac CODE 3 `export_162` library (`0xf5`) XMIC case. This deterministic
    /// selector runs in the annual January sweep. Its census input is the
    /// saved XBLD `0xf5` word; `fffff01a` remains the separately persisted
    /// monthly growth scalar. Funding is the active-document school funding
    /// value supplied by the caller.
    pub fn refresh_mac_monthly_library_microsims(&mut self, school_funding: u32) {
        let library_tiles = self.mac_xbld_tile_count(0xf5);
        let population_scalar = self.mac_state_misc_u32_at(0x102c).unwrap_or(0);
        let Some(microsims) = self.microsims.as_mut() else {
            return;
        };
        let mut changed = false;
        for record in microsims.iter_mut().filter(|record| record.tile_id == 0xf5) {
            mac_update_library_record(record, library_tiles, population_scalar, school_funding);
            changed = true;
        }
        if changed {
            self.modified = true;
        }
    }

    /// Mac CODE 3 `export_162` City Hall (`0xd0`) XMIC case. The annual
    /// worker stores `min(200, ffff_f01a / 900)` in `stat1` and leaves every
    /// other field untouched. The helper compares the narrowed signed words.
    pub fn refresh_mac_monthly_city_hall_microsims(&mut self) {
        let population_scalar = self.mac_state_misc_u32_at(0x102c).unwrap_or(0);
        let candidate = 200_i16;
        let cap = ((population_scalar / 900) as i16).min(candidate) as u16;
        let Some(microsims) = self.microsims.as_mut() else {
            return;
        };
        let mut changed = false;
        for record in microsims.iter_mut().filter(|record| record.tile_id == 0xd0) {
            record.stat1 = cap;
            changed = true;
        }
        if changed {
            self.modified = true;
        }
    }

    /// Mac CODE 3 `export_162` marina (`0xf8`) XMIC case. Its relocated
    /// CODE 13 export 203 draw is `MacAppLfsrRng`, whose verified Mac 1.2
    /// initial A5-2 state is zero and has no application-side writer; therefore
    /// the native `random_mod_20` contribution remains zero. Only `stat1`
    /// (`+2`) is written for active marina rows.
    pub fn refresh_mac_monthly_marina_microsims(&mut self) {
        let marina_tiles = self.mac_xbld_tile_count(0xf8);
        let population_scalar = self.mac_state_misc_u32_at(0x102c).unwrap_or(0);
        let Some(microsims) = self.microsims.as_mut() else {
            return;
        };
        let mut changed = false;
        for record in microsims.iter_mut().filter(|record| record.tile_id == 0xf8) {
            mac_update_marina_record(record, marina_tiles, population_scalar);
            changed = true;
        }
        if changed {
            self.modified = true;
        }
    }

    /// Mac CODE 3 `export_162` transit XMIC cases. The tile census comes from
    /// the saved PlaceTile-maintained XBLD words. Passenger accumulators are
    /// the three Mac MISC dwords at indices 1045..=1047; each row stores their low
    /// word and the annual XMIC worker clears all three dwords after the sweep.
    pub fn refresh_mac_monthly_transit_microsims(&mut self) {
        let bus_tiles = self.mac_xbld_tile_count(0xec);
        let rail_tiles = self.mac_xbld_tile_count(0xed);
        let subway_tiles = self.mac_xbld_tile_count(0xe9);
        let bus_passengers = self.mac_transit_accumulator(0) as u16;
        let rail_passengers = self.mac_transit_accumulator(1) as u16;
        let subway_passengers = self.mac_transit_accumulator(2) as u16;

        if let Some(microsims) = self.microsims.as_mut() {
            let mut changed = false;
            for record in microsims.iter_mut() {
                match record.tile_id {
                    0xe9 => {
                        record.stat1 = subway_tiles;
                        record.stat3 = subway_passengers;
                        changed = true;
                    }
                    0xec => {
                        record.stat1 = bus_tiles >> 2;
                        record.stat2 = bus_tiles;
                        record.stat3 = bus_passengers;
                        changed = true;
                    }
                    0xed => {
                        record.stat1 = rail_tiles >> 2;
                        record.stat3 = rail_passengers;
                        changed = true;
                    }
                    _ => {}
                }
            }
            if changed {
                self.modified = true;
            }
        }

        // The original clears the three long accumulators after the XMIC walk,
        // including when no active row of a particular transit kind exists.
        for mode in 0..3 {
            self.set_mac_transit_accumulator(mode, 0);
        }
    }

    /// Mac CODE 3 `export_162` museum (`0xd4`) XMIC case. The saved Mac
    /// XBLD census stores one expanded MISC dword per tile ID; the runtime
    /// worker reads the matching WORD at `fffff07a + 2*0xd4`.
    pub fn refresh_mac_monthly_museum_microsims(&mut self, college_funding: u32) {
        let museum_tiles = self.mac_xbld_tile_count(0xd4);
        let growth = self.mac_state_misc_u32_at(0x102c).unwrap_or(0);
        let Some(microsims) = self.microsims.as_mut() else {
            return;
        };
        let mut changed = false;
        for record in microsims.iter_mut().filter(|record| record.tile_id == 0xd4) {
            mac_update_museum_record(record, museum_tiles, growth, college_funding);
            changed = true;
        }
        if changed {
            self.modified = true;
        }
    }

    /// Mac CODE 3 `export_162` large-park (`0xd5`) XMIC case. Its two input
    /// words are the saved XBLD counters for small parks (`0x0d`) and large
    /// parks (`0xd5`); the prior attendance statistic is intentionally stateful.
    pub fn refresh_mac_monthly_park_microsims(&mut self) {
        let small_parks = self.mac_xbld_tile_count(0x0d);
        let large_parks = self.mac_xbld_tile_count(0xd5);
        let population_scalar = self.mac_state_misc_u32_at(0x102c).unwrap_or(0);
        let Some(microsims) = self.microsims.as_mut() else {
            return;
        };
        let mut changed = false;
        for record in microsims.iter_mut().filter(|record| record.tile_id == 0xd5) {
            mac_update_park_record(record, small_parks, large_parks, population_scalar);
            changed = true;
        }
        if changed {
            self.modified = true;
        }
    }
}
