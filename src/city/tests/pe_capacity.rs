use super::City;
use super::{
    CITY_HEIGHT, CITY_WIDTH, CityFileKind, MAC_POWER_SUMMARY_MISC_OFFSET,
    MAC_XBLD_CENSUS_MISC_OFFSET, MIN_SIM_TEXT_ENTRIES, pe_arco_tile_pop_bonus,
    pe_microsim_scaled_capacity, pe_occupancy_arco_footprints, pe_occupancy_div9_count,
    pe_occupancy_div16_count, pe_occupancy_divisor, pe_occupancy_tile_words, pe_park_attendance,
    pe_transit_depot_count,
};
use crate::mac_random::{MacGameLcgRng, MacToolboxRng};

#[test]
fn arco_tile_bonus_zero_at_or_below_threshold() {
    assert_eq!(pe_arco_tile_pop_bonus(0), 0);
    assert_eq!(pe_arco_tile_pop_bonus(0x8c), 0);
}

#[test]
fn arco_tile_bonus_matches_fun_0044f7d0_above_threshold() {
    // PE: count*20000 - 0x2AB980 (2_800_000) when count > 0x8C.
    assert_eq!(pe_arco_tile_pop_bonus(141), 20_000);
    let load = pe_arco_tile_pop_bonus(141);
    assert_eq!(pe_microsim_scaled_capacity(load, 90, 200), 200);
    assert_eq!(pe_microsim_scaled_capacity(load, 90, 300), 222); // 20000/90
}

#[test]
fn yearly_stadium_signed_threshold_skips_cap_draw_for_high_bit_quotient() {
    let mut city =
        City::new_blank("/tmp/sc2000-stadium-signed-gate.sc2", "StadiumGate").expect("blank city");
    let first = usize::from(
        city.provision_microsim_for_building(215)
            .expect("first stadium row")
            - super::MIN_SIM_TEXT_ENTRIES,
    );
    let second = usize::from(
        city.provision_microsim_for_building(215)
            .expect("second stadium row")
            - super::MIN_SIM_TEXT_ENTRIES,
    );
    let population = 0x8000_0000;
    let seed = 0x1357_2468;

    let mut expected_rng = super::Win95CrtRand::new(seed);
    let mut expected_rows = Vec::new();
    for _ in [first, second] {
        // `CMP EAX, 25000; JLE` interprets this unsigned-DIV quotient as
        // signed, so its high bit skips the conditional cap draw.
        let attendance = super::pe_microsim_scaled_capacity(population, 5, population);
        let jitter = expected_rng.random_u16_mask(0xff);
        let grade = expected_rng.random_u16_mask(0x1f) + 9;
        let expected_attendance = attendance.wrapping_add(u32::from(jitter)) as u16;
        expected_rows.push((grade as u8, expected_attendance));
    }

    let mut actual_rng = super::Win95CrtRand::new(seed);
    let mut game_rng = super::Win95GameRand::new(1);
    let mut lfsr = super::Win95Lfsr::new(1);
    city.refresh_structure_microsim_yearly_with_rngs(
        population,
        100,
        100,
        100,
        100,
        100,
        &mut actual_rng,
        &mut game_rng,
        &mut lfsr,
    );

    assert_eq!(
        actual_rng, expected_rng,
        "stadium draw count stays in slot order"
    );
    for (index, expected) in [first, second].into_iter().zip(expected_rows) {
        let record = city.microsim_at(index).expect("stadium row");
        assert_eq!((record.stat0, record.stat1), expected);
    }
}

#[test]
fn yearly_school_and_hospital_signed_thresholds_skip_extra_draws() {
    let seed = super::Win95CrtRand::new(0x1357_2468);

    let mut school_city =
        City::new_blank("/tmp/sc2000-school-signed-gate.sc2", "SchoolGate").expect("blank city");
    let school_slots = [
        school_city.provision_microsim_for_building(214).unwrap(),
        school_city.provision_microsim_for_building(214).unwrap(),
    ]
    .map(|text| usize::from(text - super::MIN_SIM_TEXT_ENTRIES));
    school_city.pe_age_cohorts[1] = 0x8000_0000;
    let mut expected_school_rng = seed;
    let mut expected_school_students = Vec::new();
    for _ in school_slots {
        let students =
            0x8000_0000_u32.wrapping_add(u32::from(expected_school_rng.random_u16_mask(0x0f)));
        assert!((students as i32) <= 1_500);
        expected_school_students
            .push(super::pe_microsim_scaled_capacity(10_000, 20, students) as u16);
        let _teacher_jitter = expected_school_rng.random_u16_mask(7);
    }

    let mut actual_school_rng = seed;
    let mut game_rng = super::Win95GameRand::new(1);
    let mut lfsr = super::Win95Lfsr::new(1);
    school_city.refresh_structure_microsim_yearly_with_rngs(
        10_000,
        100,
        100,
        100,
        100,
        100,
        &mut actual_school_rng,
        &mut game_rng,
        &mut lfsr,
    );
    assert_eq!(actual_school_rng, expected_school_rng);
    for (slot, expected) in school_slots.into_iter().zip(expected_school_students) {
        assert_eq!(school_city.microsim_at(slot).unwrap().stat1, expected);
    }

    let mut hospital_city = City::new_blank("/tmp/sc2000-hospital-signed-gate.sc2", "HospitalGate")
        .expect("blank city");
    let hospital_slots = [
        hospital_city.provision_microsim_for_building(209).unwrap(),
        hospital_city.provision_microsim_for_building(209).unwrap(),
    ]
    .map(|text| usize::from(text - super::MIN_SIM_TEXT_ENTRIES));
    let population = 0x8000_0000_u32;
    let mut expected_hospital_rng = seed;
    let mut expected_hospital_patients = Vec::new();
    for _ in hospital_slots {
        let patients =
            population.wrapping_add(u32::from(expected_hospital_rng.random_u16_mask(0x0f)));
        assert!((patients as i32) <= 1_000);
        expected_hospital_patients
            .push(super::pe_microsim_scaled_capacity(population, 30, patients) as u16);
        let _doctor_jitter = expected_hospital_rng.random_u16_mask(7);
    }

    let mut actual_hospital_rng = seed;
    let mut game_rng = super::Win95GameRand::new(1);
    let mut lfsr = super::Win95Lfsr::new(1);
    hospital_city.refresh_structure_microsim_yearly_with_rngs(
        population,
        100,
        100,
        100,
        100,
        100,
        &mut actual_hospital_rng,
        &mut game_rng,
        &mut lfsr,
    );
    assert_eq!(actual_hospital_rng, expected_hospital_rng);
    for (slot, expected) in hospital_slots.into_iter().zip(expected_hospital_patients) {
        assert_eq!(hospital_city.microsim_at(slot).unwrap().stat1, expected);
    }
}

#[test]
fn yearly_capacity_population_load_adds_at_pe_dword_width() {
    assert_eq!(super::pe_microsim_population_load(u32::MAX - 15, 0, 32), 16);

    let mut city =
        City::new_blank("/tmp/sc2000-pe-population-wrap.sc2", "PopWrap").expect("blank city");
    let hall_slot = usize::from(
        city.provision_microsim_for_building(208)
            .expect("City Hall row")
            - super::MIN_SIM_TEXT_ENTRIES,
    );
    city.pe_dw_city_population = u32::MAX - 15;
    city.set_pe_dw_arcology_population(32);

    let mut initializer_crt = super::Win95CrtRand::new(1);
    let initialized_hall =
        super::initial_structure_microsim_record(&city, 208, &mut initializer_crt);
    assert_eq!(initialized_hall.stat1, 0);

    let mut crt = super::Win95CrtRand::new(0x1357_2468);
    let mut game_rng = super::Win95GameRand::new(1);
    let mut lfsr = super::Win95Lfsr::new(1);
    city.refresh_structure_microsim_yearly_with_rngs(
        u32::MAX - 15,
        100,
        100,
        100,
        100,
        100,
        &mut crt,
        &mut game_rng,
        &mut lfsr,
    );

    // PE adds the city and arcology dwords before DIV. (2^32 - 16) + 32
    // wraps to 16, so the City Hall capacity is 16 / 900 = 0. A saturating
    // Rust sum instead makes this field hit its 200 cap.
    assert_eq!(city.microsim_at(hall_slot).map(|row| row.stat1), Some(0));
}

#[test]
fn fun_0044f7d0_divisor_zero_becomes_100_and_quot_is_16_bit() {
    assert_eq!(pe_microsim_scaled_capacity(10_000, 0, 200), 100);
    // 0x1_0001 / 1 = 65537, masked to 1.
    // The cap is read as a signed 16-bit word; 50000 is negative, so PE
    // returns its low word instead of choosing the positive quotient.
    assert_eq!(pe_microsim_scaled_capacity(0x1_0001, 1, 50_000), 50_000);
}

#[test]
fn fun_0044f7d0_compares_the_sign_extended_cap_word() {
    assert_eq!(pe_microsim_scaled_capacity(1_000, 1, 32_767), 1_000);
    assert_eq!(pe_microsim_scaled_capacity(1_000, 1, 0x8000), 0x8000);
    assert_eq!(pe_microsim_scaled_capacity(1_000, 1, 0x1_0000), 0);
}

#[test]
fn park_attendance_uses_last_year_tiles_capped_at_pop_div6() {
    assert_eq!(pe_park_attendance(7_000, 0), 0);
    assert_eq!(pe_park_attendance(7_000, 9), 1_166);
    assert_eq!(pe_park_attendance(55_860, 9), 3_708);
    assert_eq!(pe_park_attendance(0, 9), 0);
    assert_eq!(pe_park_attendance(7_000, 158), 1_166);
}

#[test]
fn transit_depot_count_is_tiles_sar_2() {
    assert_eq!(pe_transit_depot_count(0), 0);
    assert_eq!(pe_transit_depot_count(2), 0);
    assert_eq!(pe_transit_depot_count(4), 1);
    assert_eq!(pe_transit_depot_count(8), 2);
    assert_eq!(pe_transit_depot_count(9), 2);
}

fn stamp(city: &mut City, origin_x: usize, origin_y: usize, building: u8, size: usize) {
    for y in origin_y..origin_y + size {
        for x in origin_x..origin_x + size {
            // Keep `pe_dw_tile_count` coherent for occupancy / EQ_LE site tests.
            let old = city.building_at(x, y) as usize;
            city.pe_dw_tile_count[old] = city.pe_dw_tile_count[old].wrapping_sub(1);
            city.pe_dw_tile_count[building as usize] =
                city.pe_dw_tile_count[building as usize].wrapping_add(1);
            city.set_building(x, y, building);
        }
    }
}

#[test]
fn zoo_yearly_rolls_use_the_pe_game_lcg_without_advancing_crt() {
    let mut city = City::new_blank("/tmp/sc2000-zoo-rng.sc2", "ZooRng").unwrap();
    let text = city.provision_microsim_for_building(218).unwrap();
    let index = usize::from(text - super::MIN_SIM_TEXT_ENTRIES);
    stamp(&mut city, 40, 40, 218, 4);
    for dy in 0..4 {
        for dx in 0..4 {
            city.set_text_overlay(40 + dx, 40 + dy, text);
        }
    }
    let mut crt = super::Win95CrtRand::new(0x1234_5678);
    let mut game_rand = super::Win95GameRand::new(0x7654_3211);
    let mut lfsr = super::Win95Lfsr::new(1);
    let mut expected_game_rand = game_rand;
    let expected = [
        expected_game_rand.random_mod(100),
        expected_game_rand.random_mod(100),
        expected_game_rand.random_mod(100),
        expected_game_rand.random_mod(100),
    ];
    let expected_crt = crt;
    city.refresh_structure_microsim_yearly_with_rngs(
        10_000,
        100,
        100,
        100,
        100,
        100,
        &mut crt,
        &mut game_rand,
        &mut lfsr,
    );
    assert_eq!(
        city.microsim_at(index)
            .map(|row| (row.stat0, row.stat1, row.stat2, row.stat3)),
        Some((expected[0] as u8, expected[1], expected[2], expected[3]))
    );
    assert_eq!(game_rand, expected_game_rand, "exactly four game-LCG draws");

    // Presence of a zoo adds no calls to the PE CRT stream.
    let mut no_zoo = City::new_blank("/tmp/sc2000-zoo-rng-empty.sc2", "ZooRngEmpty").unwrap();
    let mut no_zoo_crt = expected_crt;
    let mut no_zoo_game = super::Win95GameRand::new(0x7654_3211);
    let mut no_zoo_lfsr = super::Win95Lfsr::new(1);
    no_zoo.refresh_structure_microsim_yearly_with_rngs(
        10_000,
        100,
        100,
        100,
        100,
        100,
        &mut no_zoo_crt,
        &mut no_zoo_game,
        &mut no_zoo_lfsr,
    );
    assert_eq!(crt, no_zoo_crt);
}

#[test]
fn yearly_llama_uses_truncated_sighting_ratios_and_wraps_pe_words() {
    fn llama_crt_states(
        city: &City,
        index: usize,
        population: u32,
        seed: u32,
    ) -> (super::Win95CrtRand, super::Win95CrtRand) {
        let crt = super::Win95CrtRand::new(seed);
        let inputs = super::YearlyMicrosimRngInputs {
            normal_population: population,
            population_load: population.saturating_add(city.pe_dw_arcology_population()),
            school_student_pool: 0,
            school_count: 1,
            college_student_pool: 0,
            college_count: 1,
            hospital_count: 0,
            stadium_count: 1,
            old_arrests: 0,
            prison_arrest_divisor: 1,
            marina_tiles: 0,
            llama_short_path: city.name == "aus",
            no_disasters: false,
            city_funds: 0,
        };
        let schedule = super::yearly_microsim_rng_schedule(
            city,
            city.microsims.as_ref().expect("microsim table"),
            crt,
            super::Win95Lfsr::new(1),
            inputs,
        );
        (
            schedule.crt_start_by_slot[index].expect("scheduled Llama Dome CRT state"),
            schedule.crt_after_records,
        )
    }

    let population = 600_000;
    let seed = 0x44f1_4e01;

    let mut city =
        City::new_blank("/tmp/sc2000-llama-yearly-width.sc2", "LlamaWidth").expect("blank city");
    city.set_pe_dw_arcology_population(80_000);
    let text = city.provision_microsim_for_building(255).unwrap();
    let index = usize::from(text - super::MIN_SIM_TEXT_ENTRIES);
    stamp(&mut city, 40, 40, 255, 4);
    for dy in 0..4 {
        for dx in 0..4 {
            city.set_text_overlay(40 + dx, 40 + dy, text);
        }
    }

    let (llama_crt, expected_after) = llama_crt_states(&city, index, population, seed);
    let mut expected_crt = llama_crt;
    let expected_stat0 = (expected_crt.random_u15() & 0xff) as u8;
    let visitors = (population >> 3) + u32::from(expected_crt.random_u16_mask(0x03ff));
    let expected_stat1 = visitors as u16;
    let expected_stat2 =
        u32::from(expected_crt.random_u16_mask(0x7f)).wrapping_add(visitors >> 3) as u16;
    let expected_stat3 =
        u32::from(expected_crt.random_u16_mask(0x3f)).wrapping_add(visitors >> 4) as u16;

    let mut crt = super::Win95CrtRand::new(seed);
    let mut game_rng = super::Win95GameRand::new(1);
    let mut lfsr = super::Win95Lfsr::new(1);
    city.refresh_structure_microsim_yearly_with_rngs(
        population,
        100,
        100,
        100,
        100,
        100,
        &mut crt,
        &mut game_rng,
        &mut lfsr,
    );
    let row = city.microsim_at(index).expect("Llama Dome row");
    assert_eq!(
        (row.stat0, row.stat1, row.stat2, row.stat3),
        (
            expected_stat0,
            expected_stat1,
            expected_stat2,
            expected_stat3
        ),
        "PE stores visitor/sighting/bungee values through 16-bit writes and truncates both ratios toward zero"
    );
    assert!(visitors > u32::from(u16::MAX));
    assert_eq!(
        crt, expected_after,
        "ordinary Llama Dome consumes four draws at its XMIC slot"
    );

    // The exact city-name short path has three draws and also truncates
    // its visitor sum to the PE's 16-bit XMIC field.
    let mut aus =
        City::new_blank("/tmp/sc2000-llama-yearly-aus-width.sc2", "aus").expect("blank city");
    aus.set_pe_dw_arcology_population(80_000);
    let text = aus.provision_microsim_for_building(255).unwrap();
    let index = usize::from(text - super::MIN_SIM_TEXT_ENTRIES);
    stamp(&mut aus, 40, 40, 255, 4);
    for dy in 0..4 {
        for dx in 0..4 {
            aus.set_text_overlay(40 + dx, 40 + dy, text);
        }
    }
    let (llama_crt, expected_after) = llama_crt_states(&aus, index, population, seed);
    let mut expected_crt = llama_crt;
    let expected_stat1 =
        ((population >> 3).wrapping_add(u32::from(expected_crt.random_u16_mask(0x03ff)))) as u16;
    let expected_stat0 = expected_crt.random_u16_mask(0x7f) as u8;
    let expected_stat2 = expected_crt.random_u16_mask(0x7f) + 10;
    let mut crt = super::Win95CrtRand::new(seed);
    let mut game_rng = super::Win95GameRand::new(1);
    let mut lfsr = super::Win95Lfsr::new(1);
    aus.refresh_structure_microsim_yearly_with_rngs(
        population,
        100,
        100,
        100,
        100,
        100,
        &mut crt,
        &mut game_rng,
        &mut lfsr,
    );
    let row = aus.microsim_at(index).expect("aus Llama Dome row");
    assert_eq!(
        (row.stat0, row.stat1, row.stat2),
        (expected_stat0, expected_stat1, expected_stat2),
        "the aus branch uses the same truncating PE visitor store"
    );
    assert_eq!(
        crt, expected_after,
        "aus Llama Dome consumes three draws at its XMIC slot"
    );
}

#[test]
fn yearly_microsim_random_rolls_follow_xmic_slot_order() {
    let mut police_first = City::new_blank("/tmp/sc2000-order-police.sc2", "Order").unwrap();
    let police_slot = usize::from(
        police_first.provision_microsim_for_building(210).unwrap() - super::MIN_SIM_TEXT_ENTRIES,
    );
    let fire_slot = usize::from(
        police_first.provision_microsim_for_building(211).unwrap() - super::MIN_SIM_TEXT_ENTRIES,
    );

    let mut fire_first = City::new_blank("/tmp/sc2000-order-fire.sc2", "Order").unwrap();
    let fire_first_slot = usize::from(
        fire_first.provision_microsim_for_building(211).unwrap() - super::MIN_SIM_TEXT_ENTRIES,
    );
    let police_second_slot = usize::from(
        fire_first.provision_microsim_for_building(210).unwrap() - super::MIN_SIM_TEXT_ENTRIES,
    );
    assert_eq!(police_slot, fire_first_slot);
    assert_eq!(fire_slot, police_second_slot);

    let seed = super::Win95CrtRand::new(0x1357_2468);
    let mut control = City::new_blank("/tmp/sc2000-order-control.sc2", "Order").unwrap();
    let mut after_poll = seed;
    let mut game_rand = super::Win95GameRand::new(0x7654_3211);
    let mut lfsr = super::Win95Lfsr::new(1);
    control.refresh_structure_microsim_yearly_with_rngs(
        10_000,
        100,
        100,
        100,
        100,
        100,
        &mut after_poll,
        &mut game_rand,
        &mut lfsr,
    );
    let mut expected = after_poll;
    let police_roll_first = expected.random_u16_mask(0x0f);
    let fire_roll_second = expected.random_u15() % 20 + 2;
    let mut reverse_expected = after_poll;
    let fire_roll_first = reverse_expected.random_u15() % 20 + 2;
    let police_roll_second = reverse_expected.random_u15();
    let expected_final = expected;

    for city in [&mut police_first, &mut fire_first] {
        let mut crt = seed;
        let mut game_rand = super::Win95GameRand::new(0x7654_3211);
        let mut lfsr = super::Win95Lfsr::new(1);
        city.refresh_structure_microsim_yearly_with_rngs(
            10_000,
            100,
            100,
            100,
            100,
            100,
            &mut crt,
            &mut game_rand,
            &mut lfsr,
        );
        assert_eq!(
            crt, expected_final,
            "slot walk preserves post-record CRT state"
        );
    }

    assert_eq!(
        police_first.microsim_at(police_slot).map(|row| row.stat3),
        Some(u16::from(police_roll_first))
    );
    assert_eq!(
        police_first.microsim_at(fire_slot).map(|row| row.stat3),
        Some(fire_roll_second)
    );
    assert_eq!(
        fire_first.microsim_at(fire_first_slot).map(|row| row.stat3),
        Some(fire_roll_first)
    );
    assert_eq!(
        fire_first
            .microsim_at(police_second_slot)
            .map(|row| row.stat3),
        Some(u16::from(police_roll_second & 0x0f))
    );
}

#[test]
fn yearly_hospital_extra_patient_roll_shifts_later_xmic_rows() {
    let mut city = City::new_blank("/tmp/sc2000-hospital-order.sc2", "HospitalOrder").unwrap();
    city.provision_microsim_for_building(209).unwrap();
    let police_slot = usize::from(
        city.provision_microsim_for_building(210).unwrap() - super::MIN_SIM_TEXT_ENTRIES,
    );

    let seed = super::Win95CrtRand::new(0x2468_1357);
    let mut control = City::new_blank("/tmp/sc2000-hospital-control.sc2", "HospitalOrder").unwrap();
    let mut after_poll = seed;
    let mut game_rand = super::Win95GameRand::new(1);
    let mut lfsr = super::Win95Lfsr::new(1);
    control.refresh_structure_microsim_yearly_with_rngs(
        5_000,
        100,
        100,
        100,
        100,
        100,
        &mut after_poll,
        &mut game_rand,
        &mut lfsr,
    );

    // Empty map means the hospital divisor is one. Its first jitter pushes
    // 5,000 patients above 1,000, so PE takes the extra 0x7f roll, then the
    // doctor roll, before the later police record gets its arrest roll.
    let mut expected = after_poll;
    let _patients = expected.random_u16_mask(0x0f);
    let _over_capacity = expected.random_u16_mask(0x7f);
    let _doctor_jitter = expected.random_u16_mask(7);
    let police_roll = expected.random_u16_mask(0x0f);
    let mut crt = seed;
    let mut game_rand = super::Win95GameRand::new(1);
    let mut lfsr = super::Win95Lfsr::new(1);
    city.refresh_structure_microsim_yearly_with_rngs(
        5_000,
        100,
        100,
        100,
        100,
        100,
        &mut crt,
        &mut game_rand,
        &mut lfsr,
    );

    assert_eq!(
        city.microsim_at(police_slot).map(|row| row.stat3),
        Some(u16::from(police_roll)),
        "the later police row receives the draw after the hospital's conditional call"
    );
    assert_eq!(
        crt, expected,
        "conditional draw advances the shared PE stream"
    );
}

#[test]
fn yearly_school_college_and_hospital_store_zero_staff_but_grade_with_minimum_one() {
    let mut city = City::new_blank("/tmp/sc2000-zero-staff.sc2", "ZeroStaff").unwrap();
    let school_slot = usize::from(
        city.provision_microsim_for_building(214).unwrap() - super::MIN_SIM_TEXT_ENTRIES,
    );
    let college_slot = usize::from(
        city.provision_microsim_for_building(217).unwrap() - super::MIN_SIM_TEXT_ENTRIES,
    );
    let hospital_slot = usize::from(
        city.provision_microsim_for_building(209).unwrap() - super::MIN_SIM_TEXT_ENTRIES,
    );
    let mut crt = super::Win95CrtRand::new(0x2000_0049);
    let mut game_rand = super::Win95GameRand::new(1);
    let mut lfsr = super::Win95Lfsr::new(1);

    city.refresh_structure_microsim_yearly_with_rngs(
        0,
        0,
        0,
        0,
        0,
        0,
        &mut crt,
        &mut game_rand,
        &mut lfsr,
    );

    let school = city.microsim_at(school_slot).expect("school row");
    assert_eq!(school.stat2, 0, "PE stores the raw teacher count");
    assert_eq!(school.stat0, 12, "PE clamps only the grade divisor to one");
    let college = city.microsim_at(college_slot).expect("college row");
    assert_eq!(college.stat2, 0, "PE stores the raw professor count");
    assert_eq!(college.stat0, 12, "PE clamps only the grade divisor to one");
    let hospital = city.microsim_at(hospital_slot).expect("hospital row");
    assert_eq!(hospital.stat2, 0, "PE stores the raw doctor count");
    assert_eq!(
        hospital.stat0, 12,
        "PE clamps only the grade divisor to one"
    );
}

#[test]
fn yearly_hospital_funding_field_truncates_odd_percent_like_pe() {
    let mut city = City::new_blank("/tmp/sc2000-hospital-funding-round.sc2", "HospitalFunding")
        .expect("blank city");
    let hospital_slot = usize::from(
        city.provision_microsim_for_building(209).unwrap() - super::MIN_SIM_TEXT_ENTRIES,
    );
    let mut crt = super::Win95CrtRand::new(0x2000_0067);
    let mut game_rand = super::Win95GameRand::new(1);
    let mut lfsr = super::Win95Lfsr::new(1);

    city.refresh_structure_microsim_yearly_with_rngs(
        0,
        100,
        100,
        95,
        100,
        100,
        &mut crt,
        &mut game_rand,
        &mut lfsr,
    );

    assert_eq!(
        city.microsim_at(hospital_slot).map(|row| row.stat3),
        Some(47),
        "PE truncates signed 95 / 2 before storing the hospital field"
    );
}

#[test]
fn hospital_funding_and_doctor_candidates_keep_pe_dword_semantics() {
    assert_eq!(super::pe_hospital_funding_half(95), 47);
    assert_eq!(super::pe_hospital_funding_half(100), 50);
    assert_eq!(super::pe_hospital_funding_half(u32::MAX), 0);
    assert_eq!(super::pe_hospital_funding_half(0xffff_fffd), u16::MAX);
    assert_eq!(super::pe_hospital_funding_half(0x8000_0000), 0);

    // `grade_adjust` is already `2 * prior_grade - 0x18` at the callsite.
    assert_eq!(super::pe_hospital_doctors_raw(100, 7, 72), 179);
    // Funding plus jitter/grade wraps at 32 bits before JNS.
    assert_eq!(super::pe_hospital_doctors_raw(0x7fff_ffff, 7, 72), 0);
    // A signed -1 funding dword plus the positive terms wraps to 78.
    assert_eq!(super::pe_hospital_doctors_raw(u32::MAX, 7, 72), 78);
}

#[test]
fn yearly_school_extra_student_roll_shifts_later_xmic_rows() {
    let mut city = City::new_blank("/tmp/sc2000-school-order.sc2", "SchoolOrder").unwrap();
    let school_slot = usize::from(
        city.provision_microsim_for_building(214).unwrap() - super::MIN_SIM_TEXT_ENTRIES,
    );
    let police_slot = usize::from(
        city.provision_microsim_for_building(210).unwrap() - super::MIN_SIM_TEXT_ENTRIES,
    );
    assert!(school_slot < police_slot);
    // The school arm sums EQ_LE age cohorts 1 and 2, then divides by the
    // signed school-site count (minimum one) before its 0x0f jitter.
    // Force the conditional >1,500 branch so its 0xff draw must be
    // reflected in the later police record.
    city.pe_age_cohorts[1] = 1_000;
    city.pe_age_cohorts[2] = 1_000;

    let seed = super::Win95CrtRand::new(0x5c00_2140);
    let mut control = City::new_blank("/tmp/sc2000-school-control.sc2", "SchoolOrder").unwrap();
    let mut after_poll = seed;
    let mut game_rand = super::Win95GameRand::new(1);
    let mut lfsr = super::Win95Lfsr::new(1);
    control.refresh_structure_microsim_yearly_with_rngs(
        10_000,
        100,
        100,
        100,
        100,
        100,
        &mut after_poll,
        &mut game_rand,
        &mut lfsr,
    );

    let mut expected = after_poll;
    let students = 2_000 + u32::from(expected.random_u16_mask(0x0f));
    assert!(students > 1_500);
    let _extra_student_jitter = expected.random_u16_mask(0xff);
    let _teacher_jitter = expected.random_u16_mask(7);
    let police_arrests = expected.random_u16_mask(0x0f);

    let mut crt = seed;
    let mut game_rand = super::Win95GameRand::new(1);
    let mut lfsr = super::Win95Lfsr::new(1);
    city.refresh_structure_microsim_yearly_with_rngs(
        10_000,
        100,
        100,
        100,
        100,
        100,
        &mut crt,
        &mut game_rand,
        &mut lfsr,
    );

    assert_eq!(
        city.microsim_at(police_slot).map(|row| row.stat3),
        Some(u16::from(police_arrests)),
        "the later police row receives the roll after school enrollment's conditional draw and teacher jitter"
    );
    assert_eq!(
        crt, expected,
        "the school arm advances the shared PE CRT stream"
    );
    assert_eq!(
        city.microsim_at(school_slot).map(|row| row.stat1),
        Some(500)
    );
}

#[test]
fn yearly_college_extra_student_roll_shifts_later_xmic_rows() {
    let mut city = City::new_blank("/tmp/sc2000-college-order.sc2", "CollegeOrder").unwrap();
    let college_slot = usize::from(
        city.provision_microsim_for_building(217).unwrap() - super::MIN_SIM_TEXT_ENTRIES,
    );
    let police_slot = usize::from(
        city.provision_microsim_for_building(210).unwrap() - super::MIN_SIM_TEXT_ENTRIES,
    );
    assert!(college_slot < police_slot);
    // The PE college arm reads EQ_LE[3] before jitter. Force its >5,000
    // branch so the conditional 0x1ff draw must precede the later police row.
    city.pe_age_cohorts[3] = 6_000;

    let seed = super::Win95CrtRand::new(0x3141_5926);
    let mut control = City::new_blank("/tmp/sc2000-college-control.sc2", "CollegeOrder").unwrap();
    let mut after_poll = seed;
    let mut game_rand = super::Win95GameRand::new(1);
    let mut lfsr = super::Win95Lfsr::new(1);
    control.refresh_structure_microsim_yearly_with_rngs(
        10_000,
        100,
        100,
        100,
        100,
        100,
        &mut after_poll,
        &mut game_rand,
        &mut lfsr,
    );

    let mut expected = after_poll;
    let students = 6_000 + u32::from(expected.random_u16_mask(0x1f));
    assert!(students > 5_000);
    let _extra_student_jitter = expected.random_u16_mask(0x1ff);
    let _professor_jitter = expected.random_u16_mask(0x0f);
    let police_arrests = expected.random_u16_mask(0x0f);

    let mut crt = seed;
    let mut game_rand = super::Win95GameRand::new(1);
    let mut lfsr = super::Win95Lfsr::new(1);
    city.refresh_structure_microsim_yearly_with_rngs(
        10_000,
        100,
        100,
        100,
        100,
        100,
        &mut crt,
        &mut game_rand,
        &mut lfsr,
    );

    assert_eq!(
        city.microsim_at(police_slot).map(|row| row.stat3),
        Some(u16::from(police_arrests)),
        "the later police row receives the roll after both college student draws and professor jitter"
    );
    assert_eq!(
        crt, expected,
        "college's conditional draw advances the shared PE stream"
    );
    assert_eq!(
        city.microsim_at(college_slot).map(|row| row.stat3),
        Some(100),
        "college row stores the current funding dword as its low word"
    );
    assert_eq!(
        city.microsim_at(college_slot)
            .map(|row| (row.stat0, row.stat1, row.stat2, row.stat3)),
        Some((12, 333, 100, 100)),
        "college enrollment is capped by population/30, professor count by population/100, then graded"
    );
}

#[test]
fn yearly_college_preserves_pe_32bit_signed_boundaries() {
    let mut city = City::new_blank("/tmp/sc2000-college-width.sc2", "CollegeWidth").unwrap();
    let college_slot = usize::from(
        city.provision_microsim_for_building(217).unwrap() - super::MIN_SIM_TEXT_ENTRIES,
    );
    let police_slot = usize::from(
        city.provision_microsim_for_building(210).unwrap() - super::MIN_SIM_TEXT_ENTRIES,
    );
    assert!(college_slot < police_slot);

    // PE's DIV is unsigned, but the following `cmp eax, 5000; jle`
    // interprets the jittered dword as signed. Keep its high bit set so
    // the conditional 0x1ff draw must be skipped.
    city.pe_age_cohorts[3] = 0x8000_0000;
    let prior_grade = city.microsim_at(college_slot).unwrap().stat0;
    // The college body reads this raw dword. Doubling wraps to 100 before
    // the grade adjustment, unlike normal 0..=100 budget input.
    let funding = 0x8000_0032_u32;
    let seed = super::Win95CrtRand::new(0x41cc_8001);
    let mut control =
        City::new_blank("/tmp/sc2000-college-width-control.sc2", "CollegeWidth").unwrap();
    let mut after_poll = seed;
    let mut game_rand = super::Win95GameRand::new(1);
    let mut lfsr = super::Win95Lfsr::new(1);
    control.refresh_structure_microsim_yearly_with_rngs(
        10_000,
        100,
        100,
        100,
        100,
        100,
        &mut after_poll,
        &mut game_rand,
        &mut lfsr,
    );

    let mut expected = after_poll;
    let student_jitter = u32::from(expected.random_u16_mask(0x1f));
    let student_word = 0x8000_0000_u32.wrapping_add(student_jitter);
    assert!((student_word as i32) <= 5_000);
    let students = super::pe_microsim_scaled_capacity(10_000, 30, student_word);
    let professor_jitter = u32::from(expected.random_u16_mask(0xf));
    let professor_word = funding
        .wrapping_mul(2)
        .wrapping_add(professor_jitter)
        .wrapping_add(u32::from(prior_grade).wrapping_mul(4).wrapping_sub(0x30));
    assert!((professor_word as i32) >= 0);
    let professors = super::pe_microsim_scaled_capacity(10_000, 100, professor_word);
    let police_arrests = expected.random_u16_mask(0x0f);

    let mut crt = seed;
    let mut game_rand = super::Win95GameRand::new(1);
    let mut lfsr = super::Win95Lfsr::new(1);
    city.refresh_structure_microsim_yearly_with_rngs(
        10_000,
        100,
        100,
        100,
        100,
        funding,
        &mut crt,
        &mut game_rand,
        &mut lfsr,
    );

    assert_eq!(
        crt, expected,
        "the high-bit enrollment skips PE's extra jitter roll"
    );
    assert_eq!(
        city.microsim_at(police_slot).map(|row| row.stat3),
        Some(u16::from(police_arrests)),
        "the next XMIC row receives the CRT value after only the PE college draws"
    );
    assert_eq!(
        city.microsim_at(college_slot)
            .map(|row| (row.stat1, row.stat2, row.stat3)),
        Some((students as u16, professors as u16, funding as u16)),
        "PE stores low words from the signed-threshold enrollment and wrapping professor formula"
    );
}

#[test]
fn occupancy_site_counts_use_dw_tile_count_not_live_rescan() {
    // PE prologue reads `0x4CA4C8`, not a live XBLD walk.
    let mut city = City::new_blank("/tmp/sc2000-occ-census.sc2", "OccCensus").expect("blank");
    stamp(&mut city, 2, 2, 214, 3);
    assert_eq!(pe_occupancy_div9_count(&city, 214), 1);
    assert_eq!(city.pe_dw_tile_count[214], 9);
    city.pe_dw_tile_count[214] = 0;
    assert_eq!(
        pe_occupancy_div9_count(&city, 214),
        0,
        "stale zero census must ignore school tiles still on the map"
    );
    assert_eq!(city.building_at(2, 2), 214);
    city.pe_dw_tile_count[214] = 9;
    assert_eq!(pe_occupancy_div9_count(&city, 214), 1);
}

#[test]
fn occupancy_site_counts_use_tile_footprints_not_microsim_rows() {
    let mut city = City::new_blank("/tmp/sc2000-tile-count.sc2", "test").expect("blank city");
    stamp(&mut city, 2, 2, 214, 3); // one school
    stamp(&mut city, 8, 2, 209, 3); // one hospital
    stamp(&mut city, 14, 2, 217, 4); // one college
    stamp(&mut city, 20, 2, 216, 4); // one prison
    stamp(&mut city, 26, 2, 215, 4); // one stadium (16 tiles)
    stamp(&mut city, 32, 2, 210, 3); // one police (9 tiles)
    stamp(&mut city, 38, 2, 251, 4); // one plymouth arco (16 tiles)

    assert_eq!(pe_occupancy_div9_count(&city, 214), 1);
    assert_eq!(pe_occupancy_div9_count(&city, 209), 1);
    assert_eq!(pe_occupancy_div16_count(&city, 217), 1);
    assert_eq!(pe_occupancy_div16_count(&city, 216), 1);
    assert_eq!(pe_occupancy_tile_words(&city, &[215]), 16);
    assert_eq!(
        pe_occupancy_divisor(pe_occupancy_tile_words(&city, &[215])),
        16
    );
    assert_eq!(
        pe_occupancy_divisor(pe_occupancy_tile_words(&city, &[210])),
        9
    );
    assert_eq!(pe_occupancy_arco_footprints(&city), 1);
    assert_eq!(pe_occupancy_divisor(0), 1);

    stamp(&mut city, 2, 8, 214, 3); // second school
    assert_eq!(pe_occupancy_div9_count(&city, 214), 2);
}

#[test]
fn eq_le_month_counts_school_tiles_not_microsim_rows() {
    let mut city = City::new_blank("/tmp/sc2000-eq-tiles.sc2", "test").expect("blank city");
    city.provision_microsim_for_building(214)
        .expect("school row");
    assert_eq!(
        city.pe_eq_le_tile_site_counts(),
        (0, 0, 0),
        "XMIC without dwTileCount cells must not invent a school for aux caps"
    );
    stamp(&mut city, 2, 2, 214, 3);
    stamp(&mut city, 8, 2, 209, 3);
    stamp(&mut city, 14, 2, 217, 4);
    assert_eq!(city.pe_eq_le_tile_site_counts(), (1, 1, 1));
    stamp(&mut city, 2, 8, 214, 3);
    assert_eq!(city.pe_eq_le_tile_site_counts(), (2, 1, 1));
}

#[test]
fn power_plant_aging_failed_rebuild_uses_crt_rubble() {
    // PE @ `0x44e606`: age>50 without rebuild → DestroyBuilding CRT rubble, not bare 0.
    let mut city = City::new_blank("/tmp/sc2000-plant-age.sc2", "PlantAge").expect("blank");
    city.set_city_funds(0);
    for dx in 0..4 {
        for dy in 0..4 {
            city.set_building(40 + dx, 40 - dy, 207);
        }
    }
    let text = city.provision_microsim_for_building(207).unwrap();
    let index = usize::from(text - super::MIN_SIM_TEXT_ENTRIES);
    city.set_text_overlay(40, 40, text);
    assert!(city.set_microsim_stat0(index, 50));
    let mut crt = super::Win95CrtRand::new(0x44e6_0601);
    let mut control =
        City::new_blank("/tmp/sc2000-plant-age-control.sc2", "PlantAgeControl").unwrap();
    let mut expected_crt = crt;
    let mut game_rand = super::Win95GameRand::new(1);
    let mut expected_lfsr = super::Win95Lfsr::from_u32_seed(1);
    control.refresh_structure_microsim_yearly_with_rngs(
        10_000,
        100,
        100,
        100,
        100,
        100,
        &mut expected_crt,
        &mut game_rand,
        &mut expected_lfsr,
    );
    let _plant_power_roll = expected_crt.random_u16_mask(7);
    for _ in 0..super::pe_destroy_building_mode1_render_scatter_draw_count(207) {
        let _scatter = expected_crt.random_u15();
    }
    let mut expected_rubble = Vec::new();
    for dx in 0..4 {
        for dy in 0..4 {
            expected_rubble.push((40 + dx, 40 - dy, expected_crt.rubble_tile_id()));
        }
    }
    let mut lfsr = super::Win95Lfsr::from_u32_seed(1);
    let _ = city.refresh_structure_microsim_yearly_with_rng(
        10_000, 100, 100, 100, 100, 100, &mut crt, &mut lfsr,
    );
    assert_eq!(
        crt, expected_crt,
        "later XMIC slots see all PE destroy draws"
    );
    assert_eq!(city.microsim_at(index).map(|r| r.tile_id), Some(0));
    for dx in 0..4 {
        for dy in 0..4 {
            let b = city.building_at(40 + dx, 40 - dy);
            assert!(
                (1..=4).contains(&b),
                "footprint ({},{}) should be CRT rubble 1..=4, got {:#04x}",
                40 + dx,
                40 - dy,
                b
            );
            assert_eq!(city.text_overlay_at(40 + dx, 40 - dy), Some(0));
        }
    }
    for (x, y, rubble) in expected_rubble {
        assert_eq!(
            city.building_at(x, y),
            rubble,
            "PE PlaceTile order at ({x},{y})"
        );
    }
    assert_eq!(
        city.take_pe_pending_effect_sound_ids(),
        vec![crate::audio::SOUND_EXPLODE],
        "failed-rebuild demolish should queue explode sound 504"
    );
    assert_eq!(
        city.take_pe_pending_auto_goto(),
        vec![(40, 40)],
        "failed-rebuild demolish should queue AutoGoto at plant anchor"
    );
}

#[test]
fn power_plant_demolition_draws_rubble_only_for_flat_xter_cells() {
    // PE PlaceTile chooses rubble only when the current XTER byte is zero.
    // Mixed terrain pins both the cell writes and conditional CRT draws.
    let mut city =
        City::new_blank("/tmp/sc2000-plant-mixed-terrain.sc2", "PlantTerrain").expect("blank");
    city.set_city_funds(0);
    city.pe_w_city_rotation = 3;
    for dx in 0..4 {
        for dy in 0..4 {
            let x = 40 + dx;
            let y = 40 - dy;
            city.set_building(x, y, 207);
            city.set_terrain(x, y, u8::from((dx + dy) % 2 != 0));
        }
    }
    let text = city.provision_microsim_for_building(207).unwrap();
    let index = usize::from(text - super::MIN_SIM_TEXT_ENTRIES);
    city.set_zone(40, 37, 0x80);
    city.set_zone(43, 37, 0x10);
    city.set_zone(43, 40, 0x20);
    city.set_zone(40, 40, 0x40);
    // The first XTXT hit is at the low-Y corner. Cleanup walks from high Y.
    city.set_text_overlay(40, 37, text);
    city.set_building(40, 36, 0x1d);
    assert!(city.set_microsim_stat0(index, 50));

    let seed = super::Win95CrtRand::new(0x44e6_0604);
    let mut expected = seed;
    let mut control = City::new_blank("/tmp/sc2000-plant-mixed-terrain-control.sc2", "Control")
        .expect("blank control");
    let mut game_rand = super::Win95GameRand::new(1);
    let mut lfsr = super::Win95Lfsr::from_u32_seed(1);
    control.refresh_structure_microsim_yearly_with_rngs(
        10_000,
        100,
        100,
        100,
        100,
        100,
        &mut expected,
        &mut game_rand,
        &mut lfsr,
    );
    let _plant_power_roll = expected.random_u16_mask(7);
    for _ in 0..super::pe_destroy_building_mode1_render_scatter_draw_count(207) {
        let _scatter = expected.random_u15();
    }
    let mut expected_cells = Vec::new();
    for dx in 0..4 {
        for dy in 0..4 {
            let x = 40 + dx;
            let y = 40 - dy;
            let rubble = if (dx + dy) % 2 == 0 {
                expected.rubble_tile_id()
            } else {
                0
            };
            expected_cells.push((x, y, rubble));
        }
    }

    let mut crt = seed;
    let mut game_rand = super::Win95GameRand::new(1);
    let mut lfsr = super::Win95Lfsr::from_u32_seed(1);
    let _ = city.refresh_structure_microsim_yearly_with_rngs(
        10_000,
        100,
        100,
        100,
        100,
        100,
        &mut crt,
        &mut game_rand,
        &mut lfsr,
    );

    assert_eq!(crt, expected, "non-flat cells consume no rubble draw");
    assert_eq!(city.building_at(40, 36), 0x1d);
    let expired = city.microsim_at(index).unwrap();
    assert_eq!((expired.tile_id, expired.stat0), (0, 51));
    for (x, y, rubble) in expected_cells {
        assert_eq!(city.building_at(x, y), rubble, "PlaceTile at ({x},{y})");
        assert_eq!(city.text_overlay_at(x, y), Some(0));
    }
}

#[test]
fn power_plant_without_xmic_xtxt_anchor_skips_pe_demolition_call() {
    // PE `FUN_0044F890` returns zero when no XTXT byte equals 0x33+slot;
    // the caller branches to the yearly loop tail before checking rebuild
    // funds/no-disasters, DestroyBuilding, or clearing the XMIC tile byte.
    let mut city =
        City::new_blank("/tmp/sc2000-plant-no-anchor.sc2", "PlantNoAnchor").expect("blank");
    city.set_city_funds(100_000);
    city.set_misc_u32_at(super::MISC_NO_DISASTERS_OFFSET, 1);
    stamp(&mut city, 40, 40, 207, 4);
    let text = city.provision_microsim_for_building(207).unwrap();
    let index = usize::from(text - super::MIN_SIM_TEXT_ENTRIES);
    assert!(city.set_microsim_stat0(index, 50));
    assert_eq!(city.text_overlay_at(40, 40), Some(0));

    let seed = super::Win95CrtRand::new(0x44e6_0603);
    let mut control =
        City::new_blank("/tmp/sc2000-plant-no-anchor-control.sc2", "Control").expect("blank");
    let mut expected = seed;
    let mut game_rand = super::Win95GameRand::new(1);
    let mut lfsr = super::Win95Lfsr::new(1);
    control.refresh_structure_microsim_yearly_with_rngs(
        10_000,
        100,
        100,
        100,
        100,
        100,
        &mut expected,
        &mut game_rand,
        &mut lfsr,
    );
    let _plant_power_roll = expected.random_u16_mask(7);

    let mut crt = seed;
    let mut lfsr = super::Win95Lfsr::from_u32_seed(1);
    let _ = city.refresh_structure_microsim_yearly_with_rng(
        10_000, 100, 100, 100, 100, 100, &mut crt, &mut lfsr,
    );

    assert_eq!(city.microsim_at(index).map(|row| row.tile_id), Some(207));
    assert_eq!(city.microsim_at(index).map(|row| row.stat0), Some(51));
    assert_eq!(city.building_at(40, 40), 207);
    assert_eq!(city.city_funds(), 100_000);
    assert_eq!(
        crt, expected,
        "missing XTXT marker skips all DestroyBuilding draws"
    );
    assert!(city.take_pe_pending_effect_sound_ids().is_empty());
    assert!(city.take_pe_pending_auto_goto().is_empty());
}

#[test]
fn power_plant_age_increments_as_wrapping_pe_byte() {
    let mut city =
        City::new_blank("/tmp/sc2000-plant-age-wrap.sc2", "PlantAgeWrap").expect("blank city");
    stamp(&mut city, 40, 40, 207, 4);
    let text = city.provision_microsim_for_building(207).unwrap();
    let index = usize::from(text - super::MIN_SIM_TEXT_ENTRIES);
    for dy in 0..4 {
        for dx in 0..4 {
            city.set_text_overlay(40 + dx, 40 + dy, text);
        }
    }
    assert!(city.set_microsim_stat0(index, u8::MAX));

    let mut crt = super::Win95CrtRand::new(0x44e6_06ff);
    let mut lfsr = super::Win95Lfsr::from_u32_seed(1);
    let _ = city.refresh_structure_microsim_yearly_with_rng(
        10_000, 100, 100, 100, 100, 100, &mut crt, &mut lfsr,
    );

    assert_eq!(city.microsim_at(index).map(|row| row.stat0), Some(0));
    assert_eq!(city.microsim_at(index).map(|row| row.tile_id), Some(207));
    assert_eq!(city.building_at(40, 40), 207);
    assert!(city.take_pe_pending_effect_sound_ids().is_empty());
    assert!(city.take_pe_pending_auto_goto().is_empty());
}

#[test]
fn mayor_countdown_age_increments_as_wrapping_pe_byte() {
    let mut city =
        City::new_blank("/tmp/sc2000-mayor-age-wrap.sc2", "MayorAgeWrap").expect("blank city");
    let text = city.provision_microsim_for_building(243).expect("mayor");
    let index = usize::from(text - super::MIN_SIM_TEXT_ENTRIES);
    assert!(city.set_microsim_stat0(index, u8::MAX));
    assert!(city.set_microsim_stat3(index, 1));

    let mut crt = super::Win95CrtRand::new(0x44ee_bfff);
    let mut lfsr = super::Win95Lfsr::from_u32_seed(1);
    let _ = city.refresh_structure_microsim_yearly_with_rng(
        10_000, 100, 100, 100, 100, 100, &mut crt, &mut lfsr,
    );

    assert_eq!(
        city.microsim_at(index).map(|row| (row.stat0, row.stat3)),
        Some((0, 0))
    );
}

#[test]
fn power_plant_demolition_uses_first_xmic_xtxt_match_as_pe_does() {
    // PE FUN_0044F890 returns the first x-major XTXT match for the XMIC slot,
    // without preferring a cell whose XBLD byte equals the record's tile id.
    let mut city =
        City::new_blank("/tmp/sc2000-plant-slot-anchor.sc2", "PlantAnchor").expect("blank");
    city.set_city_funds(0);
    stamp(&mut city, 40, 40, 207, 4);
    city.set_building(40, 40, 206);
    let text = city.provision_microsim_for_building(207).unwrap();
    let index = usize::from(text - super::MIN_SIM_TEXT_ENTRIES);
    for dy in 0..4 {
        for dx in 0..4 {
            city.set_text_overlay(40 + dx, 40 + dy, text);
        }
    }
    assert!(city.set_microsim_stat0(index, 50));
    let mut crt = super::Win95CrtRand::new(0x44e6_0602);
    let mut lfsr = super::Win95Lfsr::from_u32_seed(1);
    let _ = city.refresh_structure_microsim_yearly_with_rng(
        10_000, 100, 100, 100, 100, 100, &mut crt, &mut lfsr,
    );
    assert_eq!(
        city.take_pe_pending_auto_goto(),
        vec![(40, 40)],
        "the PE locator chooses the first XTXT match in x-major order"
    );
}

#[test]
fn overlapping_plant_demolition_rechecks_and_releases_later_xmic_row() {
    let mut city = City::new_blank(
        "/tmp/sc2000-overlapping-plant-demolition.sc2",
        "PlantOverlap",
    )
    .expect("blank");
    city.set_city_funds(0);
    // Two intentionally overlapping 4×4 plants model a damaged/custom map.
    // The first plant's PE DestroyBuilding square contains the second
    // plant's first XTXT marker, so its resource release kills that row.
    stamp(&mut city, 30, 40, 207, 4);
    stamp(&mut city, 32, 40, 207, 4);
    let first_text = city.provision_microsim_for_building(207).unwrap();
    let second_text = city.provision_microsim_for_building(207).unwrap();
    let first = usize::from(first_text - super::MIN_SIM_TEXT_ENTRIES);
    let second = usize::from(second_text - super::MIN_SIM_TEXT_ENTRIES);
    for dy in 0..4 {
        for dx in 0..4 {
            city.set_text_overlay(30 + dx, 40 + dy, first_text);
        }
    }
    for dy in 0..4 {
        for dx in 0..4 {
            city.set_text_overlay(32 + dx, 40 + dy, second_text);
        }
    }
    assert!(city.set_microsim_stat0(first, 50));
    assert!(city.set_microsim_stat0(second, 50));

    let mut control = city.clone();
    control.pe_401dca_clear_xtxt_resources(second_text);
    let seed = super::Win95CrtRand::new(0x44e6_0602);
    let mut actual_crt = seed;
    let mut control_crt = seed;
    let mut actual_lfsr = super::Win95Lfsr::from_u32_seed(1);
    let mut control_lfsr = actual_lfsr;
    let _ = city.refresh_structure_microsim_yearly_with_rng(
        10_000,
        100,
        100,
        100,
        100,
        100,
        &mut actual_crt,
        &mut actual_lfsr,
    );
    let _ = control.refresh_structure_microsim_yearly_with_rng(
        10_000,
        100,
        100,
        100,
        100,
        100,
        &mut control_crt,
        &mut control_lfsr,
    );

    assert_eq!(city.microsim_at(second).map(|row| row.tile_id), Some(0));
    assert_eq!(city.take_pe_pending_auto_goto(), vec![(30, 40)]);
    assert_eq!(
        actual_crt, control_crt,
        "the erased later XMIC row consumes no age or demolition draws"
    );
}

#[test]
fn early_plant_demolition_skips_later_cross_class_xmic_slot() {
    let mut city = City::new_blank(
        "/tmp/sc2000-plant-clears-later-school.sc2",
        "PlantSchoolOverlap",
    )
    .expect("blank city");
    city.set_city_funds(0);
    stamp(&mut city, 30, 40, 207, 4);
    let plant_text = city.provision_microsim_for_building(207).unwrap();
    let plant = usize::from(plant_text - super::MIN_SIM_TEXT_ENTRIES);
    let school_text = city.provision_microsim_for_building(214).unwrap();
    let school = usize::from(school_text - super::MIN_SIM_TEXT_ENTRIES);
    assert!(plant < school, "fixture requires native slot order");
    city.set_text_overlay(30, 40, plant_text);
    // This deliberately malformed cross-class link lies in the plant's
    // valid 4x4 DestroyBuilding footprint. PE releases the later school
    // row before the annual slot walk reaches it.
    city.set_text_overlay(31, 40, school_text);
    assert!(city.set_microsim_stat0(plant, 50));
    assert!(city.set_microsim_stat0(school, 17));
    assert!(city.set_microsim_stat1(school, 111));
    assert!(city.set_microsim_stat2(school, 222));
    assert!(city.set_microsim_stat3(school, 333));

    let mut control = city.clone();
    control.pe_401dca_clear_xtxt_resources(school_text);
    let seed = super::Win95CrtRand::new(0x44e6_0602);
    let mut actual_crt = seed;
    let mut control_crt = seed;
    let mut actual_lfsr = super::Win95Lfsr::from_u32_seed(1);
    let mut control_lfsr = actual_lfsr;

    let _ = city.refresh_structure_microsim_yearly_with_rng(
        10_000,
        100,
        100,
        100,
        100,
        100,
        &mut actual_crt,
        &mut actual_lfsr,
    );
    let _ = control.refresh_structure_microsim_yearly_with_rng(
        10_000,
        100,
        100,
        100,
        100,
        100,
        &mut control_crt,
        &mut control_lfsr,
    );

    assert_eq!(city.microsim_at(school).map(|row| row.tile_id), Some(0));
    assert_eq!(
        actual_crt, control_crt,
        "the later school XMIC slot is released before dispatch and consumes no draws"
    );
    assert_eq!(
        actual_lfsr, control_lfsr,
        "the deleted later selector cannot alter subsequent app-LFSR slots"
    );
    assert_eq!(
        city.take_pe_pending_auto_goto(),
        vec![(30, 40)],
        "the earlier plant demolition remains the only applied side effect"
    );
}

#[test]
fn early_plant_demolition_skips_later_arcology_lfsr_slot() {
    let mut city = City::new_blank(
        "/tmp/sc2000-plant-clears-later-arcology.sc2",
        "PlantArcologyOverlap",
    )
    .expect("blank city");
    city.set_city_funds(0);
    stamp(&mut city, 30, 40, 207, 4);
    let plant_text = city.provision_microsim_for_building(207).unwrap();
    let plant = usize::from(plant_text - super::MIN_SIM_TEXT_ENTRIES);
    let arcology_text = city.provision_microsim_for_building(251).unwrap();
    let arcology = usize::from(arcology_text - super::MIN_SIM_TEXT_ENTRIES);
    assert!(plant < arcology, "fixture requires native slot order");
    city.set_text_overlay(30, 40, plant_text);
    // Deliberately malformed cross-class resource link: the plant's
    // DestroyBuilding square releases this later Arcology XMIC row.
    city.set_text_overlay(31, 40, arcology_text);
    assert!(city.set_microsim_stat0(plant, 50));
    assert!(city.set_microsim_stat1(arcology, 40));
    assert!(city.set_microsim_stat2(arcology, 1_234));

    let mut control = city.clone();
    control.pe_401dca_clear_xtxt_resources(arcology_text);
    let seed = super::Win95CrtRand::new(0x44e6_0602);
    let mut actual_crt = seed;
    let mut control_crt = seed;
    let mut actual_lfsr = super::Win95Lfsr::from_u32_seed(1);
    let mut control_lfsr = actual_lfsr;

    let _ = city.refresh_structure_microsim_yearly_with_rng(
        10_000,
        100,
        100,
        100,
        100,
        100,
        &mut actual_crt,
        &mut actual_lfsr,
    );
    let _ = control.refresh_structure_microsim_yearly_with_rng(
        10_000,
        100,
        100,
        100,
        100,
        100,
        &mut control_crt,
        &mut control_lfsr,
    );

    assert_eq!(city.microsim_at(arcology).map(|row| row.tile_id), Some(0));
    assert_eq!(
        actual_crt, control_crt,
        "the later Arcology row adds no unrelated CRT draws"
    );
    assert_eq!(
        actual_lfsr, control_lfsr,
        "the released Arcology row consumes no later LFSR draw"
    );
    assert_eq!(
        city.take_pe_pending_auto_goto(),
        vec![(30, 40)],
        "the earlier plant demolition remains the only applied side effect"
    );
}

#[test]
fn power_plant_aging_warning_year_queues_news_not_demolish_packaging() {
    // Age 49 (>48, ≤50): story 0x24 only — no explode / AutoGoto.
    let mut city = City::new_blank("/tmp/sc2000-plant-warn.sc2", "PlantWarn").expect("blank");
    city.set_city_funds(0);
    stamp(&mut city, 40, 40, 207, 4);
    let text = city.provision_microsim_for_building(207).unwrap();
    let index = usize::from(text - super::MIN_SIM_TEXT_ENTRIES);
    for dy in 0..4 {
        for dx in 0..4 {
            city.set_text_overlay(40 + dx, 40 + dy, text);
        }
    }
    assert!(city.set_microsim_stat0(index, 48));
    let mut crt = super::Win95CrtRand::new(0x44e5_4101);
    let mut lfsr = super::Win95Lfsr::from_u32_seed(1);
    let _ = city.refresh_structure_microsim_yearly_with_rng(
        10_000, 100, 100, 100, 100, 100, &mut crt, &mut lfsr,
    );
    assert_eq!(city.microsim_at(index).map(|r| r.stat0), Some(49));
    assert_eq!(city.building_at(40, 40), 207);
    assert_eq!(
        city.pe_newspaper_story_id_at(0),
        Some(u32::from(super::PE_NEWSPAPER_STORY_POWER_PLANT_AGING))
    );
    assert!(
        city.take_pe_pending_effect_sound_ids().is_empty(),
        "warning year must not explode"
    );
    assert!(
        city.take_pe_pending_auto_goto().is_empty(),
        "warning year must not AutoGoto"
    );
}

#[test]
fn school_stat3_truncates_nonnegative_funding_like_pe() {
    let mut city =
        City::new_blank("/tmp/sc2000-school-funding-round.sc2", "SchoolRound").expect("blank city");
    stamp(&mut city, 20, 20, 214, 3);
    let text = city.provision_microsim_for_building(214).unwrap();
    for dy in 0..3 {
        for dx in 0..3 {
            city.set_text_overlay(20 + dx, 20 + dy, text);
        }
    }
    let school = usize::from(text - super::MIN_SIM_TEXT_ENTRIES);
    let mut crt = super::Win95CrtRand::new(0x44e9_7801);
    let mut lfsr = super::Win95Lfsr::from_u32_seed(1);

    let _ = city.refresh_structure_microsim_yearly_with_rng(
        10_000, 100, 100, 100, 5, 100, &mut crt, &mut lfsr,
    );
    assert_eq!(city.microsim_at(school).map(|row| row.stat3), Some(1));

    let _ = city.refresh_structure_microsim_yearly_with_rng(
        10_000, 100, 100, 100, 10, 100, &mut crt, &mut lfsr,
    );
    assert_eq!(city.microsim_at(school).map(|row| row.stat3), Some(2));

    // PE wraps the signed dword adjustment before SAR, then stores AX.
    // PE stores the low word of the positive dword quotient.
    let _ = city.refresh_structure_microsim_yearly_with_rng(
        10_000,
        100,
        100,
        100,
        i32::MAX as u32,
        100,
        &mut crt,
        &mut lfsr,
    );
    assert_eq!(
        city.microsim_at(school).map(|row| row.stat3),
        Some(u16::MAX)
    );
}

#[test]
fn school_funding_and_teacher_candidates_keep_pe_dword_semantics() {
    assert_eq!(super::pe_school_funding_quarters(5), 1);
    assert_eq!(super::pe_school_funding_quarters(10), 2);
    assert_eq!(super::pe_school_funding_quarters(0x7fff_ffff), u16::MAX);
    assert_eq!(super::pe_school_funding_quarters(u32::MAX), 0);
    assert_eq!(super::pe_school_funding_quarters(0xffff_fffc), u16::MAX);
    assert_eq!(super::pe_school_funding_quarters(0x8000_0004), 1);

    assert_eq!(super::pe_school_teachers_raw(100, 7, 48), 103);
    // `funding * 6` wraps to signed -6 before IDIV, which truncates to 0.
    assert_eq!(super::pe_school_teachers_raw(0x7fff_ffff, 7, 48), 43);
    // The wrapped product is i32::MIN, so JNS takes the zero path.
    assert_eq!(super::pe_school_teachers_raw(0x4000_0000, 7, 48), 0);
}

#[test]
fn school_underfunding_queues_fund_education_newspaper() {
    // OpenSC2K services.gd: school_funding/4 < 20 → NEWS_EDUCATION `0x26`.
    let mut city = City::new_blank("/tmp/sc2000-school-fund.sc2", "SchoolFund").expect("blank");
    stamp(&mut city, 20, 20, 214, 3);
    let text = city.provision_microsim_for_building(214).unwrap();
    for dy in 0..3 {
        for dx in 0..3 {
            city.set_text_overlay(20 + dx, 20 + dy, text);
        }
    }
    let mut crt = super::Win95CrtRand::new(0x44e9_7801);
    let mut lfsr = super::Win95Lfsr::from_u32_seed(1);
    let _ = city.refresh_structure_microsim_yearly_with_rng(
        10_000, 100, 100, 100, 77, // truncating /4 = 19 < 20; ceil would be 20
        100, &mut crt, &mut lfsr,
    );
    assert_eq!(
        city.pe_newspaper_story_id_at(0),
        Some(u32::from(super::PE_NEWSPAPER_STORY_FUND_EDUCATION))
    );

    let mut city_ok = City::new_blank("/tmp/sc2000-school-fund-ok.sc2", "SchoolOk").expect("blank");
    stamp(&mut city_ok, 20, 20, 214, 3);
    let text = city_ok.provision_microsim_for_building(214).unwrap();
    for dy in 0..3 {
        for dx in 0..3 {
            city_ok.set_text_overlay(20 + dx, 20 + dy, text);
        }
    }
    let mut crt = super::Win95CrtRand::new(0x44e9_7802);
    let mut lfsr = super::Win95Lfsr::from_u32_seed(1);
    let _ = city_ok.refresh_structure_microsim_yearly_with_rng(
        10_000, 100, 100, 100, 80, // /4 = 20 — threshold refuses
        100, &mut crt, &mut lfsr,
    );
    assert_ne!(
        city_ok.pe_newspaper_story_id_at(0),
        Some(u32::from(super::PE_NEWSPAPER_STORY_FUND_EDUCATION)),
        "funding/4 == 20 must not queue fundeducation"
    );
}

#[test]
fn prison_overcrowding_queues_fundprison_newspaper() {
    // OpenSC2K services.gd: prison_stat > 105 → NEWS_PRISON `0x25` (only after 10k clamp).
    let mut city = City::new_blank("/tmp/sc2000-prison-crowd.sc2", "PrisonCrowd").expect("blank");
    stamp(&mut city, 20, 20, 216, 4);
    let text = city.provision_microsim_for_building(216).unwrap();
    for dy in 0..4 {
        for dx in 0..4 {
            city.set_text_overlay(20 + dx, 20 + dy, text);
        }
    }
    city.set_pe_w_old_arrests(20_000);
    // Force the >10k clamp arm; CRT mask must leave inmate_hundreds > 105.
    let mut found = false;
    for seed in 0x44eb_8e00u32..0x44eb_8e80 {
        let mut trial = city.clone();
        let mut crt = super::Win95CrtRand::new(seed);
        let mut lfsr = super::Win95Lfsr::from_u32_seed(1);
        let _ = trial.refresh_structure_microsim_yearly_with_rng(
            10_000, 100, 100, 100, 100, 100, &mut crt, &mut lfsr,
        );
        if trial.pe_newspaper_story_id_at(0)
            == Some(u32::from(super::PE_NEWSPAPER_STORY_PRISON_OVERCROWDED))
        {
            found = true;
            break;
        }
    }
    assert!(
        found,
        "expected a CRT seed that overcrowds past prison_stat 105"
    );

    let mut city_ok = City::new_blank("/tmp/sc2000-prison-ok.sc2", "PrisonOk").expect("blank");
    stamp(&mut city_ok, 20, 20, 216, 4);
    let text = city_ok.provision_microsim_for_building(216).unwrap();
    for dy in 0..4 {
        for dx in 0..4 {
            city_ok.set_text_overlay(20 + dx, 20 + dy, text);
        }
    }
    city_ok.set_pe_w_old_arrests(50);
    let mut crt = super::Win95CrtRand::new(0x44eb_8e01);
    let mut lfsr = super::Win95Lfsr::from_u32_seed(1);
    let _ = city_ok.refresh_structure_microsim_yearly_with_rng(
        10_000, 100, 100, 100, 100, 100, &mut crt, &mut lfsr,
    );
    assert_ne!(
        city_ok.pe_newspaper_story_id_at(0),
        Some(u32::from(super::PE_NEWSPAPER_STORY_PRISON_OVERCROWDED)),
        "low arrests must not queue fundprison"
    );
}

#[test]
fn yearly_prison_inmate_decay_uses_floor_quarter_like_pe() {
    let mut base = City::new_blank("/tmp/sc2000-prison-decay.sc2", "PrisonDecay").expect("blank");
    stamp(&mut base, 20, 20, 216, 4);
    let text = base.provision_microsim_for_building(216).unwrap();
    let index = usize::from(text - super::MIN_SIM_TEXT_ENTRIES);
    for dy in 0..4 {
        for dx in 0..4 {
            base.set_text_overlay(20 + dx, 20 + dy, text);
        }
    }

    // PE at 0x44eb99..0x44eba4 zero-extends the word before `cdq`, so
    // its signed-division correction is zero and the quarter is floored.
    // Zero arrests isolate that operation from intake and the >10,000 arm.
    for (previous, expected) in [(1, 1), (2, 2), (3, 3), (4, 3), (5, 4), (6, 5)] {
        let mut city = base.clone();
        assert!(city.set_microsim_stat1(index, previous));
        city.set_pe_w_old_arrests(0);
        let mut crt = super::Win95CrtRand::new(0x44eb_8e00);
        let mut game_rand = super::Win95GameRand::new(1);
        let mut lfsr = super::Win95Lfsr::new(1);

        city.refresh_structure_microsim_yearly_with_rngs(
            10_000,
            100,
            100,
            100,
            100,
            100,
            &mut crt,
            &mut game_rand,
            &mut lfsr,
        );

        assert_eq!(
            city.microsim_at(index).map(|row| row.stat1),
            Some(expected),
            "previous inmates {previous} must decay to {expected} before capacity"
        );
    }
}

#[test]
fn yearly_prison_grade_uses_raw_pe_unsigned_divisor_for_overfunding() {
    let mut city =
        City::new_blank("/tmp/sc2000-prison-overfunding.sc2", "PrisonFunding").expect("blank city");
    stamp(&mut city, 20, 20, 216, 4);
    let text = city.provision_microsim_for_building(216).unwrap();
    let index = usize::from(text - super::MIN_SIM_TEXT_ENTRIES);
    city.set_text_overlay(20, 20, text);
    assert!(city.set_microsim_stat1(index, 12_133));
    city.set_pe_w_old_arrests(0);

    // PE decay yields 9,100, so stat3=91. Funding 120 gives span
    // 1 + trunc((100-120)/10) = -1, then DIV uses 0xffffffff.
    let seed = super::Win95CrtRand::new(0x44ec_3a01);
    let mut control = city.clone();
    control.microsims.as_mut().unwrap()[index].tile_id = 0;
    let mut expected_crt = seed;
    let mut game_rng = super::Win95GameRand::new(1);
    let mut lfsr = super::Win95Lfsr::new(1);
    control.refresh_structure_microsim_yearly_with_rngs(
        100_000,
        120,
        100,
        100,
        100,
        100,
        &mut expected_crt,
        &mut game_rng,
        &mut lfsr,
    );
    let expected_grade = expected_crt.random_u15() as u8;

    let mut zero_divisor_city = city.clone();
    let mut actual_crt = seed;
    let mut game_rng = super::Win95GameRand::new(1);
    let mut lfsr = super::Win95Lfsr::new(1);
    city.refresh_structure_microsim_yearly_with_rngs(
        100_000,
        120,
        100,
        100,
        100,
        100,
        &mut actual_crt,
        &mut game_rng,
        &mut lfsr,
    );

    let row = city.microsim_at(index).unwrap();
    assert_eq!(row.stat2, 360, "guard cap uses raw funding × 3");
    assert_eq!(row.stat3, 91);
    assert_eq!(row.stat0, expected_grade);
    assert_eq!(actual_crt, expected_crt, "the branch consumes one CRT draw");

    // Funding 110 produces span zero. PE faults after the draw; CityGame
    // preserves the draw and leaves a safe zero grade instead.
    let mut zero_divisor_crt = seed;
    let mut game_rng = super::Win95GameRand::new(1);
    let mut lfsr = super::Win95Lfsr::new(1);
    zero_divisor_city.refresh_structure_microsim_yearly_with_rngs(
        100_000,
        110,
        100,
        100,
        100,
        100,
        &mut zero_divisor_crt,
        &mut game_rng,
        &mut lfsr,
    );
    let zero_row = zero_divisor_city.microsim_at(index).unwrap();
    assert_eq!(zero_row.stat3, 91);
    assert_eq!(zero_row.stat0, 0, "zero divisor uses the safe fallback");
    assert_eq!(
        zero_divisor_crt, actual_crt,
        "both branches consume one CRT draw"
    );
}

#[test]
fn yearly_service_selectors_use_raw_saved_funding() {
    let mut raw_city = City::new_blank("/tmp/sc2000-yearly-raw-funding.sc2", "RawYearlyFunding")
        .expect("blank city");
    let police_slot = usize::from(
        raw_city.provision_microsim_for_building(210).unwrap() - super::MIN_SIM_TEXT_ENTRIES,
    );
    let fire_slot = usize::from(
        raw_city.provision_microsim_for_building(211).unwrap() - super::MIN_SIM_TEXT_ENTRIES,
    );
    let hospital_slot = usize::from(
        raw_city.provision_microsim_for_building(209).unwrap() - super::MIN_SIM_TEXT_ENTRIES,
    );
    let school_slot = usize::from(
        raw_city.provision_microsim_for_building(214).unwrap() - super::MIN_SIM_TEXT_ENTRIES,
    );
    let college_slot = usize::from(
        raw_city.provision_microsim_for_building(217).unwrap() - super::MIN_SIM_TEXT_ENTRIES,
    );
    let museum_slot = usize::from(
        raw_city.provision_microsim_for_building(212).unwrap() - super::MIN_SIM_TEXT_ENTRIES,
    );
    let library_slot = usize::from(
        raw_city.provision_microsim_for_building(245).unwrap() - super::MIN_SIM_TEXT_ENTRIES,
    );
    raw_city.pe_dw_tile_count[212] = 9;
    raw_city.pe_dw_tile_count[245] = 9;
    let mut bounded_city = raw_city.clone();

    let mut raw_crt = super::Win95CrtRand::new(0x1357_2468);
    let mut raw_game_rand = super::Win95GameRand::new(1);
    let mut raw_lfsr = super::Win95Lfsr::new(1);
    raw_city.refresh_structure_microsim_yearly_with_rngs(
        100_000,
        120,
        120,
        120,
        120,
        120,
        &mut raw_crt,
        &mut raw_game_rand,
        &mut raw_lfsr,
    );

    let mut bounded_crt = super::Win95CrtRand::new(0x1357_2468);
    let mut bounded_game_rand = super::Win95GameRand::new(1);
    let mut bounded_lfsr = super::Win95Lfsr::new(1);
    bounded_city.refresh_structure_microsim_yearly_with_rngs(
        100_000,
        100,
        100,
        100,
        100,
        100,
        &mut bounded_crt,
        &mut bounded_game_rand,
        &mut bounded_lfsr,
    );

    let police = raw_city.microsim_at(police_slot).expect("police row");
    assert_eq!(police.stat0, 120, "police stat0 stores the raw low byte");
    assert_eq!(police.stat1, 240, "police capacity uses raw funding × 2");
    let fire = raw_city.microsim_at(fire_slot).expect("fire row");
    assert_eq!(fire.stat0, 120, "fire stat0 stores the raw low byte");
    assert_eq!(
        fire.stat1, 60,
        "fire capacity uses raw funding shifted by one"
    );
    let hospital = raw_city.microsim_at(hospital_slot).expect("hospital row");
    assert_eq!(
        hospital.stat3, 60,
        "hospital truncates the signed funding half-field like PE"
    );
    let school = raw_city.microsim_at(school_slot).expect("school row");
    assert_eq!(
        school.stat3, 30,
        "school truncates the signed funding quarter-field like PE"
    );
    let college = raw_city.microsim_at(college_slot).expect("college row");
    assert_eq!(
        college.stat3, 120,
        "college stores the raw funding low word"
    );
    let museum = raw_city.microsim_at(museum_slot).expect("museum row");
    assert_eq!(museum.stat1, 4_320, "museum cap uses raw college funding");
    assert_eq!(museum.stat2, 108, "museum display uses raw college funding");
    let library = raw_city.microsim_at(library_slot).expect("library row");
    assert_eq!(library.stat1, 4_320, "library cap uses raw school funding");
    assert_eq!(library.stat2, 630, "library books use raw school funding");

    assert_eq!(
        raw_city.pe_mayor_approval, bounded_city.pe_mayor_approval,
        "the opinion poll still sees service funding clamped to 100%"
    );
}

#[test]
fn yearly_fire_capacity_uses_signed_low_word_of_raw_half_funding() {
    let mut city = City::new_blank(
        "/tmp/sc2000-fire-funding-word-wrap.sc2",
        "FireFundingWordWrap",
    )
    .expect("blank city");
    let fire_slot = usize::from(
        city.provision_microsim_for_building(211).unwrap() - super::MIN_SIM_TEXT_ENTRIES,
    );

    let mut crt = super::Win95CrtRand::new(0x1357_2468);
    let mut game_rng = super::Win95GameRand::new(1);
    let mut lfsr = super::Win95Lfsr::new(1);
    city.refresh_structure_microsim_yearly_with_rngs(
        14_000,
        100,
        0x0001_0002,
        100,
        100,
        100,
        &mut crt,
        &mut game_rng,
        &mut lfsr,
    );

    let fire = city.microsim_at(fire_slot).expect("fire row");
    // PE arithmetic-shifts the raw funding DWORD, then FUN_0044f7d0
    // compares the low cap WORD as signed. 0x10002 >> 1 is 0x8001, a
    // negative cap WORD, so the helper returns that low word (32769)
    // instead of the population quotient (14,000 / 70 = 200).
    assert_eq!(fire.stat0, 2, "stat0 stores the raw funding low byte");
    assert_eq!(fire.stat1, 0x8001, "capacity preserves the PE cap word");
    assert_eq!(fire.stat2, 0x0801, "trucks derive from stored capacity");
}

#[test]
fn yearly_police_stat_words_keep_low_bits_but_arrest_total_saturates() {
    let mut city =
        City::new_blank("/tmp/sc2000-police-word-wrap.sc2", "PoliceWordWrap").expect("blank city");
    let police_slot = usize::from(
        city.provision_microsim_for_building(210).unwrap() - super::MIN_SIM_TEXT_ENTRIES,
    );
    // PE uses `mov [stat2],si` and `mov [stat3],cx`: the record fields
    // narrow to WORD even when crimes or arrests exceed 0xffff.
    city.set_pe_attitude_demand_inputs(0, 0, 0, 5 * 65_536);

    // The live year-roll poll runs before XMIC dispatch. Use an otherwise
    // identical inactive-row fixture to get the exact CRT state at the
    // police row, then predict the one low-nibble arrest draw.
    let mut control = city.clone();
    control.microsims.as_mut().unwrap()[police_slot].tile_id = 0;
    let seed = super::Win95CrtRand::new(0x1357_2468);
    let mut after_poll = seed;
    let mut game_rng = super::Win95GameRand::new(1);
    let mut lfsr = super::Win95Lfsr::new(1);
    control.refresh_structure_microsim_yearly_with_rngs(
        0,
        100,
        100,
        100,
        100,
        100,
        &mut after_poll,
        &mut game_rng,
        &mut lfsr,
    );
    let expected_arrest_word = after_poll.random_u16_mask(0x0f);

    let mut crt = seed;
    let mut game_rng = super::Win95GameRand::new(1);
    let mut lfsr = super::Win95Lfsr::new(1);
    city.refresh_structure_microsim_yearly_with_rngs(
        0,
        100,
        100,
        100,
        100,
        100,
        &mut crt,
        &mut game_rng,
        &mut lfsr,
    );

    let police = city.microsim_at(police_slot).expect("police row");
    assert_eq!(police.stat2, 0, "327,680 crime units narrow modulo 65,536");
    assert_eq!(
        police.stat3, expected_arrest_word,
        "65,536 plus jitter narrows to the low-word jitter"
    );
    assert_eq!(
        city.pe_w_old_arrests(),
        u16::MAX,
        "the separate annual arrest subtotal saturates at 0xffff"
    );
}

#[test]
fn yearly_police_arrest_divisor_sign_extends_saved_prison_bonus_word() {
    let mut city =
        City::new_blank("/tmp/sc2000-police-signed-bonus.sc2", "PoliceBonus").expect("blank city");
    let police_slot = usize::from(
        city.provision_microsim_for_building(210).unwrap() - super::MIN_SIM_TEXT_ENTRIES,
    );
    city.set_building(20, 20, 210);
    city.set_pe_attitude_demand_inputs(0, 0, 0, 120);
    city.set_pe_w_prison_bonus(u16::MAX); // PE sign-extends this as -1.

    let seed = super::Win95CrtRand::new(0x1357_2468);
    let mut expected_rng = seed;
    let expected_divisor = (5 - i32::from(u16::MAX as i16)).max(1) as u32;
    let expected_arrests = 120 / expected_divisor + u32::from(expected_rng.random_u16_mask(0x0f));

    let mut actual_rng = seed;
    let mut game_rng = super::Win95GameRand::new(1);
    let mut lfsr = super::Win95Lfsr::new(1);
    city.refresh_structure_microsim_yearly_with_rngs(
        0,
        100,
        100,
        100,
        100,
        100,
        &mut actual_rng,
        &mut game_rng,
        &mut lfsr,
    );

    assert_eq!(
        city.microsim_at(police_slot).unwrap().stat3,
        expected_arrests as u16,
        "signed -1 bonus makes the native divisor six"
    );
    assert_eq!(actual_rng, expected_rng);
}

#[test]
fn yearly_park_arm_does_not_activate_for_small_parks_without_xmic_record() {
    let mut city =
        City::new_blank("/tmp/sc2000-small-parks-no-xmic.sc2", "SmallParks").expect("blank");
    stamp(&mut city, 20, 20, 13, 3);
    let mut crt = super::Win95CrtRand::new(0x2130_0001);
    let mut game_rand = super::Win95GameRand::new(1);
    let mut lfsr = super::Win95Lfsr::new(1);

    city.refresh_structure_microsim_yearly_with_rngs(
        7_000,
        100,
        100,
        100,
        100,
        100,
        &mut crt,
        &mut game_rand,
        &mut lfsr,
    );

    assert_eq!(
        city.microsim_at(super::MICROSIM_SYSTEM_PARK),
        Some(super::MicrosimRecord::default()),
        "PE enters the park selector only for an existing XMIC record; small park tile 0x0d has no class/provisioned XMIC row"
    );
}

#[test]
fn yearly_fixed_system_arms_require_their_pe_xmic_tile_id() {
    let mut city =
        City::new_blank("/tmp/sc2000-fixed-systems-no-xmic.sc2", "NoSystemRows").expect("blank");
    stamp(&mut city, 10, 10, 212, 3); // museum
    stamp(&mut city, 20, 10, 245, 2); // library
    stamp(&mut city, 30, 10, 213, 3); // large park
    stamp(&mut city, 40, 10, 248, 3); // marina
    stamp(&mut city, 50, 10, 236, 2); // bus depot
    stamp(&mut city, 60, 10, 237, 2); // rail station
    stamp(&mut city, 70, 10, 233, 1); // subway station

    let system_slots = [
        super::MICROSIM_SYSTEM_MUSEUM,
        super::MICROSIM_SYSTEM_LIBRARY,
        super::MICROSIM_SYSTEM_PARK,
        super::MICROSIM_SYSTEM_MARINA,
        super::MICROSIM_SYSTEM_BUS,
        super::MICROSIM_SYSTEM_RAIL,
        super::MICROSIM_SYSTEM_SUBWAY,
    ];
    let before: Vec<_> = system_slots
        .iter()
        .map(|&slot| city.microsim_at(slot).expect("system slot"))
        .collect();
    let mut crt = super::Win95CrtRand::new(0x7130_0001);
    let mut game_rand = super::Win95GameRand::new(1);
    let mut lfsr = super::Win95Lfsr::new(0x2468);
    let lfsr_before = lfsr;

    city.refresh_structure_microsim_yearly_with_rngs(
        7_000,
        100,
        100,
        100,
        100,
        100,
        &mut crt,
        &mut game_rand,
        &mut lfsr,
    );

    let after: Vec<_> = system_slots
        .iter()
        .map(|&slot| city.microsim_at(slot).expect("system slot"))
        .collect();
    assert_eq!(after, before, "tile counts do not dispatch PE XMIC arms");
    assert_eq!(
        lfsr, lfsr_before,
        "an absent marina row consumes no LFSR draw"
    );
}

#[test]
fn yearly_unmapped_xmic_selector_rows_preserve_fields_and_random_streams() {
    // The PE selector table sends these saved tile-byte bands to its
    // shared no-class-work epilogue at 0x44f0e5. They are live rows, but
    // the annual worker must only advance past them without writes/draws.
    let unmapped_ids = [
        0xdc, 0xdd, 0xde, 0xdf, 0xe0, 0xe1, 0xe2, 0xe3, 0xe4, 0xe5, 0xe6, 0xe7, 0xe8, 0xea, 0xeb,
        0xee, 0xef, 0xf0, 0xf1, 0xf2, 0xf6, 0xf7, 0xf9,
    ];
    let mut active = City::new_blank("/tmp/sc2000-unmapped-xmic-selectors.sc2", "NoClassWork")
        .expect("blank city");
    for (offset, tile_id) in unmapped_ids.into_iter().enumerate() {
        let index = super::MICROSIM_STRUCTURE_FIRST + offset;
        active.microsims.as_mut().unwrap()[index] = super::MicrosimRecord {
            tile_id,
            stat0: 0xa0 + offset as u8,
            stat1: 0x1234 + offset as u16,
            stat2: 0x5678 + offset as u16,
            stat3: 0x9abc + offset as u16,
        };
    }
    let before: Vec<_> = unmapped_ids
        .iter()
        .enumerate()
        .map(|(offset, _)| {
            active
                .microsim_at(super::MICROSIM_STRUCTURE_FIRST + offset)
                .expect("active XMIC row")
        })
        .collect();
    let mut inactive = active.clone();
    for offset in 0..unmapped_ids.len() {
        inactive.microsims.as_mut().unwrap()[super::MICROSIM_STRUCTURE_FIRST + offset].tile_id = 0;
    }

    let mut active_crt = super::Win95CrtRand::new(0x6a09_e667);
    let mut inactive_crt = active_crt;
    let mut active_game_rng = super::Win95GameRand::new(0xbb67_ae85);
    let mut inactive_game_rng = active_game_rng;
    let mut active_lfsr = super::Win95Lfsr::new(0x3c6e);
    let mut inactive_lfsr = active_lfsr;
    active.refresh_structure_microsim_yearly_with_rngs(
        7_000,
        100,
        100,
        100,
        100,
        100,
        &mut active_crt,
        &mut active_game_rng,
        &mut active_lfsr,
    );
    inactive.refresh_structure_microsim_yearly_with_rngs(
        7_000,
        100,
        100,
        100,
        100,
        100,
        &mut inactive_crt,
        &mut inactive_game_rng,
        &mut inactive_lfsr,
    );

    let after: Vec<_> = unmapped_ids
        .iter()
        .enumerate()
        .map(|(offset, _)| {
            active
                .microsim_at(super::MICROSIM_STRUCTURE_FIRST + offset)
                .expect("active XMIC row")
        })
        .collect();
    assert_eq!(after, before, "unmapped selector classes are no-op rows");
    assert_eq!(active_crt, inactive_crt, "unmapped rows consume no CRT");
    assert_eq!(
        active_game_rng, inactive_game_rng,
        "unmapped rows consume no game LCG values"
    );
    assert_eq!(active_lfsr, inactive_lfsr, "unmapped rows consume no LFSR");
}

#[test]
fn yearly_every_xmic_tag_below_selector_floor_preserves_fields_and_random_streams() {
    // The PE worker subtracts 0xc6 in a byte register and rejects the
    // unsigned results above 0x39. Every input 0x00..=0xc5 therefore
    // takes the shared no-class-work epilogue. There are 140 ordinary
    // XMIC slots, so cover this full byte range in bounded batches.
    let mut first_id = 0usize;
    while first_id < 0xc6 {
        let end_id = (first_id + super::MICROSIM_COUNT - super::MICROSIM_STRUCTURE_FIRST).min(0xc6);
        let below_floor_ids: Vec<u8> = (first_id..end_id).map(|id| id as u8).collect();
        let mut active =
            City::new_blank("/tmp/sc2000-xmic-below-floor.sc2", "BelowFloor").expect("blank city");
        for (offset, tile_id) in below_floor_ids.iter().copied().enumerate() {
            let index = super::MICROSIM_STRUCTURE_FIRST + offset;
            active.microsims.as_mut().unwrap()[index] = super::MicrosimRecord {
                tile_id,
                stat0: 0xb0u8.wrapping_add(tile_id),
                stat1: 0x2345u16.wrapping_add(u16::from(tile_id)),
                stat2: 0x6789u16.wrapping_add(u16::from(tile_id)),
                stat3: 0xabcdu16.wrapping_add(u16::from(tile_id)),
            };
        }
        let before: Vec<_> = below_floor_ids
            .iter()
            .enumerate()
            .map(|(offset, _)| {
                active
                    .microsim_at(super::MICROSIM_STRUCTURE_FIRST + offset)
                    .expect("below-floor XMIC row")
            })
            .collect();
        let mut inactive = active.clone();
        for offset in 0..below_floor_ids.len() {
            inactive.microsims.as_mut().unwrap()[super::MICROSIM_STRUCTURE_FIRST + offset]
                .tile_id = 0;
        }

        let mut active_crt = super::Win95CrtRand::new(0x510e_527f);
        let mut inactive_crt = active_crt;
        let mut active_game_rng = super::Win95GameRand::new(0x9b05_688c);
        let mut inactive_game_rng = active_game_rng;
        let mut active_lfsr = super::Win95Lfsr::new(0x1f83);
        let mut inactive_lfsr = active_lfsr;
        active.refresh_structure_microsim_yearly_with_rngs(
            7_000,
            100,
            100,
            100,
            100,
            100,
            &mut active_crt,
            &mut active_game_rng,
            &mut active_lfsr,
        );
        inactive.refresh_structure_microsim_yearly_with_rngs(
            7_000,
            100,
            100,
            100,
            100,
            100,
            &mut inactive_crt,
            &mut inactive_game_rng,
            &mut inactive_lfsr,
        );

        let after: Vec<_> = below_floor_ids
            .iter()
            .enumerate()
            .map(|(offset, _)| {
                active
                    .microsim_at(super::MICROSIM_STRUCTURE_FIRST + offset)
                    .expect("below-floor XMIC row")
            })
            .collect();
        assert_eq!(
            after,
            before,
            "below-floor selector tags {first_id:#04x}..{:#04x} are no-op rows",
            end_id - 1
        );
        assert_eq!(active_crt, inactive_crt, "below-floor rows consume no CRT");
        assert_eq!(
            active_game_rng, inactive_game_rng,
            "below-floor rows consume no game LCG values"
        );
        assert_eq!(
            active_lfsr, inactive_lfsr,
            "below-floor rows consume no LFSR"
        );
        first_id = end_id;
    }
}

#[test]
fn yearly_marina_capacity_preserves_signed_word_cap_boundary() {
    let mut city = City::new_blank("/tmp/sc2000-marina-cap-word.sc2", "MarinaCap").expect("blank");
    // PE reads the BSS dwTileCount WORD, so seed that input directly. A
    // synthetic count of 4,096 reaches the cap's signed-word boundary:
    // 4,096 × 8 + Game_Random(20) = 0x8000 + the draw.
    city.pe_dw_tile_count[248] = 4_096;
    city.provision_microsim_for_building(248)
        .expect("marina row");

    let seed = 0x2468;
    let mut expected_lfsr = super::Win95Lfsr::new(seed);
    let expected_cap = 0x8000_u16 + expected_lfsr.random_mod(0x14);
    let mut crt = super::Win95CrtRand::new(0x7130_0001);
    let mut game_rand = super::Win95GameRand::new(1);
    let mut lfsr = super::Win95Lfsr::new(seed);

    city.refresh_structure_microsim_yearly_with_rngs(
        7_000,
        100,
        100,
        100,
        100,
        100,
        &mut crt,
        &mut game_rand,
        &mut lfsr,
    );

    let marina = city
        .microsim_at(super::MICROSIM_SYSTEM_MARINA)
        .expect("marina row");
    assert_eq!(
        marina.stat1, expected_cap,
        "PE helper compares the cap as signed WORD and returns its low-word pattern"
    );
    assert_eq!(
        lfsr, expected_lfsr,
        "the active marina consumes one LFSR draw"
    );
}

#[test]
fn launch_arco_mass_destroy_when_tile_count_and_pop_gate() {
    // PE @ `0x44f26a`: dwTileCount[0xfe]/16 > 300 and arco pop > 0x5b8d80
    // → DestroyBuilding mode1 every Launch tile; @ `0x44f2dd` funds += 100000*count.
    let mut city = City::new_blank("/tmp/sc2000-launch-wipe.sc2", "LaunchWipe").expect("blank");
    stamp(&mut city, 10, 10, 254, 4);
    let text = city.provision_microsim_for_building(254).unwrap();
    let index = usize::from(text - super::MIN_SIM_TEXT_ENTRIES);
    for dy in 0..4 {
        for dx in 0..4 {
            city.set_text_overlay(10 + dx, 10 + dy, text);
        }
    }
    // Fake census: 301 Launch footprints (PE reads dwTileCount, not live map).
    city.pe_dw_tile_count[254] = 301 * 16;
    city.set_city_funds(1_000);
    let mut crt = super::Win95CrtRand::new(0x44e6_6a01);
    city.pe_maybe_mass_destroy_launch_arcos_for_test(0x005b_8d80, &mut crt);
    assert_eq!(
        city.building_at(10, 10),
        254,
        "arco pop == 0x5b8d80 must jle refuse"
    );
    assert_eq!(city.city_funds(), 1_000, "refuse must not touch funds");
    assert!(
        city.take_pe_pending_effect_sound_ids().is_empty(),
        "refuse must not queue wipe sounds"
    );
    city.pe_maybe_mass_destroy_launch_arcos_for_test(0x005b_8d81, &mut crt);
    assert_eq!(city.microsim_at(index).map(|r| r.tile_id), Some(0));
    assert_eq!(
        city.pe_dw_tile_count[254],
        301 * 16 - 16,
        "PlaceTile census must dec dwTileCount[0xfe] for the wiped footprint"
    );
    for dy in 0..4 {
        for dx in 0..4 {
            let b = city.building_at(10 + dx, 10 + dy);
            assert!(
                (1..=4).contains(&b),
                "Launch ({},{}) should be CRT rubble, got {:#04x}",
                10 + dx,
                10 + dy,
                b
            );
        }
    }
    // One Launch microsim row → funds + 100,000.
    assert_eq!(city.city_funds(), 1_000i32.wrapping_add(100_000));
    // PE @ `0x44f29c` then `0x44f2f6`: `0x211` before wipe, `0x212` after funds.
    assert_eq!(
        city.take_pe_pending_effect_sound_ids(),
        vec![
            crate::audio::SOUND_LAUNCH_ARCO_WIPE_START,
            crate::audio::SOUND_LAUNCH_ARCO_WIPE_END
        ]
    );
}

#[test]
fn launch_arco_mass_destroy_funds_scales_with_microsim_count() {
    let mut city = City::new_blank("/tmp/sc2000-launch-funds.sc2", "LaunchFunds").expect("blank");
    stamp(&mut city, 10, 10, 254, 4);
    let t0 = city.provision_microsim_for_building(254).unwrap();
    for dy in 0..4 {
        for dx in 0..4 {
            city.set_text_overlay(10 + dx, 10 + dy, t0);
        }
    }
    stamp(&mut city, 20, 10, 254, 4);
    let t1 = city.provision_microsim_for_building(254).unwrap();
    for dy in 0..4 {
        for dx in 0..4 {
            city.set_text_overlay(20 + dx, 10 + dy, t1);
        }
    }
    // Keep an extra live map footprint without an XMIC row. The PE wipe
    // scans XBLD for demolition, but its cash multiplier comes from the
    // separate annual XMIC-row counter.
    stamp(&mut city, 30, 10, 254, 4);
    assert_eq!(city.structure_microsim_indices_for(254).len(), 2);
    city.pe_dw_tile_count[254] = 301 * 16;
    city.set_city_funds(0);
    let mut crt = super::Win95CrtRand::new(0x44e6_6a03);
    city.pe_maybe_mass_destroy_launch_arcos_for_test(0x005b_8d81, &mut crt);
    assert_eq!(city.city_funds(), 100_000i32.wrapping_mul(2));
    for origin_x in [10, 20, 30] {
        for dy in 0..4 {
            for dx in 0..4 {
                let tile = city.building_at(origin_x + dx, 10 + dy);
                assert!(
                    (1..=4).contains(&tile),
                    "Launch footprint at x={origin_x} must be wiped; got {tile:#04x}"
                );
            }
        }
    }
}

#[test]
fn launch_arco_mass_destroy_refuses_below_tile_count_gate() {
    let mut city = City::new_blank("/tmp/sc2000-launch-keep.sc2", "LaunchKeep").expect("blank");
    stamp(&mut city, 10, 10, 254, 4);
    city.pe_dw_tile_count[254] = 300 * 16; // jle refuse at ==300
    city.set_city_funds(50);
    let mut crt = super::Win95CrtRand::new(0x44e6_6a02);
    city.pe_maybe_mass_destroy_launch_arcos_for_test(0x005b_8d81, &mut crt);
    assert_eq!(city.building_at(10, 10), 254);
    assert_eq!(city.city_funds(), 50);
    assert!(
        city.take_pe_pending_effect_sound_ids().is_empty(),
        "tile-count refuse must not queue wipe sounds"
    );
}

#[test]
fn launch_arco_mass_destroy_uses_pe_x_major_scan_order() {
    fn fixture() -> City {
        let mut city =
            City::new_blank("/tmp/sc2000-launch-order.sc2", "LaunchOrder").expect("blank");
        // Arrange the two footprints so PE's X-major scan visits the lower
        // footprint first, while a transposed Y-major scan visits the other.
        for (origin_x, origin_y) in [(30, 10), (10, 30)] {
            stamp(&mut city, origin_x, origin_y, 254, 4);
            let text = city.provision_microsim_for_building(254).unwrap();
            for dy in 0..4 {
                for dx in 0..4 {
                    city.set_text_overlay(origin_x + dx, origin_y + dy, text);
                }
            }
        }
        city.pe_dw_tile_count[254] = 301 * 16;
        city
    }

    fn snapshot(city: &City) -> Vec<u8> {
        let mut tiles = Vec::with_capacity(CITY_WIDTH * CITY_HEIGHT);
        for x in 0..CITY_WIDTH {
            for y in 0..CITY_HEIGHT {
                tiles.push(city.building_at(x, y));
            }
        }
        tiles
    }

    fn expected_scan(city: &mut City, rng: &mut super::Win95CrtRand, x_major: bool) {
        if x_major {
            for x in 0..CITY_WIDTH {
                for y in 0..CITY_HEIGHT {
                    if city.building_at(x, y) == 254 {
                        city.pe_destroy_building_mode1_tile_crt_rubble(x, y, rng);
                    }
                }
            }
        } else {
            for y in 0..CITY_HEIGHT {
                for x in 0..CITY_WIDTH {
                    if city.building_at(x, y) == 254 {
                        city.pe_destroy_building_mode1_tile_crt_rubble(x, y, rng);
                    }
                }
            }
        }
    }

    const SEED: u32 = 0x44f2_95db;
    let mut actual = fixture();
    let mut actual_rng = super::Win95CrtRand::new(SEED);
    actual.pe_maybe_mass_destroy_launch_arcos_for_test(0x005b_8d81, &mut actual_rng);

    let mut x_major = fixture();
    let mut x_major_rng = super::Win95CrtRand::new(SEED);
    expected_scan(&mut x_major, &mut x_major_rng, true);
    let mut y_major = fixture();
    let mut y_major_rng = super::Win95CrtRand::new(SEED);
    expected_scan(&mut y_major, &mut y_major_rng, false);

    assert_eq!(snapshot(&actual), snapshot(&x_major));
    assert_eq!(actual_rng.seed(), x_major_rng.seed());
    assert_ne!(snapshot(&x_major), snapshot(&y_major));
}

#[test]
fn initialize_microsim_seeds_pe_structure_stats() {
    let mut city = City::new_blank("/tmp/sc2000-init.sc2", "test").expect("blank city");
    city.set_saved_date(super::CityDate {
        year: 1955,
        month: 3,
    });

    let coal = usize::from(
        city.provision_microsim_for_building(207).unwrap() - super::MIN_SIM_TEXT_ENTRIES,
    );
    assert_eq!(
        city.microsim_at(coal),
        Some(super::MicrosimRecord {
            tile_id: 207,
            stat0: 0,
            stat1: 200,
            stat2: 0,
            stat3: 0,
        })
    );

    let school = usize::from(
        city.provision_microsim_for_building(214).unwrap() - super::MIN_SIM_TEXT_ENTRIES,
    );
    assert_eq!(
        city.microsim_at(school)
            .map(|r| (r.tile_id, r.stat0, r.stat1)),
        Some((214, 6, 0))
    );

    let statue = usize::from(
        city.provision_microsim_for_building(219).unwrap() - super::MIN_SIM_TEXT_ENTRIES,
    );
    assert_eq!(
        city.microsim_at(statue),
        Some(super::MicrosimRecord {
            tile_id: 219,
            stat0: 0,
            stat1: 1955,
            stat2: 0,
            stat3: 0,
        }),
        "PE tile DB dispatches to 0x44e00e, which writes only current year to stat1"
    );

    let police = usize::from(
        city.provision_microsim_for_building(210).unwrap() - super::MIN_SIM_TEXT_ENTRIES,
    );
    assert_eq!(
        city.microsim_at(police)
            .map(|r| (r.tile_id, r.stat0, r.stat1, r.stat2)),
        Some((210, 0, 0, 0)),
        "empty leftover pop: FUN_0044f7d0 returns 0"
    );

    let fire = usize::from(
        city.provision_microsim_for_building(211).unwrap() - super::MIN_SIM_TEXT_ENTRIES,
    );
    assert_eq!(
        city.microsim_at(fire).map(|r| (r.stat0, r.stat1, r.stat2)),
        Some((0, 0, 4)),
        "fire init writes 4 trucks; capacity still 0 at leftover 0"
    );

    let prison = usize::from(
        city.provision_microsim_for_building(216).unwrap() - super::MIN_SIM_TEXT_ENTRIES,
    );
    assert_eq!(
        city.microsim_at(prison)
            .map(|r| (r.stat1, r.stat2, r.stat3)),
        Some((0, 0, 0))
    );

    let plymouth = usize::from(
        city.provision_microsim_for_building(251).unwrap() - super::MIN_SIM_TEXT_ENTRIES,
    );
    assert_eq!(
        city.microsim_at(plymouth),
        Some(super::MicrosimRecord {
            tile_id: 251,
            stat0: 5,
            stat1: 55,
            stat2: 0,
            stat3: 1955,
        })
    );

    let mayor = usize::from(
        city.provision_microsim_for_building(243).unwrap() - super::MIN_SIM_TEXT_ENTRIES,
    );
    assert_eq!(
        city.microsim_at(mayor).map(|r| (r.stat1, r.stat2, r.stat3)),
        Some((1955, 21, 47)),
        "CRT seed 1: rand%30+10 and rand%60"
    );

    let llama = usize::from(
        city.provision_microsim_for_building(255).unwrap() - super::MIN_SIM_TEXT_ENTRIES,
    );
    assert_eq!(
        city.microsim_at(llama).map(|r| r.stat3),
        Some(0x29),
        "name != aus: rand & 0x3f"
    );

    city.persist_pe_day22_population_leftover(9_000);
    let police2 = usize::from(
        city.provision_microsim_for_building(210).unwrap() - super::MIN_SIM_TEXT_ENTRIES,
    );
    assert_eq!(
        city.microsim_at(police2).map(|r| r.stat1),
        Some(100),
        "leftover 9000 / 90, capped at funding*2=200"
    );
}

#[test]
fn mayor_and_llama_initializers_advance_the_injected_pe_crt_stream() {
    let mut mayor_city =
        City::new_blank("/tmp/sc2000-mayor-rng.sc2", "MayorRng").expect("blank city");
    mayor_city.set_saved_date(super::CityDate {
        year: 1987,
        month: 1,
    });
    let seed = 0x1234_5678;
    let mut expected_crt = super::Win95CrtRand::new(seed);
    let expected_stat2 = expected_crt.random_mod(0x1e) + 10;
    let expected_stat3 = expected_crt.random_mod(0x3c);
    let mut actual_crt = super::Win95CrtRand::new(seed);
    let mayor = usize::from(
        mayor_city
            .provision_microsim_for_building_with_crt(243, &mut actual_crt)
            .unwrap()
            - super::MIN_SIM_TEXT_ENTRIES,
    );
    let record = mayor_city.microsim_at(mayor).expect("Mayor's House row");
    assert_eq!(
        (record.stat1, record.stat2, record.stat3),
        (1987, expected_stat2, expected_stat3)
    );
    assert_eq!(
        actual_crt, expected_crt,
        "Mayor's House consumes two CRT draws"
    );

    let mut llama_city =
        City::new_blank("/tmp/sc2000-llama-rng.sc2", "LlamaRng").expect("blank city");
    let seed = 0x8765_4321;
    let mut expected_crt = super::Win95CrtRand::new(seed);
    let expected_stat3 = expected_crt.random_u16_mask(0x3f);
    let mut actual_crt = super::Win95CrtRand::new(seed);
    let llama = usize::from(
        llama_city
            .provision_microsim_for_building_with_crt(255, &mut actual_crt)
            .unwrap()
            - super::MIN_SIM_TEXT_ENTRIES,
    );
    assert_eq!(llama_city.microsim_at(llama).unwrap().stat3, expected_stat3);
    assert_eq!(
        actual_crt, expected_crt,
        "non-aus Llama Dome consumes one CRT draw"
    );

    let mut aus_city = City::new_blank("/tmp/sc2000-llama-aus-rng.sc2", "aus").expect("blank city");
    let mut actual_crt = super::Win95CrtRand::new(seed);
    aus_city.provision_microsim_for_building_with_crt(255, &mut actual_crt);
    assert_eq!(actual_crt.seed(), seed, "the `aus` path skips rand()");
}

fn stamp_service_funding(city: &mut City, service_index: usize, funding: u32) {
    let row = super::MISC_BUDGET_SERVICE_ROW_START + service_index;
    let start = super::budget_row_misc_offset_for_city(city, row, 0).expect("budget row");
    let mut values = vec![0_u32; super::MISC_BUDGET_ROW_DWORDS];
    values[0] = 1;
    values[super::MISC_BUDGET_FUNDING_FIELD] = funding;
    city.set_budget_misc_u32_values(start, &values);
}

#[test]
fn initialize_police_and_fire_capacity_use_misc_funding() {
    let mut city = City::new_blank("/tmp/sc2000-init-funding.sc2", "test").expect("blank city");
    city.set_saved_date(super::CityDate {
        year: 1955,
        month: 6,
    });
    city.persist_pe_day22_population_leftover(9_000);
    stamp_service_funding(&mut city, 0, 40);
    stamp_service_funding(&mut city, 1, 50);

    let police = usize::from(
        city.provision_microsim_for_building(210).unwrap() - super::MIN_SIM_TEXT_ENTRIES,
    );
    assert_eq!(
        city.microsim_at(police).map(|r| r.stat1),
        Some(80),
        "leftover 9000 / 90 = 100, capped at live funding*2=80"
    );

    let fire = usize::from(
        city.provision_microsim_for_building(211).unwrap() - super::MIN_SIM_TEXT_ENTRIES,
    );
    assert_eq!(
        city.microsim_at(fire).map(|r| (r.stat1, r.stat2)),
        Some((25, 4)),
        "leftover 9000 / 70 = 128, capped at live funding/2=25"
    );
}

#[test]
fn initialize_police_and_fire_capacity_read_mac_budget_rows() {
    let mut city =
        City::new_blank("/tmp/sc2000-init-funding-mac.sc2", "MacImport").expect("blank city");
    city.kind = super::CityFileKind::MacCity;
    city.set_saved_date(super::CityDate {
        year: 1955,
        month: 6,
    });
    city.persist_pe_day22_population_leftover(9_000);
    stamp_service_funding(&mut city, 0, 40);
    stamp_service_funding(&mut city, 1, 50);

    assert_eq!(
        super::budget_row_misc_offset_for_city(&city, 5, 1),
        Some(0x99c)
    );
    assert_eq!(
        super::budget_row_misc_offset_for_city(&city, 6, 1),
        Some(0xa08)
    );
    let police = usize::from(
        city.provision_microsim_for_building(210).unwrap() - super::MIN_SIM_TEXT_ENTRIES,
    );
    let fire = usize::from(
        city.provision_microsim_for_building(211).unwrap() - super::MIN_SIM_TEXT_ENTRIES,
    );
    assert_eq!(city.microsim_at(police).map(|r| r.stat1), Some(80));
    assert_eq!(
        city.microsim_at(fire).map(|r| (r.stat1, r.stat2)),
        Some((25, 4))
    );
}

#[test]
fn museum_yearly_field_matches_pe_sixteen_bit_product() {
    let mut city =
        City::new_blank("/tmp/sc2000-museum-wrap.sc2", "MuseumWrap").expect("blank city");
    city.provision_microsim_for_building(212)
        .expect("PE museum XMIC system row");
    city.pe_dw_tile_count[212] = 6_554;
    let mut crt = super::Win95CrtRand::new(1);
    let mut game_rng = super::Win95GameRand::new(1);
    let mut lfsr = super::Win95Lfsr::new(1);
    city.refresh_structure_microsim_yearly_with_rngs(
        0,
        100,
        100,
        100,
        100,
        100,
        &mut crt,
        &mut game_rng,
        &mut lfsr,
    );
    assert_eq!(
        city.microsim_at(super::MICROSIM_SYSTEM_MUSEUM)
            .map(|row| row.stat2),
        Some(4),
        "PE imul ax, WORD[tile_count] wraps 10*6554 (65540) to 16-bit 4"
    );
}

#[test]
fn museum_capacity_uses_pe_signed_word_cap() {
    let mut city = City::new_blank("/tmp/sc2000-museum-cap.sc2", "MuseumCap").expect("blank city");
    city.provision_microsim_for_building(212)
        .expect("museum system row");
    city.pe_dw_tile_count[212] = 83;
    let mut crt = super::Win95CrtRand::new(1);
    let mut game_rng = super::Win95GameRand::new(1);
    let mut lfsr = super::Win95Lfsr::new(1);
    city.refresh_structure_microsim_yearly_with_rngs(
        100_000,
        100,
        100,
        100,
        100,
        100,
        &mut crt,
        &mut game_rng,
        &mut lfsr,
    );
    assert_eq!(
        city.microsim_at(super::MICROSIM_SYSTEM_MUSEUM)
            .map(|row| row.stat1),
        Some(33_200),
        "PE capacity cap 83*100*4 truncates to signed word 0x81b0"
    );
}

#[test]
fn museum_raw_funding_uses_signed_pe_division_and_wrapped_cap() {
    let mut city = City::new_blank("/tmp/sc2000-museum-funding-wrap.sc2", "MuseumFundingWrap")
        .expect("blank city");
    city.provision_microsim_for_building(212)
        .expect("PE museum XMIC system row");
    city.pe_dw_tile_count[212] = 1;

    let mut crt = super::Win95CrtRand::new(1);
    let mut game_rng = super::Win95GameRand::new(1);
    let mut lfsr = super::Win95Lfsr::new(1);
    city.refresh_structure_microsim_yearly_with_rngs(
        7_000,
        100,
        100,
        100,
        100,
        0xc000_0000,
        &mut crt,
        &mut game_rng,
        &mut lfsr,
    );

    let museum = city
        .microsim_at(super::MICROSIM_SYSTEM_MUSEUM)
        .expect("museum system row");
    assert_eq!(museum.stat1, 0, "college funding shifts to a zero cap word");
    assert_eq!(
        museum.stat2, 39_322,
        "signed IDIV result is narrowed before IMUL"
    );
}

#[test]
fn yearly_library_books_accept_only_signed_results_below_pe_limit() {
    let mut city =
        City::new_blank("/tmp/sc2000-library-book-limit.sc2", "LibraryLimit").expect("blank city");
    city.provision_microsim_for_building(245)
        .expect("PE library XMIC system row");
    city.pe_dw_tile_count[245] = 1;

    let mut crt = super::Win95CrtRand::new(1);
    let mut game_rng = super::Win95GameRand::new(1);
    let mut lfsr = super::Win95Lfsr::new(1);

    city.set_microsim_stat2(super::MICROSIM_SYSTEM_LIBRARY, 31_940);
    city.refresh_structure_microsim_yearly_with_rngs(
        7_000,
        100,
        100,
        100,
        100,
        100,
        &mut crt,
        &mut game_rng,
        &mut lfsr,
    );
    assert_eq!(
        city.microsim_at(super::MICROSIM_SYSTEM_LIBRARY)
            .map(|row| row.stat2),
        Some(31_990),
        "a positive signed result below 0x7d00 is stored"
    );

    city.set_microsim_stat2(super::MICROSIM_SYSTEM_LIBRARY, 31_950);
    city.refresh_structure_microsim_yearly_with_rngs(
        7_000,
        100,
        100,
        100,
        100,
        100,
        &mut crt,
        &mut game_rng,
        &mut lfsr,
    );
    assert_eq!(
        city.microsim_at(super::MICROSIM_SYSTEM_LIBRARY)
            .map(|row| row.stat2),
        Some(31_950),
        "the PE comparison is strict at 0x7d00 and preserves the old word"
    );

    city.set_microsim_stat2(super::MICROSIM_SYSTEM_LIBRARY, 0);
    city.refresh_structure_microsim_yearly_with_rngs(
        7_000,
        100,
        100,
        100,
        0,
        100,
        &mut crt,
        &mut game_rng,
        &mut lfsr,
    );
    assert_eq!(
        city.microsim_at(super::MICROSIM_SYSTEM_LIBRARY)
            .map(|row| row.stat2),
        Some(0),
        "a nonpositive signed result also preserves the old word"
    );
}

#[test]
fn yearly_library_raw_funding_uses_pe_wrapping_dword_arithmetic() {
    let mut city =
        City::new_blank("/tmp/sc2000-library-funding-wrap.sc2", "LibraryWrap").expect("blank city");
    city.provision_microsim_for_building(245)
        .expect("PE library XMIC system row");
    city.pe_dw_tile_count[245] = 4;
    city.set_microsim_stat2(super::MICROSIM_SYSTEM_LIBRARY, 300);

    let mut crt = super::Win95CrtRand::new(1);
    let mut game_rng = super::Win95GameRand::new(1);
    let mut lfsr = super::Win95Lfsr::new(1);
    city.refresh_structure_microsim_yearly_with_rngs(
        7_000,
        100,
        100,
        100,
        0x4000_0000,
        100,
        &mut crt,
        &mut game_rng,
        &mut lfsr,
    );

    let library = city
        .microsim_at(super::MICROSIM_SYSTEM_LIBRARY)
        .expect("library system row");
    assert_eq!(library.stat1, 0, "funding << 2 wraps to a zero cap word");
    assert_eq!(library.stat2, 100, "(funding - 50) * 4 wraps to -200");
    assert_eq!(library.stat0, 0, "the grade numerator wraps before IDIV");
}

#[test]
fn initialize_system_microsim_increments_shared_slots() {
    let mut city = City::new_blank("/tmp/sc2000-init-sys.sc2", "test").expect("blank city");
    city.provision_microsim_for_building(200).unwrap();
    city.provision_microsim_for_building(200).unwrap();
    assert_eq!(
        city.microsim_at(super::MICROSIM_SYSTEM_WIND)
            .map(|r| (r.stat1, r.stat2)),
        Some((2, 8))
    );
    city.provision_microsim_for_building(198).unwrap();
    assert_eq!(
        city.microsim_at(super::MICROSIM_SYSTEM_HYDRO)
            .map(|r| (r.stat1, r.stat2)),
        Some((1, 20))
    );
    city.provision_microsim_for_building(213).unwrap();
    assert_eq!(
        city.microsim_at(super::MICROSIM_SYSTEM_PARK)
            .map(|r| r.stat2),
        Some(9)
    );
    assert_eq!(
        city.provision_microsim_for_building(13),
        None,
        "tile < 0xc6 is rejected before class lookup"
    );
    assert_eq!(
        city.microsim_at(super::MICROSIM_SYSTEM_PARK)
            .map(|r| r.stat2),
        Some(9)
    );
    city.provision_microsim_for_building(236).unwrap();
    city.provision_microsim_for_building(236).unwrap();
    assert_eq!(
        city.microsim_at(super::MICROSIM_SYSTEM_BUS)
            .map(|r| r.stat1),
        Some(2)
    );
}

#[test]
fn mac_xmic_class_table_and_fixed_system_rows_match_recovered_policy() {
    assert_eq!(super::MAC_MICROSIM_CLASS, super::PE_MICROSIM_CLASS);
    assert_eq!(super::mac_microsim_class(0xc5), 0);
    assert_eq!(super::mac_microsim_class(0xc6), 21);
    assert_eq!(super::mac_microsim_class(0xd4), 23);
    assert_eq!(super::mac_microsim_class(0xe9), 19);
    assert_eq!(super::mac_microsim_class(0xff), 16);

    let mut city =
        City::new_blank("/tmp/sc2000-mac-fixed-xmic.sc2", "MacFixed").expect("blank city");
    city.kind = CityFileKind::MacCity;
    let mut rng = MacToolboxRng::new(MacToolboxRng::QUICKDRAW_INIT_GRAF_SEED);

    assert_eq!(
        city.provision_mac_microsim_for_building(198, &mut rng),
        Some(56),
        "hydro maps to fixed row 5 / XTXT 0x38"
    );
    city.provision_mac_microsim_for_building(199, &mut rng)
        .expect("second hydro tile shares row 5");
    assert_eq!(
        city.microsim_at(5)
            .map(|row| (row.tile_id, row.stat1, row.stat2)),
        Some((199, 2, 40))
    );

    city.provision_mac_microsim_for_building(200, &mut rng)
        .expect("wind fixed row");
    city.provision_mac_microsim_for_building(213, &mut rng)
        .expect("park fixed row");
    city.provision_mac_microsim_for_building(212, &mut rng)
        .expect("fixed row 7");
    city.provision_mac_microsim_for_building(233, &mut rng)
        .expect("fixed row 3");
    city.provision_mac_microsim_for_building(236, &mut rng)
        .expect("fixed row 1");
    city.provision_mac_microsim_for_building(237, &mut rng)
        .expect("fixed row 2");

    assert_eq!(
        city.microsim_at(4).map(|row| (row.stat1, row.stat2)),
        Some((1, 4))
    );
    assert_eq!(city.microsim_at(6).map(|row| row.stat2), Some(9));
    assert_eq!(
        city.microsim_at(7),
        Some(super::MicrosimRecord {
            tile_id: 212,
            stat0: 0,
            stat1: 0,
            stat2: 0,
            stat3: 0,
        }),
        "D4 is a fixed-class row, so the dynamic D4 constructor arm is unreachable"
    );
    assert_eq!(city.microsim_at(3).map(|row| row.stat1), Some(1));
    assert_eq!(city.microsim_at(1).map(|row| row.stat1), Some(1));
    assert_eq!(city.microsim_at(2).map(|row| row.stat1), Some(1));
    assert_eq!(
        city.provision_mac_microsim_for_building(220, &mut rng),
        None,
        "zero-class tile does not allocate an XMIC row"
    );
    assert_eq!(
        rng.seed(),
        MacToolboxRng::QUICKDRAW_INIT_GRAF_SEED,
        "fixed-row placement does not call QuickDraw Random"
    );
}

#[test]
fn mac_xmic_constructors_seed_native_fields_and_quickdraw_draw_order() {
    let mut city =
        City::new_blank("/tmp/sc2000-mac-initial-xmic.sc2", "MacInit").expect("blank city");
    city.kind = CityFileKind::MacCity;
    city.set_saved_date(super::CityDate {
        year: 2055,
        month: 6,
    });
    city.set_mac_state_misc_u32_at(0x102c, 90_000);
    stamp_service_funding(&mut city, 0, 130);
    stamp_service_funding(&mut city, 1, 201);
    let mut rng = MacToolboxRng::new(MacToolboxRng::QUICKDRAW_INIT_GRAF_SEED);

    let expected_plants = [50, 220, 500, 50, 1_600, 2_500, 200];
    for (offset, tile) in (0xc9_u8..=0xcf).enumerate() {
        let marker = city
            .provision_mac_microsim_for_building(tile, &mut rng)
            .expect("plant XMIC row");
        let index = usize::from(marker - super::MIN_SIM_TEXT_ENTRIES);
        assert_eq!(
            city.microsim_at(index).map(|row| row.stat1),
            Some(expected_plants[offset])
        );
    }

    let year_row = usize::from(
        city.provision_mac_microsim_for_building(0xd0, &mut rng)
            .unwrap()
            - super::MIN_SIM_TEXT_ENTRIES,
    );
    assert_eq!(
        city.microsim_at(year_row).map(|row| (row.stat1, row.stat2)),
        Some((100, 2055)),
        "D0 uses the recovered capacity helper and writes the current year"
    );

    for (tile, expected) in [(0xd1, (6, 0, 0)), (0xd6, (6, 0, 0)), (0xd9, (6, 0, 0))] {
        let marker = city
            .provision_mac_microsim_for_building(tile, &mut rng)
            .unwrap();
        let index = usize::from(marker - super::MIN_SIM_TEXT_ENTRIES);
        let row = city.microsim_at(index).unwrap();
        assert_eq!((row.stat0, row.stat1, row.stat2), expected);
    }

    let police = usize::from(
        city.provision_mac_microsim_for_building(0xd2, &mut rng)
            .unwrap()
            - super::MIN_SIM_TEXT_ENTRIES,
    );
    assert_eq!(
        city.microsim_at(police).map(|row| row.stat1),
        Some(260),
        "Mac constructor uses raw police funding, not the PE 0..=100 clamp"
    );
    let fire = usize::from(
        city.provision_mac_microsim_for_building(0xd3, &mut rng)
            .unwrap()
            - super::MIN_SIM_TEXT_ENTRIES,
    );
    assert_eq!(
        city.microsim_at(fire).map(|row| (row.stat1, row.stat2)),
        Some((100, 4)),
        "Mac constructor shifts the raw fire DWORD before narrowing"
    );

    let db_row = usize::from(
        city.provision_mac_microsim_for_building(0xdb, &mut rng)
            .unwrap()
            - super::MIN_SIM_TEXT_ENTRIES,
    );
    assert_eq!(
        city.microsim_at(db_row),
        Some(super::MicrosimRecord {
            tile_id: 0xdb,
            stat0: 0,
            stat1: 2055,
            stat2: 0,
            stat3: 0,
        })
    );

    let mut expected_rng = rng;
    let expected_age = expected_rng.random_u16() % 30 + 10;
    let expected_state = expected_rng.random_u16() % 60;
    let mayor_row = usize::from(
        city.provision_mac_microsim_for_building(0xf3, &mut rng)
            .unwrap()
            - super::MIN_SIM_TEXT_ENTRIES,
    );
    assert_eq!(
        city.microsim_at(mayor_row)
            .map(|row| (row.stat1, row.stat2, row.stat3)),
        Some((2055, expected_age, expected_state))
    );
    assert_eq!(
        rng.seed(),
        expected_rng.seed(),
        "F3 consumes exactly two Toolbox draws"
    );

    let arcology = usize::from(
        city.provision_mac_microsim_for_building(0xfb, &mut rng)
            .unwrap()
            - super::MIN_SIM_TEXT_ENTRIES,
    );
    assert_eq!(
        city.microsim_at(arcology)
            .map(|row| (row.stat0, row.stat1, row.stat3)),
        Some((5, 55, 2055))
    );
    let before_ff = rng;
    let ff_row = usize::from(
        city.provision_mac_microsim_for_building(0xff, &mut rng)
            .unwrap()
            - super::MIN_SIM_TEXT_ENTRIES,
    );
    assert_eq!(city.microsim_at(ff_row).map(|row| row.stat3), Some(2055));
    assert_eq!(
        rng.seed(),
        before_ff.seed(),
        "FF initializer does not consume Random"
    );
}

#[test]
fn mac_xmic_arcology_reclaims_first_non_arcology_and_clears_its_marker() {
    let mut city =
        City::new_blank("/tmp/sc2000-mac-xmic-reclaim.sc2", "MacReclaim").expect("blank city");
    city.kind = CityFileKind::MacCity;
    city.set_saved_date(super::CityDate {
        year: 2001,
        month: 1,
    });
    city.microsims = Some(vec![
        super::MicrosimRecord::default();
        super::MICROSIM_COUNT
    ]);
    for index in super::MICROSIM_STRUCTURE_FIRST..super::MICROSIM_COUNT {
        city.microsims.as_mut().unwrap()[index].tile_id = 201;
    }
    let first_marker = super::MIN_SIM_TEXT_ENTRIES + super::MICROSIM_STRUCTURE_FIRST as u8;
    let last_marker = super::MIN_SIM_TEXT_ENTRIES + (super::MICROSIM_COUNT - 1) as u8;
    city.set_text_overlay(10, 11, first_marker);
    city.set_text_overlay(12, 13, last_marker);

    let mut rng = MacToolboxRng::new(MacToolboxRng::QUICKDRAW_INIT_GRAF_SEED);
    assert_eq!(
        city.provision_mac_microsim_for_building(0xfb, &mut rng),
        Some(first_marker)
    );
    assert_eq!(city.text_overlay_at(10, 11), Some(0));
    assert_eq!(city.text_overlay_at(12, 13), Some(last_marker));
    assert_eq!(
        city.microsim_at(super::MICROSIM_STRUCTURE_FIRST),
        Some(super::MicrosimRecord {
            tile_id: 0xfb,
            stat0: 5,
            stat1: 55,
            stat2: 0,
            stat3: 2001,
        })
    );
}

#[test]
fn yearly_hydro_and_wind_capacity_fields_wrap_as_pe_words() {
    let mut city =
        City::new_blank("/tmp/sc2000-system-word-wrap.sc2", "SystemWordWrap").expect("blank city");
    city.provision_microsim_for_building(198)
        .expect("hydro system row");
    city.provision_microsim_for_building(200)
        .expect("wind system row");

    // The map-wide tile-count words are inputs to PE `FUN_0044E390`.
    // Multiplication stores the low word: 3,277 * 20 = 65,540 -> 4;
    // 16,384 << 2 = 65,536 -> 0.
    city.pe_dw_tile_count[198] = 3_277;
    city.pe_dw_tile_count[200] = 16_384;
    city.refresh_system_microsim_counts();

    assert_eq!(
        city.microsim_at(super::MICROSIM_SYSTEM_HYDRO)
            .map(|row| (row.stat1, row.stat2)),
        Some((3_277, 4))
    );
    assert_eq!(
        city.microsim_at(super::MICROSIM_SYSTEM_WIND)
            .map(|row| (row.stat1, row.stat2)),
        Some((16_384, 0))
    );
}

#[test]
fn yearly_system_census_does_not_activate_absent_hydro_or_wind_rows() {
    let mut city =
        City::new_blank("/tmp/sc2000-inactive-system-rows.sc2", "NoPlants").expect("blank city");
    city.pe_dw_tile_count[198] = 7;
    city.pe_dw_tile_count[199] = 3;
    city.pe_dw_tile_count[200] = 5;

    city.refresh_system_microsim_counts();

    assert_eq!(
        city.microsim_at(super::MICROSIM_SYSTEM_HYDRO).map(|row| (
            row.tile_id,
            row.stat0,
            row.stat1,
            row.stat2,
            row.stat3
        )),
        Some((0, 0, 0, 0, 0)),
        "the PE annual selector cannot reach hydro without its saved C6/C7 tag"
    );
    assert_eq!(
        city.microsim_at(super::MICROSIM_SYSTEM_WIND).map(|row| (
            row.tile_id,
            row.stat0,
            row.stat1,
            row.stat2,
            row.stat3
        )),
        Some((0, 0, 0, 0, 0)),
        "the PE annual selector cannot reach wind without its saved C8 tag"
    );
}

#[test]
fn mac_monthly_hydro_and_wind_rows_replace_with_saved_word_census() {
    let mut city = City::new_blank("/tmp/sc2000-mac-monthly-utilities.sc2", "MacUtilities")
        .expect("blank city");
    city.kind = CityFileKind::MacCity;
    city.set_mac_state_misc_u32_at(MAC_XBLD_CENSUS_MISC_OFFSET + 198 * 4, 1);
    city.set_mac_state_misc_u32_at(MAC_XBLD_CENSUS_MISC_OFFSET + 199 * 4, 2);
    city.set_mac_state_misc_u32_at(MAC_XBLD_CENSUS_MISC_OFFSET + 200 * 4, 4);
    city.provision_microsim_for_building(198)
        .expect("hydro system row");
    city.provision_microsim_for_building(200)
        .expect("wind system row");
    assert!(city.set_microsim_stat1(super::MICROSIM_SYSTEM_HYDRO, 77));
    assert!(city.set_microsim_stat1(super::MICROSIM_SYSTEM_WIND, 99));

    city.refresh_mac_monthly_utility_microsims();
    assert_eq!(
        city.microsim_at(super::MICROSIM_SYSTEM_HYDRO).map(|row| (
            row.tile_id,
            row.stat1,
            row.stat2
        )),
        Some((198, 3, 60)),
        "Mac sums the saved hydro tile counters, then multiplies by 20"
    );
    assert_eq!(
        city.microsim_at(super::MICROSIM_SYSTEM_WIND).map(|row| (
            row.tile_id,
            row.stat1,
            row.stat2
        )),
        Some((200, 4, 16)),
        "Mac copies the saved wind tile counter, then shifts left two"
    );

    city.refresh_mac_monthly_utility_microsims();
    assert_eq!(
        city.microsim_at(super::MICROSIM_SYSTEM_HYDRO)
            .map(|row| (row.stat1, row.stat2)),
        Some((3, 60)),
        "repeated monthly passes replace with the same saved census"
    );
    assert_eq!(
        city.microsim_at(super::MICROSIM_SYSTEM_WIND)
            .map(|row| (row.stat1, row.stat2)),
        Some((4, 16))
    );
}

#[test]
fn mac_monthly_power_plant_rows_advance_age_and_quickdraw_in_slot_order() {
    let mut city =
        City::new_blank("/tmp/sc2000-mac-monthly-plants.sc2", "MacPlants").expect("blank city");
    city.kind = CityFileKind::MacCity;
    city.set_mac_state_misc_u32_at(MAC_POWER_SUMMARY_MISC_OFFSET, 0x1234_abcd);
    let first = city
        .provision_microsim_for_building(0xc9)
        .expect("first power plant row");
    let second = city
        .provision_microsim_for_building(0xcf)
        .expect("second power plant row");
    let first_slot = usize::from(first - MIN_SIM_TEXT_ENTRIES);
    let second_slot = usize::from(second - MIN_SIM_TEXT_ENTRIES);
    assert!(city.set_microsim_stat0(first_slot, 7));
    assert!(city.set_microsim_stat0(second_slot, 0xff));
    assert!(city.set_microsim_stat2(first_slot, 0x5555));
    assert!(city.set_microsim_stat2(second_slot, 0xaaaa));

    let mut actual_rng = MacToolboxRng::new(MacToolboxRng::QUICKDRAW_INIT_GRAF_SEED);
    let mut expected_rng = actual_rng;
    let expected_first = 0xabcd_u16.wrapping_add(expected_rng.random_u16() & 7);
    let expected_second = 0xabcd_u16.wrapping_add(expected_rng.random_u16() & 7);

    city.refresh_mac_monthly_power_plant_microsims(&mut actual_rng);

    assert_eq!(
        city.microsim_at(first_slot)
            .map(|row| (row.stat0, row.stat2)),
        Some((8, expected_first))
    );
    assert_eq!(
        city.microsim_at(second_slot)
            .map(|row| (row.stat0, row.stat2)),
        Some((0, expected_second))
    );
    assert_eq!(
        actual_rng.seed(),
        expected_rng.seed(),
        "one draw per active row"
    );
}

#[test]
fn mac_aged_plants_rebuild_only_with_marker_no_disasters_and_funds() {
    let mut city = City::new_blank("/tmp/sc2000-mac-plant-renewal.sc2", "MacPlantRenewal")
        .expect("blank city");
    city.kind = CityFileKind::MacCity;
    city.set_city_funds(3_300);
    let plants = [0xc9, 0xcc, 0xcf].map(|tile_id| {
        let overlay = city
            .provision_microsim_for_building(tile_id)
            .expect("power plant row");
        usize::from(overlay - MIN_SIM_TEXT_ENTRIES)
    });
    for &slot in &plants {
        assert!(city.set_microsim_stat0(slot, 50));
        city.set_text_overlay(20 + slot, 30, (0x33 + slot) as u8);
    }

    let mut rng = MacToolboxRng::new(MacToolboxRng::QUICKDRAW_INIT_GRAF_SEED);
    let mut expected_rng = rng;
    for _ in plants {
        let _plant_power_jitter = expected_rng.random_u16() & 7;
    }
    let expected_next = expected_rng.random_u16();

    city.refresh_mac_monthly_random_xmic_rows(100, 80, 80, 80, 80, true, &mut rng);

    assert_eq!(city.microsim_at(plants[0]).unwrap().stat0, 0);
    assert_eq!(city.microsim_at(plants[1]).unwrap().stat0, 0);
    assert_eq!(
        city.microsim_at(plants[2]).unwrap().stat0,
        51,
        "the final coal plant costs more than the remaining $0"
    );
    assert_eq!(city.city_funds(), 0);
    assert_eq!(
        rng.random_u16(),
        expected_next,
        "renewal spends no extra draws"
    );
}

#[test]
fn mac_aged_plant_zero_marker_sentinel_skips_renewal() {
    let mut city = City::new_blank("/tmp/sc2000-mac-plant-renewal-gates.sc2", "MacPlantGates")
        .expect("blank city");
    city.kind = CityFileKind::MacCity;
    city.set_city_funds(10_000);
    let overlay = city.provision_microsim_for_building(0xc9).unwrap();
    let slot = usize::from(overlay - MIN_SIM_TEXT_ENTRIES);
    assert!(city.set_microsim_stat0(slot, 50));
    city.set_text_overlay(0, 0, (0x33 + slot) as u8);

    let mut rng = MacToolboxRng::new(MacToolboxRng::QUICKDRAW_INIT_GRAF_SEED);
    city.refresh_mac_monthly_random_xmic_rows(100, 80, 80, 80, 80, true, &mut rng);
    assert_eq!(
        city.microsim_at(slot).unwrap().stat0,
        51,
        "a (0,0) marker is indistinguishable from the native zero sentinel"
    );
    assert_eq!(city.city_funds(), 10_000);
}

#[test]
fn mac_aged_plant_expiry_replays_footprint_rubble_and_quickdraw_order() {
    let mut city =
        City::new_blank("/tmp/sc2000-mac-plant-expiry.sc2", "MacPlantExpiry").expect("blank city");
    city.kind = CityFileKind::MacCity;
    let slot = usize::from(
        city.provision_microsim_for_building(0xc9)
            .expect("gas plant row")
            - MIN_SIM_TEXT_ENTRIES,
    );
    let origin = (20, 20);
    let mut flat_cells = 0_u32;
    for dx in 0..4 {
        for dy in 0..4 {
            let (x, y) = (origin.0 + dx, origin.1 - dy);
            city.set_building(x, y, 0xc9);
            let is_flat = (dx + dy) % 3 != 0;
            city.set_terrain(x, y, if is_flat { 0 } else { 1 });
            flat_cells += u32::from(is_flat);
        }
    }
    city.set_mac_state_misc_u32_at(
        MAC_XBLD_CENSUS_MISC_OFFSET,
        (CITY_WIDTH * CITY_HEIGHT - 16) as u32,
    );
    city.set_mac_state_misc_u32_at(MAC_XBLD_CENSUS_MISC_OFFSET + 0xc9 * 4, 16);
    assert!(city.set_microsim_stat0(slot, 50));
    city.set_text_overlay(origin.0, origin.1, (0x33 + slot) as u8);
    for rubble in 1..=4 {
        city.set_label(rubble, format!("custom {rubble}"));
    }

    let mut actual_rng = MacToolboxRng::new(MacToolboxRng::QUICKDRAW_INIT_GRAF_SEED);
    let mut expected_rng = actual_rng;
    let _plant_power_jitter = expected_rng.random_u16();
    for _ in 0..128 {
        let _visual_sprite_or_flip = expected_rng.random_u16();
    }
    let mut expected_tiles = Vec::new();
    for dx in 0..4 {
        for dy in 0..4 {
            if (dx + dy) % 3 != 0 {
                expected_tiles.push((dx, dy, (expected_rng.random_u16() & 3) as u8 + 1));
            } else {
                expected_tiles.push((dx, dy, 0));
            }
        }
    }
    let expected_next = expected_rng.random_u16();

    city.refresh_mac_monthly_random_xmic_rows(100, 80, 80, 80, 80, false, &mut actual_rng);

    for (dx, dy, rubble) in expected_tiles {
        let (x, y) = (origin.0 + dx, origin.1 - dy);
        assert_eq!(city.building_at(x, y), rubble, "footprint tile ({x},{y})");
        assert_eq!(city.tile_bits_at(x, y).unwrap().raw_bits() & 0xc2, 0);
        assert_eq!(city.zone_at(x, y).unwrap().raw_bits() & 0xf0, 0);
    }
    assert_eq!(city.microsim_at(slot).unwrap().tile_id, 0);
    assert_eq!(
        city.text_overlay_at(origin.0, origin.1),
        Some(0),
        "export 80 clears the locator XTXT before releasing its resource"
    );
    assert_eq!(city.mac_xbld_tile_count(0xc9), 0);
    assert_eq!(
        city.mac_xbld_tile_count(0),
        (CITY_WIDTH * CITY_HEIGHT - flat_cells as usize) as u16
    );
    assert_eq!(
        (1..=4)
            .map(|tile| city.mac_xbld_tile_count(tile))
            .sum::<u16>(),
        flat_cells as u16,
        "the saved census describes the retained rubble"
    );
    for rubble in 1..=4 {
        assert_eq!(
            city.label_at(rubble),
            Some(format!("custom {rubble}").as_str())
        );
    }
    assert_eq!(actual_rng.random_u16(), expected_next);
}

#[test]
fn mac_aged_plant_expiry_applies_linked_row_clears_in_live_slot_order() {
    // Deliberately crossed XTXT links exercise export 161's actual target,
    // rather than assuming every marker belongs to its footprint class.
    for hospital_first in [false, true] {
        for inside_footprint in [false, true] {
            let mut city = City::new_blank("/tmp/sc2000-mac-plant-row-order.sc2", "MacRows")
                .expect("blank city");
            city.kind = CityFileKind::MacCity;
            let (plant, hospital) = if hospital_first {
                let hospital = city.provision_microsim_for_building(0xd1).unwrap();
                (
                    city.provision_microsim_for_building(0xc9).unwrap(),
                    hospital,
                )
            } else {
                let plant = city.provision_microsim_for_building(0xc9).unwrap();
                (plant, city.provision_microsim_for_building(0xd1).unwrap())
            };
            let plant_slot = usize::from(plant - MIN_SIM_TEXT_ENTRIES);
            let hospital_slot = usize::from(hospital - MIN_SIM_TEXT_ENTRIES);
            assert_eq!(hospital_slot < plant_slot, hospital_first);
            for dx in 0..4 {
                for dy in 0..4 {
                    city.set_building(20 + dx, 20 - dy, 0xc9);
                    city.set_terrain(20 + dx, 20 - dy, 0);
                }
            }
            city.set_microsim_stat0(plant_slot, 50);
            city.set_text_overlay(20, 20, plant);
            let link = if inside_footprint { (21, 20) } else { (30, 30) };
            city.set_text_overlay(link.0, link.1, hospital);
            let seed = 0x5187_32ab;
            let mut expected_rng = MacToolboxRng::new(seed);
            let hospital_runs = hospital_first || !inside_footprint;
            for _ in 0..(1 + 128 + 16 + if hospital_runs { 2 } else { 0 }) {
                expected_rng.random_u16();
            }
            let mut rng = MacToolboxRng::new(seed);

            city.refresh_mac_monthly_random_xmic_rows(100, 80, 80, 80, 80, false, &mut rng);

            assert_eq!(city.microsim_at(plant_slot).unwrap().tile_id, 0);
            assert_eq!(
                city.microsim_at(hospital_slot).unwrap().tile_id,
                if inside_footprint { 0 } else { 0xd1 }
            );
            assert_eq!(
                city.text_overlay_at(link.0, link.1),
                Some(if inside_footprint { 0 } else { hospital })
            );
            assert_eq!(
                rng.seed(),
                expected_rng.seed(),
                "hospital_first={hospital_first}, inside={inside_footprint}"
            );
        }
    }
}

#[test]
fn mac_aged_plant_expiry_aligns_rotated_xzon_center_signatures() {
    let center_signatures = [0x10, 0x20, 0x40, 0x80];
    for (rotation, signature) in center_signatures.into_iter().enumerate() {
        let mut city = City::new_blank(
            format!("/tmp/sc2000-mac-plant-rotation-{rotation}.sc2"),
            "MacPlantRotation",
        )
        .expect("blank city");
        city.kind = CityFileKind::MacCity;
        city.saved_rotation = rotation as u8;
        let slot = usize::from(
            city.provision_microsim_for_building(0xc9)
                .expect("gas plant row")
                - MIN_SIM_TEXT_ENTRIES,
        );
        for dx in 0..4 {
            for dy in 0..4 {
                city.set_building(20 + dx, 20 + dy, 0xc9);
                city.set_terrain(20 + dx, 20 + dy, 1);
            }
        }
        city.set_zone(20, 20, signature);
        assert!(city.set_microsim_stat0(slot, 50));
        city.set_text_overlay(20, 20, (0x33 + slot) as u8);

        let mut rng = MacToolboxRng::new(MacToolboxRng::QUICKDRAW_INIT_GRAF_SEED);
        city.refresh_mac_monthly_random_xmic_rows(100, 80, 80, 80, 80, false, &mut rng);

        for dx in 0..4 {
            for dy in 0..4 {
                assert_eq!(
                    city.building_at(20 + dx, 20 + dy),
                    0,
                    "rotation {rotation} left plant tile ({},{}) behind",
                    20 + dx,
                    20 + dy
                );
            }
        }
    }
}

#[test]
fn mac_monthly_hospital_update_uses_census_caps_and_two_quickdraws() {
    let mut city =
        City::new_blank("/tmp/sc2000-mac-monthly-hospital.sc2", "MacHospital").expect("blank city");
    city.kind = CityFileKind::MacCity;
    city.set_mac_state_misc_u32_at(MAC_XBLD_CENSUS_MISC_OFFSET + 0xd1 * 4, 9);
    city.set_mac_state_misc_u32_at(0x102c, 2_500);
    let overlay = city
        .provision_microsim_for_building(0xd1)
        .expect("hospital XMIC row");
    let slot = usize::from(overlay - MIN_SIM_TEXT_ENTRIES);
    assert!(city.set_microsim_stat0(slot, 6));

    let mut actual_rng = MacToolboxRng::new(MacToolboxRng::QUICKDRAW_INIT_GRAF_SEED);
    let mut expected_rng = actual_rng;
    let _patient_jitter = expected_rng.random_u16() & 0x0f;
    let _physician_jitter = expected_rng.random_u16() & 7;

    city.refresh_mac_monthly_hospital_microsims(80, &mut actual_rng);

    let row = city.microsim_at(slot).expect("hospital row remains active");
    assert_eq!(row.stat3, 40, "health funding is stored divided by two");
    assert_eq!(row.stat1, 83, "patient capacity is capped at f01a/30");
    assert_eq!(row.stat2, 20, "physician capacity is capped at f01a/120");
    assert_eq!(row.stat0, 12, "83*10/20 falls in the top grade band");
    assert_eq!(
        actual_rng.seed(),
        expected_rng.seed(),
        "the non-overflow patient candidate consumes two draws total"
    );
}

#[test]
fn mac_monthly_hospital_funding_keeps_signed_dword_width() {
    let mut city =
        City::new_blank("/tmp/sc2000-mac-hospital-dword.sc2", "MacHospital").expect("blank city");
    city.kind = CityFileKind::MacCity;
    city.set_mac_state_misc_u32_at(MAC_XBLD_CENSUS_MISC_OFFSET + 0xd1 * 4, 9);
    city.set_mac_state_misc_u32_at(0x102c, 1_200);
    let overlay = city
        .provision_microsim_for_building(0xd1)
        .expect("hospital XMIC row");
    let slot = usize::from(overlay - MIN_SIM_TEXT_ENTRIES);
    assert!(city.set_microsim_stat0(slot, 12));

    let mut actual_rng = MacToolboxRng::new(MacToolboxRng::QUICKDRAW_INIT_GRAF_SEED);
    let mut expected_rng = actual_rng;
    let _patient_jitter = expected_rng.random_u16() & 0x0f;
    let _physician_jitter = expected_rng.random_u16() & 7;

    city.refresh_mac_monthly_hospital_microsims(0x8000_0003, &mut actual_rng);

    let row = city.microsim_at(slot).expect("hospital row remains active");
    assert_eq!(row.stat3, 2, "signed dword /2 precedes low-word storage");
    assert_eq!(row.stat1, 40, "patient count remains capped by scalar /30");
    assert_eq!(row.stat2, 0, "signed negative funding dword is clamped");
    assert_eq!(row.stat0, 0, "grade uses the clamped physician word");
    assert_eq!(
        super::mac_hospital_funding_half(u32::MAX),
        0,
        "signed -1 / 2 truncates toward zero"
    );
    assert_eq!(
        super::mac_hospital_physicians_raw(0x0001_0000, 7, 6),
        -5,
        "physician funding stays dword-wide until the helper word input"
    );
    assert_eq!(
        super::mac_hospital_physicians_raw(0x7fff_ffff, 7, 12),
        0,
        "signed dword overflow is clamped before narrowing"
    );
    assert_eq!(
        actual_rng.random_u16(),
        expected_rng.random_u16(),
        "the ordinary candidate still consumes exactly two QuickDraw draws"
    );
}

#[test]
fn mac_monthly_hospital_overflow_replaces_patients_with_a_second_draw() {
    let mut city = City::new_blank("/tmp/sc2000-mac-hospital-overflow.sc2", "MacHospital")
        .expect("blank city");
    city.kind = CityFileKind::MacCity;
    city.set_mac_state_misc_u32_at(MAC_XBLD_CENSUS_MISC_OFFSET + 0xd1 * 4, 9);
    city.set_mac_state_misc_u32_at(0x102c, 50_000);
    let overlay = city
        .provision_microsim_for_building(0xd1)
        .expect("hospital XMIC row");
    let slot = usize::from(overlay - MIN_SIM_TEXT_ENTRIES);
    assert!(city.set_microsim_stat0(slot, 12));

    let mut actual_rng = MacToolboxRng::new(MacToolboxRng::QUICKDRAW_INIT_GRAF_SEED);
    let mut expected_rng = actual_rng;
    let first_jitter = u32::from(expected_rng.random_u16() & 0x0f);
    assert!(50_000 / 25 + first_jitter > 1_000);
    let patients = 1_000 + u32::from(expected_rng.random_u16() & 0x7f);
    let physicians = 100 + i32::from(expected_rng.random_u16() & 7);
    let ratio = (patients as i32 * 10) / physicians;
    let expected_grade = if ratio < 50 {
        12
    } else if ratio < 111 {
        ((110 - ratio) / 5) as u8
    } else {
        0
    };

    city.refresh_mac_monthly_hospital_microsims(100, &mut actual_rng);

    let row = city.microsim_at(slot).expect("hospital row remains active");
    assert_eq!(row.stat1, patients as u16);
    assert_eq!(row.stat2, physicians as u16);
    assert_eq!(row.stat3, 50);
    assert_eq!(row.stat0, expected_grade);
    assert_eq!(
        actual_rng.seed(),
        expected_rng.seed(),
        "overflow adds exactly one patient replacement draw"
    );
}

#[test]
fn mac_monthly_police_fire_rows_keep_slot_draws_and_arrest_subtotal() {
    let mut city = City::new_blank("/tmp/sc2000-mac-monthly-police-fire.sc2", "MacPoliceFire")
        .expect("blank city");
    city.kind = CityFileKind::MacCity;
    city.set_mac_state_misc_u32_at(MAC_XBLD_CENSUS_MISC_OFFSET + 0xd2 * 4, 18);
    city.set_mac_state_misc_u32_at(MAC_XBLD_CENSUS_MISC_OFFSET + 0xd3 * 4, 9);
    city.set_mac_state_misc_u32_at(1039 * super::MISC_FIELD_LEN, 2);
    city.set_mac_state_misc_u32_at(0x002c, 100);
    city.set_mac_state_misc_u32_at(0x102c, 9_000);
    let police = city
        .provision_microsim_for_building(0xd2)
        .expect("police row");
    let fire = city
        .provision_microsim_for_building(0xd3)
        .expect("fire row");
    let police_slot = usize::from(police - MIN_SIM_TEXT_ENTRIES);
    let fire_slot = usize::from(fire - MIN_SIM_TEXT_ENTRIES);

    let mut actual_rng = MacToolboxRng::new(MacToolboxRng::QUICKDRAW_INIT_GRAF_SEED);
    let mut expected_rng = actual_rng;
    let police_jitter = expected_rng.random_u16() & 0x0f;
    let fire_random = expected_rng.random_u16() % 20;

    city.refresh_mac_monthly_police_fire_microsims(100, 80, &mut actual_rng);

    let police = city
        .microsim_at(police_slot)
        .expect("police row remains active");
    assert_eq!(police.stat0, 100, "police funding byte");
    assert_eq!(police.stat1, 100, "police capacity is capped at f01a/90");
    assert_eq!(
        police.stat2, 5,
        "crime total divided by eighteen police tiles"
    );
    let arrest = (1 + u32::from(police_jitter)) as u16;
    assert_eq!(
        police.stat3, arrest,
        "prior coverage gate divides arrest share"
    );
    assert_eq!(city.mac_monthly_arrest_total(), arrest);

    let fire = city
        .microsim_at(fire_slot)
        .expect("fire row remains active");
    assert_eq!(fire.stat0, 80, "fire funding byte");
    assert_eq!(fire.stat1, 40, "fire capacity is funding/2");
    assert_eq!(fire.stat2, 3, "truck field is (capacity >> 4) + 1");
    assert_eq!(
        fire.stat3,
        fire_random + 2,
        "fire response field uses Random % 20"
    );
    assert_eq!(
        actual_rng.seed(),
        expected_rng.seed(),
        "one draw per active row"
    );
}

#[test]
fn mac_fire_funding_shifts_the_dword_before_narrowing_to_capacity_word() {
    // CODE 3 uses MOVE.L / ASR.L / MOVE.W for the fire funding cap.
    // These inputs separate that order from shifting an already-truncated
    // i16, while ordinary 0..=100 funding yields the same value either way.
    for (funding, expected_capacity, expected_trucks) in
        [(0x8000, 128, 9), (0x1_0000, 0x8000, 2049)]
    {
        for combined_dispatch in [false, true] {
            let mut city = City::new_blank(
                format!("/tmp/sc2000-mac-fire-funding-width-{funding}-{combined_dispatch}.sc2"),
                "MacFireFundingWidth",
            )
            .expect("blank city");
            city.kind = CityFileKind::MacCity;
            city.set_mac_state_misc_u32_at(0x102c, 9_000);
            let fire = city
                .provision_microsim_for_building(0xd3)
                .expect("fire row");
            let fire_slot = usize::from(fire - MIN_SIM_TEXT_ENTRIES);
            let mut rng = MacToolboxRng::new(MacToolboxRng::QUICKDRAW_INIT_GRAF_SEED);

            if combined_dispatch {
                city.refresh_mac_monthly_random_xmic_rows(
                    100, funding, 100, 100, 100, false, &mut rng,
                );
            } else {
                city.refresh_mac_monthly_police_fire_microsims(100, funding, &mut rng);
            }

            let fire = city
                .microsim_at(fire_slot)
                .expect("fire row remains active");
            assert_eq!(fire.stat0, funding as u8);
            assert_eq!(fire.stat1, expected_capacity);
            assert_eq!(fire.stat2, expected_trucks);
        }
    }

    // PE independently uses a dword SAR before its signed-word capacity
    // helper; the same raw funding boundary should preserve the cap word.
    let mut pe_city = City::new_blank(
        "/tmp/sc2000-pe-fire-funding-width.sc2",
        "PEFireFundingWidth",
    )
    .expect("blank city");
    let fire = pe_city
        .provision_microsim_for_building(211)
        .expect("PE fire row");
    let fire_slot = usize::from(fire - MIN_SIM_TEXT_ENTRIES);
    let mut crt = super::Win95CrtRand::new(1);
    let mut game_rng = super::Win95GameRand::new(1);
    let mut lfsr = super::Win95Lfsr::new(1);
    pe_city.refresh_structure_microsim_yearly_with_rngs(
        100_000,
        100,
        0x1_0000,
        100,
        100,
        100,
        &mut crt,
        &mut game_rng,
        &mut lfsr,
    );
    let fire = pe_city
        .microsim_at(fire_slot)
        .expect("PE fire row remains active");
    assert_eq!((fire.stat1, fire.stat2), (0x8000, 2049));
}

#[test]
fn mac_police_arrest_subtotal_saturates_before_row_word_truncation() {
    let mut city = City::new_blank(
        "/tmp/sc2000-mac-police-arrest-saturation.sc2",
        "MacPoliceArrestSaturation",
    )
    .expect("blank city");
    city.kind = CityFileKind::MacCity;
    // Keep one police census cell so the raw arrest value can exceed a
    // WORD even though the row itself stores only the low WORD.
    city.set_mac_state_misc_u32_at(MAC_XBLD_CENSUS_MISC_OFFSET + 0xd2 * 4, 1);
    city.set_mac_state_misc_u32_at(MAC_XBLD_CENSUS_MISC_OFFSET + 0xd8 * 4, 16);
    city.set_mac_state_misc_u32_at(0x002c, 350_000);
    city.set_mac_state_misc_u32_at(0x102c, 20_000);
    let _police = city
        .provision_microsim_for_building(0xd2)
        .expect("police row");
    let prison = city
        .provision_microsim_for_building(0xd8)
        .expect("prison row");
    let prison_slot = usize::from(prison - MIN_SIM_TEXT_ENTRIES);

    let mut actual_rng = MacToolboxRng::new(MacToolboxRng::QUICKDRAW_INIT_GRAF_SEED);
    let mut expected_rng = actual_rng;
    let _police_jitter = expected_rng.random_u16() & 0x0f;
    let expected_candidate = 10_000 + (expected_rng.random_u16() & 0x03ff);
    let expected_per_hundred = expected_candidate / 100;
    let expected_random_divisor = expected_per_hundred - 90;
    let expected_overcrowding = (expected_rng.random_u16() % expected_random_divisor) as u8;
    let expected_next = expected_rng.random_u16();

    city.refresh_mac_monthly_random_xmic_rows(100, 0, 0, 0, 0, false, &mut actual_rng);

    let prison = city
        .microsim_at(prison_slot)
        .expect("prison row remains active");
    assert_eq!(prison.stat1, 1_000, "inmate capacity still applies");
    assert_eq!(
        prison.stat3, expected_per_hundred,
        "the native arrest subtotal saturates at 0xffff before the prison overflow branch"
    );
    assert_eq!(prison.stat0, expected_overcrowding);
    assert_eq!(actual_rng.random_u16(), expected_next);
}

#[test]
fn mac_monthly_stadium_uses_saved_census_and_conditional_quickdraw() {
    let mut city =
        City::new_blank("/tmp/sc2000-mac-stadium-monthly.sc2", "MacStadium").expect("blank city");
    city.kind = CityFileKind::MacCity;
    city.set_mac_state_misc_u32_at(MAC_XBLD_CENSUS_MISC_OFFSET + 0xd7 * 4, 4);
    city.set_mac_state_misc_u32_at(0x102c, 8_000);
    let overlay = city.provision_microsim_for_building(0xd7).unwrap();
    let index = usize::from(overlay - MIN_SIM_TEXT_ENTRIES);
    city.set_microsim_stat0(index, 0x55);
    city.set_microsim_stat1(index, 0x1234);
    city.set_microsim_stat2(index, 0x5678);
    city.set_microsim_stat3(index, 0x9abc);

    let mut expected_rng = MacToolboxRng::new(MacToolboxRng::QUICKDRAW_INIT_GRAF_SEED);
    let expected_attendance = 1_600_u16.wrapping_add(expected_rng.random_u16() & 0xff);
    let expected_grade = ((expected_rng.random_u16() & 0x1f) + 9) as u8;
    let next_expected = expected_rng.random_u16();
    let mut actual_rng = MacToolboxRng::new(MacToolboxRng::QUICKDRAW_INIT_GRAF_SEED);

    city.refresh_mac_monthly_stadium_microsims(&mut actual_rng);

    let row = city.microsim_at(index).unwrap();
    assert_eq!(
        row.stat1, expected_attendance,
        "native attendance is XMIC +2"
    );
    assert_eq!(row.stat0, expected_grade);
    assert_eq!((row.stat2, row.stat3), (0x5678, 0x9abc));
    assert_eq!(
        actual_rng.random_u16(),
        next_expected,
        "exactly two draws below overflow"
    );

    city.set_mac_state_misc_u32_at(0x102c, 120_000);
    let mut overflow_expected = MacToolboxRng::new(MacToolboxRng::QUICKDRAW_INIT_GRAF_SEED);
    let overflow_candidate = 25_000_u16.wrapping_sub(overflow_expected.random_u16() & 0xff);
    let overflow_attendance = overflow_candidate
        .min((120_000 / 5) as u16)
        .wrapping_add(overflow_expected.random_u16() & 0xff);
    let overflow_grade = ((overflow_expected.random_u16() & 0x1f) + 9) as u8;
    let next_overflow_expected = overflow_expected.random_u16();
    let mut overflow_actual = MacToolboxRng::new(MacToolboxRng::QUICKDRAW_INIT_GRAF_SEED);
    city.refresh_mac_monthly_stadium_microsims(&mut overflow_actual);
    let row = city.microsim_at(index).unwrap();
    assert_eq!(row.stat1, overflow_attendance);
    assert_eq!(row.stat2, 0x5678, "native +4 remains untouched");
    assert_eq!(row.stat0, overflow_grade);
    assert_eq!(
        overflow_actual.random_u16(),
        next_overflow_expected,
        "overflow adds one draw"
    );
}

#[test]
fn mac_monthly_dispatch_stadium_writes_attendance_to_xmic_plus_two() {
    let mut city = City::new_blank(
        "/tmp/sc2000-mac-stadium-dispatch-offset.sc2",
        "MacStadiumDispatchOffset",
    )
    .expect("blank city");
    city.kind = CityFileKind::MacCity;
    city.set_mac_state_misc_u32_at(MAC_XBLD_CENSUS_MISC_OFFSET + 0xd7 * 4, 4);
    city.set_mac_state_misc_u32_at(0x102c, 8_000);
    let overlay = city.provision_microsim_for_building(0xd7).unwrap();
    let index = usize::from(overlay - MIN_SIM_TEXT_ENTRIES);
    city.set_microsim_stat1(index, 0x1234);
    city.set_microsim_stat2(index, 0x5678);
    city.set_microsim_stat3(index, 0x9abc);

    let mut expected_rng = MacToolboxRng::new(MacToolboxRng::QUICKDRAW_INIT_GRAF_SEED);
    let expected_attendance = 1_600_u16.wrapping_add(expected_rng.random_u16() & 0xff);
    let expected_grade = ((expected_rng.random_u16() & 0x1f) + 9) as u8;
    let next_expected = expected_rng.random_u16();
    let mut actual_rng = MacToolboxRng::new(MacToolboxRng::QUICKDRAW_INIT_GRAF_SEED);

    city.refresh_mac_monthly_random_xmic_rows(0, 0, 0, 0, 0, false, &mut actual_rng);

    let row = city.microsim_at(index).unwrap();
    assert_eq!(row.stat1, expected_attendance);
    assert_eq!(
        row.stat2, 0x5678,
        "native stadium arm does not write XMIC +4"
    );
    assert_eq!(row.stat3, 0x9abc);
    assert_eq!(row.stat0, expected_grade);
    assert_eq!(actual_rng.random_u16(), next_expected);
}

#[test]
fn mac_monthly_school_uses_interleaved_saved_counters_and_two_draws() {
    let mut city =
        City::new_blank("/tmp/sc2000-mac-school-monthly.sc2", "MacSchool").expect("blank city");
    city.kind = CityFileKind::MacCity;
    city.set_mac_state_misc_u32_at(MAC_XBLD_CENSUS_MISC_OFFSET + 0xd6 * 4, 18);
    city.set_mac_state_misc_u32_at(0x0088, 600);
    city.set_mac_state_misc_u32_at(0x0094, 0);
    city.set_mac_state_misc_u32_at(0x102c, 30_000);
    let overlay = city.provision_microsim_for_building(0xd6).unwrap();
    let index = usize::from(overlay - MIN_SIM_TEXT_ENTRIES);
    city.set_microsim_stat0(index, 8);
    city.set_microsim_stat1(index, 0x1111);
    city.set_microsim_stat2(index, 0x2222);
    city.set_microsim_stat3(index, 0x3333);

    let mut expected_rng = MacToolboxRng::new(MacToolboxRng::QUICKDRAW_INIT_GRAF_SEED);
    let students = 300_u16 + (expected_rng.random_u16() & 0x0f);
    let teachers = 44_u16 + (expected_rng.random_u16() & 3);
    let ratio = students * 4 / teachers;
    let expected_grade = if ratio < 50 {
        12
    } else if ratio < 111 {
        ((110 - ratio) / 5) as u8
    } else {
        0
    };
    let next_expected = expected_rng.random_u16();
    let mut actual_rng = MacToolboxRng::new(MacToolboxRng::QUICKDRAW_INIT_GRAF_SEED);

    city.refresh_mac_monthly_school_microsims(80, &mut actual_rng);

    let row = city.microsim_at(index).unwrap();
    assert_eq!(row.stat1, students);
    assert_eq!(row.stat2, teachers);
    assert_eq!(row.stat3, 20, "funding is divided by four");
    assert_eq!(row.stat0, expected_grade);
    assert_eq!(
        actual_rng.random_u16(),
        next_expected,
        "exactly two draws below overflow"
    );

    city.set_mac_state_misc_u32_at(0x0088, 4_000);
    city.set_microsim_stat0(index, 8);
    let mut overflow_expected = MacToolboxRng::new(MacToolboxRng::QUICKDRAW_INIT_GRAF_SEED);
    let _initial_jitter = overflow_expected.random_u16() & 0x0f;
    let overflow_students = 1_500_u16 + (overflow_expected.random_u16() & 0xff);
    let overflow_teachers = 44_u16 + (overflow_expected.random_u16() & 3);
    let next_overflow_expected = overflow_expected.random_u16();
    let mut overflow_actual = MacToolboxRng::new(MacToolboxRng::QUICKDRAW_INIT_GRAF_SEED);
    city.refresh_mac_monthly_school_microsims(80, &mut overflow_actual);
    let row = city.microsim_at(index).unwrap();
    assert_eq!(
        row.stat1, 1_500,
        "student capacity caps overflow replacement"
    );
    assert_eq!(row.stat2, overflow_teachers);
    assert_eq!(
        overflow_actual.random_u16(),
        next_overflow_expected,
        "overflow consumes a third draw"
    );
    assert_eq!(overflow_students.min(1_500), row.stat1);
}

#[test]
fn mac_monthly_prison_consumes_arrests_and_publishes_next_police_gate() {
    let mut city =
        City::new_blank("/tmp/sc2000-mac-prison-monthly.sc2", "MacPrison").expect("blank city");
    city.kind = CityFileKind::MacCity;
    city.set_mac_state_misc_u32_at(MAC_XBLD_CENSUS_MISC_OFFSET + 0xd8 * 4, 32);
    city.set_mac_state_misc_u32_at(0x102c, 300_000);
    city.mac_monthly_arrest_total = 100;
    let ordinary = city.provision_microsim_for_building(0xd8).unwrap();
    let overflow = city.provision_microsim_for_building(0xd8).unwrap();
    let ordinary_slot = usize::from(ordinary - MIN_SIM_TEXT_ENTRIES);
    let overflow_slot = usize::from(overflow - MIN_SIM_TEXT_ENTRIES);
    city.set_microsim_stat1(ordinary_slot, 120);
    city.set_microsim_stat1(overflow_slot, u16::MAX);

    let mut expected_rng = MacToolboxRng::new(MacToolboxRng::QUICKDRAW_INIT_GRAF_SEED);
    let overflow_candidate = 10_000_u16 + (expected_rng.random_u16() & 0x03ff);
    let overflow_inmates_per_hundred = overflow_candidate / 100;
    let overflow_random_word = expected_rng.random_u16();
    let funding_adjustment = (100_i32 - 80) / 10;
    let overflow_random_divisor = i32::from(overflow_inmates_per_hundred) - 90 + funding_adjustment;
    let overflow_random = i32::from(overflow_random_word) % overflow_random_divisor;
    let next_expected = expected_rng.random_u16();
    let mut actual_rng = MacToolboxRng::new(MacToolboxRng::QUICKDRAW_INIT_GRAF_SEED);

    city.refresh_mac_monthly_prison_microsims(80, &mut actual_rng);

    let ordinary = city.microsim_at(ordinary_slot).unwrap();
    assert_eq!(
        ordinary.stat1, 140,
        "arrest share plus prior inmates minus floor quarter"
    );
    assert_eq!(
        ordinary.stat2, 240,
        "guard capacity uses triple police funding"
    );
    assert_eq!(
        ordinary.stat3, 1,
        "pre-cap candidate contributes per hundred"
    );
    assert_eq!(
        ordinary.stat0, 0,
        "fewer than 91 inmates per hundred clears field"
    );
    let overflow = city.microsim_at(overflow_slot).unwrap();
    assert_eq!(overflow.stat1, overflow_candidate);
    assert_eq!(overflow.stat2, 240);
    assert_eq!(overflow.stat3, overflow_inmates_per_hundred);
    assert_eq!(overflow.stat0, overflow_random as u8);
    assert_eq!(
        city.mac_monthly_arrest_total(),
        0,
        "subtotal is consumed once"
    );
    assert_eq!(
        city.mac_state_misc_u32_at(1039 * super::MISC_FIELD_LEN),
        Some(1),
        "the finalized gate is published for the next police pass"
    );
    assert_eq!(
        actual_rng.random_u16(),
        next_expected,
        "overflow and high-population branches draw twice"
    );
}

#[test]
fn mac_prison_overcrowding_uses_native_32_bit_dividend() {
    assert_eq!(super::mac_prison_overcrowding_field(100, 80, 0), 0);
    assert_eq!(super::mac_prison_overcrowding_field(100, 90, 0), 0);
    assert_eq!(super::mac_prison_overcrowding_field(100, 100, 7), 7);
    assert_eq!(super::mac_prison_overcrowding_field(100, 120, 7), 7);
    assert_eq!(super::mac_prison_overcrowding_field(100, 0x1_0000, 7), 7);
    assert_eq!(super::mac_prison_guard_candidate(0x1_0000), 0);
    assert_eq!(super::mac_prison_guard_candidate(0x8000_0000), 0);
}

#[test]
fn mac_monthly_dispatch_prison_overcrowding_uses_divsl_remainder() {
    let mut city = City::new_blank(
        "/tmp/sc2000-mac-prison-dispatch-divsl.sc2",
        "MacPrisonDispatchDivsl",
    )
    .expect("blank city");
    city.kind = CityFileKind::MacCity;
    city.set_mac_state_misc_u32_at(MAC_XBLD_CENSUS_MISC_OFFSET + 0xd8 * 4, 16);
    city.set_mac_state_misc_u32_at(0x102c, 300_000);
    let prison = city.provision_microsim_for_building(0xd8).unwrap();
    let slot = usize::from(prison - MIN_SIM_TEXT_ENTRIES);
    city.set_microsim_stat1(slot, 20_000);

    let mut expected_rng = MacToolboxRng::new(MacToolboxRng::QUICKDRAW_INIT_GRAF_SEED);
    let inmate_candidate = 10_000_u16 + (expected_rng.random_u16() & 0x03ff);
    let inmates_per_hundred = inmate_candidate / 100;
    let random_word = expected_rng.random_u16();
    let funding_adjustment = (100_i32 - 80) / 10;
    let divisor = i32::from(inmates_per_hundred) - 90 + funding_adjustment;
    let expected_stat0 = (i32::from(random_word) % divisor) as u8;
    let next_expected_rng = expected_rng.random_u16();
    let mut actual_rng = MacToolboxRng::new(MacToolboxRng::QUICKDRAW_INIT_GRAF_SEED);

    city.refresh_mac_monthly_random_xmic_rows(80, 0, 0, 0, 0, false, &mut actual_rng);

    assert_eq!(city.microsim_at(slot).unwrap().stat0, expected_stat0);
    assert_eq!(actual_rng.random_u16(), next_expected_rng);
}

#[test]
fn mac_monthly_college_uses_saved_counter_and_conditional_overflow_draw() {
    let mut city =
        City::new_blank("/tmp/sc2000-mac-college-monthly.sc2", "MacCollege").expect("blank city");
    city.kind = CityFileKind::MacCity;
    city.set_mac_state_misc_u32_at(MAC_XBLD_CENSUS_MISC_OFFSET + 0xd9 * 4, 32);
    city.set_mac_state_misc_u32_at(0x00a0, 1_600);
    city.set_mac_state_misc_u32_at(0x102c, 30_000);
    let overlay = city.provision_microsim_for_building(0xd9).unwrap();
    let index = usize::from(overlay - MIN_SIM_TEXT_ENTRIES);
    city.set_microsim_stat0(index, 10);
    city.set_microsim_stat1(index, 0x1111);
    city.set_microsim_stat2(index, 0x2222);
    city.set_microsim_stat3(index, 0x3333);

    let mut expected_rng = MacToolboxRng::new(MacToolboxRng::QUICKDRAW_INIT_GRAF_SEED);
    let students = 800_u16 + (expected_rng.random_u16() & 0x1f);
    let professors = 152_u16 + (expected_rng.random_u16() & 0x0f);
    let next_expected = expected_rng.random_u16();
    let mut actual_rng = MacToolboxRng::new(MacToolboxRng::QUICKDRAW_INIT_GRAF_SEED);

    city.refresh_mac_monthly_college_microsims(80, &mut actual_rng);

    let row = city.microsim_at(index).unwrap();
    assert_eq!(row.stat1, students);
    assert_eq!(row.stat2, professors);
    assert_eq!(row.stat3, 80, "college funding is copied to the row");
    assert_eq!(row.stat0, 12);
    assert_eq!(
        actual_rng.random_u16(),
        next_expected,
        "two draws below overflow"
    );

    city.set_mac_state_misc_u32_at(0x00a0, 20_000);
    city.set_microsim_stat0(index, 10);
    let mut overflow_expected = MacToolboxRng::new(MacToolboxRng::QUICKDRAW_INIT_GRAF_SEED);
    let _first_jitter = overflow_expected.random_u16() & 0x1f;
    let _replacement_jitter = overflow_expected.random_u16() & 0x01ff;
    let overflow_professors = 152_u16 + (overflow_expected.random_u16() & 0x0f);
    let next_overflow_expected = overflow_expected.random_u16();
    let mut overflow_actual = MacToolboxRng::new(MacToolboxRng::QUICKDRAW_INIT_GRAF_SEED);
    city.refresh_mac_monthly_college_microsims(80, &mut overflow_actual);
    let row = city.microsim_at(index).unwrap();
    assert_eq!(row.stat1, 1_000, "college candidate is capped by f01a/30");
    assert_eq!(row.stat2, overflow_professors);
    assert_eq!(
        overflow_actual.random_u16(),
        next_overflow_expected,
        "overflow adds one draw"
    );
}

#[test]
fn mac_education_counters_divide_unsigned_before_signed_overflow_test() {
    fn assert_school_path(dispatch: bool) {
        let mut city = City::new_blank(
            "/tmp/sc2000-mac-school-unsigned-counter.sc2",
            "MacSchoolUnsignedCounter",
        )
        .expect("blank city");
        city.kind = CityFileKind::MacCity;
        city.set_mac_state_misc_u32_at(MAC_XBLD_CENSUS_MISC_OFFSET + 0xd6 * 4, 81);
        city.set_mac_state_misc_u32_at(0x0088, 0x8000_0000);
        city.set_mac_state_misc_u32_at(0x0094, 0);
        city.set_mac_state_misc_u32_at(0x102c, 30_000);
        let overlay = city.provision_microsim_for_building(0xd6).unwrap();
        let index = usize::from(overlay - MIN_SIM_TEXT_ENTRIES);
        city.set_microsim_stat0(index, 8);

        let mut expected_rng = MacToolboxRng::new(MacToolboxRng::QUICKDRAW_INIT_GRAF_SEED);
        let _initial_jitter = expected_rng.random_u16() & 0x0f;
        let _overflow_jitter = expected_rng.random_u16() & 0xff;
        let teachers = super::mac_school_teachers_raw(80, expected_rng.random_u16() & 3, 8);
        let next_expected = expected_rng.random_u16();
        let mut actual_rng = MacToolboxRng::new(MacToolboxRng::QUICKDRAW_INIT_GRAF_SEED);

        if dispatch {
            city.refresh_mac_monthly_random_xmic_rows(80, 0, 0, 80, 0, false, &mut actual_rng);
        } else {
            city.refresh_mac_monthly_school_microsims(80, &mut actual_rng);
        }

        let row = city.microsim_at(index).unwrap();
        assert_eq!(
            row.stat1, 1_500,
            "u32 / 9 reaches the signed overflow replacement"
        );
        assert_eq!(row.stat2, teachers as u16);
        assert_eq!(
            actual_rng.random_u16(),
            next_expected,
            "{} school path consumes initial jitter, overflow jitter, and teacher jitter",
            if dispatch { "slot-ordered" } else { "focused" }
        );
    }

    fn assert_college_path(dispatch: bool) {
        let mut city = City::new_blank(
            "/tmp/sc2000-mac-college-unsigned-counter.sc2",
            "MacCollegeUnsignedCounter",
        )
        .expect("blank city");
        city.kind = CityFileKind::MacCity;
        city.set_mac_state_misc_u32_at(MAC_XBLD_CENSUS_MISC_OFFSET + 0xd9 * 4, 144);
        city.set_mac_state_misc_u32_at(0x00a0, 0x8000_0000);
        city.set_mac_state_misc_u32_at(0x102c, 30_000);
        let overlay = city.provision_microsim_for_building(0xd9).unwrap();
        let index = usize::from(overlay - MIN_SIM_TEXT_ENTRIES);
        city.set_microsim_stat0(index, 10);

        let mut expected_rng = MacToolboxRng::new(MacToolboxRng::QUICKDRAW_INIT_GRAF_SEED);
        let _initial_jitter = expected_rng.random_u16() & 0x1f;
        let _overflow_jitter = expected_rng.random_u16() & 0x01ff;
        let professors =
            super::mac_college_professors_raw(80, expected_rng.random_u16() & 0x0f, 10);
        let next_expected = expected_rng.random_u16();
        let mut actual_rng = MacToolboxRng::new(MacToolboxRng::QUICKDRAW_INIT_GRAF_SEED);

        if dispatch {
            city.refresh_mac_monthly_random_xmic_rows(0, 0, 0, 0, 80, false, &mut actual_rng);
        } else {
            city.refresh_mac_monthly_college_microsims(80, &mut actual_rng);
        }

        let row = city.microsim_at(index).unwrap();
        assert_eq!(
            row.stat1, 1_000,
            "u32 / 9 reaches the signed overflow replacement"
        );
        assert_eq!(row.stat2, professors as u16);
        assert_eq!(
            actual_rng.random_u16(),
            next_expected,
            "{} college path consumes initial jitter, overflow jitter, and professor jitter",
            if dispatch { "slot-ordered" } else { "focused" }
        );
    }

    assert_school_path(false);
    assert_college_path(false);
    assert_school_path(true);
    assert_college_path(true);
}

#[test]
fn mac_education_teacher_candidates_keep_funding_dword_width() {
    assert_eq!(super::mac_school_teachers_raw(80, 2, 8), 46);
    assert_eq!(super::mac_school_funding_quarters(80), 20);
    assert_eq!(
        super::mac_school_funding_quarters(u32::MAX),
        0,
        "school stat3 uses signed DIVSL.L before storing the low word"
    );
    assert_eq!(
        super::mac_school_teachers_raw(0x4000_0000, 0, 12),
        0,
        "school MULU.L wraps to a signed negative before DIVSL/JNS"
    );
    assert_eq!(super::mac_college_professors_raw(80, 3, 10), 155);
    assert_eq!(
        super::mac_college_professors_raw(0x0000_8000, 7, 12),
        7,
        "college narrows only after full-dword addition and the JNS clamp"
    );
}

#[test]
fn mac_monthly_statue_consumes_one_quickdraw_word_and_preserves_other_fields() {
    let mut city =
        City::new_blank("/tmp/sc2000-mac-statue-monthly.sc2", "MacStatue").expect("blank city");
    city.kind = CityFileKind::MacCity;
    let overlay = city.provision_microsim_for_building(0xdb).unwrap();
    let index = usize::from(overlay - MIN_SIM_TEXT_ENTRIES);
    city.set_microsim_stat0(index, 5);
    city.set_microsim_stat1(index, 45);
    city.set_microsim_stat2(index, 0x2222);
    city.set_microsim_stat3(index, 0x3333);

    let seed = 0x1234_5678;
    let mut expected_rng = MacToolboxRng::new(seed);
    let native_random = expected_rng.random_i16();
    assert!(
        native_random < 0,
        "fixture must cover DIVU of a negative word"
    );
    let expected_stat2 = (native_random as u16) % 42;
    let next_expected = expected_rng.random_u16();
    let mut actual_rng = MacToolboxRng::new(seed);

    city.refresh_mac_monthly_statue_microsims(&mut actual_rng);

    let row = city.microsim_at(index).unwrap();
    assert_eq!(row.stat0, 5);
    assert_eq!(row.stat1, 45);
    assert_eq!(row.stat2, expected_stat2);
    assert_eq!(row.stat3, 0x3333);
    assert_eq!(actual_rng.random_u16(), next_expected);
}

#[test]
fn mac_monthly_water_rows_use_prior_coverage_and_three_draws_each() {
    let mut city =
        City::new_blank("/tmp/sc2000-mac-water-monthly.sc2", "MacWater").expect("blank city");
    city.kind = CityFileKind::MacCity;
    city.set_mac_state_misc_u32_at(1049 * 4, 96);
    city.set_mac_state_misc_u32_at(0x102c, 4_000);
    let treatment = city.provision_microsim_for_building(0xf4).unwrap();
    let desalination = city.provision_microsim_for_building(0xfa).unwrap();
    let treatment = usize::from(treatment - MIN_SIM_TEXT_ENTRIES);
    let desalination = usize::from(desalination - MIN_SIM_TEXT_ENTRIES);
    city.set_microsim_stat3(treatment, 0x3333);
    city.set_microsim_stat3(desalination, 0x4444);

    let mut expected_rng = MacToolboxRng::new(MacToolboxRng::QUICKDRAW_INIT_GRAF_SEED);
    let expected_row = |rng: &mut MacToolboxRng| {
        let coverage = 96_u8.wrapping_add((rng.random_u16() & 7) as u8);
        let utilization = rng.random_u16() % 100;
        let capacity = (0x87_u32 + u32::from(rng.random_u16() & 0x1f)).min(80) as u16;
        (coverage, utilization, capacity)
    };
    let treatment_expected = expected_row(&mut expected_rng);
    let desalination_expected = expected_row(&mut expected_rng);
    let next_expected = expected_rng.random_u16();
    let mut actual_rng = MacToolboxRng::new(MacToolboxRng::QUICKDRAW_INIT_GRAF_SEED);

    city.refresh_mac_monthly_water_microsims(&mut actual_rng);

    let treatment_row = city.microsim_at(treatment).unwrap();
    assert_eq!(
        (
            treatment_row.stat0,
            treatment_row.stat1,
            treatment_row.stat2
        ),
        treatment_expected
    );
    assert_eq!(treatment_row.stat3, 0x3333);
    let desalination_row = city.microsim_at(desalination).unwrap();
    assert_eq!(
        (
            desalination_row.stat0,
            desalination_row.stat1,
            desalination_row.stat2
        ),
        desalination_expected
    );
    assert_eq!(desalination_row.stat3, 0x4444);
    assert_eq!(actual_rng.random_u16(), next_expected);
}

#[test]
fn mac_monthly_llama_dome_uses_four_ordered_draws_and_saved_growth_scalar() {
    let mut city =
        City::new_blank("/tmp/sc2000-mac-llama-monthly.sc2", "MacLlama").expect("blank city");
    city.kind = CityFileKind::MacCity;
    city.set_mac_state_misc_u32_at(0x102c, 8_000);
    let overlay = city.provision_microsim_for_building(0xff).unwrap();
    let index = usize::from(overlay - MIN_SIM_TEXT_ENTRIES);
    city.set_microsim_stat0(index, 1);
    city.set_microsim_stat1(index, 2);
    city.set_microsim_stat2(index, 3);
    city.set_microsim_stat3(index, 4);

    let mut expected_rng = MacToolboxRng::new(MacToolboxRng::QUICKDRAW_INIT_GRAF_SEED);
    let stat0 = expected_rng.random_u16() as u8;
    let value = u32::from(expected_rng.random_u16() & 0x03ff) + 1_000;
    let stat1 = value as u16;
    let stat2 = (u32::from(expected_rng.random_u16() & 0x007f) + value / 8) as u16;
    let stat3 = (u32::from(expected_rng.random_u16() & 0x003f) + value / 16) as u16;
    let next_expected = expected_rng.random_u16();
    let mut actual_rng = MacToolboxRng::new(MacToolboxRng::QUICKDRAW_INIT_GRAF_SEED);

    city.refresh_mac_monthly_llama_dome_microsims(&mut actual_rng);

    let row = city.microsim_at(index).unwrap();
    assert_eq!(
        (row.stat0, row.stat1, row.stat2, row.stat3),
        (stat0, stat1, stat2, stat3)
    );
    assert_eq!(actual_rng.random_u16(), next_expected);
}

#[test]
fn mac_monthly_random_xmic_dispatch_follows_interleaved_record_slots() {
    let mut city =
        City::new_blank("/tmp/sc2000-mac-xmic-order.sc2", "MacXmicOrder").expect("blank city");
    city.kind = CityFileKind::MacCity;
    city.set_mac_state_misc_u32_at(MAC_XBLD_CENSUS_MISC_OFFSET + 0xd6 * 4, 9);
    city.set_mac_state_misc_u32_at(MAC_XBLD_CENSUS_MISC_OFFSET + 0xd1 * 4, 9);
    city.set_mac_state_misc_u32_at(MAC_XBLD_CENSUS_MISC_OFFSET + 0xd2 * 4, 18);
    city.set_mac_state_misc_u32_at(MAC_XBLD_CENSUS_MISC_OFFSET + 0xd7 * 4, 4);
    city.set_mac_state_misc_u32_at(MAC_XBLD_CENSUS_MISC_OFFSET + 0xd8 * 4, 32);
    city.set_mac_state_misc_u32_at(MAC_XBLD_CENSUS_MISC_OFFSET + 0xd9 * 4, 16);
    city.set_mac_state_misc_u32_at(0x102c, 20_000);
    city.set_mac_state_misc_u32_at(1049 * 4, 96);
    city.set_mac_state_misc_u32_at(0x00a0, 100);
    city.set_mac_state_misc_u32_at(0x002c, 100);
    // Cursor 91 is the first editable 11-row lane entry; it is unrelated
    // to the police gate at cursor 1039.
    city.set_mac_state_misc_u32_at(91 * 4, 2);
    city.set_mac_state_misc_u32_at(1039 * 4, 2);
    city.set_mac_state_misc_u32_at(MAC_POWER_SUMMARY_MISC_OFFSET, 500);

    // Deliberately interleave classes by XMIC slot rather than class ID.
    let statue = city.provision_microsim_for_building(0xdb).unwrap();
    let school = city.provision_microsim_for_building(0xd6).unwrap();
    let police = city.provision_microsim_for_building(0xd2).unwrap();
    let hospital = city.provision_microsim_for_building(0xd1).unwrap();
    let fire = city.provision_microsim_for_building(0xd3).unwrap();
    let prison = city.provision_microsim_for_building(0xd8).unwrap();
    let stadium = city.provision_microsim_for_building(0xd7).unwrap();
    let water = city.provision_microsim_for_building(0xf4).unwrap();
    let college = city.provision_microsim_for_building(0xd9).unwrap();
    let plant = city.provision_microsim_for_building(0xc9).unwrap();
    let llama = city.provision_microsim_for_building(0xff).unwrap();
    let slots = [
        statue, school, police, hospital, fire, prison, stadium, water, college, plant, llama,
    ]
    .map(|entry| usize::from(entry - MIN_SIM_TEXT_ENTRIES));
    assert!(slots.windows(2).all(|pair| pair[0] < pair[1]));
    city.set_microsim_stat0(slots[1], 8);
    city.set_microsim_stat0(slots[3], 6);
    city.set_microsim_stat0(slots[5], 3);
    city.set_microsim_stat0(slots[8], 10);

    let mut expected_rng = MacToolboxRng::new(MacToolboxRng::QUICKDRAW_INIT_GRAF_SEED);
    let expected_statue = expected_rng.random_u16() % 42;
    let expected_students = expected_rng.random_u16() & 0x0f;
    let expected_teachers = 44 + (expected_rng.random_u16() & 3);
    let expected_arrests = 1 + (expected_rng.random_u16() & 0x0f);
    let expected_patient_jitter = expected_rng.random_u16() & 0x0f;
    let expected_patients =
        (20_000 / 25 + u32::from(expected_patient_jitter)).min(20_000 / 30) as u16;
    let expected_physician_jitter = expected_rng.random_u16() & 7;
    let expected_physicians = (80 + expected_physician_jitter + 2 * 6 - 24) as u16;
    let expected_fire = (expected_rng.random_u16() % 20) + 2;
    let expected_stadium_attendance = 4_000_u16.wrapping_add(expected_rng.random_u16() & 0xff);
    let expected_stadium_grade = ((expected_rng.random_u16() & 0x1f) + 9) as u8;
    let expected_water_coverage = 96 + (expected_rng.random_u16() & 7) as u8;
    let expected_water_utilization = expected_rng.random_u16() % 100;
    let expected_water_capacity = 0x87 + (expected_rng.random_u16() & 0x1f);
    let expected_college_students = 100 + (expected_rng.random_u16() & 0x1f);
    let expected_professors = 152 + (expected_rng.random_u16() & 0x0f);
    let expected_plant_power = 500 + (expected_rng.random_u16() & 7);
    let expected_llama_stat0 = expected_rng.random_u16() as u8;
    let llama_value = u32::from(expected_rng.random_u16() & 0x03ff) + 2_500;
    let expected_llama_stat1 = llama_value as u16;
    let expected_llama_stat2 =
        (u32::from(expected_rng.random_u16() & 0x7f) + llama_value / 8) as u16;
    let expected_llama_stat3 =
        (u32::from(expected_rng.random_u16() & 0x3f) + llama_value / 16) as u16;
    let next_expected = expected_rng.random_u16();
    let mut actual_rng = MacToolboxRng::new(MacToolboxRng::QUICKDRAW_INIT_GRAF_SEED);

    city.refresh_mac_monthly_random_xmic_rows(100, 80, 80, 80, 80, false, &mut actual_rng);

    assert_eq!(city.microsim_at(slots[0]).unwrap().stat2, expected_statue);
    assert_eq!(city.microsim_at(slots[1]).unwrap().stat1, expected_students);
    assert_eq!(city.microsim_at(slots[1]).unwrap().stat2, expected_teachers);
    assert_eq!(city.microsim_at(slots[2]).unwrap().stat3, expected_arrests);
    assert_eq!(city.microsim_at(slots[3]).unwrap().stat1, expected_patients);
    assert_eq!(
        city.microsim_at(slots[3]).unwrap().stat2,
        expected_physicians
    );
    assert_eq!(city.microsim_at(slots[4]).unwrap().stat3, expected_fire);
    assert_eq!(
        city.microsim_at(slots[5]).unwrap().stat1,
        expected_arrests / 2
    );
    assert_eq!(city.microsim_at(slots[5]).unwrap().stat3, 0);
    assert_eq!(
        city.microsim_at(slots[6]).unwrap().stat1,
        expected_stadium_attendance,
        "stadium attendance is stored at native XMIC +2"
    );
    assert_eq!(city.microsim_at(slots[6]).unwrap().stat2, 0);
    assert_eq!(
        city.microsim_at(slots[6]).unwrap().stat0,
        expected_stadium_grade
    );
    assert_eq!(
        city.microsim_at(slots[7]).unwrap().stat0,
        expected_water_coverage
    );
    assert_eq!(
        city.microsim_at(slots[7]).unwrap().stat1,
        expected_water_utilization
    );
    assert_eq!(
        city.microsim_at(slots[7]).unwrap().stat2,
        expected_water_capacity
    );
    assert_eq!(
        city.microsim_at(slots[8]).unwrap().stat1,
        expected_college_students
    );
    assert_eq!(
        city.microsim_at(slots[8]).unwrap().stat2,
        expected_professors
    );
    assert_eq!(
        city.microsim_at(slots[9]).unwrap().stat2,
        expected_plant_power
    );
    assert_eq!(
        (
            city.microsim_at(slots[10]).unwrap().stat0,
            city.microsim_at(slots[10]).unwrap().stat1,
            city.microsim_at(slots[10]).unwrap().stat2,
            city.microsim_at(slots[10]).unwrap().stat3,
        ),
        (
            expected_llama_stat0,
            expected_llama_stat1,
            expected_llama_stat2,
            expected_llama_stat3,
        )
    );
    assert_eq!(city.mac_monthly_arrest_total(), 0);
    assert_eq!(city.mac_state_misc_u32_at(91 * 4), Some(2));
    assert_eq!(city.mac_state_misc_u32_at(1039 * 4), Some(1));
    assert_eq!(actual_rng.random_u16(), next_expected);
}

#[test]
fn mac_monthly_zoo_uses_four_app_lcg_draws_per_active_row() {
    let mut city =
        City::new_blank("/tmp/sc2000-mac-zoo-monthly.sc2", "MacZoo").expect("blank city");
    city.kind = CityFileKind::MacCity;
    let first = city.provision_microsim_for_building(0xda).unwrap();
    let ignored = city.provision_microsim_for_building(0xd2).unwrap();
    let second = city.provision_microsim_for_building(0xda).unwrap();
    let first_index = usize::from(first - MIN_SIM_TEXT_ENTRIES);
    let ignored_index = usize::from(ignored - MIN_SIM_TEXT_ENTRIES);
    let second_index = usize::from(second - MIN_SIM_TEXT_ENTRIES);

    let mut expected_rng = MacGameLcgRng::new(MacGameLcgRng::INITIAL_STATE);
    let expected_first = [
        expected_rng.bounded(100),
        expected_rng.bounded(100),
        expected_rng.bounded(100),
        expected_rng.bounded(100),
    ];
    let expected_second = [
        expected_rng.bounded(100),
        expected_rng.bounded(100),
        expected_rng.bounded(100),
        expected_rng.bounded(100),
    ];
    let expected_next = expected_rng.bounded(100);
    let mut actual_rng = MacGameLcgRng::new(MacGameLcgRng::INITIAL_STATE);

    city.refresh_mac_monthly_zoo_microsims(&mut actual_rng);

    let first_row = city.microsim_at(first_index).unwrap();
    assert_eq!(
        (
            first_row.stat0,
            first_row.stat1,
            first_row.stat2,
            first_row.stat3
        ),
        (
            expected_first[0] as u8,
            expected_first[1],
            expected_first[2],
            expected_first[3]
        )
    );
    let second_row = city.microsim_at(second_index).unwrap();
    assert_eq!(
        (
            second_row.stat0,
            second_row.stat1,
            second_row.stat2,
            second_row.stat3
        ),
        (
            expected_second[0] as u8,
            expected_second[1],
            expected_second[2],
            expected_second[3]
        )
    );
    assert_eq!(city.microsim_at(ignored_index).unwrap().stat0, 0);
    assert_eq!(
        actual_rng.bounded(100),
        expected_next,
        "eight zoo draws total"
    );
}

#[test]
fn mac_monthly_arcologies_use_saved_counts_tax_rates_and_water_result() {
    let mut city =
        City::new_blank("/tmp/sc2000-mac-arco-monthly.sc2", "MacArco").expect("blank city");
    city.kind = CityFileKind::MacCity;
    city.set_mac_state_misc_u32_at(0x102c, 8_000);
    for (tile_id, count) in [(0xfb_u8, 16), (0xfc, 32), (0xfd, 0), (0xfe, 16)] {
        city.set_mac_state_misc_u32_at(
            MAC_XBLD_CENSUS_MISC_OFFSET + usize::from(tile_id) * 4,
            count,
        );
    }
    let first = city.provision_microsim_for_building(0xfb).unwrap();
    let second = city.provision_microsim_for_building(0xfc).unwrap();
    let first_index = usize::from(first - MIN_SIM_TEXT_ENTRIES);
    let second_index = usize::from(second - MIN_SIM_TEXT_ENTRIES);
    city.set_microsim_stat0(first_index, 5);
    city.set_microsim_stat2(first_index, 100);
    city.set_microsim_stat0(second_index, 5);
    city.set_microsim_stat2(second_index, 200);

    // Saved counts sum to 64; arithmetic shift by four gives the helper's
    // divisor 80, so 8,000/80 caps the target at 100 before water is added.
    city.refresh_mac_monthly_arcology_microsims([7, 7, 7], 50);

    assert_eq!(city.microsim_at(first_index).unwrap().stat2, 252);
    assert_eq!(city.microsim_at(second_index).unwrap().stat2, 354);
    assert_eq!(city.mac_state_misc_u32_at(1032 * 4), Some(606));
}

#[test]
fn mac_monthly_utility_uses_pe_census_for_win95_city_files() {
    let mut city =
        City::new_blank("/tmp/sc2000-mac-on-pe-utilities.sc2", "MacOnPe").expect("blank city");
    city.set_building(20, 20, 198);
    city.set_building(21, 20, 199);
    city.set_building(22, 20, 200);
    city.pe_rebuild_dw_tile_count_434f80();
    city.provision_microsim_for_building(198)
        .expect("hydro system row");
    city.provision_microsim_for_building(200)
        .expect("wind system row");

    city.refresh_mac_monthly_utility_microsims();

    assert_eq!(
        city.microsim_at(super::MICROSIM_SYSTEM_HYDRO)
            .map(|row| (row.stat1, row.stat2)),
        Some((2, 40))
    );
    assert_eq!(
        city.microsim_at(super::MICROSIM_SYSTEM_WIND)
            .map(|row| (row.stat1, row.stat2)),
        Some((1, 4))
    );
}

#[test]
fn mac_monthly_library_update_uses_saved_tile_and_growth_counts() {
    let mut city =
        City::new_blank("/tmp/sc2000-mac-library-monthly.sc2", "MacLibrary").expect("blank city");
    city.kind = CityFileKind::MacCity;
    city.set_mac_state_misc_u32_at(MAC_XBLD_CENSUS_MISC_OFFSET + 0xf5 * 4, 9);
    city.set_mac_state_misc_u32_at(0x102c, 1_800);
    let overlay = city
        .provision_microsim_for_building(0xf5)
        .expect("library XMIC row");
    let index = usize::from(overlay - MIN_SIM_TEXT_ENTRIES);
    city.set_microsim_stat2(index, 1_000);
    city.set_microsim_stat3(index, 0x1234);

    city.refresh_mac_monthly_library_microsims(100);

    let row = city.microsim_at(index).expect("library row");
    assert_eq!(row.stat1, 100, "min(9 * (100*4), 1800/18)");
    assert_eq!(row.stat2, 1_450, "prior books plus 9 * (funding - 50)");
    assert_eq!(row.stat0, 12, "grade is capped at twelve");
    assert_eq!(row.stat3, 0x1234, "the Mac worker leaves stat3 unchanged");
}

#[test]
fn mac_monthly_library_grade_keeps_signed_wrapped_numerator() {
    let mut city = City::new_blank("/tmp/sc2000-mac-library-grade-wrap.sc2", "MacLibrary")
        .expect("blank city");
    city.kind = CityFileKind::MacCity;
    city.set_mac_state_misc_u32_at(MAC_XBLD_CENSUS_MISC_OFFSET + 0xf5 * 4, 1);
    city.set_mac_state_misc_u32_at(0x102c, 1);
    let overlay = city
        .provision_microsim_for_building(0xf5)
        .expect("library XMIC row");
    let index = usize::from(overlay - MIN_SIM_TEXT_ENTRIES);

    // 10,000,001 * 1 * 300 wraps to signed i32 -1,294,966,996.
    // DIVSL.L leaves that negative quotient below the upper cap of 12;
    // writing its low byte therefore stores 0x2c.
    city.refresh_mac_monthly_library_microsims(10_000_001);

    let row = city.microsim_at(index).expect("library row");
    assert_eq!(row.stat0, 0x2c);
    assert_eq!(row.stat1, 0);
    assert_eq!(row.stat2, 0);
}

#[test]
fn mac_monthly_city_hall_uses_growth_scalar_and_preserves_other_fields() {
    let mut city =
        City::new_blank("/tmp/sc2000-mac-city-hall-monthly.sc2", "MacHall").expect("blank city");
    city.kind = CityFileKind::MacCity;
    city.set_mac_state_misc_u32_at(0x102c, 90_000);
    let overlay = city
        .provision_microsim_for_building(0xd0)
        .expect("Mac City Hall XMIC row");
    let index = usize::from(overlay - MIN_SIM_TEXT_ENTRIES);
    city.set_microsim_stat0(index, 0x5a);
    city.set_microsim_stat2(index, 0x1234);
    city.set_microsim_stat3(index, 0xabcd);

    city.refresh_mac_monthly_city_hall_microsims();

    let row = city.microsim_at(index).expect("City Hall row");
    assert_eq!(row.tile_id, 0xd0);
    assert_eq!(row.stat1, 100, "min(200, 90_000 / 900)");
    assert_eq!(row.stat0, 0x5a);
    assert_eq!(row.stat2, 0x1234);
    assert_eq!(row.stat3, 0xabcd);

    city.set_mac_state_misc_u32_at(0x102c, 900_000);
    city.refresh_mac_monthly_city_hall_microsims();
    assert_eq!(city.microsim_at(index).map(|row| row.stat1), Some(200));

    // FUN_0001238a narrows the unsigned quotient to a WORD before the
    // signed compare. 33,333 narrows to 0x8235 (-32,203), so it wins over
    // the positive candidate 200 and is stored as its original bit pattern.
    city.set_mac_state_misc_u32_at(0x102c, 30_000_000);
    city.refresh_mac_monthly_city_hall_microsims();
    assert_eq!(city.microsim_at(index).map(|row| row.stat1), Some(0x8235));
}

#[test]
fn mac_monthly_marina_uses_zero_initialized_app_lfsr_and_preserves_other_fields() {
    let mut city =
        City::new_blank("/tmp/sc2000-mac-marina-monthly.sc2", "MacMarina").expect("blank city");
    city.kind = CityFileKind::MacCity;
    city.set_mac_state_misc_u32_at(MAC_XBLD_CENSUS_MISC_OFFSET + 0xf8 * 4, 6);
    city.set_zone(30, 30, 7);
    city.set_building(30, 30, 0xf8);
    assert_eq!(city.building_at(30, 30), 0xf8);
    assert_eq!(
        city.mac_xbld_tile_count(0xf8),
        6,
        "zone-7 PlaceTile dispatch preserves the ordinary XBLD census slot"
    );
    city.set_mac_state_misc_u32_at(0x102c, 7_200);
    let overlay = city
        .provision_microsim_for_building(0xf8)
        .expect("Mac marina XMIC row");
    let index = usize::from(overlay - MIN_SIM_TEXT_ENTRIES);
    city.set_microsim_stat0(index, 0x5a);
    city.set_microsim_stat2(index, 0x1234);
    city.set_microsim_stat3(index, 0xabcd);

    city.refresh_mac_monthly_marina_microsims();

    let row = city.microsim_at(index).expect("marina row");
    assert_eq!(row.stat1, 48, "min(7200/150, zero-LFSR + 6*8)");
    assert_eq!((row.stat0, row.stat2, row.stat3), (0x5a, 0x1234, 0xabcd));
}

#[test]
fn mac_monthly_transit_copies_tile_and_passenger_words_then_clears_inputs() {
    let mut city =
        City::new_blank("/tmp/sc2000-mac-transit-monthly.sc2", "MacTransit").expect("blank city");
    city.kind = CityFileKind::MacCity;
    for (tile, count) in [(0xe9_u8, 8), (0xec, 12), (0xed, 20)] {
        city.set_mac_state_misc_u32_at(MAC_XBLD_CENSUS_MISC_OFFSET + usize::from(tile) * 4, count);
    }
    let subway_overlay = city.provision_microsim_for_building(0xe9).unwrap();
    let bus_overlay = city.provision_microsim_for_building(0xec).unwrap();
    let rail_overlay = city.provision_microsim_for_building(0xed).unwrap();
    let row_index = |overlay: u8| usize::from(overlay - MIN_SIM_TEXT_ENTRIES);
    let (subway, bus, rail) = (
        row_index(subway_overlay),
        row_index(bus_overlay),
        row_index(rail_overlay),
    );
    city.set_microsim_stat2(subway, 0x1234);
    city.set_microsim_stat2(rail, 0x5678);
    city.set_microsim_stat0(bus, 0x5a);
    city.set_mac_state_misc_u32_at(1045 * 4, 0x0001_0002);
    city.set_mac_state_misc_u32_at(1046 * 4, 0x0002_0003);
    city.set_mac_state_misc_u32_at(1047 * 4, 0x0003_0004);
    for (slot, value) in [(97, 0x1111_2222), (98, 0x3333_4444), (99, 0x5555_6666)] {
        city.set_mac_state_misc_u32_at(slot * 4, value);
    }

    city.refresh_mac_monthly_transit_microsims();

    let subway_row = city.microsim_at(subway).unwrap();
    assert_eq!(
        (subway_row.stat1, subway_row.stat2, subway_row.stat3),
        (8, 0x1234, 4)
    );
    let bus_row = city.microsim_at(bus).unwrap();
    assert_eq!(
        (bus_row.stat0, bus_row.stat1, bus_row.stat2, bus_row.stat3),
        (0x5a, 3, 12, 2)
    );
    let rail_row = city.microsim_at(rail).unwrap();
    assert_eq!(
        (rail_row.stat1, rail_row.stat2, rail_row.stat3),
        (5, 0x5678, 3)
    );
    for offset in [1045, 1046, 1047] {
        assert_eq!(
            city.mac_state_misc_u32_at(offset * 4),
            Some(0),
            "the completed Mac month consumes and clears each passenger accumulator"
        );
    }
    for (slot, value) in [(97, 0x1111_2222), (98, 0x3333_4444), (99, 0x5555_6666)] {
        assert_eq!(city.mac_state_misc_u32_at(slot * 4), Some(value));
    }
}

#[test]
fn mac_first_open_transit_accumulators_seed_xmic_for_win_layout_city() {
    let mut city = City::new_blank(
        "/tmp/citygame-mac-first-open-transit.sc2",
        "MacFirstOpenTransit",
    )
    .expect("blank Win-layout city");
    city.kind = CityFileKind::WinSc2;
    let raw_accumulators = [0x1111_0002, 0x2222_0003, 0x3333_0004];
    for (mode, value) in raw_accumulators.into_iter().enumerate() {
        city.set_misc_u32_at((1045 + mode) * 4, value);
    }
    let subway = usize::from(
        city.provision_microsim_for_building(0xe9)
            .expect("subway XMIC row")
            - MIN_SIM_TEXT_ENTRIES,
    );
    let bus = usize::from(
        city.provision_microsim_for_building(0xec)
            .expect("bus XMIC row")
            - MIN_SIM_TEXT_ENTRIES,
    );
    let rail = usize::from(
        city.provision_microsim_for_building(0xed)
            .expect("rail XMIC row")
            - MIN_SIM_TEXT_ENTRIES,
    );

    city.prepare_mac_fidelity_from_saved_city();
    assert_eq!(
        std::array::from_fn(|mode| city.mac_transit_accumulator(mode)),
        raw_accumulators,
        "first Mac-fidelity open uses the native CODE 4 MISC slot interpretation"
    );

    city.refresh_mac_monthly_transit_microsims();

    assert_eq!(city.microsim_at(subway).unwrap().stat3, 4);
    assert_eq!(city.microsim_at(bus).unwrap().stat3, 2);
    assert_eq!(city.microsim_at(rail).unwrap().stat3, 3);
    for (mode, value) in raw_accumulators.into_iter().enumerate() {
        assert_eq!(
            city.misc_u32_at((1045 + mode) * 4),
            Some(value),
            "Mac XMIC consumption must not overwrite Win-layout MISC"
        );
        assert_eq!(city.mac_transit_accumulator(mode), 0);
    }
}

#[test]
fn mac_transit_accumulators_preserve_win_layout_misc() {
    let mut city = City::new_blank("/tmp/mac-transit-win.sc2", "Transit").unwrap();
    for (mode, sentinel) in [0x1111_2222, 0x3333_4444, 0x5555_6666]
        .into_iter()
        .enumerate()
    {
        city.set_misc_u32_at((1045 + mode) * 4, sentinel);
        city.set_mac_transit_accumulator(mode, 0x1234_0000 + mode as u32);
        assert_eq!(
            city.mac_transit_accumulator(mode),
            0x1234_0000 + mode as u32
        );
    }
    city.refresh_mac_monthly_transit_microsims();
    for (mode, sentinel) in [0x1111_2222, 0x3333_4444, 0x5555_6666]
        .into_iter()
        .enumerate()
    {
        assert_eq!(city.mac_transit_accumulator(mode), 0);
        assert_eq!(city.misc_u32_at((1045 + mode) * 4), Some(sentinel));
    }
}

#[test]
fn mac_monthly_museum_uses_saved_census_and_college_funding() {
    let mut city =
        City::new_blank("/tmp/sc2000-mac-museum-monthly.sc2", "MacMuseum").expect("blank city");
    city.kind = CityFileKind::MacCity;
    city.set_mac_state_misc_u32_at(MAC_XBLD_CENSUS_MISC_OFFSET + 0xd4 * 4, 30);
    city.set_mac_state_misc_u32_at(0x102c, 2_000);
    let overlay = city.provision_microsim_for_building(0xd4).unwrap();
    let index = usize::from(overlay - MIN_SIM_TEXT_ENTRIES);
    city.set_microsim_stat0(index, 0x5a);
    city.set_microsim_stat2(index, 0x1234);
    city.set_microsim_stat3(index, 0xabcd);

    city.refresh_mac_monthly_museum_microsims(80);

    let row = city.microsim_at(index).unwrap();
    assert_eq!(row.stat1, 100, "min(30 * (80*4), 2000/20)");
    assert_eq!(row.stat2, 240, "30 * trunc(80/10)");
    assert_eq!((row.stat0, row.stat3), (0x5a, 0xabcd));
}

#[test]
fn mac_monthly_museum_divides_full_signed_funding_dword_for_books() {
    let mut city = City::new_blank(
        "/tmp/sc2000-mac-museum-funding-width.sc2",
        "MacMuseumFundingWidth",
    )
    .expect("blank city");
    city.kind = CityFileKind::MacCity;
    city.set_mac_state_misc_u32_at(MAC_XBLD_CENSUS_MISC_OFFSET + 0xd4 * 4, 30);
    let overlay = city.provision_microsim_for_building(0xd4).unwrap();
    let index = usize::from(overlay - MIN_SIM_TEXT_ENTRIES);

    city.refresh_mac_monthly_museum_microsims(0x0001_0000);

    let row = city.microsim_at(index).unwrap();
    assert_eq!(
        row.stat1, 0,
        "the separate cap candidate uses funding's low word"
    );
    assert_eq!(
        row.stat2,
        30_u16.wrapping_mul((65_536_i32 / 10) as u16),
        "native DIVSL.L consumes the full funding dword before low-word storage"
    );
    assert_eq!(row.stat2, 0xffee);
}

#[test]
fn mac_museum_worker_matches_original_68020_replay_cases() {
    // Golden outputs from the original Mac museum worker's M68020 replay.
    // Keep them inline so this test needs no external research files.
    let cases = [
        (
            "ordinary",
            30_u16,
            100_u32,
            500_000_u32,
            12_000_u16,
            300_u16,
        ),
        ("signed capacity word", 83, 100, 500_000, 33_200, 830),
        ("zero census", 0, 100, 500_000, 0, 0),
        ("zero funding", 30, 0, 500_000, 0, 0),
        ("high funding word", 30, 65_536, 500_000, 0, 65_518),
        (
            "negative funding dword",
            17,
            0xC000_0000,
            500_000,
            0,
            13_114,
        ),
        ("census word wrap", 65_535, 100, 500_000, 65_136, 65_526),
        ("low growth cap", 30, 100, 20_000, 1_000, 300),
    ];

    for (name, census, funding, growth, expected_capacity, expected_books) in cases {
        let mut city =
            City::new_blank("/tmp/citygame-mac-museum-native.sc2", name).expect("blank city");
        city.kind = CityFileKind::MacCity;
        city.set_mac_state_misc_u32_at(MAC_XBLD_CENSUS_MISC_OFFSET + 0xd4 * 4, u32::from(census));
        city.set_mac_state_misc_u32_at(0x102c, growth);
        let overlay = city
            .provision_microsim_for_building(0xd4)
            .expect("museum XMIC row");
        let index = usize::from(overlay - MIN_SIM_TEXT_ENTRIES);
        city.set_microsim_stat0(index, 0xa1);
        city.set_microsim_stat2(index, 0x2345);
        city.set_microsim_stat3(index, 0xbcde);

        city.refresh_mac_monthly_museum_microsims(funding);

        let row = city.microsim_at(index).expect("museum row remains present");
        assert_eq!(row.stat0, 0xa1, "{name}: native leaves +1 untouched");
        assert_eq!(row.stat1, expected_capacity, "{name}: native +2 capacity");
        assert_eq!(row.stat2, expected_books, "{name}: native +4 books");
        assert_eq!(row.stat3, 0xbcde, "{name}: native leaves +6 untouched");
    }
}

#[test]
fn mac_monthly_park_uses_saved_park_census_and_prior_attendance() {
    let mut city =
        City::new_blank("/tmp/sc2000-mac-park-monthly.sc2", "MacPark").expect("blank city");
    city.kind = CityFileKind::MacCity;
    city.set_mac_state_misc_u32_at(MAC_XBLD_CENSUS_MISC_OFFSET + 13 * 4, 18);
    city.set_mac_state_misc_u32_at(MAC_XBLD_CENSUS_MISC_OFFSET + 0xd5 * 4, 27);
    city.set_mac_state_misc_u32_at(0x102c, 7_200);
    let overlay = city.provision_microsim_for_building(0xd5).unwrap();
    let index = usize::from(overlay - MIN_SIM_TEXT_ENTRIES);
    city.set_microsim_stat0(index, 0x5a);
    city.set_microsim_stat2(index, 20);

    city.refresh_mac_monthly_park_microsims();

    let row = city.microsim_at(index).unwrap();
    assert_eq!(row.stat1, 1_200, "min(20 * 412, 65000, 7200/6)");
    assert_eq!(row.stat2, 45, "small plus large park census");
    assert_eq!(row.stat3, 5, "min(45/9, 7200/120)");
    assert_eq!(row.stat0, 0x5a);
}

#[test]
fn provision_hydro_variant_199_uses_system_hydro_slot() {
    let mut city = City::new_blank("/tmp/sc2000-hydro199.sc2", "test").expect("blank city");
    let text = city
        .provision_microsim_for_building(199)
        .expect("tile 199 is class 21");
    assert_eq!(
        text,
        super::MIN_SIM_TEXT_ENTRIES + super::MICROSIM_SYSTEM_HYDRO as u8
    );
    assert_eq!(
        city.microsim_at(super::MICROSIM_SYSTEM_HYDRO)
            .map(|r| (r.tile_id, r.stat1, r.stat2)),
        Some((199, 1, 20))
    );
    assert_eq!(
        city.label_at(text),
        Some("Hydro Power"),
        "XLAB copy from default tile-name table"
    );
}

#[test]
fn provision_seeds_xlabel_once_for_system_slots() {
    let mut city = City::new_blank("/tmp/sc2000-xlabel.sc2", "test").expect("blank city");
    let bus = city.provision_microsim_for_building(236).unwrap();
    assert_eq!(city.label_at(bus), Some("Bus Depot"));
    city.set_label(bus, "Downtown Transit");
    city.provision_microsim_for_building(236).unwrap();
    assert_eq!(
        city.label_at(bus),
        Some("Downtown Transit"),
        "system class keeps a non-empty XLAB row"
    );
    let school = city.provision_microsim_for_building(214).unwrap();
    assert_eq!(city.label_at(school), Some("School"));
}

#[test]
fn arco_steals_first_non_arco_structure_slot_and_clears_xtxt() {
    let mut city = City::new_blank("/tmp/sc2000-steal.sc2", "test").expect("blank city");
    for _ in 0..3 {
        city.provision_microsim_for_building(251).unwrap();
    }
    let remaining = super::MICROSIM_COUNT - super::MICROSIM_STRUCTURE_FIRST - 3;
    for _ in 0..remaining {
        city.provision_microsim_for_building(210).unwrap();
    }
    let police_index = super::MICROSIM_STRUCTURE_FIRST + 3;
    let overlay = super::MIN_SIM_TEXT_ENTRIES + police_index as u8;
    city.set_text_overlay(4, 5, overlay);
    city.set_text_overlay(6, 7, overlay);

    let stolen = city
        .provision_microsim_for_building(252)
        .expect("llama/arco tile >= 0xfb steals");
    assert_eq!(stolen, overlay);
    assert_eq!(city.microsim_at(police_index).map(|r| r.tile_id), Some(252));
    assert_eq!(
        city.microsim_at(super::MICROSIM_STRUCTURE_FIRST)
            .map(|r| r.tile_id),
        Some(251)
    );
    assert_eq!(city.text_overlay_at(4, 5), Some(0));
    assert_eq!(city.text_overlay_at(6, 7), Some(0));
    assert_eq!(city.label_at(stolen), Some("Forest Arco"));
}

#[test]
fn provision_rejects_class_zero_and_unclassed_tiles() {
    let mut city = City::new_blank("/tmp/sc2000-class0.sc2", "test").expect("blank city");
    assert_eq!(city.provision_microsim_for_building(220), None); // water pump
    assert_eq!(city.provision_microsim_for_building(247), None); // church
    assert_eq!(city.provision_microsim_for_building(0), None);
    assert_eq!(super::pe_microsim_class(13), 0);
    assert_eq!(super::pe_microsim_class(198), 21);
    assert_eq!(super::pe_microsim_class(199), 21);
    assert_eq!(super::pe_microsim_class(236), 17);
}

#[test]
fn mac_aging_plant_poll_updates_later_mayor_rows_and_quickdraw() {
    // Mac 1.2 CODE 3 plant/Mayor/Statue arms joined through CODE 7 export 234.
    for kind in [CityFileKind::MacCity, CityFileKind::WinSc2] {
        for (age, extra, growth, land, approval, statue, seed) in [
            (47, 1, 1000, 40, 63, 39, 384_717_275),
            (48, 0, 1000, 40, 63, 39, 384_717_275),
            (48, 1, 1000, 40, 29, 26, 1_189_354_452),
            (49, 1, 1000, 40, 29, 26, 1_189_354_452),
            (48, 1, 99, 40, 63, 39, 384_717_275),
            (48, 1, 1000, 40_000, 88, 26, 1_189_354_452),
            (48, 1, 1000, 65_258, 63, 39, 384_717_275),
        ] {
            let mut city = City::new_blank("/tmp/mac-plant-poll.sc2", "Poll").unwrap();
            city.kind = kind;
            city.mac_mayor_approval = 63;
            city.set_mac_power_service_summary(0);
            for (offset, value) in [
                (0x48, 10),
                (0x4c, 25),
                (0x0fa4, 12),
                (0x1008, extra),
                (0x102c, growth),
            ] {
                city.set_mac_state_misc_u32_at(offset, value);
            }
            city.set_budget_misc_u32_values(0x0780, &[7]);
            for (index, score) in (4..).zip([10, 20, land, 30]) {
                city.set_chunk_u32_values("XGRP", index * 52 * 4, &[score]);
            }
            city.set_microsim_tile_id(9, 0xc9);
            city.set_microsim_stat0(9, age);
            for (slot, age, countdown) in [(8, 7, 0), (10, 255, 1), (60, 7, 0)] {
                city.set_microsim_tile_id(slot, 0xf3);
                city.set_microsim_stat0(slot, age);
                city.set_microsim_stat1(slot, 1955);
                city.set_microsim_stat2(slot, 21);
                city.set_microsim_stat3(slot, countdown);
            }
            city.set_microsim_tile_id(61, 0xdb);

            let mut rng = MacToolboxRng::new(123);
            city.refresh_mac_monthly_random_xmic_rows(100, 100, 100, 100, 100, false, &mut rng);

            let row = |slot| {
                let record = city.microsim_at(slot).unwrap();
                (record.stat0, record.stat1, record.stat2, record.stat3)
            };
            let case = (kind, age, extra, growth, land);
            assert_eq!(row(9).0, age + 1, "{case:?}");
            assert_eq!(row(9).2, 5, "{case:?}");
            assert_eq!(row(8), (7, 1955, 63, 0), "{case:?}");
            assert_eq!(row(10), (0, 1955, approval, 0), "{case:?}");
            assert_eq!(row(60), (7, 1955, approval, 0), "{case:?}");
            assert_eq!(row(61).2, statue, "{case:?}");
            assert_eq!(rng.seed(), seed, "{case:?}");
        }
    }
}

#[test]
fn query_mayor_countdown_age_increments_as_wrapping_pe_byte() {
    let mut city = City::new_blank("/tmp/sc2000-mayor-query-age-wrap.sc2", "MayorAgeWrap")
        .expect("blank city");
    let text = city.provision_microsim_for_building(243).expect("mayor");
    let index = usize::from(text - super::MIN_SIM_TEXT_ENTRIES);
    assert!(city.set_microsim_stat0(index, u8::MAX));
    assert!(city.set_microsim_stat3(index, 1));

    city.recalculate_mayors_house_stats(7, &mut super::Win95CrtRand::new(123));

    assert_eq!(
        city.microsim_at(index).map(|row| (row.stat0, row.stat3)),
        Some((0, 0))
    );
}

#[test]
fn recalculate_mayors_house_stats_ages_every_mayor_and_copies_approval() {
    let mut city = City::new_blank("/tmp/sc2000-mayor-query.sc2", "test").expect("blank city");
    city.set_saved_date(super::CityDate {
        year: 1955,
        month: 6,
    });
    let mayor = city.provision_microsim_for_building(243).expect("mayor");
    let mayor_index = usize::from(mayor - super::MIN_SIM_TEXT_ENTRIES);
    let second = city
        .provision_microsim_for_building(243)
        .expect("second mayor");
    let second_index = usize::from(second - super::MIN_SIM_TEXT_ENTRIES);
    assert_eq!(
        city.microsim_at(mayor_index)
            .map(|r| (r.stat0, r.stat2, r.stat3)),
        Some((0, 21, 47))
    );

    // Leftover `0x4caa74` is still 0: poll skips, Approval stays 0, countdown still ages.
    let mut rng = super::Win95CrtRand::new(123);
    city.recalculate_mayors_house_stats(7, &mut rng);
    assert_eq!(city.pe_mayor_approval(), 0);
    assert_eq!(
        city.microsim_at(mayor_index)
            .map(|r| (r.stat0, r.stat2, r.stat3)),
        Some((1, 0, 46))
    );
    assert_eq!(
        city.microsim_at(second_index)
            .map(|r| (r.stat0, r.stat2, r.stat3)),
        Some((1, 0, 46)),
        "PE walker updates every tile_id 0xF3 row, not only overlay 111"
    );

    city.persist_pe_day22_population_leftover(10_000);
    city.recalculate_mayors_house_stats(7, &mut rng);
    let approval = city.pe_mayor_approval();
    assert!(approval > 0, "leftover>=100 should sample Approval");
    assert_eq!(
        city.microsim_at(mayor_index)
            .map(|r| (r.stat0, r.stat2, r.stat3)),
        Some((2, approval, 45))
    );
    assert_eq!(
        city.microsim_at(second_index)
            .map(|r| (r.stat0, r.stat2, r.stat3)),
        Some((2, approval, 45))
    );
}

#[test]
fn yearly_power_plant_stat_matches_original_word_add_replay() {
    let csv = include_str!("../../../tests/fixtures/simulation/win95/pe_plant_power_word.csv");
    for (line, row) in csv.lines().enumerate().skip(1) {
        let v: Vec<u32> = row.split(',').map(|x| x.parse().unwrap()).collect();
        assert_eq!(v.len(), 4, "line {line}");
        let mut city = City::new_blank("/tmp/pe-plant-word.sc2", "Plant").unwrap();
        city.set_pe_power_used_percent(v[0]);
        let index = usize::from(
            city.provision_microsim_for_building(201).unwrap() - super::MIN_SIM_TEXT_ENTRIES,
        );
        let mut crt = super::Win95CrtRand::new(v[1]);
        let mut lfsr = super::Win95Lfsr::from_u32_seed(1);
        city.refresh_structure_microsim_yearly_with_rng(
            0, 100, 100, 100, 100, 100, &mut crt, &mut lfsr,
        );
        assert_eq!(
            city.microsim_at(index).unwrap().stat2,
            v[3] as u16,
            "line {line}, power {} seed {}",
            v[0],
            v[1]
        );
    }
}

#[test]
fn yearly_llama_record_matches_original_x86_replay() {
    let csv = include_str!("../../../tests/fixtures/simulation/win95/pe_llama_yearly.csv");
    for (line, row) in csv.lines().enumerate().skip(1) {
        let v: Vec<u32> = row.split(',').map(|x| x.parse().unwrap()).collect();
        assert_eq!(v.len(), 7, "line {line}");
        let mut city = City::new_blank("/tmp/pe-llama-round.sc2", "Llama").unwrap();
        let index = usize::from(
            city.provision_microsim_for_building(255).unwrap() - super::MIN_SIM_TEXT_ENTRIES,
        );
        let mut crt = super::Win95CrtRand::new(v[1]);
        let mut lfsr = super::Win95Lfsr::from_u32_seed(1);
        city.refresh_structure_microsim_yearly_with_rng(
            v[0], 100, 100, 100, 100, 100, &mut crt, &mut lfsr,
        );
        let record = city.microsim_at(index).unwrap();
        assert_eq!(
            [
                u32::from(record.stat0),
                u32::from(record.stat1),
                u32::from(record.stat2),
                u32::from(record.stat3)
            ],
            [v[2], v[3], v[4], v[5]],
            "line {line}"
        );
        assert_eq!(crt.seed(), v[6], "CRT line {line}");
    }
}

#[test]
fn yearly_stadium_attendance_wrap_matches_original_x86_replay() {
    let csv = include_str!("../../../tests/fixtures/simulation/win95/pe_stadium_word.csv");
    for (line, row) in csv.lines().enumerate().skip(1) {
        let v: Vec<u32> = row.split(',').map(|x| x.parse().unwrap()).collect();
        assert_eq!(v.len(), 4, "line {line}");
        let mut city = City::new_blank("/tmp/pe-stadium-wrap.sc2", "Stadium").unwrap();
        let slot = usize::from(
            city.provision_microsim_for_building(215).unwrap() - super::MIN_SIM_TEXT_ENTRIES,
        );
        let mut crt = super::Win95CrtRand::new(v[0]);
        let mut lfsr = super::Win95Lfsr::from_u32_seed(1);
        // Signed PE threshold skips the conditional cap draw; the capacity
        // helper returns WORD 0xffff, exercising the following WORD add.
        city.refresh_structure_microsim_yearly_with_rng(
            0x8000_ffff,
            100,
            100,
            100,
            100,
            100,
            &mut crt,
            &mut lfsr,
        );
        let record = city.microsim_at(slot).unwrap();
        assert_eq!(
            (u32::from(record.stat0), u32::from(record.stat1)),
            (v[1], v[2]),
            "line {line}"
        );
        assert_eq!(crt.seed(), v[3], "CRT line {line}");
    }
}

#[test]
fn mac_monthly_llama_ordered_dispatch_matches_original_68020_replay() {
    let csv = include_str!("../../../tests/fixtures/simulation/mac/mac_llama.csv");
    for (line, row) in csv.lines().enumerate().skip(1) {
        let v: Vec<u32> = row.split(',').map(|x| x.parse().unwrap()).collect();
        assert_eq!(v.len(), 7, "line {line}");
        let mut city = City::new_blank("/tmp/mac-llama-replay.city", "Llama").unwrap();
        city.kind = CityFileKind::MacCity;
        city.set_mac_state_misc_u32_at(0x102c, v[0]);
        let slot =
            usize::from(city.provision_microsim_for_building(255).unwrap() - MIN_SIM_TEXT_ENTRIES);
        let mut rng = MacToolboxRng::new(v[1]);
        city.refresh_mac_monthly_random_xmic_rows(100, 100, 100, 100, 100, false, &mut rng);
        let record = city.microsim_at(slot).unwrap();
        assert_eq!(
            [
                u32::from(record.stat0),
                u32::from(record.stat1),
                u32::from(record.stat2),
                u32::from(record.stat3)
            ],
            [v[2], v[3], v[4], v[5]],
            "line {line}"
        );
        assert_eq!(rng.seed(), v[6], "RNG line {line}");
    }
}

#[test]
fn mac_monthly_park_matches_original_68020_replay() {
    let csv = include_str!("../../../tests/fixtures/simulation/mac/mac_park.csv");
    for (line, row) in csv.lines().enumerate().skip(1) {
        let v: Vec<u32> = row.split(',').map(|x| x.parse().unwrap()).collect();
        assert_eq!(v.len(), 8, "line {line}");
        let mut city = City::new_blank("/tmp/mac-park-replay.city", "Park").unwrap();
        city.kind = CityFileKind::MacCity;
        city.set_mac_state_misc_u32_at(0x102c, v[0]);
        city.set_mac_state_misc_u32_at(MAC_XBLD_CENSUS_MISC_OFFSET + 13 * 4, v[2]);
        city.set_mac_state_misc_u32_at(MAC_XBLD_CENSUS_MISC_OFFSET + 0xd5 * 4, v[3]);
        let slot =
            usize::from(city.provision_microsim_for_building(0xd5).unwrap() - MIN_SIM_TEXT_ENTRIES);
        city.set_microsim_stat0(slot, 0x5a);
        city.set_microsim_stat2(slot, v[1] as u16);
        city.refresh_mac_monthly_park_microsims();
        let record = city.microsim_at(slot).unwrap();
        assert_eq!(
            [
                u32::from(record.stat0),
                u32::from(record.stat1),
                u32::from(record.stat2),
                u32::from(record.stat3)
            ],
            [v[4], v[5], v[6], v[7]],
            "line {line}"
        );
    }
}

#[test]
fn mac_water_xmic_ordered_dispatch_matches_original_68020_replay() {
    let csv = include_str!("../../../tests/fixtures/simulation/mac/mac_water_xmic.csv");
    for (line, row) in csv.lines().enumerate().skip(1) {
        let v: Vec<u32> = row.split(',').map(|x| x.parse().unwrap()).collect();
        assert_eq!(v.len(), 8, "line {line}");
        for tile in [0xf4, 0xfa] {
            let mut city = City::new_blank("/tmp/mac-water-replay.city", "Water").unwrap();
            city.kind = CityFileKind::MacCity;
            city.set_mac_state_misc_u32_at(0x102c, v[0]);
            city.set_mac_water_service_summary(v[1]);
            let slot = usize::from(
                city.provision_microsim_for_building(tile).unwrap() - MIN_SIM_TEXT_ENTRIES,
            );
            city.set_microsim_stat3(slot, 0x1234);
            let mut rng = MacToolboxRng::new(v[2]);
            city.refresh_mac_monthly_random_xmic_rows(100, 100, 100, 100, 100, false, &mut rng);
            let record = city.microsim_at(slot).unwrap();
            assert_eq!(
                [
                    u32::from(record.stat0),
                    u32::from(record.stat1),
                    u32::from(record.stat2),
                    u32::from(record.stat3)
                ],
                [v[3], v[4], v[5], v[6]],
                "line {line}, tile {tile:#x}"
            );
            assert_eq!(rng.seed(), v[7], "RNG line {line}, tile {tile:#x}");
        }
    }
}

#[test]
fn mac_hospital_dispatch_matches_original_68020_replay() {
    let csv = include_str!("../../../tests/fixtures/simulation/mac/mac_hospital.csv");
    for (line, row) in csv.lines().enumerate().skip(1) {
        let v: Vec<u32> = row.split(',').map(|x| x.parse().unwrap()).collect();
        assert_eq!(v.len(), 9, "line {line}");
        let mut city = City::new_blank("/tmp/mac-hospital-replay.city", "Hospital").unwrap();
        city.kind = CityFileKind::MacCity;
        city.set_mac_state_misc_u32_at(0x102c, v[0]);
        city.set_mac_state_misc_u32_at(MAC_XBLD_CENSUS_MISC_OFFSET + 0xd1 * 4, v[1]);
        let slot =
            usize::from(city.provision_microsim_for_building(0xd1).unwrap() - MIN_SIM_TEXT_ENTRIES);
        city.set_microsim_stat0(slot, 7);
        let mut ordered = city.clone();
        let mut focused_rng = MacToolboxRng::new(v[3]);
        let mut ordered_rng = focused_rng;
        city.refresh_mac_monthly_hospital_microsims(v[2], &mut focused_rng);
        ordered.refresh_mac_monthly_random_xmic_rows(
            100,
            100,
            v[2],
            100,
            100,
            false,
            &mut ordered_rng,
        );
        for (name, current, rng) in [
            ("focused", &city, focused_rng),
            ("ordered", &ordered, ordered_rng),
        ] {
            let record = current.microsim_at(slot).unwrap();
            assert_eq!(
                [
                    u32::from(record.stat0),
                    u32::from(record.stat1),
                    u32::from(record.stat2),
                    u32::from(record.stat3)
                ],
                [v[4], v[5], v[6], v[7]],
                "{name} line {line}"
            );
            assert_eq!(rng.seed(), v[8], "{name} RNG line {line}");
        }
    }
}

#[test]
fn mac_stadium_dispatch_matches_original_68020_replay() {
    let csv = include_str!("../../../tests/fixtures/simulation/mac/mac_stadium.csv");
    for (line, row) in csv.lines().enumerate().skip(1) {
        let v: Vec<u32> = row.split(',').map(|x| x.parse().unwrap()).collect();
        assert_eq!(v.len(), 8, "line {line}");
        let mut city = City::new_blank("/tmp/mac-stadium-replay.city", "Stadium").unwrap();
        city.kind = CityFileKind::MacCity;
        city.set_mac_state_misc_u32_at(0x102c, v[0]);
        city.set_mac_state_misc_u32_at(MAC_XBLD_CENSUS_MISC_OFFSET + 0xd7 * 4, v[1]);
        let slot =
            usize::from(city.provision_microsim_for_building(0xd7).unwrap() - MIN_SIM_TEXT_ENTRIES);
        city.set_microsim_stat2(slot, 0x5678);
        city.set_microsim_stat3(slot, 0x1234);
        let mut ordered = city.clone();
        let mut a = MacToolboxRng::new(v[2]);
        let mut b = a;
        city.refresh_mac_monthly_stadium_microsims(&mut a);
        ordered.refresh_mac_monthly_random_xmic_rows(100, 100, 100, 100, 100, false, &mut b);
        for (label, current, rng) in [("focused", &city, a), ("ordered", &ordered, b)] {
            let record = current.microsim_at(slot).unwrap();
            assert_eq!(
                [
                    u32::from(record.stat0),
                    u32::from(record.stat1),
                    u32::from(record.stat2),
                    u32::from(record.stat3)
                ],
                [v[3], v[4], v[5], v[6]],
                "{label} line {line}"
            );
            assert_eq!(rng.seed(), v[7], "{label} RNG line {line}");
        }
    }
}

#[test]
fn mac_school_dispatch_matches_original_68020_replay() {
    let csv = include_str!("../../../tests/fixtures/simulation/mac/mac_school.csv");
    for (line, row) in csv.lines().enumerate().skip(1) {
        let v: Vec<u32> = row.split(',').map(|x| x.parse().unwrap()).collect();
        assert_eq!(v.len(), 9, "line {line}");
        let mut city = City::new_blank("/tmp/mac-school-replay.city", "School").unwrap();
        city.kind = CityFileKind::MacCity;
        city.set_mac_state_misc_u32_at(0x102c, v[0]);
        city.set_mac_state_misc_u32_at(0x0088, v[0]);
        city.set_mac_state_misc_u32_at(0x0094, 777);
        city.set_mac_state_misc_u32_at(MAC_XBLD_CENSUS_MISC_OFFSET + 0xd6 * 4, v[1]);
        let slot =
            usize::from(city.provision_microsim_for_building(0xd6).unwrap() - MIN_SIM_TEXT_ENTRIES);
        city.set_microsim_stat0(slot, 7);
        city.set_microsim_stat2(slot, 0x5678);
        city.set_microsim_stat3(slot, 0x1234);
        let mut ordered = city.clone();
        let mut a = MacToolboxRng::new(v[3]);
        let mut b = a;
        city.refresh_mac_monthly_school_microsims(v[2], &mut a);
        ordered.refresh_mac_monthly_random_xmic_rows(100, 100, 100, v[2], 100, false, &mut b);
        for (label, current, rng) in [("focused", &city, a), ("ordered", &ordered, b)] {
            let record = current.microsim_at(slot).unwrap();
            assert_eq!(
                [
                    u32::from(record.stat0),
                    u32::from(record.stat1),
                    u32::from(record.stat2),
                    u32::from(record.stat3)
                ],
                [v[4], v[5], v[6], v[7]],
                "{label} line {line}"
            );
            assert_eq!(rng.seed(), v[8], "{label} RNG line {line}");
        }
    }
}

#[test]
fn mac_college_dispatch_matches_original_68020_replay() {
    let csv = include_str!("../../../tests/fixtures/simulation/mac/mac_college.csv");
    for (line, row) in csv.lines().enumerate().skip(1) {
        let v: Vec<u32> = row.split(',').map(|x| x.parse().unwrap()).collect();
        assert_eq!(v.len(), 9, "line {line}");
        let mut city = City::new_blank("/tmp/mac-college-replay.city", "College").unwrap();
        city.kind = CityFileKind::MacCity;
        city.set_mac_state_misc_u32_at(0x102c, v[0]);
        city.set_mac_state_misc_u32_at(0x00a0, v[0].wrapping_add(777));
        city.set_mac_state_misc_u32_at(MAC_XBLD_CENSUS_MISC_OFFSET + 0xd9 * 4, v[1]);
        let slot =
            usize::from(city.provision_microsim_for_building(0xd9).unwrap() - MIN_SIM_TEXT_ENTRIES);
        city.set_microsim_stat0(slot, 7);
        city.set_microsim_stat2(slot, 0x5678);
        city.set_microsim_stat3(slot, 0x1234);
        let mut ordered = city.clone();
        let mut a = MacToolboxRng::new(v[3]);
        let mut b = a;
        city.refresh_mac_monthly_college_microsims(v[2], &mut a);
        ordered.refresh_mac_monthly_random_xmic_rows(100, 100, 100, 100, v[2], false, &mut b);
        for (label, current, rng) in [("focused", &city, a), ("ordered", &ordered, b)] {
            let record = current.microsim_at(slot).unwrap();
            assert_eq!(
                [
                    u32::from(record.stat0),
                    u32::from(record.stat1),
                    u32::from(record.stat2),
                    u32::from(record.stat3)
                ],
                [v[4], v[5], v[6], v[7]],
                "{label} line {line}"
            );
            assert_eq!(rng.seed(), v[8], "{label} RNG line {line}");
        }
    }
}

#[test]
fn mac_arcology_selector_matches_original_68020_replay() {
    let csv = include_str!("../../../tests/fixtures/simulation/mac/mac_arcology.csv");
    for (line, row) in csv.lines().enumerate().skip(1) {
        let v: Vec<u32> = row.split(',').map(|s| s.parse().unwrap()).collect();
        assert_eq!(v.len(), 13);
        let mut city = City::new_blank("/tmp/mac-arcology-replay.city", "Arcology").unwrap();
        city.kind = CityFileKind::MacCity;
        for (i, count) in v[..4].iter().enumerate() {
            city.set_mac_state_misc_u32_at(MAC_XBLD_CENSUS_MISC_OFFSET + (0xfb + i) * 4, *count);
        }
        city.set_mac_state_misc_u32_at(0x102c, v[4]);
        let slot =
            usize::from(city.provision_microsim_for_building(0xfb).unwrap() - MIN_SIM_TEXT_ENTRIES);
        city.set_microsim_stat1(slot, v[8] as u16);
        city.set_microsim_stat2(slot, v[9] as u16);
        city.set_microsim_stat0(slot, v[10] as u8);
        city.refresh_mac_monthly_arcology_microsims([v[5], v[6], v[7]], v[11]);
        assert_eq!(
            u32::from(city.microsim_at(slot).unwrap().stat2),
            v[12],
            "line {line}"
        );
    }
}

#[test]
fn mac_facility_rows_read_census_at_their_native_slot() {
    for tile in [0xd4_u8, 0xd5, 0xf5, 0xf8] {
        let mut city = City::new_blank("/tmp/mac-facility-slot-census.sc2", "Facility").unwrap();
        city.kind = CityFileKind::MacCity;
        city.set_mac_state_misc_u32_at(0x102c, 18_000);
        // Explicit record slots exercise the binary's XMIC walk even when
        // the normal allocator assigns this facility a fixed system slot.
        let (before_index, plant_index, after_index) = (20_usize, 21_usize, 22_usize);
        city.set_microsim_tile_id(before_index, tile);
        city.set_microsim_tile_id(plant_index, 0xc9);
        city.set_microsim_tile_id(after_index, tile);
        let plant = MIN_SIM_TEXT_ENTRIES + plant_index as u8;
        for dx in 0..4 {
            for dy in 0..4 {
                city.set_building(20 + dx, 20 - dy, 0xc9);
                city.set_terrain(20 + dx, 20 - dy, 0);
            }
        }
        city.set_building(21, 20, tile);
        city.set_mac_state_misc_u32_at(MAC_XBLD_CENSUS_MISC_OFFSET + usize::from(tile) * 4, 2);
        city.set_microsim_stat0(plant_index, 50);
        city.set_text_overlay(20, 20, plant);
        let mut rng = MacToolboxRng::new(1);
        city.refresh_mac_monthly_random_xmic_rows(100, 100, 100, 100, 100, false, &mut rng);
        let before = city.microsim_at(before_index).unwrap();
        let after = city.microsim_at(after_index).unwrap();
        let (actual_before, actual_after, expected_before, expected_after) = match tile {
            0xd5 => (before.stat2, after.stat2, 2, 1),
            0xf8 => (before.stat1, after.stat1, 16, 8),
            _ => (before.stat1, after.stat1, 800, 400),
        };
        assert_eq!(actual_before, expected_before, "{tile:#x} before expiry");
        assert_eq!(actual_after, expected_after, "{tile:#x} after expiry");
        assert_eq!(city.mac_xbld_tile_count(tile), 1);
    }
}

#[test]
fn mac_transit_and_utility_rows_use_slot_census_and_clear_passengers_after_walk() {
    for tile in [0xc6_u8, 0xc8, 0xd2, 0xd7, 0xe9, 0xec, 0xed] {
        let mut city = City::new_blank("/tmp/mac-transit-slot.sc2", "Transit").unwrap();
        city.kind = CityFileKind::MacCity;
        city.set_mac_state_misc_u32_at(0x002c, 560);
        city.set_mac_state_misc_u32_at(0x102c, 18_000);
        city.set_mac_state_misc_u32_at(0x1008, 1);
        let (before, plant, after) = (20_usize, 21_usize, 22_usize);
        city.set_microsim_tile_id(before, tile);
        city.set_microsim_tile_id(plant, 0xc9);
        city.set_microsim_tile_id(after, tile);
        for dx in 0..4 {
            for dy in 0..4 {
                city.set_building(20 + dx, 20 - dy, 0xc9);
                city.set_terrain(20 + dx, 20 - dy, 0);
            }
        }
        city.set_building(21, 20, tile);
        city.set_mac_state_misc_u32_at(MAC_XBLD_CENSUS_MISC_OFFSET + usize::from(tile) * 4, 8);
        city.set_mac_state_misc_u32_at(1045 * 4, 0x1234_1111);
        city.set_mac_state_misc_u32_at(1046 * 4, 0x1234_2222);
        city.set_mac_state_misc_u32_at(1047 * 4, 0x1234_3333);
        city.set_mac_state_misc_u32_at(97 * 4, 0xabcd_1234);
        city.set_microsim_stat0(plant, 50);
        city.set_text_overlay(20, 20, MIN_SIM_TEXT_ENTRIES + plant as u8);
        let mut rng = MacToolboxRng::new(1);
        city.refresh_mac_monthly_random_xmic_rows(100, 100, 100, 100, 100, false, &mut rng);
        let a = city.microsim_at(before).unwrap();
        let b = city.microsim_at(after).unwrap();
        match tile {
            0xd2 => assert_eq!((a.stat2, b.stat2), (70, 80)),
            0xd7 => {
                let mut expected = MacToolboxRng::new(1);
                let first_jitter = expected.random_u16() & 0xff;
                expected.random_u16(); // first stadium grade
                expected.random_u16(); // plant age jitter
                for _ in 0..(100 + 128 + 16) {
                    expected.random_u16();
                }
                let second_jitter = expected.random_u16() & 0xff;
                assert_eq!(
                    (a.stat1, b.stat1),
                    (2250 + first_jitter, 2571 + second_jitter)
                );
            }
            0xec => assert_eq!((a.stat2, b.stat2, a.stat3, b.stat3), (8, 7, 0x1111, 0x1111)),
            0xed => assert_eq!((a.stat1, b.stat1, a.stat3, b.stat3), (2, 1, 0x2222, 0x2222)),
            0xe9 => assert_eq!((a.stat1, b.stat1, a.stat3, b.stat3), (8, 7, 0x3333, 0x3333)),
            _ => assert_eq!((a.stat1, b.stat1), (8, 7)),
        }
        for slot in [1045, 1046, 1047] {
            assert_eq!(city.mac_state_misc_u32_at(slot * 4), Some(0));
        }
        assert_eq!(city.mac_state_misc_u32_at(97 * 4), Some(0xabcd_1234));
    }
}

#[test]
fn mac_monthly_xmic_walk_skips_reserved_slot_zero() {
    let mut city = City::new_blank("/tmp/mac-xmic-reserved-zero.sc2", "SlotZero").unwrap();
    city.kind = CityFileKind::MacCity;
    city.set_microsim_tile_id(0, 0xc9);
    city.set_microsim_stat0(0, 50);
    let mut rng = MacToolboxRng::new(1);
    city.refresh_mac_monthly_random_xmic_rows(100, 100, 100, 100, 100, false, &mut rng);
    assert_eq!(city.microsim_at(0).unwrap().stat0, 50);
    assert_eq!(rng.seed(), 1, "reserved record cannot consume QuickDraw");
    city.set_microsim_tile_id(0, 0xfb);
    city.set_microsim_stat2(0, 999);
    city.republish_mac_arcology_resident_total();
    assert_eq!(city.mac_arcology_resident_total(), 0);
    assert!(!city.has_mac_monthly_arcology_microsims());
}

#[test]
fn mac_police_selector_matches_original_68020_replay() {
    let csv = include_str!("../../../tests/fixtures/simulation/mac/mac_police.csv");
    for (line, row) in csv.lines().enumerate().skip(1) {
        let v: Vec<u32> = row.split(',').map(|s| s.parse().unwrap()).collect();
        assert_eq!(v.len(), 10);
        let mut city = City::new_blank("/tmp/mac-police-replay.city", "Police").unwrap();
        city.kind = CityFileKind::MacCity;
        city.set_mac_state_misc_u32_at(0x002c, v[0]);
        city.set_mac_state_misc_u32_at(0x102c, 50_000);
        city.set_mac_state_misc_u32_at(MAC_XBLD_CENSUS_MISC_OFFSET + 0xd2 * 4, v[1]);
        city.set_mac_police_coverage_gate(v[2] as u16 as i16);
        let slot =
            usize::from(city.provision_microsim_for_building(0xd2).unwrap() - MIN_SIM_TEXT_ENTRIES);
        let mut ordered = city.clone();
        let mut a = MacToolboxRng::new(v[3]);
        let mut b = a;
        city.refresh_mac_monthly_police_fire_microsims(100, 100, &mut a);
        ordered.refresh_mac_monthly_random_xmic_rows(100, 100, 100, 100, 100, false, &mut b);
        for (name, current, rng) in [("focused", &city, a), ("ordered", &ordered, b)] {
            let record = current.microsim_at(slot).unwrap();
            assert_eq!(
                [
                    u32::from(record.stat0),
                    u32::from(record.stat1),
                    u32::from(record.stat2),
                    u32::from(record.stat3)
                ],
                [v[4], v[5], v[6], v[7]],
                "{name} line {line}"
            );
            assert_eq!(rng.seed(), v[9], "{name} RNG line {line}");
        }
        assert_eq!(
            u32::from(city.mac_monthly_arrest_total()),
            v[8],
            "subtotal line {line}"
        );
    }
}

#[test]
fn mac_ordered_xmic_uses_incoming_arrest_carry_then_publishes_current_carry() {
    let mut city =
        City::new_blank("/tmp/mac-arrest-carry-order.city", "ArrestCarry").expect("blank Mac city");
    city.kind = CityFileKind::MacCity;
    city.set_mac_state_misc_u32_at(0x002c, 1_000);
    city.set_mac_state_misc_u32_at(0x102c, 500_000);
    city.set_mac_state_misc_u32_at(MAC_XBLD_CENSUS_MISC_OFFSET + 0xd2 * 4, 1);
    city.set_mac_state_misc_u32_at(MAC_XBLD_CENSUS_MISC_OFFSET + 0xd8 * 4, 16);
    let police_slot =
        usize::from(city.provision_microsim_for_building(0xd2).unwrap() - MIN_SIM_TEXT_ENTRIES);
    let prison_slot =
        usize::from(city.provision_microsim_for_building(0xd8).unwrap() - MIN_SIM_TEXT_ENTRIES);
    assert!(police_slot < prison_slot, "fixture places D2 before D8");

    let mut rng = MacToolboxRng::new(0x2468);
    city.refresh_mac_monthly_random_xmic_rows(100, 100, 100, 100, 100, false, &mut rng);
    let first_carry = city.mac_monthly_arrest_total();
    assert!(first_carry > 0);
    let prison = city.microsim_at(prison_slot).unwrap();
    assert_eq!(
        prison.stat1, 0,
        "D8 reads incoming fffffe04, initially zero"
    );
    assert_eq!(prison.stat3, 0);
    assert_eq!(
        city.mac_state_misc_u32_at(1038 * 4),
        Some(u32::from(first_carry)),
        "the completed D2 subtotal becomes the next saved carry"
    );

    city.refresh_mac_monthly_random_xmic_rows(100, 100, 100, 100, 100, false, &mut rng);
    let prison = city.microsim_at(prison_slot).unwrap();
    assert_eq!(
        prison.stat1, first_carry,
        "the next annual D8 row consumes the prior pass's published carry"
    );
    assert_eq!(prison.stat3, first_carry / 100);
}

#[test]
fn mac_d8_saved_carry_explains_phase0_native_one_unit_delta() {
    // The phase-0 native checkpoint starts fffffe04 at zero and emits D8
    // stat3=15. Its saved-context CGMS input has MISC slot 1038 = 0x008b
    // (139), so restoring the actual saved carry raises the raw candidate from
    // 1548 to 1687 and the per-hundred field from 15 to 16.
    let mut city = City::new_blank("/tmp/mac-prison-carry-alignment.city", "Prison")
        .expect("blank Mac city");
    city.kind = CityFileKind::MacCity;
    city.set_mac_state_misc_u32_at(0x102c, 0x22ba); // 8,890: native cap inputs.
    city.set_mac_state_misc_u32_at(1038 * 4, 0x008b);
    city.set_mac_state_misc_u32_at(MAC_XBLD_CENSUS_MISC_OFFSET + 0xd8 * 4, 4);
    city.mac_monthly_arrest_total = 0x008b;

    let slot = usize::from(
        city.provision_microsim_for_building(0xd8).unwrap() - MIN_SIM_TEXT_ENTRIES,
    );
    city.set_microsim_stat0(slot, 0x23);
    city.set_microsim_stat1(slot, 2_064);
    city.set_microsim_stat2(slot, 0);
    city.set_microsim_stat3(slot, 5);

    let mut rng = MacToolboxRng::new(1_441_282_327);
    city.refresh_mac_monthly_random_xmic_rows(25, 0, 0, 0, 0, false, &mut rng);

    let prison = city.microsim_at(slot).unwrap();
    assert_eq!(prison.stat0, 0);
    assert_eq!([prison.stat1, prison.stat2, prison.stat3], [444, 74, 16]);
    assert_eq!(rng.seed(), 1_441_282_327, "this row stays below the random overflow gate");
}

#[test]
fn mac_prison_selector_matches_original_68020_replay() {
    let csv = include_str!("../../../tests/fixtures/simulation/mac/mac_prison.csv");
    for (line, row) in csv.lines().enumerate().skip(1) {
        let v: Vec<u32> = row.split(',').map(|s| s.parse().unwrap()).collect();
        assert_eq!(v.len(), 10);
        let mut city = City::new_blank("/tmp/mac-prison-replay.city", "Prison").unwrap();
        city.kind = CityFileKind::MacCity;
        city.set_mac_state_misc_u32_at(0x102c, 500_000);
        city.set_mac_state_misc_u32_at(MAC_XBLD_CENSUS_MISC_OFFSET + 0xd8 * 4, v[2]);
        city.mac_monthly_arrest_total = v[1] as u16;
        let slot =
            usize::from(city.provision_microsim_for_building(0xd8).unwrap() - MIN_SIM_TEXT_ENTRIES);
        city.set_microsim_stat1(slot, v[0] as u16);
        let mut rng = MacToolboxRng::new(v[4]);
        city.refresh_mac_monthly_prison_microsims(v[3], &mut rng);
        let r = city.microsim_at(slot).unwrap();
        assert_eq!(
            [
                u32::from(r.stat0),
                u32::from(r.stat1),
                u32::from(r.stat2),
                u32::from(r.stat3)
            ],
            [v[5], v[6], v[7], v[8]],
            "line {line}"
        );
        assert_eq!(rng.seed(), v[9], "RNG line {line}");
    }
}

#[test]
fn mac_xmic_marker_locator_matches_original_68020_replay() {
    let csv = include_str!("../../../tests/fixtures/simulation/mac/mac_xmic_locator.csv");
    for row in csv.lines().skip(1) {
        let v: Vec<_> = row.split(',').collect();
        assert_eq!(v.len(), 4);
        let slot = v[1].parse::<usize>().unwrap();
        let mut city = City::new_blank("/tmp/mac-xmic-marker-replay.city", "Marker").unwrap();
        if !v[2].is_empty() {
            for cell in v[2].split('|') {
                let c: Vec<usize> = cell.split(':').map(|s| s.parse().unwrap()).collect();
                city.set_text_overlay(c[0], c[1], c[2] as u8);
            }
        }
        let expected = v[3].parse::<u16>().unwrap();
        let result = city
            .mac_find_xmic_marker(slot)
            .map(|(x, y)| ((x as u16) << 8) | y as u16)
            .unwrap_or(0);
        assert_eq!(result, expected, "{}", v[0]);
    }
}

#[test]
fn mac_aged_plant_origin_marker_blocks_later_duplicate() {
    let mut city = City::new_blank("/tmp/mac-plant-origin-duplicate.city", "Marker").unwrap();
    city.kind = CityFileKind::MacCity;
    city.set_city_funds(10_000);
    let marker = city.provision_microsim_for_building(0xc9).unwrap();
    let slot = usize::from(marker - MIN_SIM_TEXT_ENTRIES);
    city.set_microsim_stat0(slot, 50);
    city.set_text_overlay(0, 0, marker);
    city.set_text_overlay(10, 10, marker);
    city.refresh_mac_monthly_random_xmic_rows(
        100,
        100,
        100,
        100,
        100,
        true,
        &mut MacToolboxRng::new(1),
    );
    assert_eq!(city.microsim_at(slot).unwrap().stat0, 51);
    assert_eq!(city.city_funds(), 10_000);
}
