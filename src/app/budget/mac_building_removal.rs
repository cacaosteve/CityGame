//! Mac CODE 5 export-80 building removal and footprint helpers.
//!
//! This module owns the bounded map-edit core for generic footprints,
//! connected runway/coastal groups, and the special civic cleanup arms. Shared
//! surface classification stays in `budget.rs`; bridge and portal workers
//! remain with Mac transport upkeep.

use super::*;

/// CODE 5 export 80's directly recovered map edits and QuickDraw sites.
/// The classifier calls this with rendering enabled even in editor mode.
/// Rendering callbacks/event timing are separate from this deterministic core.
pub(in crate::app) fn mac_destroy_building_export80(
    city: &mut City,
    x: usize,
    y: usize,
    rng: &mut MacToolboxRng,
    render: bool,
) -> u32 {
    if x >= CITY_WIDTH || y >= CITY_HEIGHT {
        return 0;
    }
    let building = city.building_at(x, y);
    if building < 6 {
        return 0;
    }
    if mac_cardinal_portal_vector(building).is_some() {
        // Portal endpoint stamps draw regardless of the render flag.
        let removed = mac_budget_remove_cardinal_portal_run(city, x, y, rng);
        u32::from(render && removed)
    } else if is_mac_bridge_tile(building) {
        mac_budget_remove_bridge_strip(city, x, y, rng, render)
    } else if matches!(building, 0x49..=0x50 | 0x61..=0x6b) {
        mac_budget_decay_extent2_bridge_at(city, x, y, rng, render)
    } else if matches!(building, 0xdd..=0xe0) {
        mac_export80_remove_connected_component(city, x, y, rng, building < 0xdf, render);
        u32::from(render)
    } else if let Some(extent) = mac_export80_generic_extent(building) {
        let anchor = if extent >= 3 {
            mac_export81_align_large_generic_anchor(city, x, y, extent)
        } else {
            Some((x, y))
        };
        let Some((anchor_x, anchor_y)) = anchor else {
            return 0;
        };
        if render {
            // Three nested extent loops precede all generic map writes.
            for _ in 0..(2 * extent * extent * extent) {
                rng.random_u16();
            }
        }
        mac_export80_update_special_civic_latch(city, building);
        mac_export80_update_stadium_choice_mask(city, building, anchor_x, anchor_y);
        let nested_waits = if extent == 1 {
            mac_export80_remove_generic_single(
                city,
                anchor_x,
                anchor_y,
                building,
                rng,
                building < 0x70,
            )
        } else {
            mac_export80_remove_generic_rectangle(city, anchor_x, anchor_y, extent, building, rng);
            0
        };
        nested_waits + if render { extent as u32 } else { 0 }
    } else {
        0
    }
}

/// CODE 5 export 80's bounded LIFO component core. Runways spend a rubble
/// draw per pop; coastal cells use PlaceTile(0). Rendering adds two stamp
/// draws after each map write, including duplicate pending coordinates.
fn mac_export80_remove_connected_component(
    city: &mut City,
    root_x: usize,
    root_y: usize,
    rng: &mut MacToolboxRng,
    runway: bool,
    render: bool,
) {
    const PENDING_LIMIT: usize = 511;
    let mut pending = VecDeque::with_capacity(PENDING_LIMIT);
    pending.push_back((root_x, root_y));

    while let Some((x, y)) = pending.pop_back() {
        if runway {
            let rubble = (rng.random_u16() as u8 & 3) + 1;
            super::pe_place_tile_441f00(city, x, y, rubble); // CODE 5 export 105 / PlaceTile.
        } else {
            super::pe_place_tile_441f00(city, x, y, 0); // CODE 5 export 105 / PlaceTile(0).
        }

        let zone = city.zone_at(x, y).map_or(0, |zone| zone.raw_bits());
        city.set_zone(x, y, zone & 0x0f);
        let bits = city.tile_bits_at(x, y).unwrap_or_default();
        city.set_tile_bits(x, y, TileBits::new(bits.raw_bits() & 0x3d));

        if render {
            rng.random_u16();
            rng.random_u16();
        }

        // CODE 5 checks these four cardinal cells in this order and calls
        // CODE 14 export 212 for each still-family XBLD value.
        let neighbors = [
            x.checked_sub(1).map(|nx| (nx, y)),
            (x + 1 < CITY_WIDTH).then_some((x + 1, y)),
            y.checked_sub(1).map(|ny| (x, ny)),
            (y + 1 < CITY_HEIGHT).then_some((x, y + 1)),
        ];
        for (nx, ny) in neighbors.into_iter().flatten() {
            let neighbor = city.building_at(nx, ny);
            let same_family = if runway {
                matches!(neighbor, 0xdd | 0xde)
            } else {
                matches!(neighbor, 0xdf | 0xe0)
            };
            if same_family {
                if pending.len() == PENDING_LIMIT {
                    pending.pop_front(); // native export 212 advances the tail
                }
                pending.push_back((nx, ny));
            }
        }
    }
}

/// CODE 5 `export_80` removes the selected 0xd7 XMIC choice bit from the
/// per-city `fffffe02` mask before walking an extent-4 stadium footprint.
/// Mac CODE 4 serializes that word at MISC `0x1028` (slot 1034), which is the
/// same saved role as PE's sports-team mask. The Mac runtime applies a 68k
/// WORD shift and synchronizes the shared saved field through the city codec.
fn mac_export80_update_stadium_choice_mask(
    city: &mut City,
    building: u8,
    anchor_x: usize,
    anchor_y: usize,
) {
    if building != 0xd7 {
        return;
    }
    let Some(xtxt) = city.text_overlay_at(anchor_x, anchor_y) else {
        return;
    };
    if !(0x33..=0xc8).contains(&xtxt) {
        return;
    }
    let Some(record) = city.microsim_at(usize::from(xtxt - 0x33)) else {
        return;
    };
    if record.tile_id != 0xd7 {
        return;
    }

    // The 68k executes `mask -= 1 << (choice & 0x3f)` as a WORD operation.
    // A 68k register shift masks its count to six bits but a WORD result is
    // zero for counts >= 16; Rust's wrapping_shl would incorrectly wrap those
    // counts modulo 16. Valid stadium choices are 0..=4, but preserve the
    // native behavior for malformed/stale save records too.
    let choice = u32::from(record.stat2 & 0x3f);
    let bit = 1_u16.checked_shl(choice).unwrap_or(0);
    let mask = city.mac_state_misc_u32_at(0x1028).unwrap_or(0) as u16;
    let next = mask.wrapping_sub(bit);
    city.set_mac_state_misc_u32_at(0x1028, next as i16 as u32);
}

/// Generic export-80 extents, excluding the dedicated selector branches.
/// The ordering of dedicated branches above this query protects portal,
/// bridge, airport/runway, and topology-specific export-80 paths. Special
/// civic selectors still share the generic rectangle but also update the
/// separate latch helper below.
pub(in crate::app) fn mac_export80_generic_extent(building: u8) -> Option<usize> {
    if (0x06..0x70).contains(&building)
        || matches!(building, 0x70..=0x8b | 0xc6..=0xc8 | 0xdc | 0xe1..=0xea)
        || building == 0xdb
    {
        Some(1)
    } else if matches!(building, 0x8c..=0xad | 0xeb..=0xf7) {
        Some(2)
    } else if matches!(building, 0xae..=0xc5 | 0xd0..=0xd6 | 0xf8..=0xfa) {
        Some(3)
    } else if matches!(building, 0xc9..=0xcf | 0xd7..=0xda | 0xfb..=0xff) {
        Some(4)
    } else {
        None
    }
}

/// Whether CODE 5 export 80 dispatches this root through its generic per-cell
/// loop, whose XTXT-0xfa arm decrements the saved Mac overlay counter. Keep the
/// dedicated portal, bridge, topology, and connected-component branches out.
pub(in crate::app) fn mac_export80_uses_generic_cell_cleanup(building: u8) -> bool {
    mac_cardinal_portal_vector(building).is_none()
        && !is_mac_bridge_tile(building)
        && !matches!(building, 0x49..=0x50 | 0x61..=0x6b | 0xdd..=0xe0)
        && mac_export80_generic_extent(building).is_some()
}

/// Export 80 updates CODE 4's persisted `ffffdc74` civic/tool latch when it
/// clears one of the four special civic selector buildings. The associated
/// Toolbox menu refresh is a UI side effect; the saved latch bit is modeled.
fn mac_export80_update_special_civic_latch(city: &mut City, building: u8) {
    let Some(index) = (match building {
        0xf3 => Some(0),
        0xd0 => Some(1),
        0xdb => Some(2),
        0xff => Some(3),
        _ => None,
    }) else {
        return;
    };
    let latch = city.mac_state_misc_u32_at(0x78).unwrap_or(0);
    city.set_mac_state_misc_u32_at(0x78, latch | (1 << index));
}

/// CODE 5 export 81 aligns the origin for table-backed extents 3 and 4 by
/// walking the orientation-indexed XZON corner markers around the candidate,
/// then resolving the marker on the candidate tile itself. Native probes are
/// sequential: a matched marker adjusts the coordinates used by later probes.
/// Keep off-map probes guarded: native export 81 uses signed unchecked row and
/// byte indexing, and its adjacent pointer tables can alias XBLD/XTXT at edges;
/// export 80 also walks the resulting footprint without map-bound checks. That
/// spill behavior is not modeled pending original-Mac validation.
pub(in crate::app) fn mac_export81_align_large_generic_anchor(
    city: &City,
    mut x: usize,
    mut y: usize,
    extent: usize,
) -> Option<(usize, usize)> {
    let markers = match city.saved_rotation() & 3 {
        0 => [0x10, 0x20, 0x40, 0x80],
        1 => [0x20, 0x40, 0x80, 0x10],
        2 => [0x40, 0x80, 0x10, 0x20],
        _ => [0x80, 0x10, 0x20, 0x40],
    };
    let is_marker = |px: usize, py: usize, marker: u8| {
        city.zone_at(px, py)
            .is_some_and(|zone| zone.raw_bits() & 0xf0 == marker)
    };
    let shift = |value: usize, delta: isize| value.checked_add_signed(delta);

    // Marker 0: northwest-diagonal, north, west. Coordinates are intentionally
    // updated between these checks, matching export 81's in-place WORD writes.
    if let (Some(px), Some(py)) = (shift(x, -1), shift(y, -1))
        && is_marker(px, py, markers[0])
    {
        x = px;
        y = py;
    }
    if let Some(py) = shift(y, -1)
        && is_marker(x, py, markers[0])
    {
        y = py;
    }
    if let Some(px) = shift(x, -1)
        && is_marker(px, y, markers[0])
    {
        x = px;
    }

    // Marker 1: northeast-diagonal, north, east.
    if let (Some(px), Some(py)) = (shift(x, 1), shift(y, -1))
        && is_marker(px, py, markers[1])
    {
        x = px;
        y = py;
    }
    if let Some(py) = shift(y, -1)
        && is_marker(x, py, markers[1])
    {
        y = py;
    }
    if let Some(px) = shift(x, 1)
        && is_marker(px, y, markers[1])
    {
        x = px;
    }

    // Marker 2: southeast-diagonal, south, east.
    if let (Some(px), Some(py)) = (shift(x, 1), shift(y, 1))
        && is_marker(px, py, markers[2])
    {
        x = px;
        y = py;
    }
    if let Some(py) = shift(y, 1)
        && is_marker(x, py, markers[2])
    {
        y = py;
    }
    if let Some(px) = shift(x, 1)
        && is_marker(px, y, markers[2])
    {
        x = px;
    }

    // Marker 3: southwest-diagonal, south, west.
    if let (Some(px), Some(py)) = (shift(x, -1), shift(y, 1))
        && is_marker(px, py, markers[3])
    {
        x = px;
        y = py;
    }
    if let Some(py) = shift(y, 1)
        && is_marker(x, py, markers[3])
    {
        y = py;
    }
    if let Some(px) = shift(x, -1)
        && is_marker(px, y, markers[3])
    {
        x = px;
    }

    match city.zone_at(x, y)?.raw_bits() & 0xf0 {
        marker if marker == markers[0] => y = y.checked_add(extent - 1)?,
        marker if marker == markers[1] => {
            y = y.checked_add(extent - 1)?;
            x = x.checked_sub(extent - 1)?;
        }
        marker if marker == markers[2] => x = x.checked_sub(extent - 1)?,
        _ => {}
    }
    Some((x, y))
}

/// CODE 5 `export_80`'s per-cell generic map cleanup. `export_81`
/// returns extent 1 directly for selectors below `0x70`, and its initialized
/// table returns extent 1 for the selected safe ranges above `0x70`. The stock
/// `0xe9` station is one of these. The caller handles the render draws before
/// this XTER-based PlaceTile, marker masks, XTXT cleanup, and counter update.
fn mac_export80_remove_generic_single(
    city: &mut City,
    x: usize,
    y: usize,
    root_building: u8,
    rng: &mut MacToolboxRng,
    classify: bool,
) -> u32 {
    city.mac_export80_replace_generic_cell(x, y, root_building, rng);
    // CODE 5 export 80 calls export 138 once for extent-1 selectors below
    // 0x70. This includes the 0x6d lifted-road selector and the newly covered
    // generic extent-1 selectors.
    if classify {
        mac_classify_terrain_tile(city, x, y, rng)
    } else {
        0
    }
}

/// Generic rectangle cells are visited in x+column, y-row order.
pub(in crate::app) fn mac_export80_remove_generic_rectangle(
    city: &mut City,
    x: usize,
    y: usize,
    extent: usize,
    root_building: u8,
    rng: &mut MacToolboxRng,
) {
    if x + extent > CITY_WIDTH || y + 1 < extent {
        return;
    }
    for column in 0..extent {
        for row in 0..extent {
            mac_export80_remove_generic_single(
                city,
                x + column,
                y - row,
                root_building,
                rng,
                false,
            );
        }
    }
}

#[cfg(test)]
mod late_xund_selector_tests {
    use super::mac_export80_generic_extent;

    #[test]
    fn mac68k_late_xund_generic_extent_fallback_matches_export81_bands() {
        for raw in 0_u16..=u16::from(u8::MAX) {
            let tile = raw as u8;
            let expected = match tile {
                0x06..=0x6f => Some(1),
                0x70..=0x8b => Some(1),
                0x8c..=0xad => Some(2),
                0xae..=0xc5 => Some(3),
                0xc6..=0xc8 => Some(1),
                0xc9..=0xcf => Some(4),
                0xd0..=0xd6 => Some(3),
                0xd7..=0xda => Some(4),
                0xdb..=0xdc => Some(1),
                // These four selectors leave the generic fallback for the
                // connected runway/coastal workers in the caller.
                0xdd..=0xe0 => None,
                0xe1..=0xea => Some(1),
                0xeb..=0xf7 => Some(2),
                0xf8..=0xfa => Some(3),
                0xfb..=0xff => Some(4),
                _ => None,
            };
            assert_eq!(
                mac_export80_generic_extent(tile),
                expected,
                "late XUND fallback extent for XBLD {tile:#04x}"
            );
        }
    }
}
