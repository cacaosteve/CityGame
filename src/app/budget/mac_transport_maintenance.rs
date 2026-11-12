//! Mac CODE 8 road, rail, bridge, portal, and highway upkeep.
//!
//! This owns the Mac Toolbox-RNG deterioration paths and their CODE 5 bridge
//! and portal removal workers. Surface classification remains in the parent
//! budget module because terrain editing and demolition also call it.

use super::*;

pub(in crate::app) fn mac_budget_decay_road_or_rail_at(
    city: &mut City,
    x: usize,
    y: usize,
    rng: &mut MacToolboxRng,
) {
    let zone_low = city.zone_at(x, y).map(|zone| zone.raw_bits() & 0x0f);
    if zone_low != Some(0) {
        return;
    }

    let building = city.building_at(x, y);
    let Some(budget_index) = mac_transport_upkeep_budget_index(building) else {
        return;
    };
    let funding = mac_budget_service_funding_word_for_city(city, budget_index);

    // CODE 8 compares the saved WORD with 100 before calling QuickDraw. The
    // equal branch skips the random call entirely; other values reach the
    // `Random % 100` comparison. On acceptance choose rubble with a second
    // draw `(Random & 3)+1`.
    if funding == 100 {
        return;
    }
    if funding > rng.random_u16() % 100 {
        return;
    }
    let rubble = (rng.random_u16() & 3) as u8 + 1;
    city.set_building(x, y, rubble); // CODE 5 export 105 / PlaceTile.
    let bits = city.tile_bits_at(x, y).unwrap_or_default();
    city.set_tile_bits(x, y, bits.with_powerable(false)); // XBIT `&= 0x7f`.
}

/// Mac CODE 8 `export_241` bridge upkeep arm (`0x360ec`) followed through
/// CODE 5 export 80. It dispatches the `0x51..=0x5c` strip and `0x6a/0x6b`
/// extent-2 siblings separately; neither is PE `DestroyBuilding`.
pub(in crate::app) fn mac_budget_decay_bridge_at(
    city: &mut City,
    x: usize,
    y: usize,
    rng: &mut MacToolboxRng,
) {
    if city.zone_at(x, y).map(|zone| zone.raw_bits() & 0x0f) != Some(0) {
        return;
    }
    let building = city.building_at(x, y);
    if !is_mac_bridge_tile(building) && !matches!(building, 0x6a | 0x6b) {
        return;
    }

    let funding = mac_budget_service_funding_word_for_city(city, 7);
    if funding == 100 {
        return;
    }
    let wind = city.mac_state_misc_u32_at(0x64).unwrap_or(0) & 0xff;
    if !rng.roll_bridge_underfunding_decay(funding, wind) {
        return;
    }

    if matches!(building, 0x6a | 0x6b) {
        mac_budget_decay_extent2_bridge_at(city, x, y, rng, true);
        return;
    }

    mac_budget_remove_bridge_strip(city, x, y, rng, true);
}

/// CODE 5 `export_80` single-cell bridge-strip worker. Its map effects are
/// shared by the early rendered upkeep caller and the late zero-render XUND
/// caller; only the two sprite-selection draws per removed tile are conditional.
pub(in crate::app) fn mac_budget_remove_bridge_strip(
    city: &mut City,
    x: usize,
    y: usize,
    rng: &mut MacToolboxRng,
    render: bool,
) -> u32 {
    let rotated = city.tile_bits_at(x, y).unwrap_or_default().rotate();
    let (dx, dy) = if rotated { (1isize, 0isize) } else { (0, 1) };
    let Some(mut first) = mac_bridge_span_edge(city, x, y, -dx, -dy) else {
        return u32::from(render);
    };
    let mut nested_waits = mac_lower_and_classify_bridge_endpoint(city, first.0, first.1, rng);
    first = (
        first.0.checked_add_signed(dx).unwrap_or(CITY_WIDTH),
        first.1.checked_add_signed(dy).unwrap_or(CITY_HEIGHT),
    );

    let mut removed = 0usize;
    let mut cursor = first;
    while cursor.0 < CITY_WIDTH
        && cursor.1 < CITY_HEIGHT
        && is_mac_bridge_tile(city.building_at(cursor.0, cursor.1))
    {
        if render {
            // Export 80 performs row selection and a low-bit blit draw.
            rng.random_u16();
            rng.random_u16();
        }
        super::pe_place_tile_441f00(city, cursor.0, cursor.1, 0);
        let zone = city
            .zone_at(cursor.0, cursor.1)
            .map_or(0, |zone| zone.raw_bits());
        city.set_zone(cursor.0, cursor.1, zone & 0x0f);
        let bits = city.tile_bits_at(cursor.0, cursor.1).unwrap_or_default();
        city.set_tile_bits(cursor.0, cursor.1, TileBits::new(bits.raw_bits() & !0x02));
        removed += 1;
        cursor = (
            cursor.0.checked_add_signed(dx).unwrap_or(CITY_WIDTH),
            cursor.1.checked_add_signed(dy).unwrap_or(CITY_HEIGHT),
        );
    }

    if removed > 0 && cursor.0 < CITY_WIDTH && cursor.1 < CITY_HEIGHT {
        nested_waits += mac_lower_and_classify_bridge_endpoint(city, cursor.0, cursor.1, rng);
    }
    nested_waits + u32::from(render)
}

/// Mac CODE 8 `export_241` `0x360ec` → CODE 5 `export_80` extent-2 bridge
/// sibling. This is intentionally separate from the `0x51..=0x5c` strip
/// removal and from PE's `DestroyBuilding` operation.
pub(in crate::app) fn mac_budget_decay_extent2_bridge_at(
    city: &mut City,
    x: usize,
    y: usize,
    rng: &mut MacToolboxRng,
    render: bool,
) -> u32 {
    let root_building = city.building_at(x, y);
    // CODE 5 export 81 normalizes an extent-2 footprint to even X / odd Y.
    let anchor_x = x & !1;
    let anchor_y = if y & 1 == 0 { y + 1 } else { y };
    if anchor_x + 1 >= CITY_WIDTH || anchor_y >= CITY_HEIGHT || anchor_y == 0 {
        return 0;
    }

    let mut cursor = (anchor_x, anchor_y - 1);
    let first_code = mac_export180_bridge_connection_code(city, cursor.0, cursor.1);
    if first_code < 0x0d {
        return mac_budget_decay_extent2_bridge_fallback(
            city,
            anchor_x,
            anchor_y,
            root_building,
            rng,
            render,
        );
    }

    // Export 80 selects the backward search axis from the classifier's low
    // bit. Even results walk X; odd results walk Y, both in two-cell strides.
    let (step_x, step_y) = if first_code & 1 == 0 {
        (2isize, 0isize)
    } else {
        (0, 2)
    };
    while mac_export180_bridge_connection_code(city, cursor.0, cursor.1) >= 0x0d {
        let Some(next_x) = cursor.0.checked_add_signed(-step_x) else {
            break;
        };
        let Some(next_y) = cursor.1.checked_add_signed(-step_y) else {
            break;
        };
        cursor = (next_x, next_y);
    }

    // Export 80 skips this refresh for the 0x49/0x4a selector arm; on the
    // other extent-2 IDs it calls CODE 3 export 138 in this exact order.
    let mut nested_waits = 0;
    if !matches!(root_building, 0x49 | 0x4a) {
        for (dx, dy) in [(0usize, 0usize), (0, 1), (1, 1), (1, 0)] {
            let cell_x = cursor.0 + dx;
            let cell_y = cursor.1 + dy;
            if cell_x < CITY_WIDTH && cell_y < CITY_HEIGHT {
                nested_waits += mac_classify_terrain_tile(city, cell_x, cell_y, rng);
            }
        }
    }

    // Move one extent-2 stride into the connected run, then remove each
    // accepted 2×2 footprint until the topology code falls below 0x0d.
    let Some(mut run_x) = cursor.0.checked_add_signed(step_x) else {
        return 0;
    };
    let Some(mut run_y) = cursor.1.checked_add_signed(step_y) else {
        return 0;
    };
    for _ in 0..(CITY_WIDTH / 2 + CITY_HEIGHT / 2) {
        let code = mac_export180_bridge_connection_code(city, run_x, run_y);
        if code < 0x0d || run_x + 1 >= CITY_WIDTH || run_y + 1 >= CITY_HEIGHT {
            break;
        }

        // The forward refresh at 0x6c14..0x6c70 uses A5+0x5b2
        // (scene redraw bounds). The endpoint above uses A5+0x472
        // (terrain classifier). Classifying the still-live forward
        // footprint here would recursively remove the same bridge.

        // The caller supplies render flag 0xff. The base stamp consumes one
        // frame and one flip draw. Every extent-2 code adds three flips.
        if render {
            rng.random_u16();
            rng.random_u16();
        }
        mac_budget_extent2_bridge_place_and_clear(city, run_x, run_y);
        if render {
            for _ in 0..3 {
                rng.random_u16();
            }
        }
        for (dx, dy) in [(0usize, 1usize), (1, 1), (1, 0)] {
            mac_budget_extent2_bridge_place_and_clear(city, run_x + dx, run_y + dy);
        }

        let Some(next_x) = run_x.checked_add_signed(step_x) else {
            break;
        };
        let Some(next_y) = run_y.checked_add_signed(step_y) else {
            break;
        };
        run_x = next_x;
        run_y = next_y;
    }
    // CODE 5 0x7094..0x715c classifies the far 2×2 endpoint too.
    if !matches!(root_building, 0x49 | 0x4a) {
        for (dx, dy) in [(0, 0), (1, 0), (1, 1), (0, 1)] {
            nested_waits += mac_classify_terrain_tile(city, run_x + dx, run_y + dy, rng);
        }
    }
    nested_waits + u32::from(render)
}

/// CODE 13 export 180's return class used by the `0x6a/0x6b` walk.
pub(in crate::app) fn mac_export180_bridge_connection_code(city: &City, x: usize, y: usize) -> i16 {
    if x >= CITY_WIDTH || y >= CITY_HEIGHT {
        return -1;
    }
    let building = city.building_at(x, y);
    if !((0x49..=0x50).contains(&building) || (0x61..=0x6b).contains(&building)) {
        return -1;
    }
    if city
        .zone_at(x, y)
        .is_some_and(|zone| zone.raw_bits() & 0xf0 == 0xf0)
    {
        for (cell_x, cell_y) in [(x, y), (x + 1, y), (x + 1, y + 1), (x, y + 1)] {
            let neighbor = city.building_at(cell_x, cell_y);
            if (0x4b..=0x50).contains(&neighbor) {
                return i16::from(neighbor & 1);
            }
            if city
                .tile_bits_at(cell_x, cell_y)
                .is_some_and(|bits| bits.raw_bits() & 0x04 != 0)
            {
                return if neighbor == 0x49 { 0x0d } else { 0x0e };
            }
        }
        let last = city.building_at(x, y + 1);
        return if matches!(last, 0x49 | 0x4a) {
            2 + i16::from(last & 1)
        } else {
            -1
        };
    }

    let code = i16::from(building) - 0x5d;
    if code < 0x0d {
        return code;
    }
    if city
        .tile_bits_at(x, y + 1)
        .is_some_and(|bits| bits.raw_bits() & 0x02 != 0)
    {
        0x10
    } else {
        0x0f
    }
}

pub(in crate::app) fn mac_budget_decay_extent2_bridge_fallback(
    city: &mut City,
    x: usize,
    y: usize,
    root_building: u8,
    rng: &mut MacToolboxRng,
    render: bool,
) -> u32 {
    if render {
        for _ in 0..16 {
            rng.random_u16();
        }
    }
    // The fallback first places rubble according to XTER and then invokes
    // the terrain classifier on all four cells (0x7d94..0x7dfa). In gameplay
    // the classifier clears that rubble. The 0x49/0x4a skip belongs only to
    // the backward endpoint of the connected-run branch.
    mac_export80_remove_generic_rectangle(city, x, y, 2, root_building, rng);
    let mut nested_waits = 0;
    for (tx, ty) in [(x, y), (x + 1, y), (x + 1, y - 1), (x, y - 1)] {
        nested_waits += mac_classify_terrain_tile(city, tx, ty, rng);
    }
    nested_waits + if render { 2 } else { 0 }
}

pub(in crate::app) fn mac_budget_extent2_bridge_place_and_clear(
    city: &mut City,
    x: usize,
    y: usize,
) {
    super::pe_place_tile_441f00(city, x, y, 0); // CODE 5 export 105 / PlaceTile(0).
    let zone = city.zone_at(x, y).map_or(0, |zone| zone.raw_bits());
    city.set_zone(x, y, zone & 0x0f);
    let bits = city.tile_bits_at(x, y).unwrap_or_default();
    city.set_tile_bits(x, y, TileBits::new(bits.raw_bits() & 0xfd));
}

/// Mac CODE 8 `export_241` subway portal upkeep arm (`0x36156`) followed
/// through CODE 5 `export_80`. This is the Mac cardinal run removal path; PE
/// dispatches these same tile IDs through its earlier road band.
pub(in crate::app) fn mac_budget_decay_cardinal_portal_at(
    city: &mut City,
    x: usize,
    y: usize,
    rng: &mut MacToolboxRng,
) {
    if city.zone_at(x, y).map(|zone| zone.raw_bits() & 0x0f) != Some(0) {
        return;
    }
    if tunnel_portal_direction(city.building_at(x, y)).is_none() {
        return;
    }

    // Mac budget row 15 (subway), indexed from row zero as service 10.
    let funding = mac_budget_service_funding_word_for_city(city, 10);
    if funding == 100 || funding > rng.random_u16() % 100 {
        return;
    }

    let _ = mac_budget_remove_cardinal_portal_run(city, x, y, rng);
}

/// CODE 5 export 80 cardinal portal selector edit/stamp arm shared by the
/// category-0 upkeep path and the late XUND-0x23 path.
pub(in crate::app) fn mac_budget_remove_cardinal_portal_run(
    city: &mut City,
    x: usize,
    y: usize,
    rng: &mut MacToolboxRng,
) -> bool {
    let Some((dx, dy)) = tunnel_portal_direction(city.building_at(x, y)) else {
        return false;
    };
    // Export 80 clears the origin's tunnel metadata before its two redraws.
    rng.random_u16();
    rng.random_u16();
    super::pe_place_tile_441f00(city, x, y, 0);
    let zone = city.zone_at(x, y).map_or(0, |zone| zone.raw_bits());
    city.set_zone(x, y, zone & 0x0f);
    city.clear_altm_tunnel_bits(x, y);

    let mut cursor = (x as isize + dx, y as isize + dy);
    while cursor.0 >= 0
        && cursor.1 >= 0
        && cursor.0 < CITY_WIDTH as isize
        && cursor.1 < CITY_HEIGHT as isize
    {
        let cx = cursor.0 as usize;
        let cy = cursor.1 as usize;
        let next_building = city.building_at(cx, cy);
        city.clear_altm_tunnel_bits(cx, cy);
        if tunnel_portal_direction(next_building).is_some() {
            // Export 80 does two direct Random calls for each portal redraw.
            rng.random_u16();
            rng.random_u16();
            super::pe_place_tile_441f00(city, cx, cy, 0);
            let zone = city.zone_at(cx, cy).map_or(0, |zone| zone.raw_bits());
            city.set_zone(cx, cy, zone & 0x0f);
            return true;
        }
        cursor.0 += dx;
        cursor.1 += dy;
    }
    false
}

/// Mac CODE 8 highway upkeep callers (`0x361c8..0x362ac`): a funded
/// even/even anchor applies the native 2×2 water/rubble PlaceTile sequence.
pub(in crate::app) fn mac_budget_decay_highway_block_at(
    city: &mut City,
    x: usize,
    y: usize,
    rng: &mut MacToolboxRng,
) {
    // CODE 8 `export_241` enters this upkeep family only while
    // `(XZON & 0x0f) == 0`; zoned highway-looking cells stay on RCI dispatch.
    if city.zone_at(x, y).map(|zone| zone.raw_bits() & 0x0f) != Some(0) {
        return;
    }
    let building = city.building_at(x, y);
    if !mac_highway_upkeep_tile(building) || x & 1 != 0 || y & 1 != 0 {
        return;
    }
    if x + 1 >= CITY_WIDTH || y + 1 >= CITY_HEIGHT {
        return;
    }

    // Mac budget row 11 (highway), indexed from row zero as service 6.
    let funding = mac_budget_service_funding_word_for_city(city, 6);
    if funding == 100 || funding > rng.random_u16() % 100 {
        return;
    }

    for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
        let tx = x + dx;
        let ty = y + dy;
        if city.tile_bits_at(tx, ty).unwrap_or_default().water_tile() {
            city.set_building(tx, ty, 0); // CODE 5 export 105 / PlaceTile(0).
        } else {
            let rubble = (rng.random_u16() & 3) as u8 + 1;
            city.set_building(tx, ty, rubble);
        }
    }
}

pub(in crate::app) fn mac_highway_upkeep_tile(building: u8) -> bool {
    (0x49..=0x50).contains(&building) || (0x61..=0x69).contains(&building)
}

/// Mac CODE 8 upkeep callers load the saved funding field with `MOVE.W`.
/// Preserve that low-word input instead of applying the UI's 0..=100 clamp.
/// The road/rail admission gate skips its draw at exactly 100%; other upkeep
/// families keep their separately recovered gate schedule.
pub(in crate::app) fn mac_budget_service_funding_word_for_city(
    city: &City,
    budget_index: usize,
) -> u16 {
    budget_service_funding_raw_for_city(city, budget_index) as u16
}

pub(in crate::app) fn mac_bridge_span_edge(
    city: &City,
    x: usize,
    y: usize,
    step_x: isize,
    step_y: isize,
) -> Option<(usize, usize)> {
    let (mut cx, mut cy) = (x, y);
    while is_mac_bridge_tile(city.building_at(cx, cy)) {
        let nx = cx as isize + step_x;
        let ny = cy as isize + step_y;
        if nx < 0 || ny < 0 || nx >= CITY_WIDTH as isize || ny >= CITY_HEIGHT as isize {
            return None;
        }
        cx = nx as usize;
        cy = ny as usize;
    }
    Some((cx, cy))
}

pub(in crate::app) fn is_mac_bridge_tile(building: u8) -> bool {
    (0x51..=0x5c).contains(&building)
}

pub(in crate::app) fn mac_lower_and_classify_bridge_endpoint(
    city: &mut City,
    x: usize,
    y: usize,
    rng: &mut MacToolboxRng,
) -> u32 {
    let bits = city.tile_bits_at(x, y).unwrap_or_default();
    if bits.water_tile() {
        return 0;
    }

    super::pe_place_tile_441f00(city, x, y, 0); // CODE 5 export 105 / PlaceTile(0).
    let altitude = city.altm_at(x, y);
    // The original uses a WORD decrement on the packed ALTM sample. When
    // land is zero, the borrow crosses into water and tunnel bits (0x0000 ->
    // 0xffff); decrementing only the five-bit land field loses that behavior.
    let packed_altm = u16::from(altitude.land & 0x1f)
        | (u16::from(altitude.water & 0x1f) << 5)
        | (u16::from(altitude.tunnel_levels & 0x3f) << 10);
    city.set_altm_word(x, y, packed_altm.wrapping_sub(1));
    city.set_tile_bits(x, y, TileBits::new(bits.raw_bits() | 0x04));
    let nested_waits = mac_classify_terrain_tile(city, x, y, rng);
    let updated = city.tile_bits_at(x, y).unwrap_or_default();
    city.set_tile_bits(x, y, TileBits::new(updated.raw_bits() & !0x02));
    nested_waits
}

pub(in crate::app) fn mac_transport_upkeep_budget_index(building: u8) -> Option<usize> {
    match building {
        // CODE 8 `export_241 @ 0x35f18` tests these road families before its
        // rail families. The native branch at `0x35fac..0x35ff0` includes the
        // ordinary `0x1d..=0x2b` road IDs as well as crossovers, onramps, and
        // `0x5d..=0x60`; omitting the ordinary IDs drops real upkeep draws.
        0x1d..=0x2b | 0x3f..=0x46 | 0x4b..=0x4c | 0x5d..=0x60 => Some(5),
        // The `0x45..=0x48` rail compare follows the road compare; 0x45/0x46
        // therefore already take road funding, leaving 0x47/0x48 as rail.
        0x2c..=0x3e | 0x45..=0x48 | 0x4d..=0x4e | 0x6c..=0x6f => Some(8), // row 13: rail.
        _ => None,
    }
}
