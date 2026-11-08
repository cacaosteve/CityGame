//! Mac 68k terrain generation, tree growth, and water/stream editing.

use super::*;

/// Mac 68k `FUN_00013dda` / `FUN_00013e68` height construction and the
/// immediately following water-band, smoothing, normalization, and slope
/// stages. This is intentionally separate from the PE worker: both versions
/// use an 8×8 lattice, but their boundary and RNG contracts differ.
pub(super) fn generate_mac68k_height_plane(
    hills: u8,
    water: u8,
    coast_enabled: bool,
    river_enabled: bool,
    rng: &mut MacToolboxRng,
    game_rng: &mut MacGameLcgRng,
) -> (Vec<u16>, Vec<u8>) {
    let hills = u16::from(hills);
    let modulus = hills + 11;
    let outside_height = hills + 10;
    let mut heights = vec![0u16; CITY_TILES];
    let mut coast_water = vec![0u8; CITY_TILES];

    // FUN_00013dda visits y first, then x, in 16-tile intervals. `_Random`
    // returns a signed low word, but this worker divides the word unsigned.
    for y in (0..CITY_HEIGHT).step_by(16) {
        for x in (0..CITY_WIDTH).step_by(16) {
            heights[city_index(x, y)] = rng.random_u16() % modulus + 1;
        }
    }

    for (step, alignment_mask) in [(8usize, 15usize), (4, 7), (2, 3), (1, 1)] {
        for y in (0..CITY_HEIGHT).step_by(step) {
            for x in (0..CITY_WIDTH).step_by(step) {
                if x & alignment_mask == 0 && y & alignment_mask == 0 {
                    continue;
                }
                let average = if x & alignment_mask == 0 {
                    (u32::from(mac_height_sample(
                        &heights,
                        x as isize,
                        y as isize - step as isize,
                        outside_height,
                        coast_enabled,
                    )) + u32::from(mac_height_sample(
                        &heights,
                        x as isize,
                        y as isize + step as isize,
                        outside_height,
                        coast_enabled,
                    ))) / 2
                } else if y & alignment_mask == 0 {
                    (u32::from(mac_height_sample(
                        &heights,
                        x as isize - step as isize,
                        y as isize,
                        outside_height,
                        coast_enabled,
                    )) + u32::from(mac_height_sample(
                        &heights,
                        x as isize + step as isize,
                        y as isize,
                        outside_height,
                        coast_enabled,
                    ))) / 2
                } else {
                    (u32::from(mac_height_sample(
                        &heights,
                        x as isize - step as isize,
                        y as isize - step as isize,
                        outside_height,
                        coast_enabled,
                    )) + u32::from(mac_height_sample(
                        &heights,
                        x as isize + step as isize,
                        y as isize - step as isize,
                        outside_height,
                        coast_enabled,
                    )) + u32::from(mac_height_sample(
                        &heights,
                        x as isize - step as isize,
                        y as isize + step as isize,
                        outside_height,
                        coast_enabled,
                    )) + u32::from(mac_height_sample(
                        &heights,
                        x as isize + step as isize,
                        y as isize + step as isize,
                        outside_height,
                        coast_enabled,
                    ))) / 4
                };
                heights[city_index(x, y)] =
                    (average as u16 + rng.random_u16() % step as u16).max(1);
            }
        }
    }

    let sea_level =
        ((u16::from(water) + 4) >> 3).max(u16::from(coast_enabled || river_enabled) * 4);
    if coast_enabled {
        // FUN_00014134 uses the app-owned game LCG, not QuickDraw `_Random`.
        // It starts with LCG(10)+10 and walks one step per row toward an
        // LCG(30) target. Water lies east of the boundary.
        let mut inset = 10 + usize::from(game_rng.bounded(10));
        for y in 0..CITY_HEIGHT {
            let boundary_x = CITY_WIDTH - inset;
            for x in boundary_x..CITY_WIDTH {
                let index = city_index(x, y);
                let band = if x == boundary_x {
                    0xfeu16 + sea_level
                } else {
                    0xfdu16 + sea_level
                };
                heights[index] = (heights[index] & !0x1f) | (band & 0x1f);
                coast_water[index] = u8::from(x > boundary_x);
            }
            let destination = usize::from(game_rng.bounded(30));
            inset = match destination.cmp(&inset) {
                std::cmp::Ordering::Greater => inset + 1,
                std::cmp::Ordering::Less => inset - 1,
                std::cmp::Ordering::Equal => inset,
            };
        }
    }
    if river_enabled {
        // FUN_00014206 lays seven river cells with two one-cell shoulders,
        // then advances the center from the south edge toward the north.
        let mut center = 64i16;
        let mut retained_delta = game_rng.bounded(3) as i16 - 1;
        for y in (0..CITY_HEIGHT).rev() {
            for x in center - 3..=center + 3 {
                let index = city_index(x as usize, y);
                heights[index] = (heights[index] & !0x1f) | ((0xfdu16 + sea_level) & 0x1f);
            }
            for x in [center - 4, center + 4] {
                let index = city_index(x as usize, y);
                heights[index] = (heights[index] & !0x1f) | ((0xfeu16 + sea_level) & 0x1f);
            }
            if game_rng.bounded(8) == 0 {
                retained_delta = game_rng.bounded(3) as i16 - 1;
            }
            center += game_rng.bounded(3) as i16 + retained_delta - 1;
            center = center.clamp(5, 122);
        }
    }

    // FUN_00014034 copies the plane to scratch before each pass, but combines
    // each scratch neighbor average with the scratch center. Missing neighbors
    // duplicate the center. The routine runs twice before normalization.
    smooth_mac68k_height_plane(&mut heights);
    smooth_mac68k_height_plane(&mut heights);
    for height in &mut heights {
        *height = normalize_mac68k_height(*height);
    }
    smooth_mac68k_height_plane(&mut heights);

    // Keep the Mac recursion's scan-order semantics separate from the PE
    // implementation. The rooted worker recursively relaxes cardinal slopes.
    for y in 0..CITY_HEIGHT {
        for x in 0..CITY_WIDTH {
            relax_mac68k_slope(&mut heights, x, y);
        }
    }
    (heights, coast_water)
}

pub(super) fn normalize_mac68k_height(height: u16) -> u16 {
    // export_132's 0x12ec8..0x12ef0 pass stores the transformed word directly
    // into ALTM; it does not repack it into either of the five-bit fields.
    let mut value = (i32::from(height) + 3) >> 1;
    value -= 4;
    if value < 0 {
        value += 4;
    }
    if value < 4 {
        value = 4;
    }
    value as u16
}

fn mac_height_sample(
    heights: &[u16],
    x: isize,
    y: isize,
    outside_height: u16,
    coast_enabled: bool,
) -> u16 {
    if x < 0 || y < 0 || y >= CITY_HEIGHT as isize {
        return outside_height;
    }
    if x >= CITY_WIDTH as isize {
        return if coast_enabled { 0 } else { outside_height };
    }
    heights[city_index(x as usize, y as usize)]
}

fn smooth_mac68k_height_plane(heights: &mut [u16]) {
    let scratch = heights.to_vec();
    for y in 0..CITY_HEIGHT {
        for x in 0..CITY_WIDTH {
            let center = u32::from(scratch[city_index(x, y)]);
            let neighbor = |nx: isize, ny: isize| -> u32 {
                if nx < 0 || ny < 0 || nx >= CITY_WIDTH as isize || ny >= CITY_HEIGHT as isize {
                    center
                } else {
                    u32::from(scratch[city_index(nx as usize, ny as usize)])
                }
            };
            let sum = neighbor(x as isize, y as isize - 1)
                + neighbor(x as isize + 1, y as isize)
                + neighbor(x as isize, y as isize + 1)
                + neighbor(x as isize - 1, y as isize);
            let value = ((sum >> 2) + center) >> 1;
            heights[city_index(x, y)] = value as u16;
        }
    }
}

fn relax_mac68k_slope(heights: &mut [u16], x: usize, y: usize) {
    let index = city_index(x, y);
    let mut level = heights[index] & 0x1f;
    let neighbors = [
        (x as isize, y as isize - 1),
        (x as isize + 1, y as isize),
        (x as isize, y as isize + 1),
        (x as isize - 1, y as isize),
    ];
    let too_steep = neighbors.iter().any(|&(nx, ny)| {
        if nx < 0 || ny < 0 || nx >= CITY_WIDTH as isize || ny >= CITY_HEIGHT as isize {
            return false;
        }
        let neighbor_index = city_index(nx as usize, ny as usize);
        (heights[neighbor_index] & 0x1f) + 1 < level
    });
    if !too_steep {
        return;
    }

    // FUN_00013862 lowers its own low-five-bit height by exactly one, then
    // recursively visits cardinal neighbors that are now lower than it.
    level -= 1;
    heights[index] = (heights[index] & !0x1f) | level;
    for (nx, ny) in neighbors {
        if nx < 0 || ny < 0 || nx >= CITY_WIDTH as isize || ny >= CITY_HEIGHT as isize {
            continue;
        }
        let neighbor_index = city_index(nx as usize, ny as usize);
        if heights[neighbor_index] & 0x1f < level {
            relax_mac68k_slope(heights, nx as usize, ny as usize);
        }
    }
}

/// Build the Mac 68k Make Map height and terrain canvas for the title preview
/// and the city/editor entered from it. The seed is a deterministic surrogate
/// for QuickDraw's process-wide low-memory `RndSeed`; algorithm and draw order
/// are ported for the stages covered by the local disassembly ledger. The
/// second app-owned game-LCG is passed by the caller and remains process-wide.
pub(in crate::app) fn generate(
    seed: u32,
    coast_enabled: bool,
    river_enabled: bool,
    sliders: EditMapSliderValues,
    game_rng: &mut MacGameLcgRng,
) -> (Vec<u8>, Vec<AltmSample>, Vec<u8>, Vec<u8>, u8) {
    let mut rng = MacToolboxRng::new(seed);
    generate_with_rng(&mut rng, coast_enabled, river_enabled, sliders, game_rng)
}

/// Runtime entry point: the original Make Map routines consume the shared
/// QuickDraw process stream, which has already been advanced by earlier Mac
/// work such as New City's invention-date draws. `generate` remains the
/// deterministic seeded adapter for isolated fixtures.
pub(in crate::app) fn generate_with_rng(
    rng: &mut MacToolboxRng,
    coast_enabled: bool,
    river_enabled: bool,
    sliders: EditMapSliderValues,
    game_rng: &mut MacGameLcgRng,
) -> (Vec<u8>, Vec<AltmSample>, Vec<u8>, Vec<u8>, u8) {
    let hills = sliders.hills.min(ui::EDIT_MAP_SLIDER_MAX);
    let water = sliders.water.min(ui::EDIT_MAP_SLIDER_MAX);
    let trees = sliders.trees.min(ui::EDIT_MAP_SLIDER_MAX);
    let (heights, mut salt_markers) =
        generate_mac68k_height_plane(hills, water, coast_enabled, river_enabled, rng, game_rng);
    let sea_level =
        ((u16::from(water) + 4) >> 3).max(u16::from(coast_enabled || river_enabled) * 4) as u8;
    let (mut terrain, mut altm, water_bits) = classify_mac68k_terrain(heights, sea_level);
    for (marker, water) in salt_markers.iter_mut().zip(water_bits) {
        // FUN_00014134's XBIT bit 0 is an independent salt marker. The
        // classifier decides surface water from the terrain class and height
        // threshold; salt-side membership alone does not stamp a water tile.
        *marker |= u8::from(water) << 2;
    }
    let mut buildings = vec![0u8; CITY_TILES];
    scatter_mac68k_trees(&mut buildings, &salt_markers, trees, rng);
    if coast_enabled {
        propagate_mac68k_salt(&mut salt_markers);
    }
    apply_mac68k_water_walks(
        &mut terrain,
        &mut altm,
        &mut buildings,
        &mut salt_markers,
        water,
        rng,
    );
    (terrain, altm, salt_markers, buildings, sea_level)
}

pub(super) const MAC_WATER_DX: [isize; 4] = [-1, 0, 1, 0];
pub(super) const MAC_WATER_DY: [isize; 4] = [0, 1, 0, -1];
const MAC_WATER_TURN: [u16; 4] = [0, 1, 3, 2];
const MAC_WATER_CLASS: [u8; 16] = [13, 21, 18, 8, 19, 16, 5, 1, 20, 7, 17, 4, 6, 3, 2, 0];
pub(super) const MAC_DRY_CLASS: [u8; 16] = [0, 9, 10, 0, 11, 0, 0, 0, 12, 0, 0, 0, 0, 0, 0, 0];

pub(super) fn scatter_mac68k_trees(
    buildings: &mut [u8],
    xbit: &[u8],
    trees: u8,
    rng: &mut MacToolboxRng,
) {
    // FUN_00013ca6: (Trees * Trees) / 2 outer attempts.
    let attempts = usize::from(trees) * usize::from(trees) / 2;
    for _ in 0..attempts {
        let center_x = usize::from(rng.random_u16() & 0x7f);
        let center_y = usize::from(rng.random_u16() & 0x7f);
        let candidate_count = usize::from(rng.random_u16() & 0x3f);
        for _ in 0..candidate_count {
            let x = center_x as isize + (rng.random_u16() % 5) as isize
                - (rng.random_u16() % 5) as isize;
            let y = center_y as isize + (rng.random_u16() % 5) as isize
                - (rng.random_u16() % 5) as isize;
            if x < 0 || y < 0 || x >= CITY_WIDTH as isize || y >= CITY_HEIGHT as isize {
                continue;
            }
            let index = city_index(x as usize, y as usize);
            if xbit[index] & 0x04 != 0 {
                continue;
            }
            if let Some(class) = mac68k_tree_growth_class(buildings[index], rng) {
                buildings[index] = class;
            }
        }
    }
}

pub(in crate::app) fn mac68k_tree_growth_class(current: u8, rng: &mut MacToolboxRng) -> Option<u8> {
    match current {
        0..=5 => Some(6 + (rng.random_u16() & 1) as u8),
        6..=10 => Some(current + 1),
        11..=12 => Some(11 + (rng.random_u16() & 1) as u8),
        _ => None,
    }
}

pub(super) fn propagate_mac68k_salt(xbit: &mut [u8]) {
    // FUN_00014310: four forward sweeps over the interior. Bits other than
    // salt (0) are preserved when a dry coast marker is cleared.
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

pub(super) fn apply_mac68k_water_walks(
    terrain: &mut [u8],
    altm: &mut [AltmSample],
    buildings: &mut [u8],
    xbit: &mut [u8],
    water: u8,
    rng: &mut MacToolboxRng,
) {
    // export_132 uses the unsigned quotient Water / 4. The call site draws
    // length first, then y and x; the worker signature is (x, y, length).
    for _ in 0..usize::from(water / 4) {
        let length = usize::from(rng.random_u16() & 0x7f) + 0x32;
        let y = usize::from(rng.random_u16() & 0x7f);
        let x = usize::from(rng.random_u16() & 0x7f);
        mac68k_water_walk(terrain, altm, buildings, xbit, x, y, length, rng);
    }
}

pub(super) fn mac68k_water_walk(
    terrain: &mut [u8],
    altm: &mut [AltmSample],
    buildings: &mut [u8],
    xbit: &mut [u8],
    start_x: usize,
    start_y: usize,
    length: usize,
    rng: &mut MacToolboxRng,
) {
    mac68k_water_walk_on(
        &mut MacWaterPlanes {
            terrain,
            altm,
            buildings,
            xbit,
        },
        start_x,
        start_y,
        length,
        rng,
    );
}

// The native generator and editor call the same water workers. Keep their
// terrain decisions shared while the live-city adapter performs PlaceTile's
// saved-census side effects at the original point in the write sequence.
trait MacWaterMap {
    fn terrain(&self, x: usize, y: usize) -> u8;
    fn land_height(&self, x: usize, y: usize) -> u8;
    fn is_water(&self, x: usize, y: usize) -> bool;
    fn set_terrain(&mut self, x: usize, y: usize, value: u8);
    fn flood(&mut self, x: usize, y: usize);
}

struct MacWaterPlanes<'a> {
    terrain: &'a mut [u8],
    altm: &'a mut [AltmSample],
    buildings: &'a mut [u8],
    xbit: &'a mut [u8],
}

impl MacWaterMap for MacWaterPlanes<'_> {
    fn terrain(&self, x: usize, y: usize) -> u8 {
        self.terrain[city_index(x, y)]
    }
    fn land_height(&self, x: usize, y: usize) -> u8 {
        self.altm[city_index(x, y)].land
    }
    fn is_water(&self, x: usize, y: usize) -> bool {
        self.xbit[city_index(x, y)] & 4 != 0
    }
    fn set_terrain(&mut self, x: usize, y: usize, value: u8) {
        self.terrain[city_index(x, y)] = value;
    }
    fn flood(&mut self, x: usize, y: usize) {
        let index = city_index(x, y);
        self.buildings[index] = 0;
        self.altm[index].water = self.altm[index].land;
        self.xbit[index] |= 4;
    }
}

struct MacWaterCity<'a> {
    city: &'a mut City,
    newly_flooded: usize,
}

/// CODE 3 export 136 as called by Mac kind-5's attack worker. Unlike the map
/// editor wrapper, this repairs the component without clearing XZON afterward.
pub(in crate::app) fn repair_water_component_for_actor(city: &mut City, x: usize, y: usize) {
    if x >= CITY_WIDTH || y >= CITY_HEIGHT {
        return;
    }
    repair_mac68k_water_component_on(
        &mut MacWaterCity {
            city,
            newly_flooded: 0,
        },
        x,
        y,
    );
}

impl MacWaterMap for MacWaterCity<'_> {
    fn terrain(&self, x: usize, y: usize) -> u8 {
        self.city.terrain_at(x, y)
    }
    fn land_height(&self, x: usize, y: usize) -> u8 {
        self.city.altitude_at(x, y)
    }
    fn is_water(&self, x: usize, y: usize) -> bool {
        self.city
            .tile_bits_at(x, y)
            .is_some_and(|bits| bits.water_tile())
    }
    fn set_terrain(&mut self, x: usize, y: usize, value: u8) {
        self.city.set_terrain(x, y, value);
    }
    fn flood(&mut self, x: usize, y: usize) {
        // CODE 3 0x13626..0x13672: PlaceTile(0), copy land into water
        // bits without changing the high ALTM bits, then OR XBIT 0x04.
        super::super::pe_place_tile_441f00(self.city, x, y, 0);
        let land = self.city.altitude_at(x, y);
        self.city.set_altitude(x, y, land, land);
        if let Some(bits) = self.city.tile_bits_at(x, y) {
            self.city.set_tile_bits(x, y, bits.with_water_tile(true));
        }
        self.newly_flooded += 1;
    }
}

pub(in crate::app) fn mac68k_edit_map_water(city: &mut City, x: usize, y: usize) -> bool {
    // CODE 5 FUN_000036ac. The stream caller bypasses these gates.
    if x >= CITY_WIDTH
        || y >= CITY_HEIGHT
        || city
            .tile_bits_at(x, y)
            .is_some_and(|bits| bits.water_tile())
        || city.text_overlay_at(x, y).unwrap_or(0) >= 0xfa
        || city.building_at(x, y) >= 0x0e
        || city.building_at(x, y) == 5
    {
        return false;
    }
    repair_mac68k_water_component_on(
        &mut MacWaterCity {
            city,
            newly_flooded: 0,
        },
        x,
        y,
    );
    // This follows component repair even when that helper returns early.
    // In particular, PlaceTile must still see the original zone-7 nibble.
    let zone = city.zone_at(x, y).map_or(0, |zone| zone.raw_bits());
    city.set_zone(x, y, zone & 0xf0);
    true
}

pub(in crate::app) fn mac68k_refresh_water_neighbors_after_surface_clear(
    city: &mut City,
    x: usize,
    y: usize,
) {
    if x >= CITY_WIDTH || y >= CITY_HEIGHT {
        return;
    }

    // CODE 5 FUN_0000884e reaches CODE 3 export 136 in this order after the
    // clicked cell's XBIT water flag has been cleared. Keep this separate
    // from the Win95 PlaceWater recursion: export 136 only repairs neighbors
    // which remain marked as water.
    const NEIGHBORS: [(isize, isize); 8] = [
        (-1, 0),
        (0, 1),
        (1, 0),
        (0, -1),
        (-1, -1),
        (1, -1),
        (1, 1),
        (-1, 1),
    ];
    let mut map = MacWaterCity {
        city,
        newly_flooded: 0,
    };
    for (dx, dy) in NEIGHBORS {
        let nx = x as isize + dx;
        let ny = y as isize + dy;
        if nx < 0 || ny < 0 || nx >= CITY_WIDTH as isize || ny >= CITY_HEIGHT as isize {
            continue;
        }
        let (nx, ny) = (nx as usize, ny as usize);
        if map.is_water(nx, ny) {
            repair_mac68k_water_component_on(&mut map, nx, ny);
        }
    }
}

pub(in crate::app) fn mac68k_edit_map_stream(
    city: &mut City,
    x: usize,
    y: usize,
    rng: &mut MacToolboxRng,
) -> usize {
    if x >= CITY_WIDTH || y >= CITY_HEIGHT {
        return 0;
    }
    let mut map = MacWaterCity {
        city,
        newly_flooded: 0,
    };
    // CODE 5 0x8214 pushes length 100; it draws neither length nor x/y.
    mac68k_water_walk_on(&mut map, x, y, 100, rng);
    map.newly_flooded
}

fn mac68k_water_walk_on(
    map: &mut impl MacWaterMap,
    start_x: usize,
    start_y: usize,
    length: usize,
    rng: &mut MacToolboxRng,
) {
    let mut x = start_x;
    let mut y = start_y;
    let mut heading = 1u16;
    repair_mac68k_water_component_on(map, x, y);

    for _ in 0..length {
        // CODE 3 0x13306..0x1332a reloads this from the current cell on
        // every step, including after a descent, rather than retaining the
        // initial cell's ceiling for the whole walk.
        let mut max_height = u16::from(map.land_height(x, y)) & 0x1f;
        if map.terrain(x, y) == 0x3e {
            max_height += 1;
        }
        let mut selected = None;
        for direction in 0..4 {
            let offset = MAC_WATER_TURN[direction];
            let actual = usize::from((heading + offset) & 3);
            let nx = x as isize + MAC_WATER_DX[actual];
            let ny = y as isize + MAC_WATER_DY[actual];
            if nx < 0 || ny < 0 || nx >= CITY_WIDTH as isize || ny >= CITY_HEIGHT as isize {
                // Native bounds branches go to the function epilogue.
                return;
            }
            let height = u16::from(map.land_height(nx as usize, ny as usize)) & 0x1f;
            if height > max_height {
                continue;
            }
            let class = map.terrain(nx as usize, ny as usize);
            if (0x10..0x30).contains(&class) {
                // 0x133ba also exits the whole walk; it does not try the
                // next direction as the higher-height refusal does.
                return;
            }
            if height == max_height && class != 0 {
                continue;
            }
            selected = Some((direction, nx as usize, ny as usize));
            break;
        }
        let Some((direction, next_x, next_y)) = selected else {
            break;
        };

        x = next_x;
        y = next_y;
        repair_mac68k_water_component_on(map, x, y);
        heading = (heading + MAC_WATER_TURN[direction]) & 3;
        if rng.random_u16() % 3 != 0 {
            heading = (heading + 1 + 2 * u16::from(rng.random_u16() & 1)) & 3;
        }
    }
}

fn repair_mac68k_water_component_on(map: &mut impl MacWaterMap, x: usize, y: usize) {
    let current = map.terrain(x, y);
    if current == 0x2e || current == 0x3e {
        return;
    }

    let class = if (1..0x10).contains(&current) {
        14
    } else {
        let mut water_mask = 0u8;
        // CODE 3 0x134aa..0x13512 uses N,E,S,W for mask bits 0..3.
        // The water walk's separate direction-vector table uses W,S,E,N.
        const CARDINAL_MASK: [(isize, isize); 4] = [(0, -1), (1, 0), (0, 1), (-1, 0)];
        for (direction, (dx, dy)) in CARDINAL_MASK.into_iter().enumerate() {
            let nx = x as isize + dx;
            let ny = y as isize + dy;
            if nx >= 0
                && ny >= 0
                && nx < CITY_WIDTH as isize
                && ny < CITY_HEIGHT as isize
                && map.is_water(nx as usize, ny as usize)
            {
                water_mask |= 1 << direction;
            }
        }
        if water_mask == 0x0f {
            let mut dry_mask = 0u8;
            // The alternate table is selected only when all cardinals are
            // wet; its mask is built from the four diagonal neighbors.
            const DIAGONAL: [(isize, isize); 4] = [(-1, -1), (1, -1), (1, 1), (-1, 1)];
            for (direction, (dx, dy)) in DIAGONAL.into_iter().enumerate() {
                let nx = x as isize + dx;
                let ny = y as isize + dy;
                if nx >= 0
                    && ny >= 0
                    && nx < CITY_WIDTH as isize
                    && ny < CITY_HEIGHT as isize
                    && !map.is_water(nx as usize, ny as usize)
                {
                    dry_mask |= 1 << direction;
                }
            }
            MAC_DRY_CLASS[usize::from(dry_mask)]
        } else {
            MAC_WATER_CLASS[usize::from(water_mask)]
        }
    };

    if current < 0x10 {
        map.set_terrain(x, y, 0x30 + class);
    } else if current >= 0x30 {
        let updated = 0x30 + class;
        if updated == current {
            return;
        }
        map.set_terrain(x, y, updated);
    } else if current >= 0x20 {
        // CODE 3 0x135f2..0x135fc masks BOTH classes before comparing.
        if (current & 0x0f) == (class & 0x0f) {
            return;
        }
        map.set_terrain(x, y, current.wrapping_add(0xf0));
    }

    if map.is_water(x, y) {
        return;
    }
    // FUN_00013446 calls PlaceTile(0), transfers ALTM land into the water
    // height field, and marks XBIT bit 2. The source then invokes itself on
    // every already-water 8-neighbor; those calls can update XTER but return
    // before changing ALTM/XBIT or recursing again.
    map.flood(x, y);
    // Native refresh order: W,S,E,N,NW,NE,SE,SW. These recursive calls
    // visit already-wet cells, so they repair shapes without flooding again.
    const NEIGHBORS: [(isize, isize); 8] = [
        (-1, 0),
        (0, 1),
        (1, 0),
        (0, -1),
        (-1, -1),
        (1, -1),
        (1, 1),
        (-1, 1),
    ];
    for (dx, dy) in NEIGHBORS {
        let nx = x as isize + dx;
        let ny = y as isize + dy;
        if nx < 0 || ny < 0 || nx >= CITY_WIDTH as isize || ny >= CITY_HEIGHT as isize {
            continue;
        }
        if map.is_water(nx as usize, ny as usize) {
            repair_mac68k_water_component_on(map, nx as usize, ny as usize);
        }
    }
}

pub(super) fn classify_mac68k_terrain(
    heights: Vec<u16>,
    sea_level: u8,
) -> (Vec<u8>, Vec<AltmSample>, Vec<bool>) {
    // A5 data at ffffab34/ffffab46 and ffffacc8/ffffacd0. Neighbor order is
    // N,NE,E,SE,S,SW,W,NW; each higher neighbor contributes its bit weight.
    const DX: [isize; 8] = [0, 1, 1, 1, 0, -1, -1, -1];
    const DY: [isize; 8] = [-1, -1, 0, 1, 1, 1, 0, -1];
    const WEIGHT: [u8; 8] = [0x03, 0x02, 0x06, 0x04, 0x0c, 0x08, 0x09, 0x01];
    const CLASS: [u8; 16] = [
        0x00, 0x09, 0x0a, 0x02, 0x0b, 0x0d, 0x03, 0x06, 0x0c, 0x01, 0x0d, 0x05, 0x04, 0x08, 0x07,
        0x32,
    ];

    let mut terrain = vec![0u8; CITY_TILES];
    let mut altm = vec![AltmSample::default(); CITY_TILES];
    let mut water_bits = vec![false; CITY_TILES];
    for y in 0..CITY_HEIGHT {
        for x in 0..CITY_WIDTH {
            let index = city_index(x, y);
            let packed_altm = heights[index];
            let height = packed_altm & 0x1f;
            let mut mask = 0u8;
            for direction in 0..8 {
                let nx = x as isize + DX[direction];
                let ny = y as isize + DY[direction];
                if nx >= 0
                    && ny >= 0
                    && nx < CITY_WIDTH as isize
                    && ny < CITY_HEIGHT as isize
                    && (heights[city_index(nx as usize, ny as usize)] & 0x1f) > height
                {
                    mask |= WEIGHT[direction];
                }
            }
            let class = CLASS[usize::from(mask & 0x0f)];
            // Mac export_138's special XTER 0x32 arm increments ALTM's
            // low-five land height before testing the selected water level.
            // This is a terrain write even when the tile stays dry.
            let land_word = if class == 0x32 { height + 1 } else { height };
            let land = (land_word & 0x1f) as u8;
            let wet = u16::from(land) < u16::from(sea_level);
            // The original helper preserves the existing upper ALTM fields
            // on its dry arm. The 0x32 increment can carry into water height;
            // its wet arm then replaces that field with the selected level.
            let incremented_altm = (packed_altm & !0x1f) | land_word;
            let water = if wet {
                sea_level
            } else {
                ((incremented_altm >> 5) & 0x1f) as u8
            };
            water_bits[index] = wet;
            if wet {
                terrain[index] = if class == 0x32 {
                    0x10
                } else if height + 1 == u16::from(sea_level) {
                    class.wrapping_add(0x20)
                } else {
                    class.wrapping_add(0x10)
                };
            } else if class != 0x32 {
                terrain[index] = class;
            }
            altm[index] = AltmSample {
                land,
                water,
                tunnel_levels: (packed_altm >> 10) as u8,
            };
        }
    }
    (terrain, altm, water_bits)
}

#[cfg(test)]
pub(super) fn repair_mac68k_water_component(
    terrain: &mut [u8],
    altm: &mut [AltmSample],
    buildings: &mut [u8],
    xbit: &mut [u8],
    x: usize,
    y: usize,
) {
    repair_mac68k_water_component_on(
        &mut MacWaterPlanes {
            terrain,
            altm,
            buildings,
            xbit,
        },
        x,
        y,
    );
}
