//! City layer-to-chunk synchronization and disk save path.

use super::*;

/// PE rotation permutes the first 112 WORD census entries through its
/// direction table. This differs from display artwork for bridge IDs 5d..60,
/// whose count slots remain fixed in both original rotation tables.
pub(crate) fn pe_rotated_tile_census(counts: &[u16; 256], clockwise: bool) -> [u16; 256] {
    let mut result = *counts;
    for id in 0..112_u8 {
        let target = if (0x5d..=0x60).contains(&id) {
            id
        } else {
            crate::tiles::rotated_building_id(id, if clockwise { 1 } else { 3 })
        };
        result[usize::from(target)] = counts[usize::from(id)];
    }
    result
}

fn pe_special_rotation_census_update(
    counts: &mut [u16; 256], military: &mut [u16; 16], zone: u8, old: u8, new: u8,
) {
    if zone & 0x0f == 7 {
        let a = City::pe_military_tile_count_index(old);
        let b = City::pe_military_tile_count_index(new);
        military[a] = military[a].wrapping_sub(1);
        military[b] = military[b].wrapping_add(1);
    } else {
        counts[old as usize] = counts[old as usize].wrapping_sub(1);
        counts[new as usize] = counts[new as usize].wrapping_add(1);
    }
}

#[cfg(test)]
mod pe_rotation_tests {
    use super::pe_rotated_tile_census;

    #[test]
    #[ignore = "requires private original-PE military rotation fixture"]
    fn pe_rotation_special_census_matches_mixed_military_zones() {
        fn bytes(hex: &str) -> Vec<u8> {
            hex.as_bytes().chunks_exact(2).map(|pair| {
                u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap()
            }).collect()
        }
        let path = std::env::var_os("CITYGAME_PE_SPECIAL_MILITARY_FIXTURE").unwrap();
        let cases: Vec<serde_json::Value> = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        for case in cases {
            let mut buildings = bytes(case["buildings"].as_str().unwrap());
            let mut bits = bytes(case["bits"].as_str().unwrap());
            let zones = bytes(case["zones"].as_str().unwrap());
            let mut counts = [0_u16; 256];
            let mut military = [0_u16; 16];
            crate::tiles::pe_rotate_special_tiles(&mut buildings, &mut bits, case["clockwise"].as_bool().unwrap(), |x, y, old, new| {
                super::pe_special_rotation_census_update(&mut counts, &mut military, zones[x * 128 + y], old, new);
            });
            assert_eq!(buildings, bytes(case["output"].as_str().unwrap()));
            assert_eq!(bits, bytes(case["output_bits"].as_str().unwrap()));
            assert_eq!(counts.into_iter().flat_map(u16::to_le_bytes).collect::<Vec<_>>(), bytes(case["census"].as_str().unwrap()));
            assert_eq!(military.into_iter().flat_map(u16::to_le_bytes).collect::<Vec<_>>(), bytes(case["military"].as_str().unwrap()));
        }
    }

    #[test]
    fn pe_save_persists_runtime_census_as_signed_words() {
        let path = std::env::temp_dir().join(format!("citygame-pe-census-save-{}.sc2", std::process::id()));
        let mut city = super::City::new_blank(&path, "census").unwrap();
        city.pe_dw_tile_count[0] = 0x4000;
        city.pe_dw_tile_count[29] = 0x7fff;
        city.pe_dw_tile_count[30] = 0x8000;
        city.pe_dw_tile_count[255] = 0xffff;
        city.save_pe_city(&path).unwrap();
        let loaded = super::City::load(&path).unwrap();
        for (slot, expected) in [(0, 0x4000), (29, 0x7fff), (30, 0xffff8000), (255, 0xffffffff)] {
            assert_eq!(loaded.misc_u32_at(0x1f0 + slot * 4), Some(expected), "slot {slot}");
        }
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn pe_rotation_moves_directional_counts_but_not_bridge_count_slots() {
        let mut counts = [0; 256];
        counts[0x1d] = 86;
        counts[0x1e] = 102;
        counts[0x5d] = 7;
        counts[0x60] = 9;
        counts[200] = 13;
        let actual = pe_rotated_tile_census(&counts, true);
        assert_eq!((actual[0x1d], actual[0x1e]), (102, 86));
        assert_eq!((actual[0x5d], actual[0x60], actual[200]), (7, 9, 13));
    }

    #[test]
    #[ignore = "requires private original-PE rotation tables"]
    fn pe_rotation_census_matches_original_tables() {
        let path = std::env::var_os("CITYGAME_PE_CENSUS_ROTATION_FIXTURE").unwrap();
        let tables: serde_json::Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        let counts = std::array::from_fn(|i| i as u16);
        for (name, clockwise) in [("cw", true), ("ccw", false)] {
            let actual = pe_rotated_tile_census(&counts, clockwise);
            let map = tables[name].as_array().unwrap();
            assert_eq!(map.len(), 112);
            for (source, target) in map.iter().enumerate() {
                assert_eq!(actual[target.as_u64().unwrap() as usize], source as u16, "{name} source {source}");
            }
            assert_eq!(&actual[112..], &counts[112..]);
        }
    }

    #[test]
    #[ignore = "requires private original-PE rotation saves"]
    fn pe_rotation_census_matches_live_save_bracket() {
        let root = std::path::PathBuf::from(std::env::var_os("CITYGAME_PE_CAMERA_LIVE").unwrap());
        let original = super::City::load(root.join("mx2.sc2")).unwrap();
        let mut counts: [u16; 256] = std::array::from_fn(|i| original.misc_u32_at(0x1f0 + 4 * i).unwrap() as u16);
        for filename in ["mx1.sc2", "mx180.sc2", "mx270.sc2"] {
            counts = pe_rotated_tile_census(&counts, true);
            let saved = super::City::load(root.join(filename)).unwrap();
            for (i, &count) in counts.iter().enumerate() {
                assert_eq!(count, saved.misc_u32_at(0x1f0 + 4 * i).unwrap() as u16, "{filename} census {i}");
            }
        }
    }

    #[test]
    #[ignore = "requires private original-PE rotation save"]
    fn pe_saved_census_matches_live_original_city() {
        let root = std::path::PathBuf::from(std::env::var_os("CITYGAME_PE_CAMERA_LIVE").unwrap());
        let source = super::City::load(root.join("mx2.sc2")).unwrap();
        let expected: [u32; 256] = std::array::from_fn(|i| source.misc_u32_at(0x1f0 + 4 * i).unwrap());
        let mut city = source;
        city.pe_rebuild_dw_tile_count_434f80();
        let path = std::env::temp_dir().join(format!("citygame-pe-live-census-{}.sc2", std::process::id()));
        city.save_pe_city(&path).unwrap();
        let saved = super::City::load(&path).unwrap();
        for (i, value) in expected.into_iter().enumerate() {
            assert_eq!(saved.misc_u32_at(0x1f0 + 4 * i), Some(value), "census {i}");
        }
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    #[ignore = "requires private original-PE rotation saves"]
    fn pe_rotated_snapshot_matches_live_saved_planes_and_census() {
        let root = std::path::PathBuf::from(std::env::var_os("CITYGAME_PE_CAMERA_LIVE").unwrap());
        let mut snapshot = super::City::load(root.join("mx2.sc2")).unwrap();
        snapshot.pe_rebuild_dw_tile_count_434f80();
        for filename in ["mx1.sc2", "mx180.sc2", "mx270.sc2"] {
            snapshot.pe_rotate_save_snapshot(1);
            let native = super::City::load(root.join(filename)).unwrap();
            assert_eq!(snapshot.altm, native.altm, "{filename} ALTM");
            assert_eq!(snapshot.terrain, native.terrain, "{filename} XTER");
            assert_eq!(snapshot.buildings, native.buildings, "{filename} XBLD");
            assert_eq!(snapshot.underground, native.underground, "{filename} XUND");
            assert_eq!(snapshot.zones, native.zones, "{filename} XZON");
            assert_eq!(snapshot.text_overlays, native.text_overlays, "{filename} XTXT");
            assert_eq!(snapshot.tile_bits, native.tile_bits, "{filename} XBIT");
            assert_eq!(snapshot.traffic_density, native.traffic_density, "{filename} XTRF");
            assert_eq!(snapshot.pollution_density, native.pollution_density, "{filename} XPLT");
            assert_eq!(snapshot.land_value_density, native.land_value_density, "{filename} XVAL");
            assert_eq!(snapshot.crime_density, native.crime_density, "{filename} XCRM");
            assert_eq!(snapshot.police_coverage, native.police_coverage, "{filename} XPLC");
            assert_eq!(snapshot.fire_coverage, native.fire_coverage, "{filename} XFIR");
            assert_eq!(snapshot.population_density, native.population_density, "{filename} XPOP");
            assert_eq!(snapshot.rate_of_growth, native.rate_of_growth, "{filename} XROG");
            assert_eq!(snapshot.traffic_overlays, native.traffic_overlays, "{filename} XTHG");
            for (i, &count) in snapshot.pe_dw_tile_count.iter().enumerate() {
                assert_eq!(count, native.misc_u32_at(0x1f0 + i * 4).unwrap() as u16, "{filename} census {i}");
            }
            assert_eq!(snapshot.saved_rotation, native.saved_rotation, "{filename} rotation");
        }
    }
}

impl City {
    /// Build a physically rotated PE save snapshot without touching the live
    /// city. The original rotates map storage during view rotation; CityGame
    /// keeps it fixed and therefore needs this conversion at save time.
    pub(crate) fn pe_rotate_save_snapshot(&mut self, turns: u8) {
        fn rotate<T: Copy>(plane: &mut Option<Vec<T>>) {
            let Some(data) = plane.as_mut() else { return; };
            if data.len() != 128 * 128 { return; }
            let original = data.clone();
            for x in 0..128 {
                for y in 0..128 { data[(127 - y) * 128 + x] = original[x * 128 + y]; }
            }
        }
        for _ in 0..(turns & 3) {
            rotate(&mut self.altm);
            rotate(&mut self.terrain);
            rotate(&mut self.buildings);
            rotate(&mut self.underground);
            rotate(&mut self.zones);
            rotate(&mut self.text_overlays);
            rotate(&mut self.tile_bits);
            rotate(&mut self.traffic_density);
            rotate(&mut self.pollution_density);
            rotate(&mut self.land_value_density);
            rotate(&mut self.crime_density);
            rotate(&mut self.police_coverage);
            rotate(&mut self.fire_coverage);
            rotate(&mut self.population_density);
            rotate(&mut self.rate_of_growth);
            if let Some(terrain) = self.terrain.as_mut() {
                for value in terrain {
                    *value = if matches!(*value, 0x0e | 0x0f | 0x1e | 0x1f | 0x2f | 0x3f | 0x46 | 0x47) {
                        0
                    } else { crate::tiles::rotated_terrain_id(*value, 1) };
                }
            }
            if let Some(underground) = self.underground.as_mut() {
                for value in underground {
                    *value = if (36..40).contains(value) { 0 } else { crate::tiles::rotated_underground_id(*value, 1) };
                }
            }
            if let Some(buildings) = self.buildings.as_mut() {
                for value in buildings {
                    if !(0x5d..=0x60).contains(value) {
                        *value = crate::tiles::rotated_building_id(*value, 1);
                    }
                }
            }
            self.pe_dw_tile_count = pe_rotated_tile_census(&self.pe_dw_tile_count, true);
            if let (Some(buildings), Some(bits), Some(zones)) =
                (self.buildings.as_mut(), self.tile_bits.as_mut(), self.zones.as_ref())
            {
                let mut raw_bits: Vec<_> = bits.iter().map(|value| value.raw_bits()).collect();
                let counts = &mut self.pe_dw_tile_count;
                let military = &mut self.pe_military_tile_count;
                crate::tiles::pe_rotate_special_tiles(buildings, &mut raw_bits, true, |x, y, old, new| {
                    pe_special_rotation_census_update(counts, military, zones[x * 128 + y].raw_bits(), old, new);
                });
                for (value, raw) in bits.iter_mut().zip(raw_bits) { *value = TileBits::new(raw); }
            }
            if let Some(actors) = self.traffic_overlays.as_mut() {
                crate::win95_overlay::pe_rotate_saved_actors(actors, true);
            }
            self.saved_rotation = self.saved_rotation.wrapping_sub(1) & 3;
            self.pe_w_city_rotation = (self.pe_w_city_rotation & !3) | u16::from(self.saved_rotation);
        }
    }

    /// Adopt an already-written provider snapshot without discarding pending
    /// render dirtiness. The caller keeps the simulation paused through export.
    #[cfg(any(target_os = "ios", test))]
    pub(crate) fn adopt_saved_document(&mut self, path: PathBuf) {
        self.path = path;
        self.has_backing_file = true;
        self.modified = false;
    }

    pub(crate) fn adopt_pe_rotated_save(&mut self, path: PathBuf) {
        self.path = path;
        self.has_backing_file = true;
        self.modified = false;
        self.clear_dirty();
    }

    pub fn save(&mut self, path: impl AsRef<Path>) -> Result<()> {
        self.save_with_pe_scenario_chunks(path, true)
    }

    /// PE's city writer does not serialize the scenario briefing or goals.
    /// Keep the live chunks in memory so the active scenario can continue.
    pub(crate) fn save_pe_city(&mut self, path: impl AsRef<Path>) -> Result<()> {
        self.persist_pe_tile_census_for_save();
        let path = path.as_ref();
        let result = self.save_with_pe_scenario_chunks(path, false);
        if let Err(error) = &result {
            crate::win95_file_error::record_city_io_error(path, error);
        }
        result
    }

    /// `SIMCITY.EXE` 0x433ccc sign-extends each runtime census WORD into the
    /// corresponding saved MISC DWORD. The PE reload subsequently rebuilds
    /// its runtime counts, but other readers can observe the saved values.
    pub(crate) fn persist_pe_tile_census_for_save(&mut self) {
        if !self.kind.is_win95() { return; }
        let values = self.pe_dw_tile_count.map(|value| i32::from(value as i16) as u32);
        self.set_misc_u32_values(0x1f0, &values);
    }

    fn save_with_pe_scenario_chunks(
        &mut self,
        path: impl AsRef<Path>,
        keep_scenario_chunks: bool,
    ) -> Result<()> {
        let path = path.as_ref().to_path_buf();
        self.update_chunks_from_layers();
        self.sync_mac_budget_book_to_misc();
        let filtered;
        let chunks = if !keep_scenario_chunks && self.kind == CityFileKind::WinScenario {
            filtered = self.chunks.iter().filter(|chunk| {
                !matches!(chunk.name.as_str(), "SCEN" | "TEXT" | "PICT")
            }).cloned().collect::<Vec<_>>();
            &filtered
        } else {
            &self.chunks
        };
        let bytes = encode_scdh_file_for_kind(chunks, self.kind)?;
        let mut file = fs::File::create(&path).map_err(super::CityOpenIoError)
            .with_context(|| format!("writing city {}", path.display()))?;
        std::io::Write::write_all(&mut file, &bytes)
            .with_context(|| format!("writing city {}", path.display()))?;
        self.path = path;
        self.has_backing_file = true;
        self.modified = false;
        self.clear_dirty();
        Ok(())
    }

    pub(super) fn update_chunks_from_layers(&mut self) {
        let utf8_labels = self.kind != CityFileKind::MacCity
            && self.labels.as_deref().is_some_and(|labels| {
                labels.iter().any(|label| !super::win1252::can_encode(label))
            });
        let utf8_name = self.kind.is_win95() && !super::win1252::can_encode(&self.name);
        upsert_chunk(
            &mut self.chunks,
            &mut self.chunk_index,
            CITYGAME_MAP_PLANES_CHUNK,
            CITYGAME_MAP_PLANES_VERSION,
        );
        if self.kind != CityFileKind::MacCity {
            upsert_chunk(
                &mut self.chunks,
                &mut self.chunk_index,
                super::layer_codec::CITYGAME_WIN_LABELS_CHUNK,
                if utf8_labels {
                    super::layer_codec::CITYGAME_WIN_LABELS_UTF8_VERSION
                } else {
                    super::layer_codec::CITYGAME_WIN_LABELS_VERSION
                },
            );
            upsert_chunk(
                &mut self.chunks,
                &mut self.chunk_index,
                super::layer_codec::CITYGAME_WIN_NAME_CHUNK,
                if utf8_name {
                    super::layer_codec::CITYGAME_WIN_NAME_UTF8_VERSION
                } else {
                    super::layer_codec::CITYGAME_WIN_NAME_VERSION
                },
            );
        }
        if let Some(data) = &self.terrain {
            upsert_chunk_for_kind(
                &mut self.chunks,
                &mut self.chunk_index,
                "XTER",
                data,
                self.kind,
            );
        }
        if let Some(data) = &self.buildings {
            upsert_chunk_for_kind(
                &mut self.chunks,
                &mut self.chunk_index,
                "XBLD",
                data,
                self.kind,
            );
        }
        if let Some(data) = &self.underground {
            upsert_chunk_for_kind(
                &mut self.chunks,
                &mut self.chunk_index,
                "XUND",
                data,
                self.kind,
            );
        }
        if let Some(data) = &self.zones {
            let raw = data.iter().map(|zone| zone.raw_bits()).collect::<Vec<_>>();
            upsert_chunk_for_kind(
                &mut self.chunks,
                &mut self.chunk_index,
                "XZON",
                &raw,
                self.kind,
            );
        }
        if let Some(data) = &self.text_overlays {
            upsert_chunk_for_kind(
                &mut self.chunks,
                &mut self.chunk_index,
                "XTXT",
                data,
                self.kind,
            );
        }
        if let Some(labels) = &self.labels {
            let raw = encode_labels(labels, self.kind, utf8_labels, self.pe_label_backing.as_deref(), self.mac_label_backing.as_deref());
            upsert_chunk_for_kind(
                &mut self.chunks,
                &mut self.chunk_index,
                "XLAB",
                &raw,
                self.kind,
            );
        }
        if let Some(microsims) = &self.microsims {
            upsert_chunk_for_kind(
                &mut self.chunks,
                &mut self.chunk_index,
                "XMIC",
                &encode_xmic(microsims),
                self.kind,
            );
        }
        if let Some(data) = &self.tile_bits {
            let raw = data.iter().map(|bits| bits.raw_bits()).collect::<Vec<_>>();
            upsert_chunk_for_kind(
                &mut self.chunks,
                &mut self.chunk_index,
                "XBIT",
                &raw,
                self.kind,
            );
        }
        if let Some(data) = &self.traffic_density {
            let raw = encode_xtrf_for_save(self.kind, self.xtrf_storage, data);
            upsert_chunk_for_kind(
                &mut self.chunks,
                &mut self.chunk_index,
                "XTRF",
                &raw,
                self.kind,
            );
        }
        if let Some(data) = &self.land_value_density {
            let raw = encode_microsim64_for_save(self.kind, data);
            upsert_chunk_for_kind(
                &mut self.chunks,
                &mut self.chunk_index,
                "XVAL",
                &raw,
                self.kind,
            );
        }
        if let Some(data) = &self.crime_density {
            let raw = encode_microsim64_for_save(self.kind, data);
            upsert_chunk_for_kind(
                &mut self.chunks,
                &mut self.chunk_index,
                "XCRM",
                &raw,
                self.kind,
            );
        }
        if let Some(data) = &self.pollution_density {
            upsert_chunk_for_kind(
                &mut self.chunks,
                &mut self.chunk_index,
                "XPLT",
                &encode_microsim64_for_save(self.kind, data),
                self.kind,
            );
        }
        if let Some(data) = &self.police_coverage {
            upsert_chunk_for_kind(
                &mut self.chunks,
                &mut self.chunk_index,
                "XPLC",
                &encode_microsim32_for_save(data),
                self.kind,
            );
        }
        if let Some(data) = &self.fire_coverage {
            upsert_chunk_for_kind(
                &mut self.chunks,
                &mut self.chunk_index,
                "XFIR",
                &encode_microsim32_for_save(data),
                self.kind,
            );
            upsert_chunk(
                &mut self.chunks,
                &mut self.chunk_index,
                CITYGAME_FIRE_COVERAGE_CHUNK,
                CITYGAME_FIRE_COVERAGE_VERSION,
            );
        }
        if let Some(data) = &self.population_density {
            upsert_chunk_for_kind(
                &mut self.chunks,
                &mut self.chunk_index,
                "XPOP",
                &encode_microsim32_for_save(data),
                self.kind,
            );
        }
        if let Some(data) = &self.rate_of_growth {
            upsert_chunk_for_kind(
                &mut self.chunks,
                &mut self.chunk_index,
                "XROG",
                &encode_microsim32_for_save(data),
                self.kind,
            );
        }
        if let Some(data) = &self.traffic_overlays {
            let raw = encode_xthg_records(data).expect("XTHG encode");
            upsert_chunk_for_kind(
                &mut self.chunks,
                &mut self.chunk_index,
                "XTHG",
                &raw,
                self.kind,
            );
        }
        if let Some(data) = &self.altm {
            let raw = encode_altm(data);
            upsert_chunk_for_kind(
                &mut self.chunks,
                &mut self.chunk_index,
                "ALTM",
                &raw,
                self.kind,
            );
        }
        self.persist_pe_city_value_to_misc_432ee0();
        self.persist_pe_dw_city_land_value_to_misc_432eee();
        self.persist_pe_i32_city_funds_to_misc_433aaa();
        // Mac `ffffefa4` is the persisted 25-phase dispatcher counter. Its
        // 25/300 date scale matches PE age-in-days, but Mac owns this saved
        // word, so never replace it with a PE cache snapshot. `ffffe532` is
        // Mac's one-based difficulty selector. Keep both native values.
        if self.kind != CityFileKind::MacCity {
            self.persist_pe_dw_city_age_to_misc_433a9c();
            self.persist_pe_w_city_difficulty_to_misc_433ac7();
            self.persist_pe_w_city_mode_to_misc_433a70();
            self.persist_pe_w_city_progression_to_misc_433ad6();
        }
        self.persist_pe_w_year_founded_to_misc_433a8e();
        self.persist_pe_dw_city_bonds_to_misc_433ab8();
        self.persist_mac_bond_rate_sum_to_misc();
        {
            // Merge saved compass bits like `encode_misc`, then mirror BSS `0x4c942c`.
            if self.kind != CityFileKind::MacCity {
                self.pe_w_city_rotation =
                    (self.pe_w_city_rotation & !0x03) | u16::from(self.saved_rotation & 0x03);
                self.persist_pe_w_city_rotation_to_misc_433a7f();
            }
            // Preserve Mac runtime state in Win-layout files and PE's prior
            // residential demand input in Mac-layout files. Both use CGEX
            // when their native MISC offset belongs to the other edition.
            let existing_extension = decode_citygame_extension(&self.chunks);
            let extension_present = self
                .chunks
                .iter()
                .any(|chunk| chunk.name == CITYGAME_EXTENSION_CHUNK);
            let mac_layout = self.kind == CityFileKind::MacCity;
            let mac_saved_state = if mac_layout {
                existing_extension
                    .as_ref()
                    .and_then(|extension| extension.mac_saved_state.as_ref())
                    .cloned()
            } else {
                (self.mac_cross_layout_active
                    || self.mac_scenario_runtime.is_some()
                    || existing_extension
                        .as_ref()
                        .is_some_and(|extension| extension.mac_saved_state.is_some()))
                .then(|| self.mac_cross_layout_saved_state())
            };
            let prior_pe_population = existing_extension
                .as_ref()
                .and_then(|extension| extension.pe_residential_population);
            let prior_pe_trigger = existing_extension
                .as_ref()
                .and_then(|extension| extension.pe_trigger_disaster);
            let prior_pe_neighbor_fame_ranks = existing_extension
                .as_ref()
                .and_then(|extension| extension.pe_neighbor_fame_ranks);
            let prior_pe_current_disaster_type = existing_extension
                .as_ref()
                .and_then(|extension| extension.pe_current_disaster_type);
            let prior_pe_disaster_active = existing_extension
                .as_ref()
                .and_then(|extension| extension.pe_disaster_active);
            let prior_pe_old_arrests = existing_extension
                .as_ref()
                .and_then(|extension| extension.pe_old_arrests);
            let prior_pe_prison_bonus = existing_extension
                .as_ref()
                .and_then(|extension| extension.pe_prison_bonus);
            let prior_pe_sewer_bonus = existing_extension
                .as_ref()
                .and_then(|extension| extension.pe_sewer_bonus);
            let prior_pe_city_has_ocean = existing_extension
                .as_ref()
                .and_then(|extension| extension.pe_city_has_ocean);
            let prior_pe_city_has_river = existing_extension
                .as_ref()
                .and_then(|extension| extension.pe_city_has_river);
            let prior_pe_xund_occupancy = existing_extension
                .as_ref()
                .and_then(|extension| extension.pe_xund_occupancy);
            let prior_pe_city_progression = existing_extension
                .as_ref()
                .and_then(|extension| extension.pe_city_progression);
            let prior_pe_rewards_available = existing_extension
                .as_ref()
                .and_then(|extension| extension.pe_rewards_available);
            let prior_mac_civic_latch = existing_extension
                .as_ref()
                .and_then(|extension| extension.mac_civic_latch);
            let prior_pe_industrial_mix_bonus = existing_extension
                .as_ref()
                .and_then(|extension| extension.pe_industrial_mix_bonus);
            let prior_pe_industrial_mix_pollution_bonus = existing_extension
                .as_ref()
                .and_then(|extension| extension.pe_industrial_mix_pollution_bonus);
            let prior_pe_traffic_count = existing_extension
                .as_ref()
                .and_then(|extension| extension.pe_traffic_count);
            let prior_pe_monster_xthg_index = existing_extension
                .as_ref()
                .and_then(|extension| extension.pe_monster_xthg_index);
            let pe_residential_population = if mac_layout {
                (self.pe_dw_residential_population != 0 || prior_pe_population.is_some())
                    .then_some(self.pe_dw_residential_population)
            } else {
                prior_pe_population
            };
            let pe_trigger_disaster = if mac_layout {
                (self.pe_w_trigger_disaster != 0 || prior_pe_trigger.is_some())
                    .then_some(self.pe_w_trigger_disaster)
            } else {
                prior_pe_trigger
            };
            let pe_neighbor_fame_ranks = if mac_layout && self.pe_cross_layout_active {
                Some(self.pe_w_neighbor_fame_ranks)
            } else {
                prior_pe_neighbor_fame_ranks
            };
            let pe_current_disaster_type = if mac_layout && self.pe_cross_layout_active {
                Some(self.pe_w_current_disaster_type)
            } else {
                prior_pe_current_disaster_type
            };
            let pe_disaster_active = if mac_layout && self.pe_cross_layout_active {
                Some(self.pe_dw_disaster_active)
            } else {
                prior_pe_disaster_active
            };
            let pe_old_arrests = if mac_layout && self.pe_cross_layout_active {
                Some(self.pe_w_old_arrests)
            } else {
                prior_pe_old_arrests
            };
            let pe_prison_bonus = if mac_layout && self.pe_cross_layout_active {
                Some(self.pe_w_prison_bonus)
            } else {
                prior_pe_prison_bonus
            };
            let pe_sewer_bonus = if mac_layout && self.pe_cross_layout_active {
                Some(self.pe_w_sewer_bonus)
            } else {
                prior_pe_sewer_bonus
            };
            let pe_city_has_ocean = if mac_layout && self.pe_cross_layout_active {
                Some(self.pe_dw_city_has_ocean)
            } else {
                prior_pe_city_has_ocean
            };
            let pe_city_has_river = if mac_layout && self.pe_cross_layout_active {
                Some(self.pe_dw_city_has_river)
            } else {
                prior_pe_city_has_river
            };
            let pe_xund_occupancy = if mac_layout && self.pe_cross_layout_active {
                Some(self.pe_xund_occupancy)
            } else {
                prior_pe_xund_occupancy
            };
            let pe_city_progression = if mac_layout && self.pe_cross_layout_active {
                Some(self.pe_w_city_progression)
            } else {
                prior_pe_city_progression
            };
            let pe_rewards_available = if mac_layout && self.pe_cross_layout_active {
                Some(self.pe_dw_rewards_available)
            } else {
                prior_pe_rewards_available
            };
            let mac_civic_latch = if !mac_layout && self.mac_cross_layout_active {
                self.mac_cross_layout_saved_civic_latch
            } else {
                prior_mac_civic_latch
            };
            let pe_industrial_mix_bonus = if mac_layout && self.pe_cross_layout_active {
                Some(self.pe_w_industrial_mix_bonus)
            } else {
                prior_pe_industrial_mix_bonus
            };
            let pe_industrial_mix_pollution_bonus = if mac_layout && self.pe_cross_layout_active {
                Some(self.pe_w_industrial_mix_pollution_bonus)
            } else {
                prior_pe_industrial_mix_pollution_bonus
            };
            let pe_traffic_count = if mac_layout && self.pe_cross_layout_active {
                Some(self.pe_traffic_count)
            } else {
                prior_pe_traffic_count
            };
            let pe_monster_xthg_index = if mac_layout && self.pe_cross_layout_active {
                Some(self.pe_w_monster_xthg_index)
            } else {
                prior_pe_monster_xthg_index
            };
            let should_write_extension = if extension_present {
                existing_extension.is_some()
            } else {
                !mac_layout
                    || pe_residential_population.is_some()
                    || pe_trigger_disaster.is_some()
                    || pe_neighbor_fame_ranks.is_some()
                    || pe_current_disaster_type.is_some()
                    || pe_disaster_active.is_some()
                    || pe_old_arrests.is_some()
                    || pe_prison_bonus.is_some()
                    || pe_sewer_bonus.is_some()
                    || pe_city_has_ocean.is_some()
                    || pe_city_has_river.is_some()
                    || pe_xund_occupancy.is_some()
                    || pe_city_progression.is_some()
                    || pe_rewards_available.is_some()
                    || mac_civic_latch.is_some()
                    || pe_industrial_mix_bonus.is_some()
                    || pe_industrial_mix_pollution_bonus.is_some()
                    || pe_traffic_count.is_some()
                    || pe_monster_xthg_index.is_some()
            };
            if should_write_extension {
                upsert_chunk(
                    &mut self.chunks,
                    &mut self.chunk_index,
                    CITYGAME_EXTENSION_CHUNK,
                    &encode_citygame_extension(
                        if mac_layout {
                            existing_extension
                                .as_ref()
                                .map(|extension| extension.mac_pending_action)
                                .unwrap_or(0)
                        } else {
                            self.mac_pending_action_runtime
                        },
                        mac_saved_state.as_ref(),
                        pe_residential_population,
                        pe_trigger_disaster,
                        pe_neighbor_fame_ranks.as_ref(),
                        pe_current_disaster_type,
                        pe_disaster_active,
                        pe_old_arrests,
                        pe_prison_bonus,
                        pe_sewer_bonus,
                        pe_city_has_ocean,
                        pe_city_has_river,
                        pe_xund_occupancy,
                        pe_city_progression,
                        pe_rewards_available,
                        mac_civic_latch,
                        pe_industrial_mix_bonus,
                        pe_industrial_mix_pollution_bonus,
                        pe_traffic_count,
                        pe_monster_xthg_index,
                    ),
                );
            }
        }
        // These PE mirrors overlap live Mac CODE 4 state slots in MISC:
        // +0x40 is Mac's monthly growth scalar; +0x44/+0x48/+0x4c are also
        // saved Mac runtime slots (the latter two have independent Mac
        // history/event consumers); +0x70 is Mac's pending action; +0x74 is
        // another Mac state slot; and +0x78 is Mac's optional-state selector.
        // Until native equivalence is established, do not serialize PE-derived
        // snapshots over those Mac values.
        if self.kind != CityFileKind::MacCity || self.pe_cross_layout_active {
            // PE 0x434074 mirrors the shared full unemployment DWORD into MISC.
            self.set_misc_u32_at(MISC_CITY_UNEMPLOYMENT_OFFSET, self.pe_unemployment_percent);
        }
        if self.kind != CityFileKind::MacCity {
            self.persist_pe_w_city_garbage_to_misc_433b42();
        }
        // Mac publishes the summaries directly to MISC on either layout.
        // PE saveback applies its own workforce cap only for PE fidelity.
        if (self.kind != CityFileKind::MacCity && !self.mac_cross_layout_active)
            || self.pe_cross_layout_active
        {
            self.persist_pe_eq_workforce_to_misc_433b49();
            self.persist_pe_city_pollution_rate_to_misc_432f5e();
            self.persist_pe_city_crime_rate_to_misc_432f6c();
        }
        if self.kind != CityFileKind::MacCity {
            // These are PE metrics. Mac saves these offsets as opaque state
            // slots until their owners are recovered.
            self.persist_pe_dw_city_fame_to_misc_433b26();
            self.persist_pe_dw_city_advertising_to_misc_433b34();
            self.persist_pe_w_neighbor_fame_ranks_to_misc_433dd8();
        }
        if self.kind != CityFileKind::MacCity {
            self.persist_pe_b_weather_trend_to_misc_433be1();
            self.persist_pe_b_weather_heat_to_misc_433bb6();
            self.persist_pe_b_weather_wind_to_misc_433bc1();
            self.persist_pe_b_weather_humid_to_misc_433bcf();
        }
        if self.kind != CityFileKind::MacCity {
            self.persist_pe_w_trigger_disaster_to_misc_433bf2();
        }
        if self.kind != CityFileKind::MacCity {
            self.persist_pe_dw_residential_population_to_misc_433c11();
        }
        // On Mac-layout files these offsets belong to Mac's demand modifier
        // and pollution-blur divisor; PE continuation is preserved in CGEX.
        if self.kind != CityFileKind::MacCity {
            self.persist_pe_w_industrial_mix_bonus_to_misc_43427a();
            self.persist_pe_w_industrial_mix_pollution_bonus_to_misc_434294();
        }
        if self.kind != CityFileKind::MacCity {
            // The other PE mirrors in this tail overlap distinct Mac state.
            self.persist_pe_dw_arcology_population_to_misc_43421f();
            self.persist_pe_xund_occupancy_to_misc_4340b8();
            self.persist_pe_w_sports_teams_to_misc_434251();
            self.persist_pe_w_old_arrests_to_misc_43429f();
            self.persist_pe_w_prison_bonus_to_misc_4342ba();
            self.persist_pe_w_sewer_bonus_to_misc_434310();
            self.persist_pe_w_monster_xthg_index_to_misc_4342cf();
            self.persist_pe_w_current_disaster_type_to_misc_4342e4();
            self.persist_pe_dw_disaster_active_to_misc_4342f2();
            self.persist_pe_dw_city_has_ocean_to_misc_433ed8();
            self.persist_pe_dw_city_has_river_to_misc_433eec();
        }
        if self.kind != CityFileKind::MacCity {
            self.persist_pe_dw_rewards_available_to_misc_433c1f();
        }
        self.persist_pe_dw_national_population_to_misc_433b78();
        self.persist_pe_dw_national_value_to_misc_433b86();
        self.persist_pe_w_national_tax_to_misc_433b9d();
        self.persist_pe_w_national_trend_to_misc_433ba5();
        // Crime and pollution are shared aggregate roles: each engine rebuilds
        // the total from its corresponding 64x64 output plane. Persist them
        // for the selected Win95 engine even in a Mac container. Traffic at
        // +0x30 remains edition-sensitive and is preserved for Mac files.
        if self.kind != CityFileKind::MacCity || self.pe_cross_layout_active {
            self.persist_pe_dw_city_crime_to_misc_432efc();
            self.persist_pe_dw_city_pollution_to_misc_432f18();
        }
        if self.kind != CityFileKind::MacCity {
            self.persist_pe_traffic_count_to_misc_432ef7();
        }
        // Both layouts carry 60 interleaved EQ/LE dwords at MISC 0x7c.
        // On Mac-layout saves, the PE worker publishes those values when it
        // runs; do not overwrite Mac-engine state with an inactive PE cache.
        if self.kind != CityFileKind::MacCity || self.pe_cross_layout_active {
            self.persist_pe_eq_le_tables_to_misc();
        }
        let misc_chunk = self
            .chunks
            .iter()
            .find(|chunk| chunk.name == "MISC")
            .cloned();
        let misc_rotation = if self.kind == CityFileKind::MacCity {
            decode_rotation_for_kind(&self.chunks, self.kind).unwrap_or(self.saved_rotation)
        } else {
            self.saved_rotation
        };
        upsert_chunk_for_kind(
            &mut self.chunks,
            &mut self.chunk_index,
            "MISC",
            &encode_misc(misc_rotation, misc_chunk.as_ref(), self.kind),
            self.kind,
        );
        let cnam_raw = encode_cnam(
            &self.name,
            self.chunks.iter().find(|chunk| chunk.name == "CNAM"),
            self.kind,
            utf8_name,
        );
        if self.kind.is_win95() {
            // The PE reads CNAM directly, without the fixed-size RLE decoder.
            super::file_format::upsert_encoded_chunk(
                &mut self.chunks,
                &mut self.chunk_index,
                "CNAM",
                cnam_raw,
            );
        } else {
            upsert_chunk(&mut self.chunks, &mut self.chunk_index, "CNAM", &cnam_raw);
        }
    }

    fn persist_mac_bond_rate_sum_to_misc(&mut self) {
        if self.kind != CityFileKind::MacCity {
            return;
        }
        let count = self
            .misc_u32_at(MISC_CITY_BONDS_OFFSET)
            .unwrap_or(0)
            .min(BOND_RATE_TABLE_CAPACITY as u32) as usize;
        let rate_sum = self
            .misc_u32_values(MISC_BOND_RATE_TABLE_OFFSET, count)
            .into_iter()
            .fold(0_i32, |sum, value| {
                sum.wrapping_add(i32::from(value as u16 as i16))
            });
        self.set_mac_state_misc_u32_at(MAC_BOND_RATE_SUM_MISC_OFFSET, rate_sum as u32);
    }
}

/// Write 256 length-prefixed 25-byte rows. Win95 reserves a terminator; Mac
/// permits a full 24-byte label. Both editions retain inactive record bytes;
/// PE keeps a C-string backing and Mac keeps its Pascal-format backing.
fn encode_labels(labels: &[String], kind: CityFileKind, utf8_labels: bool, pe_backing: Option<&[u8]>, mac_backing: Option<&[u8]>) -> Vec<u8> {
    let expected = CITY_LABEL_COUNT * CITY_LABEL_RECORD_LEN;
    let backing = if kind == CityFileKind::MacCity {
        mac_backing
    } else if kind.is_win95() && !utf8_labels {
        pe_backing
    } else {
        None
    };
    let mut raw = backing.filter(|bytes| bytes.len() == expected).map_or_else(|| vec![0; expected], |bytes| bytes.to_vec());
    for (index, label) in labels.iter().take(CITY_LABEL_COUNT).enumerate() {
        let max = if kind == CityFileKind::MacCity {
            CITY_LABEL_RECORD_LEN - 1
        } else {
            CITY_LABEL_TEXT_MAX
        };
        let encoded = if kind == CityFileKind::MacCity {
            super::mac_roman::encode(label, max)
        } else if utf8_labels {
            let text = label.split('\0').next().unwrap_or_default();
            let mut count = text.len().min(max);
            while !text.is_char_boundary(count) {
                count -= 1;
            }
            text.as_bytes()[..count].to_vec()
        } else {
            super::win1252::encode(label, max)
        };
        let count = encoded.len();
        let offset = index * CITY_LABEL_RECORD_LEN;
        raw[offset] = count as u8;
        raw[offset + 1..offset + 1 + count].copy_from_slice(&encoded);
    }
    raw
}

fn encode_xmic(microsims: &[MicrosimRecord]) -> Vec<u8> {
    let mut raw = Vec::with_capacity(MICROSIM_COUNT * MICROSIM_RECORD_LEN);
    for record in microsims.iter().take(MICROSIM_COUNT) {
        raw.extend_from_slice(&record.to_bytes());
    }
    while raw.len() < MICROSIM_COUNT * MICROSIM_RECORD_LEN {
        raw.extend_from_slice(&MicrosimRecord::default().to_bytes());
    }
    raw
}

fn encode_misc(rotation: u8, existing: Option<&CityChunk>, kind: CityFileKind) -> Vec<u8> {
    let mut raw = existing
        .map(|chunk| {
            decode_misc_bytes_for_kind(std::slice::from_ref(chunk), kind)
                .unwrap_or_else(|| chunk.data.clone())
        })
        .unwrap_or_default();
    raw.resize(raw.len().max(12), 0);
    let rot_word = u32::from_be_bytes(raw[8..12].try_into().unwrap());
    let patched = (rot_word & !0x03) | (rotation as u32 & 0x03);
    raw[8..12].copy_from_slice(&patched.to_be_bytes());
    raw
}

fn encode_cnam(
    name: &str,
    existing: Option<&CityChunk>,
    kind: CityFileKind,
    utf8_name: bool,
) -> Vec<u8> {
    if kind.is_win95() {
        let bytes = if utf8_name {
            let text = name.split('\0').next().unwrap_or_default();
            let mut end = text.len().min(31);
            while !text.is_char_boundary(end) {
                end -= 1;
            }
            text.as_bytes()[..end].to_vec()
        } else {
            super::win1252::encode(name, 31)
        };
        let mut raw = vec![0; 32];
        raw[0] = 31;
        raw[1..1 + bytes.len()].copy_from_slice(&bytes);
        return raw;
    }
    if let Some(chunk) = existing
        && decode_cnam_chunk(chunk) == name
    {
        return chunk.data.clone();
    }
    let bytes = name.as_bytes();
    let count = bytes.len().min(255);
    let mut raw = Vec::with_capacity(count + 1);
    raw.push(count as u8);
    raw.extend_from_slice(&bytes[..count]);
    raw
}
