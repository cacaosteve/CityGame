//! PE DestroyStructure/DestroyBuilding worker paths and footprint helpers.

use super::*;

pub(in crate::app) fn pe_flood_destroy_tile(city: &mut City, x: usize, y: usize) -> bool {
    pe_flood_destroy_tile_maybe_crt(city, None, x, y)
}

/// PE DestroyStructure with process CRT for DestroyBuilding PlaceTile rubble @ `0x45a01f`.
/// Returns false when elevated `0x3f..=0x42` early-outs before DestroyBuilding.
pub(in crate::app) fn pe_flood_destroy_tile_with_crt(
    city: &mut City,
    crt: &mut Win95CrtRand,
    x: usize,
    y: usize,
) -> bool {
    pe_flood_destroy_tile_maybe_crt(city, Some(crt), x, y)
}

pub(in crate::app) fn pe_flood_destroy_tile_maybe_crt(
    city: &mut City,
    crt: Option<&mut Win95CrtRand>,
    x: usize,
    y: usize,
) -> bool {
    pe_flood_destroy_tile_mode_maybe_crt(city, crt, x, y, true)
}

pub(in crate::app) fn pe_flood_destroy_tile_mode_maybe_crt(
    city: &mut City,
    mut crt: Option<&mut Win95CrtRand>,
    x: usize,
    y: usize,
    mode1: bool,
) -> bool {
    // PE DestroyStructure (`0x401604` → `0x461330`) calls DestroyBuilding (`0x402211`).
    // Shared clear: XTXT/XLAB, XBLD, XBIT `&= 0x3d`, XZON `&= 0x0f` (keep type, clear corners).
    // Elevated `0x3f..=0x42`: PE @ `0x46137b` returns before DestroyBuilding (keep road).
    // Size-1 nonflat → FUN_00447020 mode0 subset.
    // Highway/bridge → neighbor FUN_00447020 @ `0x45a19a`.
    // PlaceTile CRT rubble when caller supplies process CRT (DestroyStructure / bulldoze).
    let old_building = city.building_at(x, y);
    if matches!(old_building, 0x3f..=0x42) {
        return false;
    }
    // OpenSC2K DisasterDamage: record class before the footprint is cleared.
    city.record_pe_disaster_damage_class_at(x, y);
    // PE DestroyBuilding stadium arm @ `0x459f4e`/`0x459fb7`: while XBLD still live,
    // `cmp XBLD, 0xd7` then clear `wSportsTeams` bit from hit-tile XTXT microsim.
    if old_building == 215 {
        let xtxt = city.text_overlay_at(x, y).unwrap_or(0);
        city.pe_destroy_building_clear_sports_team_bit_459fb7(xtxt);
    }
    let sea = pe_selected_water_height_for_fun_00447020(city);
    if matches!(old_building, 0xdd..=0xe0) {
        // PE dispatcher `0x459888` sends DD/DE to the `0x45a238` stack walk
        // and DF/E0 to `0x45a4f9`. These special paths bypass generic footprint
        // XTXT/XLAB cleanup and the ordinary size-1 terrain tail.
        pe_destroy_building_special_connected_clear_45a238(
            city,
            crt.as_deref_mut(),
            x,
            y,
            old_building,
            mode1,
        );
        return true;
    }
    // Selector-2 highway hits with an initially high classifier use the
    // dedicated dynamic walk. Its non-exempt IDs run both helper windows;
    // each endpoint classifies low before nested DestroyBuilding, whose fixed
    // 2×2 cleanup clears the cells before its own post-cleanup helper calls.
    if pe_destroy_building_selector2_high_class_walk_run(
        city,
        crt.as_deref_mut(),
        old_building,
        x,
        y,
        mode1,
        sea,
    ) {
        return true;
    }
    let selector2_lowclass_cleanup =
        pe_destroy_building_selector2_low_class_cleanup_order(city, old_building, x, y);
    // Selector-1 bridge walk @ `0x458E0F..0x45975B` runs before generic scatter.
    // Keep it rooted to original hit IDs `0x51..=0x5C`; the live walk may cross
    // `0x6A/0x6B` continuation cells. Its body owns two mode-1 draws per live
    // support cell and clears those cells before the ordinary hit cleanup.
    if matches!(old_building, 0x51..=0x5c) {
        pe_destroy_building_selector1_support_walk_458e0f(city, crt.as_deref_mut(), x, y, mode1);
    }
    // Known generic mode-1 selector-1/2 and size-3/4 render scatter @ `0x459C40`
    // runs before PlaceTile cleanup. Selector 2 reaches this generic count only
    // when its normalized `FUN_00463C40(x,y-1)` result is below `0x0d`.
    // Selector-1 bridge-network cells have already been consumed by their
    // dynamic walk above; ordinary selector-1 classes still use this generic
    // count. PE's raw XBLD reads lack an explicit map-edge check, so the Rust
    // walk stops safely at the map boundary.
    // Selector-2 high-class calls have already returned through their dynamic
    // walker above. Selector-4 plants bypass the separate draws at
    // `0x4592A0`/`0x459318`.
    if mode1 {
        if let Some(ref mut rng) = crt {
            for _ in
                0..pe_destroy_building_mode1_scatter_draw_count_for_hit(city, old_building, x, y)
            {
                let _ = rng.random_u15();
            }
        }
    }
    // PE `0x459f02..0x459f4e` re-enables the matching civic tool after
    // destruction. Mac export 80 uses the same four building/bit pairs.
    let civic_bit = match old_building {
        0xf3 => Some(0),
        0xd0 => Some(1),
        0xdb => Some(2),
        0xff => Some(3),
        _ => None,
    };
    if let Some(bit) = civic_bit {
        city.set_rewards_available_mask(city.rewards_available_mask() | (1 << bit));
    }
    let footprint = selector2_lowclass_cleanup
        .clone()
        .unwrap_or_else(|| pe_destroy_building_footprint(city, x, y));
    let origin_x = footprint.iter().map(|&(tx, _)| tx).min().unwrap_or(x);
    let origin_y = footprint.iter().map(|&(_, ty)| ty).min().unwrap_or(y);
    for (tx, ty) in footprint {
        if city.text_overlay_at(tx, ty) == Some(0xfa)
            && !city.is_synthetic_trip_marker(tx, ty)
            && !crate::app::pe_xbld_counts_fa_network_overlay_4cc4d8(old_building)
        {
            // PE classifies every cleared marker by the captured root XBLD.
            city.pe_nonnetwork_neighbor_links = city.pe_nonnetwork_neighbor_links.wrapping_sub(1);
        }
        pe_destroy_building_clear_xtxt_45a0c4(city, tx, ty);
        // PE clears XBLD via PlaceTile (`0x441f00`) so `dwTileCount` stays coherent —
        // not a raw `set_building(0)` that skips the BSS census.
        if old_building >= 6 {
            if let Some(ref mut rng) = crt {
                pe_destroy_building_placetile_45a01f(city, rng, tx, ty);
            } else {
                pe_place_tile_441f00(city, tx, ty, 0);
            }
        } else {
            pe_place_tile_441f00(city, tx, ty, 0);
        }
        pe_destroy_building_clear_xbit_45a07f(city, tx, ty);
        let zone = city.zone_at(tx, ty).map(|z| z.raw_bits()).unwrap_or(0);
        city.set_zone(tx, ty, zone & 0x0f);
    }
    pe_destroy_building_size1_nonflat_fun_00447020_45a1ee(city, x, y, old_building, sea);
    if selector2_lowclass_cleanup.is_some() {
        pe_destroy_building_selector2_lowclass_post_cleanup_fun_00447020(
            city,
            crt,
            x,
            y,
            old_building,
            sea,
        );
    } else {
        pe_destroy_building_highway_bridge_neighbor_fun_00447020_45a19a(
            city,
            crt,
            origin_x,
            origin_y,
            old_building,
            sea,
        );
    }
    true
}

/// PE mode-1 generic render-scatter count at `0x459C40`, evaluated after
/// `FUN_0045B1C0` normalization. Selector-2 hits with classifier
/// `FUN_00463C40(x,y-1) >= 0x0D` return through the separate dynamic walker
/// before this count is consulted; low-class selector-2 hits use the generic
/// dispatcher and its ordinary size-derived scatter count.
pub(in crate::app) fn pe_destroy_building_selector2_low_class_cleanup_order(
    city: &City,
    building: u8,
    x: usize,
    y: usize,
) -> Option<Vec<(usize, usize)>> {
    if !matches!(building, 0x49..=0x50 | 0x61..=0x6b) {
        return None;
    }
    let (sx, sy) = pe_destroy_building_size2_snap_hit_45b1c0(building, x, y);
    if pe_highway_classify_463c40(city, sx as isize, sy as isize - 1) >= 0x0d {
        return None;
    }

    // PE's generic size×size cleanup at `0x459FC9..0x45A045` iterates x
    // outside and subtracts the inner counter from y. It clears every in-bounds
    // cell in this 2×2 square, even when an incomplete XBLD footprint has gaps.
    let mut tiles = Vec::with_capacity(4);
    for dx in 0..2 {
        for dy in 0..2 {
            let Some(tx) = sx.checked_add(dx).filter(|&tx| tx < CITY_WIDTH) else {
                continue;
            };
            let Some(ty) = sy.checked_sub(dy).filter(|&ty| ty < CITY_HEIGHT) else {
                continue;
            };
            tiles.push((tx, ty));
        }
    }
    Some(tiles)
}

pub(in crate::app) type PeDestroyBuildingSelector2HighClassPass = [(isize, isize); 4];

/// Bounded selector-2 high-class route for original hit IDs `0x49`/`0x4a`.
/// Those IDs skip both recursive `FUN_00447020` windows at `0x459083` and
/// `0x45975b`, so their axis/backscan/body sequence can be planned without
/// guessing recursive mutations. Selector 2 gates both walks on the classifier
/// alone; the support-XBLD bands in this function belong to selector 1.
#[cfg(test)]
pub(in crate::app) fn pe_destroy_building_selector2_high_class_walk_plan(
    city: &City,
    building: u8,
    x: usize,
    y: usize,
) -> Option<Vec<PeDestroyBuildingSelector2HighClassPass>> {
    if !matches!(building, 0x49 | 0x4a) {
        return None;
    }
    let (sx, sy) = pe_destroy_building_size2_snap_hit_45b1c0(building, x, y);
    let initial_class = pe_highway_classify_463c40(city, sx as isize, sy as isize - 1);
    if initial_class < 0x0d {
        return None;
    }

    // Selector 2 skips the selector-1 XBIT direction branch at `0x458E79`
    // and chooses this vector once from the initial class: even→X, odd→Y.
    let (dx, dy) = if initial_class & 1 == 0 {
        (1isize, 0isize)
    } else {
        (0isize, 1isize)
    };
    let mut origin_x = sx as isize;
    let mut origin_y = sy as isize - 1;
    loop {
        if pe_highway_classify_463c40(city, origin_x, origin_y) < 0x0d {
            break;
        }
        origin_x -= 2 * dx;
        origin_y -= 2 * dy;
    }

    // `0x459161..0x45918b` advances one size-two step before the first body test.
    let (mut walk_x, mut walk_y) = (origin_x + 2 * dx, origin_y + 2 * dy);
    let mut passes = Vec::new();
    while pe_highway_classify_463c40(city, walk_x, walk_y) >= 0x0d {
        passes.push([
            (walk_x, walk_y),
            (walk_x, walk_y + 1),
            (walk_x + 1, walk_y + 1),
            (walk_x + 1, walk_y),
        ]);
        walk_x += 2 * dx;
        walk_y += 2 * dy;
    }
    Some(passes)
}

/// Apply the rooted selector-2 high-class body for original hit IDs `0x49`/`0x4a`.
/// Mode 1 consumes two scatter draws before the current cell, then one before
/// each of the three remaining cells; PlaceTile and XZON/XBIT writes follow PE
/// order. This body does not clear XTXT/XLAB or call `FUN_00447020`.
pub(in crate::app) fn pe_destroy_building_selector2_high_class_walk_apply(
    city: &mut City,
    mut crt: Option<&mut Win95CrtRand>,
    passes: &[PeDestroyBuildingSelector2HighClassPass],
    mode1: bool,
) -> Vec<(usize, usize)> {
    let mut touched = Vec::new();
    for cells in passes {
        if mode1 {
            if let Some(rng) = crt.as_deref_mut() {
                for _ in 0..2 {
                    let _ = rng.random_u15();
                }
            }
        }
        for (index, &(x, y)) in cells.iter().enumerate() {
            if mode1 && index != 0 {
                if let Some(rng) = crt.as_deref_mut() {
                    let _ = rng.random_u15();
                }
            }
            if !(0..CITY_WIDTH as isize).contains(&x) || !(0..CITY_HEIGHT as isize).contains(&y) {
                continue;
            }
            let (x, y) = (x as usize, y as usize);
            pe_place_tile_441f00(city, x, y, 0);
            let zone = city.zone_at(x, y).map(|z| z.raw_bits()).unwrap_or(0);
            city.set_zone(x, y, zone & 0x0f);
            if let Some(bits) = city.tile_bits_at(x, y) {
                city.set_tile_bits(x, y, TileBits::new(bits.raw_bits() & 0xfd));
            }
            touched.push((x, y));
        }
    }
    touched
}

/// Execute selector-2's high-class highway walk in PE order. The two helper
/// windows are deliberately interleaved with live classification: their
/// `FUN_00447020` calls can take the low-class 2×2 cleanup and change which
/// cells the subsequent walk admits.
pub(in crate::app) fn pe_destroy_building_selector2_high_class_walk_run(
    city: &mut City,
    mut crt: Option<&mut Win95CrtRand>,
    building: u8,
    x: usize,
    y: usize,
    mode1: bool,
    sea: i16,
) -> bool {
    if !matches!(building, 0x49..=0x50 | 0x61..=0x6b) {
        return false;
    }
    let (sx, sy) = pe_destroy_building_size2_snap_hit_45b1c0(building, x, y);
    let mut origin_x = sx as isize;
    let mut origin_y = sy as isize - 1;
    let initial_class = pe_highway_classify_463c40(city, origin_x, origin_y);
    if initial_class < 0x0d {
        return false;
    }

    // Selector 2 chooses its size-two axis once from the initial class:
    // even→X, odd→Y. Both backscan and forward scan then reclassify live cells.
    let (dx, dy) = if initial_class & 1 == 0 {
        (1isize, 0isize)
    } else {
        (0isize, 1isize)
    };
    while pe_highway_classify_463c40(city, origin_x, origin_y) >= 0x0d {
        origin_x -= 2 * dx;
        origin_y -= 2 * dy;
    }

    let helper_windows = !matches!(building, 0x49 | 0x4a);
    if helper_windows {
        pe_destroy_building_selector2_high_class_helper_window(
            city,
            crt.as_deref_mut(),
            origin_x,
            origin_y,
            sea,
        );
    }

    let (mut walk_x, mut walk_y) = (origin_x + 2 * dx, origin_y + 2 * dy);
    loop {
        if pe_highway_classify_463c40(city, walk_x, walk_y) < 0x0d {
            break;
        }
        let pass = [[
            (walk_x, walk_y),
            (walk_x, walk_y + 1),
            (walk_x + 1, walk_y + 1),
            (walk_x + 1, walk_y),
        ]];
        pe_destroy_building_selector2_high_class_walk_apply(city, crt.as_deref_mut(), &pass, mode1);
        walk_x += 2 * dx;
        walk_y += 2 * dy;
    }

    if helper_windows {
        pe_destroy_building_selector2_high_class_helper_window(city, crt, walk_x, walk_y, sea);
    }
    true
}

/// One of the selector-2 pre-walk / walk-exit windows. The origin is the
/// low-class coordinate left by the classifier scan; every helper call is the
/// full mode-1 DestroyBuilding plus mode-0 slope/sea sequence.
pub(in crate::app) fn pe_destroy_building_selector2_high_class_helper_window(
    city: &mut City,
    mut crt: Option<&mut Win95CrtRand>,
    origin_x: isize,
    origin_y: isize,
    sea: i16,
) {
    for (x, y) in [
        (origin_x, origin_y),
        (origin_x + 1, origin_y),
        (origin_x + 1, origin_y + 1),
        (origin_x, origin_y + 1),
    ] {
        if !(0..CITY_WIDTH as isize).contains(&x) || !(0..CITY_HEIGHT as isize).contains(&y) {
            continue;
        }
        let (x, y) = (x as usize, y as usize);
        pe_fun_00447020_mode1_destroy_and_city_clear_447085(city, crt.as_deref_mut(), x, y);
        pe_fun_00447020_mode0_xund_slope_sea(city, x, y, sea);
    }
}

pub(in crate::app) fn pe_destroy_building_mode1_scatter_draw_count_for_hit(
    city: &City,
    building: u8,
    x: usize,
    y: usize,
) -> usize {
    let size = pe_destroy_building_size(building);
    if size != 2 || matches!(building, 0xdd..=0xe0) {
        return crate::city::pe_destroy_building_mode1_render_scatter_draw_count(building);
    }

    let (normalized_x, normalized_y) = pe_destroy_building_size2_snap_hit_45b1c0(building, x, y);
    if pe_highway_classify_463c40(city, normalized_x as isize, normalized_y as isize - 1) < 0x0d {
        2 * size * size
    } else {
        0
    }
}

/// PE `FUN_00458D40`'s connected special-tile cleanup worklist.
///
/// Runway/cross-runway `0xdd..=0xde` cells use the `0x45a238` arm and consume
/// one rubble draw plus two render draws per popped record in mode 1. The
/// `0xdf..=0xe0` family uses the `0x45a4f9` arm, clears with PlaceTile(0), and
/// consumes two render draws per popped record in mode 1. Both arms reset and
/// seed the shared 512-entry PE stack (`0x413cc0` / `0x413cf0`), pop LIFO
/// (`0x413db0`), clear XZON/XBIT, and push matching cardinal neighbors in
/// west/east/north/south order. A full stack drops its oldest pending record.
pub(in crate::app) fn pe_destroy_building_special_connected_clear_45a238(
    city: &mut City,
    mut crt: Option<&mut Win95CrtRand>,
    x: usize,
    y: usize,
    building: u8,
    mode1: bool,
) {
    const PENDING_LIMIT: usize = 0x1ff;
    let dd_de_family = matches!(building, 0xdd..=0xde);
    debug_assert!(dd_de_family || matches!(building, 0xdf..=0xe0));

    let mut pending = vec![(x, y)];
    while let Some((cx, cy)) = pending.pop() {
        if dd_de_family {
            // PE @ 0x45a298 always consumes `rand() & 3 + 1` in both modes.
            let rubble = crt
                .as_deref_mut()
                .map(Win95CrtRand::rubble_tile_id)
                .unwrap_or(0);
            pe_place_tile_441f00(city, cx, cy, rubble);
        } else {
            // PE @ 0x45a561: the DF/E0 family uses PlaceTile(0), not rubble.
            pe_place_tile_441f00(city, cx, cy, 0);
        }

        let zone = city.zone_at(cx, cy).map(|z| z.raw_bits()).unwrap_or(0);
        city.set_zone(cx, cy, zone & 0x0f);
        pe_destroy_building_clear_xbit_45a07f(city, cx, cy);

        if mode1 {
            if let Some(rng) = crt.as_deref_mut() {
                // The two values feed native sprite blitters at 0x45a32a /
                // 0x45a39a (DD/DE) or 0x45a5e1 / 0x45a6b0 (DF/E0). CityGame
                // does not reproduce those temporary graphics, but must retain
                // their process-CRT chronology.
                let _ = rng.random_u15();
                let _ = rng.random_u15();
            }
        }

        let neighbors = [
            (cx as isize - 1, cy as isize),
            (cx as isize + 1, cy as isize),
            (cx as isize, cy as isize - 1),
            (cx as isize, cy as isize + 1),
        ];
        for (nx, ny) in neighbors {
            if !(0..CITY_WIDTH as isize).contains(&nx) || !(0..CITY_HEIGHT as isize).contains(&ny) {
                continue;
            }
            let (nx, ny) = (nx as usize, ny as usize);
            let neighbor = city.building_at(nx, ny);
            let matches_family = if dd_de_family {
                matches!(neighbor, 0xdd..=0xde)
            } else {
                matches!(neighbor, 0xdf..=0xe0)
            };
            if matches_family {
                // FUN_00413cf0 advances the 9-bit tail. If this fills all 512
                // slots, it advances the head, keeping 511 pending records.
                if pending.len() == PENDING_LIMIT {
                    pending.remove(0);
                }
                pending.push((nx, ny));
            }
        }
    }
}

/// PE DestroyBuilding footprint XBIT clear @ `0x45a07f`: `XBIT &= 0x3d`
/// (clear rotate `0x02`, powered `0x40`, powerable `0x80`; keep watered/piped/water-tile).
pub(in crate::app) fn pe_destroy_building_clear_xbit_45a07f(city: &mut City, x: usize, y: usize) {
    let Some(bits) = city.tile_bits_at(x, y) else {
        return;
    };
    city.set_tile_bits(x, y, TileBits::new(bits.raw_bits() & 0x3d));
}

/// PE DestroyBuilding elevated `0x3f..=0x42` arm @ `0x4599aa`: `ALTM &= 0x3ff`.
pub(in crate::app) fn pe_destroy_building_clear_altm_tunnel_4599aa(
    city: &mut City,
    x: usize,
    y: usize,
) {
    city.clear_altm_tunnel_bits(x, y);
}

/// PE DestroyBuilding elevated arm @ `0x45994e`–`0x4599aa` (skip UI `0x4012c1`):
/// PlaceTile(0), XZON `&= 0x0f`, ALTM `&= 0x3ff`. No XTXT/XBIT clear / FUN_00447020.
pub(in crate::app) fn pe_destroy_building_elevated_arm_45994e(city: &mut City, x: usize, y: usize) {
    pe_place_tile_441f00(city, x, y, 0);
    let zone = city.zone_at(x, y).map(|z| z.raw_bits()).unwrap_or(0);
    city.set_zone(x, y, zone & 0x0f);
    pe_destroy_building_clear_altm_tunnel_4599aa(city, x, y);
}

/// PE DestroyBuilding mode 0/1 shared elevated / `< 6` / `>= 6` arms with optional
/// process CRT rubble (`0x45a01f`). Elevated `0x3f..=0x42` take the elevated arm;
/// XBLD `< 6` early-outs (`0x458d8a`) are a no-op. Mode 1 has extra highway-band
/// gates at `0x458218` that these call sites do not need.
pub(in crate::app) fn pe_destroy_building_optional_crt(
    city: &mut City,
    crt: Option<&mut Win95CrtRand>,
    x: usize,
    y: usize,
    mode1: bool,
) {
    let building = city.building_at(x, y);
    if matches!(building, 0x3f..=0x42) {
        pe_destroy_building_elevated_arm_45994e(city, x, y);
    } else if building >= 6 {
        pe_flood_destroy_tile_mode_maybe_crt(city, crt, x, y, mode1);
    }
}

/// PE DestroyBuilding mode 0 (`0x401611` → `0x458140`, push 0) with optional CRT.
pub(in crate::app) fn pe_destroy_building_mode0_optional_crt(
    city: &mut City,
    crt: Option<&mut Win95CrtRand>,
    x: usize,
    y: usize,
) {
    pe_destroy_building_optional_crt(city, crt, x, y, false);
}

/// PE DestroyBuilding mode 1 (`0x402211` → `0x458d40` / `0x401611` push 1) with
/// optional process CRT rubble (`0x45a01f`).
pub(in crate::app) fn pe_destroy_building_mode1_optional_crt(
    city: &mut City,
    crt: Option<&mut Win95CrtRand>,
    x: usize,
    y: usize,
) {
    pe_destroy_building_optional_crt(city, crt, x, y, true);
}

/// PE DestroyBuilding XTXT/XLAB arm @ `0x45a0c4`–`0x45a106`:
/// skip empty and `>= 0xf1` except `0xfa`; else `0x401dca` and clear XTXT when
/// `< 0xc9` or `== 0xfa`.
pub(in crate::app) fn pe_destroy_building_clear_xtxt_45a0c4(city: &mut City, x: usize, y: usize) {
    let xtxt = city.text_overlay_at(x, y).unwrap_or(0);
    // PE @ `0x45a0e1`/`0x45a0e8`: `xtxt >= 0xf1 && xtxt != 0xfa` → skip whole arm.
    if xtxt == 0 || (xtxt >= 0xf1 && xtxt != 0xfa) {
        return;
    }
    if xtxt < 0xc9 || xtxt == 0xfa {
        city.set_text_overlay(x, y, 0);
    }
    pe_clear_xtxt_sign_label(city, xtxt);
}

/// PE DestroyBuilding footprint PlaceTile @ `0x45a01f`–`0x45a02e`:
/// XTER == 0 → CRT `rand()&3+1` rubble; else PlaceTile(0).
pub(in crate::app) fn pe_destroy_building_placetile_45a01f(
    city: &mut City,
    crt: &mut Win95CrtRand,
    x: usize,
    y: usize,
) {
    let tile = if city.terrain_at(x, y) == 0 {
        crt.rubble_tile_id()
    } else {
        0
    };
    pe_place_tile_441f00(city, x, y, tile);
}

/// PE DestroyBuilding size-1 non-flat post-clear @ `0x45a1ee`–`0x45a22b`:
/// when size==1, saved XBLD is `6..0x70`, and XTER ≠ 0 → `FUN_00447020` (`0x402bd0`).
/// `DestroyBuilding` itself returns early for XBLD `< 6`; keep that caller gate
/// here because this helper is also used by the Rust bulldoze emulation path.
pub(in crate::app) fn pe_destroy_building_size1_nonflat_fun_00447020_45a1ee(
    city: &mut City,
    x: usize,
    y: usize,
    old_building: u8,
    sea: i16,
) {
    if old_building < 6 || pe_destroy_building_size(old_building) != 1 || old_building >= 0x70 {
        return;
    }
    if city.terrain_at(x, y) == 0 {
        return;
    }
    pe_fun_00447020_mode1_destroy_and_city_clear_447085(city, None, x, y);
    pe_fun_00447020_mode0_xund_slope_sea(city, x, y, sea);
}

/// Legacy approximation for selector-2's high-class helper windows in the
/// non-Win95 fallback route. Ordinary Win95 player `bulldoze_at` returns
/// through the recovered DestroyBuilding path before reaching this helper.
/// The PE DestroyBuilding core uses
/// `pe_destroy_building_selector2_high_class_walk_run` to interleave these
/// phase- and endpoint-dependent windows with live classification. The
/// low-class route's separate `0x45a19a` post-cleanup window is handled by
/// `pe_destroy_building_selector2_lowclass_post_cleanup_fun_00447020`.
///
/// Each native helper is full `FUN_00447020`: PE @ `0x447085` calls
/// DestroyBuilding mode 1 for target XBLD `>= 0x0d`, then runs slope/XUND/sea
/// arms. Do not treat this player-tool approximation as high-class map/CRT
/// parity.
pub(in crate::app) fn pe_destroy_building_highway_bridge_neighbor_fun_00447020_45a19a(
    city: &mut City,
    mut crt: Option<&mut Win95CrtRand>,
    origin_x: usize,
    origin_y: usize,
    old_building: u8,
    sea: i16,
) {
    if !matches!(old_building, 0x49..=0x50 | 0x61..=0x6b) {
        return;
    }
    // This is a temporary Rust approximation, not one native PE neighbor set.
    // Selector-2 high-class calls have a pre-walk batch at `0x459083` and a
    // dynamic walk-exit batch at `0x45975b`; initial low-class calls bypass both,
    // clear the generic size×size area, then reach the four calls at `0x45a19a`.
    // The three groups are phase- and route-specific; do not use this static union
    // as evidence of Win95 map or CRT parity.
    let neighbors: &[(isize, isize)] = if matches!(old_building, 0x49 | 0x4a) {
        &[
            (origin_x as isize, origin_y as isize),
            (origin_x as isize + 1, origin_y as isize),
            (origin_x as isize + 1, origin_y as isize - 1),
            (origin_x as isize, origin_y as isize - 1),
        ]
    } else {
        &[
            (origin_x as isize, origin_y as isize),
            (origin_x as isize + 1, origin_y as isize),
            (origin_x as isize + 1, origin_y as isize - 1),
            (origin_x as isize, origin_y as isize - 1),
            (origin_x as isize, origin_y as isize + 1),
            (origin_x as isize + 1, origin_y as isize + 1),
            (origin_x as isize, origin_y as isize + 2),
            (origin_x as isize + 1, origin_y as isize + 2),
        ]
    };
    for &(nx, ny) in neighbors {
        if !(0..CITY_WIDTH as isize).contains(&nx) || !(0..CITY_HEIGHT as isize).contains(&ny) {
            continue;
        }
        let (nx, ny) = (nx as usize, ny as usize);
        pe_fun_00447020_mode1_destroy_and_city_clear_447085(city, crt.as_deref_mut(), nx, ny);
        pe_fun_00447020_mode0_xund_slope_sea(city, nx, ny, sea);
    }
}

/// Exact `0x45A19A` window for a selector-2 call that took the generic
/// low-class route. The high-class walker bypasses this window entirely.
pub(in crate::app) fn pe_destroy_building_selector2_lowclass_post_cleanup_fun_00447020(
    city: &mut City,
    mut crt: Option<&mut Win95CrtRand>,
    x: usize,
    y: usize,
    old_building: u8,
    sea: i16,
) {
    if !matches!(old_building, 0x49..=0x50 | 0x61..=0x6b) {
        return;
    }
    let (sx, sy) = pe_destroy_building_size2_snap_hit_45b1c0(old_building, x, y);
    let sx = sx as isize;
    let sy = sy as isize;
    for (nx, ny) in [(sx, sy), (sx + 1, sy), (sx + 1, sy - 1), (sx, sy - 1)] {
        if !(0..CITY_WIDTH as isize).contains(&nx) || !(0..CITY_HEIGHT as isize).contains(&ny) {
            continue;
        }
        let (nx, ny) = (nx as usize, ny as usize);
        pe_fun_00447020_mode1_destroy_and_city_clear_447085(city, crt.as_deref_mut(), nx, ny);
        pe_fun_00447020_mode0_xund_slope_sea(city, nx, ny, sea);
    }
}

pub(in crate::app) fn pe_destroy_building_support_xbld_band(building: u8) -> bool {
    matches!(building, 0x51..=0x5c | 0x6a | 0x6b)
}

/// Win95 `0x401604(x,y,1,1)`: clear footprint XBLD/XTXT, then dry tiles get fire paint `0xff`.
pub(in crate::app) fn pe_destroy_structure_fire_paint(
    city: &mut City,
    crt: Option<&mut Win95CrtRand>,
    x: usize,
    y: usize,
) {
    // PE DestroyStructure @ `0x46137b`: elevated `0x3f..=0x42` returns before
    // DestroyBuilding and before the fire-paint footprint loop.
    if matches!(city.building_at(x, y), 0x3f..=0x42) {
        return;
    }
    // Capture footprint before DestroyStructure clear (buildings are zero afterward).
    let footprint = pe_destroy_building_footprint(city, x, y);
    // PE DestroyStructure → DestroyBuilding CRT PlaceTile rubble @ `0x45a01f`.
    pe_flood_destroy_tile_maybe_crt(city, crt, x, y);
    for (tx, ty) in footprint {
        if !city
            .tile_bits_at(tx, ty)
            .is_some_and(|bits| bits.water_tile())
        {
            city.set_text_overlay(tx, ty, 0xff);
        }
    }
}

/// Win95 `0x45b1c0` / `0x4e85d8`: highway 2×2, else `multi_tile_building_size` for XBLD `>= 0x70`.
pub(in crate::app) fn pe_destroy_building_size(building: u8) -> usize {
    match building {
        0x49..=0x50 | 0x61..=0x6b => 2,
        b if b < 0x70 => 1,
        other => usize::from(crate::city::multi_tile_building_size(other)).max(1),
    }
}

/// PE DestroyBuilding size-2 hit snap @ `0x45b1ef`–`0x45b1fa` for highway bands
/// `0x49..=0x50` / `0x61..=0x6b` only: `x &= ~1`, `y = (y & ~1) + 1`. Other sizes
/// (incl. table-sized 2×2) leave the hit unchanged.
pub(in crate::app) fn pe_destroy_building_size2_snap_hit_45b1c0(
    building: u8,
    x: usize,
    y: usize,
) -> (usize, usize) {
    if !matches!(building, 0x49..=0x50 | 0x61..=0x6b) {
        return (x, y);
    }
    (x & !1, (y & !1).saturating_add(1))
}

/// Win95 `0x402496` origin walk: size×size block of the same XBLD containing the hit tile.
pub(in crate::app) fn pe_destroy_building_footprint(
    city: &City,
    x: usize,
    y: usize,
) -> Vec<(usize, usize)> {
    let building = city.building_at(x, y);
    if building == 0 {
        return vec![(x, y)];
    }
    let size = pe_destroy_building_size(building);
    if size <= 1 {
        return vec![(x, y)];
    }
    // PE @ `0x45b1c0` snaps the hit before the size×size walk; search from the
    // snapped tile so odd hits still resolve the same 2×2.
    let hit = (x, y);
    let (sx, sy) = pe_destroy_building_size2_snap_hit_45b1c0(building, x, y);
    let min_x = sx.saturating_sub(size - 1);
    let min_y = sy.saturating_sub(size - 1);
    for origin_y in min_y..=sy {
        for origin_x in min_x..=sx {
            let Some(tiles) = building_footprint_from_origin(origin_x, origin_y, size) else {
                continue;
            };
            if tiles.contains(&(sx, sy))
                && tiles
                    .iter()
                    .all(|&(tx, ty)| city.building_at(tx, ty) == building)
            {
                return tiles;
            }
        }
    }
    vec![hit]
}
