//! City layer-to-chunk synchronization and disk save path.

use super::*;

impl City {
    /// Adopt an already-written provider snapshot without discarding pending
    /// render dirtiness. The caller keeps the simulation paused through export.
    #[cfg(any(target_os = "ios", test))]
    pub(crate) fn adopt_saved_document(&mut self, path: PathBuf) {
        self.path = path;
        self.modified = false;
    }

    pub fn save(&mut self, path: impl AsRef<Path>) -> Result<()> {
        let path = path.as_ref().to_path_buf();
        self.update_chunks_from_layers();
        self.sync_mac_budget_book_to_misc();
        let bytes = encode_scdh_file(&self.chunks)?;
        fs::write(&path, bytes).with_context(|| format!("writing city {}", path.display()))?;
        self.path = path;
        self.modified = false;
        self.clear_dirty();
        Ok(())
    }

    pub(super) fn update_chunks_from_layers(&mut self) {
        if let Some(data) = &self.terrain {
            upsert_chunk(&mut self.chunks, &mut self.chunk_index, "XTER", data);
        }
        if let Some(data) = &self.buildings {
            upsert_chunk(&mut self.chunks, &mut self.chunk_index, "XBLD", data);
        }
        if let Some(data) = &self.underground {
            upsert_chunk(&mut self.chunks, &mut self.chunk_index, "XUND", data);
        }
        if let Some(data) = &self.zones {
            let raw = data.iter().map(|zone| zone.raw_bits()).collect::<Vec<_>>();
            upsert_chunk(&mut self.chunks, &mut self.chunk_index, "XZON", &raw);
        }
        if let Some(data) = &self.text_overlays {
            upsert_chunk(&mut self.chunks, &mut self.chunk_index, "XTXT", data);
        }
        if let Some(labels) = &self.labels {
            let raw = encode_labels(labels);
            upsert_chunk(&mut self.chunks, &mut self.chunk_index, "XLAB", &raw);
        }
        if let Some(microsims) = &self.microsims {
            upsert_chunk(
                &mut self.chunks,
                &mut self.chunk_index,
                "XMIC",
                &encode_xmic(microsims),
            );
        }
        if let Some(data) = &self.tile_bits {
            let raw = data.iter().map(|bits| bits.raw_bits()).collect::<Vec<_>>();
            upsert_chunk(&mut self.chunks, &mut self.chunk_index, "XBIT", &raw);
        }
        if let Some(data) = &self.traffic_density {
            let raw = encode_xtrf_for_save(self.kind, self.xtrf_storage, data);
            upsert_chunk(&mut self.chunks, &mut self.chunk_index, "XTRF", &raw);
        }
        if let Some(data) = &self.land_value_density {
            let raw = encode_microsim64_for_save(self.kind, data);
            upsert_chunk(&mut self.chunks, &mut self.chunk_index, "XVAL", &raw);
        }
        if let Some(data) = &self.crime_density {
            let raw = encode_microsim64_for_save(self.kind, data);
            upsert_chunk(&mut self.chunks, &mut self.chunk_index, "XCRM", &raw);
        }
        if let Some(data) = &self.pollution_density {
            upsert_chunk(
                &mut self.chunks,
                &mut self.chunk_index,
                "XPLT",
                &encode_microsim64_for_save(self.kind, data),
            );
        }
        if let Some(data) = &self.police_coverage {
            upsert_chunk(
                &mut self.chunks,
                &mut self.chunk_index,
                "XPLC",
                &encode_microsim32_for_save(data),
            );
        }
        if let Some(data) = &self.fire_coverage {
            upsert_chunk(
                &mut self.chunks,
                &mut self.chunk_index,
                "XFIR",
                &encode_microsim32_for_save(data),
            );
        }
        if let Some(data) = &self.population_density {
            upsert_chunk(
                &mut self.chunks,
                &mut self.chunk_index,
                "XPOP",
                &encode_microsim32_for_save(data),
            );
        }
        if let Some(data) = &self.rate_of_growth {
            upsert_chunk(
                &mut self.chunks,
                &mut self.chunk_index,
                "XROG",
                &encode_microsim32_for_save(data),
            );
        }
        if let Some(data) = &self.traffic_overlays {
            let raw = encode_xthg_records(data).expect("XTHG encode");
            upsert_chunk(&mut self.chunks, &mut self.chunk_index, "XTHG", &raw);
        }
        if let Some(data) = &self.altm {
            let raw = encode_altm(data);
            upsert_chunk(&mut self.chunks, &mut self.chunk_index, "ALTM", &raw);
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
        // Both editions publish matching cohort-derived workforce/life/EQ
        // summaries. Write them when PE fidelity is active on a Mac layout,
        // while preserving the inactive PE cache for a Mac-engine save.
        if self.kind != CityFileKind::MacCity || self.pe_cross_layout_active {
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
            decode_rotation(&self.chunks).unwrap_or(self.saved_rotation)
        } else {
            self.saved_rotation
        };
        upsert_chunk(
            &mut self.chunks,
            &mut self.chunk_index,
            "MISC",
            &encode_misc(misc_rotation, misc_chunk.as_ref()),
        );
        let cnam_raw = encode_cnam(
            &self.name,
            self.chunks.iter().find(|chunk| chunk.name == "CNAM"),
        );
        upsert_chunk(&mut self.chunks, &mut self.chunk_index, "CNAM", &cnam_raw);
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

/// Match Win95 `FUN_00430c90`: 256 fixed 25-byte rows, with at most 23 text
/// bytes following each length byte. Unused row padding is normalized to zero.
fn encode_labels(labels: &[String]) -> Vec<u8> {
    let mut raw = vec![0u8; CITY_LABEL_COUNT * CITY_LABEL_RECORD_LEN];
    for (index, label) in labels.iter().take(CITY_LABEL_COUNT).enumerate() {
        let bytes = label.as_bytes();
        let count = bytes.len().min(CITY_LABEL_TEXT_MAX);
        let offset = index * CITY_LABEL_RECORD_LEN;
        raw[offset] = count as u8;
        raw[offset + 1..offset + 1 + count].copy_from_slice(&bytes[..count]);
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

fn encode_misc(rotation: u8, existing: Option<&CityChunk>) -> Vec<u8> {
    let mut raw = existing
        .map(|chunk| sc2_rle_decode(&chunk.data).unwrap_or_else(|_| chunk.data.clone()))
        .unwrap_or_default();
    raw.resize(raw.len().max(12), 0);
    let rot_word = u32::from_be_bytes(raw[8..12].try_into().unwrap());
    let patched = (rot_word & !0x03) | (rotation as u32 & 0x03);
    raw[8..12].copy_from_slice(&patched.to_be_bytes());
    raw
}

fn encode_cnam(name: &str, existing: Option<&CityChunk>) -> Vec<u8> {
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
