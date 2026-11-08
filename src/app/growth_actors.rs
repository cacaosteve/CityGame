//! Growth-tick port/transit actor spawning and arcology microsim updates.
//!
//! Spawn predicates, process random-stream boundaries, face retries and microsim
//! writes retain their existing behavior. Callers own growth scheduling; city
//! constructors, actor ticking and supported road traffic retain their owners.

#[cfg(test)]
use super::weather_advance_seed;
use super::{
    MARINA_BUILDING_ID, RAIL_STATION_BUILDING_ID, RUNWAY_BUILDING_ID, SEAPORT_CRANE_BUILDING_ID,
    ViewerState, ZONE_AIRPORT, ZONE_SEAPORT, pe_density_microsim_origin,
};
use crate::city::{City, MAX_SIM_TEXT_ENTRIES, MIN_SIM_TEXT_ENTRIES, ZONE_CORNER_TRIGHT};
use crate::mac_random::MacToolboxRng;
use crate::win95_overlay::{
    XTHG_AIRPLANE, XTHG_CARGO_SHIP, XTHG_DIRECTION_EAST, XTHG_DIRECTION_NORTH, XTHG_HELICOPTER,
    XTHG_SAILBOAT,
};
use crate::win95_overlay_sim::ambient_train_spawn_direction;
use crate::win95_random::{Win95CrtRand, Win95Lfsr};

trait AmbientPortSpawnRng {
    fn next_port_random_u16(&mut self) -> u16;
}

impl AmbientPortSpawnRng for MacToolboxRng {
    fn next_port_random_u16(&mut self) -> u16 {
        self.random_u16()
    }
}

impl AmbientPortSpawnRng for Win95CrtRand {
    fn next_port_random_u16(&mut self) -> u16 {
        self.random_u16()
    }
}

impl ViewerState {
    /// Win95 civilian `DoAirPortGrowth` / `DoSeaPortGrowth` spawn branch.
    ///
    /// PE SimulationGrow airport @ `0x436601`: when `(rand() & 3) != 0` (fallthrough; zero
    /// `je`s to the civilian ladder @ `0x43670d`), runway `0xDD` tiles may spawn heli/plane
    /// (`0x436607`+). Seaport @ `0x43652d`: zero `je`s to the crane/cargo picker; nonzero
    /// fallthrough requires crane `0xe0` (`0x43652f`) then inner `(rand() & 3) == 0`
    /// (`0x43653f`) before `0x402829` → `SpawnShip`. `force` skips probability gates
    /// (still requires XBIT powered/`0x40` on runways).
    fn maybe_spawn_ambient_port_traffic_at<R: AmbientPortSpawnRng>(
        &mut self,
        x: usize,
        y: usize,
        rng: &mut R,
        force: bool,
        branch_already_selected: bool,
    ) -> usize {
        let Some(zone_id) = self.city.zone_at(x, y).map(|zone| zone.zone_id()) else {
            return 0;
        };
        if !matches!(zone_id, ZONE_AIRPORT | ZONE_SEAPORT) {
            return 0;
        }
        if !force && !branch_already_selected && (rng.next_port_random_u16() & 3) == 0 {
            return 0;
        }
        let building = self.city.building_at(x, y);
        match zone_id {
            ZONE_AIRPORT if building == RUNWAY_BUILDING_ID => {
                // PE @ `0x436614`: `rand() % 0x1e == 0`.
                if !force && rng.next_port_random_u16() % 30 != 0 {
                    return 0;
                }
                // PE @ `0x43664e`: test the runway tile's saved XBIT `0x40`.
                if !pe_budget_transport_xbit_powered(&self.city, x, y) {
                    return 0;
                }
                // PE @ `0x43666a`: `rand() % 0x0a < 4` → SpawnHelicopter (`0x452ba0`/`0x452c0d`).
                let spawn_heli = if force {
                    false
                } else {
                    rng.next_port_random_u16() % 10 < 4
                };
                if spawn_heli {
                    // PE SpawnHelicopter @ `0x452c0d` hardcodes lane 2; takeoff dir unused.
                    return usize::from(self.city.place_ambient_port_actor_with_crt(
                        x,
                        y,
                        XTHG_HELICOPTER,
                        2,
                        Some(&mut self.pe_process_crt),
                    ));
                }
                let direction = ambient_airplane_takeoff_direction(
                    self.city.rotation(),
                    self.city
                        .tile_bits_at(x, y)
                        .is_some_and(|bits| bits.rotate()),
                );
                usize::from(self.city.place_ambient_port_actor_with_crt(
                    x,
                    y,
                    XTHG_AIRPLANE,
                    direction,
                    Some(&mut self.pe_process_crt),
                ))
            }
            ZONE_SEAPORT if building == SEAPORT_CRANE_BUILDING_ID => {
                // PE @ `0x43653f`: `test al, 3` / `jne` exit — SpawnShip only when == 0.
                if !force && (rng.next_port_random_u16() & 3) != 0 {
                    return 0;
                }
                // PE @ `0x402829` → `SpawnShip` (`0x452d00`); border water via process CRT.
                usize::from(self.city.place_ambient_port_actor_with_crt(
                    x,
                    y,
                    XTHG_CARGO_SHIP,
                    XTHG_DIRECTION_EAST,
                    Some(&mut self.pe_process_crt),
                ))
            }
            _ => 0,
        }
    }

    /// Win95 civilian port spawn on the growth-tick process CRT stream (`0x48cdd8`).
    pub(super) fn maybe_spawn_ambient_port_traffic_at_with_crt(
        &mut self,
        x: usize,
        y: usize,
        crt: &mut Win95CrtRand,
        force: bool,
        branch_already_selected: bool,
    ) -> usize {
        let Some(zone_id) = self.city.zone_at(x, y).map(|zone| zone.zone_id()) else {
            return 0;
        };
        if !matches!(zone_id, ZONE_AIRPORT | ZONE_SEAPORT) {
            return 0;
        }
        if !force && !branch_already_selected && (crt.next_port_random_u16() & 3) == 0 {
            return 0;
        }
        let building = self.city.building_at(x, y);
        match zone_id {
            ZONE_AIRPORT if building == RUNWAY_BUILDING_ID => {
                // PE @ `0x436614`: `rand() % 0x1e == 0`.
                if !force && crt.next_port_random_u16() % 30 != 0 {
                    return 0;
                }
                // PE @ `0x43664e`: test the runway tile's saved XBIT `0x40`.
                if !pe_budget_transport_xbit_powered(&self.city, x, y) {
                    return 0;
                }
                // PE @ `0x43666a`: `rand() % 0x0a < 4` → SpawnHelicopter.
                let spawn_heli = if force {
                    false
                } else {
                    crt.next_port_random_u16() % 10 < 4
                };
                if spawn_heli {
                    // PE SpawnHelicopter @ `0x452c0d` hardcodes lane 2; takeoff dir unused.
                    return usize::from(self.city.place_ambient_port_actor_with_crt(
                        x,
                        y,
                        XTHG_HELICOPTER,
                        2,
                        Some(crt),
                    ));
                }
                let direction = ambient_airplane_takeoff_direction(
                    self.city.rotation(),
                    self.city
                        .tile_bits_at(x, y)
                        .is_some_and(|bits| bits.rotate()),
                );
                usize::from(self.city.place_ambient_port_actor_with_crt(
                    x,
                    y,
                    XTHG_AIRPLANE,
                    direction,
                    Some(crt),
                ))
            }
            ZONE_SEAPORT if building == SEAPORT_CRANE_BUILDING_ID => {
                // PE @ `0x43653f`: `test al, 3` / `jne` exit — SpawnShip only when == 0.
                if !force && (crt.next_port_random_u16() & 3) != 0 {
                    return 0;
                }
                // PE @ `0x402829` → `SpawnShip` (`0x452d00`); border water via process CRT.
                usize::from(self.city.place_ambient_port_actor_with_crt(
                    x,
                    y,
                    XTHG_CARGO_SHIP,
                    XTHG_DIRECTION_EAST,
                    Some(crt),
                ))
            }
            _ => 0,
        }
    }

    #[cfg(test)]
    pub(super) fn spawn_ambient_port_traffic_at_for_test(
        &mut self,
        x: usize,
        y: usize,
        force: bool,
    ) -> usize {
        let mut rng = MacToolboxRng::new(
            weather_advance_seed(&self.city, self.game_clock).wrapping_add(0x5854_4847),
        );
        self.maybe_spawn_ambient_port_traffic_at(x, y, &mut rng, force, false)
    }

    #[cfg(test)]
    pub(super) fn spawn_ambient_port_traffic_at_with_crt_for_test(
        &mut self,
        x: usize,
        y: usize,
        force: bool,
        crt: &mut Win95CrtRand,
    ) -> usize {
        self.spawn_ambient_port_traffic_at_with_crt_branch_for_test(x, y, force, false, crt)
    }

    #[cfg(test)]
    pub(super) fn spawn_ambient_port_traffic_at_with_crt_branch_for_test(
        &mut self,
        x: usize,
        y: usize,
        force: bool,
        branch_already_selected: bool,
        crt: &mut Win95CrtRand,
    ) -> usize {
        self.maybe_spawn_ambient_port_traffic_at_with_crt(x, y, crt, force, branch_already_selected)
    }

    /// PE DoBudgetOvergroundTransportCheck train / sailboat spawn @ `0x4351da` / `0x43524a`.
    fn maybe_spawn_microsim_transit_actor_at(
        &mut self,
        x: usize,
        y: usize,
        lfsr: &mut Win95Lfsr,
        force: bool,
    ) -> usize {
        let building = self.city.building_at(x, y);
        match building {
            RAIL_STATION_BUILDING_ID => {
                // PE @ `0x435e09`: `test XBIT, 0x40` before the RandomAnd3 gate.
                if !pe_budget_transport_xbit_powered(&self.city, x, y) {
                    return 0;
                }
                // PE @ `0x435e0f`: `0x402261` → `0x47fb70` RandomAnd3; nonzero skips.
                if !force && lfsr.random_mask_03() != 0 {
                    return 0;
                }
                // PE @ `0x435e1e`: `dwTileCount[0xed]` (`0x4ca4c8+0x1da`) / 4 vs
                // `wActiveTrains` (`0x4e99d8`) — not a live map rescan.
                // Hard cap `>= 5` is inside SpawnTrain (`0x453603`), not this arm.
                let station_units =
                    u32::from(self.city.pe_dw_tile_count[RAIL_STATION_BUILDING_ID as usize]) / 4;
                let active_trains = self.city.ambient_active_train_count() as u32;
                if station_units <= active_trains {
                    return 0;
                }
                // PE SpawnTrain @ `0x453603` refuses at `wActiveTrains >= 5`,
                // before the face-specific direction draw at `0x4536c9`.
                // Keep the constructor guard too for non-budget callers.
                if active_trains >= 5 {
                    return 0;
                }
                // PE `0x40209f`→`0x453590`: try each station face until SpawnTrain
                // succeeds. Early SpawnTrain refuses (bounds/XBLD/XTXT) must not
                // draw the direction LFSR; only a face that reaches `0x4536c9` does.
                let faces: Vec<(usize, usize)> =
                    self.city.pe_train_station_face_tiles(x, y).collect();
                for (train_x, train_y) in faces {
                    if !(2..=0x7c).contains(&train_x) || !(2..=0x7c).contains(&train_y) {
                        continue;
                    }
                    if !matches!(self.city.building_at(train_x, train_y), 0x2c..=0x35)
                        || self.city.text_overlay_at(train_x, train_y) != Some(0)
                    {
                        continue;
                    }
                    let initial_cardinal = lfsr.random_mod(4) as u8;
                    let Some(direction) = ambient_train_spawn_direction(
                        &self.city,
                        train_x,
                        train_y,
                        initial_cardinal,
                        &mut self.pe_process_game_rand,
                    ) else {
                        continue;
                    };
                    if self.city.place_ambient_train(train_x, train_y, direction) {
                        return 1;
                    }
                }
                0
            }
            MARINA_BUILDING_ID => {
                // PE @ `0x435e73`: `test XBIT, 0x40` before the RandomAnd3 gate.
                if !pe_budget_transport_xbit_powered(&self.city, x, y) {
                    return 0;
                }
                // PE @ `0x435e79`: `0x402261` → `0x47fb70` RandomAnd3; nonzero skips.
                if !force && lfsr.random_mask_03() != 0 {
                    return 0;
                }
                // PE marina twin of rail @ `0x435e1e`: `dwTileCount[0xf8]` / 9 vs
                // `wSailingBoats` — not a live map rescan.
                let marina_units =
                    u32::from(self.city.pe_dw_tile_count[MARINA_BUILDING_ID as usize]) / 9;
                // PE @ `0x435298`: `cmp ax,[0x4e99d0]` then `jng` refuse —
                // capacity is `wSailingBoats`, not live XTHG sailboat count.
                let active_boats = u32::from(self.city.pe_sailboat_budget);
                if marina_units <= active_boats {
                    return 0;
                }
                // PE `0x402978`→`0x453380`: try each marina face until SpawnSailBoat
                // succeeds. Face scan is water+XTXT; XBLD==0 is inside the spawn.
                let faces: Vec<(usize, usize)> = self
                    .city
                    .pe_sailboat_spawn_faces_from_marina(x, y)
                    .collect();
                for (boat_x, boat_y) in faces {
                    // PE SpawnSailBoat checks XBLD==0 @ `0x453491` before `0x401cad(3)`.
                    if self.city.building_at(boat_x, boat_y) != 0 {
                        continue;
                    }
                    let direction = lfsr.random_mod(3) as u8;
                    if self
                        .city
                        .place_ambient_port_actor(boat_x, boat_y, XTHG_SAILBOAT, direction)
                    {
                        return 1;
                    }
                }
                0
            }
            _ => 0,
        }
    }

    /// PE DoBudgetOvergroundTransportCheck arco arm @ `0x4352b4` (`SetMicroSimulatorStat0`).
    fn maybe_update_arco_microsim_at(&mut self, x: usize, y: usize) -> bool {
        let building = self.city.building_at(x, y);
        if !matches!(building, 251..=254) {
            return false;
        }
        // PE @ `0x4352d4`: `(XZON & 0xf0) == 0x80` — exact TRIGHT nibble, not bit test.
        if !self
            .city
            .zone_at(x, y)
            .is_some_and(|zone| zone.raw_bits() & 0xf0 == ZONE_CORNER_TRIGHT)
        {
            return false;
        }
        let text = self.city.text_overlay_at(x, y).unwrap_or(0);
        if !(MIN_SIM_TEXT_ENTRIES..=MAX_SIM_TEXT_ENTRIES).contains(&text) {
            return false;
        }
        let index = usize::from(text.saturating_sub(MIN_SIM_TEXT_ENTRIES));
        let Some(record) = self.city.microsim_at(index) else {
            return false;
        };
        if !matches!(record.tile_id, 251..=254) {
            return false;
        }
        let (mx, my) = pe_density_microsim_origin(x, y);
        // PE @ `0x43530b`+: XVAL/XCRM/XPLT via microsim fold.
        let land = i16::from(self.city.land_value_at(mx, my).unwrap_or(0) >> 5);
        let crime = i16::from(self.city.crime_density_at(mx, my).unwrap_or(0) >> 5);
        let pollution = i16::from(self.city.pollution_density_at(mx, my).unwrap_or(0) >> 5);
        let mut stat0 = land - crime - pollution + 12;
        // PE @ `0x435fb2`: test this arcology tile's saved XBIT `0x40`.
        if !pe_budget_transport_xbit_powered(&self.city, x, y) {
            stat0 /= 2;
        }
        if !self
            .city
            .tile_bits_at(x, y)
            .is_some_and(|bits| bits.watered())
        {
            stat0 /= 2;
        }
        let stat0 = stat0.clamp(0, 12) as u8;
        self.city.set_microsim_stat0(index, stat0)
    }

    pub(super) fn update_microsim_growth_tick_at(
        &mut self,
        x: usize,
        y: usize,
        lfsr: &mut Win95Lfsr,
        force: bool,
    ) -> usize {
        let spawned = self.maybe_spawn_microsim_transit_actor_at(x, y, lfsr, force);
        self.maybe_update_arco_microsim_at(x, y);
        spawned
    }

    #[cfg(test)]
    pub(super) fn spawn_microsim_transit_actor_at_for_test(
        &mut self,
        x: usize,
        y: usize,
        force: bool,
    ) -> usize {
        let mut lfsr = self.pe_process_lfsr;
        let spawned = self.maybe_spawn_microsim_transit_actor_at(x, y, &mut lfsr, force);
        self.pe_process_lfsr = lfsr;
        spawned
    }

    #[cfg(test)]
    pub(super) fn update_arco_microsim_at_for_test(&mut self, x: usize, y: usize) -> bool {
        self.maybe_update_arco_microsim_at(x, y)
    }
}

/// Win95 runway takeoff facing from view rotation parity and XBIT flip.
pub(super) fn ambient_airplane_takeoff_direction(view_rotation: u8, flipped: bool) -> u8 {
    let takeoff_north_south = if view_rotation % 2 == 0 {
        !flipped
    } else {
        flipped
    };
    if takeoff_north_south {
        XTHG_DIRECTION_NORTH
    } else {
        XTHG_DIRECTION_EAST
    }
}

/// PE DoBudgetOvergroundTransportCheck / SimulationGrow read the tile's saved XBIT `0x40`.
pub(super) fn pe_budget_transport_xbit_powered(city: &City, x: usize, y: usize) -> bool {
    city.tile_bits_at(x, y).is_some_and(|bits| bits.powered())
}
