use super::test_state;
use crate::city::{cgms_v4_from_city_for_test, city_from_cgms_v4_for_test};
use crate::engine_fidelity::EngineFidelity;
use crate::mac_random::MacToolboxRng;

/// Opt-in real-save differential probe for Mac CODE 14 phase 1 power.
/// Set `SC2K_MAC_PHASE1_INPUT` to a CGMS v4 snapshot and
/// `SC2K_MAC_PHASE1_OUTPUT` to a directory. It emits pre/post XBIT, input
/// XBLD, the full post-worker CGMS state, power summary, and QuickDraw seed.
/// No proprietary city is checked into the repository.
#[test]
fn mac_native_phase1_cgms_differential_probe() {
    let (Ok(input), Ok(output)) = (
        std::env::var("SC2K_MAC_PHASE1_INPUT"),
        std::env::var("SC2K_MAC_PHASE1_OUTPUT"),
    ) else {
        return;
    };

    let bytes = std::fs::read(input).expect("read CGMS phase-1 input");
    let city = city_from_cgms_v4_for_test(&bytes).expect("decode CGMS v4");
    let output = std::path::PathBuf::from(output);
    std::fs::create_dir_all(&output).expect("create Rust phase-1 output directory");
    let mut input_xbit = Vec::with_capacity(crate::city::CITY_TILES);
    let mut input_xbld = Vec::with_capacity(crate::city::CITY_TILES);
    for x in 0..crate::city::CITY_WIDTH {
        for y in 0..crate::city::CITY_HEIGHT {
            input_xbit.push(city.tile_bits_at(x, y).map_or(0, |bits| bits.raw_bits()));
            input_xbld.push(city.building_at(x, y));
        }
    }
    std::fs::write(output.join("input_xbit.bin"), input_xbit)
        .expect("write phase-1 input XBIT plane");
    std::fs::write(output.join("input_xbld.bin"), input_xbld)
        .expect("write phase-1 input XBLD plane");
    let entry_seed = std::env::var("SC2K_MAC_PHASE1_SEED")
        .map(|value| value.parse().expect("parse phase-1 QuickDraw seed"))
        .unwrap_or(1_889_252_988);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.mac_process_rng = MacToolboxRng::new(entry_seed);
    crate::app::power_water::mac_power_source_trace_begin();
    crate::app::power_water::mac_power_flood_trace_begin();
    crate::app::power_water::mac_power_component_trace_begin();
    crate::app::power_water::mac_power_first_pass_trace_begin();
    state.sim_calendar_day1_power();
    let source_trace = crate::app::power_water::mac_power_source_trace_take();
    let flood_trace = crate::app::power_water::mac_power_flood_trace_take();
    let component_trace = crate::app::power_water::mac_power_component_trace_take();
    let first_pass_trace = crate::app::power_water::mac_power_first_pass_trace_take();

    let bytes = cgms_v4_from_city_for_test(&state.city).expect("encode Rust phase-1 CGMS v4");
    std::fs::write(output.join("phase1.cgms"), bytes).expect("write Rust phase-1 CGMS v4");
    let mut xbit = Vec::with_capacity(crate::city::CITY_TILES);
    for x in 0..crate::city::CITY_WIDTH {
        for y in 0..crate::city::CITY_HEIGHT {
            xbit.push(
                state
                    .city
                    .tile_bits_at(x, y)
                    .map_or(0, |bits| bits.raw_bits()),
            );
        }
    }
    std::fs::write(output.join("xbit.bin"), xbit).expect("write Rust phase-1 XBIT plane");
    let mut trace = String::from("map_x,map_y,xbld,seed_before,seed_after,units\n");
    for (x, y, building, seed_before, seed_after, units) in source_trace {
        use std::fmt::Write as _;
        let units = if units == u32::MAX {
            "trap".to_owned()
        } else {
            units.to_string()
        };
        writeln!(
            trace,
            "{x},{y},{building},{seed_before},{seed_after},{units}"
        )
        .expect("append Rust power source trace");
    }
    std::fs::write(output.join("source_trace.csv"), trace)
        .expect("write Rust phase-1 source trace");
    let mut trace = String::from("root_x,root_y,map_x,map_y,remaining,xbld,xbit_before\n");
    for (root_x, root_y, x, y, remaining, building, xbit) in flood_trace {
        use std::fmt::Write as _;
        writeln!(
            trace,
            "{root_x},{root_y},{x},{y},{remaining},{building},{xbit}"
        )
        .expect("append Rust power flood trace");
    }
    std::fs::write(output.join("flood_trace.csv"), trace).expect("write Rust phase-1 flood trace");
    let mut trace =
        String::from("root_x,root_y,capacity,consumers,available,energy_conservation\n");
    for (x, y, capacity, consumers, available, energy_conservation) in component_trace {
        use std::fmt::Write as _;
        writeln!(
            trace,
            "{x},{y},{capacity},{consumers},{available},{energy_conservation}"
        )
        .expect("append Rust power component trace");
    }
    std::fs::write(output.join("component_trace.csv"), trace)
        .expect("write Rust phase-1 component trace");
    let mut trace = String::from("root_x,root_y,map_x,map_y,xbld,xbit_before\n");
    for (root_x, root_y, x, y, building, xbit) in first_pass_trace {
        use std::fmt::Write as _;
        writeln!(trace, "{root_x},{root_y},{x},{y},{building},{xbit}")
            .expect("append Rust first-pass trace");
    }
    std::fs::write(output.join("first_pass_trace.csv"), trace)
        .expect("write Rust phase-1 first-pass trace");
    println!(
        "RUST_MAC_PHASE1_DONE output={} power_summary={} quickdraw_seed={}",
        output.display(),
        state.city.mac_power_service_summary(),
        state.mac_process_rng.seed(),
    );
}
