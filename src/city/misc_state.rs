// MISC persistence accessors and synchronization with live City caches.

use super::file_format::MacCrossLayoutSavedState;
use super::scenario_state::{MAC_ACTION_OFFSETS, MacScenarioContinuation};
use super::*;

impl City {
    pub(super) fn mac_cross_layout_saved_state(&self) -> MacCrossLayoutSavedState {
        MacCrossLayoutSavedState {
            rci_counters: self.mac_rci_counters_runtime,
            transit_accumulators: self.mac_transit_accumulators_runtime,
            xbld_census: self.mac_xbld_census_runtime,
            developed_hotspot: self.mac_kind2_hotspot_runtime,
            arcology_total: self.mac_arcology_total_runtime,
            police_gate: self.mac_police_gate_runtime,
            power_summary: self.mac_power_summary_runtime,
            water_summary: self.mac_water_summary_runtime,
            treatment_gate: self.mac_treatment_gate_runtime,
            developed_cells: self.mac_developed_cell_snapshot,
            mac_scenario_active: Some(self.mac_scenario_active()),
            residential_total: self.mac_residential_total_runtime,
            scenario: self.mac_scenario_runtime,
            road_neighbor_links: self.mac_road_neighbor_link_count_runtime,
        }
    }

    pub(crate) fn mac_residential_pair_total(&self) -> i32 {
        self.mac_residential_total_runtime
            .filter(|_| self.kind.is_win95())
            .or_else(|| self.misc_u32_at(MISC_RESIDENTIAL_POPULATION_OFFSET))
            .unwrap_or(0) as i32
    }

    pub(crate) fn set_mac_residential_pair_total(&mut self, total: i32) {
        if self.kind.is_win95() {
            self.mac_residential_total_runtime = Some(total as u32);
        } else {
            self.set_mac_state_misc_u32_at(MISC_RESIDENTIAL_POPULATION_OFFSET, total as u32);
        }
    }

    pub(crate) fn mac_rci_overlay_cap_counts(&self) -> [u16; 2] {
        [
            MAC_XTXT_FA_NETWORK_COUNT_MISC_OFFSET,
            MAC_XTXT_FA_NON_NETWORK_COUNT_MISC_OFFSET,
        ]
        .map(|offset| self.mac_state_misc_u32_at(offset).unwrap_or(0) as u16)
    }

    pub(crate) fn mac_increment_road_neighbor_link_count(&mut self) {
        let count = self.mac_rci_overlay_cap_counts()[0].wrapping_add(1);
        self.set_mac_state_misc_u32_at(MAC_XTXT_FA_NETWORK_COUNT_MISC_OFFSET, u32::from(count));
    }

    pub(crate) fn mac_increment_rail_neighbor_link_count(&mut self) {
        let count = self.mac_rci_overlay_cap_counts()[1].wrapping_add(1);
        self.set_mac_state_misc_u32_at(MAC_XTXT_FA_NON_NETWORK_COUNT_MISC_OFFSET, u32::from(count));
    }

    pub(crate) fn mac_rci_pair_totals(&self) -> [i32; 3] {
        std::array::from_fn(|index| {
            let light = self.mac_rci_counter(index * 2 + 1);
            let dense = self.mac_rci_counter(index * 2 + 2);
            light.wrapping_add(dense) as i32
        })
    }

    /// Mac CODE 2's queued action. The native Mac file stores it at MISC
    /// `+0x70`; Win95 assigns those bytes to its own disaster trigger, so
    /// Mac fidelity on a Win-layout city must keep this selector separately.
    pub(crate) fn mac_pending_action(&self) -> u32 {
        if self.kind == CityFileKind::MacCity {
            self.mac_state_misc_u32_at(MAC_PENDING_ACTION_MISC_OFFSET)
                .unwrap_or(0)
        } else {
            self.mac_pending_action_runtime
        }
    }

    pub(crate) fn set_mac_pending_action(&mut self, selector: u32) {
        if self.kind == CityFileKind::MacCity {
            self.set_mac_state_misc_u32_at(MAC_PENDING_ACTION_MISC_OFFSET, selector);
        } else {
            self.mac_pending_action_runtime = selector;
        }
    }

    /// CODE 8 export 238 clears all three cohort lanes when its current
    /// weighted scalar is zero; it leaves the separate summary fields alone.
    /// Both native layouts persist the same scalar at MISC +0x102c.
    pub(crate) fn mac_clear_cohorts_if_zero_growth(&mut self) -> bool {
        if self.mac_state_misc_u32_at(MISC_NORMAL_POPULATION_OFFSET) != Some(0) {
            return false;
        }
        self.set_budget_misc_u32_values(MISC_EQ_LE_TABLES_OFFSET, &[0; PE_EQ_LE_PACKED_DWORDS]);
        if self.kind.is_win95() {
            // Win-layout saveback mirrors these shared lanes from the PE RAM
            // cache. Keep that cache in sync with the Mac engine's clear.
            self.clear_pe_eq_le_table_cache();
        }
        true
    }

    /// Select the Mac interpretation of shared fields in a Windows-layout city.
    pub(crate) fn prepare_mac_fidelity_from_saved_city(&mut self) {
        if self.kind.is_win95() {
            self.mac_cross_layout_active = true;
            if !self.mac_cross_layout_state_restored {
                // The Mac CODE 4 loader restores these slots directly from
                // MISC before their first monthly consumers. On a first open
                // of a Win-layout city, preserve that native raw-slot
                // interpretation; later opens use the Mac CGEX snapshot.
                self.mac_rci_counters_runtime = self
                    .misc_u32_values(380 * MISC_FIELD_LEN, 8)
                    .try_into()
                    .unwrap_or([0; 8]);
                self.mac_transit_accumulators_runtime = std::array::from_fn(|mode| {
                    self.misc_u32_at((1045 + mode) * MISC_FIELD_LEN)
                        .unwrap_or(0)
                });
                self.mac_power_summary_runtime =
                    self.misc_u32_at(MAC_POWER_SUMMARY_MISC_OFFSET).unwrap_or(0);
                self.mac_water_summary_runtime =
                    self.misc_u32_at(1049 * MISC_FIELD_LEN).unwrap_or(0);
                self.mac_police_gate_runtime =
                    self.misc_u32_at(1039 * MISC_FIELD_LEN).unwrap_or(0) as u16 as i16;
                self.mac_treatment_gate_runtime =
                    Some(self.misc_u32_at(1043 * MISC_FIELD_LEN).unwrap_or(0) as u16 as i16);
            }
            if self.mac_kind2_hotspot_runtime.is_none() {
                // CODE 4 loads the hotspot pair even when no CityGame
                // extension exists. Kind-2 and kind-5 actors can consume it
                // before the day-2 derived-map refresh.
                self.mac_kind2_hotspot_runtime = Some((
                    self.misc_u32_at(MISC_DEVELOPED_CENTER_X_OFFSET)
                        .unwrap_or(0) as u16,
                    self.misc_u32_at(MISC_DEVELOPED_CENTER_Y_OFFSET)
                        .unwrap_or(0) as u16,
                ));
            }
            // Mac initially reads PE-layout MISC +0x78 as the native
            // command-strip latch. Preserve that first-open interpretation,
            // then keep Mac updates separate from PE rewards state.
            self.mac_cross_layout_saved_civic_latch =
                Some(self.mac_cross_layout_saved_civic_latch.unwrap_or_else(|| {
                    self.misc_u32_at(MISC_REWARDS_AVAILABLE_OFFSET).unwrap_or(0)
                }));
            // Older CityGame builds kept this shared Mac/PE field only in
            // CGEX while Mac fidelity ran on a Win-layout save. Migrate that
            // saved value into the native shared MISC slot before simulation
            // resumes so either selected engine loads the same total.
            if let Some(total) = self.mac_arcology_total_runtime {
                self.set_misc_u32_at(MISC_ARCOLOGY_POPULATION_OFFSET, total);
            }
            self.pe_xund_occupancy =
                self.misc_u32_at(MISC_XUND_OCCUPANCY_OFFSET).unwrap_or(0) as u16;
            if !self.mac_cross_layout_state_restored {
                // Both native loaders read the 256 expanded census dwords
                // from MISC +0x1f0. The PE loader later rebuilds its runtime
                // table from XBLD/XZON; Mac keeps the saved low WORDs. Match
                // Mac's first-open interpretation even for a Win-layout file.
                self.mac_xbld_census_runtime = std::array::from_fn(|building| {
                    self.misc_u32_at(MAC_XBLD_CENSUS_MISC_OFFSET + building * MISC_FIELD_LEN)
                        .unwrap_or(0) as u16
                });
            }
        }
    }

    /// Win95 reads the low WORDs of these shared MISC fields at `0x431b6b`
    /// and `0x431b75`. Older short MISC records use the new-city default.
    pub(crate) fn pe_city_center(&self) -> (i16, i16) {
        match self
            .misc_u32_values(MISC_DEVELOPED_CENTER_X_OFFSET, 2)
            .as_slice()
        {
            &[x, y] => (x as i16, y as i16),
            _ => (64, 64),
        }
    }

    pub(crate) fn set_pe_city_center(&mut self, x: i16, y: i16) {
        // The original save writes the WORDs as sign-extended DWORDs.
        self.set_misc_u32_values(MISC_DEVELOPED_CENTER_X_OFFSET, &[x as u32, y as u32]);
    }

    /// CODE 6 state-2 target center loaded from native Mac MISC slots 1030/1031.
    /// On Win-layout saves the corresponding bytes belong to PE state, so use
    /// a separate runtime pair and let the Mac derived-map worker refresh it.
    pub(crate) fn mac_kind2_hotspot(&self) -> Option<(u16, u16)> {
        if self.kind == CityFileKind::MacCity {
            Some((
                self.mac_state_misc_u32_at(MISC_DEVELOPED_CENTER_X_OFFSET)? as u16,
                self.mac_state_misc_u32_at(MISC_DEVELOPED_CENTER_Y_OFFSET)? as u16,
            ))
        } else {
            self.mac_kind2_hotspot_runtime
        }
    }

    pub(crate) fn set_mac_kind2_hotspot(&mut self, x: u16, y: u16) {
        self.mac_kind2_hotspot_runtime = Some((x, y));
        if self.kind == CityFileKind::MacCity {
            self.set_mac_state_misc_u32_at(MISC_DEVELOPED_CENTER_X_OFFSET, u32::from(x));
            self.set_mac_state_misc_u32_at(MISC_DEVELOPED_CENTER_Y_OFFSET, u32::from(y));
        }
    }

    /// New blank cities start with the native all-vacant census, not the
    /// zero-filled MISC placeholder used before the new-city setup runs.
    pub(crate) fn initialize_mac_fidelity_blank_xbld_census(&mut self) {
        if self.mac_cross_layout_active {
            self.mac_xbld_census_runtime = self.pe_dw_tile_count;
        }
    }

    /// Mac `fffff03e` monthly RCI counters occupy MISC slots 380–387.
    /// On Win-layout saves Mac's continuation is stored separately in CGEX.
    pub(crate) fn mac_rci_counter(&self, slot: usize) -> u32 {
        if self.kind == CityFileKind::MacCity {
            self.mac_state_misc_u32_at((380 + slot) * MISC_FIELD_LEN)
                .unwrap_or(0)
        } else {
            self.mac_rci_counters_runtime[slot]
        }
    }

    pub(crate) fn set_mac_rci_counter(&mut self, slot: usize, value: u32) {
        if self.kind == CityFileKind::MacCity {
            self.set_mac_state_misc_u32_at((380 + slot) * MISC_FIELD_LEN, value);
        } else {
            self.mac_rci_counters_runtime[slot] = value;
        }
    }

    /// CODE 4 slots 1045–1047 hold Mac bus, rail and subway route totals.
    /// Win-layout saves keep them in separate runtime state: first-open values
    /// are hydrated from raw MISC, then CityGame persists later Mac values in CGEX.
    pub(crate) fn mac_transit_accumulator(&self, mode: usize) -> u32 {
        if self.kind == CityFileKind::MacCity {
            self.mac_state_misc_u32_at((1045 + mode) * MISC_FIELD_LEN)
                .unwrap_or(0)
        } else {
            self.mac_transit_accumulators_runtime[mode]
        }
    }

    pub(crate) fn set_mac_transit_accumulator(&mut self, mode: usize, value: u32) {
        if self.kind == CityFileKind::MacCity {
            self.set_mac_state_misc_u32_at((1045 + mode) * MISC_FIELD_LEN, value);
        } else {
            self.mac_transit_accumulators_runtime[mode] = value;
        }
    }

    pub(crate) fn mac_power_service_summary(&self) -> u32 {
        if self.kind == CityFileKind::MacCity {
            self.mac_state_misc_u32_at(MAC_POWER_SUMMARY_MISC_OFFSET)
                .unwrap_or(0)
        } else {
            self.mac_power_summary_runtime
        }
    }

    pub(crate) fn set_mac_power_service_summary(&mut self, value: u32) {
        if self.kind == CityFileKind::MacCity {
            self.set_mac_state_misc_u32_at(MAC_POWER_SUMMARY_MISC_OFFSET, value);
        } else {
            self.mac_power_summary_runtime = value;
        }
    }

    pub(crate) fn mac_water_service_summary(&self) -> u32 {
        if self.kind == CityFileKind::MacCity {
            self.mac_state_misc_u32_at(1049 * MISC_FIELD_LEN)
                .unwrap_or(0)
        } else {
            self.mac_water_summary_runtime
        }
    }

    pub(crate) fn set_mac_water_service_summary(&mut self, value: u32) {
        if self.kind == CityFileKind::MacCity {
            self.set_mac_state_misc_u32_at(1049 * MISC_FIELD_LEN, value);
        } else {
            self.mac_water_summary_runtime = value;
        }
    }

    /// Mac `fffffe12`: water-treatment gate, saved in Mac MISC dword 1043.
    pub(crate) fn mac_treatment_gate(&self) -> i16 {
        if self.kind == CityFileKind::MacCity {
            self.mac_state_misc_u32_at(1043 * MISC_FIELD_LEN)
                .unwrap_or(0) as u16 as i16
        } else {
            // A Win95 save has no Mac treatment-gate field at this offset.
            // Until the Mac worker publishes a value, use an independent
            // runtime default rather than interpreting unrelated PE bytes.
            self.mac_treatment_gate_runtime.unwrap_or(0)
        }
    }

    pub(crate) fn set_mac_treatment_gate(&mut self, gate: i16) {
        if self.kind == CityFileKind::MacCity {
            self.set_mac_state_misc_u32_at(1043 * MISC_FIELD_LEN, i32::from(gate) as u32);
        } else {
            self.mac_treatment_gate_runtime = Some(gate);
        }
    }

    /// Mac `fffffdfc` and PE industrial pollution bonus share the saved WORD.
    pub(crate) fn mac_pollution_adjustment(&self) -> i16 {
        self.mac_state_misc_u32_at(1037 * MISC_FIELD_LEN)
            .unwrap_or(0) as u16 as i16
    }

    #[cfg(test)]
    pub(crate) fn set_mac_pollution_adjustment_for_test(&mut self, value: i16) {
        self.set_mac_state_misc_u32_at(1037 * MISC_FIELD_LEN, i32::from(value) as u32);
    }

    /// Mac `fffffe00` is saved at +0x103c only in the Mac MISC layout.
    pub(crate) fn mac_police_coverage_gate(&self) -> i16 {
        if self.kind == CityFileKind::MacCity {
            self.mac_state_misc_u32_at(1039 * MISC_FIELD_LEN)
                .unwrap_or(0) as u16 as i16
        } else {
            self.mac_police_gate_runtime
        }
    }

    pub(crate) fn set_mac_police_coverage_gate(&mut self, gate: i16) {
        if self.kind == CityFileKind::MacCity {
            self.set_mac_state_misc_u32_at(1039 * MISC_FIELD_LEN, i32::from(gate) as u32);
        } else {
            self.mac_police_gate_runtime = gate;
        }
    }

    pub fn misc_humidity(&self) -> Option<u32> {
        self.misc_humidity
    }

    pub fn misc_water_table_level(&self) -> Option<u32> {
        // PE loads only the selected signed WORD, then adds its sign-extended
        // value to a DWORD capacity accumulator. Retain the original MISC
        // payload separately; this accessor exposes the native runtime bits.
        self.misc_water_table_level
            .map(|value| i32::from(value as u16 as i16) as u32)
    }
}

impl City {
    /// PE runtime's technology discovery year; `0` means already invented.
    /// PE BSS neighbor-fame / tech-year WORD[17] @ `0x4cb430` (MISC `0x738`).
    pub fn technology_discovery_year(&self, technology: Technology) -> Option<u32> {
        Technology::ALL
            .iter()
            .position(|&item| item == technology)
            .map(|index| u32::from(self.pe_w_neighbor_fame_ranks[index]))
    }

    pub fn set_technology_discovery_year(&mut self, technology: Technology, year: u32) {
        if let Some(index) = Technology::ALL.iter().position(|&item| item == technology) {
            self.pe_w_neighbor_fame_ranks[index] = year.min(u32::from(u16::MAX)) as u16;
        }
        // Mac CODE 4 uses the same serialized offsets for its own technology
        // dates. Keep this PE-cache setter from mirroring into a Mac container;
        // engine-specific writers must own cross-layout persistence explicitly.
        if self.kind.is_win95() {
            self.set_misc_u32_at(technology.misc_offset(), year);
        }
    }

    pub fn apply_default_new_city_technology_schedule(&mut self) {
        for (technology, year) in DEFAULT_NEW_CITY_TECHNOLOGY_DISCOVERY_YEARS {
            self.set_technology_discovery_year(technology, year);
        }
    }

    /// Mac CODE 11 New City schedule: base-year + QuickDraw `_Random() % 20`,
    /// cleared when earlier than the selected start year. The saved slots are
    /// shared with PE, but Mac writes the native MISC view for either layout.
    pub fn mac_reseed_technology_dates_00041564(
        &mut self,
        starting_year: u16,
        rng: &mut MacToolboxRng,
    ) {
        for (index, &base_year) in TECHNOLOGY_BASE_DISCOVERY_YEARS.iter().enumerate() {
            let mut year = base_year + rng.random_u16() % 20;
            if year < starting_year {
                year = 0;
            }
            let technology = Technology::ALL[index];
            self.set_mac_state_misc_u32_at(technology.misc_offset(), u32::from(year));
            // Win-layout saveback mirrors the shared MISC words from this
            // cache. Keep it synchronized while Mac fidelity owns the table.
            if self.kind.is_win95() {
                self.pe_w_neighbor_fame_ranks[index] = year;
            }
        }
    }

    /// Read one big-endian unsigned value from the decoded `MISC` chunk.
    pub fn misc_u32_at(&self, offset: usize) -> Option<u32> {
        decode_misc_u32_for_kind(&self.chunks, offset, self.kind).ok()
    }

    /// Read a row-book dword through the file-format-specific persistence view.
    pub fn budget_misc_u32_at(&self, offset: usize) -> Option<u32> {
        self.budget_misc_u32_values(offset, 1).first().copied()
    }

    /// Read a Mac engine state dword through the preserved Mac MISC region.
    /// The caller chooses the Mac state offset; Win95 engine state should use
    /// `misc_u32_at` instead. Mac's command latch and scenario actions use
    /// separate runtime fields on PE-layout files.
    pub fn mac_state_misc_u32_at(&self, offset: usize) -> Option<u32> {
        if offset == MAC_XTXT_FA_NETWORK_COUNT_MISC_OFFSET
            && self.kind.is_win95()
            && let Some(count) = self.mac_road_neighbor_link_count_runtime
        {
            return Some(u32::from(count));
        }
        if self.kind.is_win95()
            && let Some(index) = MAC_ACTION_OFFSETS.iter().position(|&slot| slot == offset)
        {
            return Some(self.mac_scenario_runtime.unwrap_or_default().action[index]);
        }
        if offset == MISC_REWARDS_AVAILABLE_OFFSET
            && self.kind.is_win95()
            && self.mac_cross_layout_active
        {
            return self.mac_cross_layout_saved_civic_latch;
        }
        self.budget_misc_u32_at(offset)
    }

    /// Mac CODE 14's signed progression WORD is sign-extended in MISC.
    pub(crate) fn mac_city_progression(&self) -> i16 {
        self.mac_state_misc_u32_at(MISC_CITY_PROGRESSION_OFFSET)
            .unwrap_or(0) as i16
    }

    pub(crate) fn set_mac_city_progression(&mut self, level: i16) {
        self.set_mac_state_misc_u32_at(MISC_CITY_PROGRESSION_OFFSET, i32::from(level) as u32);
    }

    pub(crate) fn mac_civic_latch(&self) -> u32 {
        self.mac_state_misc_u32_at(MISC_REWARDS_AVAILABLE_OFFSET)
            .unwrap_or(0)
    }

    pub(crate) fn set_mac_civic_latch(&mut self, latch: u32) {
        self.set_mac_state_misc_u32_at(MISC_REWARDS_AVAILABLE_OFFSET, latch);
    }

    /// Mac export 240 counts zero date WORDs in slots 12–15 before asking
    /// export 104 to set the arcology latch. Export 104 also requires level 6.
    pub(crate) fn refresh_mac_arcology_latch(&mut self) {
        if self.mac_city_progression() >= 6
            && Technology::ALL[12..16].iter().any(|technology| {
                self.mac_state_misc_u32_at(technology.misc_offset())
                    .is_some_and(|date| date as u16 == 0)
            })
        {
            self.set_mac_civic_latch(self.mac_civic_latch() | (1 << 4));
        }
    }

    /// Mac CODE 14 developed-cell WORD `ffffbb50`, saved in MISC slot 1067.
    /// Win-layout cities keep this Mac simulation snapshot independently.
    pub(crate) fn mac_developed_cells(&self) -> Option<u16> {
        self.mac_developed_cell_snapshot.or_else(|| {
            (self.kind == CityFileKind::MacCity)
                .then(|| self.mac_state_misc_u32_at(0x10ac).map(|value| value as u16))
                .flatten()
        })
    }

    pub(crate) fn set_mac_developed_cells(&mut self, value: u16) {
        self.mac_developed_cell_snapshot = Some(value);
        if self.kind == CityFileKind::MacCity {
            // Native save sign-extends the WORD when serializing this slot.
            self.set_mac_state_misc_u32_at(0x10ac, i32::from(value as i16) as u32);
        }
    }

    /// Return the twenty ordinance bits from the storage slot owned by this
    /// city's file family, independent of the selected simulation engine.
    pub fn ordinance_flags_for_saved_layout(&self) -> u32 {
        let flags = if self.kind == CityFileKind::MacCity {
            self.mac_state_misc_u32_at(MAC_ORDINANCE_FLAGS_MISC_OFFSET)
        } else {
            self.misc_u32_at(MISC_ORDINANCE_FLAGS_OFFSET)
        };
        flags.unwrap_or(0) & ORDINANCE_FLAGS_MASK
    }

    /// Mac `fffffdf6` and PE `0x4ca818` both load the low WORD of MISC
    /// slot 912 and save it sign-extended. Their generators/classifiers use
    /// it as the selected terrain/water height, independent of saved layout.
    pub fn mac_selected_water_height(&self) -> Option<u32> {
        self.mac_state_misc_u32_at(MAC_SELECTED_WATER_HEIGHT_STATE_OFFSET)
            .map(|state| u32::from(state as u16))
    }

    /// Store the Mac Make Map selected terrain/water height in packed MISC slot 912.
    /// Native saveback sign-extends the selected WORD and writes a full dword.
    pub fn set_mac_selected_water_height(&mut self, height: u8) {
        self.set_selected_water_height_word(i16::from(height));
    }

    /// Both native editions save the selected terrain/water WORD sign-extended
    /// into MISC slot 912. Preserve that width for the global height controls.
    pub(crate) fn set_selected_water_height_word(&mut self, height: i16) {
        self.set_mac_state_misc_u32_at(
            MAC_SELECTED_WATER_HEIGHT_STATE_OFFSET,
            i32::from(height) as u32,
        );
    }

    /// Mac water-worker humidity input (`fffff086`) is the low byte of saved
    /// MISC dword 26. PE also loads/saves this byte at `+0x68`; the two
    /// Pump workers consume the same humidity input through their own paths.
    pub fn mac_weather_humidity(&self) -> Option<u32> {
        self.mac_state_misc_u32_at(MAC_WEATHER_HUMIDITY_STATE_OFFSET)
            .map(|state| state & 0xff)
    }

    /// Write a Mac engine state dword without interpreting its overlapping
    /// bytes as Win95 runtime-cache updates. On non-Mac files, shared state
    /// uses the ordinary MISC writer and Mac-only fields use the extension.
    pub fn set_mac_state_misc_u32_at(&mut self, offset: usize, value: u32) {
        if offset == MAC_XTXT_FA_NETWORK_COUNT_MISC_OFFSET && self.kind.is_win95() {
            self.mac_road_neighbor_link_count_runtime = Some(value as u16);
            self.modified = true;
            return;
        }
        if self.kind.is_win95()
            && let Some(index) = MAC_ACTION_OFFSETS.iter().position(|&slot| slot == offset)
        {
            self.mac_scenario_runtime
                .get_or_insert_with(MacScenarioContinuation::default)
                .action[index] = value;
            self.modified = true;
            return;
        }
        if offset == MISC_REWARDS_AVAILABLE_OFFSET
            && self.kind.is_win95()
            && self.mac_cross_layout_active
        {
            self.mac_cross_layout_saved_civic_latch = Some(value);
            return;
        }
        self.set_budget_misc_u32_values(offset, &[value]);
        if self.kind == CityFileKind::MacCity {
            self.sync_shared_pe_cache_from_mac_misc(offset, value);
        }
    }

    fn sync_shared_pe_cache_from_mac_misc(&mut self, offset: usize, value: u32) {
        match offset {
            MISC_YEAR_FOUNDED_OFFSET => {
                self.pe_w_year_founded = misc_year_founded_word(value, self.kind);
            }
            MISC_CITY_AGE_OFFSET => self.pe_dw_city_age = value,
            MISC_CITY_FUNDS_OFFSET => self.pe_i32_city_funds = value as i32,
            MISC_CITY_BONDS_OFFSET => self.pe_dw_city_bonds = value,
            MISC_DIFFICULTY_OFFSET => {
                self.pe_w_city_difficulty = misc_word_from_saved_dword(value, self.kind);
            }
            MISC_CITY_PROGRESSION_OFFSET => self.pe_w_city_progression = value as u16,
            MISC_REWARDS_AVAILABLE_OFFSET => self.pe_dw_rewards_available = value,
            MISC_CITY_VALUE_OFFSET => self.pe_city_value = value as i32,
            MISC_LAND_VALUE_OFFSET => self.pe_dw_city_land_value = value,
            MISC_SPORTS_TEAMS_OFFSET if self.pe_cross_layout_active => {
                self.pe_w_sports_teams = value as u16;
            }
            MISC_MILITARY_BASE_TYPE_OFFSET if self.pe_cross_layout_active => {
                self.pe_w_military_base_type = u32::from(value as u8);
            }
            MISC_NATIONAL_POPULATION_OFFSET => self.pe_dw_national_population = value,
            MISC_NATIONAL_VALUE_OFFSET => self.pe_dw_national_value = value,
            MISC_NATIONAL_TAX_OFFSET => {
                self.pe_w_national_tax = misc_word_from_saved_dword(value, self.kind);
            }
            MISC_NATIONAL_TREND_OFFSET => {
                self.pe_w_national_trend = misc_word_from_saved_dword(value, self.kind);
            }
            MISC_HEAT_OFFSET => {
                self.pe_b_weather_heat = weather_cache_byte_from_misc(value);
            }
            MISC_WIND_OFFSET => {
                self.pe_b_weather_wind = weather_cache_byte_from_misc(value);
            }
            MISC_HUMIDITY_OFFSET => {
                let byte = weather_cache_byte_from_misc(value);
                self.misc_humidity = Some(u32::from(byte));
                self.pe_b_weather_humid = byte;
            }
            MISC_WEATHER_TREND_OFFSET => {
                self.pe_b_weather_trend = weather_cache_byte_from_misc(value);
            }
            MISC_WATER_TABLE_LEVEL_OFFSET => self.misc_water_table_level = Some(value),
            _ => {}
        }
    }

    /// Update the low byte of a Mac engine state slot while preserving the
    /// other bytes in its serialized MISC dword.
    pub fn set_mac_state_misc_low_byte_at(&mut self, offset: usize, value: u8) {
        let prior = self.mac_state_misc_u32_at(offset).unwrap_or(0);
        self.set_mac_state_misc_u32_at(offset, (prior & !0xff) | u32::from(value));
    }

    /// Update the low WORD of a Mac state slot while preserving its upper WORD.
    pub fn set_mac_state_misc_low_word_at(&mut self, offset: usize, value: u16) {
        let prior = self.mac_state_misc_u32_at(offset).unwrap_or(0);
        self.set_mac_state_misc_u32_at(offset, (prior & 0xffff_0000) | u32::from(value));
    }

    /// Mac CODE 14 scenario-active flag (`fffffdf0`), persisted in state slot 1050.
    pub fn mac_scenario_active(&self) -> bool {
        if self.kind.is_win95()
            && let Some(active) = self.mac_scenario_active_runtime
        {
            return active;
        }
        self.mac_state_misc_u32_at(MAC_SCENARIO_ACTIVE_STATE_OFFSET)
            .is_some_and(|state| state as u8 != 0)
    }

    pub fn set_mac_scenario_active(&mut self, active: bool) {
        if self.kind.is_win95() {
            self.mac_scenario_active_runtime = Some(active);
            self.modified = true;
        } else {
            self.set_mac_state_misc_low_byte_at(MAC_SCENARIO_ACTIVE_STATE_OFFSET, u8::from(active));
        }
    }

    /// Read a row-book range. Mac rows use a preserved snapshot because PE
    /// runtime MISC fields overlap this region; Win95 continues reading MISC.
    pub fn budget_misc_u32_values(&self, offset: usize, count: usize) -> Vec<u32> {
        let byte_len = match count.checked_mul(MISC_FIELD_LEN) {
            Some(byte_len) => byte_len,
            None => return Vec::new(),
        };
        if self.kind == CityFileKind::MacCity
            && let Some(book) = self.mac_budget_book.as_deref()
            && let Some(relative) = offset.checked_sub(MAC_MISC_BUDGET_BOOK_OFFSET)
            && let Some(end) = relative.checked_add(byte_len)
            && let Some(bytes) = book.get(relative..end)
        {
            return bytes
                .chunks_exact(MISC_FIELD_LEN)
                .map(|chunk| u32::from_be_bytes(chunk.try_into().expect("four bytes")))
                .collect();
        }
        self.misc_u32_values(offset, count)
    }

    /// Write a row-book range through the file-format-specific persistence view.
    pub fn set_budget_misc_u32_values(&mut self, offset: usize, values: &[u32]) {
        if self.kind != CityFileKind::MacCity {
            self.set_misc_u32_values(offset, values);
            return;
        }
        let Some(byte_len) = values.len().checked_mul(MISC_FIELD_LEN) else {
            return;
        };
        let Some(relative) = offset.checked_sub(MAC_MISC_BUDGET_BOOK_OFFSET) else {
            self.set_misc_u32_values_preserving_pe_caches(offset, values);
            return;
        };
        let Some(end) = relative.checked_add(byte_len) else {
            return;
        };
        let book_len = MISC_BUDGET_ROW_DWORDS * MISC_BUDGET_ROW_COUNT * MISC_FIELD_LEN;
        if end > book_len {
            self.set_misc_u32_values_preserving_pe_caches(offset, values);
            return;
        }
        if self.mac_budget_book.is_none() {
            let raw = decode_misc_bytes_for_kind(&self.chunks, self.kind).unwrap_or_default();
            let mut book = vec![0; book_len];
            if let Some(saved) = raw.get(MAC_MISC_BUDGET_BOOK_OFFSET..) {
                let count = saved.len().min(book_len);
                book[..count].copy_from_slice(&saved[..count]);
            }
            self.mac_budget_book = Some(book);
        }
        if let Some(book) = self.mac_budget_book.as_mut() {
            for (index, value) in values.iter().copied().enumerate() {
                let at = relative + index * MISC_FIELD_LEN;
                book[at..at + MISC_FIELD_LEN].copy_from_slice(&value.to_be_bytes());
            }
            self.modified = true;
        }
    }

    pub(super) fn sync_mac_budget_book_to_misc(&mut self) {
        let Some(book) = self.mac_budget_book.as_deref() else {
            return;
        };
        let mut misc = decode_misc_bytes_for_kind(&self.chunks, self.kind).unwrap_or_default();
        let start = MAC_MISC_BUDGET_BOOK_OFFSET;
        let end = start + book.len();
        misc.resize(misc.len().max(end), 0);
        misc[start..end].copy_from_slice(book);
        upsert_chunk_for_kind(
            &mut self.chunks,
            &mut self.chunk_index,
            "MISC",
            &misc,
            self.kind,
        );
    }

    /// Read one big-endian signed value from the decoded `MISC` chunk.
    pub fn misc_i32_at(&self, offset: usize) -> Option<i32> {
        self.misc_u32_at(offset).map(|value| value as i32)
    }

    /// Read a contiguous sequence of big-endian unsigned values from `MISC` in one decode pass.
    pub fn misc_u32_values(&self, offset: usize, count: usize) -> Vec<u32> {
        decode_misc_bytes_for_kind(&self.chunks, self.kind)
            .and_then(|data| {
                let byte_len = count.checked_mul(4)?;
                let end = offset.checked_add(byte_len)?;
                data.get(offset..end).map(|bytes| {
                    bytes
                        .chunks_exact(4)
                        .map(|chunk| u32::from_be_bytes(chunk.try_into().expect("four bytes")))
                        .collect()
                })
            })
            .unwrap_or_default()
    }

    /// Read a contiguous sequence of big-endian signed values from `MISC` in one decode pass.
    pub fn misc_i32_values(&self, offset: usize, count: usize) -> Vec<i32> {
        self.misc_u32_values(offset, count)
            .into_iter()
            .map(|value| value as i32)
            .collect()
    }

    /// Write one big-endian unsigned value into the decoded `MISC` chunk.
    pub fn set_misc_u32_at(&mut self, offset: usize, value: u32) {
        self.set_misc_u32_values(offset, &[value]);
    }

    /// Write the shared saved traffic total without synchronizing the Win95-only
    /// `dwCityTrafficCount` cache. Mac phase 19 writes this MISC field from its
    /// own XTRF sum, while the PE global is a distinct runtime value.
    pub fn set_misc_city_traffic_total_preserving_pe_cache(&mut self, traffic: u32) {
        let pe_traffic_count = self.pe_traffic_count;
        self.set_misc_u32_at(MISC_CITY_TRAFFIC_OFFSET, traffic);
        self.pe_traffic_count = pe_traffic_count;
    }

    /// Write a contiguous sequence of big-endian unsigned values into the decoded `MISC` chunk.
    pub fn set_misc_u32_values(&mut self, offset: usize, values: &[u32]) {
        self.set_misc_u32_values_with_cache_sync(offset, values, true);
    }

    /// Store Mac-owned MISC bytes without interpreting them as Win95 cache
    /// updates that happen to share the same offsets.
    fn set_misc_u32_values_preserving_pe_caches(&mut self, offset: usize, values: &[u32]) {
        self.set_misc_u32_values_with_cache_sync(offset, values, false);
    }

    fn set_misc_u32_values_with_cache_sync(
        &mut self,
        offset: usize,
        values: &[u32],
        sync_pe_caches: bool,
    ) {
        const FIELD_LEN: usize = 4;
        let Some(byte_len) = values.len().checked_mul(FIELD_LEN) else {
            return;
        };
        let Some(end) = offset.checked_add(byte_len) else {
            return;
        };
        if sync_pe_caches && offset < MISC_RCI_DEMAND_OFFSET + 12 && end > MISC_RCI_DEMAND_OFFSET {
            self.pe_rci_demand_initialized = true;
        }

        let mut misc = decode_misc_bytes_for_kind(&self.chunks, self.kind).unwrap_or_default();
        misc.resize(misc.len().max(end), 0);
        for (index, value) in values.iter().copied().enumerate() {
            let field_offset = offset + index * FIELD_LEN;
            misc[field_offset..field_offset + FIELD_LEN].copy_from_slice(&value.to_be_bytes());
        }
        upsert_chunk_for_kind(
            &mut self.chunks,
            &mut self.chunk_index,
            "MISC",
            &misc,
            self.kind,
        );
        if !sync_pe_caches {
            return;
        }
        if self.kind.is_win95() {
            if let Some(value) =
                misc_value_written_at(offset, values, MISC_CITY_UNEMPLOYMENT_OFFSET)
            {
                self.pe_unemployment_percent = value;
            }
        }
        if let Some(value) = misc_value_written_at(offset, values, MISC_HUMIDITY_OFFSET) {
            let weather_byte = weather_cache_byte_from_misc(value);
            self.misc_humidity = Some(u32::from(weather_byte));
            self.pe_b_weather_humid = weather_byte;
        }
        if let Some(value) = misc_value_written_at(offset, values, MISC_WEATHER_TREND_OFFSET) {
            self.pe_b_weather_trend = weather_cache_byte_from_misc(value);
        }
        if let Some(value) = misc_value_written_at(offset, values, MISC_HEAT_OFFSET) {
            self.pe_b_weather_heat = weather_cache_byte_from_misc(value);
        }
        if let Some(value) = misc_value_written_at(offset, values, MISC_WIND_OFFSET) {
            self.pe_b_weather_wind = weather_cache_byte_from_misc(value);
        }
        if let Some(value) = misc_value_written_at(offset, values, MISC_TRIGGER_DISASTER_OFFSET) {
            self.pe_w_trigger_disaster = value.min(u32::from(u16::MAX)) as u16;
        }
        if let Some(value) =
            misc_value_written_at(offset, values, MISC_RESIDENTIAL_POPULATION_OFFSET)
        {
            self.pe_dw_residential_population = value;
        }
        if let Some(value) = misc_value_written_at(offset, values, MISC_REWARDS_AVAILABLE_OFFSET) {
            self.pe_dw_rewards_available = value;
        }
        if let Some(value) = misc_value_written_at(offset, values, MISC_ARCOLOGY_POPULATION_OFFSET)
        {
            self.pe_dw_arcology_population = value;
        }
        if let Some(value) = misc_value_written_at(offset, values, MISC_SPORTS_TEAMS_OFFSET) {
            self.pe_w_sports_teams = value as u16;
        }
        if let Some(value) = misc_value_written_at(offset, values, MISC_XUND_OCCUPANCY_OFFSET) {
            self.pe_xund_occupancy = value as u16;
        }
        for index in 0..MILITARY_TILE_COUNT_VALUES {
            if let Some(value) =
                misc_value_written_at(offset, values, MISC_MILITARY_TILE_COUNTS_OFFSET + index * 4)
            {
                self.pe_military_tile_count[index] = value as u16;
            }
        }
        for (index, technology) in Technology::ALL.into_iter().enumerate() {
            if let Some(value) = misc_value_written_at(offset, values, technology.misc_offset()) {
                self.pe_w_neighbor_fame_ranks[index] = value.min(u32::from(u16::MAX)) as u16;
            }
        }
        if self.kind.is_win95()
            && let Some(value) =
                misc_value_written_at(offset, values, MISC_INDUSTRIAL_MIX_BONUS_OFFSET)
        {
            self.pe_w_industrial_mix_bonus = if self.kind == CityFileKind::MacCity {
                value as u16
            } else {
                value.min(u32::from(u16::MAX)) as u16
            };
        }
        if self.kind.is_win95()
            && let Some(value) =
                misc_value_written_at(offset, values, MISC_INDUSTRIAL_MIX_POLLUTION_BONUS_OFFSET)
        {
            self.pe_w_industrial_mix_pollution_bonus = value as i16;
        }
        if let Some(value) = misc_value_written_at(offset, values, MISC_OLD_ARRESTS_OFFSET) {
            self.pe_w_old_arrests = value.min(u32::from(u16::MAX)) as u16;
        }
        if let Some(value) = misc_value_written_at(offset, values, MISC_PRISON_BONUS_OFFSET) {
            self.pe_w_prison_bonus = value.min(u32::from(u16::MAX)) as u16;
        }
        if let Some(value) = misc_value_written_at(offset, values, MISC_SEWER_BONUS_OFFSET) {
            self.pe_w_sewer_bonus = value.min(u32::from(u16::MAX)) as u16;
        }
        if let Some(value) = misc_value_written_at(offset, values, MISC_MONSTER_XTHG_INDEX_OFFSET) {
            self.pe_w_monster_xthg_index = value.min(u32::from(u16::MAX)) as u16;
        }
        if let Some(value) =
            misc_value_written_at(offset, values, MISC_CURRENT_DISASTER_TYPE_OFFSET)
        {
            self.pe_w_current_disaster_type = value.min(u32::from(u16::MAX)) as u16;
        }
        if let Some(value) = misc_value_written_at(offset, values, MISC_DISASTER_ACTIVE_OFFSET) {
            self.pe_dw_disaster_active = value;
        }
        if let Some(value) = misc_value_written_at(offset, values, MISC_CITY_HAS_OCEAN_OFFSET) {
            self.pe_dw_city_has_ocean = value;
        }
        if let Some(value) = misc_value_written_at(offset, values, MISC_CITY_HAS_RIVER_OFFSET) {
            self.pe_dw_city_has_river = value;
        }
        if let Some(value) = misc_value_written_at(offset, values, MISC_POWER_USED_PERCENT_OFFSET) {
            self.pe_power_used_percent = value;
        }
        if let Some(value) = misc_value_written_at(offset, values, MISC_BUS_PASSENGERS_OFFSET) {
            self.pe_dw_bus_passengers = value;
        }
        if let Some(value) = misc_value_written_at(offset, values, MISC_RAIL_PASSENGERS_OFFSET) {
            self.pe_dw_rail_passengers = value;
        }
        if let Some(value) = misc_value_written_at(offset, values, MISC_SUBWAY_PASSENGERS_OFFSET) {
            self.pe_dw_subway_passengers = value;
        }
        if let Some(value) = misc_value_written_at(offset, values, MISC_NATIONAL_POPULATION_OFFSET)
        {
            self.pe_dw_national_population = value;
        }
        if let Some(value) = misc_value_written_at(offset, values, MISC_NATIONAL_VALUE_OFFSET) {
            self.pe_dw_national_value = value;
        }
        if let Some(value) = misc_value_written_at(offset, values, MISC_NATIONAL_TAX_OFFSET) {
            self.pe_w_national_tax = misc_word_from_saved_dword(value, self.kind);
        }
        if let Some(value) = misc_value_written_at(offset, values, MISC_NATIONAL_TREND_OFFSET) {
            self.pe_w_national_trend = misc_word_from_saved_dword(value, self.kind);
        }
        if let Some(value) = misc_value_written_at(offset, values, MISC_CITY_FUNDS_OFFSET) {
            self.pe_i32_city_funds = value as i32;
        }
        if let Some(value) = misc_value_written_at(offset, values, MISC_CITY_AGE_OFFSET) {
            self.pe_dw_city_age = value;
        }
        if let Some(value) = misc_value_written_at(offset, values, MISC_DIFFICULTY_OFFSET) {
            self.pe_w_city_difficulty = misc_word_from_saved_dword(value, self.kind);
        }
        if let Some(value) = misc_value_written_at(offset, values, MISC_YEAR_FOUNDED_OFFSET) {
            self.pe_w_year_founded = misc_year_founded_word(value, self.kind);
        }
        if let Some(value) = misc_value_written_at(offset, values, MISC_CITY_BONDS_OFFSET) {
            self.pe_dw_city_bonds = value;
        }
        if let Some(value) = misc_value_written_at(offset, values, MISC_CITY_MODE_OFFSET) {
            self.pe_w_city_mode = value.min(u32::from(u16::MAX)) as u16;
        }
        if let Some(value) = misc_value_written_at(offset, values, MISC_CITY_ROTATION_OFFSET) {
            self.pe_w_city_rotation = value as u16;
        }
        if let Some(value) = misc_value_written_at(offset, values, MISC_CITY_PROGRESSION_OFFSET) {
            self.pe_w_city_progression = value as u16;
        }
        if self.kind != CityFileKind::MacCity {
            if let Some(value) =
                misc_value_written_at(offset, values, MISC_MILITARY_BASE_TYPE_OFFSET)
            {
                self.pe_w_military_base_type = u32::from(value as u8);
            }
        }
        if let Some(value) = misc_value_written_at(offset, values, MISC_CITY_VALUE_OFFSET) {
            self.pe_city_value = value as i32;
        }
        if let Some(value) = misc_value_written_at(offset, values, MISC_LAND_VALUE_OFFSET) {
            self.pe_dw_city_land_value = value;
        }
        if let Some(value) = misc_value_written_at(offset, values, MISC_CITY_CRIME_OFFSET) {
            self.pe_dw_city_crime = value;
        }
        if let Some(value) = misc_value_written_at(offset, values, MISC_CITY_TRAFFIC_OFFSET) {
            self.set_pe_traffic_count(value);
        }
        if let Some(value) = misc_value_written_at(offset, values, MISC_CITY_POLLUTION_OFFSET) {
            self.pe_dw_city_pollution = value;
        }
        if self.kind.is_win95() {
            if let Some(value) = misc_value_written_at(offset, values, MISC_CITY_FAME_OFFSET) {
                self.pe_dw_city_fame = value;
            }
            if let Some(value) = misc_value_written_at(offset, values, MISC_CITY_ADVERTISING_OFFSET)
            {
                self.pe_dw_city_advertising = value;
            }
        }
        if let Some(value) = misc_value_written_at(offset, values, MISC_GARBAGE_OFFSET) {
            self.pe_w_city_garbage = value;
        }
        if let Some(value) = misc_value_written_at(offset, values, MISC_WORK_FORCE_PERCENT_OFFSET) {
            self.pe_eq_workforce_percent = value;
            self.pe_eq_averages_ready = self.pe_eq_averages_ready
                || value != 0
                || self.pe_eq_life_average != 0
                || self.pe_eq_education_average != 0;
        }
        if let Some(value) = misc_value_written_at(offset, values, MISC_CITY_LIFE_EXPECTANCY_OFFSET)
        {
            self.pe_eq_life_average = value;
            self.pe_city_pollution = value;
            self.pe_eq_averages_ready = self.pe_eq_averages_ready
                || value != 0
                || self.pe_eq_workforce_percent != 0
                || self.pe_eq_education_average != 0;
        }
        if let Some(value) =
            misc_value_written_at(offset, values, MISC_CITY_EDUCATION_QUOTIENT_OFFSET)
        {
            self.pe_eq_education_average = value;
            self.pe_city_crime = value;
            self.pe_eq_averages_ready = self.pe_eq_averages_ready
                || value != 0
                || self.pe_eq_workforce_percent != 0
                || self.pe_eq_life_average != 0;
        }
        if let Some(value) = misc_value_written_at(offset, values, MISC_WATER_TABLE_LEVEL_OFFSET) {
            self.misc_water_table_level = Some(value);
        }
        self.modified = true;
    }

    pub(crate) fn pe_rci_demand_initialized(&self) -> bool {
        self.pe_rci_demand_initialized
    }
}

fn misc_value_written_at(offset: usize, values: &[u32], target_offset: usize) -> Option<u32> {
    let relative = target_offset.checked_sub(offset)?;
    if relative % MISC_FIELD_LEN != 0 {
        return None;
    }
    values.get(relative / MISC_FIELD_LEN).copied()
}

/// Both native loaders hydrate weather bytes from the low BYTE of each
/// serialized MISC dword, independently of the selected saved layout.
pub(super) fn weather_cache_byte_from_misc(value: u32) -> u8 {
    // Both native loaders take the low BYTE of each saved weather DWORD.
    value as u8
}

pub(super) fn misc_year_founded_word(value: u32, kind: CityFileKind) -> u16 {
    misc_word_from_saved_dword(value, kind)
}

pub(super) fn misc_year_founded_saved_value(year: u16, kind: CityFileKind) -> u32 {
    misc_word_to_saved_dword(year, kind)
}

pub(super) fn misc_word_from_saved_dword(value: u32, kind: CityFileKind) -> u16 {
    if kind == CityFileKind::MacCity {
        value as u16
    } else {
        value.min(u32::from(u16::MAX)) as u16
    }
}

pub(super) fn misc_word_to_saved_dword(value: u16, kind: CityFileKind) -> u32 {
    if kind == CityFileKind::MacCity {
        // CODE 4 uses MOVEA.W then MOVE.L for these WORD-valued slots: sign-
        // extend the WORD to the full MISC dword, matching native saveback.
        i32::from(value as i16) as u32
    } else {
        u32::from(value)
    }
}
