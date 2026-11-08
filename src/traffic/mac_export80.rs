//! Mac `export_80` traffic-family stamping and projected road geometry.

use super::*;

pub(super) fn traffic_overlay_stamp_at(
    city: &crate::city::City,
    x: usize,
    y: usize,
    rng: &mut crate::mac_random::MacToolboxRng,
) -> Option<TrafficOverlay> {
    if x >= crate::city::CITY_WIDTH || y >= crate::city::CITY_HEIGHT {
        return None;
    }
    let rotation = city.rotation();
    let building = crate::tiles::rotated_building_id(city.building_at(x, y), rotation);
    if !mac_export80_stamp_building(building) {
        return None;
    }

    // Mac FUN_0000884e calls export_80(row, col, -1); the nonzero traffic argument enables
    // static dash sites for eligible road branches without an XTRF congestion gate.
    // export_80 selects the static dash row with `(Random & 3)`; road orientation is already
    // represented by the road tile, so it must not select a Win95 density-table ID.
    let sprite_id = mac_static_traffic_sprite_with_variant(rng.random_u3());
    Some(TrafficOverlay {
        sprite_id,
        density: TrafficDensity::Light,
        lane: 0,
        blit_flip: rng.random_bit(),
    })
}

pub fn traffic_overlay_with_random_blit_flip(
    overlay: TrafficOverlay,
    rng: &mut crate::mac_random::MacToolboxRng,
) -> TrafficOverlay {
    TrafficOverlay {
        blit_flip: rng.random_bit(),
        ..overlay
    }
}

pub fn traffic_overlay_with_random_variant_and_blit_flip(
    overlay: TrafficOverlay,
    rng: &mut crate::mac_random::MacToolboxRng,
) -> TrafficOverlay {
    TrafficOverlay {
        sprite_id: mac_static_traffic_sprite_for_sprite_family(overlay.sprite_id, rng.random_u3()),
        blit_flip: rng.random_bit(),
        ..overlay
    }
}

/// Stamp sequence for the Mac `export_80` road branch.
///
/// A `D5 == 2` structure chooses its dash row once, then reuses that row for the three
/// additional diamond stamps while consuming a fresh `Random & 1` blit argument at each site.
pub fn mac_export80_stamp_sequence(
    building: u8,
    overlay: TrafficOverlay,
    rng: &mut crate::mac_random::MacToolboxRng,
) -> Vec<(TrafficOverlay, Vec2)> {
    if let Some(offsets) = export80_special_endpoint_offsets(building) {
        // `export_80` reaches a second, independent stamp site after walking this endpoint.
        return vec![
            (overlay, offsets[0]),
            (
                traffic_overlay_with_random_variant_and_blit_flip(overlay, rng),
                offsets[1],
            ),
        ];
    }

    let offsets = if is_multi_cell_road(building) {
        export80_multi_stamp_offsets().to_vec()
    } else {
        vec![Vec2::ZERO]
    };
    offsets
        .into_iter()
        .enumerate()
        .map(|(index, offset)| {
            let stamp = if index == 0 {
                overlay
            } else {
                traffic_overlay_with_random_blit_flip(overlay, rng)
            };
            (stamp, offset)
        })
        .collect()
}

/// City-aware Mac `export_80` stamp sequence.
///
/// The `0x3f..0x42` branch stamps its entry endpoint, advances by the cardinal direction encoded
/// by that endpoint through intervening tiles, then stamps the first endpoint it reaches. The
/// other Mac branches retain the existing fixed footprint sequence.
pub fn mac_export80_stamp_sequence_for_city(
    city: &crate::city::City,
    x: usize,
    y: usize,
    building: u8,
    overlay: TrafficOverlay,
    rng: &mut crate::mac_random::MacToolboxRng,
) -> Vec<(TrafficOverlay, Vec2)> {
    let Some(endpoint_offsets) = export80_special_endpoint_offsets(building) else {
        return mac_export80_stamp_sequence(building, overlay, rng);
    };

    let step = endpoint_offsets[1];
    let (dx, dy): (isize, isize) = match building {
        63 => (-1, 0),
        64 => (0, -1),
        65 => (1, 0),
        66 => (0, 1),
        _ => unreachable!("endpoint offsets already validated"),
    };
    let mut stamps = Vec::new();
    stamps.push((overlay, Vec2::ZERO));
    let (mut current_x, mut current_y) = (x, y);
    let mut distance = 0usize;

    loop {
        let Some(next_x) = current_x.checked_add_signed(dx) else {
            break;
        };
        let Some(next_y) = current_y.checked_add_signed(dy) else {
            break;
        };
        if next_x >= crate::city::CITY_WIDTH || next_y >= crate::city::CITY_HEIGHT {
            break;
        }
        current_x = next_x;
        current_y = next_y;
        distance += 1;

        let next_building = crate::tiles::rotated_building_id(
            city.building_at(current_x, current_y),
            city.rotation(),
        );
        if matches!(next_building, 63..=66) {
            stamps.push((
                traffic_overlay_with_random_variant_and_blit_flip(overlay, rng),
                step * distance as f32,
            ));
            break;
        }
    }

    stamps
}

/// Mac `D5==2` extra stamp offsets from `export_80` @ `0x06d8c`, `0x06dd4`, `0x06e1c`.
///
/// Values are in this viewer's unscaled screen-space map units. The caller scales by the active
/// view zoom when converting to the final sprite anchor. The scale is the projection shift
/// (`ffffe0c9`), not the sprite-row bank (`ffffe0c8`).
pub fn export80_multi_stamp_offsets_for_projection_shift(zoom_shift: u8) -> [Vec2; 4] {
    let source_scale = (1u32 << zoom_shift.min(2)) as f32;
    let step_x = MAC_EXPORT80_DIAMOND_BASE_STEP_X * source_scale;
    let step_y = MAC_EXPORT80_DIAMOND_BASE_STEP_Y * source_scale;
    [
        Vec2::ZERO,
        Vec2::new(step_x, -step_y),
        Vec2::new(step_x * 2.0, 0.0),
        Vec2::new(step_x, step_y),
    ]
}

pub fn export80_multi_stamp_offsets() -> [Vec2; 4] {
    export80_multi_stamp_offsets_for_projection_shift(MAC_EXPORT80_VIEWER_PROJECTION_SHIFT)
}

/// Mac `0x3f..0x42` branch endpoint vector from `export_80 @ 0x76e8..0x7942`.
///
/// The branch stamps the traffic family at the current endpoint, then follows an explicit
/// row/column direction derived from the tile ID, skipping intervening tile types until the
/// first endpoint. This is the recovered static-renderer search direction; callers still need
/// to resolve target-tile elevation and view-rotation anchoring for exact native placement.
pub fn export80_special_endpoint_offsets(building: u8) -> Option<[Vec2; 2]> {
    let (row_delta, col_delta) = match building {
        63 => (-1.0, 0.0),
        64 => (0.0, -1.0),
        65 => (1.0, 0.0),
        66 => (0.0, 1.0),
        _ => return None,
    };
    Some([
        Vec2::ZERO,
        export80_projected_delta(row_delta, col_delta, MAC_EXPORT80_VIEWER_PROJECTION_SHIFT),
    ])
}

fn export80_projected_delta(row_delta: f32, col_delta: f32, zoom_shift: u8) -> Vec2 {
    let source_scale = (1u32 << zoom_shift.min(2)) as f32;
    let half_width = MAC_EXPORT80_DIAMOND_BASE_STEP_X * source_scale;
    let half_height = MAC_EXPORT80_DIAMOND_BASE_STEP_Y * source_scale;
    Vec2::new(
        (row_delta - col_delta) * half_width,
        (row_delta + col_delta) * half_height,
    )
}
