//! Ambient PE transport actors: spawn, placement-face scans, and actor lookup.

use super::*;

impl City {
    /// Win95 `SpawnAeroplane` / `SpawnHelicopter` / `SpawnShip` / `SpawnTrain` / `SpawnSailBoat`.
    pub fn place_ambient_port_actor(
        &mut self,
        x: usize,
        y: usize,
        kind: u8,
        direction: u8,
    ) -> bool {
        self.place_ambient_port_actor_with_crt(x, y, kind, direction, None)
    }

    /// Same as [`Self::place_ambient_port_actor`], optionally consuming process CRT for
    /// PE `SpawnHelicopter` target rolls (`rand()&0x7f` @ `0x452c43` / `0x452c54`) and
    /// PE `SpawnShip` border-water arm (`rand()&3` @ `0x452d18`).
    pub fn place_ambient_port_actor_with_crt(
        &mut self,
        x: usize,
        y: usize,
        kind: u8,
        direction: u8,
        mut crt: Option<&mut Win95CrtRand>,
    ) -> bool {
        // PE SpawnShip / SpawnHelicopter refuse at one active; SpawnSailBoat when
        // `0x4e99d0 >= 4`; SpawnAeroplane refuses when active airplanes reach two.
        if (kind == XTHG_CARGO_SHIP && self.ambient_port_actor_count(XTHG_CARGO_SHIP) >= 1)
            || (kind == XTHG_HELICOPTER && self.ambient_port_actor_count(XTHG_HELICOPTER) >= 1)
            || (kind == XTHG_AIRPLANE && self.ambient_port_actor_count(XTHG_AIRPLANE) >= 2)
            || (kind == XTHG_SAILBOAT && self.pe_sailboat_budget >= 4)
        {
            return false;
        }

        // PE SpawnShip `FUN_00452d00`: with CRT, `rand()&3` picks a map-border water
        // tile (XTER `0x10`); lane aims at the caller (crane) via `0x402dbf`.
        // Offline callers without CRT keep the caller tile (tests / forced place).
        let (x, y, direction) = if kind == XTHG_CARGO_SHIP {
            if let Some(crt) = crt.as_deref_mut() {
                let arm = (crt.random_u16() & 3) as u8;
                let Some((sx, sy)) = pe_spawn_ship_border_water_tile(self, arm) else {
                    return false;
                };
                let lane = pe_xthg_direction_toward(sx as i16, sy as i16, x as i16, y as i16);
                (sx, sy, lane)
            } else {
                (x, y, direction)
            }
        } else {
            (x, y, direction)
        };

        // PE SpawnAeroplane / SpawnHelicopter / SpawnShip refuse when XTXT >= 0xC9.
        if matches!(kind, XTHG_AIRPLANE | XTHG_HELICOPTER | XTHG_CARGO_SHIP)
            && self.text_overlay_at(x, y).unwrap_or(0) >= XTHG_EXPLOSION_XTXT_GATE
        {
            return false;
        }
        // PE SpawnSailBoat (`0x4534a1` / `0x453491`) requires XTXT and XBLD exactly 0.
        if kind == XTHG_SAILBOAT
            && (self.text_overlay_at(x, y).unwrap_or(0) != 0 || self.building_at(x, y) != 0)
        {
            return false;
        }
        // PE `0x4e99d4 != 0` (monster occupancy) refuses plane/heli spawn.
        if matches!(kind, XTHG_AIRPLANE | XTHG_HELICOPTER) && self.pe_monster_actor().is_some() {
            return false;
        }
        let Some(slot) = self.alloc_ambient_port_actor(x, y, kind, direction, 0) else {
            return false;
        };
        if kind == XTHG_CARGO_SHIP {
            // PE `0x452eb7`/`0x452ec3`: +0xa saves prior XTXT (`0x4CA600`), not XTRF.
            let prior_xtxt = self.text_overlay_at(x, y).unwrap_or(0);
            // PE SpawnShip @ `0x452ed9`/`0x452ee0`: spawn tile → `0x4e80cc`/`0x4e80d0`
            // (not XTHG +8/+9; state 3 copies those globals later).
            self.pe_cargo_spawn_tile = (x as u8, y as u8);
            if let Some(record) = self
                .traffic_overlays
                .as_mut()
                .and_then(|table| table.get_mut(slot))
            {
                record.saved_a = prior_xtxt;
                // PE SpawnShip `FUN_00452d00` @ `0x452eab` writes `+5 = 1`,
                // not the generic `0x4E8100` table value (6).
                record.height = 1;
                // PE @ `0x452eca`: force state 0 (alloc defaults to 1).
                record.state = 0;
                // PE does not write XTHG +8/+9 here; state 3 copies globals later.
                record.target_x = 0;
                record.target_y = 0;
            }
        }
        if kind == XTHG_AIRPLANE {
            // PE SpawnAeroplane `FUN_00452830`: `rand()%10 < 5` → edge arm
            // (`0x4529a8`, height `0x10`); else airport arm (`0x452a4b`). Offline
            // callers without CRT keep the airport arm used by runway tests.
            let edge = crt
                .as_deref_mut()
                .is_some_and(|crt| crt.random_u16() % 10 < 5);
            if edge {
                let crt = crt.as_deref_mut().expect("edge arm needs CRT");
                let case = (crt.random_u16() & 3) as u8;
                let jitter = (crt.random_u16() % 100) as u8 + 10;
                let (tile_x, tile_y, lane) = match case {
                    0 => (0u8, jitter, 3u8),
                    1 => (jitter, 0, 5),
                    2 => (0x7f, jitter, 7),
                    _ => (jitter, 0x7f, 1),
                };
                let prior_xtxt = self
                    .text_overlay_at(tile_x as usize, tile_y as usize)
                    .unwrap_or(0);
                let (target_x, target_y) = if direction == 0 {
                    (x as u8, y.saturating_add(0x10) as u8)
                } else {
                    (x.saturating_sub(0x10) as u8, y as u8)
                };
                if let Some(record) = self
                    .traffic_overlays
                    .as_mut()
                    .and_then(|table| table.get_mut(slot))
                {
                    record.tile_x = tile_x;
                    record.tile_y = tile_y;
                    record.lane = lane;
                    record.height = XTHG_AIRCRASH_HEIGHT;
                    record.state = direction.wrapping_shl(4).wrapping_add(3);
                    record.saved_a = prior_xtxt;
                    record.target_x = target_x;
                    record.target_y = target_y;
                }
                if let Some(proxy) = xthg_proxy_for_slot(slot) {
                    self.set_text_overlay(tile_x as usize, tile_y as usize, proxy);
                }
                self.mark_dirty(tile_x as usize, tile_y as usize);
            } else {
                // Airport arm: height/state `0`, targets `+8/+9 = 0x14`.
                let prior_xtxt = self.text_overlay_at(x, y).unwrap_or(0);
                if let Some(record) = self
                    .traffic_overlays
                    .as_mut()
                    .and_then(|table| table.get_mut(slot))
                {
                    record.height = 0;
                    record.state = 0;
                    record.saved_a = prior_xtxt;
                    record.target_x = 0x14;
                    record.target_y = 0x14;
                }
                if let Some(proxy) = xthg_proxy_for_slot(slot) {
                    self.set_text_overlay(x, y, proxy);
                }
            }
        }
        if kind == XTHG_HELICOPTER {
            // PE SpawnHelicopter `FUN_00452ba0` @ `0x452c0d`/`0x452c6b`/`0x452c89`/`0x452c7b`:
            // lane/`+1` forced to `2` (ignore takeoff direction); height/state `0`;
            // save prior XTXT at `+0xa`; paint `slot-0x37`.
            // Targets `+8/+9` are `rand()&0x7f` from CRT @ `0x452c43`/`0x452c54`.
            let prior_xtxt = self.text_overlay_at(x, y).unwrap_or(0);
            let (target_x, target_y) = match crt {
                Some(crt) => (
                    (crt.random_u16() & 0x7f) as u8,
                    (crt.random_u16() & 0x7f) as u8,
                ),
                None => (x as u8, y as u8),
            };
            if let Some(record) = self
                .traffic_overlays
                .as_mut()
                .and_then(|table| table.get_mut(slot))
            {
                record.lane = 2;
                record.height = 0;
                record.state = 0;
                record.saved_a = prior_xtxt;
                record.target_x = target_x;
                record.target_y = target_y;
            }
            if let Some(proxy) = xthg_proxy_for_slot(slot) {
                self.set_text_overlay(x, y, proxy);
            }
        }
        if kind == XTHG_SAILBOAT {
            // PE SpawnSailBoat initializes a separate state machine at
            // sub-coordinate (4, 4), with zero height and no saved fields, then
            // paints XTXT `slot-0x37` (`0x45353c`) — not an XTRF decrement.
            if let Some(record) = self
                .traffic_overlays
                .as_mut()
                .and_then(|table| table.get_mut(slot))
            {
                record.state = 0;
                record.sub_x = 4;
                record.sub_y = 4;
                record.height = 0;
                record.target_x = 0;
                record.target_y = 0;
                record.saved_a = 0;
            }
            if let Some(proxy) = xthg_proxy_for_slot(slot) {
                self.set_text_overlay(x, y, proxy);
            }
            // PE SpawnSailBoat @ `0x45354e`: `inc word ptr [0x4e99d0]`.
            self.pe_sailboat_budget = self.pe_sailboat_budget.saturating_add(1);
        }
        if kind == XTHG_CARGO_SHIP {
            // PE SpawnShip @ `0x452ef9`: paint XTXT `slot-0x37` on `0x4CA600`
            // (same misread-as-XTRF pattern as SailBoat before the archaeology fix).
            if let Some(proxy) = xthg_proxy_for_slot(slot) {
                self.set_text_overlay(x, y, proxy);
            }
        }
        true
    }

    /// Win95 `SpawnTrain`: one engine plus two cars. Rust's `saved_b` links keep
    /// the movement leader chain; `state` stores both native forward links and a
    /// deterministic zero terminator for the final car.
    pub fn place_ambient_train(&mut self, x: usize, y: usize, direction: u8) -> bool {
        // PE `SpawnTrain` @ `0x453603`: `cmp word [0x4e99d8], 5` / `jl` else return 0.
        // Budget DoBudget only compares station census vs active (`0x435e2e`); the hard
        // cap lives here so direct callers match PE.
        let active_train_count = self.ambient_active_train_count();
        if active_train_count >= 5 {
            return false;
        }
        // PE @ `0x45361b`–`0x453646`: refuse when x/y outside `2..=0x7c`.
        if !(2..=0x7c).contains(&x) || !(2..=0x7c).contains(&y) {
            return false;
        }
        // PE @ `0x453665`–`0x4536aa`: XBLD must be surface rail `0x2c..=0x35`
        // (bands that jump to the `(xbld-0x2c)<=9` filter; other jumped bands fail it).
        if !matches!(self.building_at(x, y), 0x2c..=0x35) {
            return false;
        }
        let direction = direction % 8;
        let train_lane = direction / 2;
        let Some(source_xtext) = self.text_overlay_at(x, y) else {
            return false;
        };
        if source_xtext != 0 {
            return false;
        }
        let (dx, dy) = xthg_direction_delta(direction);
        let next_x = x as i16 + dx;
        let next_y = y as i16 + dy;
        if !(0..CITY_WIDTH as i16).contains(&next_x) || !(0..CITY_HEIGHT as i16).contains(&next_y) {
            return false;
        }
        // PE FUN_00453600 calls the allocator three times and does not check
        // slot-zero failures. Preserve that native low-capacity path: slot zero
        // is written as the allocator's failure sentinel, and the constructor
        // still paints its proxy and increments wActiveTrains.
        let engine_slot = self.alloc_ambient_train_record(x, y, XTHG_TRAIN_ENGINE, train_lane, 0);
        let first_car_slot =
            self.alloc_ambient_train_record(x, y, XTHG_TRAIN_CAR, train_lane, engine_slot as u8);
        let second_car_slot =
            self.alloc_ambient_train_record(x, y, XTHG_TRAIN_CAR, train_lane, first_car_slot as u8);
        {
            let overlays = self.traffic_overlays.as_mut().unwrap();
            overlays[engine_slot].state = first_car_slot as u8;
            // Train iPX/iPY are PE next-rail-tile targets, not generic local subtiles.
            overlays[engine_slot].sub_x = next_x as u8;
            overlays[engine_slot].sub_y = next_y as u8;
            overlays[engine_slot].target_x = 0;
            overlays[engine_slot].target_y = 0;
            overlays[engine_slot].saved_a = source_xtext;
            overlays[first_car_slot].state = second_car_slot as u8;
            overlays[first_car_slot].sub_x = x as u8;
            overlays[first_car_slot].sub_y = y as u8;
            overlays[first_car_slot].target_x = 0;
            overlays[first_car_slot].target_y = 0;
            // The native constructor leaves this terminal link unwritten; its traced
            // engine path follows only the engine and first-car links. Rust zeros it
            // for a deterministic chain.
            overlays[second_car_slot].state = 0;
            overlays[second_car_slot].sub_x = x as u8;
            overlays[second_car_slot].sub_y = y as u8;
            overlays[second_car_slot].target_x = 0;
            overlays[second_car_slot].target_y = 0;
        }
        // PE uses `slot + 0xc9`, including `0xc9` when the head allocation
        // failed and returned reserved slot zero.
        self.set_text_overlay(x, y, XTHG_PROXY_BASE + engine_slot as u8);
        if engine_slot == 0
            || first_car_slot == 0
            || second_car_slot == 0
            || self.pe_active_train_count_override.is_some()
        {
            self.pe_active_train_count_override =
                Some(active_train_count.saturating_add(1).min(u16::MAX as usize) as u16);
        }
        true
    }

    fn alloc_ambient_train_record(
        &mut self,
        x: usize,
        y: usize,
        kind: u8,
        direction: u8,
        saved_b: u8,
    ) -> usize {
        if let Some(slot) = self.alloc_ambient_port_actor(x, y, kind, direction, saved_b) {
            return slot;
        }

        // `FUN_00450850` returns index zero when all usable slots 1–39 are
        // occupied. SpawnTrain writes through that result without a guard.
        self.traffic_overlays
            .as_mut()
            .expect("allocator initialized table")[0] = Win95OverlayRecord {
            kind,
            lane: direction % 8,
            state: 1,
            tile_x: x as u8,
            tile_y: y as u8,
            height: xthg_spawn_height(kind),
            sub_x: 8,
            sub_y: 8,
            target_x: x as u8,
            target_y: y as u8,
            saved_a: 0,
            saved_b,
        };
        self.mark_dirty(x, y);
        0
    }

    /// Win95 `SpawnTrain` checks the four rail faces of a 2x2 station in this
    /// fixed order before allocating the consist.
    /// PE station face scanner `0x453590` / tables `0x4e8288`/`0x4e8290`: N/E/S/W
    /// offsets in `0..=0x7f`. XBLD/XTXT/direction are inside SpawnTrain per face.
    pub fn pe_train_station_face_tiles(
        &self,
        station_x: usize,
        station_y: usize,
    ) -> impl Iterator<Item = (usize, usize)> + '_ {
        const PE_STATION_RAIL_FACES: [(i16, i16); 4] = [(0, -1), (2, 0), (0, 2), (-1, 0)];
        PE_STATION_RAIL_FACES
            .into_iter()
            .filter_map(move |(dx, dy)| {
                let x = station_x as i16 + dx;
                let y = station_y as i16 + dy;
                if !(0..=0x7f).contains(&x) || !(0..=0x7f).contains(&y) {
                    return None;
                }
                Some((x as usize, y as usize))
            })
    }

    /// First face that already looks like a SpawnTrain-ready rail tile (tests /
    /// callers that only need a candidate, not the PE retry loop).
    pub fn pe_train_spawn_track_from_station(
        &self,
        station_x: usize,
        station_y: usize,
    ) -> Option<(usize, usize)> {
        self.pe_train_station_face_tiles(station_x, station_y)
            .find(|&(x, y)| {
                matches!(self.building_at(x, y), 44..=53) && self.text_overlay_at(x, y) == Some(0)
            })
    }

    /// PE marina sailboat scanner `0x453380`: cardinal neighbors N/E/S/W with
    /// XBIT water (`0x04` @ `0x4533fa`) and XTXT `0` (`0x453407`). XBLD/XTXT zero
    /// for the allocator is re-checked inside SpawnSailBoat (`0x453491`/`0x4534a1`).
    pub fn pe_sailboat_spawn_faces_from_marina(
        &self,
        marina_x: usize,
        marina_y: usize,
    ) -> impl Iterator<Item = (usize, usize)> + '_ {
        const PE_MARINA_WATER_FACES: [(i16, i16); 4] = [(0, -1), (1, 0), (0, 1), (-1, 0)];
        PE_MARINA_WATER_FACES
            .into_iter()
            .filter_map(move |(dx, dy)| {
                let x = marina_x as i16 + dx;
                let y = marina_y as i16 + dy;
                // PE @ `0x4533bb`–`0x4533e9`: face in `0..=0x7f`.
                if !(0..=0x7f).contains(&x) || !(0..=0x7f).contains(&y) {
                    return None;
                }
                let x = x as usize;
                let y = y as usize;
                if !self
                    .tile_bits_at(x, y)
                    .is_some_and(|bits| bits.water_tile())
                    || self.text_overlay_at(x, y).unwrap_or(0) != 0
                {
                    return None;
                }
                Some((x, y))
            })
    }

    pub fn pe_sailboat_spawn_tile_from_marina(
        &self,
        marina_x: usize,
        marina_y: usize,
    ) -> Option<(usize, usize)> {
        self.pe_sailboat_spawn_faces_from_marina(marina_x, marina_y)
            .next()
    }

    pub(super) fn alloc_ambient_port_actor(
        &mut self,
        x: usize,
        y: usize,
        kind: u8,
        direction: u8,
        saved_b: u8,
    ) -> Option<usize> {
        if !matches!(
            kind,
            XTHG_AIRPLANE
                | XTHG_HELICOPTER
                | XTHG_CARGO_SHIP
                | XTHG_SAILBOAT
                | XTHG_TRAIN_ENGINE
                | XTHG_TRAIN_CAR
                | XTHG_SUBWAY_TRAIN_ENGINE
                | XTHG_SUBWAY_TRAIN_CAR
        ) || x >= CITY_WIDTH
            || y >= CITY_HEIGHT
        {
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
        let height = xthg_spawn_height(kind);
        self.traffic_overlays.as_mut().unwrap()[slot] = Win95OverlayRecord {
            kind,
            lane: direction % 8,
            state: 1,
            tile_x: x as u8,
            tile_y: y as u8,
            height,
            sub_x: 8,
            sub_y: 8,
            target_x: x as u8,
            target_y: y as u8,
            saved_a: 0,
            saved_b,
        };
        self.mark_dirty(x, y);
        if kind == XTHG_CARGO_SHIP {
            self.mark_win95_pe_raising_bridge_render_dirty_near(x, y);
        }
        Some(slot)
    }

    pub fn ambient_port_actor_count(&self, kind: u8) -> usize {
        self.traffic_overlays
            .as_ref()
            .map(|table| {
                table
                    .iter()
                    .skip(XTHG_FIRST_USABLE_SLOT)
                    .filter(|record| record.kind == kind)
                    .count()
            })
            .unwrap_or(0)
    }

    /// Win95 `wActiveTrains` at `0x4E99D8` — surface and subway engines only (cars excluded).
    ///
    /// PE `FUN_00450890` zeros the word each overlay pass, increments once per kind
    /// `0x0A`/`0x0C` dispatch, and decrements on train-merge success (`0x4557a1`). Spawn
    /// gates compare against this count (`< 5` at `0x453603`; vs station capacity at
    /// `0x435e2e`). Rust recounts live engine slots after the actor sweep; the
    /// constructor-only override retains the native increment until that rebuild.
    pub fn ambient_active_train_count(&self) -> usize {
        self.pe_active_train_count_override
            .map(usize::from)
            .unwrap_or_else(|| {
                self.ambient_port_actor_count(XTHG_TRAIN_ENGINE)
                    .saturating_add(self.ambient_port_actor_count(XTHG_SUBWAY_TRAIN_ENGINE))
            })
    }

    pub(crate) fn clear_pe_active_train_count_override(&mut self) {
        self.pe_active_train_count_override = None;
    }

    pub fn ambient_port_actor_at(&self, x: usize, y: usize) -> Option<(usize, Win95OverlayRecord)> {
        self.traffic_overlays.as_ref().and_then(|table| {
            table
                .iter()
                .enumerate()
                .skip(XTHG_FIRST_USABLE_SLOT)
                .find_map(|(slot, record)| {
                    (record.is_ambient_port_actor()
                        && record.tile_x as usize == x
                        && record.tile_y as usize == y)
                        .then_some((slot, *record))
                })
        })
    }
}

/// Win95 `0x402dbf` / `0x4515e0`: 8-way direction from `(x,y)` toward `(tx,ty)`.
fn pe_xthg_direction_toward(x: i16, y: i16, tx: i16, ty: i16) -> u8 {
    let dx = tx - x;
    let dy = ty - y;
    let abs_dx = dx.unsigned_abs();
    let abs_dy = dy.unsigned_abs();
    if (u32::from(abs_dy) + 1) / 2 > u32::from(abs_dx) {
        if dy < 0 { 0 } else { 4 }
    } else if dx < 0 {
        if (u32::from(abs_dx) + 1) / 2 > u32::from(abs_dy) {
            6
        } else if dy < 0 {
            7
        } else {
            5
        }
    } else if (u32::from(abs_dx) + 1) / 2 > u32::from(abs_dy) {
        2
    } else if dy < 0 {
        1
    } else {
        3
    }
}

/// PE `SpawnShip` @ `0x452d18`: last XTER `0x10` on the chosen map-border arm.
/// Arms: 0 → y=2, 1 → y=0x7e, 2 → x=2, 3 → x=0x7e (scan 0..0x7f; last match wins).
fn pe_spawn_ship_border_water_tile(city: &City, arm: u8) -> Option<(usize, usize)> {
    let mut found = None;
    match arm & 3 {
        0 => {
            for x in 0..CITY_WIDTH {
                if city.terrain_at(x, 2) == 0x10 {
                    found = Some((x, 2));
                }
            }
        }
        1 => {
            for x in 0..CITY_WIDTH {
                if city.terrain_at(x, 0x7e) == 0x10 {
                    found = Some((x, 0x7e));
                }
            }
        }
        2 => {
            for y in 0..CITY_HEIGHT {
                if city.terrain_at(2, y) == 0x10 {
                    found = Some((2, y));
                }
            }
        }
        _ => {
            for y in 0..CITY_HEIGHT {
                if city.terrain_at(0x7e, y) == 0x10 {
                    found = Some((0x7e, y));
                }
            }
        }
    }
    found
}
