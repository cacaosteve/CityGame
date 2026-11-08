use crate::city::{cgms_v4_from_city_for_test, city_from_cgms_v4_for_test};
use crate::mac_random::{MacGameLcgRng, MacToolboxRng};

/// One-off differential probe for a hash-pinned native CGMS scan. Set
/// SC2K_MAC_CGMS_INPUT and SC2K_MAC_CGMS_OUTPUT to produce a Rust post-scan
/// snapshot plus partition/RNG trace. Optionally set
/// SC2K_MAC_CGMS_PARTITION_DIR for per-residue snapshots,
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
    let bytes = std::fs::read(input).expect("read CGMS input");
    let mut city = city_from_cgms_v4_for_test(&bytes).expect("decode CGMS v4");
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

    let quickdraw_seed = std::env::var("SC2K_MAC_CGMS_QUICKDRAW_SEED")
        .map(|value| value.parse().expect("parse QuickDraw seed"))
        .unwrap_or(1);
    let game_lcg_seed = std::env::var("SC2K_MAC_CGMS_GAME_LCG_SEED")
        .map(|value| value.parse().expect("parse game LCG seed"))
        .unwrap_or(1_103_527_590);
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
}
