//! City layer queries, saved-view accessors, and building draw footprints.

use super::*;

fn structure_uses_water_surface(city: &City, x: usize, y: usize) -> bool {
    if city.tile_bits_at(x, y).is_some_and(TileBits::water_tile) {
        return true;
    }
    city.terrain_at(x, y) >= 0x10
}

pub(super) fn density_sum(values: Option<&[u8]>) -> Option<u32> {
    values.map(|values| {
        values.iter().fold(0_u32, |total, value| {
            total.saturating_add(u32::from(*value))
        })
    })
}

impl City {
    #[allow(dead_code)]
    pub fn chunk(&self, name: &str) -> Option<&CityChunk> {
        self.chunk_index
            .get(name)
            .and_then(|index| self.chunks.get(*index))
    }

    pub fn terrain_at(&self, x: usize, y: usize) -> u8 {
        self.terrain
            .as_ref()
            .and_then(|data| data.get(city_index(x, y)))
            .copied()
            .unwrap_or(((x * 3 + y * 5) & 0xff) as u8)
    }

    pub fn building_at(&self, x: usize, y: usize) -> u8 {
        self.buildings
            .as_ref()
            .and_then(|data| data.get(city_index(x, y)))
            .copied()
            .unwrap_or(0)
    }

    pub fn underground_at(&self, x: usize, y: usize) -> u8 {
        self.underground
            .as_ref()
            .and_then(|data| data.get(city_index(x, y)))
            .copied()
            .unwrap_or(0)
    }

    pub fn has_underground_map(&self) -> bool {
        self.underground.is_some()
    }

    pub fn building_should_draw_at(&self, x: usize, y: usize) -> bool {
        let building = self.building_at(x, y);
        if building == 0 {
            return false;
        }

        let size = usize::from(multi_tile_building_size(building));
        if size <= 1 {
            return true;
        }

        let Some((origin_x, origin_y)) =
            self.inferred_multitile_building_origin(x, y, building, size)
        else {
            return false;
        };

        let active_corner = self.corner_key_bits[self.rotation as usize];
        if self.footprint_has_corner_bits(origin_x, origin_y, size) {
            if !self.footprint_has_complete_corner_bits(origin_x, origin_y, size) {
                return false;
            }
            let Some(zone) = self.zone_at(x, y) else {
                return false;
            };
            let corner_bits = zone.raw_bits() & 0xf0;
            if corner_bits == 0 {
                return false;
            }

            let [top_left, top_right, bottom_left, bottom_right] = self.footprint_corner_bits();
            let last = size - 1;
            let expected_corner = match (x - origin_x, y - origin_y) {
                (0, 0) => top_left,
                (dx, 0) if dx == last => top_right,
                (0, dy) if dy == last => bottom_left,
                (dx, dy) if dx == last && dy == last => bottom_right,
                _ => 0,
            };

            return corner_bits == expected_corner && corner_bits == active_corner;
        }

        self.inferred_multitile_building_draw_tile(x, y, origin_x, origin_y, size)
    }

    /// Mac `export_81` owner rule for the 0x61..0x6b highway footprint family.
    /// It normalizes the footprint anchor to even x / odd y before the Mac
    /// traffic pass emits the footprint's overlay.
    pub fn mac_export81_traffic_owner_at(&self, x: usize, y: usize) -> bool {
        let building = self.building_at(x, y);
        if !matches!(building, 97..=107) {
            return self.building_should_draw_at(x, y);
        }

        let origin_x = x & !1;
        let origin_y = (y & !1).saturating_add(1);
        (x, y) == (origin_x, origin_y)
            && self.complete_building_footprint(origin_x, origin_y, building, 2)
    }

    fn inferred_multitile_building_draw_tile(
        &self,
        x: usize,
        y: usize,
        origin_x: usize,
        origin_y: usize,
        size: usize,
    ) -> bool {
        let [top_left, top_right, bottom_left, bottom_right] = self.footprint_corner_bits();
        let active_corner = self.corner_key_bits[self.rotation as usize];
        let last = size - 1;
        let (draw_x, draw_y) = if active_corner == top_left {
            (origin_x, origin_y)
        } else if active_corner == top_right {
            (origin_x + last, origin_y)
        } else if active_corner == bottom_left {
            (origin_x, origin_y + last)
        } else if active_corner == bottom_right {
            (origin_x + last, origin_y + last)
        } else {
            (origin_x, origin_y)
        };

        (x, y) == (draw_x, draw_y)
    }

    fn inferred_multitile_building_origin(
        &self,
        x: usize,
        y: usize,
        building: u8,
        size: usize,
    ) -> Option<(usize, usize)> {
        if size == 0 || size > CITY_WIDTH || size > CITY_HEIGHT {
            return None;
        }

        if let Some((origin_x, origin_y)) =
            self.aligned_multitile_building_origin(x, y, building, size)
            && self.complete_building_footprint(origin_x, origin_y, building, size)
        {
            return Some((origin_x, origin_y));
        }

        let start_x = x.saturating_sub(size - 1);
        let start_y = y.saturating_sub(size - 1);
        let end_x = x.min(CITY_WIDTH - size);
        let end_y = y.min(CITY_HEIGHT - size);

        for origin_y in start_y..=end_y {
            for origin_x in start_x..=end_x {
                if self.complete_building_footprint(origin_x, origin_y, building, size) {
                    return Some((origin_x, origin_y));
                }
            }
        }

        None
    }

    fn aligned_multitile_building_origin(
        &self,
        x: usize,
        y: usize,
        building: u8,
        size: usize,
    ) -> Option<(usize, usize)> {
        let mut run_start_x = x;
        while run_start_x > 0 && self.building_at(run_start_x - 1, y) == building {
            run_start_x -= 1;
        }

        let mut run_start_y = y;
        while run_start_y > 0 && self.building_at(x, run_start_y - 1) == building {
            run_start_y -= 1;
        }

        let origin_x = run_start_x + ((x - run_start_x) / size) * size;
        let origin_y = run_start_y + ((y - run_start_y) / size) * size;
        (origin_x + size <= CITY_WIDTH && origin_y + size <= CITY_HEIGHT)
            .then_some((origin_x, origin_y))
    }

    pub(super) fn complete_building_footprint(
        &self,
        origin_x: usize,
        origin_y: usize,
        building: u8,
        size: usize,
    ) -> bool {
        origin_x + size <= CITY_WIDTH
            && origin_y + size <= CITY_HEIGHT
            && (0..size).all(|dy| {
                (0..size).all(|dx| self.building_at(origin_x + dx, origin_y + dy) == building)
            })
    }

    fn footprint_has_corner_bits(&self, origin_x: usize, origin_y: usize, size: usize) -> bool {
        origin_x + size <= CITY_WIDTH
            && origin_y + size <= CITY_HEIGHT
            && (0..size).any(|dy| {
                (0..size).any(|dx| {
                    self.zone_at(origin_x + dx, origin_y + dy)
                        .is_some_and(|zone| zone.raw_bits() & 0xf0 != 0)
                })
            })
    }

    fn footprint_has_complete_corner_bits(
        &self,
        origin_x: usize,
        origin_y: usize,
        size: usize,
    ) -> bool {
        if origin_x + size > CITY_WIDTH || origin_y + size > CITY_HEIGHT || size == 0 {
            return false;
        }

        let [top_left, top_right, bottom_left, bottom_right] = self.footprint_corner_bits();
        let last = size - 1;
        [
            (origin_x, origin_y, top_left),
            (origin_x + last, origin_y, top_right),
            (origin_x, origin_y + last, bottom_left),
            (origin_x + last, origin_y + last, bottom_right),
        ]
        .into_iter()
        .all(|(x, y, expected)| {
            self.zone_at(x, y)
                .is_some_and(|zone| zone.raw_bits() & 0xf0 == expected)
        })
    }

    pub fn altitude_at(&self, x: usize, y: usize) -> u8 {
        self.altm_at(x, y).land
    }

    pub fn water_height_at(&self, x: usize, y: usize) -> u8 {
        self.altm_at(x, y).water
    }

    pub fn tunnel_levels_at(&self, x: usize, y: usize) -> u8 {
        self.altm_at(x, y).tunnel_levels & 0x1f
    }

    pub fn altm_at(&self, x: usize, y: usize) -> AltmSample {
        self.altm
            .as_ref()
            .and_then(|data| data.get(city_index(x, y)).copied())
            .unwrap_or_default()
    }

    /// Vertical level for buildings, roads, zones, and traffic — land height unless the tile
    /// is on water (`XBIT` bit 2 or `XTER >= 0x10`), then `ALTM` water height (SC2KRender).
    pub fn structure_elevation_at(&self, x: usize, y: usize) -> u8 {
        let sample = self.altm_at(x, y);
        if structure_uses_water_surface(self, x, y) {
            sample.water
        } else {
            sample.land
        }
    }

    pub fn rotation(&self) -> u8 {
        self.rotation
    }

    pub fn saved_rotation(&self) -> u8 {
        self.saved_rotation
    }

    pub fn saved_date(&self) -> Option<CityDate> {
        self.saved_date
    }

    pub fn set_saved_date(&mut self, date: CityDate) {
        self.set_saved_date_and_day(date, 0);
    }

    pub fn set_saved_date_and_day(&mut self, date: CityDate, day: u8) {
        let month = date.month.clamp(1, 12);
        let day = day.min((DAYS_PER_MONTH - 1) as u8);
        let mut misc = decode_misc_bytes_for_kind(&self.chunks, self.kind).unwrap_or_default();
        misc.resize(misc.len().max(MISC_CITY_AGE_OFFSET + MISC_FIELD_LEN), 0);

        let stored_founded_raw = u32::from_be_bytes(
            misc[MISC_YEAR_FOUNDED_OFFSET..MISC_YEAR_FOUNDED_OFFSET + MISC_FIELD_LEN]
                .try_into()
                .unwrap(),
        );
        // Mac CODE 4 restores only the low WORD of MISC slot 3. The paired
        // saver sign-extends that WORD into the full saved dword.
        let stored_founded = u32::from(misc_year_founded_word(stored_founded_raw, self.kind));
        let year = u32::from(date.year);
        let fallback_founded = self
            .saved_date
            .map(|saved| u32::from(saved.year))
            .unwrap_or(year);
        let founded = if stored_founded == 0 || stored_founded > year {
            fallback_founded.min(year)
        } else {
            stored_founded
        };
        let age_days = year
            .saturating_sub(founded)
            .saturating_mul(DAYS_PER_YEAR)
            .saturating_add(u32::from(month - 1).saturating_mul(DAYS_PER_MONTH))
            .saturating_add(u32::from(day));
        let stored_time = if self.kind == CityFileKind::MacCity {
            // Mac CODE 14 stores one dispatcher tick per in-game calendar
            // day. Its 25/300 units match CityGame's date scale, so the
            // current day belongs in the persisted counter.
            year.saturating_sub(founded)
                .saturating_mul(MAC_CALENDAR_PHASES_PER_YEAR)
                .saturating_add(u32::from(month - 1).saturating_mul(MAC_CALENDAR_PHASES_PER_MONTH))
                .saturating_add(u32::from(day))
        } else {
            age_days
        };

        let founded_misc = misc_year_founded_saved_value(founded as u16, self.kind);
        misc[MISC_YEAR_FOUNDED_OFFSET..MISC_YEAR_FOUNDED_OFFSET + MISC_FIELD_LEN]
            .copy_from_slice(&founded_misc.to_be_bytes());
        misc[MISC_CITY_AGE_OFFSET..MISC_CITY_AGE_OFFSET + MISC_FIELD_LEN]
            .copy_from_slice(&stored_time.to_be_bytes());
        upsert_chunk_for_kind(
            &mut self.chunks,
            &mut self.chunk_index,
            "MISC",
            &misc,
            self.kind,
        );
        self.pe_w_year_founded = founded.min(u32::from(u16::MAX)) as u16;
        self.pe_dw_city_age = age_days;
        // PE age→calendar words @ `0x41354e` (same formulas as daily age tick / graph).
        self.pe_publish_graph_calendar_words_from_city_age_41354e();
        self.saved_date = Some(CityDate {
            year: date.year,
            month,
        });
        self.modified = true;
    }

    /// Initialize the founding year for a freshly-created title-screen city.
    /// Unlike changing the date of an existing city, a new city's selected start
    /// year is also its founding year, so its initial age and calendar counters reset.
    pub(crate) fn initialize_new_city_founding_year(&mut self, year: u16) {
        self.pe_w_year_founded = year;
        self.set_year_founded_misc(year);
        self.pe_dw_city_age = 0;
        self.set_misc_u32_at(MISC_CITY_AGE_OFFSET, 0);
        self.pe_publish_graph_calendar_words_from_city_age_41354e();
        self.modified = true;
    }

    pub fn city_age_days(&self) -> Option<u32> {
        Some(self.pe_dw_city_age)
    }

    pub fn year_founded(&self) -> Option<u32> {
        let year = u32::from(self.pe_w_year_founded);
        (year > 0).then_some(year)
    }

    pub fn saved_view(&self) -> Option<CityView> {
        self.saved_view
    }

    /// Update the Win95 MISC saved camera target and selector.
    /// Returns false when this file format or the supplied values are unsupported.
    pub fn set_saved_view(&mut self, view: CityView) -> bool {
        if !self.kind.is_win95() || view.mode > 2 || view.x > 0x7f || view.y > 0x7f {
            return false;
        }

        let packed_target = (u32::from(view.y) << 8) | u32::from(view.x);
        self.set_misc_u32_values(MISC_SAVED_VIEW_TARGET_OFFSET, &[packed_target, view.mode]);
        self.saved_view = Some(view);
        true
    }

    pub fn xtrf_storage_format(&self) -> XtrfStorageFormat {
        self.xtrf_storage
    }

    pub fn zone_at(&self, x: usize, y: usize) -> Option<ZoneFlags> {
        self.zones
            .as_ref()
            .and_then(|data| data.get(city_index(x, y)))
            .copied()
    }

    pub fn text_overlay_at(&self, x: usize, y: usize) -> Option<u8> {
        self.text_overlays
            .as_ref()
            .and_then(|data| data.get(city_index(x, y)))
            .copied()
    }

    pub fn label_at(&self, index: u8) -> Option<&str> {
        self.labels
            .as_ref()
            .and_then(|labels| labels.get(index as usize))
            .map(String::as_str)
    }

    pub fn microsim_at(&self, index: usize) -> Option<MicrosimRecord> {
        self.microsims
            .as_ref()
            .and_then(|microsims| microsims.get(index).copied())
    }
}

impl City {
    pub fn tile_bits_at(&self, x: usize, y: usize) -> Option<TileBits> {
        self.tile_bits
            .as_ref()
            .and_then(|data| data.get(city_index(x, y)))
            .copied()
    }

    pub fn set_tile_bits(&mut self, x: usize, y: usize, bits: TileBits) {
        if self.tile_bits.is_none() {
            self.tile_bits = Some(vec![TileBits::new(0); CITY_TILES]);
        }
        if let Some(data) = &mut self.tile_bits
            && let Some(tile) = data.get_mut(city_index(x, y))
        {
            *tile = bits;
            self.mark_dirty(x, y);
        }
    }

    pub fn has_tile_bits_map(&self) -> bool {
        self.tile_bits.is_some()
    }

    #[cfg(test)]
    pub fn clear_tile_bits_for_test(&mut self) {
        self.tile_bits = None;
    }

    /// Per-tile traffic sim value from the `XTRF` chunk (0–255), if the city file has one.
    pub fn traffic_density_at(&self, x: usize, y: usize) -> Option<u8> {
        self.traffic_density
            .as_ref()
            .and_then(|data| data.get(city_index(x, y)))
            .copied()
    }

    pub fn has_traffic_density_map(&self) -> bool {
        self.traffic_density.is_some()
    }

    #[cfg(test)]
    pub fn clear_traffic_density_for_test(&mut self) {
        self.traffic_density = None;
    }

    pub fn traffic_density_slice(&self) -> Option<&[u8]> {
        self.traffic_density.as_deref()
    }

    pub fn land_value_at(&self, x: usize, y: usize) -> Option<u8> {
        self.land_value_density
            .as_ref()
            .and_then(|data| data.get(city_index(x, y)))
            .copied()
    }

    pub fn land_value_slice(&self) -> Option<&[u8]> {
        self.land_value_density.as_deref()
    }

    pub fn land_value_density_sum(&self) -> Option<u32> {
        density_sum(self.land_value_density.as_deref())
    }

    pub fn crime_density_at(&self, x: usize, y: usize) -> Option<u8> {
        self.crime_density
            .as_ref()
            .and_then(|data| data.get(city_index(x, y)))
            .copied()
    }

    pub fn crime_density_slice(&self) -> Option<&[u8]> {
        self.crime_density.as_deref()
    }

    pub fn crime_density_sum(&self) -> Option<u32> {
        density_sum(self.crime_density.as_deref())
    }

    pub fn pollution_density_at(&self, x: usize, y: usize) -> Option<u8> {
        self.pollution_density
            .as_ref()
            .and_then(|data| data.get(city_index(x, y)))
            .copied()
    }

    pub fn pollution_density_slice(&self) -> Option<&[u8]> {
        self.pollution_density.as_deref()
    }

    pub fn pollution_density_sum(&self) -> Option<u32> {
        density_sum(self.pollution_density.as_deref())
    }

    pub fn police_coverage_at(&self, x: usize, y: usize) -> Option<u8> {
        self.police_coverage
            .as_ref()
            .and_then(|data| data.get(city_index(x, y)))
            .copied()
    }

    pub fn police_coverage_slice(&self) -> Option<&[u8]> {
        self.police_coverage.as_deref()
    }

    pub fn fire_coverage_at(&self, x: usize, y: usize) -> Option<u8> {
        self.fire_coverage
            .as_ref()
            .and_then(|data| data.get(city_index(x, y)))
            .copied()
    }

    pub fn fire_coverage_slice(&self) -> Option<&[u8]> {
        self.fire_coverage.as_deref()
    }

    pub fn population_density_at(&self, x: usize, y: usize) -> Option<u8> {
        self.population_density
            .as_ref()
            .and_then(|data| data.get(city_index(x, y)))
            .copied()
    }

    pub fn population_density_slice(&self) -> Option<&[u8]> {
        self.population_density.as_deref()
    }

    pub fn rate_of_growth_at(&self, x: usize, y: usize) -> Option<u8> {
        self.rate_of_growth
            .as_ref()
            .and_then(|data| data.get(city_index(x, y)))
            .copied()
    }

    pub fn rate_of_growth_slice(&self) -> Option<&[u8]> {
        self.rate_of_growth.as_deref()
    }

    /// MISC offset 0x2c / PE BSS `dwCityCrime` @ `0x4cb454`.
    pub fn misc_crime_count(&self) -> Option<u32> {
        Some(self.pe_dw_city_crime)
            .filter(|&value| value != 0)
            .or_else(|| self.misc_u32_at(MISC_CITY_CRIME_OFFSET))
            .or(self.misc_crime_count)
    }

    /// MISC offset 0x30 / PE BSS traffic count @ `0x4cc6f4`.
    pub fn misc_traffic_count(&self) -> Option<u32> {
        Some(self.pe_traffic_count())
            .filter(|&value| value != 0)
            .or_else(|| self.misc_u32_at(MISC_CITY_TRAFFIC_OFFSET))
            .or(self.misc_traffic_count)
    }

    /// MISC offset 0x34 / PE BSS `dwCityPollution` @ `0x4cc910`.
    pub fn misc_pollution_count(&self) -> Option<u32> {
        Some(self.pe_dw_city_pollution)
            .filter(|&value| value != 0)
            .or_else(|| self.misc_u32_at(MISC_CITY_POLLUTION_OFFSET))
            .or(self.misc_pollution_count)
    }
}
