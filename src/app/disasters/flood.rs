use super::*;

pub(in crate::app) fn pe_flood_seed(city: &City, x: usize, y: usize) -> u32 {
    let mut seed = 0x45DB_0000u32 ^ (x as u32).rotate_left(8) ^ (y as u32);
    for byte in city.name.bytes() {
        seed = seed.wrapping_mul(33).wrapping_add(u32::from(byte));
    }
    seed
}

pub(in crate::app) fn pe_flood_xter_ready(terrain: u8) -> bool {
    (0x20..0x30).contains(&terrain)
}

pub(in crate::app) fn pe_flood_can_paint(city: &City, x: usize, y: usize) -> bool {
    !city
        .tile_bits_at(x, y)
        .is_some_and(|bits| bits.water_tile())
}

/// Win95 type-2 start @ `0x45db00`: Chebyshev radius from trigger until XTER `0x20..0x30`.
pub(in crate::app) fn pe_flood_source_tile(
    city: &City,
    origin_x: isize,
    origin_y: isize,
) -> Option<(usize, usize)> {
    for radius in 0..0x80_isize {
        let mut di = -radius;
        while di <= radius {
            let mut bp = -radius;
            while bp <= radius {
                let x = origin_x + di;
                let y = origin_y + bp;
                if (0..CITY_WIDTH as isize).contains(&x)
                    && (0..CITY_HEIGHT as isize).contains(&y)
                    && pe_flood_xter_ready(city.terrain_at(x as usize, y as usize))
                {
                    return Some((x as usize, y as usize));
                }
                bp += 1;
            }
            di += 1;
        }
    }
    None
}

pub(in crate::app) fn pe_flood_neighbor_overlays(
    city: &City,
    origin_x: isize,
    origin_y: isize,
    found_x: usize,
    found_y: usize,
) -> std::collections::HashMap<(usize, usize), u8> {
    let di = found_x as isize - origin_x;
    let bp = found_y as isize - origin_y;
    let mut tiles = std::collections::HashMap::new();
    let mut paint = |dx: isize, dy: isize| {
        if let Some((x, y)) = offset_city_tile(found_x, found_y, dx, dy) {
            if pe_flood_can_paint(city, x, y) {
                tiles.insert((x, y), 0xfc);
            }
        }
    };
    if di > 0 {
        paint(-1, 0);
    }
    if bp > 0 {
        paint(0, -1);
    }
    if di < 0x7f {
        paint(1, 0);
    }
    if bp < 0x7f {
        paint(0, 1);
    }
    tiles
}

/// Win95 type-2 neighbor paints @ `0x45dc03+`: write XTXT `0xfc` on the map live.
pub(in crate::app) fn pe_flood_neighbor_paint_live(
    city: &mut City,
    origin_x: isize,
    origin_y: isize,
    found_x: usize,
    found_y: usize,
) -> std::collections::HashMap<(usize, usize), u8> {
    let tiles = pe_flood_neighbor_overlays(city, origin_x, origin_y, found_x, found_y);
    for (&(x, y), &marker) in &tiles {
        city.set_text_overlay(x, y, marker);
    }
    tiles
}

/// Win95 flood fallback @ `0x45db00`: `0x402b3f` RandomAnd7F until XTER `0`.
pub(in crate::app) fn pe_flood_fallback_tile(
    city: &City,
    lfsr: &mut Win95Lfsr,
) -> Option<(usize, usize)> {
    for _ in 0..0xc8 {
        let x = usize::from(lfsr.random_mask_7f());
        let y = usize::from(lfsr.random_mask_7f());
        if x < CITY_WIDTH && y < CITY_HEIGHT && city.terrain_at(x, y) == 0 {
            return Some((x, y));
        }
    }
    None
}

pub(in crate::app) fn pe_flood_overlays_with_lfsr(
    city: &City,
    origin_x: usize,
    origin_y: usize,
    lfsr: &mut Win95Lfsr,
) -> std::collections::HashMap<(usize, usize), u8> {
    if let Some((found_x, found_y)) =
        pe_flood_source_tile(city, origin_x as isize, origin_y as isize)
    {
        return pe_flood_neighbor_overlays(
            city,
            origin_x as isize,
            origin_y as isize,
            found_x,
            found_y,
        );
    }
    let mut tiles = std::collections::HashMap::new();
    if let Some(tile) = pe_flood_fallback_tile(city, lfsr) {
        tiles.insert(tile, 0xfc);
    }
    tiles
}

/// Win95 type-2 @ `0x45db00`: return the chosen source and paint `0xfc` live.
/// A valid shoreline is a successful source even if no neighbor is painted.
pub(in crate::app) fn pe_flood_paint_with_lfsr(
    city: &mut City,
    origin_x: isize,
    origin_y: isize,
    lfsr: &mut Win95Lfsr,
) -> (
    Option<(usize, usize)>,
    std::collections::HashMap<(usize, usize), u8>,
) {
    if let Some(source @ (sx, sy)) = pe_flood_source_tile(city, origin_x, origin_y) {
        let tiles = pe_flood_neighbor_paint_live(city, origin_x, origin_y, sx, sy);
        return (Some(source), tiles);
    }
    let source = pe_flood_fallback_tile(city, lfsr);
    let mut tiles = std::collections::HashMap::new();
    if let Some((x, y)) = source {
        city.set_text_overlay(x, y, 0xfc);
        tiles.insert((x, y), 0xfc);
    }
    (source, tiles)
}

pub(in crate::app) fn pe_flood_overlays(
    city: &City,
    origin_x: usize,
    origin_y: usize,
) -> std::collections::HashMap<(usize, usize), u8> {
    let mut lfsr = Win95Lfsr::from_u32_seed(pe_flood_seed(city, origin_x, origin_y));
    pe_flood_overlays_with_lfsr(city, origin_x, origin_y, &mut lfsr)
}

/// Win95 type-2 `0x4019ec` after `0x45db00`: goto XTER source, else fallback empty XTER.
pub(in crate::app) fn pe_flood_goto_tile_with_lfsr(
    city: &City,
    origin_x: usize,
    origin_y: usize,
    lfsr: &mut Win95Lfsr,
) -> Option<(usize, usize)> {
    if let Some(source) = pe_flood_source_tile(city, origin_x as isize, origin_y as isize) {
        return Some(source);
    }
    pe_flood_fallback_tile(city, lfsr)
}

pub(in crate::app) fn pe_flood_goto_tile(
    city: &City,
    origin_x: usize,
    origin_y: usize,
) -> Option<(usize, usize)> {
    let mut lfsr = Win95Lfsr::from_u32_seed(pe_flood_seed(city, origin_x, origin_y));
    pe_flood_goto_tile_with_lfsr(city, origin_x, origin_y, &mut lfsr)
}

pub(in crate::app) fn pe_flood_duration_tick_active(city: &City) -> bool {
    matches!(city.pe_w_current_disaster_type(), 0x02 | 0x0e | 0x10)
}

/// PE overlay scan `0x460195` matches XTXT `0xfc` with no disaster-type gate.
pub(in crate::app) fn pe_flood_overlay_scan_due(city: &City) -> bool {
    pe_flood_duration_tick_active(city) || pe_overlay_marker_present(city, &[0xfc])
}

pub(in crate::app) fn pe_flood_tick_seed(city: &City, duration: u16) -> u32 {
    // Offline LFSR helper seed; CRT rolls use process stream on the live tick path.
    let mut seed = 0x45F7_6000u32 ^ u32::from(duration);
    for byte in city.name.bytes() {
        seed = seed.wrapping_mul(33).wrapping_add(u32::from(byte));
    }
    seed
}

/// Win95 `0x461000`: downhill or flat, not XTER `0x10..0x20`;
/// XTXT `0..=0x32` overlay, `0x33..=0xc8` destroy, `0xf1..=0xf9` rubble.
pub(in crate::app) fn pe_flood_neighbor_paint_kind(
    city: &City,
    x: usize,
    y: usize,
    src_alt: u8,
) -> Option<PeFloodNeighborAction> {
    if (city.altitude_at(x, y) & 0x1f) > src_alt {
        return None;
    }
    if (0x10..0x20).contains(&city.terrain_at(x, y)) {
        return None;
    }
    let xtxt = city.text_overlay_at(x, y).unwrap_or(0);
    match xtxt {
        0..=0x32 => Some(PeFloodNeighborAction::OverlayOnly),
        0x33..=0xc8 => Some(PeFloodNeighborAction::Destroy),
        0xf1..=0xf9 => Some(PeFloodNeighborAction::Rubble),
        _ => None,
    }
}

/// Win95 `0x45f760` XTXT `0xfc` arm: recede / slow-skip / destroy / NESW `0x461000`.
/// Overlay rolls use CRT `0x48cdd8`; rubble `Random(4)+1` uses LFSR `0x401cad`.
pub(in crate::app) fn pe_flood_tick_city(city: &mut City, duration: u16) -> PeFloodTickStats {
    let seed = pe_flood_tick_seed(city, duration);
    let mut crt = Win95CrtRand::new(seed);
    let mut lfsr = Win95Lfsr::from_u32_seed(seed);
    pe_flood_tick_city_with_rng(city, duration, &mut crt, &mut lfsr)
}

pub(in crate::app) fn pe_flood_tick_city_with_rng(
    city: &mut City,
    duration: u16,
    crt: &mut Win95CrtRand,
    lfsr: &mut Win95Lfsr,
) -> PeFloodTickStats {
    let mut stats = PeFloodTickStats::default();
    for x in 0..CITY_WIDTH {
        for y in 0..CITY_HEIGHT {
            pe_flood_tick_cell_with_rng(city, duration, crt, lfsr, x, y, &mut stats);
        }
    }
    stats
}

pub(in crate::app) fn pe_flood_tick_cell_with_rng(
    city: &mut City,
    duration: u16,
    crt: &mut Win95CrtRand,
    lfsr: &mut Win95Lfsr,
    x: usize,
    y: usize,
    stats: &mut PeFloodTickStats,
) {
    if city.text_overlay_at(x, y) != Some(0xfc) {
        return;
    }
    // PE @ `0x4601a7..0x4601c6`: zero duration calls `0x402946`, the Win95 LFSR
    // low-bit helper; bit 1 clears this marker before the CRT slow/decay gates.
    if duration == 0 && lfsr.random_word_lsb() != 0 {
        city.set_text_overlay(x, y, 0);
        stats.receded += 1;
        return;
    }
    if duration < PE_FLOOD_TICK_SLOW_AT && crt.random_u16() & 3 != 0 {
        return;
    }
    if duration < PE_FLOOD_TICK_DESTROY_AT && crt.random_u16() & 3 == 0 {
        if city.building_at(x, y) >= 0x70 {
            // PE @ `0x460208`: DestroyStructure(0,0) → CRT PlaceTile rubble; no explode.
            if pe_flood_destroy_tile_mode_maybe_crt(city, Some(crt), x, y, false) {
                stats.destroyed += 1;
            }
        }
        city.set_text_overlay(x, y, 0);
        stats.receded += 1;
    }
    if duration == 0 {
        return;
    }
    let dir = usize::from(crt.random_u16() & 3);
    let Some((nx, ny)) = offset_city_tile(x, y, PE_FLOOD_SPREAD_DX[dir], PE_FLOOD_SPREAD_DY[dir])
    else {
        return;
    };
    let src_alt = city.altitude_at(x, y) & 0x1f;
    match pe_flood_neighbor_paint_kind(city, nx, ny, src_alt) {
        None => {}
        Some(PeFloodNeighborAction::Rubble) => {
            // PE @ `0x4610df`: `0x401cad(4)+1` then `0x40178f` PlaceTile; leave XTXT.
            pe_place_tile_441f00(city, nx, ny, pe_fire_rubble_building(lfsr));
            stats.rubble += 1;
        }
        Some(PeFloodNeighborAction::OverlayOnly) => {
            // Win95 `0x46107e`: XTXT `1..=0x32` → `0x401dca` then paint `0xfc`.
            if let Some(xtxt) = city.text_overlay_at(nx, ny) {
                pe_clear_xtxt_sign_label(city, xtxt);
            }
            city.set_text_overlay(nx, ny, 0xfc);
            pe_clear_xtrf_microsim64(city, nx, ny);
            stats.spread += 1;
        }
        Some(PeFloodNeighborAction::Destroy) => {
            // PE @ `0x4610c3`: DestroyStructure(x,y,1,0) — flag_a=1 explode via
            // DestroyBuilding; flag_b=0 skips fire `0xff`. Elevated early-out skips
            // DestroyBuilding (no explode) but caller still paints `0xfc` @ `0x46108d`.
            if pe_flood_destroy_tile_with_crt(city, crt, nx, ny) {
                stats.destroyed += 1;
                stats.explodes += 1;
            }
            city.set_text_overlay(nx, ny, 0xfc);
            pe_clear_xtrf_microsim64(city, nx, ny);
            stats.spread += 1;
        }
    }
}
