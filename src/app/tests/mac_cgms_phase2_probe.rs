use super::test_state;
use crate::app::{CityServiceOutcomes, PopulationStats};
use crate::city::{City, city_from_cgms_v4_for_test};
use crate::engine_fidelity::EngineFidelity;

/// One-off differential probe for the Mac CODE 14 phase-2 map publication.
/// Set `SC2K_MAC_PHASE2_INPUT` to a Mac save or CGMS v4 snapshot and
/// `SC2K_MAC_PHASE2_OUTPUT` to a directory. The test emits native-grid byte
/// planes in x-major order so they can be compared directly with the Unicorn
/// snapshots. It intentionally does not assert parity; mismatches are evidence
/// to investigate, not grounds for assuming either implementation is right.
#[test]
fn mac_native_phase2_cgms_differential_probe() {
    let (Ok(input), Ok(output)) = (
        std::env::var("SC2K_MAC_PHASE2_INPUT"),
        std::env::var("SC2K_MAC_PHASE2_OUTPUT"),
    ) else {
        return;
    };

    let bytes = std::fs::read(&input).expect("read phase-2 input");
    let city = if bytes.starts_with(b"CGMS") {
        city_from_cgms_v4_for_test(&bytes).expect("decode CGMS v4")
    } else {
        City::load(&input).expect("load original Mac save")
    };
    let source_words = crate::app::mac_pollution_source_words_for_test(&city);
    let mut native_order_source_words = Vec::with_capacity(64 * 64 * 2);
    for x in 0..64 {
        for y in 0..64 {
            native_order_source_words.extend_from_slice(&source_words[y * 64 + x].to_be_bytes());
        }
    }
    let (primary_grid, industrial_grid) = crate::app::mac_xval_terrain_grids_for_test(&city);
    let mut xval_terrain_grid = Vec::with_capacity(64 * 32 * 2);
    for x in 0..64 {
        for y in 0..32 {
            let grid = if x < 32 {
                &primary_grid
            } else {
                &industrial_grid
            };
            let grid_x = x % 32;
            xval_terrain_grid.extend_from_slice(&(grid[grid_x * 32 + y] as u16).to_be_bytes());
        }
    }
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.sim_calendar_day2_density(PopulationStats::default(), CityServiceOutcomes::default());

    let output = std::path::PathBuf::from(output);
    std::fs::create_dir_all(&output).expect("create Rust phase-2 output directory");
    std::fs::write(
        output.join("xplt_source_words.bin"),
        native_order_source_words,
    )
    .expect("write Rust pollution source words");
    std::fs::write(output.join("xval_terrain_grid.bin"), xval_terrain_grid)
        .expect("write Rust XVAL terrain grids");
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
    let mut planes = vec![("xbit", xbit)];
    for (name, source) in [
        ("xval", state.city.land_value_slice()),
        ("xcrm", state.city.crime_density_slice()),
        ("xplt", state.city.pollution_density_slice()),
        ("xtrf", state.city.traffic_density_slice()),
    ] {
        let source = source.expect("phase 2 publishes full-resolution density plane");
        let mut plane = Vec::with_capacity(64 * 64);
        for x in 0..64 {
            for y in 0..64 {
                plane.push(source[(x * 2) * crate::city::CITY_HEIGHT + y * 2]);
            }
        }
        planes.push((name, plane));
    }
    for (name, source) in [
        ("xplc", state.city.police_coverage_slice()),
        ("xfir", state.city.fire_coverage_slice()),
        ("xpop", state.city.population_density_slice()),
        ("xrog", state.city.rate_of_growth_slice()),
    ] {
        let source = source.expect("phase 2 publishes full-resolution service/population plane");
        let mut plane = Vec::with_capacity(32 * 32);
        for x in 0..32 {
            for y in 0..32 {
                plane.push(source[(x * 4) * crate::city::CITY_HEIGHT + y * 4]);
            }
        }
        planes.push((name, plane));
    }

    for (name, plane) in planes {
        let path = output.join(format!("{name}.bin"));
        std::fs::write(path, plane).expect("write Rust phase-2 output plane");
    }
    let totals: Vec<_> = [0x28, 0x34, 0x2c]
        .into_iter()
        .flat_map(|offset| {
            state
                .city
                .mac_state_misc_u32_at(offset)
                .unwrap()
                .to_be_bytes()
        })
        .collect();
    std::fs::write(output.join("totals.bin"), totals).expect("write Rust phase-2 totals");
    println!("RUST_MAC_PHASE2_DONE output={}", output.display());
}
