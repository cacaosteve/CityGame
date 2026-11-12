use super::*;

/// Known CRT calls made by the Win95 `DestroyBuilding(1)` generic render-scatter loop.
///
/// Ordinary selector-1 classes reach `0x459888` and consume two calls at
/// `0x459DEB`/`0x459E19`; selector-1 network tiles `0x51..=0x5C` take a
/// separate path. Selectors 3/4 (including plants `0xC9..=0xCF`) also reach
/// `0x459888` and consume two calls per cell, repeated size times. Selector 2 is
/// coordinate/data-dependent: `FUN_00463C40(x, y-1) < 0x0D` reaches the generic
/// loop, while higher results take network-specific arms. This tile-only
/// helper returns no selector-2 count; the app's hit-aware caller applies the
/// coordinate gate for the generic route. The high-class network walker has
/// separate draw sites. The dedicated `0x3F..=0x42` / `0xDD..=0xE0`
/// arms are excluded.
pub(crate) fn pe_destroy_building_mode1_render_scatter_draw_count(building: u8) -> usize {
    if building < 6 || matches!(building, 0x3f..=0x42 | 0x51..=0x5c | 0xdd..=0xe0) {
        return 0;
    }
    let size = usize::from(multi_tile_building_size(building).max(1));
    match size {
        1 => 2,
        2 => 0,
        3 | 4 => 2 * size * size * size,
        _ => 0,
    }
}

/// PE ChooseAndPlace multi-tile XZON corners (`0x4370e9` / table `0x4dc4d0`).
/// Returns `[NW, NE, SW, SE]` for `building_zone_bits_for_tile` (PE stamps NW,NE,SE,SW).
pub(crate) fn pe_choose_and_place_corner_bits(rotation: u8) -> [u8; 4] {
    // Words at 0x4dc4d0+[rot*8]: NW, NE, SE, SW → remap SE/SW for Rust index order.
    const PE_NW_NE_SE_SW: [[u8; 4]; 4] = [
        [0x10, 0x20, 0x40, 0x80],
        [0x20, 0x40, 0x80, 0x10],
        [0x40, 0x80, 0x10, 0x20],
        [0x80, 0x10, 0x20, 0x40],
    ];
    let [nw, ne, se, sw] = PE_NW_NE_SE_SW[usize::from(rotation & 3)];
    [nw, ne, sw, se]
}

/// Win95 `0x45b23c..0x45b4aa` probes XZON corners around the hit.
pub(crate) fn pe_destroy_building_corner_origin(
    rotation: u8,
    x: usize,
    y: usize,
    size: usize,
    corner_at: impl Fn((usize, usize)) -> u8,
) -> Option<(usize, usize)> {
    let [nw, ne, sw, se] = pe_choose_and_place_corner_bits(rotation);
    let mut tile = (x, y);
    if size >= 3 {
        // Each successful probe moves the cursor before the next probe.
        for (corner, offsets) in [
            (nw, [(-1, -1), (0, -1), (-1, 0)]),
            (ne, [(1, -1), (0, -1), (1, 0)]),
            (se, [(1, 1), (0, 1), (1, 0)]),
            (sw, [(-1, 1), (0, 1), (-1, 0)]),
        ] {
            for (dx, dy) in offsets {
                if let (Some(px), Some(py)) =
                    (tile.0.checked_add_signed(dx), tile.1.checked_add_signed(dy))
                    && px < CITY_WIDTH
                    && py < CITY_HEIGHT
                    && corner_at((px, py)) == corner
                {
                    tile = (px, py);
                }
            }
        }
    }
    // Native returns the low-X/high-Y corner. Our footprint takes min X/Y.
    let (x, y) = tile;
    match corner_at(tile) {
        corner if corner == nw => Some((x, y)),
        corner if corner == ne => Some((x.checked_sub(size - 1)?, y)),
        corner if corner == se => Some((x.checked_sub(size - 1)?, y.checked_sub(size - 1)?)),
        corner if corner == sw => Some((x, y.checked_sub(size - 1)?)),
        _ => None,
    }
}

/// Win95-style origin walk: size×size block of `expected` containing `(x,y)`.
fn pe_find_uniform_building_footprint(
    city: &City,
    x: usize,
    y: usize,
    expected: u8,
    size: usize,
) -> Vec<(usize, usize)> {
    if size <= 1 {
        return vec![(x, y)];
    }
    let min_x = x.saturating_sub(size - 1);
    let min_y = y.saturating_sub(size - 1);
    for origin_y in min_y..=y {
        for origin_x in min_x..=x {
            if origin_x + size > CITY_WIDTH || origin_y + size > CITY_HEIGHT {
                continue;
            }
            let mut tiles = Vec::with_capacity(size * size);
            let mut ok = true;
            for dy in 0..size {
                for dx in 0..size {
                    let tx = origin_x + dx;
                    let ty = origin_y + dy;
                    if city.building_at(tx, ty) != expected {
                        ok = false;
                        break;
                    }
                    tiles.push((tx, ty));
                }
                if !ok {
                    break;
                }
            }
            if ok && tiles.contains(&(x, y)) {
                return tiles;
            }
        }
    }
    vec![(x, y)]
}

impl City {
    /// PE power-plant aging failed-rebuild @ `0x44e606` → `DestroyBuilding(1)`.
    /// CRT PlaceTile rubble on flat XTER; mid-band XTXT clear; XBIT `&= 0x3d`;
    /// XZON `&= 0x0f` (keep type, clear corners).
    /// OpenSC2K annual utilities: explode sound `504` + AutoGoto center when Options Auto-Goto is on.
    pub(super) fn clear_map_structure_for_microsim(
        &mut self,
        expected_building: u8,
        (x, y): (usize, usize),
        rng: &mut Win95CrtRand,
    ) {
        let size = usize::from(multi_tile_building_size(expected_building)).max(1);
        let footprint = pe_find_uniform_building_footprint(self, x, y, expected_building, size);
        for (tx, ty) in footprint {
            self.pe_destroy_building_mode1_tile_crt_rubble(tx, ty, rng);
        }
        // OpenSC2K `utilities.gd` update_power: SOUND_EXPLOSION (504) + view center on demolish.
        self.pe_pending_effect_sound_ids
            .push(crate::audio::SOUND_EXPLODE);
        self.pe_pending_auto_goto.push((x, y));
        self.modified = true;
    }

    pub(super) fn clear_map_structure_for_microsim_pe_plant(
        &mut self,
        expected_building: u8,
        (x, y): (usize, usize),
        cells: &[(usize, usize, bool)],
        rng: &mut Win95CrtRand,
        microsims: &mut [MicrosimRecord],
    ) {
        // Plant selector 4 bypasses the earlier entry-draw branch; only the
        // generic four passes over 16 cells with two draws each precede map writes.
        for _ in 0..pe_destroy_building_mode1_render_scatter_draw_count(expected_building) {
            let _ = rng.random_u15();
        }
        for &(tx, ty, flat) in cells {
            let rubble = if flat { rng.rubble_tile_id() } else { 0 };
            self.pe_destroy_building_mode1_tile_with_rubble(tx, ty, rubble, Some(&mut *microsims));
        }
        self.pe_pending_effect_sound_ids
            .push(crate::audio::SOUND_EXPLODE);
        self.pe_pending_auto_goto.push((x, y));
        self.modified = true;
    }

    /// PE DestroyBuilding mode1 tile clear used by microsim demolish / Launch wipe.
    pub(super) fn pe_destroy_building_mode1_tile_crt_rubble(
        &mut self,
        tx: usize,
        ty: usize,
        rng: &mut Win95CrtRand,
    ) {
        let old_building = self.building_at(tx, ty);
        let new_building = if old_building >= 6 {
            if self.terrain_at(tx, ty) == 0 {
                rng.rubble_tile_id()
            } else {
                0
            }
        } else {
            0
        };
        self.pe_destroy_building_mode1_tile_with_rubble(tx, ty, new_building, None);
    }

    pub(super) fn pe_destroy_building_mode1_tile_with_rubble(
        &mut self,
        tx: usize,
        ty: usize,
        new_building: u8,
        mut microsims: Option<&mut [MicrosimRecord]>,
    ) {
        let old_building = self.building_at(tx, ty);
        // PE PlaceTile census: non-military adjusts `dwTileCount`; military adjusts
        // `0x4ca934` WORD[16] before the XBLD write.
        let military = self.zone_at(tx, ty).is_some_and(|z| z.zone_id() == 7);
        if military {
            self.pe_adjust_military_tile_count_441f86(old_building, new_building);
        } else {
            let old = old_building as usize;
            self.pe_dw_tile_count[old] = self.pe_dw_tile_count[old].wrapping_sub(1);
            self.pe_dw_tile_count[new_building as usize] =
                self.pe_dw_tile_count[new_building as usize].wrapping_add(1);
        }
        self.set_building(tx, ty, new_building);
        if let Some(bits) = self.tile_bits_at(tx, ty) {
            self.set_tile_bits(tx, ty, TileBits::new(bits.raw_bits() & 0x3d));
        }
        let zone = self.zone_at(tx, ty).map(|z| z.raw_bits()).unwrap_or(0);
        self.set_zone(tx, ty, zone & 0x0f);
        // PE DestroyBuilding orders PlaceTile, XBIT, and XZON before reading
        // XTXT at `0x45a0c4` and releasing its XMIC resources via `0x401dca`.
        let xtxt = self.text_overlay_at(tx, ty).unwrap_or(0);
        if xtxt != 0 && !(xtxt >= 0xf1 && xtxt != 0xfa) {
            if xtxt < 0xc9 || xtxt == 0xfa {
                self.set_text_overlay(tx, ty, 0);
            }
            if let Some(records) = microsims.as_deref_mut() {
                self.pe_401dca_clear_xtxt_resources_with_microsims(xtxt, records);
            } else {
                self.pe_401dca_clear_xtxt_resources(xtxt);
            }
        }
    }

    /// PE yearly structure epilogue @ `0x44f26a`–`0x44f311`: when Launch arco
    /// `dwTileCount[0xfe]/16 > 300` and `dwArcologyPopulation > 0x5b8d80`,
    /// play `0x211`, DestroyBuilding mode 1 every XBLD `0xfe` in PE x-major order, adjust funds
    /// (`0x4ca444`) from the Launch microsim counter (`[esp+0x14]`), then play `0x212`.
    pub(super) fn pe_maybe_mass_destroy_launch_arcos(
        &mut self,
        arco_residents_total: u32,
        rng: &mut Win95CrtRand,
    ) {
        let footprints = pe_sar_div(i32::from(self.pe_dw_tile_count[254]), 4);
        if footprints <= 300 || arco_residents_total <= 0x005b_8d80 {
            return;
        }
        // PE @ `0x44f29c`–`0x44f2a1`: `0x4b174f(0x211)` before the map wipe.
        self.pe_pending_effect_sound_ids
            .push(crate::audio::SOUND_LAUNCH_ARCO_WIPE_START);
        // PE Launch counter @ `0x44f003`: one inc per tile_id `0xfe` XMIC row.
        let launch_count = self.structure_microsim_indices_for(254).len();
        let dim = 0x80.min(CITY_WIDTH).min(CITY_HEIGHT);
        // PE @ `0x44f295..0x44f2db`: SI is the outer X coordinate; DI is the
        // inner Y coordinate. DestroyBuilding consumes shared CRT randomness,
        // so preserve this x-major order across separate Launch footprints.
        for x in 0..dim {
            for y in 0..dim {
                if self.building_at(x, y) != 254 {
                    continue;
                }
                self.pe_destroy_building_mode1_tile_crt_rubble(x, y, rng);
            }
        }
        if let Some(microsims) = self.microsims.as_mut() {
            for record in microsims.iter_mut().skip(MICROSIM_STRUCTURE_FIRST) {
                if record.tile_id == 254 {
                    *record = MicrosimRecord::default();
                }
            }
        }
        // PE @ `0x44f2dd`–`0x44f306`: eax=32*val-val; then
        // ecx=val+4*eax=125*val, edx=5*ecx, eax=5*edx, eax*=32.
        // This yields exactly 100,000*val.
        let val = launch_count as i32;
        let funds = self.city_funds().wrapping_add(val.wrapping_mul(100_000));
        self.set_city_funds(funds);
        // PE @ `0x44f2f6`/`0x44f30c`: `0x4b174f(0x212)` after funds write.
        self.pe_pending_effect_sound_ids
            .push(crate::audio::SOUND_LAUNCH_ARCO_WIPE_END);
        self.modified = true;
    }

    /// Drain PE effect sound ids queued by sim (`0x4b174f`); order preserved.
    pub fn take_pe_pending_effect_sound_ids(&mut self) -> Vec<u16> {
        std::mem::take(&mut self.pe_pending_effect_sound_ids)
    }

    /// Drain plant-expiry (etc.) AutoGoto centers queued by yearly structure; order preserved.
    pub fn take_pe_pending_auto_goto(&mut self) -> Vec<(usize, usize)> {
        std::mem::take(&mut self.pe_pending_auto_goto)
    }

    #[cfg(test)]
    pub fn pe_maybe_mass_destroy_launch_arcos_for_test(
        &mut self,
        arco_residents_total: u32,
        rng: &mut Win95CrtRand,
    ) {
        self.pe_maybe_mass_destroy_launch_arcos(arco_residents_total, rng);
    }

    pub(super) fn structure_microsim_indices_for(&self, tile_id: u8) -> Vec<usize> {
        self.microsims
            .as_ref()
            .map(|microsims| {
                microsims
                    .iter()
                    .enumerate()
                    .skip(MICROSIM_STRUCTURE_FIRST)
                    .filter_map(|(index, record)| (record.tile_id == tile_id).then_some(index))
                    .collect()
            })
            .unwrap_or_default()
    }
}
