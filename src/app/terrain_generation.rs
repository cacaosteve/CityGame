use crate::city::{AltmSample, CITY_HEIGHT, CITY_TILES, CITY_WIDTH, City, TileBits};
use crate::mac_random::{MacGameLcgRng, MacToolboxRng};
use crate::ui::{self, EditMapSliderValues};
use crate::win95_random::{Win95CrtRand, Win95GameRand};

/// Win95 `FUN_00410950` height-plane stages through recursive slope relaxation
/// (`0x4476a0`, `0x447740`, `0x4479e0`, `0x446e60`). The returned plane uses
/// City's x-major indexing. The shared editor/PlaceWater pipeline runs in
/// `generate_win95`; the app injects persistent random streams, whose original
/// startup chronology remains approximate.
pub(super) fn generate_win95_height_plane(
    hills: u8,
    water: u8,
    ocean_enabled: bool,
    river_enabled: bool,
    rng: &mut Win95CrtRand,
    game_rng: &mut Win95GameRand,
) -> (Vec<u16>, Vec<u8>) {
    let hills = u16::from(hills);
    let modulus = hills + 11;
    let outside_height = hills + 10;
    let mut heights = vec![0u16; CITY_TILES];
    let mut xbit = vec![0u8; CITY_TILES];

    // PE `0x4476a0`: the worker increments its Hills argument by ten, then
    // passes that value plus one: `rand() % (Hills + 11) + 1`.
    for lattice_x in 0..8 {
        for lattice_y in 0..8 {
            let x = lattice_x * 16;
            let y = lattice_y * 16;
            heights[city_index(x, y)] = rng.random_u15() % modulus + 1;
        }
    }

    // PE `0x447740`: four diamond-square-like refinements. When one
    // coordinate is aligned, average the two neighboring edge samples; when
    // neither is aligned, average the four diagonal samples.
    for (step, alignment_mask) in [(8usize, 15usize), (4, 7), (2, 3), (1, 1)] {
        for x in (0..CITY_WIDTH).step_by(step) {
            for y in (0..CITY_HEIGHT).step_by(step) {
                if x & alignment_mask == 0 && y & alignment_mask == 0 {
                    continue;
                }
                let average = if x & alignment_mask == 0 {
                    (u32::from(height_sample(
                        &heights,
                        x as isize,
                        y as isize - step as isize,
                        outside_height,
                        ocean_enabled,
                    )) + u32::from(height_sample(
                        &heights,
                        x as isize,
                        y as isize + step as isize,
                        outside_height,
                        ocean_enabled,
                    ))) / 2
                } else if y & alignment_mask == 0 {
                    (u32::from(height_sample(
                        &heights,
                        x as isize - step as isize,
                        y as isize,
                        outside_height,
                        ocean_enabled,
                    )) + u32::from(height_sample(
                        &heights,
                        x as isize + step as isize,
                        y as isize,
                        outside_height,
                        ocean_enabled,
                    ))) / 2
                } else {
                    (u32::from(height_sample(
                        &heights,
                        x as isize - step as isize,
                        y as isize - step as isize,
                        outside_height,
                        ocean_enabled,
                    )) + u32::from(height_sample(
                        &heights,
                        x as isize + step as isize,
                        y as isize - step as isize,
                        outside_height,
                        ocean_enabled,
                    )) + u32::from(height_sample(
                        &heights,
                        x as isize - step as isize,
                        y as isize + step as isize,
                        outside_height,
                        ocean_enabled,
                    )) + u32::from(height_sample(
                        &heights,
                        x as isize + step as isize,
                        y as isize + step as isize,
                        outside_height,
                        ocean_enabled,
                    ))) / 4
                };
                let height = average as u16 + rng.random_u15() % step as u16;
                heights[city_index(x, y)] = height.max(1);
            }
        }
    }

    let target = ((u16::from(water) + 4) >> 3).max(u16::from(ocean_enabled || river_enabled) * 4);
    apply_win95_mode_bands(
        &mut heights,
        &mut xbit,
        target,
        ocean_enabled,
        river_enabled,
        game_rng,
    );

    // PE `0x4479e0` is a synchronous scratch-plane smoothing pass. It is
    // called after stages 2 and 3.
    smooth_win95_height_plane(&mut heights);
    smooth_win95_height_plane(&mut heights);

    // PE `0x410b79..0x410bc4`: packed-height conversion, followed by stage 4
    // and its third smoothing pass.
    for height in &mut heights {
        let transformed = ((*height as i32 + 3) >> 1) - 4;
        *height = if transformed >= 4 {
            transformed as u16
        } else if transformed >= 0 {
            4
        } else {
            (transformed + 4) as u16
        };
    }
    smooth_win95_height_plane(&mut heights);

    // PE `0x446e90`: recursive cardinal slope relaxation. This is deliberately
    // in-place and scan-order-sensitive, matching the original worker.
    for x in 0..CITY_WIDTH {
        for y in 0..CITY_HEIGHT {
            relax_win95_slope(&mut heights, x, y);
        }
    }
    (heights, xbit)
}

fn city_index(x: usize, y: usize) -> usize {
    x * CITY_HEIGHT + y
}

fn height_sample(
    heights: &[u16],
    x: isize,
    y: isize,
    outside_height: u16,
    ocean_enabled: bool,
) -> u16 {
    if x < 0 || y < 0 || y >= CITY_HEIGHT as isize {
        return outside_height;
    }
    if x >= CITY_WIDTH as isize {
        return if ocean_enabled { 0 } else { outside_height };
    }
    heights[city_index(x as usize, y as usize)]
}

fn apply_win95_mode_bands(
    heights: &mut [u16],
    xbit: &mut [u8],
    target: u16,
    ocean_enabled: bool,
    river_enabled: bool,
    game_rng: &mut Win95GameRand,
) {
    if ocean_enabled {
        let mut offset = game_rng.random_mod(10) + 10;
        for y in 0..CITY_HEIGHT {
            let x_a = 128 - usize::from(offset);
            let x_b = x_a + 1;
            heights[city_index(x_a, y)] =
                (heights[city_index(x_a, y)] & !0x1f) | target.saturating_sub(2);
            heights[city_index(x_b, y)] =
                (heights[city_index(x_b, y)] & !0x1f) | target.saturating_sub(3);
            xbit[city_index(x_b, y)] |= 0x01;

            let destination = game_rng.random_mod(30);
            if destination > offset {
                offset += 1;
            } else if destination < offset {
                offset -= 1;
            }
        }
    }

    if river_enabled {
        let mut center = 64i16;
        let mut retained_delta = game_rng.random_mod(3) as i16 - 1;
        for y in (0..CITY_HEIGHT).rev() {
            for x in (center - 3)..=(center + 3) {
                heights[city_index(x as usize, y)] =
                    (heights[city_index(x as usize, y)] & !0x1f) | target.saturating_sub(3);
            }
            for x in [center - 4, center + 4] {
                heights[city_index(x as usize, y)] =
                    (heights[city_index(x as usize, y)] & !0x1f) | target.saturating_sub(2);
            }
            if game_rng.random_mod(8) == 0 {
                retained_delta = game_rng.random_mod(3) as i16 - 1;
            }
            center += game_rng.random_mod(3) as i16 + retained_delta - 1;
            center = center.clamp(5, 122);
        }
    }
}

fn propagate_win95_ocean_xbit(xbit: &mut [u8]) {
    // PE `0x447e20`: four full sweeps, each y-major with x as the inner loop.
    for _ in 0..4 {
        for y in 1..CITY_HEIGHT {
            for x in 1..CITY_WIDTH {
                let index = city_index(x, y);
                match xbit[index] & 0x05 {
                    0x01 => xbit[index] &= !0x01,
                    0x05 => {
                        xbit[city_index(x, y - 1)] |= 0x01;
                        xbit[city_index(x - 1, y)] |= 0x01;
                    }
                    _ => {}
                }
            }
        }
    }
}

fn smooth_win95_height_plane(heights: &mut [u16]) {
    let source = heights.to_vec();
    for x in 0..CITY_WIDTH {
        for y in 0..CITY_HEIGHT {
            let center = u32::from(source[city_index(x, y)]);
            let north = if y == 0 {
                center
            } else {
                u32::from(source[city_index(x, y - 1)])
            };
            let south = if y + 1 == CITY_HEIGHT {
                center
            } else {
                u32::from(source[city_index(x, y + 1)])
            };
            let west = if x == 0 {
                center
            } else {
                u32::from(source[city_index(x - 1, y)])
            };
            let east = if x + 1 == CITY_WIDTH {
                center
            } else {
                u32::from(source[city_index(x + 1, y)])
            };
            let neighbor_average = (north + south + west + east) / 4;
            heights[city_index(x, y)] = ((center + neighbor_average) >> 1) as u16;
        }
    }
}

fn relax_win95_slope(heights: &mut [u16], x: usize, y: usize) {
    let index = city_index(x, y);
    let current = heights[index] & 0x1f;
    let neighbors = [
        (x as isize, y as isize - 1),
        (x as isize + 1, y as isize),
        (x as isize, y as isize + 1),
        (x as isize - 1, y as isize),
    ];
    let drop = neighbors.iter().any(|&(nx, ny)| {
        nx >= 0
            && ny >= 0
            && nx < CITY_WIDTH as isize
            && ny < CITY_HEIGHT as isize
            && (heights[city_index(nx as usize, ny as usize)] & 0x1f) + 1 < current
    });
    if drop {
        heights[index] = (heights[index] & !0x1f) | current.saturating_sub(1);
    }
    let adjusted = heights[index] & 0x1f;
    for &(nx, ny) in &neighbors {
        if nx < 0 || ny < 0 || nx >= CITY_WIDTH as isize || ny >= CITY_HEIGHT as isize {
            continue;
        }
        let neighbor_index = city_index(nx as usize, ny as usize);
        if (heights[neighbor_index] & 0x1f) > adjusted {
            relax_win95_slope(heights, nx as usize, ny as usize);
        }
    }
}

mod mac68k;

pub(super) use mac68k::{
    generate, generate_with_rng, mac68k_edit_map_stream, mac68k_edit_map_water,
    mac68k_refresh_water_neighbors_after_surface_clear, mac68k_tree_growth_class,
};

pub(in crate::app) fn mac68k_repair_water_component(city: &mut City, x: usize, y: usize) {
    mac68k::repair_water_component_for_actor(city, x, y);
}

/// Run the recovered Win95 generated-map worker stages against a blank city.
/// CRT and game-LCG state are injected from the app because the original PE
/// consumes process/thread streams shared with other UI and simulation paths.
/// The algorithms and draw order are ported; initial stream seeding remains an
/// app-level approximation until the PE's full title-session chronology is
/// reproduced.
pub(super) fn generate_win95(
    city: &mut City,
    sliders: EditMapSliderValues,
    ocean_enabled: bool,
    river_enabled: bool,
    crt: &mut Win95CrtRand,
    game_rng: &mut Win95GameRand,
) {
    let hills = sliders.hills.min(ui::EDIT_MAP_SLIDER_MAX);
    let water = sliders.water.min(ui::EDIT_MAP_SLIDER_MAX);
    let trees = sliders.trees.min(ui::EDIT_MAP_SLIDER_MAX);
    let (heights, mut xbit) =
        generate_win95_height_plane(hills, water, ocean_enabled, river_enabled, crt, game_rng);
    let terrain = vec![0; CITY_TILES];
    let altm = heights
        .into_iter()
        .map(|height| AltmSample {
            land: (height & 0x1f) as u8,
            water: 0,
            tunnel_levels: 0,
        })
        .collect();
    city.replace_edit_map_canvas(terrain, altm);

    for x in 0..CITY_WIDTH {
        for y in 0..CITY_HEIGHT {
            let index = city_index(x, y);
            city.set_tile_bits(x, y, TileBits::new(xbit[index]));
        }
    }

    let mut target = (u16::from(water) + 4) >> 3;
    if ocean_enabled || river_enabled {
        target = target.max(4);
    }
    let sea = target as i16;
    // PE 0x410af1/0x410b19 publishes the selected WORD before classification.
    city.set_selected_water_height_word(sea);
    for x in 0..CITY_WIDTH {
        for y in 0..CITY_HEIGHT {
            super::pe_fun_00447020_mode0_xund_slope_sea(city, x, y, sea);
        }
    }

    let tree_count = super::pe_fun_00447500_new_city_tree_count(u16::from(trees)) as u16;
    super::pe_fun_00447500(city, crt, tree_count);

    if ocean_enabled {
        for x in 0..CITY_WIDTH {
            for y in 0..CITY_HEIGHT {
                xbit[city_index(x, y)] = city.tile_bits_at(x, y).unwrap_or_default().raw_bits();
            }
        }
        propagate_win95_ocean_xbit(&mut xbit);
        for x in 0..CITY_WIDTH {
            for y in 0..CITY_HEIGHT {
                city.set_tile_bits(x, y, TileBits::new(xbit[city_index(x, y)]));
            }
        }
    }

    let walk_count = water / 4;
    for _ in 0..walk_count {
        let length = (crt.random_u16() & 0x7f) + 0x32;
        let y = usize::from(crt.random_u16() & 0x7f);
        let x = usize::from(crt.random_u16() & 0x7f);
        super::pe_terrain_generation_water_walk_446440(city, crt, x, y, length);
    }
}

#[cfg(test)]
#[path = "terrain_generation/tests.rs"]
mod tests;
