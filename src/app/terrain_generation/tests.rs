use super::mac68k::*;
use super::*;
use crate::city::BLANK_MAP_BASE_HEIGHT;

#[test]
fn win95_height_generation_consumes_exactly_the_lattice_and_refinement_draws() {
    let seed = 0x1234_5678;
    let mut actual_rng = Win95CrtRand::new(seed);
    let mut game_rng = Win95GameRand::new(1);
    let (heights, _) =
        generate_win95_height_plane(15, 5, false, false, &mut actual_rng, &mut game_rng);

    let mut expected_rng = Win95CrtRand::new(seed);
    for _ in 0..16_384 {
        expected_rng.random_u15();
    }
    assert_eq!(actual_rng, expected_rng);
    assert_eq!(heights.len(), CITY_TILES);
    assert!(heights.iter().all(|height| *height <= 0x1f));
}

#[test]
fn win95_height_generation_is_repeatable_and_uses_hills_as_lattice_range() {
    let mut a_rng = Win95CrtRand::new(7);
    let mut b_rng = Win95CrtRand::new(7);
    let mut a_game_rng = Win95GameRand::new(2);
    let mut b_game_rng = Win95GameRand::new(2);
    let (a, _) = generate_win95_height_plane(15, 5, false, false, &mut a_rng, &mut a_game_rng);
    let (b, _) = generate_win95_height_plane(15, 5, false, false, &mut b_rng, &mut b_game_rng);
    assert_eq!(a, b);

    let mut lattice_rng = Win95CrtRand::new(7);
    let modulus = 15u16 + 11;
    let expected_first = lattice_rng.random_u15() % modulus + 1;
    assert_eq!(expected_first, 10);
    assert!(a.iter().any(|height| *height >= 4));
}

#[test]
fn win95_ocean_band_sets_the_xbit_mark_and_respects_original_low_bands() {
    let mut heights = vec![12u16; CITY_TILES];
    let mut xbit = vec![0u8; CITY_TILES];
    let mut game_rng = Win95GameRand::new(0x1234);
    let mut expected_rng = game_rng;
    let first_offset = expected_rng.random_mod(10) + 10;
    let expected_x_a = 128 - usize::from(first_offset);
    let expected_x_b = expected_x_a + 1;

    apply_win95_mode_bands(&mut heights, &mut xbit, 6, true, false, &mut game_rng);

    assert_eq!(heights[city_index(expected_x_a, 0)] & 0x1f, 4);
    assert_eq!(heights[city_index(expected_x_b, 0)] & 0x1f, 3);
    assert_eq!(xbit[city_index(expected_x_b, 0)] & 0x01, 0x01);
    assert_eq!(xbit[city_index(expected_x_a, 0)] & 0x01, 0);
}

#[test]
fn win95_river_band_is_nine_tiles_wide_and_moves_down_the_map() {
    let mut heights = vec![12u16; CITY_TILES];
    let mut xbit = vec![0u8; CITY_TILES];
    let mut game_rng = Win95GameRand::new(0x9876);
    let initial_center = 64i16;

    apply_win95_mode_bands(&mut heights, &mut xbit, 7, false, true, &mut game_rng);

    for x in (initial_center - 3)..=(initial_center + 3) {
        assert_eq!(heights[city_index(x as usize, 127)] & 0x1f, 4);
    }
    for x in [initial_center - 4, initial_center + 4] {
        assert_eq!(heights[city_index(x as usize, 127)] & 0x1f, 5);
    }
    assert_eq!(
        heights[city_index((initial_center - 5) as usize, 127)] & 0x1f,
        12
    );
    assert!(xbit.iter().all(|bits| bits & 0x01 == 0));
}

#[test]
fn win95_ocean_xbit_pass_propagates_across_water_and_clears_dry_marks() {
    let mut xbit = vec![0u8; CITY_TILES];
    xbit[city_index(10, 10)] = 0x05;
    xbit[city_index(10, 9)] = 0x04;
    xbit[city_index(10, 8)] = 0x04;
    xbit[city_index(9, 10)] = 0x04;
    xbit[city_index(8, 10)] = 0x04;
    xbit[city_index(20, 20)] = 0x01;

    propagate_win95_ocean_xbit(&mut xbit);

    assert_eq!(xbit[city_index(10, 10)], 0x05);
    for (x, y) in [(10, 9), (10, 8), (9, 10), (8, 10)] {
        assert_eq!(xbit[city_index(x, y)] & 0x01, 0x01);
    }
    assert_eq!(xbit[city_index(20, 20)] & 0x01, 0);
}

#[test]
fn win95_generation_runs_height_editor_scatter_salt_and_water_stages_repeatably() {
    let mut sliders = EditMapSliderValues::default();
    sliders.set(ui::EditMapSlider::Hills, 18);
    sliders.set(ui::EditMapSlider::Water, 24);
    sliders.set(ui::EditMapSlider::Trees, 12);
    let mut first =
        City::new_blank("/tmp/sc2000-win95-terrain-a.sc2", "TerrainA").expect("blank city");
    let mut second =
        City::new_blank("/tmp/sc2000-win95-terrain-b.sc2", "TerrainB").expect("blank city");
    // PE `FUN_0040E250` clears XTRF before starting either new-city or
    // edit-map terrain setup. Seed stale values to guard that reset path.
    first.set_traffic_density(40, 40, 0x55);
    second.set_traffic_density(80, 80, 0x66);
    let mut first_crt = Win95CrtRand::new(0x1357_2468);
    let mut second_crt = first_crt;
    let mut first_game = Win95GameRand::new(0x2468_1357);
    let mut second_game = first_game;

    generate_win95(
        &mut first,
        sliders,
        true,
        true,
        &mut first_crt,
        &mut first_game,
    );
    generate_win95(
        &mut second,
        sliders,
        true,
        true,
        &mut second_crt,
        &mut second_game,
    );

    assert_eq!(first.traffic_density_at(40, 40), Some(0));
    assert_eq!(first.traffic_density_at(41, 41), Some(0));
    assert_eq!(second.traffic_density_at(80, 80), Some(0));
    assert_eq!(second.traffic_density_at(81, 81), Some(0));

    let mut wet = 0;
    let mut salt = 0;
    let mut trees = 0;
    let mut nonflat = 0;
    for x in 0..CITY_WIDTH {
        for y in 0..CITY_HEIGHT {
            let bits = first.tile_bits_at(x, y).unwrap_or_default().raw_bits();
            wet += usize::from(bits & 0x04 != 0);
            salt += usize::from(bits & 0x01 != 0);
            trees += usize::from((6..=0x0c).contains(&first.building_at(x, y)));
            nonflat += usize::from(first.altitude_at(x, y) != BLANK_MAP_BASE_HEIGHT);
            assert_eq!(first.terrain_at(x, y), second.terrain_at(x, y));
            assert_eq!(first.altm_at(x, y), second.altm_at(x, y));
            assert_eq!(first.building_at(x, y), second.building_at(x, y));
            assert_eq!(
                bits,
                second.tile_bits_at(x, y).unwrap_or_default().raw_bits()
            );
        }
    }
    assert!(wet > 0, "sea stage and water walks should create wet cells");
    assert!(salt > 0, "ocean pass should mark connected water");
    assert!(trees > 0, "tree scatter should place XBLD tree classes");
    assert!(nonflat > 0, "height core should produce varied land");
    assert_eq!(first_crt, second_crt);
    assert_eq!(first_game, second_game);
}

#[test]
fn mac68k_height_worker_consumes_lattice_and_refinement_draws_in_order() {
    let mut rng = MacToolboxRng::new(0x1234_5678);
    let mut game_rng = MacGameLcgRng::new(MacGameLcgRng::INITIAL_STATE);
    let mut expected = rng;
    let (heights, coast) =
        generate_mac68k_height_plane(15, 5, false, false, &mut rng, &mut game_rng);
    for _ in 0..16_384 {
        expected.random_u16();
    }

    assert_eq!(rng.seed(), expected.seed());
    assert_eq!(heights.len(), CITY_TILES);
    assert!(heights.iter().any(|height| height & 0x1f > 4));
    assert!(coast.iter().all(|marker| *marker == 0));
}

#[test]
fn mac68k_coast_worker_consumes_one_initial_and_one_draw_per_row() {
    let mut rng = MacToolboxRng::new(0x1020_3040);
    let mut game_rng = MacGameLcgRng::new(0x1020_3040);
    let mut expected_game_rng = game_rng;
    let mut expected = rng;
    let (heights, coast) =
        generate_mac68k_height_plane(15, 8, true, false, &mut rng, &mut game_rng);
    for _ in 0..16_384 {
        expected.random_u16();
    }
    expected_game_rng.bounded(10);
    for _ in 0..CITY_HEIGHT {
        expected_game_rng.bounded(30);
    }

    assert_eq!(rng.seed(), expected.seed());
    assert_eq!(game_rng.state(), expected_game_rng.state());
    assert!(coast.iter().any(|marker| *marker != 0));
    assert!(heights.iter().all(|height| height & 0x1f <= 31));
}

#[test]
fn mac68k_optional_bands_keep_the_quickdraw_and_game_lcg_streams_separate() {
    let mut quickdraw = MacToolboxRng::new(0x3141_5926);
    let mut game_lcg = MacGameLcgRng::new(0x2718_2818);
    let (heights, _) =
        generate_mac68k_height_plane(15, 8, true, true, &mut quickdraw, &mut game_lcg);

    let mut expected_quickdraw = MacToolboxRng::new(0x3141_5926);
    for _ in 0..16_384 {
        expected_quickdraw.random_u16();
    }
    assert_eq!(quickdraw.seed(), expected_quickdraw.seed());

    let mut expected_game_lcg = MacGameLcgRng::new(0x2718_2818);
    expected_game_lcg.bounded(10);
    for _ in 0..CITY_HEIGHT {
        expected_game_lcg.bounded(30);
    }
    expected_game_lcg.bounded(3);
    for _ in 0..CITY_HEIGHT {
        if expected_game_lcg.bounded(8) == 0 {
            expected_game_lcg.bounded(3);
        }
        expected_game_lcg.bounded(3);
    }
    assert_eq!(game_lcg.state(), expected_game_lcg.state());
    assert!(heights.iter().any(|height| height & 0x1f > 4));
}

#[test]
fn mac68k_height_normalization_stores_the_full_transformed_word() {
    assert_eq!(normalize_mac68k_height(0), 4);
    assert_eq!(normalize_mac68k_height(20), 7);
    assert_eq!(normalize_mac68k_height(0x0400), 509);
}

#[test]
fn mac68k_salt_pass_clears_dry_markers_and_propagates_wet_coast_in_four_sweeps() {
    let mut xbit = vec![0u8; CITY_TILES];
    xbit[city_index(10, 10)] = 0x05;
    xbit[city_index(10, 9)] = 0x04;
    xbit[city_index(9, 10)] = 0x04;
    xbit[city_index(20, 20)] = 0x11;

    propagate_mac68k_salt(&mut xbit);

    assert_eq!(xbit[city_index(10, 10)] & 0x01, 0x01);
    assert_eq!(xbit[city_index(10, 9)] & 0x01, 0x01);
    assert_eq!(xbit[city_index(9, 10)] & 0x01, 0x01);
    assert_eq!(xbit[city_index(20, 20)], 0x10);
}

#[test]
fn mac68k_water_component_uses_recovered_neighbor_class_and_sets_plane_bits() {
    let mut terrain = vec![0u8; CITY_TILES];
    let mut altm = vec![AltmSample::default(); CITY_TILES];
    let mut buildings = vec![0u8; CITY_TILES];
    let mut xbit = vec![0u8; CITY_TILES];
    let center = city_index(64, 64);
    let west = city_index(63, 64);
    terrain[center] = 0x30;
    xbit[west] = 0x04;
    altm[center].land = 7;
    buildings[center] = 8;

    repair_mac68k_water_component(&mut terrain, &mut altm, &mut buildings, &mut xbit, 64, 64);

    // CODE 3 0x13512: west contributes mask bit 3 (0x08).
    assert_eq!(terrain[center], 0x44);
    assert_eq!(xbit[center] & 0x04, 0x04);
    assert_eq!(altm[center].water, 7);
    assert_eq!(buildings[center], 0);
}

#[test]
fn mac68k_water_component_compares_low_nibbles_before_flood() {
    // CODE 3 0x135f2..0x135fc masks both the old class and the
    // candidate class, including candidate values above 0x0f.
    for (mask, current) in [
        (1, 0x25),
        (2, 0x22),
        (4, 0x23),
        (5, 0x20),
        (8, 0x24),
        (10, 0x21),
    ] {
        let mut terrain = vec![0u8; CITY_TILES];
        let mut altm = vec![AltmSample::default(); CITY_TILES];
        let mut buildings = vec![0u8; CITY_TILES];
        let mut xbit = vec![0u8; CITY_TILES];
        let index = city_index(64, 64);
        terrain[index] = current;
        buildings[index] = 13;
        altm[index] = AltmSample {
            land: 7,
            water: 3,
            tunnel_levels: 0x21,
        };
        for (bit, (x, y)) in [(64, 63), (65, 64), (64, 65), (63, 64)]
            .into_iter()
            .enumerate()
        {
            if mask & (1 << bit) != 0 {
                xbit[city_index(x, y)] = 4;
            }
        }
        let before = (
            terrain.clone(),
            altm.clone(),
            buildings.clone(),
            xbit.clone(),
        );

        repair_mac68k_water_component(&mut terrain, &mut altm, &mut buildings, &mut xbit, 64, 64);

        assert!(
            (terrain, altm, buildings, xbit) == before,
            "state changed for mask={mask}, current={current:#x}"
        );
    }
}

#[test]
fn mac68k_water_component_uses_diagonals_for_the_all_cardinals_wet_table() {
    let mut terrain = vec![0u8; CITY_TILES];
    let mut altm = vec![AltmSample::default(); CITY_TILES];
    let mut buildings = vec![0u8; CITY_TILES];
    let mut xbit = vec![0u8; CITY_TILES];
    let center = city_index(64, 64);
    terrain[center] = 0x30;
    for (dx, dy) in MAC_WATER_DX.into_iter().zip(MAC_WATER_DY) {
        xbit[city_index((64isize + dx) as usize, (64isize + dy) as usize)] = 0x04;
    }
    for (dx, dy) in [(-1isize, -1isize), (1, -1), (1, 1), (-1, 1)] {
        xbit[city_index((64 + dx) as usize, (64 + dy) as usize)] = 0x04;
    }
    xbit[city_index(63, 63)] = 0;

    repair_mac68k_water_component(&mut terrain, &mut altm, &mut buildings, &mut xbit, 64, 64);

    assert_eq!(terrain[center], 0x30 + MAC_DRY_CLASS[1]);
}

#[test]
fn mac68k_water_component_cardinal_shapes_follow_north_east_south_west_bits() {
    // CODE 3 0x134aa..0x13512 supplies bits 1/2/4/8 to the
    // recovered A5-0x5388 WORD table, independently of walk headings.
    for (nx, ny, expected) in [
        (64, 63, 0x45),
        (65, 64, 0x42),
        (64, 65, 0x43),
        (63, 64, 0x44),
    ] {
        let mut terrain = vec![0u8; CITY_TILES];
        let mut altm = vec![AltmSample::default(); CITY_TILES];
        let mut buildings = vec![0u8; CITY_TILES];
        let mut xbit = vec![0u8; CITY_TILES];
        xbit[city_index(nx, ny)] = 0x04;

        repair_mac68k_water_component(&mut terrain, &mut altm, &mut buildings, &mut xbit, 64, 64);

        assert_eq!(
            terrain[city_index(64, 64)],
            expected,
            "wet neighbor ({nx},{ny})"
        );
    }
}

#[test]
fn mac68k_water_walk_edge_and_water_class_stop_before_alternate_directions() {
    // Initial heading 1 probes y+1 first. The native bounds and
    // XTER 0x10..=0x2f branches both go straight to the return epilogue.
    for (x, y) in [(64, 127), (64, 64)] {
        let mut terrain = vec![0u8; CITY_TILES];
        let mut altm = vec![AltmSample::default(); CITY_TILES];
        let mut buildings = vec![0u8; CITY_TILES];
        let mut xbit = vec![0u8; CITY_TILES];
        if y == 64 {
            terrain[city_index(x, y + 1)] = 0x10;
            xbit[city_index(x, y + 1)] = 0x04;
        }
        let mut expected_bits = xbit.clone();
        expected_bits[city_index(x, y)] = 0x04;
        let mut rng = MacToolboxRng::new(3);

        mac68k_water_walk(
            &mut terrain,
            &mut altm,
            &mut buildings,
            &mut xbit,
            x,
            y,
            1,
            &mut rng,
        );

        assert!(
            xbit == expected_bits,
            "unexpected water cells from start ({x},{y})"
        );
        assert_eq!(rng.seed(), 3, "a terminal refusal spends no draw");
    }
}

#[test]
fn mac68k_water_walk_reloads_the_height_ceiling_after_descending() {
    let mut terrain = vec![0u8; CITY_TILES];
    let mut altm = vec![AltmSample::default(); CITY_TILES];
    for sample in &mut altm {
        sample.land = 31;
    }
    altm[city_index(64, 64)].land = 8;
    altm[city_index(64, 65)].land = 4;
    altm[city_index(64, 66)].land = 6;
    let mut buildings = vec![0u8; CITY_TILES];
    buildings[city_index(64, 66)] = 8;
    let mut xbit = vec![0u8; CITY_TILES];
    // The first QuickDraw result is 50,421, divisible by three, so
    // heading stays y+1. The next height six exceeds the new ceiling four.
    let mut rng = MacToolboxRng::new(3);

    mac68k_water_walk(
        &mut terrain,
        &mut altm,
        &mut buildings,
        &mut xbit,
        64,
        64,
        2,
        &mut rng,
    );

    assert_eq!(xbit.iter().filter(|bits| **bits & 0x04 != 0).count(), 2);
    assert_eq!(xbit[city_index(64, 64)] & 0x04, 0x04);
    assert_eq!(xbit[city_index(64, 65)] & 0x04, 0x04);
    assert_eq!(xbit[city_index(64, 66)], 0);
    assert_eq!(buildings[city_index(64, 66)], 8);
    assert_eq!(rng.seed(), 50_421);
}

#[test]
fn mac68k_water_walk_higher_candidate_skips_before_the_water_stop_gate() {
    let mut terrain = vec![0u8; CITY_TILES];
    let mut altm = vec![AltmSample::default(); CITY_TILES];
    for sample in &mut altm {
        sample.land = 8;
    }
    // 0x13398 branches to the next candidate before testing XTER.
    // The first candidate is high water; the second (east) is eligible.
    altm[city_index(64, 65)].land = 9;
    terrain[city_index(64, 65)] = 0x10;
    let mut buildings = vec![0u8; CITY_TILES];
    let mut xbit = vec![0u8; CITY_TILES];
    xbit[city_index(64, 65)] = 0x04;
    let mut rng = MacToolboxRng::new(3);

    mac68k_water_walk(
        &mut terrain,
        &mut altm,
        &mut buildings,
        &mut xbit,
        64,
        64,
        1,
        &mut rng,
    );

    assert_eq!(xbit[city_index(65, 64)] & 0x04, 0x04);
    assert_eq!(xbit.iter().filter(|bits| **bits & 0x04 != 0).count(), 3);
    assert_eq!(rng.seed(), 50_421);
}

#[test]
fn mac68k_water_walk_draws_length_then_y_then_x_before_first_step() {
    let mut terrain = vec![0x10u8; CITY_TILES];
    let mut altm = vec![AltmSample::default(); CITY_TILES];
    let mut buildings = vec![0u8; CITY_TILES];
    let mut xbit = vec![0u8; CITY_TILES];
    let mut rng = MacToolboxRng::new(0x7654_3210);
    let mut expected_rng = rng;
    let _length = expected_rng.random_u16() & 0x7f;
    let y = usize::from(expected_rng.random_u16() & 0x7f);
    let x = usize::from(expected_rng.random_u16() & 0x7f);
    for sample in &mut altm {
        sample.land = 8;
    }

    apply_mac68k_water_walks(
        &mut terrain,
        &mut altm,
        &mut buildings,
        &mut xbit,
        4,
        &mut rng,
    );

    assert_eq!(rng.seed(), expected_rng.seed());
    assert_eq!(xbit[city_index(x, y)], 0x04);
    assert_eq!(xbit.iter().filter(|bits| **bits & 0x04 != 0).count(), 1);
}

#[test]
fn mac68k_water_attempt_count_is_water_div_four_and_walk_draws_are_repeatable() {
    let mut terrain_a = vec![0u8; CITY_TILES];
    let mut altm_a = vec![AltmSample::default(); CITY_TILES];
    let mut buildings_a = vec![0u8; CITY_TILES];
    let mut xbit_a = vec![0u8; CITY_TILES];
    let mut terrain_b = terrain_a.clone();
    let mut altm_b = altm_a.clone();
    let mut buildings_b = buildings_a.clone();
    let mut xbit_b = xbit_a.clone();
    for sample in &mut altm_a {
        sample.land = 8;
    }
    altm_b.clone_from(&altm_a);
    let mut a = MacToolboxRng::new(0x1234_5678);
    let mut b = a;
    apply_mac68k_water_walks(
        &mut terrain_a,
        &mut altm_a,
        &mut buildings_a,
        &mut xbit_a,
        3,
        &mut a,
    );
    assert_eq!(a.seed(), MacToolboxRng::new(0x1234_5678).seed());
    apply_mac68k_water_walks(
        &mut terrain_a,
        &mut altm_a,
        &mut buildings_a,
        &mut xbit_a,
        4,
        &mut a,
    );
    apply_mac68k_water_walks(
        &mut terrain_b,
        &mut altm_b,
        &mut buildings_b,
        &mut xbit_b,
        4,
        &mut b,
    );

    assert_ne!(a.seed(), MacToolboxRng::new(0x1234_5678).seed());
    assert_eq!(a.seed(), b.seed());
    assert_eq!(terrain_a, terrain_b);
    assert_eq!(altm_a, altm_b);
    assert_eq!(buildings_a, buildings_b);
    assert_eq!(xbit_a, xbit_b);
    assert!(xbit_a.iter().any(|bits| bits & 0x04 != 0));
}

#[test]
fn mac68k_tree_scatter_growth_bands_preserve_their_branch_dependent_draws() {
    let mut actual = MacToolboxRng::new(42);
    let mut expected = actual;
    let expected_small = 6 + (expected.random_u16() & 1) as u8;
    assert_eq!(
        mac68k_tree_growth_class(5, &mut actual),
        Some(expected_small)
    );
    assert_eq!(actual.seed(), expected.seed());

    let expected_tall = 11 + (expected.random_u16() & 1) as u8;
    assert_eq!(
        mac68k_tree_growth_class(11, &mut actual),
        Some(expected_tall)
    );
    assert_eq!(actual.seed(), expected.seed());

    assert_eq!(mac68k_tree_growth_class(9, &mut actual), Some(10));
    assert_eq!(actual.seed(), expected.seed());
    assert_eq!(mac68k_tree_growth_class(13, &mut actual), None);
    assert_eq!(actual.seed(), expected.seed());
}

#[test]
fn mac68k_tree_scatter_draws_every_candidate_even_when_all_tiles_are_water() {
    let mut buildings = vec![0u8; CITY_TILES];
    let xbit = vec![0x04u8; CITY_TILES];
    let mut actual = MacToolboxRng::new(0x2468_1357);
    let mut expected = actual;
    for _ in 0..8 {
        expected.random_u16();
        expected.random_u16();
        let candidates = usize::from(expected.random_u16() & 0x3f);
        for _ in 0..candidates {
            for _ in 0..4 {
                expected.random_u16();
            }
        }
    }

    scatter_mac68k_trees(&mut buildings, &xbit, 4, &mut actual);

    assert_eq!(actual.seed(), expected.seed());
    assert!(buildings.iter().all(|building| *building == 0));
}

#[test]
fn mac68k_tree_scatter_places_repeatable_xbld_classes_only_on_dry_tiles() {
    let xbit = vec![0u8; CITY_TILES];
    let mut buildings_a = vec![0u8; CITY_TILES];
    let mut buildings_b = vec![0u8; CITY_TILES];
    let mut a = MacToolboxRng::new(0x1357_2468);
    let mut b = a;
    scatter_mac68k_trees(&mut buildings_a, &xbit, 10, &mut a);
    scatter_mac68k_trees(&mut buildings_b, &xbit, 10, &mut b);

    assert_eq!(buildings_a, buildings_b);
    assert!(
        buildings_a
            .iter()
            .any(|building| (0x06..=0x0c).contains(building))
    );
    assert!(
        buildings_a
            .iter()
            .all(|building| *building == 0 || (0x06..=0x0c).contains(building))
    );
}

#[test]
fn mac68k_generated_map_keeps_coast_salt_markers_separate_from_water() {
    let mut sliders = EditMapSliderValues::default();
    sliders.set(ui::EditMapSlider::Trees, 10);
    let mut game_rng_a = MacGameLcgRng::new(MacGameLcgRng::INITIAL_STATE);
    let mut game_rng_b = MacGameLcgRng::new(MacGameLcgRng::INITIAL_STATE);
    let (terrain_a, altm_a, xbit_a, buildings_a, water_level_a) =
        generate(0x5566_7788, true, true, sliders, &mut game_rng_a);
    let (terrain_b, altm_b, xbit_b, buildings_b, water_level_b) =
        generate(0x5566_7788, true, true, sliders, &mut game_rng_b);
    assert_eq!(terrain_a, terrain_b);
    assert_eq!(altm_a, altm_b);
    assert_eq!(xbit_a, xbit_b);
    assert_eq!(buildings_a, buildings_b);
    assert_eq!(water_level_a, 4);
    assert_eq!(water_level_a, water_level_b);
    assert_eq!(terrain_a.len(), CITY_TILES);
    assert_eq!(altm_a.len(), CITY_TILES);
    assert!(
        buildings_a
            .iter()
            .any(|building| (0x06..=0x0c).contains(building))
    );
    assert!(xbit_a.iter().any(|bits| bits & 0x01 != 0));
    assert!(xbit_a.iter().any(|bits| bits & 0x05 == 0x01));
    for x in 0..CITY_WIDTH {
        for y in 0..CITY_HEIGHT {
            let index = city_index(x, y);
            if xbit_a[index] & 0x04 != 0 {
                assert!(altm_a[index].water >= altm_a[index].land);
            }
        }
    }
}

#[test]
fn mac68k_xter_classification_uses_the_recovered_a5_neighbor_order() {
    let mut heights = vec![0u16; CITY_TILES];
    heights[city_index(10, 10)] = 5;
    heights[city_index(10, 9)] = 6; // north is weight 0x03, class 0x02

    let (terrain, _, _) = classify_mac68k_terrain(heights, 0);

    assert_eq!(terrain[city_index(10, 10)], 0x02);
}

#[test]
fn mac68k_special_xter_32_contour_raises_land_before_water_test() {
    let mut heights = vec![0u16; CITY_TILES];
    let (x, y) = (64usize, 64usize);
    heights[city_index(x, y)] = 5;
    for dy in -1isize..=1 {
        for dx in -1isize..=1 {
            if dx != 0 || dy != 0 {
                heights[city_index((x as isize + dx) as usize, (y as isize + dy) as usize)] = 6;
            }
        }
    }

    let (dry_terrain, dry_altm, dry_water) = classify_mac68k_terrain(heights.clone(), 6);
    let index = city_index(x, y);
    assert_eq!(dry_terrain[index], 0);
    assert_eq!(
        dry_altm[index].land, 6,
        "the 0x32 arm increments land even when dry"
    );
    assert!(!dry_water[index]);

    let (wet_terrain, wet_altm, wet_bits) = classify_mac68k_terrain(heights, 7);
    assert_eq!(wet_terrain[index], 0x10);
    assert_eq!(wet_altm[index].land, 6);
    assert_eq!(wet_altm[index].water, 7);
    assert!(wet_bits[index]);
}

#[test]
fn mac68k_flat_terrain_below_water_level_becomes_surface_water() {
    let heights = vec![5u16; CITY_TILES];

    let (terrain, altm, water_bits) = classify_mac68k_terrain(heights, 6);
    let index = city_index(64, 64);
    assert_eq!(terrain[index], 0x20);
    assert_eq!(altm[index].land, 5);
    assert_eq!(altm[index].water, 6);
    assert!(water_bits[index]);

    let mut dry_heights = vec![0u16; CITY_TILES];
    dry_heights[index] = 5 | (3 << 5) | (0x15 << 10);
    let (dry_terrain, dry_altm, dry_bits) = classify_mac68k_terrain(dry_heights, 5);
    assert_eq!(dry_terrain[index], 0);
    assert_eq!(dry_altm[index].land, 5);
    assert_eq!(
        dry_altm[index].water, 3,
        "dry non-0x32 path preserves ALTM upper water bits"
    );
    assert_eq!(dry_altm[index].tunnel_levels, 0x15);
    assert!(!dry_bits[index]);
}
