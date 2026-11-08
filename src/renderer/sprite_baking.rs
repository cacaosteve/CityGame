//! Pixel transforms and atlas packing for imported animated and traffic sprites.

use super::*;

pub(super) fn normalize_smoke_building_sprites(
    rgba: &mut RgbaImage,
    manifest_frames: &[AtlasFrame],
) {
    for frame in manifest_frames {
        if !is_smoke_sprite(frame.id) {
            continue;
        }
        for y in 0..frame.h {
            for x in 0..frame.w {
                let src_x = frame.x + x;
                let src_y = frame.y + y;
                if src_x >= rgba.width() || src_y >= rgba.height() {
                    continue;
                }
                let pixel = rgba.get_pixel(src_x, src_y).0;
                if is_smoke_plume_pixel(frame.id, frame.w, frame.h, x, y, pixel) {
                    rgba.put_pixel(src_x, src_y, image::Rgba([0, 0, 0, 0]));
                }
            }
        }
    }
}

pub(super) fn bake_traffic_variants_for_source(
    source: &AtlasSource,
    rgba: &mut RgbaImage,
    frames: &mut HashMap<u32, AtlasFrame>,
    palette: &AnimatedPalette,
    width: &mut u32,
    height: &mut u32,
    animation_counts: &mut HashMap<u32, u8>,
) {
    // Authored replacement art is already full-color and uses its dedicated
    // traffic atlas. Palette-baking it here both alters the artwork and can
    // append a very large animation strip to the replacement atlas.
    if source.name.starts_with("replacement_") {
        return;
    }

    bake_traffic_color_variants(
        rgba,
        frames,
        palette,
        &source.manifest.frames,
        width,
        height,
        animation_counts,
    );
}

pub(super) fn bake_traffic_color_variants(
    rgba: &mut RgbaImage,
    frames: &mut HashMap<u32, AtlasFrame>,
    palette: &AnimatedPalette,
    manifest_frames: &[AtlasFrame],
    width: &mut u32,
    height: &mut u32,
    animation_counts: &mut HashMap<u32, u8>,
) {
    struct TrafficBakeInput {
        frame: AtlasFrame,
        base_sprite: RgbaImage,
        anim_count: u8,
        bake_variants: bool,
    }

    let traffic_inputs: Vec<TrafficBakeInput> = manifest_frames
        .iter()
        .filter(|frame| is_traffic_sprite(frame.id))
        .map(|frame| {
            let base_sprite = extract_sprite_region(rgba, frame);
            let is_small_dash = is_small_traffic_dash(frame.id);
            let bake_variants = should_bake_traffic_variants(frame.id) && !is_small_dash;
            let anim_count = traffic_sprite_anim_count(frame, &base_sprite);
            TrafficBakeInput {
                frame: frame.clone(),
                base_sprite,
                anim_count,
                bake_variants,
            }
        })
        .collect();
    if traffic_inputs.is_empty() {
        return;
    }

    let color_count = TRAFFIC_CAR_COLOR_INDICES.len() as u32;
    let extra_height: u32 = traffic_inputs
        .iter()
        .map(|input| {
            if input.bake_variants {
                input.frame.h * color_count * input.anim_count as u32 - input.frame.h
            } else if input.anim_count > 1 {
                input.frame.h * (input.anim_count as u32 - 1)
            } else {
                0
            }
        })
        .sum();
    if extra_height == 0 {
        for input in &traffic_inputs {
            animation_counts.insert(input.frame.id, input.anim_count);
            if should_bake_base_traffic_sprite(input.frame.id) {
                let mut base_sprite = input.base_sprite.clone();
                if is_lane_dash_family(input.frame.id) {
                    palette.bake_traffic_sprite_frame(
                        &mut base_sprite,
                        traffic_color_index_for_slot(0),
                        0,
                    );
                } else if is_traffic_car_shape(input.frame.id) {
                    palette.bake_traffic_car_sprite(
                        &mut base_sprite,
                        traffic_color_index_for_slot(0),
                        0,
                    );
                } else {
                    palette.bake_traffic_sprite_frame(
                        &mut base_sprite,
                        traffic_color_index_for_slot(0),
                        0,
                    );
                }
                blit_sprite_region(rgba, &input.frame, &base_sprite);
            }
        }
        return;
    }

    let new_height = height.saturating_add(extra_height);
    let mut expanded = RgbaImage::from_pixel(*width, new_height, image::Rgba([0, 0, 0, 0]));
    for y in 0..*height {
        for x in 0..*width {
            expanded.put_pixel(x, y, *rgba.get_pixel(x, y));
        }
    }
    *rgba = expanded;
    *height = new_height;

    let mut cursor_y = *height - extra_height;
    for input in traffic_inputs {
        animation_counts.insert(input.frame.id, input.anim_count);

        if should_bake_base_traffic_sprite(input.frame.id) {
            let mut base_sprite = input.base_sprite.clone();
            if is_lane_dash_family(input.frame.id) {
                palette.bake_traffic_sprite_frame(
                    &mut base_sprite,
                    traffic_color_index_for_slot(0),
                    0,
                );
            } else if is_traffic_car_shape(input.frame.id) {
                palette.bake_traffic_car_sprite(
                    &mut base_sprite,
                    traffic_color_index_for_slot(0),
                    0,
                );
            } else {
                palette.bake_traffic_sprite_frame(
                    &mut base_sprite,
                    traffic_color_index_for_slot(0),
                    0,
                );
            }
            blit_sprite_region(rgba, &input.frame, &base_sprite);
        }

        if !input.bake_variants {
            for anim_frame in 1..input.anim_count {
                let mut sprite = input.base_sprite.clone();
                if is_lane_dash_family(input.frame.id) {
                    palette.bake_traffic_sprite_frame(
                        &mut sprite,
                        traffic_color_index_for_slot(0),
                        anim_frame,
                    );
                } else if is_traffic_car_shape(input.frame.id) {
                    palette.bake_traffic_car_sprite(
                        &mut sprite,
                        traffic_color_index_for_slot(0),
                        anim_frame,
                    );
                } else {
                    palette.bake_traffic_sprite_frame(
                        &mut sprite,
                        traffic_color_index_for_slot(0),
                        anim_frame,
                    );
                }

                let sprite_id = animated_sprite_id(input.frame.id, anim_frame);
                let baked_frame = AtlasFrame {
                    file: input.frame.file.clone(),
                    index: input.frame.index,
                    id: sprite_id,
                    source_offset: input.frame.source_offset,
                    x: 0,
                    y: cursor_y,
                    w: input.frame.w,
                    h: input.frame.h,
                    logical_size: input.frame.logical_size,
                    anchor: input.frame.anchor,
                };
                blit_sprite_region(rgba, &baked_frame, &sprite);
                frames.insert(sprite_id, baked_frame);
                cursor_y += input.frame.h;
            }
            continue;
        }

        for slot in 0..TRAFFIC_CAR_COLOR_INDICES.len() as u8 {
            let colored_id = traffic_colored_sprite_id(input.frame.id, slot);
            animation_counts.insert(colored_id, input.anim_count);

            for anim_frame in 0..input.anim_count {
                let mut sprite = input.base_sprite.clone();
                palette.bake_traffic_sprite_frame(
                    &mut sprite,
                    traffic_color_index_for_slot(slot),
                    anim_frame,
                );

                if slot == 0 && anim_frame == 0 {
                    blit_sprite_region(rgba, &input.frame, &sprite);
                    continue;
                }

                let sprite_id = if anim_frame == 0 {
                    colored_id
                } else {
                    animated_sprite_id(colored_id, anim_frame)
                };

                let baked_frame = AtlasFrame {
                    file: input.frame.file.clone(),
                    index: input.frame.index,
                    id: sprite_id,
                    source_offset: input.frame.source_offset,
                    x: 0,
                    y: cursor_y,
                    w: input.frame.w,
                    h: input.frame.h,
                    logical_size: input.frame.logical_size,
                    anchor: input.frame.anchor,
                };
                blit_sprite_region(rgba, &baked_frame, &sprite);
                frames.insert(sprite_id, baked_frame);
                cursor_y += input.frame.h;
            }
        }
    }
}

pub(super) fn build_traffic_atlas(
    sources: &[AtlasSource],
    palette: &AnimatedPalette,
) -> Result<Option<BuiltAtlas>> {
    struct TrafficFrame {
        frame: AtlasFrame,
        base_id: u32,
        sprite_id: u32,
        image: RgbaImage,
    }

    let mut generated = Vec::new();
    let mut animation_counts = HashMap::new();
    let mut seen_base_ids = HashSet::new();

    for source in sources {
        let image = source.load_rgba()?;
        // Clean-room replacement packs supply fully authored RGBA traffic sprites. The
        // extracted game sprites are palette masks and need baking; applying that mask
        // conversion to full-color replacement art makes almost every pixel transparent.
        let preserve_authored_color = source.name.starts_with("replacement_");
        for frame in source
            .manifest
            .frames
            .iter()
            .filter(|frame| is_traffic_sprite(frame.id))
        {
            // Sprite IDs are not globally unique across the source formats/shape banks.
            // For example, replacement IDs 400..=447 and 900..=947 are smoke, rail, and
            // marker details, while the traffic renderer also uses those IDs for traffic
            // masks. Replacement IDs 1400..=1449 are full-tile car illustrations, not the
            // palette masks expected by this atlas. Never let replacement art claim a
            // traffic ID here: the selected original graphics source owns this context.
            if preserve_authored_color && is_traffic_sprite(frame.id) {
                continue;
            }
            let base_sprite = extract_sprite_region(&image, frame);
            // PAL_MSTR's repeated RGB values cannot recover animation indices. Do not let
            // an older flattened export claim an ID before a lossless source can supply it.
            if !crate::palette::has_traffic_mask_pixels(&base_sprite)
                || !seen_base_ids.insert(frame.id)
            {
                continue;
            }
            let anim_count = if preserve_authored_color {
                1
            } else {
                traffic_atlas_anim_count(frame, &base_sprite)
            };
            animation_counts.insert(frame.id, anim_count);

            for anim_frame in 0..anim_count {
                let mut sprite = base_sprite.clone();
                if !preserve_authored_color {
                    if is_traffic_car_shape(frame.id) {
                        palette.bake_traffic_car_sprite(
                            &mut sprite,
                            traffic_color_index_for_slot(0),
                            anim_frame,
                        );
                    } else {
                        palette.bake_traffic_overlay_sprite_frame(
                            &mut sprite,
                            traffic_color_index_for_slot(0),
                            anim_frame,
                        );
                    }
                }
                generated.push(TrafficFrame {
                    frame: frame.clone(),
                    base_id: frame.id,
                    sprite_id: animated_sprite_id(frame.id, anim_frame),
                    image: sprite,
                });
            }
        }
    }

    if generated.is_empty() {
        log::warn!(
            "no indexed traffic masks available; reimport the original game assets to preserve traffic animation"
        );
        return Ok(None);
    }

    let mut cursor_x = 0u32;
    let mut cursor_y = 0u32;
    let mut row_h = 0u32;
    let mut width = 0u32;
    let mut frames = HashMap::new();
    let mut placements = Vec::with_capacity(generated.len());

    for frame in &generated {
        let w = frame.image.width();
        let h = frame.image.height();
        if cursor_x > 0 && cursor_x + w > TRAFFIC_ATLAS_MAX_WIDTH {
            cursor_x = 0;
            cursor_y += row_h;
            row_h = 0;
        }

        let packed = AtlasFrame {
            file: frame.frame.file.clone(),
            index: frame.frame.index,
            id: frame.sprite_id,
            source_offset: frame.frame.source_offset,
            x: cursor_x,
            y: cursor_y,
            w,
            h,
            logical_size: frame.frame.logical_size,
            anchor: frame.frame.anchor,
        };
        frames.insert(frame.sprite_id, packed.clone());
        placements.push(packed);

        cursor_x += w;
        width = width.max(cursor_x);
        row_h = row_h.max(h);
    }

    let height = (cursor_y + row_h).max(1);
    anyhow::ensure!(
        height <= ANIMATION_ATLAS_MAX_HEIGHT,
        "traffic animation atlas height {height} exceeds GPU texture limit {ANIMATION_ATLAS_MAX_HEIGHT}"
    );
    let width = width.max(1);
    let mut rgba = RgbaImage::from_pixel(width, height, image::Rgba([0, 0, 0, 0]));
    for (frame, placement) in generated.iter().zip(placements.iter()) {
        blit_sprite_region(&mut rgba, placement, &frame.image);
    }

    let mut ids: Vec<u32> = generated.iter().map(|frame| frame.base_id).collect();
    ids.sort_unstable();
    ids.dedup();

    Ok(Some(BuiltAtlas {
        rgba,
        width,
        height,
        frames,
        animation_counts,
        ids,
    }))
}

pub(super) fn traffic_atlas_anim_count(frame: &AtlasFrame, base_sprite: &RgbaImage) -> u8 {
    if (400..=426).contains(&frame.id)
        || (900..=926).contains(&frame.id)
        || (1400..=1449).contains(&frame.id)
    {
        return TRAFFIC_ATLAS_ANIM_FRAMES;
    }
    traffic_sprite_anim_count(frame, base_sprite)
}

pub(super) fn should_bake_traffic_variants(sprite_id: u32) -> bool {
    // Lane dashes only; 1400+ heavy art lives in SPRT_00129 and must stay in-place
    // (100 sprites × colors × anim bands would exceed Metal's 8192px texture limit).
    (428..=447).contains(&sprite_id)
}

/// 8px lane marks (400–417). 418+ are 16px car shapes and need mask→body baking.
pub(super) fn is_small_traffic_dash(sprite_id: u32) -> bool {
    (400..=417).contains(&sprite_id)
}

/// Mac traffic overlay family across zoom banks.
///
/// These shapes are mask art. In this renderer they draw as transparent-backed blue/white
/// lane marks, not as opaque road patches.
pub(super) fn is_lane_dash_family(sprite_id: u32) -> bool {
    is_small_traffic_dash(sprite_id)
        || (900..=926).contains(&sprite_id)
        || (1400..=1426).contains(&sprite_id)
}

pub(super) fn is_traffic_car_shape(sprite_id: u32) -> bool {
    (418..=426).contains(&sprite_id)
}

pub(super) fn should_bake_base_traffic_sprite(sprite_id: u32) -> bool {
    (400..=426).contains(&sprite_id)
        || (900..=947).contains(&sprite_id)
        || (1400..=1449).contains(&sprite_id)
}

/// SC2K traffic tile art uses palette-cycled bands.
pub(super) fn traffic_sprite_anim_count(frame: &AtlasFrame, base_sprite: &RgbaImage) -> u8 {
    if (1400..=1449).contains(&frame.id) {
        return 1;
    }
    if (400..=426).contains(&frame.id) || (900..=926).contains(&frame.id) {
        return 8;
    }
    if should_bake_traffic_variants(frame.id) {
        return traffic_animation_band_count(base_sprite);
    }
    1
}

pub(super) fn build_animation_atlas(
    sources: &[AtlasSource],
    palette: &AnimatedPalette,
) -> Result<Option<BuiltAtlas>> {
    struct AnimationFrame {
        frame: AtlasFrame,
        base_id: u32,
        sprite_id: u32,
        image: RgbaImage,
    }

    let mut generated = Vec::new();
    let mut animation_counts = HashMap::new();
    let mut seen_base_ids = HashSet::new();

    for source in sources {
        let image = source.load_rgba()?;
        for frame in source
            .manifest
            .frames
            .iter()
            .filter(|frame| should_animate_sprite(frame.id))
        {
            if !seen_base_ids.insert(frame.id) {
                continue;
            }
            let base_sprite = extract_sprite_region(&image, frame);
            let anim_count = sprite_animation_frame_count(frame.id);
            animation_counts.insert(frame.id, anim_count);

            for anim_frame in 0..anim_count {
                let mut sprite = base_sprite.clone();
                if is_water_sprite(frame.id) {
                    palette.bake_water_sprite_frame(&mut sprite, anim_frame);
                } else if frame.id == 1005 {
                    sprite = radioactive_waste_sprite_frame(&base_sprite, anim_frame);
                } else {
                    sprite =
                        smoke_overlay_sprite_frame(&base_sprite, palette, frame.id, anim_frame);
                }
                generated.push(AnimationFrame {
                    frame: frame.clone(),
                    base_id: frame.id,
                    sprite_id: animated_sprite_id(frame.id, anim_frame),
                    image: sprite,
                });
            }
        }
    }

    if generated.is_empty() {
        return Ok(None);
    }

    let mut cursor_x = 0u32;
    let mut cursor_y = 0u32;
    let mut row_h = 0u32;
    let mut width = 0u32;
    let mut frames = HashMap::new();
    let mut placements = Vec::with_capacity(generated.len());

    for frame in &generated {
        let w = frame.image.width();
        let h = frame.image.height();
        if cursor_x > 0 && cursor_x + w > ANIMATION_ATLAS_MAX_WIDTH {
            cursor_x = 0;
            cursor_y += row_h;
            row_h = 0;
        }

        let packed = AtlasFrame {
            file: frame.frame.file.clone(),
            index: frame.frame.index,
            id: frame.sprite_id,
            source_offset: frame.frame.source_offset,
            x: cursor_x,
            y: cursor_y,
            w,
            h,
            logical_size: frame.frame.logical_size,
            anchor: frame.frame.anchor,
        };
        frames.insert(frame.sprite_id, packed.clone());
        placements.push(packed);

        cursor_x += w;
        width = width.max(cursor_x);
        row_h = row_h.max(h);
    }

    let height = (cursor_y + row_h).max(1);
    anyhow::ensure!(
        height <= ANIMATION_ATLAS_MAX_HEIGHT,
        "water/smoke animation atlas height {height} exceeds GPU texture limit {ANIMATION_ATLAS_MAX_HEIGHT}"
    );
    let width = width.max(1);
    let mut rgba = RgbaImage::from_pixel(width, height, image::Rgba([0, 0, 0, 0]));
    for (frame, placement) in generated.iter().zip(placements.iter()) {
        blit_sprite_region(&mut rgba, placement, &frame.image);
    }

    let mut ids: Vec<u32> = generated.iter().map(|frame| frame.base_id).collect();
    ids.sort_unstable();
    ids.dedup();

    Ok(Some(BuiltAtlas {
        rgba,
        width,
        height,
        frames,
        animation_counts,
        ids,
    }))
}

pub(super) fn smoke_overlay_sprite_frame(
    base_sprite: &RgbaImage,
    palette: &AnimatedPalette,
    sprite_id: u32,
    anim_frame: u8,
) -> RgbaImage {
    let width = base_sprite.width();
    let height = base_sprite.height();
    let mut baked = base_sprite.clone();
    palette.bake_smoke_sprite_frame(&mut baked, sprite_id, anim_frame);

    let mut overlay = RgbaImage::from_pixel(width, height, image::Rgba([0, 0, 0, 0]));
    for y in 0..height {
        for x in 0..width {
            let pixel = base_sprite.get_pixel(x, y).0;
            if is_smoke_plume_pixel(sprite_id, width, height, x, y, pixel) {
                overlay.put_pixel(x, y, *baked.get_pixel(x, y));
            }
        }
    }
    overlay
}

/// The radioactive-waste marker is a four-phase pulse. Keep the imported
/// trefoil silhouette and cycle only its red/orange signal pixels.
pub(super) fn radioactive_waste_sprite_frame(base_sprite: &RgbaImage, anim_frame: u8) -> RgbaImage {
    const PULSE: [[u8; 3]; 4] = [[255, 92, 32], [255, 190, 48], [174, 226, 64], [240, 72, 44]];
    let mut frame = base_sprite.clone();
    let pulse = PULSE[anim_frame as usize % PULSE.len()];
    for pixel in frame.pixels_mut() {
        let [r, g, b, a] = pixel.0;
        if a > 0 && r > g.saturating_add(24) && r > b.saturating_add(24) {
            *pixel = image::Rgba([pulse[0], pulse[1], pulse[2], a]);
        }
    }
    frame
}

pub(super) fn sprite_animation_frame_count(sprite_id: u32) -> u8 {
    if sprite_id == 1005 {
        4
    } else if is_water_sprite(sprite_id) || is_smoke_sprite(sprite_id) {
        8
    } else {
        1
    }
}

pub(super) fn bake_base_animated_sprites(
    rgba: &mut RgbaImage,
    animation_counts: &mut HashMap<u32, u8>,
    palette: &AnimatedPalette,
    manifest_frames: &[AtlasFrame],
) {
    for frame in manifest_frames {
        if !should_animate_sprite(frame.id) {
            continue;
        }

        let frame_count = sprite_animation_frame_count(frame.id);
        animation_counts.insert(frame.id, frame_count);
        let mut baked = extract_sprite_region(rgba, frame);
        if is_water_sprite(frame.id) {
            palette.bake_water_sprite_frame(&mut baked, 0);
        } else {
            palette.bake_smoke_sprite_frame(&mut baked, frame.id, 0);
        }
        blit_sprite_region(rgba, frame, &baked);
    }
}

pub(super) fn extract_sprite_region(rgba: &RgbaImage, frame: &AtlasFrame) -> RgbaImage {
    let mut sprite = RgbaImage::new(frame.w, frame.h);
    for y in 0..frame.h {
        for x in 0..frame.w {
            let src_x = frame.x + x;
            let src_y = frame.y + y;
            if src_x < rgba.width() && src_y < rgba.height() {
                sprite.put_pixel(x, y, *rgba.get_pixel(src_x, src_y));
            }
        }
    }
    sprite
}

pub(super) fn blit_sprite_region(rgba: &mut RgbaImage, frame: &AtlasFrame, sprite: &RgbaImage) {
    for y in 0..frame.h {
        for x in 0..frame.w {
            let dst_x = frame.x + x;
            let dst_y = frame.y + y;
            if dst_x < rgba.width() && dst_y < rgba.height() {
                rgba.put_pixel(dst_x, dst_y, *sprite.get_pixel(x, y));
            }
        }
    }
}
