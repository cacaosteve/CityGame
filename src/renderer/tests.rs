use super::geometry::{push_sprite_vertices, to_ndc};
use super::*;

#[test]
fn radioactive_waste_animation_has_four_distinct_frames_and_keeps_alpha() {
    let base = RgbaImage::from_fn(3, 1, |x, _| match x {
        0 => image::Rgba([0, 0, 0, 255]),
        1 => image::Rgba([220, 20, 10, 255]),
        _ => image::Rgba([200, 10, 20, 0]),
    });
    let frames: Vec<_> = (0..4)
        .map(|phase| radioactive_waste_sprite_frame(&base, phase))
        .collect();
    assert_eq!(sprite_animation_frame_count(1005), 4);
    assert_eq!(frames[0].get_pixel(0, 0).0, [0, 0, 0, 255]);
    assert_eq!(frames[0].get_pixel(1, 0).0, [255, 92, 32, 255]);
    assert_eq!(frames[1].get_pixel(1, 0).0, [255, 190, 48, 255]);
    assert_eq!(frames[2].get_pixel(1, 0).0, [174, 226, 64, 255]);
    assert_eq!(frames[3].get_pixel(1, 0).0, [240, 72, 44, 255]);
    assert_eq!(frames[0].get_pixel(2, 0).0[3], 0);
}

#[test]
fn converts_pixels_to_ndc() {
    assert_eq!(to_ndc(0.0, 0.0, 100.0, 100.0), [-1.0, 1.0]);
    assert_eq!(to_ndc(100.0, 100.0, 100.0, 100.0), [1.0, -1.0]);
}

#[test]
fn extracts_sprite_region_from_atlas() {
    let mut atlas = RgbaImage::from_pixel(4, 4, image::Rgba([0, 0, 0, 0]));
    atlas.put_pixel(1, 2, image::Rgba([12, 34, 56, 255]));
    let frame = AtlasFrame {
        file: String::new(),
        index: 0,
        id: 1,
        source_offset: 0,
        x: 1,
        y: 2,
        w: 1,
        h: 1,
        logical_size: None,
        anchor: None,
    };
    let sprite = extract_sprite_region(&atlas, &frame);
    assert_eq!(sprite.get_pixel(0, 0).0, [12, 34, 56, 255]);
}

#[test]
fn replacement_anchor_controls_sprite_origin() {
    let frame = AtlasFrame {
        file: String::new(),
        index: 0,
        id: 1,
        source_offset: 0,
        x: 0,
        y: 0,
        w: 4,
        h: 5,
        logical_size: None,
        anchor: Some([1.0, 2.0]),
    };
    let mut vertices = Vec::new();
    push_sprite_vertices(
        &mut vertices,
        &frame,
        4,
        5,
        Vec2::new(10.0, 10.0),
        2.0,
        false,
        false,
        [1.0; 4],
        100.0,
        100.0,
    );
    assert_eq!(vertices[0].position, to_ndc(8.0, 6.0, 100.0, 100.0));
}

#[test]
fn high_density_sprite_uses_logical_geometry() {
    let frame = AtlasFrame {
        file: String::new(),
        index: 0,
        id: 1,
        source_offset: 0,
        x: 0,
        y: 0,
        w: 16,
        h: 20,
        logical_size: Some([4, 5]),
        anchor: Some([1.0, 2.0]),
    };
    let mut vertices = Vec::new();
    push_sprite_vertices(
        &mut vertices,
        &frame,
        16,
        20,
        Vec2::new(10.0, 10.0),
        2.0,
        false,
        false,
        [1.0; 4],
        100.0,
        100.0,
    );
    assert_eq!(vertices[0].position, to_ndc(8.0, 6.0, 100.0, 100.0));
    assert_eq!(vertices[2].position, to_ndc(16.0, 16.0, 100.0, 100.0));
    assert_eq!(atlas_min_filter([&frame]), wgpu::FilterMode::Linear);
}

#[test]
fn smoke_sprite_body_stays_off_water_blue() {
    use crate::assets::AtlasSource;
    use crate::palette::{is_smoke_plume_pixel, smoke_plume_rows};
    use std::path::Path;

    let Some(asset_root) = crate::paths::test_asset_root() else {
        return;
    };
    let atlas_dir = asset_root.as_path().join("sprites/SPRT_00129_ShapeData");
    if !atlas_dir.is_dir() {
        return;
    }

    let source = AtlasSource::load(&atlas_dir).expect("load atlas");
    let palette = AnimatedPalette::load(&asset_root).expect("palette");
    let mut rgba = image::open(&source.image_path)
        .expect("open atlas png")
        .to_rgba8();
    let frame = source
        .manifest
        .frames
        .iter()
        .find(|frame| frame.id == 1201)
        .expect("smoke sprite");
    let source_sprite = extract_sprite_region(&rgba, frame);
    normalize_smoke_building_sprites(&mut rgba, &source.manifest.frames);
    let before = extract_sprite_region(&rgba, frame);
    let mut baked = before.clone();
    palette.bake_smoke_sprite_frame(&mut baked, frame.id, 0);

    let plume_rows = smoke_plume_rows(frame.id, frame.h);
    let mut cleared_plume_pixels = 0usize;
    for y in 0..plume_rows {
        for x in 0..frame.w {
            let source_pixel = source_sprite.get_pixel(x, y).0;
            if is_smoke_plume_pixel(frame.id, frame.w, frame.h, x, y, source_pixel) {
                cleared_plume_pixels += 1;
                assert_eq!(
                    before.get_pixel(x, y).0[3],
                    0,
                    "base smoke sprite plume pixel at ({x},{y}) must be transparent"
                );
                assert_eq!(
                    baked.get_pixel(x, y).0[3],
                    0,
                    "base smoke sprite plume pixel at ({x},{y}) must stay transparent after base bake"
                );
            }
        }
    }
    assert!(
        cleared_plume_pixels > 0,
        "source smoke sprite should have plume pixels to clear from the base atlas"
    );

    for y in plume_rows..frame.h {
        for x in 0..frame.w {
            assert_eq!(
                before.get_pixel(x, y).0,
                baked.get_pixel(x, y).0,
                "building body pixel at ({x},{y}) must be unchanged by smoke bake"
            );
        }
    }

    let mut plume_water_blue = 0usize;
    for y in 0..plume_rows {
        for x in 0..frame.w {
            let p = baked.get_pixel(x, y).0;
            if p[3] > 0 && p[0..3] == [35, 55, 255] {
                plume_water_blue += 1;
            }
        }
    }
    assert_eq!(
        plume_water_blue, 0,
        "smoke plume must not be baked to water-bright-blue"
    );
}

#[test]
fn water_sprite_baked_frames_differ_in_animation_atlas() {
    use crate::assets::AtlasSource;
    use crate::palette::{AnimatedPalette, animated_sprite_id};

    let Some(asset_root) = crate::paths::test_asset_root() else {
        return;
    };
    let atlas_dir = asset_root.as_path().join("sprites/SPRT_00129_ShapeData");
    if !atlas_dir.is_dir() {
        return;
    }

    let source = AtlasSource::load(&atlas_dir).expect("load atlas");
    let palette = AnimatedPalette::load(&asset_root).expect("palette");
    let atlas = build_animation_atlas(std::slice::from_ref(&source), &palette)
        .expect("build animation atlas")
        .expect("animation atlas");

    assert_eq!(
        *atlas.animation_counts.get(&1270).unwrap_or(&0),
        8,
        "water sprite 1270 should have 8 baked frames"
    );
    let frame0 = atlas.frames.get(&1270).expect("base water frame");
    let frame1 = atlas
        .frames
        .get(&animated_sprite_id(1270, 1))
        .expect("baked water frame 1");
    let mut diff_pixels = 0usize;
    let mut sample_opaque = [0u8; 4];
    for y in 0..frame0.h {
        for x in 0..frame0.w {
            let p0 = atlas.rgba.get_pixel(frame0.x + x, frame0.y + y).0;
            let p1 = atlas.rgba.get_pixel(frame1.x + x, frame1.y + y).0;
            if p0[3] > 0 && sample_opaque[3] == 0 {
                sample_opaque = p0;
            }
            if p0 != p1 {
                diff_pixels += 1;
            }
        }
    }
    assert!(
        diff_pixels > 0,
        "baked water frames 0 and 1 should differ (sample opaque pixel {sample_opaque:?}, diff count {diff_pixels})"
    );
}

#[test]
fn smoke_sprite_baked_frames_differ_in_animation_atlas() {
    use crate::assets::AtlasSource;
    use crate::palette::{
        AnimatedPalette, animated_sprite_id, is_smoke_plume_pixel, smoke_plume_rows,
    };

    let Some(asset_root) = crate::paths::test_asset_root() else {
        return;
    };
    let atlas_dir = asset_root.as_path().join("sprites/SPRT_00129_ShapeData");
    if !atlas_dir.is_dir() {
        return;
    }

    let source = AtlasSource::load(&atlas_dir).expect("load atlas");
    let palette = AnimatedPalette::load(&asset_root).expect("palette");
    let atlas = build_animation_atlas(std::slice::from_ref(&source), &palette)
        .expect("build animation atlas")
        .expect("animation atlas");

    assert_eq!(
        *atlas.animation_counts.get(&1201).unwrap_or(&0),
        8,
        "smoke sprite 1201 should have 8 baked frames"
    );
    for sprite_id in [1201, 1202, 1207, 1250] {
        assert_eq!(
            *atlas.animation_counts.get(&sprite_id).unwrap_or(&0),
            8,
            "smoke sprite {sprite_id} should have 8 baked frames"
        );
        assert!(
            atlas.frames.contains_key(&animated_sprite_id(sprite_id, 1)),
            "smoke sprite {sprite_id} should have an animated frame in the atlas"
        );
    }
    assert_eq!(
        *atlas.animation_counts.get(&1220).unwrap_or(&0),
        0,
        "water pump sprite 1220 should not be baked as smoke"
    );
    for sprite_id in [1160, 1162, 1163, 1164, 1165, 1189, 1191] {
        assert_eq!(
            *atlas.animation_counts.get(&sprite_id).unwrap_or(&0),
            0,
            "RCI growth sprite {sprite_id} must not be baked as smoke"
        );
    }
    let frame0 = atlas.frames.get(&1201).expect("base smoke frame");
    let frame2 = atlas
        .frames
        .get(&animated_sprite_id(1201, 2))
        .expect("baked smoke frame 2");
    let plume_rows = smoke_plume_rows(1201, frame0.h);
    let mut diff_pixels = 0usize;
    let mut visible_frame0_pixels = 0usize;
    let mut visible_frame2_pixels = 0usize;
    let mut body_opaque_pixels = 0usize;
    for y in 0..plume_rows {
        for x in 0..frame0.w {
            let p0 = atlas.rgba.get_pixel(frame0.x + x, frame0.y + y).0;
            let p2 = atlas.rgba.get_pixel(frame2.x + x, frame2.y + y).0;
            if p0[3] > 0 {
                visible_frame0_pixels += 1;
            }
            if p2[3] > 0 {
                visible_frame2_pixels += 1;
            }
            if p0 != p2 {
                diff_pixels += 1;
            }
        }
    }
    for y in plume_rows..frame0.h {
        for x in 0..frame0.w {
            let p0 = atlas.rgba.get_pixel(frame0.x + x, frame0.y + y).0;
            let p2 = atlas.rgba.get_pixel(frame2.x + x, frame2.y + y).0;
            if p0[3] > 0 || p2[3] > 0 {
                body_opaque_pixels += 1;
            }
        }
    }
    let mut source_plume_pixels = 0usize;
    let source_frame = source
        .manifest
        .frames
        .iter()
        .find(|frame| frame.id == 1201)
        .expect("source smoke sprite 1201");
    let source_image = image::open(&source.image_path)
        .expect("open source atlas")
        .to_rgba8();
    for y in 0..source_frame.h {
        for x in 0..source_frame.w {
            let pixel = source_image
                .get_pixel(source_frame.x + x, source_frame.y + y)
                .0;
            if is_smoke_plume_pixel(1201, source_frame.w, source_frame.h, x, y, pixel) {
                source_plume_pixels += 1;
                let overlay_pixel0 = atlas.rgba.get_pixel(frame0.x + x, frame0.y + y).0;
                let overlay_pixel2 = atlas.rgba.get_pixel(frame2.x + x, frame2.y + y).0;
                assert!(
                    overlay_pixel0[3] > 0 || overlay_pixel2[3] > 0,
                    "smoke overlay plume pixel ({x},{y}) should be visible in at least one sampled frame"
                );
            }
        }
    }
    assert!(
        source_plume_pixels > 0,
        "source smoke sprite should expose plume marker pixels"
    );
    assert!(
        diff_pixels > 0,
        "baked smoke plume frames 0 and 2 should differ"
    );
    assert!(visible_frame0_pixels > 0);
    assert!(visible_frame2_pixels > 0);
    assert_eq!(
        body_opaque_pixels, 0,
        "smoke animation atlas frames must be transparent outside the plume mask"
    );
}

#[test]
fn smoke_animation_atlas_contains_plume_overlays_only() {
    use crate::assets::AtlasSource;
    use crate::palette::{AnimatedPalette, animated_sprite_id, is_smoke_plume_pixel};

    let Some(asset_root) = crate::paths::test_asset_root() else {
        return;
    };
    let atlas_dir = asset_root.as_path().join("sprites/SPRT_00129_ShapeData");
    if !atlas_dir.is_dir() {
        return;
    }

    let source = AtlasSource::load(&atlas_dir).expect("load atlas");
    let palette = AnimatedPalette::load(&asset_root).expect("palette");
    let animation_atlas = build_animation_atlas(std::slice::from_ref(&source), &palette)
        .expect("build animation atlas")
        .expect("animation atlas");
    let source_image = image::open(&source.image_path)
        .expect("open source atlas")
        .to_rgba8();

    for sprite_id in [1201, 1202, 1207, 1250] {
        let source_frame = source
            .manifest
            .frames
            .iter()
            .find(|frame| frame.id == sprite_id)
            .unwrap_or_else(|| panic!("source smoke sprite {sprite_id}"));
        let overlay0 = animation_atlas
            .frames
            .get(&sprite_id)
            .unwrap_or_else(|| panic!("smoke overlay frame 0 for {sprite_id}"));
        let overlay2 = animation_atlas
            .frames
            .get(&animated_sprite_id(sprite_id, 2))
            .unwrap_or_else(|| panic!("smoke overlay frame 2 for {sprite_id}"));

        let mut plume_pixels = 0usize;
        let mut plume_diff_pixels = 0usize;
        for y in 0..source_frame.h {
            for x in 0..source_frame.w {
                let source_pixel = source_image
                    .get_pixel(source_frame.x + x, source_frame.y + y)
                    .0;
                let p0 = animation_atlas
                    .rgba
                    .get_pixel(overlay0.x + x, overlay0.y + y)
                    .0;
                let p2 = animation_atlas
                    .rgba
                    .get_pixel(overlay2.x + x, overlay2.y + y)
                    .0;
                if is_smoke_plume_pixel(
                    sprite_id,
                    source_frame.w,
                    source_frame.h,
                    x,
                    y,
                    source_pixel,
                ) {
                    plume_pixels += 1;
                    if p0 != p2 {
                        plume_diff_pixels += 1;
                    }
                    assert!(
                        p0[3] > 0 && p2[3] > 0,
                        "plume pixel ({x},{y}) for {sprite_id} should stay visible while its palette color cycles"
                    );
                } else {
                    assert_eq!(
                        p0[3], 0,
                        "non-plume pixel ({x},{y}) for {sprite_id} must be transparent"
                    );
                    assert_eq!(
                        p2[3], 0,
                        "animated non-plume pixel ({x},{y}) for {sprite_id} must be transparent"
                    );
                }
            }
        }

        assert!(
            plume_pixels > 0,
            "sprite {sprite_id} should have plume pixels"
        );
        assert!(
            plume_diff_pixels > 0,
            "sprite {sprite_id} plume overlay should animate by palette color"
        );
    }
}

#[test]
fn compact_animation_atlas_excludes_non_smoke_city_buildings() {
    use crate::assets::AtlasSource;
    use crate::palette::{AnimatedPalette, animated_sprite_id};

    let Some(asset_root) = crate::paths::test_asset_root() else {
        return;
    };
    let atlas_dir = asset_root.as_path().join("sprites/SPRT_00129_ShapeData");
    if !atlas_dir.is_dir() {
        return;
    }

    let source = AtlasSource::load(&atlas_dir).expect("load atlas");
    let palette = AnimatedPalette::load(&asset_root).expect("palette");
    let animation_atlas = build_animation_atlas(std::slice::from_ref(&source), &palette)
        .expect("build animation atlas")
        .expect("animation atlas");

    for &base_id in animation_atlas.animation_counts.keys() {
        assert!(
            base_id == 1005 || is_water_sprite(base_id) || is_smoke_sprite(base_id),
            "compact animation atlas must not carry non-water/non-smoke sprite {base_id}"
        );
    }

    for frame in &source.manifest.frames {
        if !(1001..=1255).contains(&frame.id) || frame.id == 1005 || is_smoke_sprite(frame.id) {
            continue;
        }
        assert!(
            !animation_atlas.animation_counts.contains_key(&frame.id),
            "non-animated city building sprite {} must not have animation frames",
            frame.id
        );
        for anim_frame in 0..8 {
            let resolved = animated_sprite_id(frame.id, anim_frame);
            assert!(
                !animation_atlas.frames.contains_key(&resolved),
                "compact animation atlas must not contain frame {anim_frame} for non-smoke city building sprite {}",
                frame.id
            );
        }
    }
}

#[test]
fn traffic_bake_keeps_shape_atlas_within_gpu_limit() {
    use crate::assets::AtlasSource;
    use crate::palette::AnimatedPalette;
    use std::path::Path;

    let Some(asset_root) = crate::paths::test_asset_root() else {
        return;
    };
    const MAX_TEXTURE_DIM: u32 = 8192;
    let atlas_dir = asset_root.as_path().join("sprites/SPRT_00129_ShapeData");
    if !atlas_dir.is_dir() {
        return;
    }

    let source = AtlasSource::load(&atlas_dir).expect("load atlas");
    let palette = AnimatedPalette::load(&asset_root).expect("palette");
    let image = image::open(&source.image_path)
        .expect("open atlas png")
        .to_rgba8();
    let (mut width, mut height) = image.dimensions();
    let mut rgba = image;
    let mut frames: HashMap<u32, AtlasFrame> = source
        .manifest
        .frames
        .iter()
        .cloned()
        .map(|frame| (frame.id, frame))
        .collect();
    let mut animation_counts = HashMap::new();
    bake_traffic_color_variants(
        &mut rgba,
        &mut frames,
        &palette,
        &source.manifest.frames,
        &mut width,
        &mut height,
        &mut animation_counts,
    );

    assert!(
        height <= MAX_TEXTURE_DIM,
        "SPRT_00129 baked height {height} exceeds GPU limit {MAX_TEXTURE_DIM}"
    );
}

#[test]
fn authored_replacement_traffic_does_not_expand_replacement_atlas() {
    use crate::assets::load_replacement_sprite_source;
    use crate::palette::AnimatedPalette;
    use std::path::Path;

    let Some(root) = crate::paths::test_replacement_root() else {
        return;
    };
    let source = load_replacement_sprite_source(&root)
        .expect("load replacement pack")
        .expect("replacement sprites");
    let palette = AnimatedPalette::load("/nonexistent/citygame-test-palette")
        .expect("neutral palette fallback");
    let mut rgba = source.load_rgba().expect("replacement atlas image");
    let original_dimensions = rgba.dimensions();
    let (mut width, mut height) = original_dimensions;
    let mut frames: HashMap<u32, AtlasFrame> = source
        .manifest
        .frames
        .iter()
        .cloned()
        .map(|frame| (frame.id, frame))
        .collect();
    let mut animation_counts = HashMap::new();

    bake_traffic_variants_for_source(
        &source,
        &mut rgba,
        &mut frames,
        &palette,
        &mut width,
        &mut height,
        &mut animation_counts,
    );

    assert_eq!((width, height), original_dimensions);
    assert!(
        height <= 16_384,
        "replacement atlas exceeds GPU texture limit"
    );
    assert!(animation_counts.is_empty());
}

#[test]
fn water_smoke_animation_atlas_keeps_shape_atlas_within_gpu_limit() {
    use crate::assets::AtlasSource;
    use crate::palette::{AnimatedPalette, animated_sprite_id};
    use std::path::Path;

    let Some(asset_root) = crate::paths::test_asset_root() else {
        return;
    };
    const MAX_TEXTURE_DIM: u32 = 8192;
    let atlas_dir = asset_root.as_path().join("sprites/SPRT_00129_ShapeData");
    if !atlas_dir.is_dir() {
        return;
    }

    let source = AtlasSource::load(&atlas_dir).expect("load atlas");
    let palette = AnimatedPalette::load(&asset_root).expect("palette");
    assert!(
        source
            .manifest
            .frames
            .iter()
            .all(|frame| frame.id != animated_sprite_id(1270, 1)),
        "source shape atlas should not contain appended water animation frames"
    );
    let image = image::open(&source.image_path)
        .expect("open atlas png")
        .to_rgba8();
    let (_width, height) = image.dimensions();
    let mut rgba = image;
    let mut animation_counts = HashMap::new();
    normalize_smoke_building_sprites(&mut rgba, &source.manifest.frames);
    bake_base_animated_sprites(
        &mut rgba,
        &mut animation_counts,
        &palette,
        &source.manifest.frames,
    );
    let animation_atlas = build_animation_atlas(&[source], &palette)
        .expect("build water/smoke animation atlas")
        .expect("water/smoke animation atlas");

    assert!(
        height <= MAX_TEXTURE_DIM,
        "SPRT_00129 base height {height} should not grow for water/smoke animation"
    );
    assert!(
        animation_atlas.height <= MAX_TEXTURE_DIM,
        "water/smoke animation atlas height {} exceeds GPU limit {MAX_TEXTURE_DIM}",
        animation_atlas.height
    );
    assert!(
        animation_atlas.width <= ANIMATION_ATLAS_MAX_WIDTH,
        "water/smoke animation atlas width {} exceeds configured width {ANIMATION_ATLAS_MAX_WIDTH}",
        animation_atlas.width
    );
    assert!(
        animation_atlas
            .frames
            .contains_key(&animated_sprite_id(1270, 1)),
        "compact animation atlas should contain water frame 1"
    );
    assert!(
        animation_atlas
            .frames
            .contains_key(&animated_sprite_id(1201, 1)),
        "compact animation atlas should contain smoke frame 1"
    );
}

#[test]
fn traffic_bake_colors_small_dash_art() {
    use crate::assets::AtlasSource;
    use crate::palette::AnimatedPalette;
    use std::path::Path;

    let Some(asset_root) = crate::paths::test_asset_root() else {
        return;
    };
    let atlas_dir = asset_root.as_path().join("sprites/SPRT_00128_ShapeData");
    if !atlas_dir.is_dir() {
        return;
    }

    let source = AtlasSource::load(&atlas_dir).expect("load atlas");
    let palette = AnimatedPalette::load(&asset_root).expect("palette");
    let image = image::open(&source.image_path)
        .expect("open atlas png")
        .to_rgba8();
    let (mut width, mut height) = image.dimensions();
    let mut rgba = image;
    let mut frames: HashMap<u32, AtlasFrame> = source
        .manifest
        .frames
        .iter()
        .cloned()
        .map(|frame| (frame.id, frame))
        .collect();
    let mut animation_counts = HashMap::new();

    bake_traffic_color_variants(
        &mut rgba,
        &mut frames,
        &palette,
        &source.manifest.frames,
        &mut width,
        &mut height,
        &mut animation_counts,
    );

    let after_frame = frames.get(&400).expect("traffic dash 400 after bake");
    let after = extract_sprite_region(&rgba, after_frame);
    let opaque: Vec<_> = after.pixels().filter(|pixel| pixel.0[3] > 0).collect();
    let road_gray_pixels = opaque
        .iter()
        .filter(|pixel| {
            let [r, g, b, _] = pixel.0;
            (120..=160).contains(&r) && (120..=160).contains(&g) && (120..=160).contains(&b)
        })
        .count();
    let black_pixels = opaque
        .iter()
        .filter(|pixel| {
            let [r, g, b, _] = pixel.0;
            r < 20 && g < 20 && b < 20
        })
        .count();
    let blue_pixels = opaque
        .iter()
        .filter(|pixel| {
            let [r, g, b, _] = pixel.0;
            r <= 40 && g <= 40 && b >= 120
        })
        .count();
    let magenta_pixels = opaque
        .iter()
        .filter(|pixel| pixel.0[0] > 150 && pixel.0[2] > 150 && pixel.0[1] < 100)
        .count();
    assert_eq!(
        road_gray_pixels, 0,
        "traffic overlay must not draw road-gray pavement pixels"
    );
    assert_eq!(
        black_pixels, 0,
        "traffic overlay must not draw black mask pixels as vehicles"
    );
    assert!(
        blue_pixels > 0,
        "small traffic overlay should bake to visible blue vehicle pixels, got {blue_pixels}"
    );
    assert_eq!(
        magenta_pixels, 0,
        "small traffic overlay must not retain magenta mask pixels"
    );
}

#[test]
fn traffic_bake_colors_medium_dash_art() {
    use crate::assets::AtlasSource;
    use crate::palette::AnimatedPalette;
    use std::path::Path;

    let Some(asset_root) = crate::paths::test_asset_root() else {
        return;
    };
    let atlas_dir = asset_root.as_path().join("sprites/SPRT_00128_ShapeData");
    if !atlas_dir.is_dir() {
        return;
    }

    let source = AtlasSource::load(&atlas_dir).expect("load atlas");
    let palette = AnimatedPalette::load(&asset_root).expect("palette");
    let image = image::open(&source.image_path)
        .expect("open atlas png")
        .to_rgba8();
    let (mut width, mut height) = image.dimensions();
    let mut rgba = image;
    let mut frames: HashMap<u32, AtlasFrame> = source
        .manifest
        .frames
        .iter()
        .cloned()
        .map(|frame| (frame.id, frame))
        .collect();
    let mut animation_counts = HashMap::new();

    bake_traffic_color_variants(
        &mut rgba,
        &mut frames,
        &palette,
        &source.manifest.frames,
        &mut width,
        &mut height,
        &mut animation_counts,
    );

    let frame = frames.get(&900).expect("medium traffic dash 900");
    let baked = extract_sprite_region(&rgba, frame);
    let opaque: Vec<_> = baked.pixels().filter(|pixel| pixel.0[3] > 0).collect();
    assert!(
        opaque.len() >= 12,
        "medium traffic dash should bake to a visible mark, got {} opaque pixels",
        opaque.len()
    );
    let road_gray_pixels = opaque
        .iter()
        .filter(|pixel| {
            let [r, g, b, _] = pixel.0;
            (120..=160).contains(&r) && (120..=160).contains(&g) && (120..=160).contains(&b)
        })
        .count();
    let black_pixels = opaque
        .iter()
        .filter(|pixel| {
            let [r, g, b, _] = pixel.0;
            r < 20 && g < 20 && b < 20
        })
        .count();
    let blue_pixels = opaque
        .iter()
        .filter(|pixel| {
            let [r, g, b, _] = pixel.0;
            r <= 40 && g <= 40 && b >= 120
        })
        .count();
    assert_eq!(
        road_gray_pixels, 0,
        "traffic overlay must not draw road-gray pavement pixels"
    );
    assert_eq!(
        black_pixels, 0,
        "traffic overlay must not draw black mask pixels as vehicles"
    );
    assert!(
        blue_pixels >= 8,
        "medium traffic dash should have visible blue vehicle pixels, got {blue_pixels}"
    );
}

#[test]
fn traffic_bake_colors_large_dash_art_without_pavement_leak() {
    use crate::assets::AtlasSource;
    use crate::palette::AnimatedPalette;
    use std::path::Path;

    let Some(asset_root) = crate::paths::test_asset_root() else {
        return;
    };
    let atlas_dir = asset_root.as_path().join("sprites/SPRT_00129_ShapeData");
    if !atlas_dir.is_dir() {
        return;
    }

    let source = AtlasSource::load(&atlas_dir).expect("load atlas");
    let palette = AnimatedPalette::load(&asset_root).expect("palette");
    let image = image::open(&source.image_path)
        .expect("open atlas png")
        .to_rgba8();
    let (mut width, mut height) = image.dimensions();
    let mut rgba = image;
    let mut frames: HashMap<u32, AtlasFrame> = source
        .manifest
        .frames
        .iter()
        .cloned()
        .map(|frame| (frame.id, frame))
        .collect();
    let mut animation_counts = HashMap::new();

    bake_traffic_color_variants(
        &mut rgba,
        &mut frames,
        &palette,
        &source.manifest.frames,
        &mut width,
        &mut height,
        &mut animation_counts,
    );

    let frame = frames.get(&1402).expect("large tall traffic dash 1402");
    let baked = extract_sprite_region(&rgba, frame);
    let opaque: Vec<_> = baked.pixels().filter(|pixel| pixel.0[3] > 0).collect();
    assert!(
        (64..=160).contains(&opaque.len()),
        "large traffic overlay should bake to a road-sized lane mark, got {} opaque pixels",
        opaque.len()
    );
    let pavement_pixels = opaque
        .iter()
        .filter(|pixel| {
            let [r, g, b, _] = pixel.0;
            ((120..=170).contains(&r) && (120..=170).contains(&g) && (120..=170).contains(&b))
                || ((80..=130).contains(&r) && (35..=90).contains(&g) && b <= 45)
        })
        .count();
    let blue_pixels = opaque
        .iter()
        .filter(|pixel| {
            let [r, g, b, _] = pixel.0;
            r <= 40 && g <= 40 && b >= 120
        })
        .count();
    assert_eq!(
        pavement_pixels, 0,
        "large traffic overlay must not draw gray road or brown anchor pixels"
    );
    assert!(
        blue_pixels > 0,
        "large traffic dash should have visible blue vehicle pixels"
    );
}

#[test]
fn compact_traffic_atlas_carries_large_overlay_animation_frames() {
    use crate::assets::load_sprite_sources;
    use crate::palette::{AnimatedPalette, animated_sprite_id};
    use std::path::Path;

    let Some(asset_root) = crate::paths::test_asset_root() else {
        return;
    };
    if !asset_root
        .as_path()
        .join("sprites/SPRT_00129_ShapeData")
        .is_dir()
    {
        return;
    }

    let mut sources = vec![
        crate::assets::load_replacement_sprite_source(
            match crate::paths::test_replacement_root() {
                Some(root) => root,
                None => return,
            },
        )
        .expect("load replacement source")
        .expect("replacement sprites"),
    ];
    sources.extend(load_sprite_sources(&asset_root).expect("load sprite sources"));
    let palette = AnimatedPalette::load(&asset_root).expect("palette");
    let atlas = build_traffic_atlas(&sources, &palette)
        .expect("build traffic atlas")
        .expect("traffic atlas");

    assert!(atlas.width <= TRAFFIC_ATLAS_MAX_WIDTH);
    assert!(
        atlas.height <= ANIMATION_ATLAS_MAX_HEIGHT,
        "compact traffic atlas height {} exceeds GPU texture limit {ANIMATION_ATLAS_MAX_HEIGHT}",
        atlas.height
    );
    assert_eq!(
        atlas.animation_counts.get(&1400).copied(),
        Some(TRAFFIC_ATLAS_ANIM_FRAMES)
    );
    assert!(
        atlas.frames.contains_key(&400) && atlas.frames.contains_key(&900),
        "small and medium traffic masks must come from the original traffic source"
    );
    assert!(
        atlas.frames.contains_key(&animated_sprite_id(1400, 7)),
        "large traffic overlay should have phase 7 in the compact atlas"
    );
    assert!(
        !atlas.frames.contains_key(&1450) && !atlas.frames.contains_key(&1467),
        "watered underground pipe sprites must not be baked as traffic"
    );

    let frame0 = atlas.frames.get(&1400).expect("large traffic frame 0");
    let frame7 = atlas
        .frames
        .get(&animated_sprite_id(1400, 7))
        .expect("large traffic frame 7");
    let image0 = extract_sprite_region(&atlas.rgba, frame0);
    let image7 = extract_sprite_region(&atlas.rgba, frame7);
    assert_ne!(
        image0.as_raw(),
        image7.as_raw(),
        "large traffic frame pixels should differ across phases"
    );
}

#[test]
fn native_win95_traffic_masks_animate_all_positions_without_empty_frames() {
    let Some(root) = crate::paths::test_win95_root() else {
        return;
    };
    let source = crate::win95_import::traffic_mask_source_from_install(&root)
        .unwrap()
        .expect("native Win95 traffic masks");
    let palette = AnimatedPalette::load(crate::paths::asset_root()).unwrap();
    let atlas = build_traffic_atlas(&[source], &palette).unwrap().unwrap();
    let ids = [1400, 1401, 1427, 1428];
    let mut sequences = Vec::new();
    for id in ids {
        assert_eq!(atlas.animation_counts[&id], 8);
        let mut frames = Vec::new();
        for phase in 0..8 {
            let rect = &atlas.frames[&animated_sprite_id(id, phase)];
            let image = extract_sprite_region(&atlas.rgba, rect);
            assert_eq!(image.dimensions(), (32, 17));
            assert!(
                image.pixels().any(|p| p[3] != 0),
                "empty {id} phase {phase}"
            );
            frames.push(image);
        }
        let unique: HashSet<_> = frames.iter().map(|image| image.as_raw()).collect();
        assert_eq!(
            unique.len(),
            8,
            "{id} must move through all eight positions"
        );
        sequences.push(frames);
    }

    // Optional visual evidence from the actual native importer and traffic atlas baker.
    if let Some(output) = std::env::var_os("SC2000_TEST_TRAFFIC_PREVIEW") {
        let output = PathBuf::from(output);
        std::fs::create_dir_all(&output).unwrap();
        let originals = load_sprite_sources(crate::paths::asset_root()).unwrap();
        let roads: Vec<_> = [1029, 1030]
            .iter()
            .map(|id| {
                let source = originals
                    .iter()
                    .find(|s| s.frame_by_id(*id).is_some())
                    .unwrap();
                extract_sprite_region(
                    &source.load_rgba().unwrap(),
                    source.frame_by_id(*id).unwrap(),
                )
            })
            .collect();
        let mut sheet = RgbaImage::new(32 * 8, 17 * 4);
        let mut gif_frames = Vec::new();
        for phase in 0..8 {
            let mut preview = RgbaImage::new(64, 34);
            for (row, sequence) in sequences.iter().enumerate() {
                let mut tile = roads[row % 2].clone();
                image::imageops::overlay(&mut tile, &sequence[phase], 0, 0);
                image::imageops::overlay(&mut sheet, &tile, phase as i64 * 32, row as i64 * 17);
                image::imageops::overlay(
                    &mut preview,
                    &tile,
                    (row % 2) as i64 * 32,
                    (row / 2) as i64 * 17,
                );
            }
            let preview =
                image::imageops::resize(&preview, 512, 272, image::imageops::FilterType::Nearest);
            gif_frames.push(image::Frame::from_parts(
                preview,
                0,
                0,
                image::Delay::from_numer_denom_ms(250, 1),
            ));
        }
        image::imageops::resize(&sheet, 1024, 272, image::imageops::FilterType::Nearest)
            .save(output.join("traffic-eight-phases.png"))
            .unwrap();
        let mut gif = image::codecs::gif::GifEncoder::new(
            std::fs::File::create(output.join("traffic-restored.gif")).unwrap(),
        );
        gif.set_repeat(image::codecs::gif::Repeat::Infinite)
            .unwrap();
        gif.encode_frames(gif_frames).unwrap();
    }
}

#[test]
fn native_highway_traffic_uses_full_piece_canvases_in_every_phase() {
    let Some(root) = crate::paths::test_win95_root() else {
        return;
    };
    let Some(asset_root) = crate::paths::test_asset_root() else {
        return;
    };
    let source = crate::win95_import::traffic_mask_source_from_install(&root)
        .unwrap()
        .unwrap();
    let palette = AnimatedPalette::load(&asset_root).unwrap();
    let atlas = build_traffic_atlas(&[source], &palette).unwrap().unwrap();
    let originals = load_sprite_sources(&asset_root).unwrap();
    let mut previews = (0..8)
        .map(|_| RgbaImage::from_pixel(240, 216, image::Rgba([62, 32, 12, 255])))
        .collect::<Vec<_>>();
    for (piece, building) in (97..=105).enumerate() {
        let surface_id = 1000 + u32::from(building);
        let source = originals
            .iter()
            .find(|s| s.frame_by_id(surface_id).is_some())
            .unwrap();
        let surface = extract_sprite_region(
            &source.load_rgba().unwrap(),
            source.frame_by_id(surface_id).unwrap(),
        );
        assert_eq!(surface.width(), 64);
        for density in [
            crate::traffic::TrafficDensity::Light,
            crate::traffic::TrafficDensity::Heavy,
        ] {
            let id = crate::traffic::traffic_sprite_for_road(building, density).unwrap() + 1000;
            for phase in 0..8 {
                let rect = &atlas.frames[&animated_sprite_id(id, phase)];
                let traffic = extract_sprite_region(&atlas.rgba, rect);
                assert_eq!(traffic.width(), surface.width());
                assert!(traffic.height() <= surface.height());
                assert!(traffic.pixels().any(|p| p[3] != 0), "{id} phase {phase}");
                if density == crate::traffic::TrafficDensity::Heavy {
                    let mut tile = surface.clone();
                    image::imageops::overlay(
                        &mut tile,
                        &traffic,
                        0,
                        i64::from(surface.height() - traffic.height()),
                    );
                    image::imageops::overlay(
                        &mut previews[phase as usize],
                        &tile,
                        (piece % 3) as i64 * 80 + 8,
                        (piece / 3) as i64 * 72 + 4,
                    );
                }
            }
        }
    }
    if let Some(output) = std::env::var_os("SC2000_TEST_TRAFFIC_PREVIEW") {
        let output = PathBuf::from(output);
        std::fs::create_dir_all(&output).unwrap();
        let frames = previews.into_iter().map(|image| {
            image::Frame::from_parts(
                image::imageops::resize(&image, 720, 648, image::imageops::FilterType::Nearest),
                0,
                0,
                image::Delay::from_numer_denom_ms(250, 1),
            )
        });
        let mut gif = image::codecs::gif::GifEncoder::new(
            std::fs::File::create(output.join("highway-traffic-restored.gif")).unwrap(),
        );
        gif.set_repeat(image::codecs::gif::Repeat::Infinite)
            .unwrap();
        gif.encode_frames(frames).unwrap();
    }
}

#[test]
fn native_flat_highway_traffic_moves_along_the_deck_in_both_directions() {
    use crate::traffic::{TrafficDensity, road_tile_overlay_flip_x, traffic_overlay_at_for_engine};
    let Some(root) = crate::paths::test_win95_root() else {
        return;
    };
    let Some(asset_root) = crate::paths::test_asset_root() else {
        return;
    };
    let source = crate::win95_import::traffic_mask_source_from_install(&root)
        .unwrap()
        .unwrap();
    let palette = AnimatedPalette::load(&asset_root).unwrap();
    let atlas = build_traffic_atlas(&[source], &palette).unwrap().unwrap();
    let originals = load_sprite_sources(&asset_root).unwrap();
    let mut previews = (0..8)
        .map(|_| RgbaImage::from_pixel(80, 72, image::Rgba([62, 32, 12, 255])))
        .collect::<Vec<_>>();
    let blue_positions = |image: &RgbaImage| -> HashSet<(i32, i32)> {
        image
            .enumerate_pixels()
            .filter_map(|(x, y, pixel)| {
                (pixel[3] != 0 && pixel[2] > pixel[0] && pixel[2] > pixel[1])
                    .then_some((x as i32, y as i32))
            })
            .collect()
    };
    for raw_building in 73..=80 {
        let mut city =
            crate::city::City::new_blank("/tmp/traffic-highway-direction.sc2", "HighwayDirection")
                .unwrap();
        city.set_building(20, 30, raw_building);
        city.set_tile_bits(20, 30, crate::city::TileBits::new(0x02));
        for rotation in 0..4 {
            city.set_view_rotation(rotation);
            for density in [TrafficDensity::Light, TrafficDensity::Heavy] {
                let mut rng = crate::mac_random::MacToolboxRng::new(1);
                let overlay = traffic_overlay_at_for_engine(
                    &city,
                    20,
                    30,
                    crate::engine_fidelity::EngineFidelity::Win95,
                    |_, _| {
                        if density == TrafficDensity::Light {
                            0x30
                        } else {
                            0x80
                        }
                    },
                    &mut rng,
                    0,
                )
                .unwrap();
                let id = overlay.sprite_id + 1000;
                let flip = road_tile_overlay_flip_x(&city, 20, 30);
                let frame = |phase| {
                    let image = extract_sprite_region(
                        &atlas.rgba,
                        &atlas.frames[&animated_sprite_id(id, phase)],
                    );
                    if flip {
                        image::imageops::flip_horizontal(&image)
                    } else {
                        image
                    }
                };
                let before = blue_positions(&frame(0));
                let after = blue_positions(&frame(2));
                let dx = if density == TrafficDensity::Light {
                    4
                } else {
                    2
                };
                let grid_x = (raw_building & 1 == 0) ^ (rotation & 1 != 0);
                let dy = if grid_x { dx / 2 } else { -dx / 2 };
                let along_deck = before
                    .iter()
                    .filter(|&&(x, y)| after.contains(&(x + dx, y + dy)))
                    .count();
                let across_deck = before
                    .iter()
                    .filter(|&&(x, y)| after.contains(&(x + dx, y - dy)))
                    .count();
                assert!(
                    along_deck * 2 > before.len() && along_deck > across_deck,
                    "tile={raw_building} rotation={rotation} density={density:?} id={id} flip={flip}: along={along_deck}, across={across_deck}, marks={}",
                    before.len()
                );

                if raw_building <= 74 && rotation == 0 {
                    let surface_id = 1000 + u32::from(raw_building);
                    let source = originals
                        .iter()
                        .find(|s| s.frame_by_id(surface_id).is_some())
                        .unwrap();
                    let surface = extract_sprite_region(
                        &source.load_rgba().unwrap(),
                        source.frame_by_id(surface_id).unwrap(),
                    );
                    let row = usize::from(density == TrafficDensity::Heavy);
                    let column = usize::from(raw_building == 74);
                    for phase in 0..8 {
                        let mut tile = surface.clone();
                        let traffic = frame(phase);
                        image::imageops::overlay(
                            &mut tile,
                            &traffic,
                            0,
                            i64::from(surface.height() - traffic.height()),
                        );
                        image::imageops::overlay(
                            &mut previews[phase as usize],
                            &tile,
                            column as i64 * 40 + 4,
                            row as i64 * 36 + 4,
                        );
                    }
                }
            }
        }
    }
    if let Some(output) = std::env::var_os("SC2000_TEST_TRAFFIC_PREVIEW") {
        let output = PathBuf::from(output);
        std::fs::create_dir_all(&output).unwrap();
        let frames = previews.into_iter().map(|image| {
            image::Frame::from_parts(
                image::imageops::resize(&image, 640, 576, image::imageops::FilterType::Nearest),
                0,
                0,
                image::Delay::from_numer_denom_ms(250, 1),
            )
        });
        let mut gif = image::codecs::gif::GifEncoder::new(
            std::fs::File::create(output.join("highway-directions-restored.gif")).unwrap(),
        );
        gif.set_repeat(image::codecs::gif::Repeat::Infinite)
            .unwrap();
        gif.encode_frames(frames).unwrap();
    }
}

#[test]
fn replacement_id_collisions_are_not_used_as_traffic_overlays() {
    use crate::assets::load_replacement_sprite_source;
    use crate::palette::AnimatedPalette;
    use std::path::Path;

    let Some(replacement_root) = crate::paths::test_replacement_root() else {
        return;
    };
    let source = load_replacement_sprite_source(&replacement_root)
        .expect("load replacement sprites")
        .expect("replacement pack");
    let palette = AnimatedPalette::load("/nonexistent/citygame-test-palette")
        .expect("neutral palette fallback");
    let atlas =
        build_traffic_atlas(std::slice::from_ref(&source), &palette).expect("build traffic atlas");
    assert!(
        atlas.is_none(),
        "replacement IDs that collide with traffic masks must not populate the traffic atlas"
    );
}

#[test]
fn native_traffic_masks_win_over_replacement_car_ids_in_combined_atlas() {
    use crate::assets::load_replacement_sprite_source;
    use crate::palette::{AnimatedPalette, animated_sprite_id};
    use std::path::Path;

    let Some(root) = crate::paths::test_win95_root() else {
        return;
    };
    let Some(replacement_root) = crate::paths::test_replacement_root() else {
        return;
    };
    let replacement = load_replacement_sprite_source(&replacement_root)
        .expect("load replacement pack")
        .expect("replacement sprites");
    let native = crate::win95_import::traffic_mask_source_from_install(&root)
        .expect("load native Win95 traffic masks")
        .expect("native traffic masks");
    let palette = AnimatedPalette::load(crate::paths::asset_root()).expect("palette");

    // Put replacement art first to exercise the collision case: its authored
    // 1400/1427 car illustrations must not claim the native palette-mask IDs.
    let atlas = build_traffic_atlas(&[replacement, native], &palette)
        .expect("build combined traffic atlas")
        .expect("native traffic atlas remains available");

    for id in [1400, 1427] {
        assert_eq!(atlas.animation_counts.get(&id).copied(), Some(8));
        let mut phases = Vec::new();
        for phase in 0..8 {
            let frame = &atlas.frames[&animated_sprite_id(id, phase)];
            let image = extract_sprite_region(&atlas.rgba, frame);
            assert_eq!(image.dimensions(), (32, 17));
            assert!(image.pixels().any(|pixel| pixel[3] != 0));
            phases.push(image);
        }
        let unique: HashSet<_> = phases.iter().map(|image| image.as_raw()).collect();
        assert_eq!(unique.len(), 8, "native mask {id} must retain all phases");
    }
}

#[test]
fn compact_traffic_atlas_keeps_heavy_overlay_frames_separate_from_shape_atlas() {
    use crate::assets::load_sprite_sources;
    use crate::palette::{AnimatedPalette, animated_sprite_id};
    use std::path::Path;

    let Some(asset_root) = crate::paths::test_asset_root() else {
        return;
    };
    if !asset_root
        .as_path()
        .join("sprites/SPRT_00129_ShapeData")
        .is_dir()
    {
        return;
    }

    let sources = load_sprite_sources(&asset_root).expect("load sprite sources");
    let palette = AnimatedPalette::load(&asset_root).expect("palette");
    let atlas = build_traffic_atlas(&sources, &palette)
        .expect("build traffic atlas")
        .expect("traffic atlas");

    assert_eq!(
        atlas.animation_counts.get(&1427).copied(),
        Some(TRAFFIC_ATLAS_ANIM_FRAMES)
    );
    let frame0 = atlas.frames.get(&1427).expect("heavy frame 0");
    let frame7 = atlas
        .frames
        .get(&animated_sprite_id(1427, 7))
        .expect("heavy frame 7");
    assert_ne!(
        (frame0.x, frame0.y),
        (frame7.x, frame7.y),
        "animated traffic frames should occupy separate compact atlas rects"
    );
}
