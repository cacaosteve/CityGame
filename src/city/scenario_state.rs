use super::*;

/// CODE 4 restores the countdown and effective goals after the active flag.
const MAC_SCENARIO_GOALS_OFFSET: usize = MAC_SCENARIO_ACTIVE_STATE_OFFSET + 4;

/// Mac CODE 4 fields whose native offsets collide with Win95 MISC.
pub(super) const MAC_ACTION_OFFSETS: [usize; 5] = [0x1040, 0x1044, 0x1048, 0x10b0, 0x10b4];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct MacScenarioContinuation {
    pub goals: Option<[u32; 16]>,
    pub action: [u32; 5],
    pub pending_point: Option<(u8, u8)>,
}

impl Default for MacScenarioContinuation {
    fn default() -> Self {
        Self {
            goals: None,
            action: [u32::MAX, 0, 0, 0, 0],
            pending_point: None,
        }
    }
}

pub(crate) enum MacScenarioResult {
    Won,
    TimedOut,
}

impl City {
    /// Apply the CODE 4 resource replacement after the ordinary data-fork load.
    pub(crate) fn initialize_mac_scenario(&mut self, start: crate::mac_scenario::MacScenarioStart) {
        if self.kind.is_win95() {
            self.mac_scenario_runtime = Some(MacScenarioContinuation {
                goals: Some(start.goals),
                pending_point: (start.action != 0).then_some(start.point),
                ..MacScenarioContinuation::default()
            });
            self.set_mac_scenario_active(true);
        } else {
            self.set_misc_u32_values(MAC_SCENARIO_GOALS_OFFSET, &start.goals);
            self.set_mac_state_misc_low_byte_at(MAC_SCENARIO_ACTIVE_STATE_OFFSET, 0xff);
        }
        self.set_mac_pending_action(u32::from(start.action));
    }

    pub(crate) fn mac_start_from_win_scenario(
        &self,
    ) -> Option<crate::mac_scenario::MacScenarioStart> {
        if self.kind != CityFileKind::WinScenario || self.mac_scenario_runtime.is_some() {
            return None;
        }
        let raw = &self.chunks.iter().find(|chunk| chunk.name == "SCEN")?.data;
        crate::mac_scenario::MacScenarioStart::decode_win95(raw).or_else(|| {
            crate::mac_scenario::MacScenarioStart::decode_win95(&sc2_rle_decode(raw).ok()?)
        })
    }

    pub(crate) fn take_mac_scenario_pending_point(&mut self) -> Option<(u8, u8)> {
        if self.kind.is_win95() {
            self.mac_scenario_runtime.as_mut()?.pending_point.take()
        } else {
            None
        }
    }

    fn mac_scenario_goals(&self) -> Option<[u32; 16]> {
        if self.kind.is_win95() {
            self.mac_scenario_runtime?.goals
        } else {
            self.misc_u32_values(MAC_SCENARIO_GOALS_OFFSET, 16)
                .try_into()
                .ok()
        }
    }

    /// CODE 14 phase 22 uses the effective goal vector saved by the Mac
    /// runtime. The fresh resource and the Win95 SCEN chunk have separate layouts.
    pub(crate) fn tick_mac_saved_scenario(&mut self) -> Option<MacScenarioResult> {
        if !self.mac_scenario_active() {
            return None;
        }
        let [
            months,
            population,
            residential,
            commercial,
            industrial,
            cash,
            land_value,
            life,
            education,
            pollution_cap,
            crime_cap,
            traffic_cap,
            first_building,
            second_building,
            first_count,
            second_count,
        ] = self.mac_scenario_goals()?;
        let misc = |offset| self.mac_state_misc_u32_at(offset).unwrap_or(0);
        let budget_current = |row: usize| {
            misc(MAC_MISC_BUDGET_BOOK_OFFSET + row * MISC_BUDGET_ROW_DWORDS * MISC_FIELD_LEN) as i32
        };
        let signed_word = |value: u32| i32::from(value as i16) as u32;
        let within_cap = |cap: u32, offset| cap as i32 <= 0 || misc(offset) <= cap;
        let enough_tiles = |id: u32, count: u32| {
            let id = id as u8;
            id == 0 || self.mac_xbld_tile_count(id) >= count as u16
        };
        let met = misc(MISC_NORMAL_POPULATION_OFFSET) >= population
            && budget_current(0) >= residential as i32
            && budget_current(1) >= commercial as i32
            && budget_current(2) >= industrial as i32
            && self
                .city_funds()
                .wrapping_sub(misc(MISC_CITY_BONDS_OFFSET) as i32)
                >= cash as i32
            && misc(MISC_LAND_VALUE_OFFSET) >= land_value
            && misc(MISC_CITY_LIFE_EXPECTANCY_OFFSET) >= signed_word(life)
            && misc(MISC_CITY_EDUCATION_QUOTIENT_OFFSET) >= signed_word(education)
            && within_cap(pollution_cap, MISC_CITY_POLLUTION_OFFSET)
            && within_cap(crime_cap, MISC_CITY_CRIME_OFFSET)
            && within_cap(traffic_cap, MISC_CITY_TRAFFIC_OFFSET)
            && enough_tiles(first_building, first_count)
            && enough_tiles(second_building, second_count);

        let result = if met {
            MacScenarioResult::Won
        } else {
            let remaining = (months as u16).wrapping_sub(1);
            // The native saver sign-extends the WORD into the MISC DWORD.
            let remaining_dword = i32::from(remaining as i16) as u32;
            if self.kind.is_win95() {
                // The evaluator only runs after reading this goal vector.
                if let Some(goals) = self
                    .mac_scenario_runtime
                    .as_mut()
                    .and_then(|state| state.goals.as_mut())
                {
                    goals[0] = remaining_dword;
                    self.modified = true;
                }
            } else {
                self.set_mac_state_misc_u32_at(MAC_SCENARIO_GOALS_OFFSET, remaining_dword);
            }
            if remaining != 0 {
                return None;
            }
            MacScenarioResult::TimedOut
        };
        self.set_mac_scenario_active(false);
        Some(result)
    }

    /// PE `wScenarioTimeLimitMonths` (`0x4CC91C`), SCEN BE u16 @ `0x08`.
    pub fn scenario_time_limit_months(&self) -> Option<u16> {
        let raw = self.scen_chunk_bytes()?;
        if raw.len() < 0x0a {
            return None;
        }
        Some(u16::from_be_bytes([raw[0x08], raw[0x09]]))
    }

    pub fn set_scenario_time_limit_months(&mut self, months: u16) {
        let mut raw = self.scen_chunk_bytes().unwrap_or_default();
        if raw.len() < 0x0a {
            raw.resize(0x0a, 0);
        }
        raw[0x08..0x0a].copy_from_slice(&months.to_be_bytes());
        // Stock Win95 SCEN chunks are raw records, unlike compressed map
        // planes. Preserve that on disk even when reading an older CityGame
        // save that encoded the record using the generic RLE writer.
        if let Some(index) = self.chunks.iter().position(|chunk| chunk.name == "SCEN") {
            self.chunks[index].raw_len = raw.len();
            self.chunks[index].data = raw;
            self.chunk_index.insert("SCEN".to_owned(), index);
        } else {
            let index = self.chunks.len();
            self.chunks.push(CityChunk {
                name: "SCEN".to_owned(),
                offset: 0,
                raw_len: raw.len(),
                data: raw,
            });
            self.chunk_index.insert("SCEN".to_owned(), index);
        }
        self.modified = true;
    }

    pub fn pe_scenario_deadline_expired(&self) -> bool {
        self.pe_scenario_deadline_expired
    }

    pub fn set_pe_scenario_deadline_expired(&mut self, expired: bool) {
        self.pe_scenario_deadline_expired = expired;
    }

    /// Win95 runtime scenario-active global `0x4CAD44` (not persisted in MISC).
    pub fn pe_scenario_active(&self) -> bool {
        self.pe_scenario_active
    }

    pub fn set_pe_scenario_active(&mut self, active: bool) {
        self.pe_scenario_active = active;
    }

    fn scen_chunk_bytes(&self) -> Option<Vec<u8>> {
        let chunk = self.chunks.iter().find(|chunk| chunk.name == "SCEN")?;
        Some(sc2_rle_decode(&chunk.data).unwrap_or_else(|_| chunk.data.clone()))
    }

    /// Decoded SCEN payload (RLE-aware) for scenario goal parsing.
    pub fn scen_bytes(&self) -> Option<Vec<u8>> {
        self.scen_chunk_bytes()
    }
}
