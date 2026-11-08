//! PE runtime-cache hydration and saveback mirrors for MISC.

use super::*;

impl City {
    /// Win95 fertile-band averages at `0x469be8` into `0x4cb410` / `0x4cc4b4` / `0x4cb460`.
    pub(super) fn pe_age_cohorts_write_fertile_averages(&mut self, city_population: u32) {
        let (life_avg, eq_avg, workforce) = pe_eq_fertile_averages(
            &self.pe_age_cohorts,
            &self.pe_eq_life_points,
            &self.pe_eq_aux_points,
            city_population,
        );
        self.pe_eq_life_average = life_avg;
        self.pe_eq_education_average = eq_avg;
        self.pe_eq_workforce_percent = workforce;
        self.pe_eq_averages_ready = true;
        // PE poll `0x472510` reads these same globals (`0x4cc4b4` / `0x4cb410`).
        self.pe_city_crime = eq_avg;
        self.pe_city_pollution = life_avg;
    }

    /// Pack RAM tables into MISC `0x7c` as PE `[cohort, aux, life] × 20`.
    pub(super) fn persist_pe_eq_le_tables_to_misc(&mut self) {
        let mut packed = [0_u32; PE_EQ_LE_PACKED_DWORDS];
        for index in 0..PE_EQ_LE_BUCKETS {
            packed[index * 3] = self.pe_age_cohorts[index];
            packed[index * 3 + 1] = self.pe_eq_aux_points[index];
            packed[index * 3 + 2] = self.pe_eq_life_points[index];
        }
        self.set_misc_u32_values(MISC_EQ_LE_TABLES_OFFSET, &packed);
    }

    /// Keep PE saveback's cache aligned when another engine updates the shared
    /// saved cohort lanes directly.
    pub(crate) fn clear_pe_eq_le_table_cache(&mut self) {
        self.pe_age_cohorts = [0; PE_EQ_LE_BUCKETS];
        self.pe_eq_aux_points = [0; PE_EQ_LE_BUCKETS];
        self.pe_eq_life_points = [0; PE_EQ_LE_BUCKETS];
    }

    /// PE save MISC mirror @ `0x432ee0`: `[0x4ca4d0]` → `[MISC+0x24]`.
    pub(super) fn persist_pe_city_value_to_misc_432ee0(&mut self) {
        self.set_misc_u32_at(MISC_CITY_VALUE_OFFSET, self.pe_city_value as u32);
    }

    /// PE save MISC mirror @ `0x433b42`: `[0x4ca5f0]` → `[MISC+0x40]`.
    pub(super) fn persist_pe_w_city_garbage_to_misc_433b42(&mut self) {
        self.set_misc_u32_at(MISC_GARBAGE_OFFSET, self.pe_w_city_garbage);
    }

    /// PE save MISC mirror @ `0x433b26`: `[0x4cad28]` → `[MISC+0x38]`.
    pub(super) fn persist_pe_dw_city_fame_to_misc_433b26(&mut self) {
        self.set_misc_u32_at(MISC_CITY_FAME_OFFSET, self.pe_dw_city_fame);
    }

    /// PE save MISC mirror @ `0x433b34`: `[0x4cb018]` → `[MISC+0x3c]`.
    pub(super) fn persist_pe_dw_city_advertising_to_misc_433b34(&mut self) {
        self.set_misc_u32_at(MISC_CITY_ADVERTISING_OFFSET, self.pe_dw_city_advertising);
    }

    /// PE save MISC mirror @ `0x433be1`: `[0x4cb40c]` → `[MISC+0x6c]`.
    pub(super) fn persist_pe_b_weather_trend_to_misc_433be1(&mut self) {
        self.set_misc_u32_at(
            MISC_WEATHER_TREND_OFFSET,
            u32::from(self.pe_b_weather_trend),
        );
    }

    /// PE save MISC mirror @ `0x433bb6`: `[0x4cade0]` → `[MISC+0x60]`.
    pub(super) fn persist_pe_b_weather_heat_to_misc_433bb6(&mut self) {
        self.set_misc_u32_at(MISC_HEAT_OFFSET, u32::from(self.pe_b_weather_heat));
    }

    /// PE save MISC mirror @ `0x433bc1`: `[0x4cae0c]` → `[MISC+0x64]`.
    pub(super) fn persist_pe_b_weather_wind_to_misc_433bc1(&mut self) {
        self.set_misc_u32_at(MISC_WIND_OFFSET, u32::from(self.pe_b_weather_wind));
    }

    /// PE save MISC mirror @ `0x433bcf`: `[0x4cb3d4]` → `[MISC+0x68]`.
    pub(super) fn persist_pe_b_weather_humid_to_misc_433bcf(&mut self) {
        self.set_misc_u32_at(MISC_HUMIDITY_OFFSET, u32::from(self.pe_b_weather_humid));
    }

    /// PE save MISC mirror @ `0x433bf2`: word `[0x4ca420]` → `[MISC+0x70]`.
    pub(super) fn persist_pe_w_trigger_disaster_to_misc_433bf2(&mut self) {
        self.set_misc_u32_at(
            MISC_TRIGGER_DISASTER_OFFSET,
            u32::from(self.pe_w_trigger_disaster),
        );
    }

    /// PE save writes MISC `+0x74` from `*[0x4cc474]+4` @ `0x433c11`; we mirror BSS
    /// `0x4ca194` (hydrated from that MISC slot @ `0x431530`) so round-trip matches load.
    pub(super) fn persist_pe_dw_residential_population_to_misc_433c11(&mut self) {
        self.set_misc_u32_at(
            MISC_RESIDENTIAL_POPULATION_OFFSET,
            self.pe_dw_residential_population,
        );
    }

    /// PE save MISC mirror @ `0x43421f`: `[0x4c94c4]` → `[MISC+0x1020]`.
    pub(super) fn persist_pe_dw_arcology_population_to_misc_43421f(&mut self) {
        self.set_misc_u32_at(
            MISC_ARCOLOGY_POPULATION_OFFSET,
            self.pe_dw_arcology_population,
        );
    }

    /// PE save MISC mirror @ `0x4340b8`: word `[0x4ca41c]` → `[MISC+0xfe8]`.
    pub(crate) fn persist_pe_xund_occupancy_to_misc_4340b8(&mut self) {
        self.set_misc_u32_at(
            MISC_XUND_OCCUPANCY_OFFSET,
            u32::from(self.pe_xund_occupancy),
        );
    }

    /// PE save MISC mirror @ `0x434251`: word `[0x4cc4e0]` → `[MISC+0x1028]`.
    pub(super) fn persist_pe_w_sports_teams_to_misc_434251(&mut self) {
        self.set_misc_u32_at(
            MISC_SPORTS_TEAMS_OFFSET,
            i32::from(self.pe_w_sports_teams as i16) as u32,
        );
    }

    /// PE save MISC mirror @ `0x433dd8`: WORD[17] `[0x4cb430]` → `[MISC+0x738]` (tech years).
    pub(super) fn persist_pe_w_neighbor_fame_ranks_to_misc_433dd8(&mut self) {
        for (index, technology) in Technology::ALL.into_iter().enumerate() {
            self.set_misc_u32_at(
                technology.misc_offset(),
                u32::from(self.pe_w_neighbor_fame_ranks[index]),
            );
        }
    }

    /// PE load MISC→BSS @ `0x43179f`: WORD[17] `[MISC+0x738]` → `[0x4cb430]`.
    pub(super) fn hydrate_pe_w_neighbor_fame_ranks_from_misc_43179f(&mut self) {
        for (index, technology) in Technology::ALL.into_iter().enumerate() {
            self.pe_w_neighbor_fame_ranks[index] = self
                .misc_u32_at(technology.misc_offset())
                .unwrap_or(0)
                .min(u32::from(u16::MAX)) as u16;
        }
    }

    /// PE save MISC mirror @ `0x43427a`: word `[0x4ca1e8]` → `[MISC+0x1030]`.
    pub(super) fn persist_pe_w_industrial_mix_bonus_to_misc_43427a(&mut self) {
        let value = if self.kind == CityFileKind::MacCity {
            i32::from(self.pe_w_industrial_mix_bonus as i16) as u32
        } else {
            u32::from(self.pe_w_industrial_mix_bonus)
        };
        self.set_misc_u32_at(MISC_INDUSTRIAL_MIX_BONUS_OFFSET, value);
    }

    /// PE save MISC mirror @ `0x434294`: word `[0x4c9428]` → `[MISC+0x1034]`.
    pub(super) fn persist_pe_w_industrial_mix_pollution_bonus_to_misc_434294(&mut self) {
        self.set_misc_u32_at(
            MISC_INDUSTRIAL_MIX_POLLUTION_BONUS_OFFSET,
            self.pe_w_industrial_mix_pollution_bonus as u32,
        );
    }

    /// PE save MISC mirror @ `0x43429f`: word `[0x4ca430]` → `[MISC+0x1038]`.
    pub(super) fn persist_pe_w_old_arrests_to_misc_43429f(&mut self) {
        self.set_misc_u32_at(MISC_OLD_ARRESTS_OFFSET, u32::from(self.pe_w_old_arrests));
    }

    /// PE save MISC mirror @ `0x4342ba`: word `[0x4ca1dc]` → `[MISC+0x103C]`.
    pub(super) fn persist_pe_w_prison_bonus_to_misc_4342ba(&mut self) {
        self.set_misc_u32_at(MISC_PRISON_BONUS_OFFSET, u32::from(self.pe_w_prison_bonus));
    }

    /// PE save MISC mirror @ `0x434310`: word `[0x4cb3dc]` → `[MISC+0x1050]`.
    pub(super) fn persist_pe_w_sewer_bonus_to_misc_434310(&mut self) {
        self.set_misc_u32_at(MISC_SEWER_BONUS_OFFSET, u32::from(self.pe_w_sewer_bonus));
    }

    /// PE save MISC mirror @ `0x4342cf`: word `[0x4ca81c]` → `[MISC+0x1040]`.
    pub(super) fn persist_pe_w_monster_xthg_index_to_misc_4342cf(&mut self) {
        self.set_misc_u32_at(
            MISC_MONSTER_XTHG_INDEX_OFFSET,
            u32::from(self.pe_w_monster_xthg_index),
        );
    }

    /// PE save MISC mirror @ `0x4342e4`: word `[0x4ca93c]` → `[MISC+0x1044]`.
    pub(super) fn persist_pe_w_current_disaster_type_to_misc_4342e4(&mut self) {
        self.set_misc_u32_at(
            MISC_CURRENT_DISASTER_TYPE_OFFSET,
            u32::from(self.pe_w_current_disaster_type),
        );
    }

    /// PE save MISC mirror @ `0x4342f2`: `[0x4c9ee8]` → `[MISC+0x1048]`.
    pub(super) fn persist_pe_dw_disaster_active_to_misc_4342f2(&mut self) {
        self.set_misc_u32_at(MISC_DISASTER_ACTIVE_OFFSET, self.pe_dw_disaster_active);
    }

    /// PE save MISC mirror @ `0x433ed8`: `[0x4c94c0]` → `[MISC+0x0e44]`.
    pub(super) fn persist_pe_dw_city_has_ocean_to_misc_433ed8(&mut self) {
        self.set_misc_u32_at(MISC_CITY_HAS_OCEAN_OFFSET, self.pe_dw_city_has_ocean);
    }

    /// PE save MISC mirror @ `0x433eec`: `[0x4cb3f8]` → `[MISC+0x0e48]`.
    pub(super) fn persist_pe_dw_city_has_river_to_misc_433eec(&mut self) {
        self.set_misc_u32_at(MISC_CITY_HAS_RIVER_OFFSET, self.pe_dw_city_has_river);
    }

    /// PE save MISC mirror @ `0x433c1f`: `[0x4e9a24]` → `[MISC+0x78]`.
    pub(super) fn persist_pe_dw_rewards_available_to_misc_433c1f(&mut self) {
        self.set_misc_u32_at(MISC_REWARDS_AVAILABLE_OFFSET, self.pe_dw_rewards_available);
    }

    /// PE save MISC mirror @ `0x433b78`: `[0x4ca928]` → `[MISC+0x50]`.
    pub(super) fn persist_pe_dw_national_population_to_misc_433b78(&mut self) {
        self.set_misc_u32_at(
            MISC_NATIONAL_POPULATION_OFFSET,
            self.pe_dw_national_population,
        );
    }

    /// PE save MISC mirror @ `0x433b86`: `[0x4cb014]` → `[MISC+0x54]`.
    pub(super) fn persist_pe_dw_national_value_to_misc_433b86(&mut self) {
        self.set_misc_u32_at(MISC_NATIONAL_VALUE_OFFSET, self.pe_dw_national_value);
    }

    /// PE save MISC mirror @ `0x433b9d`: word `[0x4ca938]` → `[MISC+0x58]`.
    pub(super) fn persist_pe_w_national_tax_to_misc_433b9d(&mut self) {
        self.set_misc_u32_at(
            MISC_NATIONAL_TAX_OFFSET,
            misc_word_to_saved_dword(self.pe_w_national_tax, self.kind),
        );
    }

    /// PE save MISC mirror @ `0x433ba5`: word `[0x4ca1bc]` → `[MISC+0x5c]`.
    pub(super) fn persist_pe_w_national_trend_to_misc_433ba5(&mut self) {
        self.set_misc_u32_at(
            MISC_NATIONAL_TREND_OFFSET,
            misc_word_to_saved_dword(self.pe_w_national_trend, self.kind),
        );
    }

    /// PE save MISC mirror @ `0x432eee`: `[0x4ca440]` → `[MISC+0x28]`.
    pub(super) fn persist_pe_dw_city_land_value_to_misc_432eee(&mut self) {
        self.set_misc_u32_at(MISC_LAND_VALUE_OFFSET, self.pe_dw_city_land_value);
    }

    /// PE save MISC mirror @ `0x433aaa`: `[0x4ca444]` → `[MISC+0x14]`.
    pub(super) fn persist_pe_i32_city_funds_to_misc_433aaa(&mut self) {
        self.set_misc_u32_at(MISC_CITY_FUNDS_OFFSET, self.pe_i32_city_funds as u32);
    }

    /// PE save MISC mirror @ `0x433a9c`: `[0x4cae04]` → `[MISC+0x10]`.
    pub(super) fn persist_pe_dw_city_age_to_misc_433a9c(&mut self) {
        self.set_misc_u32_at(MISC_CITY_AGE_OFFSET, self.pe_dw_city_age);
    }

    /// PE save MISC mirror @ `0x433ac7`: word `[0x4cb404]` → `[MISC+0x1c]`.
    pub(super) fn persist_pe_w_city_difficulty_to_misc_433ac7(&mut self) {
        self.set_misc_u32_at(MISC_DIFFICULTY_OFFSET, u32::from(self.pe_w_city_difficulty));
    }

    /// PE save MISC mirror @ `0x433a8e`: word `[0x4ca5f4]` → `[MISC+0x0c]`.
    pub(super) fn persist_pe_w_year_founded_to_misc_433a8e(&mut self) {
        self.set_year_founded_misc(self.pe_w_year_founded);
    }

    pub(super) fn set_year_founded_misc(&mut self, year: u16) {
        let value = misc_year_founded_saved_value(year, self.kind);
        self.set_misc_u32_at(MISC_YEAR_FOUNDED_OFFSET, value);
    }

    /// PE save MISC mirror @ `0x433ab8`: `[0x4cc4e8]` → `[MISC+0x18]`.
    pub(super) fn persist_pe_dw_city_bonds_to_misc_433ab8(&mut self) {
        self.set_misc_u32_at(MISC_CITY_BONDS_OFFSET, self.pe_dw_city_bonds);
    }

    /// PE save MISC mirror @ `0x433a70`: word `[0x4ca42c]` → `[MISC+0x04]`.
    pub(super) fn persist_pe_w_city_mode_to_misc_433a70(&mut self) {
        self.set_misc_u32_at(MISC_CITY_MODE_OFFSET, u32::from(self.pe_w_city_mode));
    }

    /// PE save MISC mirror @ `0x433a7f`: word `[0x4c942c]` → `[MISC+0x08]`.
    pub(super) fn persist_pe_w_city_rotation_to_misc_433a7f(&mut self) {
        self.set_misc_u32_at(
            MISC_CITY_ROTATION_OFFSET,
            u32::from(self.pe_w_city_rotation),
        );
    }

    /// PE save MISC mirror @ `0x433ad6`: word `[0x4cb010]` → `[MISC+0x20]`.
    pub(super) fn persist_pe_w_city_progression_to_misc_433ad6(&mut self) {
        self.set_misc_u32_at(
            MISC_CITY_PROGRESSION_OFFSET,
            u32::from(self.pe_w_city_progression),
        );
    }

    /// PE save MISC mirror @ `0x432f0a`: `[0x4cc6f4]` → `[MISC+0x30]`.
    pub(super) fn persist_pe_traffic_count_to_misc_432ef7(&mut self) {
        self.set_misc_u32_at(MISC_CITY_TRAFFIC_OFFSET, self.pe_traffic_count);
    }

    /// PE save MISC mirror @ `0x432efc`: `[0x4cb454]` → `[MISC+0x2c]`.
    pub(super) fn persist_pe_dw_city_crime_to_misc_432efc(&mut self) {
        self.set_misc_u32_at(MISC_CITY_CRIME_OFFSET, self.pe_dw_city_crime);
    }

    /// PE save MISC mirror @ `0x432f18`: `[0x4cc910]` → `[MISC+0x34]`.
    pub(super) fn persist_pe_dw_city_pollution_to_misc_432f18(&mut self) {
        self.set_misc_u32_at(MISC_CITY_POLLUTION_OFFSET, self.pe_dw_city_pollution);
    }

    /// PE save MISC mirror @ `0x432f5e` / `0x433b5c`: `[0x4cb410]` → `[MISC+0x48]`.
    pub(super) fn persist_pe_city_pollution_rate_to_misc_432f5e(&mut self) {
        let value = if self.pe_eq_life_average != 0 {
            self.pe_eq_life_average
        } else {
            self.pe_city_pollution
        };
        self.set_misc_u32_at(MISC_CITY_LIFE_EXPECTANCY_OFFSET, value);
    }

    /// PE save MISC mirror @ `0x432f6c` / `0x433b6a`: `[0x4cc4b4]` → `[MISC+0x4c]`.
    pub(super) fn persist_pe_city_crime_rate_to_misc_432f6c(&mut self) {
        let value = if self.pe_eq_education_average != 0 {
            self.pe_eq_education_average
        } else {
            self.pe_city_crime
        };
        self.set_misc_u32_at(MISC_CITY_EDUCATION_QUOTIENT_OFFSET, value);
    }

    /// PE save MISC mirror @ `0x433b49`: `[0x4cb460]` → `[MISC+0x44]`.
    pub(super) fn persist_pe_eq_workforce_to_misc_433b49(&mut self) {
        self.set_misc_u32_at(
            MISC_WORK_FORCE_PERCENT_OFFSET,
            self.pe_eq_workforce_percent.min(100),
        );
    }

    /// Restore EQ_LE tables from MISC `0x7c` after `City::load`.
    pub(super) fn hydrate_pe_eq_le_from_misc(&mut self) {
        if self.kind == CityFileKind::MacCity {
            self.pe_age_cohorts.fill(0);
            self.pe_eq_life_points.fill(0);
            self.pe_eq_aux_points.fill(0);
            self.pe_eq_workforce_percent = 0;
            self.pe_eq_life_average = 0;
            self.pe_eq_education_average = 0;
            self.pe_eq_averages_ready = false;
            return;
        }

        self.hydrate_shared_pe_eq_le_from_misc();
    }

    /// PE city-open interpretation of shared saved simulation state.
    /// Only the Win95 engine should call this for a Mac-layout city.
    pub(crate) fn prepare_pe_fidelity_from_saved_city(&mut self) {
        if self.kind == CityFileKind::MacCity {
            if self.pe_cross_layout_active {
                return;
            }
            self.pe_cross_layout_active = true;
            // Both originals publish the full unemployment ratio at MISC
            // +0xfa4. PE city-open restores it on either container, so hydrate
            // the selected PE cache from the shared field before first use.
            self.pe_unemployment_percent =
                self.misc_u32_at(MISC_CITY_UNEMPLOYMENT_OFFSET).unwrap_or(0);
            // PE loads city progression from MISC +0x20, where Mac stores a
            // native progression index. Preserve that first-open loader
            // interpretation, then keep PE continuation separately in CGEX.
            self.pe_w_city_progression = self
                .pe_cross_layout_saved_city_progression
                .unwrap_or_else(|| {
                    self.misc_u32_at(MISC_CITY_PROGRESSION_OFFSET).unwrap_or(0) as u16
                });
            self.pe_cross_layout_saved_city_progression = Some(self.pe_w_city_progression);
            // Win95 reads MISC +0x78 as the civic rewards mask. Mac stores a
            // different command-strip latch there, so keep Win95's view in
            // CGEX after its initial loader-compatible interpretation.
            self.pe_dw_rewards_available = self
                .pe_cross_layout_saved_rewards_available
                .unwrap_or_else(|| self.misc_u32_at(MISC_REWARDS_AVAILABLE_OFFSET).unwrap_or(0));
            self.pe_cross_layout_saved_rewards_available = Some(self.pe_dw_rewards_available);
            // PE reads these low WORDs from MISC on first open, but Mac CODE 4
            // owns the same offsets as its industrial-demand modifier and
            // pollution-blur divisor. Preserve the loader interpretation in
            // PE runtime state, then carry PE continuation in CGEX.
            self.pe_w_industrial_mix_bonus = self
                .pe_cross_layout_saved_industrial_mix_bonus
                .unwrap_or_else(|| {
                    self.misc_u32_at(MISC_INDUSTRIAL_MIX_BONUS_OFFSET)
                        .unwrap_or(0) as u16
                });
            self.pe_w_industrial_mix_pollution_bonus = self
                .pe_cross_layout_saved_industrial_mix_pollution_bonus
                .unwrap_or_else(|| {
                    self.misc_u32_at(MISC_INDUSTRIAL_MIX_POLLUTION_BONUS_OFFSET)
                        .unwrap_or(0) as i16
                });
            self.pe_cross_layout_saved_industrial_mix_bonus = Some(self.pe_w_industrial_mix_bonus);
            self.pe_cross_layout_saved_industrial_mix_pollution_bonus =
                Some(self.pe_w_industrial_mix_pollution_bonus);
            // PE's loader reads MISC +0x30 into `dwCityTrafficCount`, but Mac
            // owns that slot as its XTRF total. Keep PE's first-open raw-slot
            // interpretation, then use the independent CGEX value on reload.
            self.pe_traffic_count = self
                .pe_cross_layout_saved_traffic_count
                .unwrap_or_else(|| self.misc_u32_at(MISC_CITY_TRAFFIC_OFFSET).unwrap_or(0));
            self.pe_cross_layout_saved_traffic_count = Some(self.pe_traffic_count);
            // PE's loader reads MISC +0x1040 into `wMonsterXTHGIndex`. Mac
            // owns the same slot in its saved state vector. Preserve that
            // first-open interpretation, then use PE's separate CGEX value
            // on subsequent Mac-layout saves. Maxis Man construction reads
            // this global again after a prior monster actor may be cleared.
            self.pe_w_monster_xthg_index = self
                .pe_cross_layout_saved_monster_xthg_index
                .unwrap_or_else(|| {
                    self.misc_u32_at(MISC_MONSTER_XTHG_INDEX_OFFSET)
                        .unwrap_or(0xffff)
                        .min(u32::from(u16::MAX)) as u16
                });
            self.pe_cross_layout_saved_monster_xthg_index = Some(self.pe_w_monster_xthg_index);
            // Both editions use the low byte of the same four weather slots.
            // Use the saved bytes directly; update chronology differs, storage
            // ownership does not.
            let weather = [
                self.misc_u32_at(MISC_HEAT_OFFSET).unwrap_or(0) as u8,
                self.misc_u32_at(MISC_WIND_OFFSET).unwrap_or(0) as u8,
                self.misc_u32_at(MISC_HUMIDITY_OFFSET).unwrap_or(0) as u8,
                self.misc_u32_at(MISC_WEATHER_TREND_OFFSET).unwrap_or(0) as u8,
            ];
            [
                self.pe_b_weather_heat,
                self.pe_b_weather_wind,
                self.pe_b_weather_humid,
                self.pe_b_weather_trend,
            ] = weather;
            // Both native loaders restore their arcology resident total from
            // MISC +0x1020. Interpret the saved value as PE runtime input only
            // when the Win95 engine is explicitly prepared for this city.
            self.pe_dw_arcology_population = self.mac_arcology_resident_total();
            self.pe_w_military_base_type = u32::from(
                self.misc_u32_at(MISC_MILITARY_BASE_TYPE_OFFSET)
                    .unwrap_or(0) as u8,
            );
            self.pe_dw_city_population =
                self.misc_u32_at(MISC_NORMAL_POPULATION_OFFSET).unwrap_or(0);
            // These are shared aggregate roles: Mac CODE 4 stores the sums
            // rebuilt from XCRM/XPLT, and PE's native loader restores the
            // same MISC slots into its live globals. Initialize the selected
            // PE runtime from those values before its day-2 rebuild so early
            // consumers (notably annual police and EQ/LE) see saved state.
            self.pe_dw_city_crime = self.misc_u32_at(MISC_CITY_CRIME_OFFSET).unwrap_or(0);
            self.pe_dw_city_pollution = self.misc_u32_at(MISC_CITY_POLLUTION_OFFSET).unwrap_or(0);
            self.hydrate_shared_pe_eq_le_from_misc();
            self.pe_w_sports_teams = self.misc_u32_at(MISC_SPORTS_TEAMS_OFFSET).unwrap_or(0) as u16;
            self.pe_xund_occupancy =
                self.pe_cross_layout_saved_xund_occupancy
                    .unwrap_or_else(|| {
                        self.misc_u32_at(MISC_XUND_OCCUPANCY_OFFSET).unwrap_or(0) as u16
                    });
            self.pe_cross_layout_saved_xund_occupancy = Some(self.pe_xund_occupancy);
            // PE loads these MISC words into its disaster globals. On a Mac
            // container, defer that interpretation until this engine is
            // selected, and prefer CityGame's saved PE copy after a prior
            // cross-layout save. The Mac-owned MISC bytes remain untouched.
            self.pe_w_current_disaster_type = self
                .pe_cross_layout_saved_current_disaster_type
                .unwrap_or_else(|| {
                    self.misc_u32_at(MISC_CURRENT_DISASTER_TYPE_OFFSET)
                        .unwrap_or(0)
                        .min(u32::from(u16::MAX)) as u16
                });
            self.pe_dw_disaster_active = self
                .pe_cross_layout_saved_disaster_active
                .unwrap_or_else(|| self.misc_u32_at(MISC_DISASTER_ACTIVE_OFFSET).unwrap_or(0));
            self.pe_cross_layout_saved_current_disaster_type =
                Some(self.pe_w_current_disaster_type);
            self.pe_cross_layout_saved_disaster_active = Some(self.pe_dw_disaster_active);
            // PE loads the prior annual police inputs from these MISC words.
            // They sit inside Mac's opaque state vector, so retain raw first-open
            // interpretation but prefer CityGame's PE snapshot on later opens.
            self.pe_w_old_arrests = self.pe_cross_layout_saved_old_arrests.unwrap_or_else(|| {
                self.misc_u32_at(MISC_OLD_ARRESTS_OFFSET)
                    .unwrap_or(0)
                    .min(u32::from(u16::MAX)) as u16
            });
            self.pe_w_prison_bonus = self.pe_cross_layout_saved_prison_bonus.unwrap_or_else(|| {
                self.misc_u32_at(MISC_PRISON_BONUS_OFFSET)
                    .unwrap_or(0)
                    .min(u32::from(u16::MAX)) as u16
            });
            self.pe_cross_layout_saved_old_arrests = Some(self.pe_w_old_arrests);
            self.pe_cross_layout_saved_prison_bonus = Some(self.pe_w_prison_bonus);
            // PE's saved sewer flag controls the next density rebuild's blur
            // divisor. Mac uses this byte for its scenario-active state.
            self.pe_w_sewer_bonus = self.pe_cross_layout_saved_sewer_bonus.unwrap_or_else(|| {
                self.misc_u32_at(MISC_SEWER_BONUS_OFFSET)
                    .unwrap_or(0)
                    .min(u32::from(u16::MAX)) as u16
            });
            self.pe_cross_layout_saved_sewer_bonus = Some(self.pe_w_sewer_bonus);
            // Mac CODE 4 saves coast/river option bytes in the same dwords
            // that the PE loader reads as its ocean/river flags. On a first
            // Win95-engine open, preserve the PE loader's raw-slot view; after
            // a cross-layout save, prefer the separately saved PE values.
            self.pe_dw_city_has_ocean = self
                .pe_cross_layout_saved_city_has_ocean
                .unwrap_or_else(|| self.misc_u32_at(MISC_CITY_HAS_OCEAN_OFFSET).unwrap_or(0));
            self.pe_dw_city_has_river = self
                .pe_cross_layout_saved_city_has_river
                .unwrap_or_else(|| self.misc_u32_at(MISC_CITY_HAS_RIVER_OFFSET).unwrap_or(0));
            self.pe_cross_layout_saved_city_has_ocean = Some(self.pe_dw_city_has_ocean);
            self.pe_cross_layout_saved_city_has_river = Some(self.pe_dw_city_has_river);
            if let Some(ranks) = self.pe_cross_layout_saved_neighbor_fame_ranks {
                self.pe_w_neighbor_fame_ranks = ranks;
            } else {
                // The PE city-open loader reads WORD[17] from these MISC
                // offsets. On first PE-fidelity open, preserve that loader
                // interpretation in a runtime cache; the Mac codec owns the
                // bytes and must not be overwritten on save.
                self.hydrate_pe_w_neighbor_fame_ranks_from_misc_43179f();
            }
            self.pe_cross_layout_saved_neighbor_fame_ranks = Some(self.pe_w_neighbor_fame_ranks);
            for index in 0..MILITARY_TILE_COUNT_VALUES {
                self.pe_military_tile_count[index] = self
                    .misc_u32_at(MISC_MILITARY_TILE_COUNTS_OFFSET + index * MISC_FIELD_LEN)
                    .unwrap_or(0) as u16;
            }
        }
    }

    fn hydrate_shared_pe_eq_le_from_misc(&mut self) {
        // PE MISC→BSS @ `0x4314bb`/`0x4314c4`/`0x4314cd`: workforce / LE / EQ dwords
        // always copy even when the EQ_LE pyramid at `0x7c` is empty.
        self.pe_eq_workforce_percent = self
            .misc_u32_at(MISC_WORK_FORCE_PERCENT_OFFSET)
            .unwrap_or(0);
        self.pe_eq_life_average = self
            .misc_u32_at(MISC_CITY_LIFE_EXPECTANCY_OFFSET)
            .unwrap_or(0);
        self.pe_eq_education_average = self
            .misc_u32_at(MISC_CITY_EDUCATION_QUOTIENT_OFFSET)
            .unwrap_or(0);
        let packed = self.misc_u32_values(MISC_EQ_LE_TABLES_OFFSET, PE_EQ_LE_PACKED_DWORDS);
        if packed.len() >= PE_EQ_LE_PACKED_DWORDS {
            for index in 0..PE_EQ_LE_BUCKETS {
                self.pe_age_cohorts[index] = packed[index * 3];
                self.pe_eq_aux_points[index] = packed[index * 3 + 1];
                self.pe_eq_life_points[index] = packed[index * 3 + 2];
            }
        }
        // The PE loader copies all three summary globals from MISC even when
        // they are zero. Presence of this validated Win95 chunk therefore
        // makes those zero values authoritative; do not infer validity from
        // nonzero content. A fresh New City has no MISC chunk yet and stays on
        // the transient service-estimate path until the EQ/LE worker publishes.
        self.pe_eq_averages_ready = self.chunks.iter().any(|chunk| chunk.name == "MISC");
        if self.pe_eq_averages_ready {
            self.pe_city_crime = self.pe_eq_education_average;
            self.pe_city_pollution = self.pe_eq_life_average;
        }
    }

    pub(super) fn persist_pe_eq_misc_from_averages(&mut self) {
        self.persist_pe_eq_le_tables_to_misc();
        if !self.pe_eq_averages_ready {
            return;
        }
        self.set_misc_u32_at(
            MISC_WORK_FORCE_PERCENT_OFFSET,
            self.pe_eq_workforce_percent.min(100),
        );
        self.set_misc_u32_at(MISC_CITY_LIFE_EXPECTANCY_OFFSET, self.pe_eq_life_average);
        self.set_misc_u32_at(
            MISC_CITY_EDUCATION_QUOTIENT_OFFSET,
            self.pe_eq_education_average,
        );
    }

    #[cfg(test)]
    pub fn pe_eq_le_tile_site_counts(&self) -> (u32, u32, u32) {
        (
            pe_occupancy_div9_count(self, 214).max(0) as u32,
            pe_occupancy_div9_count(self, 209).max(0) as u32,
            pe_occupancy_div16_count(self, 217).max(0) as u32,
        )
    }

    /// Test stand-in for a loaded city's MISC `0x7c` pyramid (not a PE yearly step).
    #[cfg(test)]
    pub fn test_seed_pe_age_cohorts_from_pop(&mut self, pop_load: u32) {
        self.seed_pe_age_cohorts_from_pop(pop_load);
    }

    #[cfg(test)]
    pub fn pe_eq_life_points_at(&self, index: usize) -> u32 {
        self.pe_eq_life_points.get(index).copied().unwrap_or(0)
    }

    /// Add people to one EQ_LE bucket using the current ordinance LE (tests).
    #[cfg(test)]
    pub fn test_add_pe_eq_cohort(&mut self, index: usize, pop: u32) {
        if index >= 20 || pop == 0 {
            return;
        }
        let life_per =
            pe_eq_ordinance_life_expectancy(self.ordinance_flags_for_saved_layout()).max(1);
        self.pe_age_cohorts[index] = self.pe_age_cohorts[index].saturating_add(pop);
        self.pe_eq_life_points[index] =
            self.pe_eq_life_points[index].saturating_add(pop * life_per);
        self.pe_eq_aux_points[index] = self.pe_eq_aux_points[index].saturating_add(pop);
    }

    #[cfg(test)]
    pub fn pe_eq_aux_points_at(&self, index: usize) -> u32 {
        self.pe_eq_aux_points.get(index).copied().unwrap_or(0)
    }
}

impl City {
    /// Both editions serialize their military selector as a zero-extended byte.
    pub(crate) fn set_pe_military_base_type(&mut self, base_type: u32) {
        self.pe_w_military_base_type = u32::from(base_type as u8);
        self.set_misc_u32_at(MISC_MILITARY_BASE_TYPE_OFFSET, self.pe_w_military_base_type);
    }
}
