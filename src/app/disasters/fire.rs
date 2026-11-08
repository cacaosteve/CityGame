use super::*;

pub(in crate::app) fn pe_fire_tick_active(city: &City) -> bool {
    matches!(
        city.pe_w_current_disaster_type(),
        0x01 | 0x06 | 0x09 | 0x0b | 0x0c
    )
}

/// PE overlay scan `0x45f7c8` matches XTXT `0xff` with no disaster-type gate.
/// Type-family tick still runs with no burning tile; a live `0xff` (riot ignite,
/// leftover marker) is enough while that tile implies `dwDisasterActive`.
pub(in crate::app) fn pe_fire_overlay_scan_due(city: &City) -> bool {
    if pe_fire_tick_active(city) {
        return true;
    }
    for x in 0..CITY_WIDTH {
        for y in 0..CITY_HEIGHT {
            if city.text_overlay_at(x, y) == Some(0xff) {
                return true;
            }
        }
    }
    false
}

pub(in crate::app) fn pe_fire_tick_seed(city: &City) -> u32 {
    // Offline LFSR helper seed; CRT rolls use process stream on the live tick path.
    let mut seed = 0x45F7_C800u32;
    for byte in city.name.bytes() {
        seed = seed.wrapping_mul(33).wrapping_add(u32::from(byte));
    }
    seed
}

pub(in crate::app) fn pe_fire_coverage_cell(city: &City, x: usize, y: usize) -> u8 {
    city.fire_coverage_at(x, y).unwrap_or(0)
}

/// Win95 `0x401cad(4)+1` then `0x40178f`: XBLD rubble after fire/flood/dispatch.
pub(in crate::app) fn pe_fire_rubble_building(lfsr: &mut Win95Lfsr) -> u8 {
    (lfsr.random_mod(4) + 1) as u8
}

pub(in crate::app) fn pe_fire_apply_rubble(
    city: &mut City,
    lfsr: &mut Win95Lfsr,
    x: usize,
    y: usize,
) {
    city.set_text_overlay(x, y, 0);
    pe_place_tile_441f00(city, x, y, pe_fire_rubble_building(lfsr));
}

/// PE fire arm 5 plant epilogue @ `0x45fa4e`–`0x45fb7b`: after extinguish, XBLD
/// `0x85`/`0x9f`/`0xbc` spill `0xfb` via `0x402dc9` from footprint NW origin.
/// Size rings: always origin; `>1` → N/NE/E of origin; `>2` → further N/NE/E.
pub(in crate::app) fn pe_fire_burn_down_plant_fb_spill(
    city: &mut City,
    origin_x: usize,
    origin_y: usize,
    size: usize,
) {
    let paint = |city: &mut City, x: isize, y: isize| {
        if x < 0 || y < 0 {
            return;
        }
        let (x, y) = (x as usize, y as usize);
        if x < CITY_WIDTH && y < CITY_HEIGHT {
            pe_fun_00461190_paint_fb_if_low(city, x, y);
        }
    };
    let ox = origin_x as isize;
    let oy = origin_y as isize;
    paint(city, ox, oy);
    if size > 1 {
        paint(city, ox, oy - 1);
        paint(city, ox + 1, oy - 1);
        paint(city, ox + 1, oy);
    }
    if size > 2 {
        paint(city, ox, oy - 2);
        paint(city, ox + 1, oy - 2);
        paint(city, ox + 2, oy - 2);
        paint(city, ox + 2, oy - 1);
        paint(city, ox + 2, oy);
    }
}

/// PE fire jump-table arm 5 @ `0x45f9f6`: `0x401604(x,y,1,1)` then
/// `0x402b85` extinguish (`FUN_004611e0`) on the hit tile; `0x4025e5`
/// (`LFSR & 0xf == 0`) may call `FUN_00453280` with `(height,state,saved_b)=(0,0,1)`.
/// Plant IDs then spill `0xfb` via `FUN_00461190` (hit XTXT is low after extinguish).
pub(in crate::app) fn pe_fire_burn_down_tile(
    city: &mut City,
    crt: &mut Win95CrtRand,
    lfsr: &mut Win95Lfsr,
    x: usize,
    y: usize,
) {
    // PE saves XBLD before DestroyStructure for the `0x85`/`0x9f`/`0xbc` gate.
    let building = city.building_at(x, y);
    let footprint = pe_destroy_building_footprint(city, x, y);
    let (origin_x, origin_y) = footprint_origin(&footprint).unwrap_or((x, y));
    let size = pe_destroy_building_size(building);
    pe_destroy_structure_fire_paint(city, Some(crt), x, y);
    // PE @ `0x45fa2c`: `0x402b85` → extinguish, not bare LFSR rubble.
    let _ = pe_dispatch_extinguish_fire(city, crt, lfsr, x, y);
    if lfsr.random_mask_0f() == 0 {
        let _ = city.place_pe_explosion_actor(x, y, 0, 0, 1);
    }
    // PE @ `0x45fa4e`: plant IDs → `0x402496` origin + `0x402dc9` `0xfb` rings.
    if matches!(building, 0x85 | 0x9f | 0xbc) {
        pe_fire_burn_down_plant_fb_spill(city, origin_x, origin_y, size);
    }
}

/// Win95 `0x45f7c8` XTXT `0xff` arm: 1-in-4 act, water recedes, `rand&7` jump table `0x460744`.
/// Act / table / coverage rolls are CRT; rubble is LFSR `Random(4)+1`.
pub(in crate::app) fn pe_fire_tick_city(city: &mut City) -> PeFireTickStats {
    let seed = pe_fire_tick_seed(city);
    let mut crt = Win95CrtRand::new(seed);
    let mut lfsr = Win95Lfsr::from_u32_seed(seed);
    pe_fire_tick_city_with_rng(city, &mut crt, &mut lfsr)
}

pub(in crate::app) fn pe_fire_tick_city_with_rng(
    city: &mut City,
    crt: &mut Win95CrtRand,
    lfsr: &mut Win95Lfsr,
) -> PeFireTickStats {
    let mut stats = PeFireTickStats::default();
    for x in 0..CITY_WIDTH {
        for y in 0..CITY_HEIGHT {
            pe_fire_tick_cell_with_rng(city, crt, lfsr, x, y, &mut stats);
        }
    }
    stats
}

pub(in crate::app) fn pe_fire_tick_cell_with_rng(
    city: &mut City,
    crt: &mut Win95CrtRand,
    lfsr: &mut Win95Lfsr,
    x: usize,
    y: usize,
    stats: &mut PeFireTickStats,
) {
    if city.text_overlay_at(x, y) != Some(0xff) {
        return;
    }
    if crt.random_u16() & 3 != 0 {
        return;
    }
    if city.tile_bits_at(x, y).is_some_and(TileBits::water_tile) {
        city.set_text_overlay(x, y, 0);
        stats.receded += 1;
        return;
    }
    match usize::from(crt.random_u16() & 7) {
        dir @ 0..=3 => {
            let Some((nx, ny)) =
                offset_city_tile(x, y, PE_FLOOD_SPREAD_DX[dir], PE_FLOOD_SPREAD_DY[dir])
            else {
                return;
            };
            // PE NESW arms → `0x401bf4` flag 0 directly; `test eax` counts paint success.
            if pe_fun_00460e10_ignite(city, Some(crt), lfsr, nx, ny, 0) {
                stats.ignited += 1;
            }
        }
        4 | 6 | 7 => {
            // PE jump table `0x460744`: values 4 and the `>5` default both enter
            // `0x45f830`, so rand&7 values 4/6/7 share the coverage/destruction arm.
            let threshold = u16::from(pe_fire_coverage_cell(city, x, y)).saturating_add(8);
            if crt.random_u16() & 0xff < threshold {
                if city.building_at(x, y) != 0 && !matches!(city.building_at(x, y), 0x3f..=0x42) {
                    stats.destroyed += 1;
                }
                pe_destroy_structure_fire_paint(city, Some(crt), x, y);
                let _ = pe_dispatch_extinguish_fire(city, crt, lfsr, x, y);
                stats.receded += 1;
            }
        }
        5 => {
            if city.building_at(x, y) >= 0x70 {
                pe_fire_burn_down_tile(city, crt, lfsr, x, y);
                stats.destroyed += 1;
            }
        }
        _ => unreachable!("fire arm is masked to 0..=7"),
    }
}
