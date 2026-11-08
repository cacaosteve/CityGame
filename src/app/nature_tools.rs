//! Water, stream, tree and forest tools, with shared native placement workers.
//!
//! Existing pricing, Shift behavior, RNG calls, drag order, and map writes are
//! preserved. Height editing, generation orchestration, water-service simulation,
//! and map rendering remain in their existing modules.

use super::{
    PlacementSound, ToolPlacement, ToolPreview, ViewerState, is_water_terrain, neighbor_tiles,
    network_drag_path, pe_place_tile_441f00, play_placement_sound, terrain_generation,
};
use crate::city::{CITY_HEIGHT, CITY_WIDTH, City, TileBits};
use crate::engine_fidelity::EngineFidelity;
use crate::ui::EditTool;
use crate::win95_random::Win95CrtRand;
use glam::Vec2;

/// Win95 `FUN_00447500` per-tile tree arm @ `0x4475ec` → PlaceTile `0x40178f`.
/// Skips XBIT water-tile. Vacant/`<6` → CRT `&1+6`; `6..=0x0A` → `+1`; `0x0B..=0x0C` →
/// CRT `&1+0x0B`; `>=0x0D` unchanged. Leaves XTXT/XTRF/watered.
pub(super) fn pe_fun_00447500_tree_place_tile(
    city: &mut City,
    rng: &mut Win95CrtRand,
    x: usize,
    y: usize,
) -> bool {
    if city.tile_bits_at(x, y).is_some_and(TileBits::water_tile) {
        return false;
    }
    let building = city.building_at(x, y);
    let next = if building < 6 {
        (rng.random_u16() & 1) as u8 + 6
    } else if building < 0x0b {
        building.saturating_add(1)
    } else if building < 0x0d {
        (rng.random_u16() & 1) as u8 + 0x0b
    } else {
        return false;
    };
    pe_place_tile_441f00(city, x, y, next);
    true
}

/// Win95 `FUN_00447500` (`0x447500`): `count` outer CRT picks; each runs `rand()&0x3f`
/// jittered neighbor PlaceTile tree attempts around a `rand()&0x7f` base tile.
pub(super) fn pe_fun_00447500(city: &mut City, rng: &mut Win95CrtRand, mut count: u16) {
    while count > 0 {
        let base_x = (rng.random_u16() & 0x7f) as i16;
        let base_y = (rng.random_u16() & 0x7f) as i16;
        let mut inner = rng.random_u16() & 0x3f;
        if inner != 0 {
            loop {
                inner = inner.saturating_sub(1);
                let ox = (rng.random_u16() % 5) as i16 - (rng.random_u16() % 5) as i16;
                let oy = (rng.random_u16() % 5) as i16 - (rng.random_u16() % 5) as i16;
                let x = base_x + ox;
                let y = base_y + oy;
                if (0..0x80).contains(&x) && (0..0x80).contains(&y) {
                    let _ = pe_fun_00447500_tree_place_tile(city, rng, x as usize, y as usize);
                }
                if inner == 0 {
                    break;
                }
            }
        }
        count = count.saturating_sub(1);
    }
}

/// PE new-city terrain @ `0x410c9a`: tree-scatter count is `(trees_param * trees_param) / 2`
/// before `FUN_00447500`. Not wired into edit-map XTER tree gen (different representation).
pub(super) fn pe_fun_00447500_new_city_tree_count(trees_param: u16) -> u32 {
    let n = u32::from(trees_param);
    (n * n) / 2
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct NatureDrag {
    pub(super) current: (usize, usize),
    pub(super) placed_tiles: usize,
    pub(super) tool: EditTool,
    pub(super) shift_down: bool,
}

pub(super) fn play_nature_placement_sound(
    sound: Option<&crate::audio::SoundPlayer>,
    tool: EditTool,
    placed_tiles: usize,
) {
    if placed_tiles == 0 {
        return;
    }
    let effect = match tool {
        EditTool::Water | EditTool::Stream => PlacementSound::Flood,
        EditTool::Trees | EditTool::Forest => PlacementSound::Plop,
        _ => return,
    };
    play_placement_sound(sound, effect);
}

/// PE PlaceWater neighbor-water mask → slope nibble (`0x446653`/`0x446688` stack words,
/// lookup @ `0x446858`). Index is NESW water-tile bits: N=1 E=2 S=4 W=8. Values for
/// `0..14`; mask `>=0x0F` uses the diagonal secondary table @ `0x446862`/`0x44696b`.
pub(super) const PE_PLACE_WATER_SLOPE_NIBBLE: [u8; 15] = [
    0x0d, 0x15, 0x12, 0x08, 0x13, 0x10, 0x05, 0x01, 0x14, 0x07, 0x11, 0x04, 0x06, 0x03, 0x02,
];

/// PE PlaceWater mask`>=0xF` secondary nibble (`[esp+0x18+2*i]`): bits mark *missing*
/// diagonal water (NW=1 NE=2 SE=4 SW=8). Lookup @ `0x44696b`.
pub(super) const PE_PLACE_WATER_MASKF_NIBBLE: [u8; 16] = [
    0x00, 0x09, 0x0a, 0x00, 0x0b, 0x00, 0x00, 0x00, 0x0c, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
];

pub(super) fn pe_place_water_tile_bit(city: &City, x: isize, y: isize) -> bool {
    if !(0..CITY_WIDTH as isize).contains(&x) || !(0..CITY_HEIGHT as isize).contains(&y) {
        return false;
    }
    city.tile_bits_at(x as usize, y as usize)
        .is_some_and(crate::city::TileBits::water_tile)
}

pub(super) fn pe_place_water_neighbor_mask_446773(city: &City, x: usize, y: usize) -> u8 {
    let mut mask = 0u8;
    if pe_place_water_tile_bit(city, x as isize, y as isize - 1) {
        mask |= 0x01;
    }
    if pe_place_water_tile_bit(city, x as isize + 1, y as isize) {
        mask |= 0x02;
    }
    if pe_place_water_tile_bit(city, x as isize, y as isize + 1) {
        mask |= 0x04;
    }
    if pe_place_water_tile_bit(city, x as isize - 1, y as isize) {
        mask |= 0x08;
    }
    mask
}

pub(super) fn pe_place_water_slope_nibble_446858(city: &City, x: usize, y: usize, mask: u8) -> u8 {
    if mask < 0x0f {
        return PE_PLACE_WATER_SLOPE_NIBBLE[usize::from(mask)];
    }
    // PE @ `0x446862`: all NESW water → diagonal *missing* mask → secondary table.
    let mut diag = 0u8;
    let xi = x as isize;
    let yi = y as isize;
    if !pe_place_water_tile_bit(city, xi - 1, yi - 1) {
        diag = 0x01;
    }
    if !pe_place_water_tile_bit(city, xi + 1, yi - 1) {
        diag |= 0x02;
    }
    if !pe_place_water_tile_bit(city, xi + 1, yi + 1) {
        diag |= 0x04;
    }
    if !pe_place_water_tile_bit(city, xi - 1, yi + 1) {
        diag |= 0x08;
    }
    PE_PLACE_WATER_MASKF_NIBBLE[usize::from(diag)]
}

/// Win95 PlaceWater helper `0x446650`: terrain water, ALTM water:=land, clear XBLD, OR XBIT `0x04`.
/// PE @ `0x44674b`/`0x446755`: existing XTER `0x2E`/`0x3E` exits (waterfall tiles).
/// PE @ `0x44675f`/`0x44676a`: existing XTER `1..=0x0F` → write XTER `0x3E` (`0x0E+0x30`).
/// Else neighbor mask → slope nibble; XTER write @ `0x446970`:
/// - existing `< 0x10`: write `nibble+0x30` (mask0 → `0x3D`)
/// - existing `0x10..=0x1F`: keep XTER
/// - existing `0x20..=0x2F`: if `(nibble^XTER)&0x0F==0` full exit; else write `XTER-0x10`
/// - existing `>=0x30`: if `nibble-XTER == -0x30` full exit; else write `nibble+0x30`
/// PE @ `0x4469df`: water-tile bit skips PlaceTile/ALTM/OR/recurse (after XTER write).
/// PE @ `0x446a66`+: after OR water, re-call PlaceWater on W/S/E/N then NW/NE/SE/SW
/// neighbors that already have water-tile (re-slope). No XTRF write.
pub(super) fn pe_place_water_helper_446650(city: &mut City, x: usize, y: usize) {
    let terrain = city.terrain_at(x, y);
    if matches!(terrain, 0x2e | 0x3e) {
        return;
    }
    let nibble = if (1..=0x0f).contains(&terrain) {
        0x0e
    } else {
        pe_place_water_slope_nibble_446858(
            city,
            x,
            y,
            pe_place_water_neighbor_mask_446773(city, x, y),
        )
    };
    if (0x20..=0x2f).contains(&terrain) {
        if (nibble ^ terrain) & 0x0f == 0 {
            return;
        }
        city.set_terrain(x, y, terrain - 0x10);
    } else if terrain >= 0x30 {
        if i16::from(nibble) - i16::from(terrain) == -0x30 {
            return;
        }
        city.set_terrain(x, y, nibble.wrapping_add(0x30));
    } else if (1..=0x0f).contains(&terrain) {
        city.set_terrain(x, y, 0x3e);
    } else if !(0x10..=0x1f).contains(&terrain) {
        city.set_terrain(x, y, nibble.wrapping_add(0x30));
    }
    // PE @ `0x4469df`: water-tile → exit after XTER write (enables neighbor re-slope).
    if city
        .tile_bits_at(x, y)
        .is_some_and(crate::city::TileBits::water_tile)
    {
        return;
    }
    let land = city.altitude_at(x, y);
    city.set_altitude(x, y, land, land);
    // PE `0x4469ed`: `push 0; call 0x40178f` → PlaceTile `0x441f00` (clear XBLD).
    pe_place_tile_441f00(city, x, y, 0);
    let bits = city.tile_bits_at(x, y).unwrap_or_default();
    let with_water = bits.with_water_tile(true);
    if with_water != bits {
        city.set_tile_bits(x, y, with_water);
    }
    // PE @ `0x446a66`+: recurse PlaceWater on water-tile neighbors (W S E N NW NE SE SW).
    pe_place_water_reslope_neighbors_446a66(city, x, y);
}

pub(super) fn pe_place_water_reslope_neighbors_446a66(city: &mut City, x: usize, y: usize) {
    let neighbors: [(isize, isize); 8] = [
        (-1, 0),
        (0, 1),
        (1, 0),
        (0, -1),
        (-1, -1),
        (1, -1),
        (1, 1),
        (-1, 1),
    ];
    for (dx, dy) in neighbors {
        let nx = x as isize + dx;
        let ny = y as isize + dy;
        if !(0..CITY_WIDTH as isize).contains(&nx) || !(0..CITY_HEIGHT as isize).contains(&ny) {
            continue;
        }
        let (nx, ny) = (nx as usize, ny as usize);
        if city
            .tile_bits_at(nx, ny)
            .is_some_and(crate::city::TileBits::water_tile)
        {
            pe_place_water_helper_446650(city, nx, ny);
        }
    }
}

/// Win95 generated-map water walk `FUN_00446440` (`0x446440`). The worker supplies
/// seed coordinates and length from the shared CRT stream. Direction 1 is the
/// initial heading; an out-of-map candidate or a forbidden `0x11..0x2f` terrain
/// class terminates the walk immediately, as in the PE's early returns.
pub(super) fn pe_terrain_generation_water_walk_446440(
    city: &mut City,
    rng: &mut Win95CrtRand,
    mut x: usize,
    mut y: usize,
    mut length: u16,
) {
    const DX: [i16; 4] = [-1, 0, 1, 0];
    const DY: [i16; 4] = [0, 1, 0, -1];
    const TURN_AFTER_TRY: [i16; 4] = [0, 1, 3, 2];

    let mut heading = 1i32;
    pe_place_water_helper_446650(city, x, y);
    while length != 0 {
        length -= 1;
        let mut current_height = u16::from(city.altitude_at(x, y) & 0x1f);
        if city.terrain_at(x, y) == 0x3e {
            current_height += 1;
        }

        let mut selected = None;
        for tried in 0..4usize {
            let direction = (i32::from(TURN_AFTER_TRY[tried]) + heading) & 3;
            let nx = x as i16 + DX[direction as usize];
            let ny = y as i16 + DY[direction as usize];
            if !(0..0x80).contains(&nx) || !(0..0x80).contains(&ny) {
                return;
            }
            let next_x = nx as usize;
            let next_y = ny as usize;
            let next_height = u16::from(city.altitude_at(next_x, next_y) & 0x1f);
            if next_height > current_height {
                selected = Some((next_x, next_y, tried));
                break;
            }

            let next_terrain = city.terrain_at(next_x, next_y);
            if (0x11..0x30).contains(&next_terrain) {
                return;
            }
            if next_height < current_height || next_terrain == 0 {
                selected = Some((next_x, next_y, tried));
                break;
            }
        }
        let Some((next_x, next_y, chosen_try)) = selected else {
            return;
        };

        pe_place_water_helper_446650(city, next_x, next_y);
        heading = (heading + i32::from(TURN_AFTER_TRY[chosen_try])) & 3;
        if rng.random_mod(3) != 0 {
            heading = (heading + i32::from(rng.random_u15()) * 2 + 1) & 3;
        }
        x = next_x;
        y = next_y;
    }
}

pub(super) fn is_edit_map_tree_terrain(terrain: u8) -> bool {
    matches!(terrain, 0x06..=0x0c)
}

pub(super) fn is_tree_building(building: u8) -> bool {
    matches!(building, 6..=12)
}

/// PE MapToolPlaceTree / Forest place arm XBLD result (`0x43fbcd` / `0x43fd5c`).
/// Returns `None` when XBLD `>= 0x0d` (skip / refuse).
pub(super) fn pe_tree_place_xbld(existing: u8, crt: &mut Win95CrtRand) -> Option<u8> {
    if existing < 6 {
        Some((crt.random_u16() & 1) as u8 + 6)
    } else if existing < 0x0b {
        Some(existing + 1)
    } else if existing < 0x0d {
        Some((crt.random_u16() & 1) as u8 + 0x0b)
    } else {
        None
    }
}

/// PE MapToolPlaceForest shift arm XBLD result (`0x43fd95`).
pub(super) fn pe_forest_shift_xbld(existing: u8) -> Option<u8> {
    if existing < 6 {
        None
    } else if existing == 6 {
        Some(0)
    } else if existing < 0x0d {
        Some(existing - 1)
    } else {
        None
    }
}

impl ViewerState {
    pub(super) fn begin_nature_drag(&mut self, x: usize, y: usize, tool: EditTool) {
        self.begin_nature_drag_with_shift(x, y, tool, false)
    }

    pub(super) fn begin_nature_drag_with_shift(
        &mut self,
        x: usize,
        y: usize,
        tool: EditTool,
        shift_down: bool,
    ) {
        let placed_tiles = self.place_nature_tool_at_with_shift(x, y, tool, shift_down);
        self.nature_drag = Some(NatureDrag {
            current: (x, y),
            placed_tiles,
            tool,
            shift_down,
        });
    }

    pub(super) fn update_nature_drag_at(
        &mut self,
        screen: Vec2,
        window_size: (u32, u32),
        pixel_scale: f32,
    ) -> usize {
        self.tile_at_screen(screen, window_size, pixel_scale)
            .map_or(0, |tile| self.update_nature_drag_to(tile))
    }

    pub(super) fn update_nature_drag_to(&mut self, tile: (usize, usize)) -> usize {
        let Some(drag) = self.nature_drag else {
            return 0;
        };
        if drag.current == tile {
            return 0;
        }

        let path = network_drag_path(drag.current, tile);
        let shift_down = drag.shift_down;
        let tool = drag.tool;
        let mut placed = 0;
        for (tx, ty) in path.into_iter().skip(1) {
            placed += self.place_nature_tool_at_with_shift(tx, ty, tool, shift_down);
        }
        if let Some(drag) = self.nature_drag.as_mut() {
            drag.current = tile;
            drag.placed_tiles += placed;
        }
        placed
    }

    pub(super) fn finish_nature_drag(&mut self) -> Option<(EditTool, usize)> {
        self.nature_drag
            .take()
            .map(|drag| (drag.tool, drag.placed_tiles))
    }

    pub(super) fn cancel_nature_drag(&mut self) {
        self.nature_drag = None;
    }

    pub(super) fn nature_drag_active(&self) -> bool {
        self.nature_drag.is_some()
    }

    pub(super) fn surface_water_placement_at(&self, x: usize, y: usize) -> ToolPlacement {
        if self.can_place_surface_water_tile(x, y) {
            ToolPlacement::valid(ToolPreview::Terrain, vec![(x, y)])
        } else {
            ToolPlacement {
                valid_tiles: Vec::new(),
                invalid_tiles: vec![(x, y)],
                preview: ToolPreview::Terrain,
            }
        }
    }

    pub(super) fn can_place_surface_water_tile(&self, x: usize, y: usize) -> bool {
        // PE MapToolPlaceWater @ 0x43fe63: refuse XBIT water bit 0x04.
        // No general water-XTER refuse — helper handles 0x2E/0x3E / slope / mask.
        if self
            .city
            .tile_bits_at(x, y)
            .is_some_and(TileBits::water_tile)
        {
            return false;
        }
        if self.city.dispatch_actor_at(x, y).is_some() {
            return false;
        }
        // PE MapToolPlaceWater @ 0x43fe30: refuse XTXT >= 0xfa.
        if self.city.text_overlay_at(x, y).unwrap_or(0) >= 0xfa {
            return false;
        }
        // PE @ 0x43fe96: accept XBLD < 0x0e except radioactivity 5.
        let building = self.city.building_at(x, y);
        building < 0x0e && building != 5
    }

    pub(super) fn place_surface_water_at(&mut self, x: usize, y: usize) -> usize {
        if !self.can_place_surface_water_tile(x, y) {
            return 0;
        }
        if !self.can_afford_tool_build_cost(EditTool::Water, 1) {
            return 0;
        }

        pe_place_water_helper_446650(&mut self.city, x, y);
        // PE MapToolPlaceWater @ 0x43fedf: XZON &= 0xf0 (clear zone type, keep corners).
        let zone_bits = self
            .city
            .zone_at(x, y)
            .map(|zone| zone.raw_bits() & 0xf0)
            .unwrap_or(0);
        self.city.set_zone(x, y, zone_bits);
        self.clear_zone_growth_for_player_tiles(&[(x, y)]);
        // PE MapToolPlaceWater / helper never write XTRF (`0x4ca940`).
        // PE path is helper + redraw only — no invent citywide water-network refresh.
        self.debit_tool_build_cost(EditTool::Water, 1);
        self.query_is_inspect = false;
        self.refresh_status_line();
        1
    }

    pub(super) fn tree_placement_at(&self, x: usize, y: usize) -> ToolPlacement {
        let preview = ToolPreview::Building(6);
        if self.can_place_tree_at(x, y) {
            ToolPlacement::valid(preview, vec![(x, y)])
        } else {
            ToolPlacement {
                valid_tiles: Vec::new(),
                invalid_tiles: vec![(x, y)],
                preview,
            }
        }
    }

    pub(super) fn can_place_tree_at(&self, x: usize, y: usize) -> bool {
        if !self.tree_tool_shared_gates_ok(x, y) {
            return false;
        }
        // PE @ 0x43fb99 / 0x43fbfe: refuse radioactivity 5 and XBLD >= 0x0d.
        let building = self.city.building_at(x, y);
        building != 5 && building < 0x0d
    }

    /// PE MapToolPlaceTree shift arm (GetAsyncKeyState VK_SHIFT): same early
    /// refuses, then SetBuilding(0). Does not apply the XBLD `>= 0x0d` place gate.
    pub(super) fn can_clear_tree_tool_at(&self, x: usize, y: usize) -> bool {
        if !self.tree_tool_shared_gates_ok(x, y) {
            return false;
        }
        self.city.building_at(x, y) != 5
    }

    pub(super) fn tree_tool_shared_gates_ok(&self, x: usize, y: usize) -> bool {
        // PE MapToolPlaceTree @ 0x43fb73: refuse XBIT water bit 0x04.
        if self
            .city
            .tile_bits_at(x, y)
            .is_some_and(TileBits::water_tile)
        {
            return false;
        }
        // PE @ 0x43fbb4 / 0x43fbba: refuse XTER 0x2e / 0x3e only (not all water XTER).
        let terrain = self.city.terrain_at(x, y);
        if matches!(terrain, 0x2e | 0x3e) {
            return false;
        }
        if self.city.dispatch_actor_at(x, y).is_some() {
            return false;
        }
        true
    }

    pub(super) fn place_edit_map_water_at(&mut self, x: usize, y: usize) {
        if self.engine_fidelity == EngineFidelity::Mac68k {
            terrain_generation::mac68k_edit_map_water(&mut self.city, x, y);
            return;
        }
        let land = self.city.altitude_at(x, y);
        let water = land.saturating_add(1).max(1);
        self.city.set_terrain(x, y, 0x30);
        self.city.set_altitude(x, y, land, water);
        // PE PlaceTile (`0x441f00`): clear XBLD; leave XZON/watered/piped/XTRF.
        pe_place_tile_441f00(&mut self.city, x, y, 0);
        // Edit-map water: terrain/ALTM/XBLD only — leave XZON/watered/piped/XTRF.
    }

    pub(super) fn place_edit_map_stream_at(&mut self, x: usize, y: usize) -> usize {
        if self.engine_fidelity == EngineFidelity::Mac68k {
            return terrain_generation::mac68k_edit_map_stream(
                &mut self.city,
                x,
                y,
                &mut self.mac_process_rng,
            );
        }
        let mut current = (x, y);
        let mut previous = None;
        let mut placed = 0;
        for _ in 0..64 {
            self.place_edit_map_water_at(current.0, current.1);
            placed += 1;
            let current_altitude = self.city.altitude_at(current.0, current.1);
            let Some(next) = neighbor_tiles(current.0, current.1)
                .filter(|tile| Some(*tile) != previous)
                .filter(|&(nx, ny)| self.city.altitude_at(nx, ny) < current_altitude)
                .min_by_key(|&(nx, ny)| self.city.altitude_at(nx, ny))
            else {
                break;
            };
            previous = Some(current);
            current = next;
        }
        placed
    }

    pub(super) fn place_edit_map_tree_at(
        &mut self,
        x: usize,
        y: usize,
        variant_seed: usize,
    ) -> bool {
        if self.engine_fidelity == EngineFidelity::Mac68k {
            return self.mac_edit_map_tree_at(x, y, false);
        }
        if is_water_terrain(self.city.terrain_at(x, y)) {
            return false;
        }
        self.city.set_terrain(x, y, 0x06 + (variant_seed % 7) as u8);
        // PE PlaceTile (`0x441f00`): clear XBLD; leave XZON/watered/piped/XTRF.
        pe_place_tile_441f00(&mut self.city, x, y, 0);
        // Edit-map tree stamp: terrain/XBLD only — leave XZON/watered/piped/XTRF.
        true
    }

    pub(super) fn remove_edit_map_tree_at(&mut self, x: usize, y: usize) -> bool {
        if self.engine_fidelity == EngineFidelity::Mac68k {
            return self.mac_edit_map_tree_at(x, y, true);
        }
        if !is_edit_map_tree_terrain(self.city.terrain_at(x, y)) {
            return false;
        }
        self.city.set_terrain(x, y, 0);
        // Edit-map tree clear: XTER only — leave watered/piped/XTRF.
        true
    }

    pub(super) fn place_edit_map_forest_at(&mut self, x: usize, y: usize) -> usize {
        if self.engine_fidelity == EngineFidelity::Mac68k {
            return self.mac_edit_map_forest_at(x, y, false);
        }
        let mut placed = 0;
        let min_x = x.saturating_sub(3);
        let max_x = (x + 3).min(CITY_WIDTH - 1);
        let min_y = y.saturating_sub(3);
        let max_y = (y + 3).min(CITY_HEIGHT - 1);
        for ty in min_y..=max_y {
            for tx in min_x..=max_x {
                let dx = tx.abs_diff(x);
                let dy = ty.abs_diff(y);
                if dx * dx + dy * dy <= 10 && self.place_edit_map_tree_at(tx, ty, tx * 17 + ty * 31)
                {
                    placed += 1;
                }
            }
        }
        placed
    }

    pub(super) fn remove_edit_map_forest_at(&mut self, x: usize, y: usize) -> usize {
        if self.engine_fidelity == EngineFidelity::Mac68k {
            return self.mac_edit_map_forest_at(x, y, true);
        }
        let mut removed = 0;
        let min_x = x.saturating_sub(3);
        let max_x = (x + 3).min(CITY_WIDTH - 1);
        let min_y = y.saturating_sub(3);
        let max_y = (y + 3).min(CITY_HEIGHT - 1);
        for ty in min_y..=max_y {
            for tx in min_x..=max_x {
                let dx = tx.abs_diff(x);
                let dy = ty.abs_diff(y);
                if dx * dx + dy * dy <= 10 && self.remove_edit_map_tree_at(tx, ty) {
                    removed += 1;
                }
            }
        }
        removed
    }

    pub(super) fn mac_edit_map_tree_at(&mut self, x: usize, y: usize, clear: bool) -> bool {
        // CODE 5 FUN_00003474: XBIT water / XBLD 5 refuse before Shift.
        // XTER holds terrain shape and is neither a gate nor a tree plane.
        if x >= CITY_WIDTH
            || y >= CITY_HEIGHT
            || self
                .city
                .tile_bits_at(x, y)
                .is_some_and(|bits| bits.water_tile())
        {
            return false;
        }
        let old = self.city.building_at(x, y);
        if old == 5 {
            return false;
        }
        let target = if clear {
            Some(0)
        } else {
            terrain_generation::mac68k_tree_growth_class(old, &mut self.mac_process_rng)
        };
        let Some(target) = target else {
            return false;
        };
        // Native export 105 writes XBLD and its ordinary/zone-7 census.
        // This shared interface maintains the Mac saved census through
        // set_building, and the compatibility PE census for PE city files.
        pe_place_tile_441f00(&mut self.city, x, y, target);
        true
    }

    pub(super) fn mac_edit_map_forest_at(&mut self, x: usize, y: usize, clear: bool) -> usize {
        // CODE 5 FUN_00003556: exactly twenty attempts; rejected candidates
        // still spend all four coordinate draws. Repeated hits are allowed.
        let mut placed = 0;
        for _ in 0..20 {
            let tx = x as i32 + i32::from(self.mac_process_rng.random_u16() & 7)
                - i32::from(self.mac_process_rng.random_u16() & 7);
            let ty = y as i32 + i32::from(self.mac_process_rng.random_u16() & 7)
                - i32::from(self.mac_process_rng.random_u16() & 7);
            if !(0..CITY_WIDTH as i32).contains(&tx) || !(0..CITY_HEIGHT as i32).contains(&ty) {
                continue;
            }
            let (tx, ty) = (tx as usize, ty as usize);
            if self
                .city
                .tile_bits_at(tx, ty)
                .is_some_and(|bits| bits.water_tile())
            {
                continue;
            }
            let old = self.city.building_at(tx, ty);
            let target = if clear {
                match old {
                    6 => Some(0),
                    7..=12 => Some(old - 1),
                    _ => None,
                }
            } else {
                // Unlike the single-tree worker, forest accepts old XBLD 5.
                terrain_generation::mac68k_tree_growth_class(old, &mut self.mac_process_rng)
            };
            if let Some(target) = target {
                pe_place_tile_441f00(&mut self.city, tx, ty, target);
                // Count accepted PlaceTile calls, including same-class writes.
                placed += 1;
            }
        }
        placed
    }

    pub(super) fn place_tree_at(&mut self, x: usize, y: usize) -> usize {
        self.place_tree_at_with_shift(x, y, false)
    }

    pub(super) fn place_tree_at_with_shift(
        &mut self,
        x: usize,
        y: usize,
        shift_down: bool,
    ) -> usize {
        if shift_down {
            if !self.can_clear_tree_tool_at(x, y) {
                return 0;
            }
        } else if !self.can_place_tree_at(x, y) {
            return 0;
        }
        if !self.can_afford_tool_build_cost(EditTool::Trees, 1) {
            return 0;
        }

        let tree = if shift_down {
            // PE MapToolPlaceTree @ 0x43fbc9: VK_SHIFT → SetBuilding(0).
            0
        } else {
            // PE MapToolPlaceTree @ 0x43fbcd..0x43fc0d:
            // XBLD < 6 → CRT rand()&1 + 6; 6..10 → inc; 11..12 → CRT rand()&1 + 0xb.
            let existing = self.city.building_at(x, y);
            pe_tree_place_xbld(existing, &mut self.pe_process_crt)
                .expect("can_place_tree_at already gated XBLD")
        };
        self.commit_tree_tool_building(x, y, tree, EditTool::Trees)
    }

    /// PE `FUN_0043fc80` MapToolPlaceForest: 20 CRT-jittered stamps around `(x,y)`.
    pub(super) fn place_forest_at_with_shift(
        &mut self,
        x: usize,
        y: usize,
        shift_down: bool,
    ) -> usize {
        if !self.can_afford_tool_build_cost(EditTool::Forest, 1) {
            return 0;
        }
        let mut stamped = 0usize;
        for _ in 0..20 {
            // PE @ 0x43fc93..0x43fcc6: (rand&7)-(rand&7) jitter on each axis.
            let jx = (self.pe_process_crt.random_u16() & 7) as i16
                - (self.pe_process_crt.random_u16() & 7) as i16;
            let jy = (self.pe_process_crt.random_u16() & 7) as i16
                - (self.pe_process_crt.random_u16() & 7) as i16;
            let tx = x as i16 + jx;
            let ty = y as i16 + jy;
            if !(0..CITY_WIDTH as i16).contains(&tx) || !(0..CITY_HEIGHT as i16).contains(&ty) {
                continue;
            }
            let (tx, ty) = (tx as usize, ty as usize);
            if self
                .city
                .tile_bits_at(tx, ty)
                .is_some_and(TileBits::water_tile)
            {
                continue;
            }
            let terrain = self.city.terrain_at(tx, ty);
            // PE @ `0x43fd3e`/`0x43fd44`: refuse XTER 0x2E/0x3E only — not general water XTER.
            if matches!(terrain, 0x2e | 0x3e) {
                continue;
            }
            if self.city.dispatch_actor_at(tx, ty).is_some() {
                continue;
            }
            let existing = self.city.building_at(tx, ty);
            let next = if shift_down {
                // PE Forest shift arm @ 0x43fd95: <6 skip; ==6 → 0; <0xd → dec.
                match pe_forest_shift_xbld(existing) {
                    Some(building) => building,
                    None => continue,
                }
            } else {
                // PE Forest place arm @ 0x43fd5c: no radioactivity refuse; >=0xd skip.
                match pe_tree_place_xbld(existing, &mut self.pe_process_crt) {
                    Some(building) => building,
                    None => continue,
                }
            };
            // PE MapToolPlaceForest @ `0x43fdb1`: PlaceTile `0x40178f`→`0x441f00`.
            pe_place_tile_441f00(&mut self.city, tx, ty, next);
            self.clear_zone_growth_for_player_tiles(&[(tx, ty)]);
            // PE MapToolPlaceForest / PlaceTree: no watered/piped clear; no XTRF clear.
            stamped += 1;
        }
        if stamped == 0 {
            return 0;
        }
        // PE FUN_0043fc80: PlaceTile + redraw only — no invent water-network refresh.
        self.debit_tool_build_cost(EditTool::Forest, 1);
        self.query_is_inspect = false;
        self.refresh_status_line();
        stamped
    }

    pub(super) fn commit_tree_tool_building(
        &mut self,
        x: usize,
        y: usize,
        tree: u8,
        tool: EditTool,
    ) -> usize {
        // PE MapToolPlaceTree @ `0x43fc10`: PlaceTile `0x40178f`→`0x441f00` (XZON unchanged).
        pe_place_tile_441f00(&mut self.city, x, y, tree);
        self.clear_zone_growth_for_player_tiles(&[(x, y)]);
        // PE MapToolPlaceTree (`0x43fb40`): PlaceTile + redraw only — no watered/piped/XTRF
        // clear and no full water-network refresh.
        self.debit_tool_build_cost(tool, 1);
        self.query_is_inspect = false;
        self.refresh_status_line();
        1
    }

    pub(super) fn place_nature_tool_at(&mut self, x: usize, y: usize, tool: EditTool) -> usize {
        self.place_nature_tool_at_with_shift(x, y, tool, false)
    }

    pub(super) fn place_nature_tool_at_with_shift(
        &mut self,
        x: usize,
        y: usize,
        tool: EditTool,
        shift_down: bool,
    ) -> usize {
        match tool {
            EditTool::Trees => self.place_tree_at_with_shift(x, y, shift_down),
            EditTool::Water => self.place_surface_water_at(x, y),
            _ => 0,
        }
    }
}
