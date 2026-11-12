//! City-owned XTHG actor slots, proxy overlays, and dispatch/ambient actor state.

use super::*;

/// PE `FUN_0044f910`: signed station XBLD tile census WORD, `sar 3`.
/// Negative values are malformed census state and do not expose a usable actor capacity.
pub(super) fn dispatch_capacity_from_station_tile_count(tile_count: u16) -> usize {
    ((tile_count as i16) >> 3).max(0) as usize
}

#[derive(Clone, Debug, Default)]
pub(super) struct MacDispatch {
    capacities: Option<[usize; 3]>,
    cursors: [usize; 3],
    coordinates: [[(u8, u8); 29]; 3],
}

fn mac_dispatch_proxy_slot(proxy: u8) -> Option<usize> {
    (0xc9..=0xf0)
        .contains(&proxy)
        .then(|| usize::from(proxy - 0xc9))
}

impl City {
    /// Disaster tiles occupy XTXT values 0xfb through 0xff in both the Mac and Win95 saves.
    /// Tornado / monster / air-crash count via their XTHG actors; PE explosions extend an
    /// already-current disaster until their kind-6 animation clears.
    pub fn has_active_disaster(&self) -> bool {
        self.text_overlays
            .as_deref()
            .is_some_and(|overlays| overlays.iter().any(|value| (0xfb..=0xff).contains(value)))
            || self.has_pe_xthg_disaster_actor()
            || (self.pe_w_current_disaster_type != 0
                && self.has_pe_xthg_disaster_continuation_explosion())
    }

    pub fn has_pe_xthg_disaster_actor(&self) -> bool {
        self.pe_aircrash_actor().is_some()
            || self.pe_monster_actor().is_some()
            || self.pe_tornado_actor().is_some()
    }

    /// Win95 `SimulationStartDisaster` keeps continuation active for any kind-6 XTHG
    /// explosion while a disaster type is current (`0x45d0d7..0x45d117`).
    pub(super) fn has_pe_xthg_disaster_continuation_explosion(&self) -> bool {
        self.traffic_overlays.as_deref().is_some_and(|records| {
            records
                .iter()
                .skip(XTHG_FIRST_USABLE_SLOT)
                .any(|record| record.is_pe_explosion_actor())
        })
    }

    /// Win95 overlay actor table from `XTHG` (40×12 bytes). The default renderer keeps this
    /// separate from shared XTRF road-overlay traffic.
    pub fn has_traffic_overlays(&self) -> bool {
        self.traffic_overlays.is_some()
    }

    /// Win95 `DrawLargeTile` / `DrawSmallTile`: a lowered raising bridge is drawn raised when
    /// the first active cargo ship is strictly within `SHIP_MIN_DIST` (8 Manhattan tiles).
    /// The saved tile remains the lowered variant; this is a render-time selection only.
    pub fn win95_pe_raising_bridge_building_for_draw(
        &self,
        x: usize,
        y: usize,
        building: u8,
    ) -> u8 {
        const RAISING_BRIDGE_LOWERED: u8 = 88;
        const RAISING_BRIDGE_RAISED: u8 = 89;
        const SHIP_MIN_DIST: usize = 8;
        if building != RAISING_BRIDGE_LOWERED {
            return building;
        }
        let Some(ship) = self.traffic_overlays.as_ref().and_then(|table| {
            table
                .iter()
                .skip(XTHG_FIRST_USABLE_SLOT)
                .find(|record| record.kind == XTHG_CARGO_SHIP)
        }) else {
            return building;
        };
        let distance = x.abs_diff(ship.tile_x as usize) + y.abs_diff(ship.tile_y as usize);
        (distance < SHIP_MIN_DIST)
            .then_some(RAISING_BRIDGE_RAISED)
            .unwrap_or(building)
    }

    /// Dirty lowered raising-bridge tiles whose PE raised/lowered result can change when a ship
    /// enters or leaves the Manhattan radius. The caller supplies one ship position at a time.
    pub fn mark_win95_pe_raising_bridge_render_dirty_near(&mut self, x: usize, y: usize) {
        const RAISING_BRIDGE_LOWERED: u8 = 88;
        const SHIP_MIN_DIST: usize = 8;
        let min_x = x.saturating_sub(SHIP_MIN_DIST);
        let max_x = x
            .saturating_add(SHIP_MIN_DIST)
            .min(CITY_WIDTH.saturating_sub(1));
        let min_y = y.saturating_sub(SHIP_MIN_DIST);
        let max_y = y
            .saturating_add(SHIP_MIN_DIST)
            .min(CITY_HEIGHT.saturating_sub(1));
        let bridge_tiles = (min_y..=max_y)
            .flat_map(|ty| (min_x..=max_x).map(move |tx| (tx, ty)))
            .filter(|&(tx, ty)| self.building_at(tx, ty) == RAISING_BRIDGE_LOWERED)
            .collect::<Vec<_>>();
        for (tx, ty) in bridge_tiles {
            self.mark_render_dirty(tx, ty);
        }
    }

    pub fn traffic_overlays(&self) -> Option<&[Win95OverlayRecord]> {
        self.traffic_overlays.as_deref()
    }

    /// Whether this record belongs to the port's animated road-traffic path.
    ///
    /// Legacy kind `0x06` remains accepted for blank/test cities, but a serialized PE table
    /// must treat that byte as `XTHG_EXPLOSION`, never as a road car.
    pub(crate) fn is_port_traffic_record(&self, record: Win95OverlayRecord) -> bool {
        record.kind == RUST_TRAFFIC_ACTOR_KIND
            || (!self.traffic_overlays_from_file && record.kind == OVERLAY_KIND_TRAFFIC)
    }

    pub fn traffic_overlays_mut(&mut self) -> Option<&mut [Win95OverlayRecord]> {
        self.traffic_overlays.as_deref_mut()
    }

    /// Write one native Mac XTHG slot while marking the city as changed.
    pub(crate) fn set_mac_object_slot_record(&mut self, slot: usize, record: Win95OverlayRecord) {
        if let Some(target) = self.mac_object_slot_records_mut().get_mut(slot) {
            *target = record;
            self.modified = true;
        }
    }

    /// Mac CODE 6's 40-slot `XTHG` object table shares the serialized chunk
    /// size with Win95, but its kind-specific record semantics are separate.
    /// Create the table when an older Mac city omitted the optional chunk.
    pub(crate) fn mac_object_slot_records_mut(&mut self) -> &mut [Win95OverlayRecord] {
        if self.traffic_overlays.is_none() {
            self.traffic_overlays = Some(empty_overlay_table());
        }
        self.traffic_overlays
            .as_deref_mut()
            .expect("table initialized")
    }

    pub fn traffic_overlays_free_slot(&self) -> Option<usize> {
        self.traffic_overlays
            .as_ref()
            .and_then(|table| table.iter().position(|record| record.kind == 0))
    }

    pub fn traffic_overlays_free_slots(&self) -> usize {
        self.traffic_overlays
            .as_ref()
            .map(|table| table.iter().filter(|record| record.kind == 0).count())
            .unwrap_or(0)
    }

    pub fn dispatch_actor_at(&self, x: usize, y: usize) -> Option<(usize, Win95OverlayRecord)> {
        if x >= CITY_WIDTH || y >= CITY_HEIGHT {
            return None;
        }
        // A live dispatch actor needs its XTXT proxy and matching coordinates.
        // Legacy XBLD-only records cannot hide later map art.
        let slot = xthg_slot_for_proxy(self.text_overlay_at(x, y)?)?;
        let record = *self.traffic_overlays.as_ref()?.get(slot)?;
        (record.is_dispatch_actor() && record.tile_x as usize == x && record.tile_y as usize == y)
            .then_some((slot, record))
    }

    /// Number of simultaneously deployed emergency actors available to a service.
    ///
    /// Win95 `FUN_0044f910` derives police/fire availability from the XBLD tile-census WORDs at
    /// runtime offsets `+0x1a4/+0x1a6`, shifted right by three. This is a tile-count threshold,
    /// not a count of complete station footprints. Military availability follows the
    /// Win95 base type cache (MISC `0x0e4c` in PE files).
    pub fn dispatch_capacity(&self, kind: DispatchKind) -> usize {
        self.dispatch_capacity_for_engine(kind, EngineFidelity::Win95)
    }

    pub(crate) fn dispatch_capacity_for_engine(
        &self,
        kind: DispatchKind,
        fidelity: EngineFidelity,
    ) -> usize {
        if fidelity == EngineFidelity::Mac68k {
            return self
                .mac_dispatch
                .capacities
                .unwrap_or_else(|| self.mac_dispatch_capacities())[kind.index()];
        }
        let police = dispatch_capacity_from_station_tile_count(self.pe_dw_tile_count[210])
            .min(DispatchKind::Police.capacity_limit());
        let fire = dispatch_capacity_from_station_tile_count(self.pe_dw_tile_count[211])
            .min(DispatchKind::Firefighters.capacity_limit());
        let military: usize = match self.pe_w_military_base_type {
            2 => 5,
            3 => 2,
            4 => 3,
            _ => 0,
        }
        .min(DispatchKind::Military.capacity_limit());

        match kind {
            DispatchKind::Police => police,
            DispatchKind::Firefighters => fire,
            DispatchKind::Military if police == 0 && fire == 0 && military == 0 => 1,
            DispatchKind::Military => military,
        }
    }

    fn mac_dispatch_capacities(&self) -> [usize; 3] {
        // CODE 3 export 163 uses LSR.W and one-based coordinates.
        // Limit station counts to the 29 usable entries before the next lane.
        let police = usize::from(self.mac_xbld_tile_count(210) >> 3).min(29);
        let fire = usize::from(self.mac_xbld_tile_count(211) >> 3).min(29);
        let selector = self
            .mac_state_misc_u32_at(MAC_DISPATCH_SELECTOR_MISC_OFFSET)
            .unwrap_or(0) as u8;
        let military = MAC_DISPATCH_MILITARY_CAPACITY_BY_SELECTOR
            .get(usize::from(selector))
            .copied()
            .unwrap_or(0);
        [
            police,
            fire,
            military.max(usize::from(police == 0 && fire == 0)),
        ]
    }

    pub fn can_place_dispatch_actor(&self, x: usize, y: usize, kind: DispatchKind) -> bool {
        self.can_place_dispatch_actor_for_engine(x, y, kind, EngineFidelity::Win95)
    }

    pub(crate) fn can_place_dispatch_actor_for_engine(
        &self,
        x: usize,
        y: usize,
        kind: DispatchKind,
        fidelity: EngineFidelity,
    ) -> bool {
        // Win95 `FUN_0044FB50` / `FUN_0044FD60` / `FUN_0044FF70` and Mac
        // CODE 3 exports 164/165/166 all reject XBIT bit 2 and require an empty
        // XTXT-like marker byte. The Mac handlers read `ffffed40` and
        // `fffffb44`; the former is the XBIT water plane (confirmed by its
        // terrain/water writers). Neither edition's placement gate checks XBLD
        // or infers water from XTER, so a dry road/building tile is eligible.
        // Mac capacity uses its slot-915 selector table. Selector 6 reads the
        // uninitialized local-frame WORD at A6-0x06; keep it bounded as zero.
        let surface_allowed = !self.tile_bits_at(x, y).is_some_and(TileBits::water_tile);
        if self.dispatch_capacity_for_engine(kind, fidelity) == 0
            || x >= CITY_WIDTH
            || y >= CITY_HEIGHT
            || self.text_overlay_at(x, y).unwrap_or(0) != 0
            || self.dispatch_actor_at(x, y).is_some()
            || !surface_allowed
        {
            return false;
        }

        if fidelity == EngineFidelity::Mac68k {
            // CODE 3 updates the coordinate ring even if CODE 6 cannot allocate.
            return true;
        }
        self.traffic_overlays.as_ref().is_none_or(|table| {
            table
                .iter()
                .enumerate()
                .skip(XTHG_FIRST_USABLE_SLOT)
                .any(|(slot, record)| {
                    record.kind == 0
                        || (record.is_rust_dispatch_actor() && record.kind == kind.overlay_kind())
                        || self.legacy_dispatch_xbld_proxy(slot, *record)
                })
        })
    }

    /// Place an emergency actor using the original service-specific ring cursor. Once all
    /// available units are deployed, the next click relocates the oldest unit of that service.
    pub fn place_dispatch_actor(&mut self, x: usize, y: usize, kind: DispatchKind) -> bool {
        self.place_dispatch_actor_for_engine(x, y, kind, EngineFidelity::Win95)
    }

    pub(crate) fn place_dispatch_actor_for_engine(
        &mut self,
        x: usize,
        y: usize,
        kind: DispatchKind,
        fidelity: EngineFidelity,
    ) -> bool {
        if !self.can_place_dispatch_actor_for_engine(x, y, kind, fidelity) {
            return false;
        }
        if fidelity == EngineFidelity::Mac68k {
            return self.place_mac_dispatch_actor(x, y, kind);
        }
        if self.traffic_overlays.is_none() {
            self.traffic_overlays = Some(empty_overlay_table());
        }
        self.clear_legacy_dispatch_actor_proxies();

        let capacity = self.dispatch_capacity_for_engine(kind, fidelity);
        let ring_index = self.dispatch_cursors[kind.index()] % capacity;
        let kind_slots = self
            .traffic_overlays
            .as_ref()
            .expect("overlay table initialized")
            .iter()
            .enumerate()
            .skip(XTHG_FIRST_USABLE_SLOT)
            .filter_map(|(slot, record)| {
                (record.is_rust_dispatch_actor() && record.kind == kind.overlay_kind())
                    .then_some(slot)
            })
            .collect::<Vec<_>>();
        let free_slot = self
            .traffic_overlays
            .as_ref()
            .unwrap()
            .iter()
            .enumerate()
            .skip(XTHG_FIRST_USABLE_SLOT)
            .find_map(|(slot, record)| (record.kind == 0).then_some(slot));
        let Some(slot) = kind_slots
            .get(ring_index)
            .copied()
            .or(free_slot)
            .or_else(|| kind_slots.first().copied())
        else {
            return false;
        };

        self.remove_dispatch_actor_slot(slot);

        let height = xthg_spawn_height(kind.overlay_kind());
        let saved_a = self.text_overlay_at(x, y).unwrap_or(0);
        self.traffic_overlays.as_mut().unwrap()[slot] = Win95OverlayRecord {
            kind: kind.overlay_kind(),
            lane: 0,
            state: 0,
            tile_x: x as u8,
            tile_y: y as u8,
            height,
            sub_x: 0,
            sub_y: 0,
            target_x: 0,
            target_y: 0,
            saved_a,
            saved_b: RUST_DISPATCH_ACTOR_SENTINEL,
        };
        if let Some(proxy) = xthg_proxy_for_slot(slot) {
            self.set_text_overlay(x, y, proxy);
        }
        self.mark_dirty(x, y);
        self.dispatch_cursors[kind.index()] = (ring_index + 1) % capacity;
        true
    }

    fn place_mac_dispatch_actor(&mut self, x: usize, y: usize, kind: DispatchKind) -> bool {
        let service = kind.index();
        let capacity = self.dispatch_capacity_for_engine(kind, EngineFidelity::Mac68k);
        let next = self.mac_dispatch.cursors[service] % capacity;
        self.mac_dispatch.cursors[service] = next + 1;
        let (old_x, old_y) = std::mem::replace(
            &mut self.mac_dispatch.coordinates[service][next],
            (x as u8, y as u8),
        );
        let (old_x, old_y) = (usize::from(old_x), usize::from(old_y));
        let old_slot = self
            .text_overlay_at(old_x, old_y)
            .and_then(mac_dispatch_proxy_slot);
        let records = self.mac_object_slot_records_mut();
        if let Some(slot) = old_slot
            && records[slot].kind == kind.overlay_kind()
        {
            // The native cleanup keys on the old map proxy and kind, even
            // when the stored record coordinates have gone stale.
            records[slot].kind = 0;
            self.set_text_overlay(old_x, old_y, 0);
        }
        let Some((slot, record)) = self
            .mac_object_slot_records_mut()
            .iter_mut()
            .enumerate()
            .skip(XTHG_FIRST_USABLE_SLOT)
            .find(|(_, record)| record.kind == 0)
        else {
            return false;
        };
        // CODE 6 export 108 keeps +2, +8/+9, and +11 from the free slot.
        record.kind = kind.overlay_kind();
        record.lane = 0;
        record.tile_x = x as u8;
        record.tile_y = y as u8;
        record.height = 0;
        record.sub_x = 0;
        record.sub_y = 0;
        record.saved_a = 0;
        self.paint_xthg_proxy_tile(slot, x, y);
        true
    }

    pub(super) fn restore_xthg_proxy_tile(&mut self, slot: usize, x: usize, y: usize, saved_a: u8) {
        if x >= CITY_WIDTH || y >= CITY_HEIGHT {
            return;
        }
        if let Some(proxy) = xthg_proxy_for_slot(slot)
            && self.text_overlay_at(x, y) == Some(proxy)
        {
            self.set_text_overlay(x, y, saved_a);
        }
        self.mark_dirty(x, y);
    }

    pub(super) fn paint_xthg_proxy_tile(&mut self, slot: usize, x: usize, y: usize) {
        if x < CITY_WIDTH
            && y < CITY_HEIGHT
            && let Some(proxy) = xthg_proxy_for_slot(slot)
        {
            self.set_text_overlay(x, y, proxy);
        }
    }

    pub(crate) fn move_ambient_train_xthg_proxy(
        &mut self,
        slot: usize,
        old_x: usize,
        old_y: usize,
        old_saved_a: u8,
        new_x: usize,
        new_y: usize,
    ) -> u8 {
        self.restore_xthg_proxy_tile(slot, old_x, old_y, old_saved_a);
        let displaced = self.text_overlay_at(new_x, new_y).unwrap_or(0);
        if new_x < CITY_WIDTH
            && new_y < CITY_HEIGHT
            && let Some(proxy) = xthg_proxy_for_slot(slot)
        {
            self.set_text_overlay(new_x, new_y, proxy);
        }
        displaced
    }

    /// PE `FUN_00456E10` cargo tile handoff: unconditional old-tile XTXT restore from
    /// `+A` (`0x456efb`), then dest XTXT into `+A` and paint `slot-0x37` (`0x456f50`).
    pub(crate) fn move_ambient_cargo_xthg_proxy(
        &mut self,
        slot: usize,
        old_x: usize,
        old_y: usize,
        old_saved_a: u8,
        new_x: usize,
        new_y: usize,
    ) -> u8 {
        if old_x < CITY_WIDTH && old_y < CITY_HEIGHT {
            self.set_text_overlay(old_x, old_y, old_saved_a);
            self.mark_dirty(old_x, old_y);
        }
        let displaced = self.text_overlay_at(new_x, new_y).unwrap_or(0);
        self.paint_xthg_proxy_tile(slot, new_x, new_y);
        displaced
    }

    /// PE `FUN_00455500` @ `0x455822` train XTXT handoff before `FUN_004029FF` consist copy:
    /// 1. first-car proxy (`slot-0x37`) at engine's old tile
    /// 2. second-car proxy at first car's old tile
    /// 3. second car's old tile ← second `+A` (unconditional)
    /// 4. displaced target XTXT saved for engine `+A`, then engine proxy at target
    ///
    /// Returns the displaced target byte for engine `+A`.
    pub(crate) fn advance_ambient_train_xthg_proxies(
        &mut self,
        engine_slot: usize,
        engine: Win95OverlayRecord,
        first_car_slot: usize,
        first_car: Win95OverlayRecord,
        second_car_slot: usize,
        second_car: Win95OverlayRecord,
        engine_new_x: usize,
        engine_new_y: usize,
    ) -> u8 {
        self.paint_xthg_proxy_tile(
            first_car_slot,
            engine.tile_x as usize,
            engine.tile_y as usize,
        );
        self.paint_xthg_proxy_tile(
            second_car_slot,
            first_car.tile_x as usize,
            first_car.tile_y as usize,
        );
        // PE @ `0x455873` writes `+A` unconditionally (no proxy-match gate).
        let second_x = second_car.tile_x as usize;
        let second_y = second_car.tile_y as usize;
        if second_x < CITY_WIDTH && second_y < CITY_HEIGHT {
            self.set_text_overlay(second_x, second_y, second_car.saved_a);
            self.mark_dirty(second_x, second_y);
        }
        let displaced = self
            .text_overlay_at(engine_new_x, engine_new_y)
            .unwrap_or(0);
        self.paint_xthg_proxy_tile(engine_slot, engine_new_x, engine_new_y);
        displaced
    }

    pub fn remove_dispatch_actor_at(&mut self, x: usize, y: usize) -> bool {
        let Some((slot, _)) = self.dispatch_actor_at(x, y) else {
            return false;
        };
        self.remove_dispatch_actor_slot(slot)
    }

    /// Opening the original emergency palette removes all prior dispatch markers and resets the
    /// three service cursors before accepting a new dispatch sequence.
    pub(crate) fn reset_emergency_dispatch_for_engine(&mut self, fidelity: EngineFidelity) {
        if fidelity.uses_win95() {
            self.reset_emergency_dispatch();
        } else {
            self.mac_dispatch.capacities = Some(self.mac_dispatch_capacities());
            self.mac_dispatch.cursors = [0; 3];
            self.mac_clear_dispatch_proxies();
        }
    }

    /// Native setup and end-of-action cleanup visit proxies in x-major order.
    pub(crate) fn mac_clear_dispatch_proxies(&mut self) {
        for x in 0..CITY_WIDTH {
            for y in 0..CITY_HEIGHT {
                let Some(slot) = self.text_overlay_at(x, y).and_then(mac_dispatch_proxy_slot)
                else {
                    continue;
                };
                let Some(record) = self
                    .traffic_overlays
                    .as_mut()
                    .and_then(|table| table.get_mut(slot))
                else {
                    continue;
                };
                if record.is_dispatch_actor() {
                    record.kind = 0;
                    self.set_text_overlay(x, y, 0);
                }
            }
        }
    }

    pub fn reset_emergency_dispatch(&mut self) {
        self.dispatch_cursors = [0; 3];
        self.pe_maxis_daily_clear_dispatch_xtxt_proxies();
    }

    /// PE palette setup at `0x44fa53` and disaster end at `0x45d22f` clear
    /// responder records reached through XTXT in increasing X, then Y.
    pub fn pe_maxis_daily_clear_dispatch_xtxt_proxies(&mut self) -> usize {
        if self.traffic_overlays.is_none() || self.text_overlays.is_none() {
            return 0;
        }
        let mut cleared = 0;
        for x in 0..CITY_WIDTH {
            for y in 0..CITY_HEIGHT {
                let Some(slot) = self
                    .text_overlay_at(x, y)
                    .and_then(|marker| marker.checked_sub(XTHG_PROXY_BASE))
                    .map(usize::from)
                else {
                    continue;
                };
                let Some(record) = self
                    .traffic_overlays
                    .as_mut()
                    .and_then(|table| table.get_mut(slot))
                else {
                    continue;
                };
                if record.is_dispatch_actor() {
                    record.kind = 0;
                    self.set_text_overlay(x, y, 0);
                    cleared += 1;
                }
            }
        }
        if cleared > 0 {
            self.modified = true;
        }
        cleared
    }

    fn legacy_dispatch_xbld_proxy(&self, slot: usize, record: Win95OverlayRecord) -> bool {
        let Some(proxy) = xthg_proxy_for_slot(slot) else {
            return false;
        };
        let (x, y) = (usize::from(record.tile_x), usize::from(record.tile_y));
        // Older CityGame dispatch wrote its proxy into XBLD and saved the
        // displaced building in +0xa. Native dispatch puts the proxy in XTXT.
        record.is_dispatch_actor()
            && [
                record.lane,
                record.state,
                record.sub_x,
                record.sub_y,
                record.target_x,
                record.target_y,
                record.saved_b,
            ] == [0; 7]
            && self.building_at(x, y) == proxy
            && self
                .text_overlays
                .as_ref()
                .is_none_or(|plane| !plane.contains(&proxy))
    }

    pub fn clear_legacy_dispatch_actor_proxies(&mut self) -> usize {
        let legacy_slots: Vec<_> = self
            .traffic_overlays
            .iter()
            .flat_map(|table| {
                table
                    .iter()
                    .copied()
                    .enumerate()
                    .skip(XTHG_FIRST_USABLE_SLOT)
            })
            .filter(|&(slot, record)| self.legacy_dispatch_xbld_proxy(slot, record))
            .collect();
        for &(slot, record) in &legacy_slots {
            self.traffic_overlays.as_mut().unwrap()[slot].kind = 0;
            self.set_building(
                usize::from(record.tile_x),
                usize::from(record.tile_y),
                record.saved_a,
            );
        }
        if !legacy_slots.is_empty() {
            self.modified = true;
        }
        legacy_slots.len()
    }

    pub(super) fn remove_dispatch_actor_slot(&mut self, slot: usize) -> bool {
        let Some(record) = self
            .traffic_overlays
            .as_ref()
            .and_then(|table| table.get(slot))
            .copied()
            .filter(|record| record.is_dispatch_actor())
        else {
            return false;
        };
        self.traffic_overlays.as_mut().unwrap()[slot].kind = 0;
        self.modified = true;

        let x = record.tile_x as usize;
        let y = record.tile_y as usize;
        if x < CITY_WIDTH && y < CITY_HEIGHT {
            self.restore_xthg_proxy_tile(slot, x, y, record.saved_a);
            self.mark_dirty(x, y);
        }
        true
    }

    pub fn mark_traffic_layers_dirty(&mut self) {
        self.modified = true;
        let tiles: Vec<(usize, usize)> = self
            .traffic_overlays
            .as_ref()
            .map(|table| {
                table
                    .iter()
                    .filter(|record| {
                        self.is_port_traffic_record(**record) && record.is_drawable_traffic()
                    })
                    .map(|record| (record.tile_x as usize, record.tile_y as usize))
                    .collect()
            })
            .unwrap_or_default();
        for (x, y) in tiles {
            self.mark_render_dirty(x, y);
        }
    }

    #[cfg(test)]
    pub fn set_traffic_overlays_for_test(&mut self, table: Vec<Win95OverlayRecord>) {
        self.traffic_overlays = Some(table);
        self.traffic_overlays_from_file = false;
    }

    /// Install an XTHG table as if it were decoded from a PE save (`from_file = true`).
    #[cfg(test)]
    pub fn set_traffic_overlays_loaded_for_test(&mut self, table: Vec<Win95OverlayRecord>) {
        self.traffic_overlays = Some(table);
        self.traffic_overlays_from_file = true;
    }
}
