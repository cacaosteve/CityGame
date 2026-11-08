use super::*;

/// Win95 tech-unlock UI defaults @ `0x4e9560` (copied to `0x4e9a10` except index 5).
const PE_TECH_UI_DEFAULTS_4E9560: [u32; 18] = [
    0x1f, 0x3, 0x3, 0x3, 0x7, 0x0, 0x5, 0x5, 0x1, 0x3, 0x3, 0x3, 0xf, 0xf, 0x1f, 0x0, 0x0, 0x0,
];

impl City {
    /// Win95 local fame-base word @ `0x4cb020` (graph/UpdateGraphData; newspaper threshold addend).
    pub fn set_pe_w_local_fame_base(&mut self, fame: u16) {
        self.pe_w_local_fame_base = fame;
    }

    pub fn pe_w_local_fame_base(&self) -> u16 {
        self.pe_w_local_fame_base
    }

    /// PE `UpdateGraphData` @ `0x41354e`–`0x4137c1`: publish month-mod, season, and
    /// local fame-base from city age (without the PE age `inc` at `0x413543`).
    /// `[0x4cb01c]=(age/0x19)%0xc`, `[0x4cb3e8]=((month+1)%0xc)/3`, `[0x4cb020]=age/0x12c`.
    pub fn pe_publish_graph_calendar_words_from_city_age_41354e(&mut self) {
        // PE sign-extends the age DWORD for IDIV and stores the quotient
        // and remainders as WORDs, including their wrap on large saved ages.
        let age = self.pe_dw_city_age as i32;
        let month_mod = (age / 0x19) % 0xc;
        self.pe_w_calendar_month_mod = month_mod as u16;
        self.pe_w_season = (((month_mod + 1) % 0xc) / 3) as u16;
        self.pe_w_local_fame_base = (age / 0x12c) as u16;
    }

    /// PE `UpdateGraphData` @ `0x41358c`: `[0x4cb020] = [0x4cae04] / 0x12c`.
    pub fn pe_publish_local_fame_base_from_city_age_41358c(&mut self) {
        self.pe_publish_graph_calendar_words_from_city_age_41354e();
    }

    pub fn pe_w_calendar_month_mod(&self) -> u16 {
        self.pe_w_calendar_month_mod
    }

    pub fn pe_w_season(&self) -> u16 {
        self.pe_w_season
    }

    /// PE tech-unlock mask walk @ `0x46a0be`: copy defaults `0x4e9560→0x4e9a10`
    /// (skip index 5 / rewards `0x4e9a24`), then for each zero fame-rank slot OR bits into
    /// `[0x4e9e88]` (starts at `7`). Indices `12..=15` increment `[0x4cdb2c]`.
    /// Ordinance dword `[0x4caa40]` gates index 1 (`0x20000`). Arms `6..=11` OR UI
    /// globals. Epilogue: military base type `0x4cc4e4` in `{2,3,4}` → `0x4e9a18 |= 4`.
    pub fn pe_rebuild_tech_unlock_mask_46a0be(&mut self, ordinances: u32) -> u16 {
        // PE prologue @ `0x46a087`: copy defaults except slot 5 (rewards).
        for (index, &default) in PE_TECH_UI_DEFAULTS_4E9560.iter().enumerate() {
            if index == 5 {
                continue;
            }
            self.pe_dw_tech_ui_4e9a10[index] = default;
        }
        let mut mask = 7_u16;
        let mut high_zero_count = 0_u16;
        for index in 0..17 {
            let rank = self
                .pe_w_neighbor_fame_ranks
                .get(index)
                .copied()
                .unwrap_or(0);
            if rank == 0 && index <= 15 {
                match index {
                    0 => mask |= 0x8,
                    1 => {
                        if ordinances & 0x20000 == 0 {
                            mask |= 0x10;
                        }
                    }
                    2 => mask |= 0x40,
                    3 => mask |= 0x20,
                    4 => mask |= 0x80,
                    5 => mask |= 0x100,
                    6 => self.pe_dw_tech_ui_4e9a10[8] |= 2,
                    7 => self.pe_dw_tech_ui_4e9a10[6] |= 0xa,
                    8 => self.pe_dw_tech_ui_4e9a10[6] |= 0x10,
                    9 => self.pe_dw_tech_ui_4e9a10[7] |= 0x1a,
                    10 => self.pe_dw_tech_ui_4e9a10[4] |= 8,
                    11 => self.pe_dw_tech_ui_4e9a10[4] |= 0x10,
                    12..=15 => high_zero_count = high_zero_count.wrapping_add(1),
                    _ => {}
                }
            }
        }
        // PE epilogue @ `0x46a196`: `[0x4cc4e4]` in {2,3,4} → `[0x4e9a18] |= 4`.
        let military_base = self.pe_w_military_base_type.min(u32::from(u8::MAX)) as u8;
        if matches!(military_base, 2 | 3 | 4) {
            self.pe_dw_tech_ui_4e9a10[2] |= 4;
        }
        self.pe_w_tech_unlock_mask = mask;
        self.pe_w_tech_unlock_high_zero_count = high_zero_count;
        mask
    }

    pub fn pe_dw_tech_ui_4e9a18(&self) -> u32 {
        self.pe_dw_tech_ui_4e9a10[2]
    }

    pub fn pe_dw_tech_ui_4e9a20(&self) -> u32 {
        self.pe_dw_tech_ui_4e9a10[4]
    }

    pub fn pe_dw_tech_ui_4e9a28(&self) -> u32 {
        self.pe_dw_tech_ui_4e9a10[6]
    }

    pub fn pe_dw_tech_ui_4e9a2c(&self) -> u32 {
        self.pe_dw_tech_ui_4e9a10[7]
    }

    pub fn pe_dw_tech_ui_4e9a30(&self) -> u32 {
        self.pe_dw_tech_ui_4e9a10[8]
    }

    pub fn pe_w_tech_unlock_mask(&self) -> u16 {
        self.pe_w_tech_unlock_mask
    }

    pub fn pe_w_tech_unlock_high_zero_count(&self) -> u16 {
        self.pe_w_tech_unlock_high_zero_count
    }

    /// Win95 neighbor-fame rank WORD[17] @ `0x4cb430`.
    pub fn set_pe_w_neighbor_fame_rank(&mut self, index: usize, rank: u16) {
        if let Some(slot) = self.pe_w_neighbor_fame_ranks.get_mut(index) {
            *slot = rank;
        }
    }

    pub fn pe_w_neighbor_fame_rank(&self, index: usize) -> Option<u16> {
        self.pe_w_neighbor_fame_ranks.get(index).copied()
    }

    /// PE national pop ladder from fame+year @ `0x484d1e`:
    /// `threshold = [0x4cb020]+[0x4ca5f4]`; pop `0x249f0`/`0xea60`/`0x61a8`/`0x2710`
    /// at cuts `>0x802` / `>0x7d0` / `>0x79e`. Also sets value = pop×3/10.
    pub fn pe_scale_national_population_from_fame_484d1e(&mut self) {
        let threshold = i32::from(self.pe_w_local_fame_base as i16)
            .wrapping_add(i32::from(self.pe_w_year_founded as i16));
        let pop = if threshold > 0x802 {
            0x249f0_u32
        } else if threshold > 0x7d0 {
            0xea60
        } else if threshold > 0x79e {
            0x61a8
        } else {
            0x2710
        };
        self.set_misc_u32_at(MISC_NATIONAL_POPULATION_OFFSET, pop);
        self.set_misc_u32_at(MISC_NATIONAL_VALUE_OFFSET, pop.saturating_mul(3) / 10);
    }

    /// PE FUN_00484cd0 ocean/river prologue @ `0x484cd0`: XBIT water bit `0x04` at
    /// `(0,0)` (`[0x4cab10][0]`) and `(0x7f,0x7f)` (`[0x4cad0c]+0x7f`) → ocean=1/river=0;
    /// else river=1/ocean=0. PE also writes sea-level word `0x4ca818` = 2 (skipped —
    /// sea remains max-ALTM derived).
    pub fn pe_publish_ocean_river_flags_484cd0(&mut self) {
        let sw_water = self
            .tile_bits_at(0, 0)
            .is_some_and(|bits| bits.water_tile());
        let ne_water = self
            .tile_bits_at(0x7f, 0x7f)
            .is_some_and(|bits| bits.water_tile());
        if sw_water && ne_water {
            self.pe_dw_city_has_ocean = 1;
            self.pe_dw_city_has_river = 0;
        } else {
            self.pe_dw_city_has_river = 1;
            self.pe_dw_city_has_ocean = 0;
        }
    }

    /// PE scenario/national join @ `0x484cd0` after ocean flags: fame pop ladder,
    /// tax `3`, trend `difficulty-1`, then fame-rank reseed @ `0x484d9f` (same as `0x42e585`).
    pub fn pe_fun_00484cd0_national_from_fame(&mut self, crt: &mut Win95CrtRand) {
        self.pe_publish_ocean_river_flags_484cd0();
        self.pe_scale_national_population_from_fame_484d1e();
        self.set_misc_u32_at(MISC_NATIONAL_TAX_OFFSET, 3);
        self.set_misc_u32_at(
            MISC_NATIONAL_TREND_OFFSET,
            u32::from(self.pe_w_city_difficulty).saturating_sub(1),
        );
        self.pe_reseed_neighbor_fame_ranks_42e585(crt);
    }

    pub fn pe_dw_city_has_ocean(&self) -> u32 {
        self.pe_dw_city_has_ocean
    }

    pub fn pe_dw_city_has_river(&self) -> u32 {
        self.pe_dw_city_has_river
    }

    pub fn set_pe_dw_city_has_ocean(&mut self, value: u32) {
        self.pe_dw_city_has_ocean = value;
    }

    pub fn set_pe_dw_city_has_river(&mut self, value: u32) {
        self.pe_dw_city_has_river = value;
    }

    /// PE new-city ocean/river defaults @ `0x40e173`: `[0x4c94c0]=0`, `[0x4cb3f8]=1`.
    /// Mirrors into MISC `0x0e44`/`0x0e48` (Edit Map Coast/River).
    pub fn pe_init_ocean_river_new_city_40e173(&mut self) {
        self.pe_dw_city_has_ocean = 0;
        self.pe_dw_city_has_river = 1;
        // These PE snapshots overlap Mac CODE 4's saved coast/river controls.
        // Keep the PE runtime cache current without replacing those Mac bytes.
        if self.kind.is_win95() {
            self.set_misc_u32_at(MISC_CITY_HAS_OCEAN_OFFSET, 0);
            self.set_misc_u32_at(MISC_CITY_HAS_RIVER_OFFSET, 1);
        }
    }

    /// PE clear fame ranks @ `0x412290` (cheat UI `0x40cb75`→thunk `0x401c0d`):
    /// zero WORD[17] @ `0x4cb430`, set progression `[0x4cb010]=6`, tech-unlock rebuild
    /// @ `0x4023ec`→`0x46a080`, OR reward bits `0..=4` into `0x4e9a24` via `0x441250`,
    /// then add `0x7a120` ($500,000) to funds @ `0x4ca444`. Mirrors MISC tech years.
    pub fn pe_clear_neighbor_fame_ranks_and_rebuild_412290(&mut self) {
        for technology in Technology::ALL {
            self.set_technology_discovery_year(technology, 0);
        }
        self.pe_w_city_progression = 6;
        // Mac CODE 4 owns +0x20 as its saved scenario progression index.
        if self.kind.is_win95() {
            self.set_misc_u32_at(MISC_CITY_PROGRESSION_OFFSET, 6);
        }
        let ordinances = self.ordinance_flags_for_saved_layout();
        let _ = self.pe_rebuild_tech_unlock_mask_46a0be(ordinances);
        // PE @ `0x4122b6`..: `0x441250(bit, 1)` for bits `0..=4`.
        self.set_rewards_available_mask(self.rewards_available_mask() | 0x1f);
        // PE funds add `0x7a120` (same cheat body epilogue).
        self.set_city_funds(self.city_funds().saturating_add(0x7a_120));
    }

    /// Alias kept for the mis-mapped `0x411690` name from an earlier leftover.
    pub fn pe_clear_neighbor_fame_ranks_and_rebuild_411690(&mut self) {
        self.pe_clear_neighbor_fame_ranks_and_rebuild_412290();
    }

    /// PE cheat UI sound arm @ `0x40cb0a`: `push 0x19; call 0x40191f` → `0x473a40`.
    /// Records the PE sound id; host audio playback stays outside City.
    pub fn pe_cheat_sound_40cb0a(&mut self) {
        self.pe_w_last_cheat_sound_id = 0x19;
    }

    /// PE cheat UI clear-fame arm @ `0x40cb75`: thiscall into `0x412290`.
    pub fn pe_cheat_clear_fame_40cb75(&mut self) {
        self.pe_clear_neighbor_fame_ranks_and_rebuild_412290();
    }

    /// PE cheat UI funds bump @ `0x40cb4d` (jump-table arm 1): `[0x4ca444] += 0xfa`.
    pub fn pe_cheat_add_funds_40cb4d(&mut self) {
        self.set_city_funds(self.city_funds().saturating_add(0xfa));
    }

    /// Alias for the mis-labeled `0x40bf4d` name (raw-file-offset skew).
    pub fn pe_cheat_add_funds_40bf4d(&mut self) {
        self.pe_cheat_add_funds_40cb4d();
    }

    /// PE cheat UI trigger-disaster @ `0x40cb7f`: `[0x4ca420] = 0xe` (target point
    /// write to `0x4caa58` omitted — center coords are UI-side).
    pub fn pe_cheat_set_trigger_disaster_40cb7f(&mut self) {
        self.pe_w_trigger_disaster = 0xe;
        if self.kind != CityFileKind::MacCity {
            self.set_misc_u32_at(MISC_TRIGGER_DISASTER_OFFSET, 0xe);
        }
    }

    /// Alias for the mis-labeled `0x40bf7f` name (raw-file-offset skew).
    pub fn pe_cheat_set_trigger_disaster_40bf7f(&mut self) {
        self.pe_cheat_set_trigger_disaster_40cb7f();
    }

    /// PE cheat UI trigger-disaster @ `0x40cba9`/`0x40cb28`: `[0x4ca420] = 0xc`.
    pub fn pe_cheat_set_trigger_disaster_40cba9(&mut self) {
        self.pe_w_trigger_disaster = 0xc;
        if self.kind != CityFileKind::MacCity {
            self.set_misc_u32_at(MISC_TRIGGER_DISASTER_OFFSET, 0xc);
        }
    }

    /// Alias for the mis-labeled `0x40bfa9` name (raw-file-offset skew).
    pub fn pe_cheat_set_trigger_disaster_40bfa9(&mut self) {
        self.pe_cheat_set_trigger_disaster_40cba9();
    }

    /// PE cheat military dialog @ `0x4142c0` (caller `0x40cc8e`→`0x403017`): when
    /// MessageBox result is `7` (IDYES), `[0x4cc4e4] = 1`.
    pub fn pe_cheat_military_dialog_4142c0(&mut self, dialog_result: i32) {
        if dialog_result == 7 {
            self.pe_cheat_accept_military_base_4136da();
        }
    }

    /// PE cheat military-base accept @ `0x4136da` / `0x4142da` (dialog returns 7):
    /// `[0x4cc4e4] = 1`.
    pub fn pe_cheat_accept_military_base_4136da(&mut self) {
        self.set_pe_military_base_type(1);
    }

    /// PE cheat jump table @ `0x40cd08` (indices `0..=7` from BSS `0x4e6520`).
    /// Wired arms: `0` sound id `0x19`, `1` funds+`0xfa`, `2` clear-fame, `3`/`4`
    /// trigger `0xe`/`0xc`, `6` military dialog YES. UI-only arms `5`/`7` omitted.
    pub fn pe_cheat_jump_table_40cd08(&mut self, index: u32) {
        match index {
            0 => self.pe_cheat_sound_40cb0a(),
            1 => self.pe_cheat_add_funds_40cb4d(),
            2 => self.pe_cheat_clear_fame_40cb75(),
            3 => self.pe_cheat_set_trigger_disaster_40cb7f(),
            4 => self.pe_cheat_set_trigger_disaster_40cba9(),
            6 => self.pe_cheat_military_dialog_4142c0(7),
            _ => {}
        }
    }

    /// PE new-city fame-rank reseed @ `0x42e585`: for each of 17 slots,
    /// `[0x4cb430+i] = [0x4e99e8+i] + rand()%0x14`, zeroed when below year-founded.
    /// Mirrors into MISC `0x738` tech-year dwords. PE then calls tech-unlock rebuild
    /// @ `0x42e5bf` → `0x4023ec` → `0x46a080`.
    pub fn pe_reseed_neighbor_fame_ranks_42e585(&mut self, crt: &mut Win95CrtRand) {
        let founded = self.pe_w_year_founded;
        for (index, &base_year) in TECHNOLOGY_BASE_DISCOVERY_YEARS.iter().enumerate() {
            let mut rank = base_year.wrapping_add(crt.random_u15() % 0x14);
            if rank < founded {
                rank = 0;
            }
            self.set_technology_discovery_year(Technology::ALL[index], u32::from(rank));
        }
        let ordinances = self.ordinance_flags_for_saved_layout();
        let _ = self.pe_rebuild_tech_unlock_mask_46a0be(ordinances);
    }
}
