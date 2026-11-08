//! PE XTHG disaster actor allocation, lookup, and removal.

use super::*;

impl City {
    /// PE `FUN_00453280` / thunk `0x402C39`: allocate XTHG kind `0x06` (explosion / transition).
    ///
    /// Requires the tile's `0x4CA600` occupancy (XTXT) `< 0xC9`, writes sub-tiles `(8,8)`,
    /// saves the prior XTXT byte in `+0xA`, and paints XTXT `slot-0x37`. Does not touch XTRF.
    /// Call sites include Maxis Man (`0x45526b`), train-handler merge (`0x4557bb`), and fire
    /// burn-down (`0x45fa46`). Things JT kind-6 targets `0x4546f0` (age `+1` then clear).
    pub fn place_pe_explosion_actor(
        &mut self,
        x: usize,
        y: usize,
        height: u8,
        state: u8,
        saved_b: u8,
    ) -> Option<usize> {
        if x >= CITY_WIDTH || y >= CITY_HEIGHT {
            return None;
        }
        let prior_xtxt = self.text_overlay_at(x, y).unwrap_or(0);
        if prior_xtxt >= XTHG_EXPLOSION_XTXT_GATE {
            return None;
        }
        if self.traffic_overlays.is_none() {
            self.traffic_overlays = Some(empty_overlay_table());
        }
        let slot = pe_overlay_spawn_slot(self.traffic_overlays.as_ref()?)?;
        self.traffic_overlays.as_mut().unwrap()[slot] = Win95OverlayRecord {
            kind: XTHG_EXPLOSION,
            lane: 0,
            state,
            tile_x: x as u8,
            tile_y: y as u8,
            height,
            sub_x: XTHG_EXPLOSION_SUBTILE,
            sub_y: XTHG_EXPLOSION_SUBTILE,
            target_x: 0,
            target_y: 0,
            saved_a: prior_xtxt,
            saved_b,
        };
        if let Some(proxy) = xthg_proxy_for_slot(slot) {
            self.set_text_overlay(x, y, proxy);
        }
        self.mark_dirty(x, y);
        Some(slot)
    }

    pub fn pe_explosion_actor_at(&self, x: usize, y: usize) -> Option<(usize, Win95OverlayRecord)> {
        self.traffic_overlays.as_ref().and_then(|table| {
            table
                .iter()
                .enumerate()
                .skip(XTHG_FIRST_USABLE_SLOT)
                .find_map(|(slot, record)| {
                    (record.is_pe_explosion_actor()
                        && record.tile_x as usize == x
                        && record.tile_y as usize == y)
                        .then_some((slot, *record))
                })
        })
    }

    /// Win95 air-crash start `0x45e7d0`: allocate an XTHG airplane (`state=7`, height `0x10`)
    /// and paint XTXT `slot-0x37` (`xthg_proxy_for_slot`). No `0xff` footprint.
    ///
    /// PE probe @ `0x45e7fb` only accepts tiles with XTXT exactly `0`, and
    /// `0x45e88e` forces XTHG `+A = 0` (does not save a prior occupancy byte).
    pub fn place_pe_aircrash_actor(&mut self, x: usize, y: usize) -> Option<usize> {
        if x >= CITY_WIDTH || y >= CITY_HEIGHT {
            return None;
        }
        if self.text_overlay_at(x, y).unwrap_or(0) != 0 {
            return None;
        }
        if self.traffic_overlays.is_none() {
            self.traffic_overlays = Some(empty_overlay_table());
        }
        let slot = self
            .traffic_overlays
            .as_ref()
            .unwrap()
            .iter()
            .enumerate()
            .skip(XTHG_FIRST_USABLE_SLOT)
            .find_map(|(slot, record)| (record.kind == 0).then_some(slot))?;
        self.traffic_overlays.as_mut().unwrap()[slot] = Win95OverlayRecord {
            kind: XTHG_AIRPLANE,
            lane: 0,
            state: XTHG_AIRCRASH_STATE,
            tile_x: x as u8,
            tile_y: y as u8,
            height: XTHG_AIRCRASH_HEIGHT,
            sub_x: XTHG_AIRCRASH_SUBTILE,
            sub_y: XTHG_AIRCRASH_SUBTILE,
            target_x: 0,
            target_y: 0,
            saved_a: 0,
            saved_b: 0,
        };
        if let Some(proxy) = xthg_proxy_for_slot(slot) {
            self.set_text_overlay(x, y, proxy);
        }
        self.mark_dirty(x, y);
        Some(slot)
    }

    pub fn pe_aircrash_actor(&self) -> Option<(usize, Win95OverlayRecord)> {
        self.traffic_overlays.as_ref().and_then(|table| {
            table
                .iter()
                .enumerate()
                .skip(XTHG_FIRST_USABLE_SLOT)
                .find_map(|(slot, record)| record.is_pe_aircrash_actor().then_some((slot, *record)))
        })
    }

    /// PE tornado/monster start: if tile XTXT is in the actor proxy band
    /// `0xC9..0xF1`, call `FUN_00401528(xtxt-0xC9)` → clear that slot's kind and
    /// zero XTXT before re-reading `+A` / painting the new `slot-0x37`.
    pub(super) fn pe_clear_xtxt_proxy_band_occupant(&mut self, x: usize, y: usize) {
        let xtxt = self.text_overlay_at(x, y).unwrap_or(0);
        if !(0xc9..0xf1).contains(&xtxt) {
            return;
        }
        if let Some(slot) = xthg_slot_for_proxy(xtxt)
            && let Some(table) = self.traffic_overlays.as_mut()
            && let Some(record) = table.get_mut(slot)
        {
            // `FUN_004507D0`: clear kind only.
            record.kind = 0;
        }
        self.set_text_overlay(x, y, 0);
        self.mark_dirty(x, y);
    }

    /// Win95 monster start `0x45ee10`: XTHG kind 5, dir east, height `0x0f`, sub 8/8.
    /// Paints XTXT `slot-0x37`; no `0xff` footprint.
    pub fn place_pe_monster_actor(
        &mut self,
        x: usize,
        y: usize,
        target_x: u8,
        target_y: u8,
        _saved_a: u8,
        saved_b: u8,
    ) -> Option<usize> {
        if x >= CITY_WIDTH || y >= CITY_HEIGHT || self.pe_monster_actor().is_some() {
            return None;
        }
        if self.traffic_overlays.is_none() {
            self.traffic_overlays = Some(empty_overlay_table());
        }
        // PE `0x45eea2`–`0x45eeb8`: clear proxy-band occupant, then re-read XTXT into `+A`.
        self.pe_clear_xtxt_proxy_band_occupant(x, y);
        let saved_a = self.text_overlay_at(x, y).unwrap_or(0);
        let slot = self
            .traffic_overlays
            .as_ref()
            .unwrap()
            .iter()
            .enumerate()
            .skip(XTHG_FIRST_USABLE_SLOT)
            .find_map(|(slot, record)| (record.kind == 0).then_some(slot))?;
        self.traffic_overlays.as_mut().unwrap()[slot] = Win95OverlayRecord {
            kind: XTHG_MONSTER,
            lane: XTHG_DIRECTION_EAST,
            state: 0,
            tile_x: x as u8,
            tile_y: y as u8,
            height: XTHG_MONSTER_HEIGHT,
            sub_x: XTHG_MONSTER_SUBTILE,
            sub_y: XTHG_MONSTER_SUBTILE,
            target_x,
            target_y,
            saved_a,
            saved_b,
        };
        if let Some(proxy) = xthg_proxy_for_slot(slot) {
            self.set_text_overlay(x, y, proxy);
        }
        // PE monster start writes slot into `wMonsterXTHGIndex` @ `0x4ca81c`.
        self.pe_w_monster_xthg_index = slot as u16;
        if self.kind != CityFileKind::MacCity {
            self.set_misc_u32_at(MISC_MONSTER_XTHG_INDEX_OFFSET, slot as u32);
        }
        self.mark_dirty(x, y);
        Some(slot)
    }

    pub fn pe_monster_actor(&self) -> Option<(usize, Win95OverlayRecord)> {
        self.traffic_overlays.as_ref().and_then(|table| {
            table
                .iter()
                .enumerate()
                .skip(XTHG_FIRST_USABLE_SLOT)
                .find_map(|(slot, record)| record.is_pe_monster_actor().then_some((slot, *record)))
        })
    }

    pub fn pe_monster_actor_at(&self, x: usize, y: usize) -> Option<(usize, Win95OverlayRecord)> {
        self.pe_monster_actor()
            .filter(|(_, record)| record.tile_x as usize == x && record.tile_y as usize == y)
    }

    /// Win95 tornado start `0x45f090`: XTHG kind `0x0f`, dir `rand&7`, height `ALTM&0x1f`.
    /// Paints XTXT `slot-0x37`; no `0xff` footprint.
    pub fn place_pe_tornado_actor(
        &mut self,
        x: usize,
        y: usize,
        lane: u8,
        height: u8,
        target_x: u8,
        target_y: u8,
        _saved_a: u8,
    ) -> Option<usize> {
        if x >= CITY_WIDTH || y >= CITY_HEIGHT || self.pe_tornado_actor().is_some() {
            return None;
        }
        if self.traffic_overlays.is_none() {
            self.traffic_overlays = Some(empty_overlay_table());
        }
        // PE `0x45f121`–`0x45f137`: clear proxy-band occupant, then re-read XTXT into `+A`.
        self.pe_clear_xtxt_proxy_band_occupant(x, y);
        let saved_a = self.text_overlay_at(x, y).unwrap_or(0);
        let slot = self
            .traffic_overlays
            .as_ref()
            .unwrap()
            .iter()
            .enumerate()
            .skip(XTHG_FIRST_USABLE_SLOT)
            .find_map(|(slot, record)| (record.kind == 0).then_some(slot))?;
        self.traffic_overlays.as_mut().unwrap()[slot] = Win95OverlayRecord {
            kind: XTHG_TORNADO,
            lane,
            state: 0,
            tile_x: x as u8,
            tile_y: y as u8,
            height,
            sub_x: XTHG_TORNADO_SUBTILE,
            sub_y: XTHG_TORNADO_SUBTILE,
            target_x,
            target_y,
            saved_a,
            saved_b: 0,
        };
        if let Some(proxy) = xthg_proxy_for_slot(slot) {
            self.set_text_overlay(x, y, proxy);
        }
        self.mark_dirty(x, y);
        Some(slot)
    }

    pub fn pe_tornado_actor(&self) -> Option<(usize, Win95OverlayRecord)> {
        self.traffic_overlays.as_ref().and_then(|table| {
            table
                .iter()
                .enumerate()
                .skip(XTHG_FIRST_USABLE_SLOT)
                .find_map(|(slot, record)| record.is_pe_tornado_actor().then_some((slot, *record)))
        })
    }

    pub fn pe_tornado_actor_at(&self, x: usize, y: usize) -> Option<(usize, Win95OverlayRecord)> {
        self.pe_tornado_actor()
            .filter(|(_, record)| record.tile_x as usize == x && record.tile_y as usize == y)
    }
}

impl City {
    /// Drop consume-`0x45f090` tornados without touching ambient traffic.
    pub fn clear_pe_tornado_actors(&mut self) -> usize {
        let tornado_slots = self
            .traffic_overlays
            .as_ref()
            .map(|table| {
                table
                    .iter()
                    .enumerate()
                    .skip(XTHG_FIRST_USABLE_SLOT)
                    .filter_map(|(slot, record)| {
                        record.is_pe_tornado_actor().then_some((
                            slot,
                            record.tile_x as usize,
                            record.tile_y as usize,
                        ))
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        let mut cleared = 0;
        for (slot, x, y) in tornado_slots {
            if let Some(table) = self.traffic_overlays.as_mut()
                && let Some(record) = table.get_mut(slot)
            {
                // PE `FUN_004507D0`: clear kind only; height/saved_b remain.
                record.kind = 0;
                cleared += 1;
            }
            // PE `FUN_00401528` → `FUN_004507D0`: zero XTXT (no `+A` restore).
            if x < CITY_WIDTH && y < CITY_HEIGHT {
                self.set_text_overlay(x, y, 0);
                self.mark_dirty(x, y);
            }
        }
        cleared
    }

    /// Drop consume-`0x45ee10` monsters without touching ambient traffic.
    pub fn clear_pe_monster_actors(&mut self) -> usize {
        let monster_slots = self
            .traffic_overlays
            .as_ref()
            .map(|table| {
                table
                    .iter()
                    .enumerate()
                    .skip(XTHG_FIRST_USABLE_SLOT)
                    .filter_map(|(slot, record)| {
                        record.is_pe_monster_actor().then_some((
                            slot,
                            record.tile_x as usize,
                            record.tile_y as usize,
                        ))
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        let mut cleared = 0;
        for (slot, x, y) in monster_slots {
            if let Some(table) = self.traffic_overlays.as_mut()
                && let Some(record) = table.get_mut(slot)
            {
                record.kind = 0;
                cleared += 1;
            }
            // PE `FUN_00401528` → `FUN_004507D0`: zero XTXT (no `+A` restore).
            if x < CITY_WIDTH && y < CITY_HEIGHT {
                self.set_text_overlay(x, y, 0);
                self.mark_dirty(x, y);
            }
        }
        if cleared > 0 {
            // PE `FUN_0045d144`: `mov word [0x4ca81c], 0xffff`.
            self.pe_w_monster_xthg_index = 0xffff;
            if self.kind != CityFileKind::MacCity {
                self.set_misc_u32_at(MISC_MONSTER_XTHG_INDEX_OFFSET, 0xffff);
            }
        }
        cleared
    }

    /// Drop consume-`0x45e7d0` crash airplanes without touching ambient runway traffic.
    pub fn clear_pe_aircrash_actors(&mut self) -> usize {
        let crash_slots = self
            .traffic_overlays
            .as_ref()
            .map(|table| {
                table
                    .iter()
                    .enumerate()
                    .skip(XTHG_FIRST_USABLE_SLOT)
                    .filter_map(|(slot, record)| {
                        record.is_pe_aircrash_actor().then_some((
                            slot,
                            record.tile_x as usize,
                            record.tile_y as usize,
                        ))
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        let mut cleared = 0;
        for (slot, x, y) in crash_slots {
            if let Some(table) = self.traffic_overlays.as_mut()
                && let Some(record) = table.get_mut(slot)
            {
                record.kind = 0;
                cleared += 1;
            }
            // PE `FUN_00401528` → `FUN_004507D0`: zero XTXT (no `+A` restore).
            if x < CITY_WIDTH && y < CITY_HEIGHT {
                self.set_text_overlay(x, y, 0);
                self.mark_dirty(x, y);
            }
        }
        cleared
    }
}
