use super::*;

impl City {
    /// Win95 `wMonsterXTHGIndex` @ `0x4ca81c` (`0xffff` when cleared).
    pub fn set_pe_w_monster_xthg_index(&mut self, index: u16) {
        self.pe_w_monster_xthg_index = index;
    }

    pub fn pe_w_monster_xthg_index(&self) -> u16 {
        self.pe_w_monster_xthg_index
    }

    /// Win95 `wCurrentDisasterType` @ `0x4ca93c`.
    pub fn set_pe_w_current_disaster_type(&mut self, disaster_type: u16) {
        self.pe_w_current_disaster_type = disaster_type;
    }

    pub fn pe_w_current_disaster_type(&self) -> u16 {
        self.pe_w_current_disaster_type
    }

    /// OpenSC2K `DisasterDamage.record_damage_class` / PE table `0x4e8848`.
    /// Keeps the highest-weight class damaged during the active disaster.
    pub fn record_pe_disaster_damage_class_at(&mut self, x: usize, y: usize) {
        if self.pe_w_current_disaster_type == 0 && self.pe_dw_disaster_active == 0 {
            return;
        }
        let tile = self.building_at(x, y);
        // OpenSC2K skips tunnel entrances `0x3f..=0x42`.
        if matches!(tile, 0x3f..=0x42) {
            return;
        }
        let zone = self.zone_at(x, y).map(|z| z.raw_bits()).unwrap_or(0);
        let mut damage_class = i16::from(zone & 0x0f);
        if tile >= 0xc6 {
            damage_class = if tile == 0xc6 {
                10
            } else {
                i16::from(tile.saturating_sub(0xbd))
            };
        }
        const WEIGHTS: &[u8] = &[
            0, 1, 1, 1, 1, 1, 1, 1, 4, 8, 5, 5, 5, 5, 5, 5, 5, 5, 5, 3, 4, 4, 4, 2, 2, 3, 2, 4, 3,
            2, 3, 2, 8, 8, 2, 4, 8, 1, 4, 1, 1, 8, 1, 1, 3, 8, 2, 2, 2, 1, 1, 4, 1, 1, 2, 2, 2, 1,
            3, 2, 5, 3, 6, 6, 6, 6, 6,
        ];
        let idx = usize::try_from(damage_class).unwrap_or(usize::MAX);
        if idx >= WEIGHTS.len() {
            return;
        }
        let current = self.pe_disaster_damage_class;
        if current < 0
            || WEIGHTS
                .get(usize::try_from(current).unwrap_or(usize::MAX))
                .copied()
                .unwrap_or(0)
                < WEIGHTS[idx]
        {
            self.pe_disaster_damage_class = damage_class;
        }
    }

    pub fn pe_disaster_damage_class(&self) -> i16 {
        self.pe_disaster_damage_class
    }

    pub fn clear_pe_disaster_damage_class(&mut self) {
        self.pe_disaster_damage_class = -1;
    }

    pub fn take_pe_disaster_damage_class_param(&mut self) -> u8 {
        let param = self.pe_disaster_damage_class.max(0) as u8;
        self.pe_disaster_damage_class = -1;
        param
    }

    /// Win95 `dwDisasterActive` @ `0x4c9ee8`.
    pub fn set_pe_dw_disaster_active(&mut self, active: u32) {
        self.pe_dw_disaster_active = active;
    }

    pub fn pe_dw_disaster_active(&self) -> u32 {
        self.pe_dw_disaster_active
    }
}
