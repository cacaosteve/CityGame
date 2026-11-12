use super::mac68k::*;
use super::*;
use crate::city::BLANK_MAP_BASE_HEIGHT;

#[test]
fn mac_terrain_lattice_matches_native_altm_with_and_without_coast() {
    for (coast, expected) in [
        (false, 0xe9bc_d744_4d76_023f),
        (true, 0x596f_b7f9_6054_c6b9),
    ] {
        let mut rng = MacToolboxRng::new(1_106_567_514);
        let heights = build_mac68k_height_lattice(12, coast, &mut rng);
        let fingerprint =
            terrain_fingerprint(heights.iter().flat_map(|height| height.to_be_bytes()));
        assert_eq!(fingerprint, expected, "coast={coast}");
        assert_eq!(rng.seed(), 92_257_252);
    }
}

fn terrain_fingerprint(bytes: impl Iterator<Item = u8>) -> u64 {
    bytes.fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
        (hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3)
    })
}

fn packed_altm_word(tile: AltmSample) -> u16 {
    u16::from(tile.land) | (u16::from(tile.water) << 5) | (u16::from(tile.tunnel_levels) << 10)
}

fn mac_altm_fingerprint(altm: &[AltmSample]) -> u64 {
    terrain_fingerprint(
        altm.iter()
            .map(|&tile| packed_altm_word(tile))
            .flat_map(u16::to_be_bytes),
    )
}

#[test]
fn mac_terrain_stages_match_native_through_tree_scatter_and_salt() {
    // CODE 3 through initial classification, with native CODE 13 for
    // water bands and CODE 5 for the blank-map tile writes.
    for (
        (hills, water, coast, river),
        [slope_hash, salt_hash],
        [altm_hash, xter_hash, xbit_hash],
        [trees_hash, salted_hash],
        (game_seed, tree_seed),
    ) in [
        (
            (12, 5, false, false),
            [0x3630_a191_8209_ecec, 0x9c1b_da7f_8c87_2325],
            [
                0xd366_915c_bad1_a04a,
                0xfa43_dda5_e73f_9f77,
                0x9c1b_da7f_8c87_2325,
            ],
            [0x5e0f_b96c_19e5_db1a, 0x9c1b_da7f_8c87_2325],
            (1_103_527_590, 1_055_200_499),
        ),
        (
            (12, 5, true, false),
            [0x9ea0_7654_e975_4dfd, 0x38af_4651_3d2c_a3fa],
            [
                0xc4bf_8dd3_4325_f1ea,
                0xd65e_9b49_d681_6972,
                0x105a_1b5b_a554_559e,
            ],
            [0x5d18_6d31_0976_da75, 0x4832_fc4e_1286_2888],
            (3_124_316_263, 1_848_532_771),
        ),
        (
            (12, 5, false, true),
            [0x4a5b_7b16_9d68_cb6c, 0x9c1b_da7f_8c87_2325],
            [
                0x3b41_fc16_b3ae_5031,
                0x4bd5_2d8c_e14c_9fcf,
                0xc2cf_b771_a002_31b5,
            ],
            [0xc5f1_09e1_1bfc_1540, 0xc2cf_b771_a002_31b5],
            (1_692_766_423, 1_241_968_176),
        ),
        (
            (12, 5, true, true),
            [0xe490_caee_3171_18df, 0x38af_4651_3d2c_a3fa],
            [
                0xf4d1_3d7a_4470_b725,
                0xa972_ea4d_e8be_2d50,
                0x3137_fdc2_2b1d_4eb6,
            ],
            [0x8976_b35b_a74b_2760, 0xa56b_7c99_fb0b_933f],
            (1_869_663_702, 2_110_068_802),
        ),
        (
            (47, 47, false, false),
            [0x01f7_2b3c_e291_bed9, 0x9c1b_da7f_8c87_2325],
            [
                0x3ed9_7572_c746_5a2a,
                0x03e6_e882_9093_7182,
                0x60b8_6ef4_1fef_6445,
            ],
            [0xd444_41e3_bec2_034c, 0x60b8_6ef4_1fef_6445],
            (1_103_527_590, 527_973_869),
        ),
        (
            (47, 47, true, false),
            [0xabfb_9e71_0545_1e51, 0x38af_4651_3d2c_a3fa],
            [
                0xa9e6_697c_7e44_7960,
                0x4c73_39a4_e4c4_3479,
                0x46c5_f129_9ec0_acf2,
            ],
            [0x76de_a761_c522_4073, 0x749e_80f8_75d2_e367],
            (3_124_316_263, 664_150_869),
        ),
        (
            (47, 47, false, true),
            [0x3c0f_980e_5df8_9e5b, 0x9c1b_da7f_8c87_2325],
            [
                0x3801_f375_5e7a_f56e,
                0x7649_6aa8_8b6e_635b,
                0x06f0_e966_8f8e_0bfd,
            ],
            [0x4427_33e2_95d4_8951, 0x06f0_e966_8f8e_0bfd],
            (1_692_766_423, 1_757_146_547),
        ),
        (
            (47, 47, true, true),
            [0x7dd8_fd11_380a_b5a5, 0x38af_4651_3d2c_a3fa],
            [
                0xfb8f_3523_b0a4_4149,
                0xdad7_224e_5a38_7319,
                0x3599_16a0_08ba_77ba,
            ],
            [0xb93a_8186_4c51_d636, 0x6371_a66e_dfcc_ae2f],
            (1_869_663_702, 616_111_013),
        ),
    ] {
        let mut rng = MacToolboxRng::new(1_106_567_514);
        let mut game_rng = MacGameLcgRng::after_startup_playlist_choice();
        let (heights, markers) =
            generate_mac68k_height_plane(hills, water, coast, river, &mut rng, &mut game_rng);
        assert_eq!(
            terrain_fingerprint(heights.iter().flat_map(|height| height.to_be_bytes())),
            slope_hash,
            "hills={hills}, water={water}, coast={coast}, river={river}",
        );
        assert_eq!(terrain_fingerprint(markers.iter().copied()), salt_hash);
        assert_eq!(rng.seed(), 92_257_252);
        assert_eq!(game_rng.state(), game_seed);

        let sea_level = ((water + 4) >> 3).max(u8::from(coast || river) * 4);
        let (terrain, altm, wet) = classify_mac68k_terrain(heights, sea_level);
        let mut xbit: Vec<_> = markers
            .into_iter()
            .zip(wet)
            .map(|(salt, wet)| salt | (u8::from(wet) << 2))
            .collect();
        let case = (hills, water, coast, river);
        assert_eq!(mac_altm_fingerprint(&altm), altm_hash, "ALTM {case:?}");
        assert_eq!(
            terrain_fingerprint(terrain.into_iter()),
            xter_hash,
            "XTER {case:?}"
        );
        assert_eq!(
            terrain_fingerprint(xbit.iter().copied()),
            xbit_hash,
            "XBIT {case:?}"
        );

        let mut buildings = vec![0; CITY_TILES];
        scatter_mac68k_trees(&mut buildings, &xbit, 15, &mut rng);
        assert_eq!(
            terrain_fingerprint(buildings.into_iter()),
            trees_hash,
            "XBLD {case:?}"
        );
        assert_eq!(rng.seed(), tree_seed, "tree draws {case:?}");
        if coast {
            propagate_mac68k_salt(&mut xbit);
        }
        assert_eq!(
            terrain_fingerprint(xbit.into_iter()),
            salted_hash,
            "salt {case:?}"
        );
    }
}

#[test]
fn mac_terrain_final_planes_match_native_after_streams() {
    // Native CODE 3 generation with 15 Trees and controlled post-reset RNG state.
    for ((hills, water, coast, river), expected_planes, expected_seeds) in [
        (
            (12, 5, false, false),
            [
                0x298c_e439_bdfb_cdca,
                0x33ad_b8d2_f347_efd9,
                0x21cc_4122_66f0_5b29,
                0x739d_dbcb_6181_ee4c,
            ],
            (948_910_582, 1_103_527_590),
        ),
        (
            (12, 5, true, false),
            [
                0x4e11_5916_4506_64aa,
                0xac30_4602_49d2_6d49,
                0x35a1_1d00_b3e4_2324,
                0x5d18_6d31_0976_da75,
            ],
            (468_419_878, 3_124_316_263),
        ),
        (
            (12, 5, false, true),
            [
                0x1380_9664_988d_ce31,
                0x3427_26b7_a441_2245,
                0x56ca_9c25_d456_ef5d,
                0xfcf6_6af1_4517_f5f1,
            ],
            (1_586_019_082, 1_692_766_423),
        ),
        (
            (12, 5, true, true),
            [
                0xc583_bca8_28ff_3465,
                0xcf1d_df54_2ca8_4062,
                0xfa25_0ff5_448a_c447,
                0x9acb_dec6_3722_6fde,
            ],
            (308_520_939, 1_869_663_702),
        ),
        (
            (47, 47, false, false),
            [
                0x6bd1_883c_3927_8d7d,
                0x5c03_7283_d229_dd80,
                0x970a_a36d_6ed9_d8c5,
                0x1a1b_bedd_f7f1_302f,
            ],
            (655_202_818, 1_103_527_590),
        ),
        (
            (47, 47, true, false),
            [
                0xaca6_8a8a_53fa_7727,
                0x4979_d793_320f_33f2,
                0xd72e_3b5b_b6be_9283,
                0x4446_e68c_9a30_e399,
            ],
            (1_450_719_170, 3_124_316_263),
        ),
        (
            (47, 47, false, true),
            [
                0x65f8_6f3f_ecbd_0e01,
                0x0c2c_97cd_66d0_9661,
                0x4d52_2366_a26b_fb25,
                0xad56_4b01_b681_dd44,
            ],
            (1_844_971_800, 1_692_766_423),
        ),
        (
            (47, 47, true, true),
            [
                0x6027_124c_9d25_3f82,
                0x2db2_0fc1_a3c4_7efd,
                0xaeb8_0369_17dc_42d7,
                0x0174_8e8f_0518_5af4,
            ],
            (1_580_757_233, 1_869_663_702),
        ),
    ] {
        let mut rng = MacToolboxRng::new(1_106_567_514);
        let mut game_rng = MacGameLcgRng::after_startup_playlist_choice();
        let (terrain, altm, xbit, buildings, _) = generate_with_rng(
            &mut rng,
            coast,
            river,
            ui::EditMapSliderValues {
                hills,
                water,
                trees: 15,
            },
            &mut game_rng,
        );
        let planes = [
            mac_altm_fingerprint(&altm),
            terrain_fingerprint(terrain.into_iter()),
            terrain_fingerprint(xbit.into_iter()),
            terrain_fingerprint(buildings.into_iter()),
        ];
        let case = (hills, water, coast, river);
        assert_eq!(planes, expected_planes, "ALTM/XTER/XBIT/XBLD {case:?}");
        assert_eq!(
            (rng.seed(), game_rng.state()),
            expected_seeds,
            "RNG {case:?}"
        );
    }
}

#[test]
fn mac_terrain_hills_water_slider_endpoints_match_native_after_streams() {
    // Native CODE 3 worker captures at the remaining slider endpoints.
    for ((hills, water, coast, river), expected_planes, expected_seeds) in [
        (
            (0, 0, false, false),
            [
                0xa966_191a_8bec_9d1a,
                0x6be4_74ea_7639_d173,
                0x9c1b_da7f_8c87_2325,
                0x5e0f_b96c_19e5_db1a,
            ],
            (1_055_200_499, 1_103_527_590),
        ),
        (
            (0, 0, true, false),
            [
                0xd4ea_173f_8ae4_62d8,
                0xf5c0_617a_9b8f_b00e,
                0x27b5_83a2_902a_d7bc,
                0xe34d_ce26_a89e_58fc,
            ],
            (2_119_118_042, 3_124_316_263),
        ),
        (
            (0, 0, false, true),
            [
                0x94dd_ce12_867f_7613,
                0xccd8_435a_345c_a6d4,
                0x231a_0207_b69a_5999,
                0x738c_3048_518b_1de5,
            ],
            (964_946_546, 1_692_766_423),
        ),
        (
            (0, 0, true, true),
            [
                0xdad1_28d4_e2fe_b5c8,
                0x35bf_4879_5f07_6f48,
                0xf1c7_cc5b_49aa_d2ac,
                0x6514_e9ba_fc41_023d,
            ],
            (1_089_891_459, 1_869_663_702),
        ),
        (
            (23, 23, false, false),
            [
                0x6361_31e3_54fa_1664,
                0x8edf_2111_926a_1537,
                0xcef6_f9b2_f90e_2cf1,
                0xcab1_7463_2e97_db87,
            ],
            (1_736_408_120, 1_103_527_590),
        ),
        (
            (23, 23, true, false),
            [
                0x3362_1730_55ab_2ba0,
                0x0b41_a66b_6684_90e5,
                0xaa43_d7ae_2b26_9e54,
                0x4a41_a45f_1438_a66f,
            ],
            (345_583_826, 3_124_316_263),
        ),
        (
            (23, 23, false, true),
            [
                0xb289_8023_042d_921a,
                0x057f_6e7b_6746_e245,
                0x2cb7_429a_f8eb_7be1,
                0x5d46_ff65_216c_34bf,
            ],
            (1_706_416_083, 1_692_766_423),
        ),
        (
            (23, 23, true, true),
            [
                0x3956_20aa_5884_9dca,
                0xabb8_2557_80b0_411a,
                0xed07_6dc9_f70b_b684,
                0x67e8_cafd_0211_8617,
            ],
            (1_050_622_800, 1_869_663_702),
        ),
    ] {
        let mut rng = MacToolboxRng::new(1_106_567_514);
        let mut game_rng = MacGameLcgRng::after_startup_playlist_choice();
        let (terrain, altm, xbit, buildings, _) = generate_with_rng(
            &mut rng,
            coast,
            river,
            ui::EditMapSliderValues {
                hills,
                water,
                trees: 15,
            },
            &mut game_rng,
        );
        let planes = [
            mac_altm_fingerprint(&altm),
            terrain_fingerprint(terrain.into_iter()),
            terrain_fingerprint(xbit.into_iter()),
            terrain_fingerprint(buildings.into_iter()),
        ];
        let case = (hills, water, coast, river);
        assert_eq!(planes, expected_planes, "ALTM/XTER/XBIT/XBLD {case:?}");
        assert_eq!(
            (rng.seed(), game_rng.state()),
            expected_seeds,
            "RNG {case:?}"
        );
    }
}

#[test]
fn mac_terrain_hills_water_midrange_sliders_match_native_after_streams() {
    // Native CODE 3 captures at intermediate slider values, with Trees=15.
    for ((hills, water, coast, river), expected_planes, expected_seeds) in [
        (
            (31, 18, false, false),
            [
                0xff2e_0e0e_a9cf_6329,
                0x2e37_781a_871d_14ac,
                0x5ff4_3cda_0fe3_1761,
                0x40a6_7fb1_2c67_ecc5,
            ],
            (29_691_791, 1_103_527_590),
        ),
        (
            (31, 18, true, false),
            [
                0x08ef_e536_3dce_c712,
                0x6867_a2ac_29f5_c9e1,
                0x8b29_7245_6495_bf4c,
                0xb8f8_f47f_553a_ece0,
            ],
            (438_399_887, 3_124_316_263),
        ),
        (
            (31, 18, false, true),
            [
                0xc415_cac4_d35f_6645,
                0x8a72_4e88_4ac6_9d08,
                0x559a_a179_8b87_2871,
                0x513d_dcf6_787c_1835,
            ],
            (664_150_869, 1_692_766_423),
        ),
        (
            (31, 18, true, true),
            [
                0x33cb_1fc1_9c6b_79c9,
                0x4215_6265_3548_845b,
                0xdc9f_648b_c4f1_10df,
                0x6be7_38b6_918b_553e,
            ],
            (174_699_442, 1_869_663_702),
        ),
        (
            (7, 39, false, false),
            [
                0x99f4_8fd3_edf9_b6ea,
                0xe561_924f_a81c_2ca6,
                0x3718_38df_1fe8_41a5,
                0x1f4b_62c1_8925_f363,
            ],
            (1_457_653_723, 1_103_527_590),
        ),
        (
            (7, 39, true, false),
            [
                0x375e_540c_1c7d_1e48,
                0x4684_ebb2_ce07_ddbe,
                0x155e_8fec_0f78_334f,
                0x7ac8_f03a_2915_d581,
            ],
            (956_190_740, 3_124_316_263),
        ),
        (
            (7, 39, false, true),
            [
                0x99de_3601_8e56_1ab7,
                0x31bc_644a_97c8_742c,
                0x66f9_1868_2a6d_88f5,
                0x1867_1314_2999_16b1,
            ],
            (1_818_394_483, 1_692_766_423),
        ),
        (
            (7, 39, true, true),
            [
                0xdd2f_7f83_1c21_8779,
                0xf8f6_ddbe_2b29_35a8,
                0xd8c1_ad6e_3024_8857,
                0xb8a1_8475_3393_7fef,
            ],
            (169_955_316, 1_869_663_702),
        ),
    ] {
        let mut rng = MacToolboxRng::new(1_106_567_514);
        let mut game_rng = MacGameLcgRng::after_startup_playlist_choice();
        let (terrain, altm, xbit, buildings, _) = generate_with_rng(
            &mut rng,
            coast,
            river,
            ui::EditMapSliderValues {
                hills,
                water,
                trees: 15,
            },
            &mut game_rng,
        );
        let planes = [
            mac_altm_fingerprint(&altm),
            terrain_fingerprint(terrain.into_iter()),
            terrain_fingerprint(xbit.into_iter()),
            terrain_fingerprint(buildings.into_iter()),
        ];
        let case = (hills, water, coast, river);
        assert_eq!(planes, expected_planes, "ALTM/XTER/XBIT/XBLD {case:?}");
        assert_eq!(
            (rng.seed(), game_rng.state()),
            expected_seeds,
            "RNG {case:?}"
        );
    }
}

#[test]
fn mac_terrain_hills_water_cross_extremes_match_native_after_streams() {
    // Native CODE 3 captures where one terrain slider is at each end.
    for ((hills, water, coast, river), expected_planes, expected_seeds) in [
        (
            (0, 47, false, false),
            [
                0x62f8_75f3_70dd_5a1a,
                0x2903_5d7d_4a5e_33a3,
                0xcfcf_e895_5a28_2325,
                0x9c1b_da7f_8c87_2325,
            ],
            (1_072_945_271, 1_103_527_590),
        ),
        (
            (0, 47, true, false),
            [
                0x73b8_964c_f226_ebba,
                0xc78d_c43e_51de_0501,
                0xa542_8fe7_e3de_2986,
                0x9c1b_da7f_8c87_2325,
            ],
            (1_072_945_271, 3_124_316_263),
        ),
        (
            (0, 47, false, true),
            [
                0x3dac_796d_98f3_4c0d,
                0xfdd6_e318_bba7_db59,
                0xcfcf_e895_5a28_2325,
                0x9c1b_da7f_8c87_2325,
            ],
            (1_072_945_271, 1_692_766_423),
        ),
        (
            (0, 47, true, true),
            [
                0xd132_660b_ff7f_31f5,
                0xb9ee_3cd6_bfb9_fe1f,
                0xa542_8fe7_e3de_2986,
                0x9c1b_da7f_8c87_2325,
            ],
            (1_072_945_271, 1_869_663_702),
        ),
        (
            (47, 0, false, false),
            [
                0x1f71_aaba_bc2c_1f2a,
                0x4f6a_ebbf_b1c7_34b2,
                0x9c1b_da7f_8c87_2325,
                0x5e0f_b96c_19e5_db1a,
            ],
            (1_055_200_499, 1_103_527_590),
        ),
        (
            (47, 0, true, false),
            [
                0xb419_c20f_b21b_2bf3,
                0x7ff0_25cc_01b3_1ffc,
                0x9209_d2a7_9e5f_d6f7,
                0x9b61_0d62_8822_fcf4,
            ],
            (1_936_646_148, 3_124_316_263),
        ),
        (
            (47, 0, false, true),
            [
                0xe7f0_5855_616a_20c9,
                0xf86e_7a88_222c_50c3,
                0x2318_5427_8266_0ae9,
                0x480e_0e84_c1ba_a875,
            ],
            (332_460_123, 1_692_766_423),
        ),
        (
            (47, 0, true, true),
            [
                0xcf1e_9b1c_7b82_fea7,
                0x78be_44ad_efe1_bd97,
                0x408c_d34d_ebe7_6063,
                0xce0f_be59_f2b4_2ee7,
            ],
            (529_818_974, 1_869_663_702),
        ),
    ] {
        let mut rng = MacToolboxRng::new(1_106_567_514);
        let mut game_rng = MacGameLcgRng::after_startup_playlist_choice();
        let (terrain, altm, xbit, buildings, _) = generate_with_rng(
            &mut rng,
            coast,
            river,
            ui::EditMapSliderValues {
                hills,
                water,
                trees: 15,
            },
            &mut game_rng,
        );
        let planes = [
            mac_altm_fingerprint(&altm),
            terrain_fingerprint(terrain.into_iter()),
            terrain_fingerprint(xbit.into_iter()),
            terrain_fingerprint(buildings.into_iter()),
        ];
        let case = (hills, water, coast, river);
        assert_eq!(planes, expected_planes, "ALTM/XTER/XBIT/XBLD {case:?}");
        assert_eq!(
            (rng.seed(), game_rng.state()),
            expected_seeds,
            "RNG {case:?}"
        );
    }
}

#[test]
fn mac_terrain_tree_slider_endpoints_match_native_after_streams() {
    // Native CODE 3 captures with Trees 0 and 47 at Hills/Water 12/5.
    for ((trees, coast, river), expected_planes, expected_seeds) in [
        (
            (0, false, false),
            [
                0xa918_3d1b_7c5a_7a2a,
                0xa1f7_67c1_7562_5e6e,
                0x095a_4ccc_2652_a135,
                0x9c1b_da7f_8c87_2325,
            ],
            (1_089_154_676, 1_103_527_590),
        ),
        (
            (0, true, false),
            [
                0xd4c3_02c7_3527_a66a,
                0x6e0a_baa0_8068_de84,
                0xa35b_cb09_d49f_e0ec,
                0x9c1b_da7f_8c87_2325,
            ],
            (218_555_032, 3_124_316_263),
        ),
        (
            (0, false, true),
            [
                0x6d82_fbd5_3d81_24b1,
                0x5a4c_e970_99e5_9ba9,
                0xb2da_8228_6b49_0a21,
                0x9c1b_da7f_8c87_2325,
            ],
            (218_555_032, 1_692_766_423),
        ),
        (
            (0, true, true),
            [
                0xf4d1_3d7a_4470_b725,
                0xa972_ea4d_e8be_2d50,
                0xa56b_7c99_fb0b_933f,
                0x9c1b_da7f_8c87_2325,
            ],
            (1_336_354_106, 1_869_663_702),
        ),
        (
            (47, false, false),
            [
                0x2e9a_bfe4_704c_ffca,
                0x4798_8ffa_b6b4_7608,
                0x472e_d616_59d8_5519,
                0xa0a6_6e3d_e260_368a,
            ],
            (134_767_361, 1_103_527_590),
        ),
        (
            (47, true, false),
            [
                0xc4bf_8dd3_4325_f1ea,
                0xd65e_9b49_d681_6972,
                0x4832_fc4e_1286_2888,
                0x3e41_b1d4_f39f_c22d,
            ],
            (267_348_982, 3_124_316_263),
        ),
        (
            (47, false, true),
            [
                0x7fd5_c8e5_9b55_d071,
                0x0c96_5579_b670_cfc9,
                0x1d3e_433c_78f4_7449,
                0x09e9_047c_4b39_cdb0,
            ],
            (648_607_364, 1_692_766_423),
        ),
        (
            (47, true, true),
            [
                0xf4ce_9619_c886_6a05,
                0x5c5e_cb3b_9108_a81a,
                0xbd25_b248_39e2_5ea3,
                0x7e0e_079e_7826_58cc,
            ],
            (1_472_184_410, 1_869_663_702),
        ),
    ] {
        let mut rng = MacToolboxRng::new(1_106_567_514);
        let mut game_rng = MacGameLcgRng::after_startup_playlist_choice();
        let (terrain, altm, xbit, buildings, _) = generate_with_rng(
            &mut rng,
            coast,
            river,
            ui::EditMapSliderValues {
                hills: 12,
                water: 5,
                trees,
            },
            &mut game_rng,
        );
        let planes = [
            mac_altm_fingerprint(&altm),
            terrain_fingerprint(terrain.into_iter()),
            terrain_fingerprint(xbit.into_iter()),
            terrain_fingerprint(buildings.into_iter()),
        ];
        let case = (trees, coast, river);
        assert_eq!(planes, expected_planes, "ALTM/XTER/XBIT/XBLD {case:?}");
        assert_eq!(
            (rng.seed(), game_rng.state()),
            expected_seeds,
            "RNG {case:?}"
        );
    }
}

#[test]
fn win95_height_plane_matches_native_through_slope_relaxation() {
    let mut actual = Vec::new();
    let mut expected = Vec::new();
    for ((hills, water, coast, river), expected_planes, game_seed) in [
        (
            (12, 5, false, false),
            [0xf4b4_8747_d29b_5777, 0x9c1b_da7f_8c87_2325],
            1,
        ),
        (
            (12, 5, true, false),
            [0x445f_20a3_8462_663c, 0x3860_af71_fe07_d7c9],
            519_488_550,
        ),
        (
            (12, 5, false, true),
            [0xe72d_4d92_7c75_e621, 0x9c1b_da7f_8c87_2325],
            1_692_766_423,
        ),
        (
            (12, 5, true, true),
            [0xc740_8cd9_849f_dde7, 0x3860_af71_fe07_d7c9],
            1_759_879_576,
        ),
        (
            (47, 47, false, false),
            [0x41cb_a4b0_542c_9edd, 0x9c1b_da7f_8c87_2325],
            1,
        ),
        (
            (47, 47, true, false),
            [0x5569_7f15_9a3e_84d7, 0x3860_af71_fe07_d7c9],
            519_488_550,
        ),
        (
            (47, 47, false, true),
            [0x9e5c_f763_63c4_0332, 0x9c1b_da7f_8c87_2325],
            1_692_766_423,
        ),
        (
            (47, 47, true, true),
            [0xc54c_efdd_633c_5859, 0x3860_af71_fe07_d7c9],
            1_759_879_576,
        ),
    ] {
        let mut crt = Win95CrtRand::new(0x1234_5678);
        let mut game = Win95GameRand::new(1);
        let (heights, salt) =
            generate_win95_height_plane(hills, water, coast, river, &mut crt, &mut game);
        let planes = [
            terrain_fingerprint(heights.iter().flat_map(|height| height.to_le_bytes())),
            terrain_fingerprint(salt.into_iter()),
        ];
        let case = (hills, water, coast, river);
        actual.push((case, planes, game.state()));
        expected.push((case, expected_planes, game_seed));
        assert_eq!(crt.seed(), 1_856_935_544, "CRT state {case:?}");
    }
    assert_eq!(actual, expected);
}

#[test]
fn win95_terrain_final_planes_match_native_after_streams() {
    let mut actual = Vec::new();
    let mut expected = Vec::new();
    for ((hills, water, coast, river), expected_planes, expected_seeds) in [
        (
            (12, 5, false, false),
            [
                0xf5a7_9c0c_d47d_1bc5,
                0x2a9f_1846_fa6a_a5bb,
                0x0880_273a_ed5b_c915,
                0x2988_4950_b11c_cf61,
            ],
            (2_791_813_854, 1),
        ),
        (
            (12, 5, true, false),
            [
                0x963d_ff2a_29a6_ad68,
                0xaa3f_a85e_0c18_5f31,
                0x15fd_5854_26b5_92ea,
                0x816f_896b_2b56_8b30,
            ],
            (605_971_404, 519_488_550),
        ),
        (
            (12, 5, false, true),
            [
                0x3226_9893_ce89_76b0,
                0xb0fd_cb0f_61b1_36aa,
                0x67c4_4693_734e_8fd5,
                0xdfa7_3910_eb5c_0341,
            ],
            (2_315_638_343, 1_692_766_423),
        ),
        (
            (12, 5, true, true),
            [
                0x130d_7e63_2fe5_cb1b,
                0xc0c1_61be_cfe7_56d2,
                0xff79_bc12_af11_e1d9,
                0x85c2_5303_2abe_ec9f,
            ],
            (3_587_676_559, 1_759_879_576),
        ),
        (
            (47, 47, false, false),
            [
                0x3258_3797_d6e5_a65a,
                0xb37a_2d97_d929_bb9c,
                0xcefc_eee3_36ca_46dd,
                0x55ba_b822_16f1_57d2,
            ],
            (2_030_671_706, 1),
        ),
        (
            (47, 47, true, false),
            [
                0xd72d_6d69_e432_5b5e,
                0xf617_98a0_9c89_5935,
                0x6934_25ff_8283_449d,
                0xde7a_da86_e7c0_0f85,
            ],
            (2_818_902_286, 519_488_550),
        ),
        (
            (47, 47, false, true),
            [
                0xb200_a0c8_0cd0_3c97,
                0x07d5_58a1_bd30_c7fd,
                0x84d6_d6cf_2893_a90d,
                0x3729_4404_eda5_e518,
            ],
            (3_928_575_218, 1_692_766_423),
        ),
        (
            (47, 47, true, true),
            [
                0x6504_fe9b_ac67_1c22,
                0xb897_eea5_e973_4b20,
                0xd4c3_5d26_015e_e221,
                0x8af0_d243_e36d_8448,
            ],
            (728_653_835, 1_759_879_576),
        ),
    ] {
        let mut city = City::new_blank("/tmp/citygame-win95-terrain-replay.sc2", "Terrain")
            .expect("blank city");
        let mut crt = Win95CrtRand::new(0x1234_5678);
        let mut game = Win95GameRand::new(1);
        generate_win95(
            &mut city,
            EditMapSliderValues {
                hills,
                water,
                trees: 15,
            },
            coast,
            river,
            &mut crt,
            &mut game,
        );
        let cells = || (0..CITY_WIDTH).flat_map(|x| (0..CITY_HEIGHT).map(move |y| (x, y)));
        let planes = [
            terrain_fingerprint(
                cells().flat_map(|(x, y)| packed_altm_word(city.altm_at(x, y)).to_le_bytes()),
            ),
            terrain_fingerprint(cells().map(|(x, y)| city.terrain_at(x, y))),
            terrain_fingerprint(cells().map(|(x, y)| city.tile_bits_at(x, y).unwrap().raw_bits())),
            terrain_fingerprint(cells().map(|(x, y)| city.building_at(x, y))),
        ];
        let case = (hills, water, coast, river);
        actual.push((case, planes, (crt.seed(), game.state())));
        expected.push((case, expected_planes, expected_seeds));
    }
    assert_eq!(actual, expected, "ALTM/XTER/XBIT/XBLD and RNG states");
}

#[test]
fn win95_terrain_tree_slider_endpoints_and_midpoint_match_native_with_all_coast_river_modes() {
    for (trees, (hills, water, coast, river), expected_planes, expected_seeds) in [
        (
            0,
            (12, 5, false, false),
            [
                0x0b1b_4148_e326_d225,
                0xbfd3_425d_1bd5_4e77,
                0x45ec_0c44_c921_9b61,
                0x9c1b_da7f_8c87_2325,
            ],
            (3_353_134_645, 1),
        ),
        (
            0,
            (12, 5, true, false),
            [
                0x42b7_451b_55ba_8988,
                0xbd45_e07a_88f8_3948,
                0x4826_735e_df2b_50b2,
                0x9c1b_da7f_8c87_2325,
            ],
            (3_353_134_645, 519_488_550),
        ),
        (
            0,
            (12, 5, false, true),
            [
                0x6055_5dfd_a228_d170,
                0xb6da_ea04_c58e_96a6,
                0xedade0977bddce35,
                0x9c1b_da7f_8c87_2325,
            ],
            (3_353_134_645, 1_692_766_423),
        ),
        (
            0,
            (12, 5, true, true),
            [
                0x34fe_ceb6_8e30_197b,
                0x64d7_b6ed_73f6_44ec,
                0x3a80_cfe3_3bba_84f1,
                0x9c1b_da7f_8c87_2325,
            ],
            (3_353_134_645, 1_759_879_576),
        ),
        (
            23,
            (12, 5, false, false),
            [
                0x52f0_c957_8f49_ad85,
                0x7919_5361_8ec8_556b,
                0xc2a8_04fa_b7ef_7b4d,
                0x11eb_530d_0489_b329,
            ],
            (4_189_579_579, 1),
        ),
        (
            23,
            (12, 5, true, false),
            [
                0xd4ba_0221_49c1_4588,
                0x530f_1538_4bb1_49f1,
                0x5e58_f11d_d0ec_14de,
                0xdbfa_a977_c74f_f4f6,
            ],
            (801_239_473, 519_488_550),
        ),
        (
            23,
            (12, 5, false, true),
            [
                0xdd0f_65cf_33a4_33b0,
                0x8646_ff59_dbc5_3fdd,
                0xf91d_7d96_9019_8551,
                0x13ed_f7a1_b94e_3776,
            ],
            (2_635_699_365, 1_692_766_423),
        ),
        (
            23,
            (12, 5, true, true),
            [
                0xaeeb_e66d_a639_2b9b,
                0x1460_7bb8_8cdb_c460,
                0xc291_af51_cc2a_a78d,
                0x177c_9b9e_4d72_a621,
            ],
            (3_185_879_445, 1_759_879_576),
        ),
        (
            47,
            (12, 5, false, false),
            [
                0xccb4_e5f7_40c8_aaa5,
                0x790e_45bc_ae9a_cda4,
                0x4440_f58a_ac48_3b6d,
                0xf387_e1aa_e976_6565,
            ],
            (3_136_190_816, 1),
        ),
        (
            47,
            (12, 5, true, false),
            [
                0x7d64_41f7_6316_1708,
                0x6775_0575_9b4c_f6c0,
                0x2618_46df_46df_6dea,
                0x17ba_ce61_dac5_e578,
            ],
            (593_237_467, 519_488_550),
        ),
        (
            47,
            (12, 5, false, true),
            [
                0xa041_b6eb_6266_3950,
                0x528a_70c6_a390_6e12,
                0xce28_a498_4e7b_f569,
                0xf294_4bf4_f736_c4ff,
            ],
            (2_699_992_303, 1_692_766_423),
        ),
        (
            47,
            (12, 5, true, true),
            [
                0x96b5_9b84_8e14_469b,
                0x2073_41dd_c484_66a2,
                0x1c81_8905_cbb8_902d,
                0x24b3_9df3_d1f9_e638,
            ],
            (2_842_535_940, 1_759_879_576),
        ),
    ] {
        let mut city =
            City::new_blank("/tmp/citygame-win95-tree-slider.sc2", "Terrain").expect("blank city");
        let mut crt = Win95CrtRand::new(0x1234_5678);
        let mut game = Win95GameRand::new(1);
        generate_win95(
            &mut city,
            EditMapSliderValues {
                hills,
                water,
                trees,
            },
            coast,
            river,
            &mut crt,
            &mut game,
        );
        let cells = || (0..CITY_WIDTH).flat_map(|x| (0..CITY_HEIGHT).map(move |y| (x, y)));
        let planes = [
            terrain_fingerprint(
                cells().flat_map(|(x, y)| packed_altm_word(city.altm_at(x, y)).to_le_bytes()),
            ),
            terrain_fingerprint(cells().map(|(x, y)| city.terrain_at(x, y))),
            terrain_fingerprint(cells().map(|(x, y)| city.tile_bits_at(x, y).unwrap().raw_bits())),
            terrain_fingerprint(cells().map(|(x, y)| city.building_at(x, y))),
        ];
        let case = (trees, hills, water, coast, river);
        assert_eq!(
            (planes, (crt.seed(), game.state())),
            (expected_planes, expected_seeds),
            "tree slider native case {case:?}"
        );
    }
}

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
    for x in expected_x_b..CITY_WIDTH {
        assert_eq!(heights[city_index(x, 0)] & 0x1f, 3);
        assert_eq!(xbit[city_index(x, 0)] & 0x01, 0x01);
    }
    assert_eq!(xbit[city_index(expected_x_a, 0)] & 0x01, 0);
}

#[test]
fn win95_river_band_is_nine_tiles_wide_and_moves_west() {
    let mut heights = vec![12u16; CITY_TILES];
    let mut xbit = vec![0u8; CITY_TILES];
    let mut game_rng = Win95GameRand::new(0x9876);
    let initial_center = 64i16;

    apply_win95_mode_bands(&mut heights, &mut xbit, 7, false, true, &mut game_rng);

    for y in (initial_center - 3)..=(initial_center + 3) {
        assert_eq!(heights[city_index(127, y as usize)] & 0x1f, 4);
    }
    for y in [initial_center - 4, initial_center + 4] {
        assert_eq!(heights[city_index(127, y as usize)] & 0x1f, 5);
    }
    assert_eq!(
        heights[city_index(127, (initial_center - 5) as usize)] & 0x1f,
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
    assert_eq!(normalize_mac68k_height(0), 1);
    assert_eq!(normalize_mac68k_height(4), 3);
    assert_eq!(normalize_mac68k_height(5), 4);
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
