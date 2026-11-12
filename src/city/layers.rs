//! Dirty tracking and city tile-layer mutation with census/cache upkeep.

use super::*;

pub(crate) fn multi_tile_building_size(building: u8) -> u8 {
    match building {
        // Win95 DestroyStructure size table (`0x45b1c0` / `0x4e85d8`): highways 2×2.
        0x49..=0x50 | 0x61..=0x6b => 2,
        140..=173 => 2,
        174..=197 => 3,
        201..=207 => 4,
        208..=214 => 3,
        215..=218 => 4,
        235..=247 => 2,
        248..=250 => 3,
        251..=255 => 4,
        _ => 1,
    }
}

pub(super) fn infer_corner_key_bits(
    buildings: Option<&[u8]>,
    zones: Option<&[ZoneFlags]>,
) -> [u8; 4] {
    let (Some(buildings), Some(zones)) = (buildings, zones) else {
        return DEFAULT_CORNER_KEY_BITS;
    };

    let mut counts = [[0u32; 4]; 4];
    for x in 0..CITY_WIDTH {
        for y in 0..CITY_HEIGHT {
            let building = buildings[city_index(x, y)];
            let size = multi_tile_building_size(building);
            if building == 0 || size <= 1 {
                continue;
            }
            let size = usize::from(size);
            if x + size > CITY_WIDTH || y + size > CITY_HEIGHT {
                continue;
            }
            let footprint_matches = (0..size)
                .all(|dx| (0..size).all(|dy| buildings[city_index(x + dx, y + dy)] == building));
            if !footprint_matches {
                continue;
            }

            let corners = [
                zones[city_index(x, y)].raw_bits() & 0xf0,
                zones[city_index(x + size - 1, y)].raw_bits() & 0xf0,
                zones[city_index(x, y + size - 1)].raw_bits() & 0xf0,
                zones[city_index(x + size - 1, y + size - 1)].raw_bits() & 0xf0,
            ];
            if !corners.iter().all(|bits| bits.count_ones() == 1) {
                continue;
            }

            for (position, bits) in corners.into_iter().enumerate() {
                if let Some(bit_index) = corner_bit_index(bits) {
                    counts[position][bit_index] += 1;
                }
            }
        }
    }

    let top_left = strongest_corner_bit(counts[0]).unwrap_or(0x10);
    let top_right = strongest_corner_bit(counts[1]).unwrap_or(0x20);
    let bottom_left = strongest_corner_bit(counts[2]).unwrap_or(0x80);
    let bottom_right = strongest_corner_bit(counts[3]).unwrap_or(0x40);

    [bottom_right, top_right, top_left, bottom_left]
}

fn corner_bit_index(bits: u8) -> Option<usize> {
    match bits {
        0x10 => Some(0),
        0x20 => Some(1),
        0x40 => Some(2),
        0x80 => Some(3),
        _ => None,
    }
}

fn strongest_corner_bit(counts: [u32; 4]) -> Option<u8> {
    let (index, count) = counts
        .into_iter()
        .enumerate()
        .max_by_key(|&(_index, count)| count)?;
    (count > 0).then(|| [0x10, 0x20, 0x40, 0x80][index])
}

/// Mac `fffff07a` XBLD census: MISC cursor slot 124, one expanded dword per XBLD ID.
pub(super) const MAC_XBLD_CENSUS_MISC_OFFSET: usize = 0x01f0;
/// Mac `fffff07e` zone-7 XZON-class census: MISC slots 1002..1017.
pub(super) const MAC_ZONE7_CENSUS_MISC_OFFSET: usize = 0x0fa8;
/// Mac CODE 5 `fffff082`: saved WORD count for selected XUND byte classes.
pub(super) const MAC_XUND_CLASS_COUNT_MISC_OFFSET: usize = 0x0fe8;

/// CODE 5 export 105's 16-class `fffff07e` table, restricted to cells whose
/// pre-PlaceTile XZON low nibble is 7. Entry zero is the switch default.
fn mac_zone7_census_class(building: u8) -> usize {
    match building {
        0xdd => 1,
        0xde => 2,
        0xef => 3,
        0xf2 => 4,
        0xea => 5,
        0xe3 => 6,
        0xe4 => 7,
        0xe5 => 8,
        0xf1 => 9,
        0xe0 => 10,
        0xe2 => 11,
        0xe7 => 12,
        0xe8 => 13,
        0xf6 => 14,
        0xf9 => 15,
        _ => 0,
    }
}

#[cfg(test)]
#[test]
fn mac_and_pe_military_tile_classes_match_for_every_xbld_value() {
    for building in u8::MIN..=u8::MAX {
        assert_eq!(
            mac_zone7_census_class(building),
            City::pe_military_tile_count_index(building),
            "military XBLD {building:#04x}"
        );
    }
}

fn mac_xund_counted_value(value: u16) -> bool {
    (1..0x10).contains(&value) || matches!(value, 0x1f | 0x20 | 0x22 | 0x23)
}

impl City {
    pub fn is_modified(&self) -> bool {
        self.modified
    }

    pub fn mark_all_dirty(&mut self) {
        self.dirty_tiles.clear();
        for y in 0..CITY_HEIGHT {
            for x in 0..CITY_WIDTH {
                self.dirty_tiles.insert((x, y));
            }
        }
    }

    pub fn clear_dirty(&mut self) {
        self.dirty_tiles.clear();
    }

    pub fn take_dirty_tiles(&mut self) -> HashSet<(usize, usize)> {
        std::mem::take(&mut self.dirty_tiles)
    }

    pub fn has_dirty_tiles(&self) -> bool {
        !self.dirty_tiles.is_empty()
    }

    pub fn dirty_tile_coords(&self) -> impl Iterator<Item = (usize, usize)> + '_ {
        self.dirty_tiles.iter().copied()
    }

    pub(super) fn mark_dirty(&mut self, x: usize, y: usize) {
        self.modified = true;
        for dy in 0..4 {
            for dx in 0..4 {
                let tx = x as i32 + dx - 1;
                let ty = y as i32 + dy - 1;
                if tx >= 0 && ty >= 0 && (tx as usize) < CITY_WIDTH && (ty as usize) < CITY_HEIGHT {
                    self.dirty_tiles.insert((tx as usize, ty as usize));
                }
            }
        }
    }

    /// Mark tiles for incremental sprite rebuild (edits, moving Win95 overlay actors).
    pub fn mark_render_dirty(&mut self, x: usize, y: usize) {
        self.mark_dirty(x, y);
    }

    pub fn set_name(&mut self, name: impl Into<String>) {
        self.name = name.into();
        self.modified = true;
    }

    pub fn set_terrain(&mut self, x: usize, y: usize, value: u8) {
        if let Some(data) = self.terrain.as_mut() {
            data[city_index(x, y)] = value;
            self.mark_dirty(x, y);
        }
    }

    pub fn set_building(&mut self, x: usize, y: usize, value: u8) {
        if (self.kind == CityFileKind::MacCity || self.mac_cross_layout_active)
            && x < CITY_WIDTH
            && y < CITY_HEIGHT
        {
            let old = self.building_at(x, y);
            if old != value {
                if self
                    .zone_at(x, y)
                    .is_some_and(|zone| zone.raw_bits() & 0x0f == 7)
                {
                    self.adjust_mac_zone7_census(old, value);
                } else {
                    self.adjust_mac_xbld_census(old, value);
                }
            }
        }
        self.write_building(value, x, y);
    }

    /// Store an XBLD byte without the Mac PlaceTile-maintained census side
    /// effect. Use only for original-code direct map writes that bypass
    /// PlaceTile (for example, runtime-object proxy markers).
    pub(crate) fn set_building_without_mac_census(&mut self, x: usize, y: usize, value: u8) {
        self.write_building(value, x, y);
    }

    fn write_building(&mut self, value: u8, x: usize, y: usize) {
        if let Some(data) = self.buildings.as_mut() {
            data[city_index(x, y)] = value;
            self.mark_dirty(x, y);
            self.rebuild_corner_metadata();
        }
    }

    /// Mac `fffff07a` is a saved, PlaceTile-maintained WORD census. Keep that
    /// serialized copy authoritative for Mac files and restore the same saved
    /// WORDs into independent runtime state for Mac fidelity on PE files.
    pub(crate) fn mac_xbld_tile_count(&self, building: u8) -> u16 {
        if self.kind == CityFileKind::MacCity {
            self.mac_state_misc_u32_at(MAC_XBLD_CENSUS_MISC_OFFSET + usize::from(building) * 4)
                .unwrap_or(0) as u16
        } else if self.mac_cross_layout_active {
            self.mac_xbld_census_runtime[usize::from(building)]
        } else {
            self.pe_dw_tile_count[usize::from(building)]
        }
    }

    /// Mac's saved XUND class counter shares the PE saved WORD; their writers
    /// differ on military-zone cells. Unprepared Win cities retain the
    /// existing map-derived fallback for preview and non-Mac callers.
    pub(crate) fn mac_xund_class_count(&self) -> u16 {
        if self.kind == CityFileKind::MacCity {
            self.mac_state_misc_u32_at(MAC_XUND_CLASS_COUNT_MISC_OFFSET)
                .unwrap_or(0) as u16
        } else if self.mac_cross_layout_active {
            self.pe_xund_occupancy
        } else {
            self.underground
                .as_deref()
                .unwrap_or(&[])
                .iter()
                .filter(|&&value| mac_xund_counted_value(u16::from(value)))
                .count() as u16
        }
    }

    /// Mac CODE 8 zone-7 selector census (`fffff07e`), stored as 16 WORDs in
    /// MISC slots 1002..1017. This saved table is the selector input; do not
    /// rebuild it from the map when loading a Mac city.
    pub(crate) fn mac_zone7_census_count(&self, class: usize) -> u16 {
        if class >= 16 {
            return 0;
        }
        if self.mac_cross_layout_active {
            self.pe_military_tile_count[class]
        } else {
            self.mac_state_misc_u32_at(MAC_ZONE7_CENSUS_MISC_OFFSET + class * 4)
                .unwrap_or(0) as u16
        }
    }

    /// Write a selected inland military cell from Mac CODE 14.
    /// The caller checks XTER, XBIT, and the zone category while selecting
    /// the plot. The 3×3 fallback selects all six patches before stamping,
    /// so an overlapping cell changes both censuses on every visit.
    pub(crate) fn mac_mark_military_zone_candidate(&mut self, x: usize, y: usize) -> bool {
        if (self.kind != CityFileKind::MacCity && !self.mac_cross_layout_active)
            || x >= CITY_WIDTH
            || y >= CITY_HEIGHT
        {
            return false;
        }
        let Some(zone) = self.zone_at(x, y) else {
            return false;
        };
        let building = self.building_at(x, y);
        if building >= 0x0d {
            return false;
        }

        let xbld_count = self.mac_xbld_tile_count(building);
        self.set_mac_xbld_census_count(building, xbld_count.wrapping_sub(1));
        if self.mac_cross_layout_active {
            let count = &mut self.pe_dw_tile_count[usize::from(building)];
            *count = count.wrapping_sub(1);
        }
        self.set_zone(x, y, (zone.raw_bits() & 0xf0) | 7);
        let zone7_count = self.mac_zone7_census_count(0);
        self.set_mac_state_misc_u32_at(
            MAC_ZONE7_CENSUS_MISC_OFFSET,
            u32::from(zone7_count.wrapping_add(1)),
        );
        true
    }

    /// Export 105 accepts a WORD but writes its low byte to XBLD. A
    /// connector retile can pass an index outside the saved ordinary census.
    pub(crate) fn mac_connector_place_tile(&mut self, x: usize, y: usize, value: u16) {
        if value <= u16::from(u8::MAX) || self.zone_at(x, y).is_some_and(|zone| zone.zone_id() == 7)
        {
            self.set_building(x, y, value as u8);
            return;
        }
        let old = self.building_at(x, y);
        let old_count = self.mac_xbld_tile_count(old);
        self.set_mac_xbld_census_count(old, old_count.wrapping_sub(1));
        self.write_building(value as u8, x, y);
    }

    fn adjust_mac_zone7_census(&mut self, old: u8, new: u8) {
        let old_class = mac_zone7_census_class(old);
        let new_class = mac_zone7_census_class(new);
        if old_class == new_class {
            return;
        }
        let old_offset = MAC_ZONE7_CENSUS_MISC_OFFSET + old_class * 4;
        let new_offset = MAC_ZONE7_CENSUS_MISC_OFFSET + new_class * 4;
        let old_count = self.mac_zone7_census_count(old_class);
        let new_count = self.mac_zone7_census_count(new_class);
        self.set_mac_state_misc_u32_at(old_offset, u32::from(old_count.wrapping_sub(1)));
        self.set_mac_state_misc_u32_at(new_offset, u32::from(new_count.wrapping_add(1)));
    }

    fn adjust_mac_xbld_census(&mut self, old: u8, new: u8) {
        if (self.kind != CityFileKind::MacCity && !self.mac_cross_layout_active) || old == new {
            return;
        }
        let old_count = self.mac_xbld_tile_count(old);
        let new_count = self.mac_xbld_tile_count(new);
        self.set_mac_xbld_census_count(old, old_count.wrapping_sub(1));
        self.set_mac_xbld_census_count(new, new_count.wrapping_add(1));
    }

    fn set_mac_xbld_census_count(&mut self, building: u8, value: u16) {
        if self.mac_cross_layout_active {
            self.mac_xbld_census_runtime[usize::from(building)] = value;
        } else {
            self.set_mac_state_misc_u32_at(
                MAC_XBLD_CENSUS_MISC_OFFSET + usize::from(building) * MISC_FIELD_LEN,
                u32::from(value),
            );
        }
    }

    /// Mac CODE 5 `export_106` updates `fffff082` when replacing XUND bytes in
    /// its recovered class set. This is independent of Win95's occupancy
    /// mirror and has no XZON gate.
    fn adjust_mac_xund_class_count(&mut self, old: u16, new: u16) {
        if mac_xund_counted_value(old) == mac_xund_counted_value(new) {
            return;
        }
        let count = self
            .mac_state_misc_u32_at(MAC_XUND_CLASS_COUNT_MISC_OFFSET)
            .unwrap_or(0) as u16;
        let next = if mac_xund_counted_value(new) {
            count.wrapping_add(1)
        } else {
            count.wrapping_sub(1)
        };
        self.set_mac_state_misc_u32_at(MAC_XUND_CLASS_COUNT_MISC_OFFSET, u32::from(next));
    }

    pub fn replace_edit_map_canvas(&mut self, terrain: Vec<u8>, altm: Vec<AltmSample>) {
        self.replace_edit_map_canvas_with_buildings(terrain, altm, vec![0u8; CITY_TILES]);
    }

    pub fn replace_edit_map_canvas_with_buildings(
        &mut self,
        terrain: Vec<u8>,
        altm: Vec<AltmSample>,
        buildings: Vec<u8>,
    ) {
        if terrain.len() != CITY_TILES || altm.len() != CITY_TILES || buildings.len() != CITY_TILES
        {
            return;
        }
        self.terrain = Some(terrain);
        self.altm = Some(altm);
        self.buildings = Some(buildings);
        self.underground = Some(vec![0u8; CITY_TILES]);
        self.zones = Some(vec![ZoneFlags::new(0); CITY_TILES]);
        // The Mac Make Map tree stage calls PlaceTile for each accepted tree,
        // transferring counts from XBLD class 0. Rust generates the final
        // plane directly, so rebuild both editions' counters after blanking zones.
        self.pe_rebuild_dw_tile_count_434f80();
        self.mac_rebuild_xbld_census_after_map_replacement();
        self.text_overlays = Some(vec![0u8; CITY_TILES]);
        self.microsims = Some(vec![MicrosimRecord::default(); MICROSIM_COUNT]);
        self.tile_bits = Some(vec![TileBits::new(0); CITY_TILES]);
        self.traffic_density = Some(vec![0u8; CITY_TILES]);
        self.land_value_density = None;
        self.crime_density = None;
        self.pollution_density = None;
        self.police_coverage = None;
        self.fire_coverage = None;
        self.population_density = None;
        self.rate_of_growth = None;
        self.traffic_overlays = Some(empty_overlay_table());
        self.traffic_overlays_from_file = false;
        self.dispatch_cursors = [0; 3];
        self.mac_dispatch = Default::default();
        self.rebuild_corner_metadata();
        self.modified = true;
        self.mark_all_dirty();
    }

    pub fn set_underground(&mut self, x: usize, y: usize, value: u8) {
        self.mac_place_xund_word(x, y, u16::from(value));
    }

    /// CODE 5 export 106 compares the full WORD for census accounting,
    /// then stores only the low byte in XUND.
    pub(crate) fn mac_place_xund_word(&mut self, x: usize, y: usize, value: u16) {
        if self.kind == CityFileKind::MacCity && x < CITY_WIDTH && y < CITY_HEIGHT {
            self.adjust_mac_xund_class_count(u16::from(self.underground_at(x, y)), value);
        } else if self.mac_cross_layout_active && x < CITY_WIDTH && y < CITY_HEIGHT {
            let old = self.underground_at(x, y);
            let was_counted = mac_xund_counted_value(u16::from(old));
            let is_counted = mac_xund_counted_value(value);
            if was_counted != is_counted {
                self.pe_xund_occupancy = if is_counted {
                    self.pe_xund_occupancy.wrapping_add(1)
                } else {
                    self.pe_xund_occupancy.wrapping_sub(1)
                };
                self.persist_pe_xund_occupancy_to_misc_4340b8();
            }
        }
        if self.underground.is_none() {
            self.underground = Some(vec![0u8; CITY_TILES]);
        }
        if let Some(data) = self.underground.as_mut() {
            data[city_index(x, y)] = value as u8;
            self.mark_dirty(x, y);
        }
    }

    pub fn set_zone(&mut self, x: usize, y: usize, value: u8) {
        if let Some(data) = self.zones.as_mut() {
            data[city_index(x, y)] = ZoneFlags::new(value);
            self.mark_dirty(x, y);
            self.rebuild_corner_metadata();
        }
    }

    /// PE `0x434f80` (thunk `0x402ef5`): zero `0x4ca4c8` WORD[256], then count every
    /// non-military (XZON type ≠ 7) XBLD across the 128×128 map. Vacant `0` is counted.
    pub(crate) fn pe_rebuild_dw_tile_count_434f80(&mut self) {
        self.pe_dw_tile_count = [0; 256];
        for x in 0..CITY_WIDTH {
            for y in 0..CITY_HEIGHT {
                if self.zone_at(x, y).is_some_and(|z| z.zone_id() == 7) {
                    continue;
                }
                let b = usize::from(self.building_at(x, y));
                self.pe_dw_tile_count[b] = self.pe_dw_tile_count[b].wrapping_add(1);
            }
        }
    }

    /// Mac CODE 3 `export_143` resets both XBLD census tables before Make Map.
    /// Its blank XZON plane means every replacement XBLD cell belongs in the
    /// ordinary 256-entry census; the zone-7 16-entry census starts at zero.
    fn mac_rebuild_xbld_census_after_map_replacement(&mut self) {
        if self.kind != CityFileKind::MacCity && !self.mac_cross_layout_active {
            return;
        }

        let mut counts = [0u16; 256];
        if let Some(buildings) = self.buildings.as_deref() {
            for &building in buildings {
                counts[usize::from(building)] = counts[usize::from(building)].wrapping_add(1);
            }
        }
        if self.mac_cross_layout_active {
            self.mac_xbld_census_runtime = counts;
            self.set_misc_u32_values(MAC_ZONE7_CENSUS_MISC_OFFSET, &[0; 16]);
            return;
        }
        for (building, count) in counts.into_iter().enumerate() {
            self.set_mac_state_misc_u32_at(
                MAC_XBLD_CENSUS_MISC_OFFSET + building * MISC_FIELD_LEN,
                u32::from(count),
            );
        }
        for class in 0..16 {
            self.set_mac_state_misc_u32_at(
                MAC_ZONE7_CENSUS_MISC_OFFSET + class * MISC_FIELD_LEN,
                0,
            );
        }
    }

    /// PE PlaceTile military word index @ `0x441f93`/`0x44206e`: XBLD−`0xdd` via
    /// `0x442208`/`0x442268` case map, else word 0.
    pub(crate) fn pe_military_tile_count_index(building: u8) -> usize {
        let delta = usize::from(building.wrapping_sub(0xdd));
        if delta > 0x1c {
            return 0;
        }
        let case = usize::from(PE_MILITARY_TILE_CASE_MAP[delta]);
        usize::from(PE_MILITARY_TILE_CASE_WORD[case])
    }

    /// PE PlaceTile military census @ `0x441f86`/`0x442066`: dec old / inc new in
    /// `0x4ca934` WORD[16] before the XBLD write.
    pub(crate) fn pe_adjust_military_tile_count_441f86(&mut self, old: u8, new: u8) {
        if self.mac_cross_layout_active {
            // Shared PE-named helpers call `set_building` immediately after
            // this; the selected Mac writer maintains the shared census.
            return;
        }
        let old_index = Self::pe_military_tile_count_index(old);
        let new_index = Self::pe_military_tile_count_index(new);
        self.pe_military_tile_count[old_index] =
            self.pe_military_tile_count[old_index].wrapping_sub(1);
        self.pe_military_tile_count[new_index] =
            self.pe_military_tile_count[new_index].wrapping_add(1);
    }

    /// PE military siting writes XZON directly and increments class zero;
    /// unlike PlaceTile it does not trigger `set_building` census maintenance.
    pub(crate) fn pe_increment_military_zone_census(&mut self, old_building: u8) {
        self.pe_military_tile_count[0] = self.pe_military_tile_count[0].wrapping_add(1);
        if self.kind == CityFileKind::MacCity {
            let old_count = self.mac_xbld_tile_count(old_building);
            self.set_mac_state_misc_u32_at(
                MAC_XBLD_CENSUS_MISC_OFFSET + usize::from(old_building) * MISC_FIELD_LEN,
                u32::from(old_count.wrapping_sub(1)),
            );
            let saved = i32::from(self.pe_military_tile_count[0] as i16) as u32;
            self.set_mac_state_misc_u32_at(MAC_ZONE7_CENSUS_MISC_OFFSET, saved);
        }
    }

    pub fn set_text_overlay(&mut self, x: usize, y: usize, value: u8) {
        self.pe_synthetic_trip_markers[city_index(x, y)] = false;
        if self.text_overlays.is_none() {
            self.text_overlays = Some(vec![0u8; CITY_TILES]);
        }
        if let Some(data) = self.text_overlays.as_mut() {
            data[city_index(x, y)] = value;
            self.mark_dirty(x, y);
        }
    }

    /// Record an XTXT marker added by CityGame's off-map trip compatibility.
    pub(crate) fn set_synthetic_trip_marker(&mut self, x: usize, y: usize) {
        self.set_text_overlay(x, y, 0xfa);
        self.pe_synthetic_trip_markers[city_index(x, y)] = true;
    }

    pub(crate) fn is_synthetic_trip_marker(&self, x: usize, y: usize) -> bool {
        self.pe_synthetic_trip_markers[city_index(x, y)]
    }

    /// Win95 Provision steal: zero XTXT cells whose overlay is `slot+0x33`.
    pub(super) fn clear_text_overlays_matching(&mut self, overlay: u8) {
        let Some(data) = self.text_overlays.as_ref() else {
            return;
        };
        let hits: Vec<(usize, usize)> = data
            .iter()
            .enumerate()
            .filter(|(_, value)| **value == overlay)
            .map(|(index, _)| (index / CITY_HEIGHT, index % CITY_HEIGHT))
            .collect();
        for (x, y) in hits {
            self.set_text_overlay(x, y, 0);
        }
    }

    pub fn set_label(&mut self, index: u8, text: impl Into<String>) {
        if self.labels.is_none() {
            self.labels = Some(vec![String::new(); CITY_LABEL_COUNT]);
        }
        if let Some(labels) = self.labels.as_mut()
            && let Some(label) = labels.get_mut(index as usize)
        {
            *label = text.into();
            self.modified = true;
        }
    }

    pub fn set_altitude(&mut self, x: usize, y: usize, land: u8, water: u8) {
        if let Some(data) = self.altm.as_mut() {
            let tunnel_levels = data[city_index(x, y)].tunnel_levels;
            data[city_index(x, y)] = AltmSample {
                land: land & 0x1f,
                water: water & 0x1f,
                tunnel_levels,
            };
            self.mark_dirty(x, y);
        }
    }

    /// Write a raw `ALTM` word, retaining all six high bits exactly.
    /// Most callers should use [`set_altitude`](Self::set_altitude) and
    /// [`set_tunnel_levels`](Self::set_tunnel_levels); this is for native
    /// routines whose word arithmetic can spill across the packed fields.
    pub fn set_altm_word(&mut self, x: usize, y: usize, word: u16) {
        if let Some(data) = self.altm.as_mut() {
            data[city_index(x, y)] = AltmSample {
                land: (word & 0x001f) as u8,
                water: ((word >> 5) & 0x001f) as u8,
                tunnel_levels: ((word >> 10) & 0x003f) as u8,
            };
            self.mark_dirty(x, y);
        }
    }

    pub fn set_tunnel_levels(&mut self, x: usize, y: usize, levels: u8) {
        if let Some(data) = self.altm.as_mut() {
            let sample = &mut data[city_index(x, y)];
            sample.tunnel_levels = (sample.tunnel_levels & 0x20) | (levels & 0x1f);
            self.mark_dirty(x, y);
        }
    }

    /// PE DestroyBuilding elevated arm @ `0x4599aa`: `ALTM &= 0x3ff` (clear bits 10–15).
    pub fn clear_altm_tunnel_bits(&mut self, x: usize, y: usize) {
        if let Some(data) = self.altm.as_mut() {
            data[city_index(x, y)].tunnel_levels = 0;
            self.mark_dirty(x, y);
        }
    }

    pub fn set_rotation(&mut self, rotation: u8) {
        self.rotation = rotation & 0x03;
        self.modified = true;
        self.mark_all_dirty();
    }

    /// Change the effective display compass without marking the city as edited. This affects
    /// rotated sprite ids and keyed building corners; screen-grid rotation is renderer state.
    pub fn set_view_rotation(&mut self, rotation: u8) {
        self.rotation = rotation & 0x03;
    }

    pub fn active_corner_key_bit(&self) -> u8 {
        self.corner_key_bits[self.rotation as usize]
    }

    /// Mac CODE 8 `export_241` orientation mask selected from the A5 table at
    /// `A5-0x1f26`. This table is distinct from the renderer's corner-key
    /// order and uses the saved `ffffe0c6` selector, not the display compass:
    /// rotations 0..3 test `0x80, 0x10, 0x20, 0x40` in XZON's high nibble.
    pub(crate) fn mac_growth_scan_corner_mask(&self) -> u8 {
        [0x80, 0x10, 0x20, 0x40][(self.saved_rotation & 3) as usize]
    }

    pub fn footprint_corner_bits(&self) -> [u8; 4] {
        [
            self.corner_key_bits[2],
            self.corner_key_bits[1],
            self.corner_key_bits[3],
            self.corner_key_bits[0],
        ]
    }

    pub fn set_traffic_density(&mut self, x: usize, y: usize, value: u8) {
        if x >= CITY_WIDTH || y >= CITY_HEIGHT {
            return;
        }
        if self.traffic_density.is_none() {
            self.traffic_density = Some(vec![0u8; CITY_TILES]);
        }
        // Win95 XTRF and Mac ffffe740 are 64×64 grids; each cell covers a 2×2
        // of the 128×128 city map. Mac export_217 indexes the native byte with
        // x/2 and y/2, so the expanded storage preserves the same cell aliasing.
        let origin_x = (x / 2) * 2;
        let origin_y = (y / 2) * 2;
        if let Some(data) = self.traffic_density.as_mut() {
            for dy in 0..2 {
                for dx in 0..2 {
                    let tx = origin_x + dx;
                    let ty = origin_y + dy;
                    if tx >= CITY_WIDTH || ty >= CITY_HEIGHT {
                        continue;
                    }
                    data[city_index(tx, ty)] = value;
                }
            }
        }
        for dy in 0..2 {
            for dx in 0..2 {
                let tx = origin_x + dx;
                let ty = origin_y + dy;
                if tx < CITY_WIDTH && ty < CITY_HEIGHT {
                    self.mark_dirty(tx, ty);
                }
            }
        }
    }

    /// Saturating add into the microsim 2×2 covering `(x, y)` (Win95 `XTRF[x/2][y/2]`,
    /// also Mac route density `ffffe740[y/2][x/2]`). PE `FUN_00457620` @ `0x4582e2`
    /// adds once into the native 64×64 cell and saturates at `0xFF`.
    pub fn add_traffic_density_saturating(&mut self, x: usize, y: usize, delta: u8) {
        if delta == 0 || x >= CITY_WIDTH || y >= CITY_HEIGHT {
            return;
        }
        // `set_traffic_density` already fills the whole 2×2; add once like PE.
        let current = self.traffic_density_at(x, y).unwrap_or(0);
        self.set_traffic_density(x, y, current.saturating_add(delta));
    }

    pub fn sync_traffic_from_slice(&mut self, values: &[u8]) {
        if values.len() != CITY_TILES {
            return;
        }
        self.traffic_density = Some(values.to_vec());
        self.modified = true;
    }

    pub fn sync_land_value_from_slice(&mut self, values: &[u8]) {
        if values.len() != CITY_TILES {
            return;
        }
        self.land_value_density = Some(values.to_vec());
        self.modified = true;
    }

    pub fn sync_crime_density_from_slice(&mut self, values: &[u8]) {
        if values.len() != CITY_TILES {
            return;
        }
        self.crime_density = Some(values.to_vec());
        self.modified = true;
    }

    pub fn sync_pollution_density_from_slice(&mut self, values: &[u8]) {
        if values.len() != CITY_TILES {
            return;
        }
        self.pollution_density = Some(values.to_vec());
        self.modified = true;
    }

    pub fn sync_police_coverage_from_slice(&mut self, values: &[u8]) {
        if values.len() != CITY_TILES {
            return;
        }
        self.police_coverage = Some(values.to_vec());
        self.modified = true;
    }

    pub fn sync_fire_coverage_from_slice(&mut self, values: &[u8]) {
        if values.len() != CITY_TILES {
            return;
        }
        self.fire_coverage = Some(values.to_vec());
        self.modified = true;
    }

    pub fn sync_population_density_from_slice(&mut self, values: &[u8]) {
        if values.len() != CITY_TILES {
            return;
        }
        self.population_density = Some(values.to_vec());
        self.modified = true;
    }

    pub fn sync_rate_of_growth_from_slice(&mut self, values: &[u8]) {
        if values.len() != CITY_TILES {
            return;
        }
        self.rate_of_growth = Some(values.to_vec());
        self.modified = true;
    }

    pub(super) fn rebuild_corner_metadata(&mut self) {
        self.corner_key_bits =
            infer_corner_key_bits(self.buildings.as_deref(), self.zones.as_deref());
    }
}
