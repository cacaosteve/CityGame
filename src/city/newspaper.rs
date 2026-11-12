use super::*;

const MISC_NEWSPAPER_PAPER_LIST_OFFSET: usize = 0x0e50;

#[derive(Clone, Debug)]
pub(crate) struct PeNewspaperTables {
    pub papers: [u32; 30],
    pub stories: [u32; NEWSPAPER_STORY_LIST_MISC_DWORDS],
}

#[derive(Clone, Debug, Default)]
pub(super) enum PeNewspaperStorage {
    #[default]
    Uninitialized,
    Misc,
    Runtime(Box<PeNewspaperTables>),
}

impl City {
    /// Read the live Win95 newspaper tables without interpreting Mac MISC.
    pub(crate) fn pe_newspaper_tables(&self) -> Option<PeNewspaperTables> {
        if let PeNewspaperStorage::Runtime(tables) = &self.pe_newspaper_storage {
            return Some((**tables).clone());
        }
        if self.kind == CityFileKind::MacCity {
            return None;
        }
        Some(PeNewspaperTables {
            papers: self
                .misc_u32_values(MISC_NEWSPAPER_PAPER_LIST_OFFSET, 30)
                .try_into()
                .ok()?,
            stories: self
                .misc_u32_values(
                    MISC_NEWSPAPER_STORY_LIST_OFFSET,
                    NEWSPAPER_STORY_LIST_MISC_DWORDS,
                )
                .try_into()
                .ok()?,
        })
    }

    /// A complete Win95 MISC table takes precedence over the prior process state.
    pub(crate) fn adopt_saved_pe_newspaper_tables(&mut self) -> bool {
        if self.kind.is_win95() && self.pe_newspaper_tables().is_some() {
            self.pe_newspaper_storage = PeNewspaperStorage::Misc;
            true
        } else {
            false
        }
    }

    pub(crate) fn install_pe_newspaper_tables(&mut self, tables: PeNewspaperTables) {
        if self.kind == CityFileKind::MacCity {
            self.pe_newspaper_storage = PeNewspaperStorage::Runtime(Box::new(tables));
        } else {
            self.set_misc_u32_values(MISC_NEWSPAPER_PAPER_LIST_OFFSET, &tables.papers);
            self.set_misc_u32_values(MISC_NEWSPAPER_STORY_LIST_OFFSET, &tables.stories);
            self.pe_newspaper_storage = PeNewspaperStorage::Misc;
        }
    }

    fn pe_newspaper_story_values(&self) -> Vec<u32> {
        match &self.pe_newspaper_storage {
            PeNewspaperStorage::Runtime(tables) => tables.stories.to_vec(),
            _ if self.kind == CityFileKind::MacCity => Vec::new(),
            _ => self.misc_u32_values(
                MISC_NEWSPAPER_STORY_LIST_OFFSET,
                NEWSPAPER_STORY_LIST_MISC_DWORDS,
            ),
        }
    }

    fn write_pe_newspaper_story_values(&mut self, values: &[u32]) {
        if let PeNewspaperStorage::Runtime(tables) = &mut self.pe_newspaper_storage {
            tables.stories.copy_from_slice(values);
        } else {
            self.set_misc_u32_values(MISC_NEWSPAPER_STORY_LIST_OFFSET, values);
        }
    }

    /// Win95 `0x47b5c0` newspaper story insert into the MISC `0x0EC8` queue.
    /// On a Mac-layout city the Win95 engine uses the process table in memory.
    /// When the inserted id is `3..=5` or `0x24`, marks Extra popup pending (`0x47b697`).
    pub fn queue_pe_newspaper_story(&mut self, story_id: u16, param: u8) {
        if self.kind == CityFileKind::MacCity
            && !matches!(self.pe_newspaper_storage, PeNewspaperStorage::Runtime(_))
        {
            return;
        }
        self.ensure_newspaper_story_list_defaults();
        let priority = pe_newspaper_story_priority(story_id);
        let mut values = self.pe_newspaper_story_values();
        values.resize(NEWSPAPER_STORY_LIST_MISC_DWORDS, 0);

        let mut cx: i32 = 5;
        loop {
            if cx < 0 {
                break;
            }
            let index = cx as usize;
            let existing_priority = values[index * NEWSPAPER_STORY_MISC_FIELDS + 1] as u16;
            if existing_priority > priority {
                cx += 1;
                break;
            }
            let dest = index + 1;
            if dest < NEWSPAPER_STORY_RECORD_COUNT {
                let src = index * NEWSPAPER_STORY_MISC_FIELDS;
                let dst = dest * NEWSPAPER_STORY_MISC_FIELDS;
                for field in 0..NEWSPAPER_STORY_MISC_FIELDS {
                    values[dst + field] = values[src + field];
                }
            }
            cx -= 1;
        }
        let insert_at = cx.max(0) as usize;
        if insert_at >= NEWSPAPER_STORY_RECORD_COUNT {
            return;
        }
        let base = insert_at * NEWSPAPER_STORY_MISC_FIELDS;
        values[base] = u32::from(story_id);
        values[base + 1] = u32::from(priority);
        values[base + 2] = u32::from(param);
        values[base + 3] = 0xff;
        values[base + 4] = 0xff;
        values[base + 5] = 0xff;
        self.write_pe_newspaper_story_values(&values);
        // PE `0x47b681`: after insert, if Extra is on and story is 3..=5 or 0x24, open Extra.
        // Extra-enabled gating stays in the app; insert only records the story trigger.
        if pe_newspaper_story_triggers_extra_popup(story_id) {
            self.pe_newspaper_extra_popup_pending = true;
        }
    }

    /// Win95 `FUN_00479a70`: decay and stable-sort the seven active story rows
    /// before the day-21 worker inserts that month's new stories. The two
    /// tables are DATA resources `0x3EC` (base priorities) and `0x3ED` (decay).
    pub fn age_pe_newspaper_story_queue(&mut self) {
        if self.kind == CityFileKind::MacCity
            && !matches!(self.pe_newspaper_storage, PeNewspaperStorage::Runtime(_))
        {
            return;
        }
        self.ensure_newspaper_story_list_defaults();
        let mut values = self.pe_newspaper_story_values();
        values.resize(NEWSPAPER_STORY_LIST_MISC_DWORDS, 0);

        for slot in 0..7 {
            let record = slot * NEWSPAPER_STORY_MISC_FIELDS;
            let story_id = values[record] as u16 as i16;
            let Some(decay) = usize::try_from(story_id)
                .ok()
                .and_then(|id| PE_NEWSPAPER_STORY_DECAY.get(id))
                .copied()
            else {
                // Older CityGame saves can contain IDs outside the PE DATA table.
                continue;
            };
            let priority = values[record + 1] as u16 as i16;
            values[record + 1] = if i32::from(decay) < i32::from(priority) {
                u32::from((priority - decay as i16) as u16)
            } else {
                0
            };
        }

        // PE's nested WORD comparisons swap complete eight-byte records only
        // when the earlier priority is strictly smaller; ties remain stable.
        for left in 0..6 {
            for right in left + 1..7 {
                let left_record = left * NEWSPAPER_STORY_MISC_FIELDS;
                let right_record = right * NEWSPAPER_STORY_MISC_FIELDS;
                if (values[left_record + 1] as u16 as i16)
                    < (values[right_record + 1] as u16 as i16)
                {
                    for field in 0..NEWSPAPER_STORY_MISC_FIELDS {
                        values.swap(left_record + field, right_record + field);
                    }
                }
            }
        }
        self.write_pe_newspaper_story_values(&values);
    }

    /// Consume the PE Extra auto-open latch set by [`Self::queue_pe_newspaper_story`].
    pub fn take_pe_newspaper_extra_popup_pending(&mut self) -> bool {
        let pending = self.pe_newspaper_extra_popup_pending;
        self.pe_newspaper_extra_popup_pending = false;
        pending
    }

    /// Consume the disaster-end newspaper open latch (`0x45cf10` / OpenSC2K paper 0).
    pub fn take_pe_disaster_end_newspaper_pending(&mut self) -> bool {
        let pending = self.pe_disaster_end_newspaper_pending;
        self.pe_disaster_end_newspaper_pending = false;
        pending
    }

    pub fn set_pe_disaster_end_newspaper_pending(&mut self) {
        self.pe_disaster_end_newspaper_pending = true;
    }

    fn ensure_newspaper_story_list_defaults(&mut self) {
        if !matches!(self.pe_newspaper_storage, PeNewspaperStorage::Uninitialized) {
            return;
        }
        let existing = self.pe_newspaper_story_values();
        if existing.iter().any(|&value| value != 0) {
            return;
        }
        let mut values = [0u32; NEWSPAPER_STORY_LIST_MISC_DWORDS];
        for index in 0..NEWSPAPER_STORY_RECORD_COUNT as u32 {
            let rec = index as usize * NEWSPAPER_STORY_MISC_FIELDS;
            values[rec] = index + 0x0B;
            values[rec + 1] = 0;
            values[rec + 2] = 0;
            values[rec + 3] = 0xFF;
            values[rec + 4] = 0xFF;
            values[rec + 5] = 0xFF;
        }
        self.write_pe_newspaper_story_values(&values);
    }

    /// First queued newspaper story id in MISC `0x0EC8`, if present.
    pub fn pe_newspaper_story_id_at(&self, slot: usize) -> Option<u32> {
        let values = self.pe_newspaper_story_values();
        values.get(slot * NEWSPAPER_STORY_MISC_FIELDS).copied()
    }

    pub fn pe_newspaper_story_param_at(&self, slot: usize) -> Option<u32> {
        let values = self.pe_newspaper_story_values();
        values.get(slot * NEWSPAPER_STORY_MISC_FIELDS + 2).copied()
    }

    pub fn pe_newspaper_story_priority_at(&self, slot: usize) -> Option<u32> {
        let values = self.pe_newspaper_story_values();
        values.get(slot * NEWSPAPER_STORY_MISC_FIELDS + 1).copied()
    }

    pub fn pe_mayor_approval(&self) -> u16 {
        self.pe_mayor_approval
    }

    /// Test/helper: set the latched Approval score.
    pub fn set_pe_mayor_approval(&mut self, approval: u16) {
        self.pe_mayor_approval = approval.min(100);
    }

    pub fn set_pe_unemployment_percent(&mut self, percent: u32) {
        self.pe_unemployment_percent = percent;
    }

    pub fn pe_unemployment_percent(&self) -> u32 {
        self.pe_unemployment_percent
    }

    pub fn pe_city_crime(&self) -> u32 {
        self.pe_city_crime
    }

    pub fn pe_city_pollution(&self) -> u32 {
        self.pe_city_pollution
    }

    pub fn set_pe_traffic_count(&mut self, traffic: u32) {
        self.pe_traffic_count = traffic;
    }

    pub fn pe_traffic_count(&self) -> u32 {
        self.pe_traffic_count
    }

    pub fn set_pe_attitude_tax_bases(&mut self, tax_bases: [u32; 3]) {
        self.pe_attitude_tax_bases = tax_bases;
    }

    /// Inputs for attitude scores 1..3 (`0x4c93b4` / `0x4cc910` / `0x4ca440` / `0x4cb454`).
    pub fn set_pe_attitude_demand_inputs(
        &mut self,
        developed_tiles: u32,
        pollution_total: u32,
        land_value_total: u32,
        crime_total: u32,
    ) {
        self.pe_w_city_developed_tiles = developed_tiles;
        self.pe_dw_city_pollution = pollution_total;
        self.pe_dw_city_land_value = land_value_total;
        self.pe_dw_city_crime = crime_total;
        self.pe_attitude_totals_ready = true;
    }

    /// Crime total last written by [`Self::set_pe_attitude_demand_inputs`] (attitude score 3).
    pub fn pe_attitude_crime_total(&self) -> u32 {
        self.pe_dw_city_crime
    }

    pub fn pe_attitude_score_at(&self, index: usize) -> u32 {
        self.pe_attitude_scores.get(index).copied().unwrap_or(0)
    }

    /// Hydrate the latest four PE graph-series values after city open.
    /// The native pointer table addresses series 4..7, each 52 DWORDs long.
    pub(crate) fn hydrate_pe_attitude_scores_from_xgrp(&mut self) {
        for index in 0..4 {
            let offset = (index + 4) * 52 * 4;
            self.pe_attitude_scores[index] = self
                .chunk_u32_values("XGRP", offset, 1)
                .first()
                .copied()
                .unwrap_or(0);
        }
    }

    /// Recompute estimated attitude scores from cached inputs. When demand
    /// inputs were never set, use funding stand-ins with a zero site count.
    /// The live graph uses the document's transport currents instead.
    pub fn recompute_pe_attitude_scores(
        &mut self,
        school_funding: u32,
        health_funding: u32,
        police_funding: u32,
    ) {
        let (sites, pollution, land_value, crime) = if !self.pe_attitude_totals_ready {
            (
                0,
                school_funding.min(100),
                police_funding.min(100),
                health_funding.min(100),
            )
        } else {
            (
                self.pe_w_city_developed_tiles,
                self.pe_dw_city_pollution,
                self.pe_dw_city_land_value,
                self.pe_dw_city_crime,
            )
        };
        self.pe_attitude_scores = pe_attitude_scores_from_inputs(
            self.pe_traffic_count,
            self.pe_attitude_tax_bases,
            pollution,
            land_value,
            crime,
            sites,
        );
    }

    /// Publish the graph scores using the active document's transport rows.
    pub(crate) fn publish_pe_attitude_scores(&mut self, transport_currents: [u32; 3]) {
        self.pe_attitude_scores = pe_attitude_scores_from_inputs(
            self.pe_traffic_count,
            transport_currents,
            self.pe_dw_city_pollution,
            self.pe_dw_city_land_value,
            self.pe_dw_city_crime,
            self.pe_w_city_developed_tiles,
        );
    }

    /// Win95 opinion poll `0x472510` (thunk `0x402383`): 100 CRT samples; category 7
    /// increments Approval (`0x4E9904`). Requires `pop >= 100` and a non-zero weight sum.
    /// Returns whether the score crossed the high-approval notice threshold.
    pub fn run_pe_opinion_poll_with_rng(
        &mut self,
        residential_tax_rate: u32,
        rng: &mut Win95CrtRand,
    ) -> bool {
        // The poll consumes the last graph-published scores and the current
        // EQ/LE globals; it does not recalculate them from display maps.
        let (weights, total) = pe_opinion_poll_weights(
            self.pe_attitude_scores[0],
            self.pe_attitude_scores[1],
            self.pe_attitude_scores[3],
            self.pe_attitude_scores[2],
            residential_tax_rate,
            self.pe_unemployment_percent,
            self.pe_city_crime,
            self.pe_city_pollution,
        );
        if total == 0 || self.pe_dw_city_population < 100 {
            return false;
        }
        // PE tests the WORD total for zero, then sign-extends it into IDIV's divisor.
        let divisor = i32::from(total as i16);
        let previous = self.pe_mayor_approval;
        self.pe_mayor_approval = sample_opinion_poll(&weights, || {
            // `and eax,0xffff; cdq; idiv edi`; the remainder is compared as a signed WORD.
            (i32::from(rng.random_u16()) % divisor) as i16
        });
        (previous as i16) < 80 && self.pe_mayor_approval >= 80
    }

    /// CODE 4 restores the Extra menu flag from the low byte of its saved dword.
    pub(crate) fn mac_newspaper_extra_enabled(&self) -> bool {
        self.mac_state_misc_u32_at(MISC_NEWSPAPER_EXTRA_OFFSET)
            .is_some_and(|value| value as u8 != 0)
    }

    /// CODE 2 toggles the flag to either byte 0 or 1.
    pub(crate) fn toggle_mac_newspaper_extra(&mut self) {
        let value = u32::from(!self.mac_newspaper_extra_enabled());
        self.set_mac_state_misc_u32_at(MISC_NEWSPAPER_EXTRA_OFFSET, value);
    }

    /// CODE 4 restores the Subscription flag from its saved low byte.
    pub(crate) fn mac_newspaper_subscription_enabled(&self) -> bool {
        self.mac_state_misc_u32_at(MISC_NEWSPAPER_SUBSCRIPTION_OFFSET)
            .is_some_and(|value| value as u8 != 0)
    }

    pub(crate) fn toggle_mac_newspaper_subscription(&mut self) {
        let value = u32::from(!self.mac_newspaper_subscription_enabled());
        self.set_mac_state_misc_u32_at(MISC_NEWSPAPER_SUBSCRIPTION_OFFSET, value);
    }

    /// CODE 7 export 234 admits selectors 3–5 and 0x24 when Extra is enabled.
    pub(crate) fn mac_poll_for_extra(&mut self, random_word: impl FnMut() -> u16) {
        if self.mac_newspaper_extra_enabled() {
            self.mac_poll_for_newspaper(random_word);
        }
    }

    /// Carry the process-owned poll state when replacing a city document.
    pub(crate) fn inherit_mac_newspaper_state(&mut self, previous: &Self) {
        self.mac_mayor_approval = previous.mac_mayor_approval;
        self.mac_newspaper_suppressed = previous.mac_newspaper_suppressed;
    }

    pub(crate) fn set_mac_newspaper_suppressed(&mut self, suppressed: bool) {
        self.mac_newspaper_suppressed = suppressed;
    }

    /// CODE 7 export 233 gates the poll at `0x35d42` on application activity.
    pub(crate) fn mac_poll_for_newspaper(&mut self, mut random_word: impl FnMut() -> u16) {
        if self.mac_newspaper_suppressed || self.mac_state_misc_u32_at(0x102c).unwrap_or(0) < 100 {
            return;
        }
        const VALUES_PER_SERIES: usize = 52;
        let history = self.chunk_u32_values(
            "XGRP",
            4 * VALUES_PER_SERIES * size_of::<u32>(),
            3 * VALUES_PER_SERIES + 1,
        );
        let score = |index| history.get(index * VALUES_PER_SERIES).copied().unwrap_or(0);
        let (weights, total) = pe_opinion_poll_weights(
            score(0),
            score(1),
            score(3),
            score(2),
            self.budget_misc_u32_at(MISC_TAX_RATE_R_OFFSET)
                .unwrap_or(DEFAULT_TAX_RATE),
            self.mac_state_misc_u32_at(0x0fa4).unwrap_or(0),
            self.mac_state_misc_u32_at(MISC_CITY_EDUCATION_QUOTIENT_OFFSET)
                .unwrap_or(0),
            self.mac_state_misc_u32_at(MISC_CITY_LIFE_EXPECTANCY_OFFSET)
                .unwrap_or(0),
        );
        if total != 0 {
            // Mac DIVU.W uses the unsigned total, with signed bucket comparisons.
            self.mac_mayor_approval =
                sample_opinion_poll(&weights, || (random_word() % total) as i16);
        }
    }

    /// Win95 `RecalculateMayorsHouseStats` (`0x414b40` / thunk `0x40247d`).
    ///
    /// Re-runs the opinion poll (`0x402383` / leftover `0x4caa74`), then for every
    /// XMIC with tile `0xF3` copies Approval into `stat2` and, when the Rating
    /// countdown (`stat3`) is still live, decrements it and increments `stat0`.
    /// `QuerySpecificItem` only invokes this when `XTXT == 0x6f`.
    pub fn recalculate_mayors_house_stats(
        &mut self,
        residential_tax_rate: u32,
        rng: &mut Win95CrtRand,
    ) -> bool {
        let crossed = self.run_pe_opinion_poll_with_rng(residential_tax_rate, rng);
        let approval = self.pe_mayor_approval;
        let Some(microsims) = self.microsims.as_mut() else {
            return crossed;
        };
        // PE `edx` starts at 8: skip XMIC[0], walk while `edx < 0x4b0` (150×8).
        for record in microsims.iter_mut().skip(1) {
            if record.tile_id != 243 {
                continue;
            }
            record.stat2 = approval;
            if record.stat3 != 0 {
                record.stat3 = record.stat3.saturating_sub(1);
                // QuerySpecificItem `FUN_00414b40` also uses `inc byte` at `0x414b7e`.
                record.stat0 = record.stat0.wrapping_add(1);
            }
        }
        self.modified = true;
        crossed
    }

    /// PE sports milestone advance @ `0x4837a5` (newspaper stadium-reward join).
    /// Ladder `0 → 1 → 3 → 7 → 0xf → 0x1f`. Returns the new team bit index `0..=4`.
    ///
    /// `None` when the mask is `> 0xf` or off the milestone ladder (JT slot 5 /
    /// PE `0x4837bd` CRT path) — those arms leave `wSportsTeams` unchanged and
    /// pick the microsim index via CRT instead (`pe_newspaper_stadium_sports_reward_4837a5`).
    pub fn pe_advance_w_sports_teams_4837a5(&mut self) -> Option<u16> {
        if u32::from(self.pe_w_sports_teams) > 0xf {
            return None;
        }
        let (next, team_index) = match self.pe_w_sports_teams {
            0 => (1_u16, 0_u16),
            1 => (3, 1),
            3 => (7, 2),
            7 => (0xf, 3),
            0xf => (0x1f, 4),
            _ => return None,
        };
        self.pe_w_sports_teams = next;
        self.persist_pe_sports_teams_if_cross_layout();
        Some(team_index)
    }

    /// PE post-advance microsim write @ `0x483816`: LE word at record+4 = team index;
    /// word at +6 = `0xfb + team_index` (stadium team XLAB). XTXT must be `0x33..0xc8`.
    pub fn pe_write_sports_team_index_to_microsim_483816(
        &mut self,
        xtxt: u8,
        team_index: u16,
    ) -> bool {
        if !(0x33..0xc9).contains(&xtxt) {
            return false;
        }
        let index = usize::from(xtxt.saturating_sub(MIN_SIM_TEXT_ENTRIES));
        // PE swaps the saved XMIC words on load; stat2 is the numeric word.
        if !self.set_microsim_stat2(index, team_index) {
            return false;
        }
        let label = 0xfb_u16.saturating_add(team_index);
        self.set_microsim_stat3(index, label)
    }

    /// PE stadium team-name OR @ `0x47ce74`: `wSportsTeams |= 1 << team_index`.
    /// Used by the team-name dialog after writing the microsim team index.
    pub fn pe_or_w_sports_teams_bit_47ce74(&mut self, team_index: u16) {
        let shift = u32::from(team_index & 31);
        self.pe_w_sports_teams |= 1u16.checked_shl(shift).unwrap_or(0);
        self.persist_pe_sports_teams_if_cross_layout();
    }

    /// PE newspaper stadium-reward join @ `0x4837a5` + microsim write @ `0x483816`.
    /// Milestone ladder advances `wSportsTeams`; off-ladder / `> 0xf` takes PE
    /// CRT `rand() % 5` @ `0x4837bd` (mask unchanged) then still writes microsim.
    pub fn pe_newspaper_stadium_sports_reward_4837a5(
        &mut self,
        xtxt: u8,
        crt: &mut Win95CrtRand,
    ) -> bool {
        let team_index = match self.pe_advance_w_sports_teams_4837a5() {
            Some(index) => index,
            // PE @ `0x4837bd`: `call rand` / `div 5` → `dx`; leave `[0x4cc4e0]`.
            None => crt.random_mod(5),
        };
        self.pe_write_sports_team_index_to_microsim_483816(xtxt, team_index)
    }

    /// PE DestroyBuilding stadium arm @ `0x459fb7`: when hit XTXT is `0x33..0xc8` and
    /// that microsim's `tile_id == 0xd7` (stadium), clear bit `1 << record[4]` from
    /// `wSportsTeams` via wrapping `+= (0xffff << bit)` (x86 `add` of `shl 0xffff, cl`).
    ///
    /// Team bit index is the low byte of the decoded XMIC word. No-op when
    /// XTXT/microsim gates fail.
    pub fn pe_destroy_building_clear_sports_team_bit_459fb7(&mut self, xtxt: u8) {
        if !(0x33..0xc9).contains(&xtxt) {
            return;
        }
        let index = usize::from(xtxt.saturating_sub(MIN_SIM_TEXT_ENTRIES));
        let Some(record) = self.microsim_at(index) else {
            return;
        };
        if record.tile_id != 215 {
            return;
        }
        // The x86 low byte is the low byte of the decoded numeric word.
        let bit = u32::from(record.stat2 as u8 & 31);
        // x86 masks the shift count to five bits; counts 16..31 yield zero.
        let addend = if bit >= 16 {
            0u16
        } else {
            0xffffu16 << (bit as u16)
        };
        self.pe_w_sports_teams = self.pe_w_sports_teams.wrapping_add(addend);
        self.persist_pe_sports_teams_if_cross_layout();
    }
}

/// Win95 power-plant aging bulletin (`FUN_0044e541` → `0x402900` story `0x24`).
pub(super) const PE_NEWSPAPER_STORY_POWER_PLANT_AGING: u16 = 0x24;
/// OpenSC2K annual prison overcrowding / DATA `fundprison` (`NEWS_PRISON`).
pub(super) const PE_NEWSPAPER_STORY_PRISON_OVERCROWDED: u16 = 0x25;
/// OpenSC2K annual school underfunding / DATA `fundeducation` (`NEWS_EDUCATION`).
pub(super) const PE_NEWSPAPER_STORY_FUND_EDUCATION: u16 = 0x26;
pub(super) const PE_NEWSPAPER_STORY_NATIONAL_TREND: u16 = 7;
pub(super) const PE_NEWSPAPER_STORY_NATIONAL_TAX_UP: u16 = 9;
pub(super) const PE_NEWSPAPER_STORY_NATIONAL_TAX_DOWN: u16 = 0xa;
/// Win95 DATA resource `0x3EC` story priorities (big-endian words from `DATA_USA.DAT`).
const PE_NEWSPAPER_STORY_PRIORITIES: [u16; 80] = [
    0, 0, 1000, 1000, 1000, 1000, 360, 200, 200, 200, 200, 200, 200, 200, 200, 200, 200, 200, 200,
    200, 200, 200, 1000, 1000, 1000, 1000, 1000, 1000, 1000, 1000, 1000, 1000, 1000, 1000, 1000,
    1000, 1000, 200, 200, 300, 200, 200, 0, 0, 0, 0, 500, 500, 500, 500, 500, 500, 500, 500, 500,
    500, 500, 500, 500, 500, 500, 200, 200, 200, 200, 200, 200, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0,
];
/// Win95 DATA resource `0x3ED` per-month story-priority decay (big-endian words from `DATA_USA.DAT`).
const PE_NEWSPAPER_STORY_DECAY: [u16; 80] = [
    0, 0, 500, 500, 250, 250, 10, 50, 50, 50, 50, 50, 50, 50, 50, 50, 50, 50, 50, 50, 50, 50, 250,
    250, 250, 250, 250, 250, 250, 250, 250, 250, 250, 250, 250, 250, 100, 50, 50, 50, 50, 50, 0, 0,
    0, 0, 500, 500, 500, 500, 500, 500, 500, 500, 500, 500, 500, 500, 500, 500, 500, 50, 50, 50,
    50, 50, 50, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
];

fn pe_newspaper_story_priority(story_id: u16) -> u16 {
    PE_NEWSPAPER_STORY_PRIORITIES
        .get(usize::from(story_id))
        .copied()
        .unwrap_or(0)
}

/// Win95 `0x47b68b`/`0x47b697`: Extra auto-open for story ids 3..=5 or power-plant aging `0x24`.
pub fn pe_newspaper_story_triggers_extra_popup(story_id: u16) -> bool {
    matches!(story_id, 3..=5 | PE_NEWSPAPER_STORY_POWER_PLANT_AGING)
}

/// Win95 attitude scores 1..3 divisor from word `0x4c93b4` @ `0x413fb6`:
/// `movsx` / MSVC signed `/4` (`cdq; and edx,3; add; sar 2`) then `+ 1`.
/// Non-negative counts (the developed-tile domain) are `count/4 + 1`.
pub fn pe_attitude_education_divisor(education_site_count: u32) -> u32 {
    let sx = (education_site_count.min(u32::from(u16::MAX)) as i16) as i32;
    let mut eax = sx;
    let edx = eax >> 31; // cdq: 0 or -1
    eax = eax.wrapping_add(edx & 3);
    let quot16 = (eax >> 2) as i16;
    (i32::from(quot16).wrapping_add(1)) as u32
}

/// Win95 attitude scores for `0x413f89` (poll weights `*[0x4cc480..0x4cc48c]`).
///
/// Score 0 divides traffic by the wrapping sum of road, highway and bridge
/// currents plus one. Scores 1..3 are full-dword
/// `pollution/div`, `land_value/div`, `crime/div` for `0x4cc910` / `0x4ca440` /
/// `0x4cb454`; the opinion-poll consumer later truncates these to WORDs.
pub fn pe_attitude_scores_from_inputs(
    traffic: u32,
    transport_currents: [u32; 3],
    pollution_total: u32,
    land_value_total: u32,
    crime_total: u32,
    developed_tiles: u32,
) -> [u32; 4] {
    let denom = transport_currents[0]
        .wrapping_add(transport_currents[1])
        .wrapping_add(transport_currents[2])
        .wrapping_add(1);
    let div = pe_attitude_education_divisor(developed_tiles);
    let traffic_score = traffic / denom;
    [
        traffic_score,
        pollution_total / div,
        land_value_total / div,
        crime_total / div,
    ]
}

/// Win95 `0x472510` opinion-poll weight vector and sample modulus.
///
/// The poll weights traffic, pollution, and crime scores. Land value
/// contributes to the total without occupying a response bucket.
pub fn pe_opinion_poll_weights(
    attitude0: u32,
    attitude1: u32,
    crime_score: u32,
    land_value_score: u32,
    residential_tax_rate: u32,
    unemployment: u32,
    city_crime: u32,
    city_pollution: u32,
) -> ([u16; 7], u16) {
    let crime_term = if city_crime > 100 {
        0
    } else {
        100u32.saturating_sub(city_crime)
    };
    let pollution_term = if city_pollution > 70 {
        0
    } else {
        70u32.saturating_sub(city_pollution)
    };
    // FUN_00472510 copies dword scores to local WORD slots and multiplies the low
    // residential tax WORD by three with 16-bit wrap. The total is also a WORD.
    let weights: [u16; 7] = [
        attitude0 as u16,
        attitude1 as u16,
        crime_score as u16,
        (residential_tax_rate as u16).wrapping_mul(3),
        unemployment as u16,
        crime_term as u16,
        pollution_term as u16,
    ];
    let mut total = (land_value_score as u16).wrapping_add(0x32);
    for &weight in &weights {
        total = total.wrapping_add(weight);
    }
    (weights, total)
}

fn sample_opinion_poll(weights: &[u16; 7], mut next_remainder: impl FnMut() -> i16) -> u16 {
    let mut approval = 0;
    'sample: for _ in 0..100 {
        let mut remainder = next_remainder();
        for &weight in weights {
            if remainder < weight as i16 {
                continue 'sample;
            }
            remainder = remainder.wrapping_sub(weight as i16);
        }
        approval += 1;
    }
    approval
}

impl City {
    /// Win95 `wSportsTeams` @ `0x4cc4e0` (bitmask, not stadium count).
    pub fn set_pe_w_sports_teams(&mut self, teams: u16) {
        self.pe_w_sports_teams = teams;
        self.persist_pe_sports_teams_if_cross_layout();
    }

    fn persist_pe_sports_teams_if_cross_layout(&mut self) {
        if self.pe_cross_layout_active {
            self.persist_pe_w_sports_teams_to_misc_434251();
        }
    }

    pub fn pe_w_sports_teams(&self) -> u16 {
        self.pe_w_sports_teams
    }
}
