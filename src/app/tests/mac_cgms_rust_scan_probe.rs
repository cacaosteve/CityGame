use crate::city::{City, CityDate, cgms_v4_from_city_for_test, city_from_cgms_v4_for_test};
use crate::game_clock::GameClock;
use crate::mac_random::{MacGameLcgRng, MacToolboxRng};

fn encode_probe_xmic(city: &City) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(150 * 8);
    for index in 0..150 {
        let record = city.microsim_at(index).unwrap_or_default();
        bytes.push(record.tile_id);
        bytes.push(record.stat0);
        bytes.extend_from_slice(&record.stat1.to_be_bytes());
        bytes.extend_from_slice(&record.stat2.to_be_bytes());
        bytes.extend_from_slice(&record.stat3.to_be_bytes());
    }
    bytes
}

/// One-off differential probe for a hash-pinned native Mac scan. Set
/// SC2K_MAC_CGMS_INPUT to a CGMS v4 snapshot or authentic Mac city save, and
/// SC2K_MAC_CGMS_OUTPUT to produce a Rust post-scan snapshot plus partition/RNG
/// trace. Optionally set SC2K_MAC_CGMS_BASELINE_OUTPUT to export a loaded save
/// as CGMS v4 for the native harness, and SC2K_MAC_CGMS_PARTITION_DIR for
/// per-residue snapshots. Set SC2K_MAC_CGMS_CALENDAR_PREFIX_DIR to also emit
/// calendar days 0–2 before running the scan.
/// SC2K_MAC_CGMS_QUICKDRAW_SEED for the Toolbox Random seed (default 1), and
/// SC2K_MAC_CGMS_GAME_LCG_SEED for the app LCG state (default 1103527590).
/// No proprietary fixture is checked in.
#[test]
fn mac_native_leaf_cgms_differential_probe() {
    let (Ok(input), Ok(output)) = (
        std::env::var("SC2K_MAC_CGMS_INPUT"),
        std::env::var("SC2K_MAC_CGMS_OUTPUT"),
    ) else {
        return;
    };
    let bytes = std::fs::read(&input).expect("read Mac scan input");
    let mut city = if bytes.starts_with(b"CGMS") {
        city_from_cgms_v4_for_test(&bytes).expect("decode CGMS v4")
    } else {
        City::load(&input).expect("load Mac city save")
    };
    if let Ok(path) = std::env::var("SC2K_MAC_CGMS_XMIC_INPUT") {
        let xmic = std::fs::read(path).expect("read Mac live XMIC input");
        assert_eq!(xmic.len(), 150 * 8, "Mac live XMIC must be 1,200 bytes");
        for (index, record) in xmic.chunks_exact(8).enumerate() {
            city.set_microsim_tile_id(index, record[0]);
            city.set_microsim_stat0(index, record[1]);
            city.set_microsim_stat1(index, u16::from_be_bytes([record[2], record[3]]));
            city.set_microsim_stat2(index, u16::from_be_bytes([record[4], record[5]]));
            city.set_microsim_stat3(index, u16::from_be_bytes([record[6], record[7]]));
        }
    }
    if let Ok(path) = std::env::var("SC2K_MAC_CGMS_BASELINE_OUTPUT") {
        let baseline = cgms_v4_from_city_for_test(&city).expect("encode loaded city as CGMS v4");
        std::fs::write(path, baseline).expect("write CGMS baseline snapshot");
    }
    let mut quickdraw_seed = std::env::var("SC2K_MAC_CGMS_QUICKDRAW_SEED")
        .map(|value| value.parse().expect("parse QuickDraw seed"))
        .unwrap_or(1);
    let mut game_lcg_seed = std::env::var("SC2K_MAC_CGMS_GAME_LCG_SEED")
        .map(|value| value.parse().expect("parse game LCG seed"))
        .unwrap_or(1_103_527_590);

    // Optional calendar-path replay through Mac phase 21. This calls the
    // production per-day dispatcher in order, without pretending to model the
    // outer CODE 2 event/TickCount loop or its actor-maintenance interleave.
    if let Ok(output_dir) = std::env::var("SC2K_MAC_CGMS_CALENDAR_MONTH_DIR") {
        let output_dir = std::path::PathBuf::from(output_dir);
        std::fs::create_dir_all(&output_dir).expect("create calendar-month output directory");
        let mut state = crate::app::tests::test_state(city.clone());
        state.engine_fidelity = crate::app::EngineFidelity::Mac68k;
        state.mac_process_rng = MacToolboxRng::new(quickdraw_seed);
        state.mac_game_rng = MacGameLcgRng::new(game_lcg_seed);
        let date = state.city.saved_date().unwrap_or(CityDate {
            year: 1900,
            month: 1,
        });
        state.game_clock = GameClock::new_with_day(date.year, date.month, 24);
        for phase in 0..=21 {
            state.game_clock = state.game_clock.wrapping_add_days(1);
            let at = state.game_clock;
            state.handle_simulation_day_advanced(at.year(), at.month(), at.day());
            let snapshot = cgms_v4_from_city_for_test(&state.city)
                .expect("encode Rust calendar-month CGMS v4");
            let phase_dir = output_dir.join(format!("phase-{phase:02}"));
            std::fs::create_dir_all(&phase_dir).expect("create calendar-month phase directory");
            std::fs::write(phase_dir.join("city.cgms"), snapshot)
                .expect("write Rust calendar-month CGMS");
            for (name, plane) in [
                ("xplc", state.city.police_coverage_slice()),
                ("xfir", state.city.fire_coverage_slice()),
                ("xpop", state.city.population_density_slice()),
                ("xrog", state.city.rate_of_growth_slice()),
            ] {
                if let Some(plane) = plane {
                    std::fs::write(phase_dir.join(format!("{name}.bin")), plane)
                        .unwrap_or_else(|_| panic!("write Rust calendar-month {name}"));
                }
            }
            if std::env::var_os("SC2K_MAC_CGMS_CALENDAR_XMIC_DIR").is_some() {
                std::fs::write(phase_dir.join("live_xmic.bin"), encode_probe_xmic(&state.city))
                    .expect("write Rust calendar-month live XMIC");
            }
            println!(
                "RUST_CALENDAR_MONTH {phase} date={}-{:02}-{:02} quickdraw={} game_lcg={}",
                at.year(),
                at.month(),
                at.day(),
                state.mac_process_rng.seed(),
                state.mac_game_rng.state(),
            );
        }
        quickdraw_seed = state.mac_process_rng.seed();
        game_lcg_seed = state.mac_game_rng.state();
        println!(
            "RUST_CALENDAR_MONTH_DONE counters={}",
            (0..8)
                .map(|slot| state.city.mac_rci_counter(slot).to_string())
                .collect::<Vec<_>>()
                .join(","),
        );
    }

    // Optional whole-scheduler join: replay the Mac calendar callbacks before
    // CODE 14 from the same saved state, emitting CGMS checkpoints that can
    // be compared with the native phase-0/1/2 captures. Without this switch,
    // retain the historical isolated CODE 14 scan probe below.
    if let Ok(output_dir) = std::env::var("SC2K_MAC_CGMS_CALENDAR_PREFIX_DIR") {
        let output_dir = std::path::PathBuf::from(output_dir);
        std::fs::create_dir_all(&output_dir).expect("create calendar-prefix output directory");
        let mut state = crate::app::tests::test_state(city);
        state.engine_fidelity = crate::app::EngineFidelity::Mac68k;
        state.mac_process_rng = MacToolboxRng::new(quickdraw_seed);
        state.mac_game_rng = MacGameLcgRng::new(game_lcg_seed);
        for slot in 0..8 {
            state.city.set_mac_rci_counter(slot, 0);
        }

        let date = state.city.saved_date().unwrap_or(CityDate {
            year: 1900,
            month: 1,
        });
        state.game_clock = GameClock::new_with_day(date.year, date.month, 24);
        for phase in 0..=2 {
            state.game_clock = state.game_clock.wrapping_add_days(1);
            let at = state.game_clock;
            state.handle_simulation_day_advanced(at.year(), at.month(), at.day());
            let snapshot = cgms_v4_from_city_for_test(&state.city)
                .expect("encode Rust calendar-prefix CGMS v4");
            std::fs::write(output_dir.join(format!("phase-{phase:02}.cgms")), snapshot)
                .expect("write Rust calendar-prefix CGMS");
            for (name, plane) in [
                ("xplc", state.city.police_coverage_slice()),
                ("xfir", state.city.fire_coverage_slice()),
                ("xpop", state.city.population_density_slice()),
                ("xrog", state.city.rate_of_growth_slice()),
            ] {
                if let Some(plane) = plane {
                    std::fs::write(
                        output_dir.join(format!("phase-{phase:02}.{name}.bin")),
                        plane,
                    )
                    .unwrap_or_else(|_| panic!("write Rust calendar-prefix {name}"));
                }
            }
            if std::env::var_os("SC2K_MAC_CGMS_CALENDAR_XMIC_DIR").is_some() {
                std::fs::write(
                    output_dir.join(format!("phase-{phase:02}.live_xmic.bin")),
                    encode_probe_xmic(&state.city),
                )
                .expect("write Rust calendar-prefix live XMIC");
            }
            println!(
                "RUST_CALENDAR_PREFIX {phase} date={}-{:02}-{:02} quickdraw={} game_lcg={}",
                at.year(),
                at.month(),
                at.day(),
                state.mac_process_rng.seed(),
                state.mac_game_rng.state(),
            );
        }
        quickdraw_seed = state.mac_process_rng.seed();
        game_lcg_seed = state.mac_game_rng.state();
        city = state.city;
    }

    let partition_dir =
        std::env::var_os("SC2K_MAC_CGMS_PARTITION_DIR").map(std::path::PathBuf::from);
    if let Some(dir) = &partition_dir {
        std::fs::create_dir_all(dir).expect("create partition snapshot directory");
    }

    // The native bounded scan starts a fresh eight-lane accumulation array;
    // the CGMS fixture's saved counters are map history, not this scan's output.
    for slot in 0..8 {
        city.set_mac_rci_counter(slot, 0);
    }

    let mut quickdraw = MacToolboxRng::new(quickdraw_seed);
    // The default reproduces the native runner's startup playlist first step.
    let mut game_rng = MacGameLcgRng::new(game_lcg_seed);
    let mut sound_deadline = 0;
    for phase in 0..16 {
        crate::app::mac_budget_accumulate_monthly_rci_scan_partition_with_game_rng(
            &mut city,
            phase / 4,
            phase % 4,
            &mut quickdraw,
            &mut game_rng,
            &mut sound_deadline,
        );
        if let Some(dir) = &partition_dir {
            let snapshot =
                cgms_v4_from_city_for_test(&city).expect("encode Rust per-residue CGMS v4");
            let path = dir.join(format!("partition-{phase:02}.cgms"));
            std::fs::write(path, snapshot).expect("write Rust per-residue CGMS v4");
        }
        let counters = (0..8)
            .map(|slot| city.mac_rci_counter(slot).to_string())
            .collect::<Vec<_>>()
            .join(",");
        println!(
            "RUST_PARTITION {phase} residue={},{} counters={counters} quickdraw={} game_lcg={}",
            phase / 4,
            phase % 4,
            quickdraw.seed(),
            game_rng.state(),
        );
    }

    let output_bytes = cgms_v4_from_city_for_test(&city).expect("encode post-scan CGMS v4");
    std::fs::write(output, output_bytes).expect("write Rust post-scan CGMS");
    println!(
        "RUST_SCAN_DONE quickdraw={} game_lcg={} counters={}",
        quickdraw.seed(),
        game_rng.state(),
        (0..8)
            .map(|slot| city.mac_rci_counter(slot).to_string())
            .collect::<Vec<_>>()
            .join(","),
    );

    // Optional phase-21 replay: continue the same loaded Mac city through the
    // production Mac phase-21 path, preserving its original format adapter.
    // A CGMS-only replay cannot distinguish MacCity from a Win-layout city.
    // This still starts after a controlled scan, not from natural CODE 2/6
    // event and actor chronology.
    if let Ok(path) = std::env::var("SC2K_MAC_CGMS_PHASE21_OUTPUT") {
        let mut state = crate::app::tests::test_state(city);
        state.engine_fidelity = crate::app::EngineFidelity::Mac68k;
        state.game_clock = crate::app::game_clock_from_city(&state.city);
        state.mac_process_rng = MacToolboxRng::new(quickdraw.seed());
        state.mac_game_rng = MacGameLcgRng::new(game_rng.state());
        state.sim_calendar_day21_demand_and_graphs();
        let snapshot = cgms_v4_from_city_for_test(&state.city)
            .expect("encode Rust post-phase-21 CGMS v4");
        std::fs::write(path, snapshot).expect("write Rust phase-21 CGMS v4");
        println!(
            "RUST_PHASE21_DONE quickdraw={} population_balances={:?} class_two={} demands={:?} industry_populations={:?} cohort_summary={:?} cohort_lanes={:?}",
            state.mac_process_rng.seed(),
            state.city.mac_population_change_balances,
            state.city.mac_rci_counter(7),
            state.city.misc_i32_values(0x0718, 3),
            (0..11)
                .map(|row| state.city.mac_state_misc_u32_at(0x016c + row * 12 + 8).unwrap_or(0))
                .collect::<Vec<_>>(),
            state.city.misc_u32_values(0x0044, 3),
            state.city.misc_u32_values(0x007c, 60),
        );
    }
}
