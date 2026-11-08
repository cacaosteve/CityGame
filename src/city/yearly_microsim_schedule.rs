//! Win95 yearly XMIC slot order, RNG schedule, and shadow demolition preview.

use super::*;

#[derive(Debug, Clone, Copy)]
pub(super) struct YearlyMicrosimRngInputs {
    pub(super) normal_population: u32,
    pub(super) population_load: u32,
    pub(super) school_student_pool: u32,
    pub(super) school_count: u32,
    pub(super) college_student_pool: u32,
    pub(super) college_count: u32,
    pub(super) hospital_count: i32,
    pub(super) stadium_count: u32,
    pub(super) old_arrests: u32,
    pub(super) prison_arrest_divisor: u32,
    pub(super) marina_tiles: u32,
    pub(super) llama_short_path: bool,
    pub(super) no_disasters: bool,
    pub(super) city_funds: i32,
}

#[derive(Debug, Clone)]
pub(super) struct PowerPlantDemolitionSite {
    pub(super) anchor: (usize, usize),
    pub(super) cells: Vec<(usize, usize, bool)>,
}

#[derive(Debug, Clone)]
pub(super) struct YearlyMicrosimRngSchedule {
    /// Rows the PE XMIC walker reaches before an earlier slot's in-line
    /// DestroyBuilding path releases their XTXT resource.
    pub(super) dispatched_rows: [bool; MICROSIM_COUNT],
    pub(super) crt_start_by_slot: [Option<Win95CrtRand>; MICROSIM_COUNT],
    pub(super) crt_after_records: Win95CrtRand,
    pub(super) lfsr_start_by_slot: [Option<Win95Lfsr>; MICROSIM_COUNT],
    pub(super) lfsr_after_records: Win95Lfsr,
    pub(super) demolition_anchors: [Option<(usize, usize)>; MICROSIM_COUNT],
    pub(super) demolition_sites: [Option<PowerPlantDemolitionSite>; MICROSIM_COUNT],
}

/// Assign yearly-updater RNG streams to the XMIC slot order used by PE
/// `FUN_0044e390` (`BP=1..149`) even though the Rust writes remain grouped by
/// class. The cursor is advanced using each case's actual random-call gates;
/// each class arm later starts from its assigned stream state.
pub(super) fn yearly_microsim_rng_schedule(
    city: &City,
    microsims: &[MicrosimRecord],
    crt: Win95CrtRand,
    lfsr: Win95Lfsr,
    inputs: YearlyMicrosimRngInputs,
) -> YearlyMicrosimRngSchedule {
    let mut crt_cursor = crt;
    let mut lfsr_cursor = lfsr;
    let mut crt_start_by_slot = [None; MICROSIM_COUNT];
    let mut lfsr_start_by_slot = [None; MICROSIM_COUNT];
    let mut dispatched_rows = [false; MICROSIM_COUNT];
    let mut simulated_funds = inputs.city_funds;
    let mut demolition_anchors = [None; MICROSIM_COUNT];
    let mut demolition_sites = std::array::from_fn(|_| None);
    // Keep the small planes read by the PE's inline DestroyBuilding path in
    // shadow form. This lets later XMIC slots observe earlier PlaceTile and
    // XTXT/resource writes without mutating City during schedule construction.
    let mut shadow_buildings = vec![0; CITY_WIDTH * CITY_HEIGHT];
    let mut shadow_text = vec![0; CITY_WIDTH * CITY_HEIGHT];
    let mut shadow_terrain = vec![0; CITY_WIDTH * CITY_HEIGHT];
    for x in 0..CITY_WIDTH {
        for y in 0..CITY_HEIGHT {
            let offset = city_index(x, y);
            shadow_buildings[offset] = city.building_at(x, y);
            shadow_text[offset] = city.text_overlay_at(x, y).unwrap_or(0);
            shadow_terrain[offset] = city.terrain_at(x, y);
        }
    }
    let mut live_rows = [false; MICROSIM_COUNT];
    for (index, record) in microsims.iter().enumerate().take(MICROSIM_COUNT).skip(1) {
        live_rows[index] = record.tile_id != 0;
    }

    for index in 1..MICROSIM_COUNT {
        let Some(record) = microsims.get(index) else {
            continue;
        };
        let tile_id = record.tile_id;
        if !live_rows[index] {
            continue;
        }
        dispatched_rows[index] = true;

        let crt_before_slot = crt_cursor;
        let mut consumed_crt = false;
        match tile_id {
            201..=207 => {
                let _ = crt_cursor.random_u16_mask(7);
                consumed_crt = true;
                let age = record.stat0.wrapping_add(1);
                let marker = MIN_SIM_TEXT_ENTRIES.wrapping_add(index as u8);
                let anchor = pe_first_text_match(&shadow_text, marker);
                demolition_anchors[index] = anchor;
                if age > 50 {
                    if let Some(anchor) = anchor
                        .filter(|&(x, y)| (201..=207).contains(&shadow_buildings[city_index(x, y)]))
                    {
                        let cost = power_plant_rebuild_cost(tile_id) as i32;
                        if inputs.no_disasters && cost > 0 && simulated_funds >= cost {
                            simulated_funds -= cost;
                        } else {
                            let site =
                                pe_power_plant_demolition_site_from_planes(anchor, &shadow_terrain);
                            // Plant selector 4 enters the generic size×size
                            // render-scatter loop directly; the earlier two
                            // draws belong to a separate branch it bypasses.
                            for _ in 0..pe_destroy_building_mode1_render_scatter_draw_count(
                                tile_id as u8,
                            ) {
                                let _ = crt_cursor.random_u15();
                            }
                            for &(x, y, flat) in &site.cells {
                                let rubble = if flat { crt_cursor.rubble_tile_id() } else { 0 };
                                shadow_buildings[city_index(x, y)] = rubble;
                                let text = shadow_text[city_index(x, y)];
                                if (0x01..=0xc8).contains(&text) {
                                    shadow_text[city_index(x, y)] = 0;
                                    if text >= 0x3d {
                                        let released = usize::from(text - MIN_SIM_TEXT_ENTRIES);
                                        if released < live_rows.len() {
                                            live_rows[released] = false;
                                        }
                                    }
                                }
                            }
                            demolition_sites[index] = Some(site);
                        }
                    }
                }
            }
            209 => {
                let divisor = pe_occupancy_divisor(inputs.hospital_count.saturating_mul(25));
                let mut patients = (inputs.normal_population / divisor)
                    .wrapping_add(u32::from(crt_cursor.random_u16_mask(0x0f)));
                if (patients as i32) > 1_000 {
                    patients = 1_000 + u32::from(crt_cursor.random_u16_mask(0x7f));
                }
                let _ = patients;
                let _ = crt_cursor.random_u16_mask(7);
                consumed_crt = true;
            }
            210 => {
                let _ = crt_cursor.random_u16_mask(0x0f);
                consumed_crt = true;
            }
            211 => {
                let _ = crt_cursor.random_u15();
                consumed_crt = true;
            }
            214 => {
                let mut students = (inputs.school_student_pool / inputs.school_count)
                    .wrapping_add(u32::from(crt_cursor.random_u16_mask(0x0f)));
                if (students as i32) > 1_500 {
                    students = 1_500 + u32::from(crt_cursor.random_u16_mask(0xff));
                }
                let _ = students;
                let _ = crt_cursor.random_u16_mask(7);
                consumed_crt = true;
            }
            215 => {
                let mut attendance = inputs.population_load / inputs.stadium_count;
                // Match the stadium arm's signed `JLE` after unsigned DIV.
                if (attendance as i32) > 25_000 {
                    attendance = 25_000 - u32::from(crt_cursor.random_u16_mask(0xff));
                }
                let _ = attendance;
                let _ = crt_cursor.random_u16_mask(0xff);
                let _ = crt_cursor.random_u16_mask(0x1f);
                consumed_crt = true;
            }
            216 => {
                let previous = u32::from(record.stat1);
                let base_inmates = previous
                    .saturating_sub(previous / 4)
                    .saturating_add(inputs.old_arrests / inputs.prison_arrest_divisor);
                let mut inmates = base_inmates;
                if inmates > 10_000 {
                    inmates = 10_000 + u32::from(crt_cursor.random_u16_mask(0x03ff));
                }
                if inmates / 100 > 90 {
                    let _ = crt_cursor.random_u15();
                }
                consumed_crt = base_inmates > 10_000 || inmates / 100 > 90;
            }
            217 => {
                let mut students = (inputs.college_student_pool / inputs.college_count)
                    .wrapping_add(u32::from(crt_cursor.random_u16_mask(0x1f)));
                if (students as i32) > 5_000 {
                    students = 5_000 + u32::from(crt_cursor.random_u16_mask(0x01ff));
                }
                let _ = students;
                let _ = crt_cursor.random_u16_mask(0x0f);
                consumed_crt = true;
            }
            219 => {
                let _ = crt_cursor.random_mod(0x2a);
                consumed_crt = true;
            }
            244 | 250 => {
                let _ = crt_cursor.random_u16_mask(7);
                let _ = crt_cursor.random_mod(100);
                let _ = crt_cursor.random_u16_mask(0x1f);
                consumed_crt = true;
            }
            255 if inputs.llama_short_path => {
                let _ = crt_cursor.random_u16_mask(0x03ff);
                let _ = crt_cursor.random_u16_mask(0x7f);
                let _ = crt_cursor.random_u16_mask(0x7f);
                consumed_crt = true;
            }
            255 => {
                let _ = crt_cursor.random_u15();
                let _ = crt_cursor.random_u16_mask(0x03ff);
                let _ = crt_cursor.random_u16_mask(0x7f);
                let _ = crt_cursor.random_u16_mask(0x3f);
                consumed_crt = true;
            }
            _ => {}
        }
        // Prison always participates in the CRT-backed class loop even when
        // both inmate thresholds suppress its draws; preserve its slot's
        // current cursor so the updater can select that state uniformly.
        if consumed_crt || tile_id == 216 {
            crt_start_by_slot[index] = Some(crt_before_slot);
        }

        let lfsr_draws = if index == MICROSIM_SYSTEM_MARINA && tile_id == 248 {
            1
        } else if (251..=254).contains(&tile_id) {
            1
        } else {
            0
        };
        if lfsr_draws != 0 {
            lfsr_start_by_slot[index] = Some(lfsr_cursor);
            if index == MICROSIM_SYSTEM_MARINA {
                let _ = lfsr_cursor.random_mod(0x14);
            } else {
                let _ = lfsr_cursor.random_mod64();
            }
        }
    }

    YearlyMicrosimRngSchedule {
        dispatched_rows,
        crt_start_by_slot,
        crt_after_records: crt_cursor,
        lfsr_start_by_slot,
        lfsr_after_records: lfsr_cursor,
        demolition_anchors,
        demolition_sites,
    }
}

fn pe_first_text_match(text_plane: &[u8], marker: u8) -> Option<(usize, usize)> {
    // FUN_0044F890 scans x-major, then y-major, and returns the first XTXT
    // cell carrying 0x33 + XMIC slot.
    for x in 0..CITY_WIDTH {
        for y in 0..CITY_HEIGHT {
            if text_plane.get(city_index(x, y)).copied() == Some(marker) {
                return Some((x, y));
            }
        }
    }
    None
}

fn pe_power_plant_demolition_site_from_planes(
    anchor: (usize, usize),
    terrain: &[u8],
) -> PowerPlantDemolitionSite {
    let mut cells = Vec::with_capacity(16);
    for dx in 0..4 {
        for dy in 0..4 {
            let tx = anchor.0 + dx;
            let Some(ty) = anchor.1.checked_sub(dy) else {
                continue;
            };
            if tx < CITY_WIDTH && ty < CITY_HEIGHT {
                cells.push((tx, ty, terrain[city_index(tx, ty)] == 0));
            }
        }
    }
    PowerPlantDemolitionSite { anchor, cells }
}

pub(super) fn power_plant_rebuild_cost(tile_id: u8) -> u32 {
    // Win95 cost table `0x4DC1D0` via type index `0x4E7E24[tile]`.
    match tile_id {
        201 => 2_000,  // gas
        202 => 6_600,  // oil
        203 => 15_000, // nuclear
        204 => 1_300,  // solar
        205 => 28_000, // microwave
        206 => 40_000, // fusion
        207 => 4_000,  // coal
        _ => 0,
    }
}
