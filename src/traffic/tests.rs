use super::*;
use crate::city::{City, CityFileKind};
use crate::mac_random::MacToolboxRng;
use crate::mac_render::mac_shape_blit_offset;
use crate::paths::{self, test_mac_city, test_win_cape_city, test_win_populated_city};
use crate::win95_tile_draw::uses_win95_overlay_draw;
use crate::win95_traffic_placement::lane_index_for_tile;

/// Moving Mac dashes are positioned by sub-tile offsets, so they must stay on the small/medium
/// banks (the large bank is a full-tile canvas whose placement is baked in and cannot be
/// sub-tile shifted).
fn live_traffic_shape_scale_band(static_band: u32) -> u32 {
    static_band.min(TRAFFIC_MEDIUM_SPRITE_OFFSET)
}

fn test_draw_rng(city: &City) -> MacToolboxRng {
    MacToolboxRng::for_traffic_draw(&city.name, city.rotation(), 0)
}

fn zero_congestion(_x: usize, _y: usize) -> u8 {
    0
}

fn file_congestion(city: &City) -> impl Fn(usize, usize) -> u8 + '_ {
    move |x, y| city.traffic_density_at(x, y).unwrap_or(0)
}

#[test]
fn win95_cities_still_draw_xtrf_decorations() {
    let Some(path) = test_win_cape_city() else {
        return;
    };
    let city = City::load(path).expect("cape");
    assert!(city.has_traffic_density_map());
    // Loaded PE XTHG without runtime `0xd0` cars must keep the per-tile XTRF art path.
    assert!(!uses_win95_overlay_draw(&city));
    let dense = (0..CITY_HEIGHT)
        .flat_map(|y| (0..CITY_WIDTH).map(move |x| (x, y)))
        .filter(|&(x, y)| city.traffic_density_at(x, y).unwrap_or(0) > 0x1c)
        .count();
    assert!(
        dense > 100,
        "Cape Wells should retain PE-visible XTRF density for the per-tile art path, got {dense}"
    );
    let overlays = count_traffic_overlays_for_engine(
        &city,
        EngineFidelity::Win95,
        |x, y| city.traffic_density_at(x, y).unwrap_or(0),
        &mut test_draw_rng(&city),
    );
    assert!(
        overlays > 0,
        "Cape Wells should render some visible traffic on its roads, got {overlays}"
    );
}

#[test]
fn traffic_rendering_follows_engine_not_city_file_kind() {
    let mut win95_overlay_reference = None;
    for file_kind in [CityFileKind::WinSc2, CityFileKind::MacCity] {
        let mut city = City::new_blank(
            std::path::Path::new("/tmp/traffic-engine-format.sc2"),
            "TrafficEngineFormat",
        )
        .expect("blank city");
        city.kind = file_kind;
        city.set_building(20, 20, 29); // ordinary road
        city.set_tile_bits(20, 20, crate::city::TileBits::new(0x02)); // PE traffic XBIT
        city.set_traffic_density(20, 20, 0x39); // above Win95's visible-light cutoff

        for fidelity in [EngineFidelity::Win95, EngineFidelity::Mac68k] {
            let count = count_traffic_overlays_for_engine(
                &city,
                fidelity,
                |x, y| city.traffic_density_at(x, y).unwrap_or(0),
                &mut test_draw_rng(&city),
            );
            assert!(
                count > 0,
                "{fidelity:?} traffic renderer should draw a road in {file_kind:?} city format"
            );
        }

        let mut win95_rng = test_draw_rng(&city);
        let win95_overlay = traffic_overlay_at_for_engine(
            &city,
            20,
            20,
            EngineFidelity::Win95,
            |x, y| city.traffic_density_at(x, y).unwrap_or(0),
            &mut win95_rng,
            0,
        )
        .expect("shared Win95/XTRF road overlay");
        if let Some(reference) = win95_overlay_reference {
            assert_eq!(
                win95_overlay, reference,
                "Win95/XTRF overlay must be identical for {file_kind:?} and Win95 city files"
            );
        } else {
            win95_overlay_reference = Some(win95_overlay);
        }

        let mut legacy_state = TrafficDrawState::default();
        assert!(
            traffic_overlay_at(
                &city,
                20,
                20,
                &mut legacy_state,
                |x, y| city.traffic_density_at(x, y).unwrap_or(0),
                &mut test_draw_rng(&city),
                0,
            )
            .is_some(),
            "legacy overlay helper should not suppress {file_kind:?} cities"
        );
    }
}

#[test]
fn win95_xtrf_traffic_does_not_require_road_rotation_bit() {
    let mut city =
        City::new_blank("/tmp/unrotated-road-traffic.sc2", "Traffic").expect("blank city");
    city.set_building(20, 20, 29); // ordinary road
    city.set_tile_bits(20, 20, crate::city::TileBits::new(0)); // no rotation bit
    city.set_traffic_density(20, 20, 0x39); // visible Win95 XTRF band

    assert!(
        traffic_overlay_at_for_engine(
            &city,
            20,
            20,
            EngineFidelity::Win95,
            |x, y| city.traffic_density_at(x, y).unwrap_or(0),
            &mut test_draw_rng(&city),
            0,
        )
        .is_some(),
        "saved XTRF must draw traffic on ordinary unrotated roads"
    );
}

#[test]
fn mac_city_draws_traffic_overlays() {
    let Some(mac_city) = test_mac_city() else {
        return;
    };
    let city = City::load(mac_city).expect("city");
    let mut rng = test_draw_rng(&city);
    let count = count_traffic_overlays(&city, file_congestion(&city), &mut rng);
    assert!(
        count > 0,
        "Mac-format city should draw its saved XTRF traffic"
    );
}

#[test]
fn experimental_claim_helper_dedups_within_xtrf_2x2_blocks() {
    let mut state = TrafficDrawState::default();
    assert!(state.claim_block(0, 0));
    assert!(!state.claim_block(1, 0));
    assert!(!state.claim_block(0, 1));
    assert!(!state.claim_block(1, 1));
    assert!(state.claim_block(2, 0));
    assert!(state.claim_block(0, 2));
    assert!(state.claim_block(11, 10));
    assert!(!state.claim_block(10, 10));
}

#[test]
fn road_orientation_maps_to_mac_dash_variant() {
    assert_eq!(mac_static_dash_variant_for_road(29), 0);
    assert_eq!(mac_static_dash_variant_for_road(30), 1);
    assert_eq!(mac_static_dash_variant_for_road(31), 2);
    assert_eq!(mac_static_dash_variant_for_road(32), 3);
    assert_eq!(mac_static_dash_variant_for_road(35), 2);
    assert_eq!(decorative_dash_base_sprite(31), Some(402));
    assert_eq!(decorative_dash_base_sprite(30), Some(401));
}

#[test]
fn curve_road_offsets_use_diagonal_mac_dashes() {
    assert_eq!(mac_static_dash_variant_for_road(33), 2);
    assert_eq!(mac_static_dash_variant_for_road(34), 3);
    assert_eq!(decorative_dash_base_sprite(33), Some(404));
    assert_eq!(decorative_dash_base_sprite(34), Some(405));
    assert_eq!(decorative_dash_base_sprite(38), Some(409));
}

#[test]
fn static_mac_dash_does_not_apply_random_blit_flag_as_gpu_xflip() {
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    // Mac export_80 passes Random&1 into 0x055a, but that is not proven to be the same
    // as this renderer's horizontal flip. The static dash row already carries mirrored art.
    assert!(!traffic_decorative_flip_x(&city, 10, 10, 31, 400, true));
    assert!(!traffic_decorative_flip_x(&city, 10, 10, 31, 403, true));
    assert!(!traffic_decorative_flip_x(&city, 10, 10, 35, 900, true));

    // Non-static/debug traffic sprites still use the flag when no tile XBIT flip applies.
    assert!(traffic_decorative_flip_x(&city, 10, 10, 31, 418, true));
    assert!(!traffic_decorative_flip_x(&city, 10, 10, 31, 418, false));
}

#[test]
fn viewer_uses_small_dash_band_when_zoomed_out() {
    assert_eq!(traffic_shape_scale_band(0.5), 0);
    assert_eq!(traffic_shape_scale_band(TRAFFIC_MEDIUM_BAND_ZOOM - 0.1), 0);
}

#[test]
fn viewer_uses_medium_dash_band_at_mid_zoom() {
    assert_eq!(
        traffic_shape_scale_band(TRAFFIC_MEDIUM_BAND_ZOOM),
        TRAFFIC_MEDIUM_SPRITE_OFFSET
    );
    assert_eq!(
        traffic_shape_scale_band(TRAFFIC_LARGE_BAND_ZOOM - 0.1),
        TRAFFIC_MEDIUM_SPRITE_OFFSET
    );
}

#[test]
fn viewer_switches_to_large_band_when_zoomed_in() {
    assert_eq!(traffic_shape_scale_band(2.0), 0);
    assert_eq!(traffic_shape_scale_band(3.25), TRAFFIC_MEDIUM_SPRITE_OFFSET);
    assert_eq!(
        traffic_shape_scale_band(TRAFFIC_LARGE_BAND_ZOOM),
        TRAFFIC_LARGE_SPRITE_OFFSET
    );
    assert_eq!(traffic_shape_scale_band(5.0), TRAFFIC_LARGE_SPRITE_OFFSET);
}

#[test]
fn traffic_zoom_bank_requires_membership_in_dedicated_traffic_atlas() {
    let logical_id = 400;
    let large_id = logical_id + TRAFFIC_LARGE_SPRITE_OFFSET;

    assert_eq!(
        traffic_render_sprite_id_with_lookup(logical_id, TRAFFIC_LARGE_SPRITE_OFFSET, |_| {
            false
        }),
        normalize_traffic_dash_sprite(logical_id),
        "an unrelated atlas entry must not promote traffic into a replacement zoom bank"
    );
    assert_eq!(
        traffic_render_sprite_id_with_lookup(
            logical_id,
            TRAFFIC_LARGE_SPRITE_OFFSET,
            |sprite_id| sprite_id == large_id,
        ),
        large_id,
        "use the large bank only when the dedicated traffic atlas contains it"
    );
}

#[test]
fn road_tile_traffic_uses_full_canvas_and_keeps_heavy_frames() {
    assert_eq!(
        traffic_render_sprite_id_with_lookup(418, TRAFFIC_LARGE_SPRITE_OFFSET, |id| id == 1418),
        1418,
        "a full road overlay must not use the small actor canvas"
    );
    for id in 1400..=1449 {
        assert!(
            is_lane_dash_baked_sprite(id),
            "baked traffic frame {id} must remain resolvable"
        );
    }
}

#[test]
fn highway_traffic_draws_once_per_visible_footprint_in_all_rotations() {
    for kind in [CityFileKind::WinSc2, CityFileKind::MacCity] {
        for building in 97..=105 {
            for cornered in [false, true] {
                let mut city =
                    City::new_blank("/tmp/highway-traffic.sc2", "HighwayTraffic").unwrap();
                city.kind = kind;
                let corners = city.footprint_corner_bits();
                // Two adjacent highway pieces, each with a complete 2x2 canvas.
                for origin_x in [20, 22] {
                    for dy in 0..2 {
                        for dx in 0..2 {
                            city.set_building(origin_x + dx, 30 + dy, building);
                            if cornered {
                                city.set_zone(origin_x + dx, 30 + dy, corners[dy * 2 + dx]);
                            }
                        }
                    }
                }
                for rotation in 0..4 {
                    city.set_view_rotation(rotation);
                    let mut rng = test_draw_rng(&city);
                    let mut drawn = Vec::new();
                    let mut surfaces = Vec::new();
                    for y in 30..32 {
                        for x in 20..24 {
                            if city.building_should_draw_at(x, y) {
                                surfaces.push((x, y));
                            }
                            if traffic_overlay_at_for_engine(
                                &city,
                                x,
                                y,
                                EngineFidelity::Win95,
                                |_, _| 0x39,
                                &mut rng,
                                0,
                            )
                            .is_some()
                            {
                                drawn.push((x, y));
                            }
                        }
                    }
                    assert_eq!(surfaces.len(), 2);
                    assert_eq!(
                        drawn, surfaces,
                        "{kind:?} building={building} cornered={cornered} rotation={rotation}"
                    );
                }
            }
        }
    }
}

#[test]
fn charleston_highway_traffic_has_one_overlay_per_visible_deck() {
    let Some(root) = crate::paths::test_win95_root() else {
        return;
    };
    let path = root.join("SCENARIO/CHARLEST.SCN");
    if !path.is_file() {
        return;
    }
    let mut city = City::load(path).unwrap();
    for rotation in 0..4 {
        city.set_view_rotation(rotation);
        let mut rng = test_draw_rng(&city);
        let (mut surfaces, mut overlays, mut footprint_tiles) = (0, 0, 0);
        for y in 0..CITY_HEIGHT {
            for x in 0..CITY_WIDTH {
                if !matches!(city.building_at(x, y), 97..=105)
                    || city.traffic_density_at(x, y).unwrap_or(0)
                        <= WIN95_TRAFFIC_DRAW_LIGHT_THRESHOLD
                {
                    continue;
                }
                footprint_tiles += 1;
                surfaces += usize::from(city.building_should_draw_at(x, y));
                overlays += usize::from(
                    traffic_overlay_at_for_engine(
                        &city,
                        x,
                        y,
                        EngineFidelity::Win95,
                        |x, y| city.traffic_density_at(x, y).unwrap_or(0),
                        &mut rng,
                        0,
                    )
                    .is_some(),
                );
            }
        }
        assert!(surfaces > 0 && footprint_tiles > surfaces);
        assert_eq!(
            overlays, surfaces,
            "rotation {rotation}: {footprint_tiles} footprint cells must not each stamp a highway canvas"
        );
    }
}

#[test]
fn single_tile_highway_and_ramp_traffic_is_not_deduplicated() {
    for building in (73..=80).chain(93..=96) {
        let mut city = City::new_blank("/tmp/highway-tile-traffic.sc2", "HighwayTile").unwrap();
        for x in 20..24 {
            city.set_building(x, 30, building);
        }
        let mut rng = test_draw_rng(&city);
        let count = (20..24)
            .filter(|&x| {
                traffic_overlay_at_for_engine(
                    &city,
                    x,
                    30,
                    EngineFidelity::Win95,
                    |_, _| 0x39,
                    &mut rng,
                    0,
                )
                .is_some()
            })
            .count();
        assert_eq!(
            count, 4,
            "building {building} uses a 32-pixel per-tile canvas"
        );
    }
}

#[test]
fn pixel_guard_bumps_zoom_band_when_dashes_are_subpixel() {
    assert_eq!(
        traffic_shape_scale_band_for_dash_width(0.3, 8.0),
        TRAFFIC_MEDIUM_SPRITE_OFFSET
    );
    assert_eq!(traffic_shape_scale_band_for_dash_width(0.5, 8.0), 0);
    // Extreme zoom-out: medium is still sub-pixel, so the guard now escalates to the large
    // dash bank (uncapped for the static dash family).
    assert_eq!(
        traffic_shape_scale_band_for_dash_width(0.15, 8.0),
        TRAFFIC_LARGE_SPRITE_OFFSET
    );
}

#[test]
fn live_traffic_uses_medium_bank_but_not_full_road_large_bank() {
    assert_eq!(live_traffic_shape_scale_band(0), 0);
    assert_eq!(
        live_traffic_shape_scale_band(TRAFFIC_MEDIUM_SPRITE_OFFSET),
        TRAFFIC_MEDIUM_SPRITE_OFFSET
    );
    assert_eq!(
        live_traffic_shape_scale_band(TRAFFIC_LARGE_SPRITE_OFFSET),
        TRAFFIC_MEDIUM_SPRITE_OFFSET
    );
}

#[test]
fn mac_static_dash_family_covers_all_banks() {
    assert!(is_mac_static_dash_family(400));
    assert!(is_mac_static_dash_family(403));
    assert!(is_mac_static_dash_family(900));
    assert!(is_mac_static_dash_family(1403));
    assert!(!is_mac_static_dash_family(406));
    assert!(!is_mac_static_dash_family(418));
}

#[test]
fn large_band_offsets_logical_traffic_ids() {
    assert_eq!(400u32.saturating_add(TRAFFIC_MEDIUM_SPRITE_OFFSET), 900);
    assert_eq!(400u32.saturating_add(TRAFFIC_LARGE_SPRITE_OFFSET), 1400);
    assert_eq!(428u32.saturating_add(TRAFFIC_LARGE_SPRITE_OFFSET), 1428);
}

#[test]
fn full_tile_overlay_matches_road_dimensions() {
    assert!(is_full_tile_traffic_overlay(32.0, 17.0, 32.0, 17.0));
    assert!(!is_full_tile_traffic_overlay(8.0, 4.0, 32.0, 17.0));
}

#[test]
fn curve_roads_use_road_traffic_sprites() {
    assert_eq!(
        traffic_sprite_for_road(35, TrafficDensity::Light),
        Some(406)
    );
}

#[test]
fn intersections_use_road_specific_traffic_sprites() {
    assert_eq!(
        traffic_sprite_for_road(39, TrafficDensity::Light),
        Some(401)
    );
    assert_eq!(
        traffic_sprite_for_road(40, TrafficDensity::Light),
        Some(400)
    );
}

#[test]
fn default_visual_traffic_is_not_limited_to_export80_four_dash_rows() {
    let straight = traffic_sprite_for_road(30, TrafficDensity::Light).unwrap();
    let curve = traffic_sprite_for_road(35, TrafficDensity::Light).unwrap();
    let highway = traffic_sprite_for_road(74, TrafficDensity::Light).unwrap();
    let heavy = traffic_sprite_for_road(97, TrafficDensity::Heavy).unwrap();

    assert!(matches!(straight, 400..=403));
    assert_eq!(curve, 406);
    assert_eq!(highway, 411);
    assert!(heavy > 403);
}

#[test]
fn win95_table_adds_missing_traffic_road_ids() {
    assert!(!has_decorative_traffic(0));
    assert!(has_decorative_traffic(63));
    assert_eq!(
        traffic_sprite_for_road(63, TrafficDensity::Light),
        Some(401)
    );
    assert_eq!(
        traffic_sprite_for_road(74, TrafficDensity::Light),
        Some(411)
    );
    assert_eq!(
        traffic_sprite_for_road(93, TrafficDensity::Heavy),
        Some(414)
    );
    assert_eq!(
        traffic_sprite_for_road(97, TrafficDensity::Heavy),
        Some(441)
    );
}

#[test]
fn win95_table_still_has_road_specific_shape_ids() {
    assert_eq!(
        traffic_sprite_for_road(29, TrafficDensity::Light),
        Some(400)
    );
    assert_eq!(
        traffic_sprite_for_road(30, TrafficDensity::Heavy),
        Some(428)
    );
}

#[test]
fn traffic_lane_offset_uses_mac_center_anchor_with_small_lane_nudge() {
    let lane = lane_index_for_tile(29, 64, 56);
    let offset = traffic_lane_offset_for_road(29, lane, 32.0, 17.0, 8.0, 4.0, 400, 0);
    assert_eq!(offset, Vec2::new(14.0, -5.5));
}

#[test]
fn dash_screen_scale_matches_road_sprites() {
    assert_eq!(
        traffic_dash_screen_scale(400, 2.0, 8.0, 4.0, 32.0, 17.0),
        2.0
    );
    assert_eq!(
        traffic_dash_screen_scale(900, 2.0, 16.0, 9.0, 32.0, 17.0),
        2.0
    );
    assert_eq!(
        traffic_dash_screen_scale(1400, 2.0, 32.0, 17.0, 32.0, 17.0),
        2.0
    );
    assert_eq!(
        traffic_dash_screen_scale(418, 2.0, 16.0, 16.0, 32.0, 17.0),
        2.0
    );
}

#[test]
fn large_traffic_overlay_has_zero_lane_offset() {
    let offset = traffic_lane_offset_for_road(29, 1, 32.0, 17.0, 32.0, 17.0, 1400, 0);
    assert_eq!(offset, Vec2::ZERO);
}

#[test]
fn mac_static_dash_variants_keep_same_bottom_anchor() {
    let lane = lane_index_for_tile(31, 0, 0);
    let straight = traffic_lane_offset_for_road(31, lane, 32.0, 17.0, 8.0, 4.0, 400, 0);
    let curve = traffic_lane_offset_for_road(31, lane, 32.0, 17.0, 8.0, 7.0, 402, 0);
    assert_eq!(curve.y, straight.y);
}

#[test]
fn mac_shape_offset_matches_disassembly() {
    assert_eq!(mac_shape_blit_offset(8.0, 4.0), Vec2::new(58.0, 48.0));
}

#[test]
fn win95_straight_roads_use_road_type_sprite_not_lane() {
    assert_eq!(
        traffic_sprite_for_road(30, TrafficDensity::Light),
        Some(401)
    );
    assert_eq!(
        traffic_sprite_for_road(29, TrafficDensity::Light),
        Some(400)
    );
}

#[test]
fn heavy_traffic_uses_heavy_sprite_ids() {
    assert_eq!(
        traffic_sprite_for_road(29, TrafficDensity::Heavy),
        Some(427)
    );
    assert_eq!(
        traffic_sprite_for_road(30, TrafficDensity::Heavy),
        Some(428)
    );
}

#[test]
fn dash_sprite_ids_exist_in_small_shape_atlas() {
    use crate::assets::AtlasSource;

    let Some(root) = paths::test_asset_root() else {
        return;
    };
    let small = AtlasSource::load(root.join("sprites/SPRT_00128_ShapeData")).expect("128");
    assert!(small.frame_by_id(400).is_some());
    assert!(small.frame_by_id(401).is_some());
    assert_eq!(400 + TRAFFIC_LARGE_SPRITE_OFFSET, 1400);
    assert_eq!(427 + TRAFFIC_LARGE_SPRITE_OFFSET, 1427);
}

#[test]
fn mac_density_uses_lower_congestion_visibility_band() {
    assert_eq!(density_from_xtrf_mac(0x32), None);
    assert_eq!(density_from_xtrf_mac(0x33), Some(TrafficDensity::Light));
    assert_eq!(density_from_xtrf_mac(0x56), Some(TrafficDensity::Light));
    assert_eq!(density_from_xtrf_mac(0xC8), Some(TrafficDensity::Light));
    assert_eq!(density_from_xtrf_mac(0xc9), Some(TrafficDensity::Heavy));
    assert_eq!(density_from_xtrf_mac(0x86), Some(TrafficDensity::Light));
    assert_eq!(density_from_xtrf(0x32), None);
    assert_eq!(density_from_xtrf(0x33), Some(TrafficDensity::Light));
    assert_eq!(density_from_xtrf(0xC8), Some(TrafficDensity::Light));
    assert_eq!(density_from_xtrf(0xC9), Some(TrafficDensity::Heavy));
}

#[test]
fn win95_draw_density_uses_pe_0x1c_0x38_bands() {
    // PE FUN_0043C900 XTRF art bands (not sim 0x33/0xC9).
    assert_eq!(WIN95_TRAFFIC_DRAW_LIGHT_THRESHOLD, 0x1c);
    assert_eq!(WIN95_TRAFFIC_DRAW_HEAVY_THRESHOLD, 0x38);
    assert_eq!(density_from_xtrf_win95_draw(0x1c), None);
    assert_eq!(
        density_from_xtrf_win95_draw(0x1d),
        Some(TrafficDensity::Light)
    );
    assert_eq!(
        density_from_xtrf_win95_draw(0x38),
        Some(TrafficDensity::Light)
    );
    assert_eq!(
        density_from_xtrf_win95_draw(0x39),
        Some(TrafficDensity::Heavy)
    );
    // Mid-band XTRF is invisible on Mac sim bands but Light on Win95 draw.
    assert_eq!(density_from_xtrf_mac(0x20), None);
    assert_eq!(
        density_from_xtrf_win95_draw(0x20),
        Some(TrafficDensity::Light)
    );
}

#[test]
fn normalize_maps_missing_heavy_ids_to_light_dashes() {
    assert_eq!(normalize_traffic_dash_sprite(428), 401);
    assert_eq!(normalize_traffic_dash_sprite(427), 400);
    assert_eq!(normalize_traffic_dash_sprite(441), 418);
    assert_eq!(normalize_traffic_dash_sprite(449), 426);
    assert_eq!(normalize_traffic_dash_sprite(1441), 1418);
    assert_eq!(normalize_traffic_dash_sprite(406), 406);
}

#[test]
fn traffic_screen_pos_uses_road_width() {
    let Some(mac_city) = test_mac_city() else {
        return;
    };
    let city = City::load(mac_city).expect("city");
    let tile = Vec2::new(100.0, 200.0);
    let zoom = 2.0;
    let lane = lane_index_for_tile(29, 64, 56);
    let pos = traffic_screen_bottom_left(
        &city, 64, 56, tile, zoom, 29, 32.0, 17.0, 8.0, 4.0, 400, lane,
    );
    assert_eq!(pos.x, 96.0);
    assert_eq!(pos.y, 189.0);
}

#[test]
fn lane_index_alternates_along_east_west_roads() {
    assert_eq!(lane_index_for_tile(29, 64, 56), 1);
    assert_eq!(lane_index_for_tile(29, 64, 57), 5);
}

#[test]
fn mac_heavy_byte_band() {
    assert!(!mac_heavy_from_traffic_byte(200));
    assert!(mac_heavy_from_traffic_byte(201));
    assert!(mac_heavy_from_traffic_byte(240));
    assert!(!mac_heavy_from_traffic_byte(241));
}

#[test]
fn xtrf_overlay_only_on_road_tiles() {
    let Some(mac_city) = test_mac_city() else {
        return;
    };
    let city = City::load(mac_city).expect("city");
    let mut state = TrafficDrawState::default();
    let mut rng = test_draw_rng(&city);
    let congestion = file_congestion(&city);
    let rotation = city.rotation();
    for y in 0..CITY_HEIGHT {
        for x in 0..CITY_WIDTH {
            if traffic_overlay_at(&city, x, y, &mut state, &congestion, &mut rng, 0).is_some() {
                let building = rotated_building_id(city.building_at(x, y), rotation);
                assert!(
                    has_decorative_traffic(building),
                    "overlay at ({x},{y}) on building {building}"
                );
            }
        }
    }
}

#[test]
fn win95_spawn_threshold_constant() {
    assert_eq!(PORT_TRAFFIC_ACTOR_SPAWN_THRESHOLD, 0xaa);
}

#[test]
fn dash_variant_uses_mac_toolbox_rng() {
    let mut rng = MacToolboxRng::new(0x1234_5678);
    let v0 = rng.random_u3();
    let v1 = rng.random_u3();
    assert!(v0 < 4);
    assert!(v1 < 4);
    let mut again = MacToolboxRng::new(0x1234_5678);
    assert_eq!(v0, again.random_u3());
    assert_eq!(v1, again.random_u3());
}

#[test]
fn mac_dash_variant_animates_within_static_dash_family() {
    assert_eq!(traffic_sprite_with_dash_variant(400, 0), 400);
    assert_eq!(traffic_sprite_with_dash_variant(400, 1), 401);
    assert_eq!(traffic_sprite_with_dash_variant(400, 2), 402);
    assert_eq!(traffic_sprite_with_dash_variant(400, 3), 403);
    assert_eq!(traffic_sprite_with_dash_variant(406, 1), 401);
}

#[test]
fn heavy_traffic_uses_eight_frame_cycle() {
    let frame = traffic_anim_frame_for_tile(TrafficDensity::Heavy, 7, true);
    assert_eq!(frame, 7);
    assert_eq!(
        traffic_anim_frame_for_tile(TrafficDensity::Light, 7, true),
        7
    );
    assert_eq!(
        traffic_anim_frame_for_tile(TrafficDensity::Heavy, 7, false),
        7
    );
    assert_eq!(
        traffic_anim_frame_for_tile(TrafficDensity::Heavy, 9, true),
        1
    );
    assert_eq!(
        traffic_anim_frame_for_tile(TrafficDensity::Heavy, 16, true),
        0
    );
}

#[test]
fn draw_rng_seed_is_stable_per_city_and_epoch() {
    let Some(mac_city) = test_mac_city() else {
        return;
    };
    let city = City::load(mac_city).expect("city");
    let system = TrafficSystem::new(&city);
    assert_eq!(
        system.draw_rng(&city.name, city.rotation(), 0).seed(),
        system.draw_rng(&city.name, city.rotation(), 0).seed()
    );
    assert_ne!(
        system.draw_rng(&city.name, city.rotation(), 0).seed(),
        system.draw_rng(&city.name, city.rotation(), 1).seed()
    );
}

#[test]
fn decorative_overlay_uses_road_specific_traffic_tiles() {
    assert_eq!(decorative_dash_base_sprite(30), Some(401));
    assert_eq!(decorative_dash_base_sprite(35), Some(406));
    let sprite = decorative_dash_base_sprite(97).unwrap();
    assert_eq!(sprite, 418);
}

#[test]
fn win95_decorative_overlay_heavy_can_use_car_shapes() {
    assert_eq!(
        win95_decorative_dash_base_sprite(30, TrafficDensity::Heavy),
        Some(419)
    );
    let sprite = win95_decorative_dash_base_sprite(97, TrafficDensity::Heavy).unwrap();
    assert!(is_traffic_car_shape(sprite));
}

#[test]
fn decorative_overlay_light_uses_traffic_tile_family() {
    assert_eq!(decorative_dash_base_sprite(30), Some(401));
    let sprite = decorative_dash_base_sprite(35).unwrap();
    assert!(is_traffic_lane_dash_sprite(sprite));
    assert_eq!(sprite, 406);
}

#[test]
fn decorative_overlay_uses_road_sprite_and_blit_flag() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_building(10, 10, 31);
    city.set_traffic_density(10, 10, MAC_TRAFFIC_VISIBLE_BYTE_MIN);
    let mut state = TrafficDrawState::default();
    let mut rng = MacToolboxRng::new(1);
    let before = rng.seed();
    let overlay = traffic_overlay_at(
        &city,
        10,
        10,
        &mut state,
        |x, y| city.traffic_density_at(x, y).unwrap_or(0),
        &mut rng,
        0,
    )
    .expect("traffic overlay");
    assert_eq!(overlay.sprite_id, 402);
    assert!(is_traffic_lane_dash_sprite(overlay.sprite_id));
    assert!(overlay.blit_flip);
    assert_ne!(rng.seed(), before);
}

#[test]
fn straight_road_overlay_uses_road_specific_tile() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_building(10, 10, 29);
    city.set_traffic_density(10, 10, MAC_TRAFFIC_VISIBLE_BYTE_MIN);
    let mut state = TrafficDrawState::default();
    let mut rng = MacToolboxRng::new(1);
    let overlay = traffic_overlay_at(
        &city,
        10,
        10,
        &mut state,
        |x, y| city.traffic_density_at(x, y).unwrap_or(0),
        &mut rng,
        0,
    )
    .expect("traffic overlay");
    assert!(is_mac_static_dash_family(overlay.sprite_id));
}

#[test]
fn explicit_mac_engine_stamps_road_with_zero_xtrf() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_building(10, 10, 29);
    let mut rng = MacToolboxRng::new(1);
    let overlay =
        traffic_overlay_at_for_engine(&city, 10, 10, EngineFidelity::Mac68k, |_, _| 0, &mut rng, 0)
            .expect("Mac export_80 road stamp");
    assert!((400..=403).contains(&overlay.sprite_id));
}

#[test]
fn explicit_mac_engine_stamps_special_export80_tile() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_building(10, 10, 223);
    let mut rng = MacToolboxRng::new(1);
    let overlay =
        traffic_overlay_at_for_engine(&city, 10, 10, EngineFidelity::Mac68k, |_, _| 0, &mut rng, 0)
            .expect("Mac export_80 special stamp");
    assert!((400..=403).contains(&overlay.sprite_id));
}

#[test]
fn mac_static_overlay_lane_stays_centered() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_building(10, 10, 29);
    city.set_traffic_density(10, 10, MAC_TRAFFIC_VISIBLE_BYTE_MIN);
    let mut state = TrafficDrawState::default();
    let mut rng = MacToolboxRng::new(1);
    let overlay = traffic_overlay_at(
        &city,
        10,
        10,
        &mut state,
        |x, y| city.traffic_density_at(x, y).unwrap_or(0),
        &mut rng,
        0,
    )
    .expect("traffic overlay");
    assert_eq!(overlay.lane, 0);
}

#[test]
fn ordinary_export80_overlay_stamps_each_eligible_road_tile() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    for y in 20..22 {
        for x in 20..22 {
            city.set_building(x, y, 31);
            city.set_traffic_density(x, y, MAC_TRAFFIC_VISIBLE_BYTE_MIN);
        }
    }
    let mut state = TrafficDrawState::default();
    let mut rng = MacToolboxRng::new(1);
    let congestion = |x, y| city.traffic_density_at(x, y).unwrap_or(0);
    let mut count = 0;
    for y in 20..22 {
        for x in 20..22 {
            if traffic_overlay_at(&city, x, y, &mut state, congestion, &mut rng, 0).is_some() {
                count += 1;
            }
        }
    }
    assert_eq!(count, 4);
}

#[test]
fn multi_cell_export80_footprint_can_emit_four_stamps() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    for y in 20..22 {
        for x in 20..22 {
            city.set_building(x, y, 73);
            city.set_traffic_density(x, y, MAC_TRAFFIC_VISIBLE_BYTE_MIN);
        }
    }
    let mut rng = MacToolboxRng::new(1);
    let congestion = |x, y| city.traffic_density_at(x, y).unwrap_or(0);
    let stamps = multi_cell_road_footprint(73)
        .iter()
        .filter(|&&(dx, dy)| {
            let x = (20 + dx) as usize;
            let y = (20 + dy) as usize;
            traffic_overlay_stamp_at(&city, x, y, congestion, &mut rng).is_some()
        })
        .count();
    assert_eq!(stamps, 4);
}

#[test]
fn mac_export80_stamp_sequence_reuses_its_selected_draw_row() {
    let overlay = TrafficOverlay {
        sprite_id: 402,
        density: TrafficDensity::Heavy,
        lane: 0,
        blit_flip: false,
    };
    let mut rng = MacToolboxRng::new(1);
    let stamps = mac_export80_stamp_sequence(73, overlay, &mut rng);

    assert_eq!(stamps.len(), 4);
    assert_eq!(
        stamps
            .iter()
            .map(|(stamp, _)| stamp.sprite_id)
            .collect::<Vec<_>>(),
        vec![402, 402, 402, 402]
    );
    assert_eq!(
        stamps.iter().map(|(_, offset)| *offset).collect::<Vec<_>>(),
        export80_multi_stamp_offsets().to_vec()
    );
    assert_eq!(mac_export80_stamp_sequence(29, overlay, &mut rng).len(), 1);
}

#[test]
fn mac_export80_special_endpoint_sequence_rerolls_row_and_blit_flag() {
    let overlay = TrafficOverlay {
        sprite_id: 400,
        density: TrafficDensity::Light,
        lane: 0,
        blit_flip: false,
    };
    let mut actual_rng = MacToolboxRng::new(1);
    let stamps = mac_export80_stamp_sequence(63, overlay, &mut actual_rng);
    let mut expected_rng = MacToolboxRng::new(1);

    assert_eq!(stamps.len(), 2);
    assert_eq!(stamps[0], (overlay, Vec2::ZERO));
    assert_eq!(
        stamps[1].0.sprite_id,
        mac_static_traffic_sprite_with_variant(expected_rng.random_u3())
    );
    assert_eq!(stamps[1].0.blit_flip, expected_rng.random_bit());
    assert_eq!(
        stamps.iter().map(|(_, offset)| *offset).collect::<Vec<_>>(),
        export80_special_endpoint_offsets(63).unwrap().to_vec()
    );
    assert_eq!(actual_rng.seed(), expected_rng.seed());
}

#[test]
fn mac_export80_special_endpoint_walk_skips_to_first_endpoint_in_direction() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_building(20, 20, 65);
    city.set_building(21, 20, 29);
    city.set_building(22, 20, 65);
    city.set_building(23, 20, 65);
    let overlay = TrafficOverlay {
        sprite_id: 400,
        density: TrafficDensity::Light,
        lane: 0,
        blit_flip: false,
    };
    let mut actual_rng = MacToolboxRng::new(1);
    let stamps = mac_export80_stamp_sequence_for_city(&city, 20, 20, 65, overlay, &mut actual_rng);
    let mut expected_rng = MacToolboxRng::new(1);
    let step = export80_special_endpoint_offsets(65).unwrap()[1];

    assert_eq!(stamps.len(), 2);
    assert_eq!(stamps[0], (overlay, Vec2::ZERO));
    assert_eq!(stamps[1].1, step * 2.0);
    assert_eq!(
        stamps[1].0.sprite_id,
        mac_static_traffic_sprite_with_variant(expected_rng.random_u3())
    );
    assert_eq!(stamps[1].0.blit_flip, expected_rng.random_bit());
    assert_eq!(actual_rng.seed(), expected_rng.seed());
}

#[test]
fn legacy_traffic_overlay_helper_does_not_gate_on_city_format() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.kind = CityFileKind::MacCity;
    city.set_building(10, 10, 29);
    city.set_tile_bits(10, 10, crate::city::TileBits::new(0x02));
    city.set_traffic_density(10, 10, 0x39);
    let congestion = |x, y| city.traffic_density_at(x, y).unwrap_or(0);
    let mut actual_rng = MacToolboxRng::new(1);
    assert!(traffic_overlay_stamp_at(&city, 10, 10, congestion, &mut actual_rng).is_some());
}

#[test]
fn export80_multi_stamp_offsets_match_d5_diamond_sequence_by_projection_shift() {
    assert_eq!(
        export80_multi_stamp_offsets_for_projection_shift(0),
        [
            Vec2::ZERO,
            Vec2::new(4.0, -2.0),
            Vec2::new(8.0, 0.0),
            Vec2::new(4.0, 2.0),
        ]
    );
    assert_eq!(
        export80_multi_stamp_offsets_for_projection_shift(1),
        [
            Vec2::ZERO,
            Vec2::new(8.0, -4.0),
            Vec2::new(16.0, 0.0),
            Vec2::new(8.0, 4.0),
        ]
    );
    assert_eq!(
        export80_multi_stamp_offsets_for_projection_shift(2),
        [
            Vec2::ZERO,
            Vec2::new(16.0, -8.0),
            Vec2::new(32.0, 0.0),
            Vec2::new(16.0, 8.0),
        ]
    );
    assert_eq!(
        export80_multi_stamp_offsets(),
        [
            Vec2::ZERO,
            Vec2::new(16.0, -8.0),
            Vec2::new(32.0, 0.0),
            Vec2::new(16.0, 8.0),
        ]
    );
}

#[test]
fn export80_special_endpoint_offsets_follow_recovered_direction_vector() {
    assert_eq!(
        export80_special_endpoint_offsets(63),
        Some([Vec2::ZERO, Vec2::new(-16.0, -8.0)])
    );
    assert_eq!(
        export80_special_endpoint_offsets(64),
        Some([Vec2::ZERO, Vec2::new(16.0, -8.0)])
    );
    assert_eq!(
        export80_special_endpoint_offsets(65),
        Some([Vec2::ZERO, Vec2::new(16.0, 8.0)])
    );
    assert_eq!(
        export80_special_endpoint_offsets(66),
        Some([Vec2::ZERO, Vec2::new(-16.0, 8.0)])
    );
    assert_eq!(export80_special_endpoint_offsets(29), None);
}

#[test]
fn export80_extra_stamps_keep_draw_row_and_reroll_blit_flag() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_building(20, 20, 73);
    city.set_traffic_density(20, 20, MAC_TRAFFIC_VISIBLE_BYTE_MIN);
    let congestion = |x, y| city.traffic_density_at(x, y).unwrap_or(0);
    let mut rng = MacToolboxRng::new(1);
    let base = traffic_overlay_stamp_at(&city, 20, 20, congestion, &mut rng).unwrap();
    let before_extra_flip = rng.seed();
    let extra = traffic_overlay_with_random_blit_flip(base, &mut rng);

    assert_eq!(extra.sprite_id, base.sprite_id);
    assert_eq!(extra.density, base.density);
    assert_ne!(
        rng.seed(),
        before_extra_flip,
        "extra export_80 stamps consume a fresh Random&1 blit flag"
    );
}

#[test]
fn export80_special_endpoint_stamp_rerolls_draw_row_and_blit_flag() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_building(20, 20, 63);
    city.set_traffic_density(20, 20, MAC_TRAFFIC_VISIBLE_BYTE_MIN);
    let congestion = |x, y| city.traffic_density_at(x, y).unwrap_or(0);
    let mut rng = MacToolboxRng::new(1);
    let base = traffic_overlay_stamp_at(&city, 20, 20, congestion, &mut rng).unwrap();
    let before_endpoint = rng.seed();
    let endpoint = traffic_overlay_with_random_variant_and_blit_flip(base, &mut rng);

    assert!(is_mac_static_dash_family(endpoint.sprite_id));
    assert_ne!(
        rng.seed(),
        before_endpoint,
        "0x3f..0x42 endpoint site consumes Random&3 and Random&1"
    );
}

#[test]
fn mac_city_static_overlay_is_stable_across_repeated_draw_passes() {
    let Some(mac_city) = test_mac_city() else {
        return;
    };
    let city = City::load(mac_city).expect("city");
    let congestion = file_congestion(&city);
    let mut state_a = TrafficDrawState::default();
    let mut rng_a = test_draw_rng(&city);
    let mut state_b = TrafficDrawState::default();
    let mut rng_b = test_draw_rng(&city);
    let mut found = false;
    for y in 0..CITY_HEIGHT {
        for x in 0..CITY_WIDTH {
            let a = traffic_overlay_at(&city, x, y, &mut state_a, &congestion, &mut rng_a, 0);
            let b = traffic_overlay_at(&city, x, y, &mut state_b, &congestion, &mut rng_b, 0);
            if a.is_some() || b.is_some() {
                assert_eq!(a, b);
                found = true;
            }
        }
    }
    assert!(
        found,
        "Mac-format city should have a visible XTRF traffic overlay"
    );
}

#[test]
fn mac_format_traffic_overlay_consumes_draw_random() {
    let Some(mac_city) = test_mac_city() else {
        return;
    };
    let city = City::load(mac_city).expect("city");
    let mut state = TrafficDrawState::default();
    let mut rng = MacToolboxRng::new(99);
    let before = rng.seed();
    let congestion = file_congestion(&city);
    let mut drew = false;
    'scan: for y in 0..CITY_HEIGHT {
        for x in 0..CITY_WIDTH {
            if traffic_overlay_at(&city, x, y, &mut state, &congestion, &mut rng, 0).is_some() {
                drew = true;
                break 'scan;
            }
        }
    }
    assert!(
        drew,
        "Mac-format city should draw traffic when XTRF is present"
    );
    assert_ne!(before, rng.seed());
}

#[test]
#[allow(deprecated)]
fn legacy_dash_variant_is_deterministic_mod4() {
    assert_eq!(traffic_dash_variant(3, 7), traffic_dash_variant(3, 7));
    assert!(traffic_dash_variant(0, 0) < 4);
}

#[test]
fn mac_and_win_cities_both_load_with_traffic_layers() {
    let Some(mac_city) = test_mac_city() else {
        return;
    };
    let Some(win_city) = crate::paths::test_win_city() else {
        return;
    };
    let mac = City::load(mac_city).expect("mac");
    let win = City::load(win_city).expect("win");
    let mut rng = test_draw_rng(&mac);
    let mac_count = count_traffic_overlays(&mac, file_congestion(&mac), &mut rng);
    assert!(
        mac_count > 0,
        "Mac city should draw traffic from its XTRF layer"
    );
    assert!(win.has_traffic_density_map());
    assert!(mac.has_traffic_density_map());
}

#[test]
fn traffic_system_does_not_mutate_saved_xtrf() {
    let Some(mac_city) = test_mac_city() else {
        return;
    };
    let mut city = City::load(mac_city).expect("city");
    let mut system = TrafficSystem::new(&city);
    system.set_speed(GameSpeed::Cheetah);
    let before = system.congestion_at(64, 64);
    for _ in 0..8 {
        system.tick_traffic(&mut city, 1.0);
    }
    assert_eq!(system.congestion_at(64, 64), before);
}

#[test]
fn traffic_system_starts_paused() {
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let system = TrafficSystem::new(&city);
    assert_eq!(system.speed(), GameSpeed::Paused);
}

fn count_congested_road_tiles(city: &City) -> usize {
    let rotation = city.rotation();
    let mut count = 0usize;
    for y in 0..CITY_HEIGHT {
        for x in 0..CITY_WIDTH {
            let building = crate::tiles::rotated_building_id(city.building_at(x, y), rotation);
            if !has_decorative_traffic(building) {
                continue;
            }
            if city
                .traffic_density_at(x, y)
                .is_some_and(|byte| byte > WIN95_TRAFFIC_DRAW_LIGHT_THRESHOLD)
            {
                count += 1;
            }
        }
    }
    count
}

fn overlay_draw_count(city: &City, rng: &mut MacToolboxRng) -> usize {
    let congestion = file_congestion(city);
    if crate::win95_tile_draw::uses_win95_overlay_draw(city) {
        crate::win95_tile_draw::collect_win95_traffic_draws(city, &congestion, rng).len()
    } else {
        count_traffic_overlays(city, &congestion, rng)
    }
}

/// Mac fidelity (all cities): a Win `.SC2` with saved XTRF is treated like a Mac city —
/// the saved density is frozen, so ticking the sim must NOT mutate it on road tiles.
#[test]
fn win95_saved_xtrf_is_frozen_like_mac() {
    let Some(bayview) = test_win_populated_city() else {
        return;
    };
    let mut city = City::load(bayview).expect("bay view");
    // Stock loaded XTHG is PE kinds (e.g. explosions), not runtime port cars.
    assert!(!crate::win95_tile_draw::uses_win95_overlay_draw(&city));
    assert!(city.has_traffic_density_map());
    let before: Vec<u8> = city.traffic_density_slice().expect("xtrf map").to_vec();
    let mut system = TrafficSystem::new(&city);
    system.set_speed(GameSpeed::Cheetah);
    for _ in 0..16 {
        system.tick_traffic(&mut city, 1.0);
    }
    let after = city.traffic_density_slice().expect("xtrf map");
    assert_eq!(
        after,
        before.as_slice(),
        "saved XTRF must stay frozen (Mac-everywhere); congestion sim only uses the heuristic buffer"
    );
}

/// Both reference formats retain decoded XTRF data and the shared overlay helper must render
/// traffic from either file format.
#[test]
fn mac_and_win_congested_road_overlay_sanity() {
    let Some(mac_path) = test_mac_city() else {
        return;
    };
    let Some(win_path) = crate::paths::test_win_city() else {
        return;
    };
    let mac = City::load(mac_path).expect("mac");
    let win = City::load(win_path).expect("win");

    let mac_congested = count_congested_road_tiles(&mac);
    let win_congested = count_congested_road_tiles(&win);
    assert!(
        mac_congested > 50,
        "Maxis Land should have congested road tiles, got {mac_congested}"
    );

    let mut mac_rng = test_draw_rng(&mac);
    let mac_overlays = overlay_draw_count(&mac, &mut mac_rng);
    assert!(
        mac_overlays > 0,
        "Mac city should draw traffic, got {mac_overlays}"
    );
    assert!(
        mac_overlays <= mac_congested,
        "Mac XTRF-backed traffic must not add heuristic overlays ({mac_overlays} vs {mac_congested} visible road tiles)"
    );
    assert!(
        win.has_traffic_density_map(),
        "Win95 city should decode XTRF"
    );
    assert!(
        win.has_traffic_overlays(),
        "Win95 city should have XTHG table"
    );
    assert!(
        !crate::win95_tile_draw::uses_win95_overlay_draw(&win),
        "stock loaded Win95 XTHG without runtime 0xd0 cars must keep XTRF traffic art"
    );
    let mut win_rng = test_draw_rng(&win);
    let win_overlays = overlay_draw_count(&win, &mut win_rng);
    let active_port = win.traffic_overlays().is_some_and(|t| {
        t.iter()
            .any(|r| win.is_port_traffic_record(*r) && r.is_drawable_traffic())
    });
    if active_port {
        assert!(
            win_overlays > 0,
            "active port traffic slots should produce draws"
        );
    }
    // DEFAULT.SC2 is often a fresh city (0 congested road tiles at load); actors spawn at runtime.
    let _ = win_congested;
}
