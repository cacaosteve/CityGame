//! Mac CODE 3 slot-ordered XMIC worker and its service formulas.
//!
//! Runtime invokes this worker during the annual January sweep. The public
//! `mac_monthly_*` method names are historical; several inputs are accumulated
//! by distinct monthly producers before this ordered pass consumes them.

use super::*;

impl City {
    /// Run the QuickDraw-consuming Mac CODE 3 XMIC arms in native record-slot
    /// order. Runtime calls this in the annual January pass. The selector
    /// helpers above remain available for focused formula regressions; runtime
    /// dispatch must not group rows by class because all of these arms share
    /// the process QuickDraw stream.
    pub fn refresh_mac_monthly_random_xmic_rows(
        &mut self,
        police_funding: u32,
        fire_funding: u32,
        health_funding: u32,
        school_funding: u32,
        college_funding: u32,
        no_disasters: bool,
        rng: &mut MacToolboxRng,
    ) {
        self.refresh_mac_monthly_random_xmic_rows_with_external_slots(
            police_funding,
            fire_funding,
            health_funding,
            school_funding,
            college_funding,
            no_disasters,
            rng,
            |_, _| {},
        );
    }

    pub(crate) fn refresh_mac_monthly_random_xmic_rows_with_external_slots(
        &mut self,
        police_funding: u32,
        fire_funding: u32,
        health_funding: u32,
        school_funding: u32,
        college_funding: u32,
        no_disasters: bool,
        rng: &mut MacToolboxRng,
        mut external_slot: impl FnMut(&mut City, usize),
    ) {
        let plant_power = self.mac_power_service_summary() as u16;
        let hospital_tiles = self.mac_xbld_tile_count(0xd1);
        let school_tiles = self.mac_xbld_tile_count(0xd6);
        let prison_tiles = self.mac_xbld_tile_count(0xd8);
        let college_tiles = self.mac_xbld_tile_count(0xd9);
        let crime_total = self.mac_state_misc_u32_at(0x002c).unwrap_or(0);
        let prior_police_gate = self.mac_police_coverage_gate();
        let growth_scalar = self.mac_state_misc_u32_at(0x102c).unwrap_or(0);
        let school_counter = self
            .mac_state_misc_u32_at(0x0088)
            .unwrap_or(0)
            .wrapping_add(self.mac_state_misc_u32_at(0x0094).unwrap_or(0));
        let college_counter = self.mac_state_misc_u32_at(0x00a0).unwrap_or(0);
        let mut funds = self.city_funds();

        let fire_funding_half_word = ((fire_funding as i32) >> 1) as i16;
        let school_funding_word = mac_school_funding_quarters(school_funding);
        let hospital_divisor = {
            let divisor = ((hospital_tiles / 9).wrapping_mul(25)) as i16;
            if divisor < 1 { 1 } else { divisor as u16 }
        };
        let arrest_divisor = (5_i16.wrapping_sub(prior_police_gate)).max(1) as u32;
        let police_capacity = ((growth_scalar / 90) as i16)
            .min((police_funding as u16).wrapping_mul(2) as i16)
            as u16;
        let fire_capacity = ((growth_scalar / 70) as i16).min(fire_funding_half_word) as u16;
        let prison_raw_divisor = (prison_tiles >> 4) as i16;
        let prison_divisor = if prison_raw_divisor == 0 {
            1
        } else {
            prison_raw_divisor
        };
        let guard_capacity =
            ((growth_scalar / 120) as i16).min(mac_prison_guard_candidate(police_funding)) as u16;
        let college_divisor = ((college_tiles >> 4) as i16).max(1);
        let school_divisor = ((school_tiles / 9) as i16).max(1);

        let Some(record_count) = self.microsims.as_ref().map(Vec::len) else {
            self.mac_monthly_arrest_total = 0;
            self.persist_mac_police_arrest_subtotal();
            self.clear_mac_monthly_transit_accumulators();
            return;
        };
        // Native CODE 3 clears a local subtotal for this pass. D8 reads the
        // prior value in global fffffe04; D2 rows publish this pass's subtotal
        // only after the slot walk finishes.
        let incoming_arrest_total = self.mac_monthly_arrest_total;
        let mut arrest_total = 0_u16;
        let mut prison_gate_sum = 0_i16;
        let mut changed = false;

        // Keep the live table installed: export-80/XTXT cleanup can clear
        // earlier or later rows while the ordered native slot walk is active.
        // CODE 3 starts D7 at 1 and stops before 150; slot zero is unused.
        for slot in 1..record_count {
            let mut record = self.microsims.as_ref().expect("live XMIC table")[slot];
            match record.tile_id {
                0xc6 | 0xc7 => {
                    let count = self
                        .mac_xbld_tile_count(0xc6)
                        .wrapping_add(self.mac_xbld_tile_count(0xc7));
                    record.stat1 = count;
                    record.stat2 = count.wrapping_mul(20);
                    changed = true;
                }
                0xc8 => {
                    let count = self.mac_xbld_tile_count(0xc8);
                    record.stat1 = count;
                    record.stat2 = count.wrapping_shl(2);
                    changed = true;
                }
                0xc9..=0xcf => {
                    record.stat0 = record.stat0.wrapping_add(1);
                    record.stat2 = plant_power.wrapping_add(rng.random_u16() & 7);
                    if record.stat0 > 48 {
                        self.mac_poll_for_extra(|| rng.random_u16());
                    }
                    if record.stat0 > 50
                        && let Some(marker) = self.mac_find_xmic_marker(slot)
                    {
                        let cost = mac_power_plant_rebuild_cost(record.tile_id);
                        if no_disasters && funds >= cost {
                            funds -= cost;
                            record.stat0 = 0;
                        } else {
                            self.clear_mac_aged_power_plant(marker, rng);
                            record.tile_id = 0;
                        }
                    }
                    changed = true;
                }
                0xd0 => {
                    record.stat1 = ((growth_scalar / 900) as i16).min(200) as u16;
                    changed = true;
                }
                0xd1 => {
                    record.stat3 = mac_hospital_funding_half(health_funding);
                    let first_candidate = (growth_scalar / u32::from(hospital_divisor))
                        .wrapping_add(u32::from(rng.random_u16() & 0x0f));
                    // CODE 3 follows the longword add with a signed BLE.
                    let patient_candidate = if (first_candidate as i32) > 1_000 {
                        1_000 + u32::from(rng.random_u16() & 0x7f)
                    } else {
                        first_candidate
                    } as u16;
                    let patient_cap = (growth_scalar / 30) as i16;
                    let patients = if patient_cap < patient_candidate as i16 {
                        patient_cap as u16
                    } else {
                        patient_candidate
                    };
                    record.stat1 = patients;
                    let physician_candidate = mac_hospital_physicians_raw(
                        health_funding,
                        rng.random_u16() & 7,
                        record.stat0,
                    ) as u16;
                    let physician_cap = (growth_scalar / 120) as i16;
                    let physicians = if physician_cap < physician_candidate as i16 {
                        physician_cap as u16
                    } else {
                        physician_candidate
                    };
                    record.stat2 = physicians;
                    let ratio =
                        (i32::from(patients as i16) * 10) / i32::from((physicians as i16).max(1));
                    record.stat0 = if ratio < 50 {
                        12
                    } else if ratio < 111 {
                        ((110 - ratio) / 5) as u8
                    } else {
                        0
                    };
                    changed = true;
                }
                0xd2 => {
                    record.stat0 = police_funding as u8;
                    record.stat1 = police_capacity;
                    let police_tile_count = u32::from(self.mac_xbld_tile_count(0xd2)).max(1);
                    let arrests_per_police = crime_total / police_tile_count;
                    record.stat2 = arrests_per_police as u16;
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
                0xd4 => {
                    let count = self.mac_xbld_tile_count(0xd4);
                    mac_update_museum_record(&mut record, count, growth_scalar, college_funding);
                    changed = true;
                }
                0xd5 => {
                    let small = self.mac_xbld_tile_count(0x0d);
                    let large = self.mac_xbld_tile_count(0xd5);
                    mac_update_park_record(&mut record, small, large, growth_scalar);
                    changed = true;
                }
                0xd6 => {
                    record.stat3 = school_funding_word;
                    let students = mac_education_student_candidate(
                        school_counter,
                        u32::from(school_divisor as u16),
                        rng,
                        0x0f,
                        1_500,
                        1_500,
                        0xff,
                    )
                    .min((growth_scalar / 20) as i16) as u16;
                    record.stat1 = students;
                    let teachers =
                        mac_school_teachers_raw(school_funding, rng.random_u16() & 3, record.stat0);
                    let teachers = teachers.min((growth_scalar / 100) as i16) as u16;
                    record.stat2 = teachers;
                    let ratio =
                        (i32::from(students as i16) * 4) / i32::from((teachers as i16).max(1));
                    record.stat0 = if ratio < 50 {
                        12
                    } else if ratio < 111 {
                        ((110 - ratio) / 5) as u8
                    } else {
                        0
                    };
                    changed = true;
                }
                0xd7 => {
                    // The original reads this census WORD at the stadium slot.
                    let stadium_tiles = self.mac_xbld_tile_count(0xd7).max(1);
                    let mut candidate = growth_scalar / u32::from(stadium_tiles);
                    if (candidate as i32) > 25_000 {
                        candidate = 25_000_u32.wrapping_sub(u32::from(rng.random_u16() & 0xff));
                    }
                    let capacity = ((growth_scalar / 5) as i16).min(candidate as u16 as i16) as u16;
                    record.stat1 = capacity.wrapping_add(rng.random_u16() & 0xff);
                    record.stat0 = ((rng.random_u16() & 0x1f) + 9) as u8;
                    changed = true;
                }
                0xd8 => {
                    let prior = u32::from(record.stat1);
                    let raw = u32::from(incoming_arrest_total) / prison_divisor as u32 + prior
                        - prior / 4;
                    let inmate_candidate = if raw > 10_000 {
                        10_000 + u32::from(rng.random_u16() & 0x03ff)
                    } else {
                        raw
                    };
                    let inmates =
                        (inmate_candidate as u16 as i16).min((growth_scalar / 20) as i16) as u16;
                    record.stat1 = inmates;
                    record.stat2 = guard_capacity;
                    let per_hundred = (inmate_candidate / 100) as i16;
                    record.stat3 = per_hundred as u16;
                    prison_gate_sum = prison_gate_sum.wrapping_add(per_hundred);
                    if per_hundred < 91 {
                        record.stat0 = 0;
                    } else {
                        record.stat0 = mac_prison_overcrowding_field(
                            per_hundred,
                            police_funding,
                            rng.random_u16(),
                        );
                    }
                    changed = true;
                }
                0xd9 => {
                    record.stat3 = college_funding as u16;
                    let students = mac_education_student_candidate(
                        college_counter,
                        u32::from(college_divisor as u16),
                        rng,
                        0x1f,
                        5_000,
                        5_000,
                        0x01ff,
                    )
                    .min((growth_scalar / 30) as i16) as u16;
                    record.stat1 = students;
                    let professors = mac_college_professors_raw(
                        college_funding,
                        rng.random_u16() & 0x0f,
                        record.stat0,
                    );
                    let professors = professors.min((growth_scalar / 100) as i16) as u16;
                    record.stat2 = professors;
                    let ratio =
                        (i32::from(students as i16) * 4) / i32::from((professors as i16).max(1));
                    record.stat0 = if ratio < 50 {
                        12
                    } else if ratio < 111 {
                        ((110 - ratio) / 5) as u8
                    } else {
                        0
                    };
                    changed = true;
                }
                0xdb => {
                    record.stat2 = rng.random_u16() % 42;
                    changed = true;
                }
                0xe9 | 0xec | 0xed => {
                    let count = self.mac_xbld_tile_count(record.tile_id);
                    let mode = match record.tile_id {
                        0xe9 => 2,
                        0xec => 0,
                        _ => 1,
                    };
                    record.stat3 = self.mac_transit_accumulator(mode) as u16;
                    record.stat1 = if record.tile_id == 0xe9 {
                        count
                    } else {
                        count >> 2
                    };
                    if record.tile_id == 0xec {
                        record.stat2 = count;
                    }
                    changed = true;
                }
                0xf3 => {
                    changed |= record.stat2 != self.mac_mayor_approval;
                    record.stat2 = self.mac_mayor_approval;
                    if record.stat3 != 0 {
                        record.stat0 = record.stat0.wrapping_add(1);
                        record.stat3 = record.stat3.wrapping_sub(1);
                        changed = true;
                    }
                }
                0xf4 | 0xfa => {
                    record.stat0 = (self.mac_water_service_summary() as u8)
                        .wrapping_add((rng.random_u16() & 7) as u8);
                    record.stat1 = rng.random_u16() % 100;
                    let capacity = 0x87_u32 + u32::from(rng.random_u16() & 0x1f);
                    record.stat2 = capacity.min(growth_scalar / 50) as u16;
                    changed = true;
                }
                0xf5 => {
                    let count = self.mac_xbld_tile_count(0xf5);
                    mac_update_library_record(&mut record, count, growth_scalar, school_funding);
                    changed = true;
                }
                0xf8 => {
                    let count = self.mac_xbld_tile_count(0xf8);
                    mac_update_marina_record(&mut record, count, growth_scalar);
                    changed = true;
                }
                0xda | 0xfb..=0xfe => {
                    external_slot(self, slot);
                    continue;
                }
                0xff => {
                    record.stat0 = rng.random_u16() as u8;
                    let value = u32::from(rng.random_u16() & 0x03ff) + (growth_scalar >> 3);
                    record.stat1 = value as u16;
                    record.stat2 = (u32::from(rng.random_u16() & 0x007f) + value / 8) as u16;
                    record.stat3 = (u32::from(rng.random_u16() & 0x003f) + value / 16) as u16;
                    changed = true;
                }
                _ => {}
            }
            self.microsims.as_mut().expect("live XMIC table")[slot] = record;
        }
        self.mac_monthly_arrest_total = arrest_total;
        self.persist_mac_police_arrest_subtotal();
        self.clear_mac_monthly_transit_accumulators();
        let police_gate = if prison_raw_divisor > 0 && prison_gate_sum / prison_raw_divisor < 80 {
            1
        } else {
            0
        };
        self.set_mac_police_coverage_gate(police_gate as i16);
        if funds != self.city_funds() {
            self.set_city_funds(funds);
        }
        if changed {
            self.modified = true;
        }
    }

    fn clear_mac_monthly_transit_accumulators(&mut self) {
        for mode in 0..3 {
            self.set_mac_transit_accumulator(mode, 0);
        }
    }

    /// CODE 3 FUN_000123c8 returns the first XTXT match in y-major order.
    /// Packed (0,0) equals its not-found sentinel, even with later matches.
    pub(crate) fn mac_find_xmic_marker(&self, slot: usize) -> Option<(usize, usize)> {
        let marker = 0x33_usize.checked_add(slot).filter(|&id| id < 0xc9)? as u8;
        for y in 0..CITY_HEIGHT.min(128) {
            for x in 0..CITY_WIDTH.min(128) {
                if self.text_overlay_at(x, y) == Some(marker) {
                    return if x == 0 && y == 0 { None } else { Some((x, y)) };
                }
            }
        }
        None
    }

    /// Mac CODE 3's expired-plant route calls CODE 5 export 80 with drawing
    /// enabled. Preserve its 4x4 animation draws before the per-cell rubble
    /// draws, then replay the PlaceTile/XBIT/XZON/export-161 write order.
    fn clear_mac_aged_power_plant(&mut self, marker: (usize, usize), rng: &mut MacToolboxRng) {
        let root_building = self.building_at(marker.0, marker.1);
        if root_building < 6 {
            // export_80 returns before export_81, drawing, and tile writes.
            return;
        }
        let (mut x, mut y) = marker;
        // CODE 5 export 81 reads the Mac map's saved orientation. CityGame
        // can open that map with an independently normalized display compass.
        let rotation = usize::from(self.saved_rotation & 3);
        let masks = [
            [0x10, 0x20, 0x40, 0x80],
            [0x20, 0x40, 0x80, 0x10],
            [0x40, 0x80, 0x10, 0x20],
            [0x80, 0x10, 0x20, 0x40],
        ][rotation];

        // export_81 aligns the locator cell to the plant's rotated 4x4
        // footprint by walking the four oriented XZON corner signatures.
        let probe = |origin_x: usize, origin_y: usize, dx: i32, dy: i32| {
            let px = origin_x as i32 + dx;
            let py = origin_y as i32 + dy;
            (px >= 0 && py >= 0 && px < CITY_WIDTH as i32 && py < CITY_HEIGHT as i32)
                .then(|| {
                    self.zone_at(px as usize, py as usize)
                        .map(|z| z.raw_bits() & 0xf0)
                })
                .flatten()
                .unwrap_or(0)
        };
        for (mask, points) in [
            (masks[0], [(-1, -1), (0, -1), (-1, 0)]),
            (masks[1], [(1, -1), (0, -1), (1, 0)]),
            (masks[2], [(1, 1), (0, 1), (1, 0)]),
            (masks[3], [(-1, 1), (0, 1), (-1, 0)]),
        ] {
            if probe(x, y, points[0].0, points[0].1) == mask {
                x = (x as i32 + points[0].0) as usize;
                y = (y as i32 + points[0].1) as usize;
            }
            if probe(x, y, points[1].0, points[1].1) == mask {
                x = (x as i32 + points[1].0) as usize;
                y = (y as i32 + points[1].1) as usize;
            }
            if probe(x, y, points[2].0, points[2].1) == mask {
                x = (x as i32 + points[2].0) as usize;
                y = (y as i32 + points[2].1) as usize;
            }
        }
        let center = self.zone_at(x, y).map(|z| z.raw_bits() & 0xf0).unwrap_or(0);
        if center == masks[0] || center == masks[1] {
            y = y.saturating_add(3).min(CITY_HEIGHT - 1);
        }
        if center == masks[1] || center == masks[2] {
            x = x.saturating_sub(3);
        }

        // export_80's visible 4×4 scatter consumes two QuickDraw values per
        // cell before PlaceTile begins, even though CityGame draws its own map.
        for _ in 0..(4 * 4 * 4 * 2) {
            let _ = rng.random_u16();
        }
        for dx in 0..4 {
            for dy in 0..4 {
                let Some(tx) = x.checked_add(dx).filter(|&tx| tx < CITY_WIDTH) else {
                    continue;
                };
                let Some(ty) = y.checked_sub(dy).filter(|&ty| ty < CITY_HEIGHT) else {
                    continue;
                };
                self.mac_export80_replace_generic_cell(tx, ty, root_building, rng);
            }
        }
        self.modified = true;
    }
}

/// Mac CODE 3 library row; census is read at this slot, not at sweep entry.
pub(super) fn mac_update_library_record(
    record: &mut MicrosimRecord,
    tiles: u16,
    growth: u32,
    funding: u32,
) {
    let candidate = (tiles as i16).wrapping_mul(((funding as u16).wrapping_shl(2)) as i16);
    record.stat1 = ((growth / 18) as i16).min(candidate) as u16;
    let books = u32::from(record.stat2)
        .wrapping_add(u32::from(tiles).wrapping_mul(funding.wrapping_sub(50)))
        as i32;
    if (1..32_000).contains(&books) {
        record.stat2 = books as u16;
    }
    let numerator = funding.wrapping_mul(u32::from(tiles)).wrapping_mul(300) as i32;
    record.stat0 = (numerator / (growth as i32).max(1)).min(12) as u8;
}

/// Mac CODE 3 museum row; census is read at this slot.
pub(super) fn mac_update_museum_record(
    record: &mut MicrosimRecord,
    tiles: u16,
    growth: u32,
    funding: u32,
) {
    let candidate = (tiles as i16).wrapping_mul(((funding as u16).wrapping_shl(2)) as i16);
    record.stat1 = ((growth / 20) as i16).min(candidate) as u16;
    record.stat2 = tiles.wrapping_mul((funding as i32 / 10) as u16);
}

/// Mac CODE 3 park row, using the census at this slot.
pub(super) fn mac_update_park_record(
    record: &mut MicrosimRecord,
    small: u16,
    large: u16,
    growth: u32,
) {
    let candidate = (record.stat2 as i16).wrapping_mul(0x019c) as u16;
    let cap = (growth / 6).min(65_000) as u16;
    record.stat1 = candidate.min(65_000).min(cap);
    record.stat2 = (small as i16).wrapping_add(large as i16) as u16;
    record.stat3 = ((growth / 120) as i16).min((record.stat2 / 9) as i16) as u16;
}

/// Mac 1.2's app LFSR starts at zero, its fixed point, so this draw is zero.
pub(super) fn mac_update_marina_record(record: &mut MicrosimRecord, tiles: u16, growth: u32) {
    let mut rng = MacAppLfsrRng::new(MacAppLfsrRng::INITIAL_STATE);
    let candidate = (rng.bounded(20) as i16).wrapping_add((tiles as i16).wrapping_mul(8));
    record.stat1 = ((growth / 150) as i16).min(candidate) as u16;
}

/// Mac CODE 3 `export_162` accumulates the police row's full pre-store value
/// into a WORD subtotal (`local_10`), clamping when `subtotal + value >= 0xffff`.
/// The XMIC `+6` field is separately truncated to a signed WORD before storage.
pub(super) fn mac_police_arrest_subtotal_after_row(subtotal: u16, raw_row_value: u32) -> u16 {
    if (raw_row_value.wrapping_add(u32::from(subtotal)) as i32) < i32::from(u16::MAX) {
        subtotal.wrapping_add(raw_row_value as u16)
    } else {
        u16::MAX
    }
}

/// Mac CODE 3 school teacher path: multiply the active-document funding dword
/// by six, signed-divide the wrapped dword by ten, then add jitter and the
/// prior grade adjustment before the signed JNS clamp and XMIC word narrowing.
pub(super) fn mac_school_teachers_raw(funding: u32, jitter: u16, prior_grade: u8) -> i16 {
    let funding_term = (funding.wrapping_mul(6) as i32) / 10;
    let candidate = funding_term
        .wrapping_add(i32::from(jitter))
        .wrapping_add(i32::from(prior_grade))
        .wrapping_sub(12);
    if candidate >= 0 {
        candidate as u16 as i16
    } else {
        0
    }
}

/// Mac CODE 3 school `stat3`: the source is a signed funding dword, divided
/// toward zero by four, with only the quotient's low word stored.
pub(super) fn mac_school_funding_quarters(funding: u32) -> u16 {
    ((funding as i32) / 4) as u16
}

/// Mac CODE 3 school/college student path: DIVUL.L leaves a 32-bit unsigned
/// quotient, then ADD.L wraps the masked QuickDraw jitter. The following
/// CMPI/BLE tests those bits as a signed long; overflow replacement narrows
/// only after its optional second draw.
pub(super) fn mac_education_student_candidate(
    numerator: u32,
    divisor: u32,
    rng: &mut MacToolboxRng,
    jitter_mask: u16,
    overflow_limit: i32,
    overflow_base: i32,
    overflow_jitter_mask: u16,
) -> i16 {
    debug_assert_ne!(divisor, 0, "native education divisor is clamped to one");
    let mut candidate = (numerator / divisor) as i32;
    candidate = candidate.wrapping_add(i32::from(rng.random_u16() & jitter_mask));
    if candidate > overflow_limit {
        candidate = overflow_base + i32::from(rng.random_u16() & overflow_jitter_mask);
    }
    candidate as u16 as i16
}

/// Mac CODE 3 hospital `stat3`: signed dword `/2`, then keep the quotient's
/// low word. The same active-document dword feeds the physician candidate.
pub(super) fn mac_hospital_funding_half(funding: u32) -> u16 {
    ((funding as i32) / 2) as u16
}

/// Mac CODE 3 hospital physician candidate: add the QuickDraw nibble to the
/// full funding dword, subtract the signed grade adjustment at dword width,
/// clamp a negative signed result, then narrow to the helper's word input.
pub(super) fn mac_hospital_physicians_raw(funding: u32, jitter: u16, prior_grade: u8) -> i16 {
    let grade_delta = i32::from(12_i16.wrapping_sub(i16::from(prior_grade))).wrapping_mul(2) as u32;
    let candidate = funding
        .wrapping_add(u32::from(jitter))
        .wrapping_sub(grade_delta);
    if (candidate as i32) >= 0 {
        candidate as u16 as i16
    } else {
        0
    }
}

/// Mac CODE 3 prison overcrowding field: a signed dword funding adjustment
/// supplies both the divisor offset and the high dword of the DIVSL dividend.
/// The low dword is the zero-extended QuickDraw result; only the remainder's
/// low byte is stored in the XMIC row.
pub(super) fn mac_prison_overcrowding_field(
    inmates_per_hundred: i16,
    police_funding: u32,
    random_word: u16,
) -> u8 {
    let funding_adjustment = (100_i32.wrapping_sub(police_funding as i32)) / 10;
    let divisor = i32::from(inmates_per_hundred)
        .wrapping_sub(90)
        .wrapping_add(funding_adjustment);
    // Extension word 0x0801 selects a 32-bit dividend in D0; D1 receives
    // the remainder and is not the high half of the dividend.
    (i32::from(random_word) % divisor) as u8
}

/// Mac CODE 3 prison guard-cap candidate: MULU.L multiplies three by the
/// funding dword, then the caller passes the product's low word to the helper.
pub(super) fn mac_prison_guard_candidate(funding: u32) -> i16 {
    funding.wrapping_mul(3) as u16 as i16
}

/// Mac CODE 3 college teacher path: D4 becomes `2 * funding + jitter` at
/// dword width, then subtracts `4 * (12 - prior_grade)`. The signed dword
/// clamp precedes the helper's signed-word candidate comparison.
pub(super) fn mac_college_professors_raw(funding: u32, jitter: u16, prior_grade: u8) -> i16 {
    let twice_funding_plus_jitter = funding
        .wrapping_add(funding)
        .wrapping_add(u32::from(jitter));
    let grade_delta = i32::from(12_i16.wrapping_sub(i16::from(prior_grade))).wrapping_mul(4) as u32;
    let candidate = twice_funding_plus_jitter.wrapping_sub(grade_delta);
    if (candidate as i32) >= 0 {
        candidate as u16 as i16
    } else {
        0
    }
}

fn mac_power_plant_rebuild_cost(tile_id: u8) -> i32 {
    // Mac DATA/ZERO lookup at ffff a892 → ffff d990, used by CODE 3 export 162.
    match tile_id {
        0xc9 => 2_000,
        0xca => 6_600,
        0xcb => 15_000,
        0xcc => 1_300,
        0xcd => 28_000,
        0xce => 40_000,
        0xcf => 4_000,
        _ => 0,
    }
}
