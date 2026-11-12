//! Water service maps, capacity, allocation, and service summaries.
//!
//! The native PE/Mac allocation paths consume saved XBIT roots. Inferred pipe
//! geometry remains in `pipes.rs`, while power-map dependency calls stay
//! explicit through the parent service module.

use super::*;

pub(in crate::app) fn is_water_source_building(building: u8) -> bool {
    matches!(building, 220 | 250)
}

pub(in crate::app) fn water_pump_output_tiles(
    city: &City,
    x: usize,
    y: usize,
    sea_level: u32,
    precipitation: u32,
) -> u32 {
    // PE FUN_004649E0 classifies the complete centered 3×3, including the
    // pump cell, from XBIT bits 0 and 2: 0x04 is fresh surface water and 0x05
    // is salt water. Use the saved plane directly instead of inferring this
    // from terrain, which can disagree on imported or hand-edited cities.
    let fresh_water_tiles = centered_tiles(x, y)
        .filter(|&(tx, ty)| pe_fresh_surface_water(city, tx, ty))
        .count() as u32;
    // PE 0x464b6c sign-extends the selected WORD before the DWORD ADDs.
    // Explicit wrapping also retains that arithmetic in checked builds.
    sea_level
        .wrapping_mul(5)
        .wrapping_add(fresh_water_tiles * 10)
        .wrapping_add(precipitation / 2)
}

/// CityGame's added capacity estimate treats a pump as powered when its
/// geometry-derived network is energized. The PE water worker itself roots
/// pumps from the saved XBIT `0x40` bit in `pe_water_service_maps_from_saved_xbit`.
pub(in crate::app) fn water_pump_has_power_in_capacity_estimate(
    power_maps: &PowerServiceMaps,
    index: usize,
) -> bool {
    power_maps.energized_tiles[index]
}

pub(in crate::app) fn water_supply_capacity_tiles(
    city: &City,
    sea_level: u32,
    precipitation: u32,
) -> u32 {
    water_supply_capacity_tiles_with_entry_seed(
        city,
        sea_level,
        precipitation,
        pe_power_recalc_seed(city),
    )
}

pub(in crate::app) fn water_supply_capacity_tiles_with_entry_seed(
    city: &City,
    sea_level: u32,
    precipitation: u32,
    entry_seed: u32,
) -> u32 {
    let power_maps = power_service_maps_with_entry_seed(city, entry_seed);
    let mut seen = vec![false; CITY_TILES];
    let mut capacity = 0;
    for y in 0..CITY_HEIGHT {
        for x in 0..CITY_WIDTH {
            let index = y * CITY_WIDTH + x;
            match city.building_at(x, y) {
                220 if water_pump_has_power_in_capacity_estimate(&power_maps, index) => {
                    capacity += water_pump_output_tiles(city, x, y, sea_level, precipitation);
                }
                250 => {
                    let Some((origin_x, origin_y)) =
                        building_footprint_origin_containing(city, x, y, 250, 3)
                    else {
                        continue;
                    };
                    let origin_index = origin_y * CITY_WIDTH + origin_x;
                    if seen[origin_index] {
                        continue;
                    }
                    seen[origin_index] = true;
                    if powered_building_footprint_at_origin(
                        city,
                        &power_maps,
                        origin_x,
                        origin_y,
                        250,
                        3,
                    ) {
                        capacity += desalination_output_tiles(city, origin_x, origin_y);
                    }
                }
                _ => {}
            }
        }
    }
    capacity
}

pub(in crate::app) fn connected_water_supply_capacity_tiles(
    city: &City,
    connected_tiles: &[bool],
) -> u32 {
    let power_maps = power_service_maps(city);
    connected_water_supply_capacity_tiles_with_power(city, connected_tiles, &power_maps)
}

pub(in crate::app) fn connected_water_supply_capacity_tiles_with_power(
    city: &City,
    connected_tiles: &[bool],
    power_maps: &PowerServiceMaps,
) -> u32 {
    connected_water_seed_capacity_tiles(city, connected_tiles, power_maps)
}

pub(in crate::app) fn connected_water_source_capacity_tiles(
    city: &City,
    connected_tiles: &[bool],
    power_maps: &PowerServiceMaps,
) -> u32 {
    connected_water_source_capacity_tiles_with_inputs(
        city,
        connected_tiles,
        power_maps,
        city.misc_water_table_level().unwrap_or(0),
        city.misc_humidity().unwrap_or(0),
    )
}

fn water_source_level_for_fidelity(city: &City, fidelity: EngineFidelity) -> u32 {
    if fidelity.uses_win95() {
        city.misc_water_table_level().unwrap_or(0)
    } else {
        city.mac_selected_water_height().unwrap_or(0)
    }
}

fn water_source_humidity_for_fidelity(city: &City, fidelity: EngineFidelity) -> u32 {
    if fidelity.uses_win95() {
        u32::from(city.pe_b_weather_humid)
    } else {
        city.mac_weather_humidity().unwrap_or(0)
    }
}

pub(in crate::app) fn connected_water_source_capacity_tiles_with_fidelity(
    city: &City,
    connected_tiles: &[bool],
    power_maps: &PowerServiceMaps,
    fidelity: EngineFidelity,
) -> u32 {
    let sea_level = water_source_level_for_fidelity(city, fidelity);
    let humidity = water_source_humidity_for_fidelity(city, fidelity);
    if fidelity == EngineFidelity::Mac68k {
        return mac_connected_water_source_capacity_tiles(
            city,
            connected_tiles,
            sea_level,
            humidity,
        );
    }
    connected_water_source_capacity_tiles_with_inputs(
        city,
        connected_tiles,
        power_maps,
        sea_level,
        humidity,
    )
}

/// Mac CODE 14 counts source output per reached XBLD tile. In particular,
/// every connected, powered Desalination tile contributes its own centered
/// salt-water scan; the original worker does not require a complete 3x3
/// footprint before counting a surviving `0xFA` tile.
fn mac_connected_water_source_capacity_tiles(
    city: &City,
    connected_tiles: &[bool],
    sea_level: u32,
    humidity: u32,
) -> u32 {
    connected_tiles
        .iter()
        .enumerate()
        .filter(|(index, connected)| {
            **connected
                && city
                    .tile_bits_at(index % CITY_WIDTH, index / CITY_WIDTH)
                    .is_some_and(TileBits::powered)
        })
        .map(|(index, _)| {
            let x = index % CITY_WIDTH;
            let y = index / CITY_WIDTH;
            match city.building_at(x, y) {
                220 => water_pump_output_tiles(city, x, y, sea_level, humidity),
                250 => {
                    centered_tiles(x, y)
                        .filter(|&(tx, ty)| pe_salt_surface_water(city, tx, ty))
                        .count() as u32
                        * 20
                }
                _ => 0,
            }
        })
        .sum()
}

fn connected_water_source_capacity_tiles_with_inputs(
    city: &City,
    connected_tiles: &[bool],
    power_maps: &PowerServiceMaps,
    sea_level: u32,
    precipitation: u32,
) -> u32 {
    let mut source_capacity = 0;
    let mut seen = vec![false; CITY_TILES];
    for y in 0..CITY_HEIGHT {
        for x in 0..CITY_WIDTH {
            let index = y * CITY_WIDTH + x;
            match city.building_at(x, y) {
                220 if connected_tiles[index]
                    && water_pump_has_power_in_capacity_estimate(&power_maps, index) =>
                {
                    source_capacity +=
                        water_pump_output_tiles(city, x, y, sea_level, precipitation);
                }
                250 => {
                    let Some((origin_x, origin_y)) =
                        building_footprint_origin_containing(city, x, y, 250, 3)
                    else {
                        continue;
                    };
                    let origin_index = origin_y * CITY_WIDTH + origin_x;
                    if seen[origin_index] {
                        continue;
                    }
                    seen[origin_index] = true;
                    if complete_connected_building_footprint_at_origin(
                        city,
                        connected_tiles,
                        origin_x,
                        origin_y,
                        250,
                        3,
                    ) && connected_powered_building_footprint_at_origin(
                        city,
                        connected_tiles,
                        power_maps,
                        origin_x,
                        origin_y,
                        250,
                        3,
                    ) {
                        source_capacity += desalination_output_tiles(city, origin_x, origin_y);
                    }
                }
                _ => {}
            }
        }
    }
    source_capacity
}

pub(in crate::app) fn connected_water_storage_capacity_tiles(
    city: &City,
    connected_tiles: &[bool],
) -> u32 {
    let mut capacity = 0;
    let mut seen = vec![false; CITY_TILES];
    for y in 0..CITY_HEIGHT {
        for x in 0..CITY_WIDTH {
            if !complete_connected_building_footprint_at_origin(city, connected_tiles, x, y, 235, 2)
            {
                continue;
            }
            let Some((origin_x, origin_y)) =
                building_footprint_origin_containing(city, x, y, 235, 2)
            else {
                continue;
            };
            let origin_index = origin_y * CITY_WIDTH + origin_x;
            if seen[origin_index] {
                continue;
            }
            seen[origin_index] = true;
            capacity += 400;
        }
    }
    capacity
}

pub(in crate::app) fn connected_filled_water_storage_capacity_tiles(
    city: &City,
    connected_tiles: &[bool],
) -> u32 {
    let mut capacity = 0;
    let mut seen = vec![false; CITY_TILES];
    for y in 0..CITY_HEIGHT {
        for x in 0..CITY_WIDTH {
            if !complete_connected_building_footprint_at_origin(city, connected_tiles, x, y, 235, 2)
                || !building_footprint_has_watered_tile(city, x, y, 2)
            {
                continue;
            }
            let Some((origin_x, origin_y)) =
                building_footprint_origin_containing(city, x, y, 235, 2)
            else {
                continue;
            };
            let origin_index = origin_y * CITY_WIDTH + origin_x;
            if seen[origin_index] {
                continue;
            }
            seen[origin_index] = true;
            capacity += 400;
        }
    }
    capacity
}

pub(in crate::app) fn connected_water_seed_capacity_tiles(
    city: &City,
    connected_tiles: &[bool],
    power_maps: &PowerServiceMaps,
) -> u32 {
    let source_capacity = connected_water_source_capacity_tiles(city, connected_tiles, power_maps);
    if source_capacity > 0 {
        source_capacity + connected_water_storage_capacity_tiles(city, connected_tiles)
    } else {
        connected_filled_water_storage_capacity_tiles(city, connected_tiles)
    }
}

fn connected_water_seed_capacity_tiles_with_fidelity(
    city: &City,
    connected_tiles: &[bool],
    fidelity: EngineFidelity,
) -> u32 {
    if fidelity.uses_win95() {
        pe_connected_water_source_capacity_tiles(city, connected_tiles).wrapping_add(
            connected_water_carried_tower_capacity_tiles(city, connected_tiles),
        )
    } else {
        let source_capacity = connected_water_source_capacity_tiles_with_fidelity(
            city,
            connected_tiles,
            &power_service_maps(city),
            fidelity,
        );
        if source_capacity > 0 {
            source_capacity + connected_water_storage_capacity_tiles(city, connected_tiles)
        } else {
            connected_filled_water_storage_capacity_tiles(city, connected_tiles)
        }
    }
}

/// PE's first water pass counts source output only at powered source cells in
/// the saved-XBIT component. Pumps contribute per `0xDC` cell; desalination
/// contributes per powered `0xFA` cell, matching the worker's per-dequeued-
/// tile switch rather than requiring a complete 3×3 footprint.
pub(in crate::app) fn pe_connected_water_source_capacity_tiles(
    city: &City,
    connected_tiles: &[bool],
) -> u32 {
    let sea_level = city.misc_water_table_level().unwrap_or(0);
    let precipitation = u32::from(city.pe_b_weather_humid);
    connected_tiles
        .iter()
        .enumerate()
        .filter(|(_, connected)| **connected)
        .map(|(index, _)| {
            let x = index % CITY_WIDTH;
            let y = index / CITY_WIDTH;
            if !city.tile_bits_at(x, y).is_some_and(TileBits::powered) {
                return 0;
            }
            match city.building_at(x, y) {
                220 => water_pump_output_tiles(city, x, y, sea_level, precipitation),
                250 => {
                    centered_tiles(x, y)
                        .filter(|&(tx, ty)| pe_salt_surface_water(city, tx, ty))
                        .count() as u32
                        * 20
                }
                _ => 0,
            }
        })
        .fold(0u32, u32::wrapping_add)
}

/// PE preserves Water Tower served bits across the wrapper clear and converts
/// each connected, previously served `XBLD 0xEB` cell into 100 units of carry.
/// It is tile-based because the worker's switch runs once per map cell, without
/// requiring a complete 2×2 footprint.
pub(in crate::app) fn connected_water_carried_tower_capacity_tiles(
    city: &City,
    connected_tiles: &[bool],
) -> u32 {
    connected_tiles
        .iter()
        .enumerate()
        .filter(|(index, connected)| {
            **connected
                && city.building_at(index % CITY_WIDTH, index / CITY_WIDTH) == 0xeb
                && city
                    .tile_bits_at(index % CITY_WIDTH, index / CITY_WIDTH)
                    .is_some_and(TileBits::watered)
        })
        .count() as u32
        * 100
}

/// PE rounds the available tower reserve to the nearest 100-unit cell before
/// its second pass (`0x464EA0..0x464EB0`), then decrements one per `0xEB` tile.
pub(in crate::app) fn pe_water_tower_refill_budget(available_units: u32, tower_tiles: u32) -> u32 {
    (tower_tiles
        .saturating_mul(100)
        .min(available_units)
        .saturating_add(50))
        / 100
}

pub(in crate::app) fn connected_powered_building_footprint_at_origin(
    city: &City,
    connected_tiles: &[bool],
    power_maps: &PowerServiceMaps,
    origin_x: usize,
    origin_y: usize,
    building: u8,
    size: usize,
) -> bool {
    // Win95 water refresh scans source buildings through XBIT power state; any
    // powered tile in a complete footprint can seed that source.
    complete_connected_building_footprint_at_origin(
        city,
        connected_tiles,
        origin_x,
        origin_y,
        building,
        size,
    ) && (origin_y..origin_y + size)
        .any(|y| (origin_x..origin_x + size).any(|x| power_maps.powered_tiles[y * CITY_WIDTH + x]))
}

pub(in crate::app) fn powered_building_footprint_at_origin(
    city: &City,
    power_maps: &PowerServiceMaps,
    origin_x: usize,
    origin_y: usize,
    building: u8,
    size: usize,
) -> bool {
    complete_building_footprint_at_origin(city, origin_x, origin_y, building, size)
        && (origin_y..origin_y + size).any(|y| {
            (origin_x..origin_x + size).any(|x| power_maps.powered_tiles[y * CITY_WIDTH + x])
        })
}

pub(in crate::app) fn building_footprint_has_watered_tile(
    city: &City,
    origin_x: usize,
    origin_y: usize,
    size: usize,
) -> bool {
    origin_x + size <= CITY_WIDTH
        && origin_y + size <= CITY_HEIGHT
        && (origin_y..origin_y + size).any(|y| {
            (origin_x..origin_x + size)
                .any(|x| city.tile_bits_at(x, y).is_some_and(|bits| bits.watered()))
        })
}

pub(in crate::app) fn current_watered_water_tower_tile(city: &City, x: usize, y: usize) -> bool {
    // Win95 FUN_00464550 clears XBIT water for most tiles, but skips building 0xeb
    // so filled water-tower tiles can keep serving after source water is lost.
    city.building_at(x, y) == 235 && city.tile_bits_at(x, y).is_some_and(|bits| bits.watered())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::app) struct WaterSupplySummary {
    pub(in crate::app) capacity: u32,
    pub(in crate::app) demand: u32,
}

impl WaterSupplySummary {
    pub(in crate::app) fn shortage(self) -> u32 {
        self.demand.saturating_sub(self.capacity)
    }

    pub(in crate::app) fn has_shortage(self) -> bool {
        self.shortage() > 0
    }
}

pub(in crate::app) fn water_supply_summary(
    city: &City,
    sea_level: u32,
    precipitation: u32,
) -> WaterSupplySummary {
    WaterSupplySummary {
        capacity: water_supply_capacity_tiles(city, sea_level, precipitation),
        demand: water_demand_tiles(city),
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::app) struct WaterServiceMaps {
    pub(in crate::app) pipe_tiles: Vec<bool>,
    pub(in crate::app) watered_tiles: Vec<bool>,
    pub(in crate::app) pe_worker_capacity_units: u32,
    pub(in crate::app) pe_worker_served_cells: u32,
    pub(in crate::app) pe_worker_pending_tiles: Vec<bool>,
    pub(in crate::app) pe_worker_contract_complete: bool,
    pub(in crate::app) mac_worker_capacity_units: u32,
    pub(in crate::app) mac_worker_served_cells: u32,
    pub(in crate::app) mac_worker_contract_complete: bool,
}

impl Default for WaterServiceMaps {
    fn default() -> Self {
        Self {
            pipe_tiles: vec![false; CITY_TILES],
            watered_tiles: vec![false; CITY_TILES],
            pe_worker_capacity_units: 0,
            pe_worker_served_cells: 0,
            pe_worker_pending_tiles: Vec::new(),
            pe_worker_contract_complete: false,
            mac_worker_capacity_units: 0,
            mac_worker_served_cells: 0,
            mac_worker_contract_complete: false,
        }
    }
}

impl WaterServiceMaps {
    pub(in crate::app) fn watered_consumers(&self, city: &City) -> u32 {
        let mut count = 0;
        for y in 0..CITY_HEIGHT {
            for x in 0..CITY_WIDTH {
                let index = y * CITY_WIDTH + x;
                if self.watered_tiles[index] && water_consuming_tile(city, x, y) {
                    count += 1;
                }
            }
        }
        count
    }

    /// PE 0x4648a8 publishes the capped worker count over raw source capacity.
    pub(in crate::app) fn pe_worker_utilization_percent(&self) -> u32 {
        if self.pe_worker_capacity_units == 0 {
            100
        } else {
            self.pe_worker_served_cells.wrapping_mul(100) / self.pe_worker_capacity_units
        }
    }

    /// Mac CODE 14 `export_209` publishes capped generic cells divided by raw
    /// source units. Use this only when every powered, conductive source was
    /// handled through an exact saved-XBIT component.
    pub(in crate::app) fn mac_worker_utilization_percent(&self) -> Option<u32> {
        if !self.mac_worker_contract_complete {
            return None;
        }
        if self.mac_worker_capacity_units == 0 {
            return Some(100);
        }
        Some(
            (u64::from(self.mac_worker_served_cells) * 100
                / u64::from(self.mac_worker_capacity_units)) as u32,
        )
    }

    pub(in crate::app) fn status_text_at(&self, city: &City, x: usize, y: usize) -> &'static str {
        let index = y * CITY_WIDTH + x;
        if !self.pipe_tiles[index] && !water_consuming_tile(city, x, y) {
            ""
        } else if self.watered_tiles[index] {
            " water=on"
        } else {
            " water=off"
        }
    }
}

pub(in crate::app) fn water_service_maps(city: &City) -> WaterServiceMaps {
    water_service_maps_with_entry_seed(city, pe_power_recalc_seed(city))
}

/// Kept for call-site compatibility. Both recovered water workers use their
/// saved XBIT service roots rather than sampling plant power from this seed.
pub(in crate::app) fn water_service_maps_with_entry_seed(
    city: &City,
    entry_seed: u32,
) -> WaterServiceMaps {
    water_service_maps_with_entry_seed_and_fidelity(city, entry_seed, EngineFidelity::from_env())
}

pub(in crate::app) fn water_service_maps_with_entry_seed_and_fidelity(
    city: &City,
    _entry_seed: u32,
    fidelity: EngineFidelity,
) -> WaterServiceMaps {
    let (pipe_tiles, _) = water_pipe_connection_maps(city);
    match fidelity {
        EngineFidelity::Win95 => pe_water_service_maps_from_saved_xbit(city, pipe_tiles),
        EngineFidelity::Mac68k => mac_water_service_maps_from_saved_xbit(city, pipe_tiles),
    }
}

/// Mac export_209 compares the raw served count to 2,000 per complete
/// four-tile treatment plant and sets `fffffe12` when served <= threshold.
pub(in crate::app) fn mac_water_treatment_gate(served: u32, treatment_tiles: u16) -> i16 {
    i16::from(served <= u32::from(treatment_tiles >> 2) * 2_000)
}

/// Mac CODE 14's water worker traverses the persisted XBIT conductive plane
/// directly. Keep that simulation contract independent of CityGame's inferred
/// underground-pipe geometry, which is useful for display and editing but can
/// disagree with saved runtime membership in imported or edited cities.
fn mac_water_service_maps_from_saved_xbit(city: &City, pipe_tiles: Vec<bool>) -> WaterServiceMaps {
    let mut work = vec![false; CITY_TILES];
    let mut watered_tiles = vec![false; CITY_TILES];
    for index in 0..CITY_TILES {
        let x = index % CITY_WIDTH;
        let y = index / CITY_WIDTH;
        // export_209 clears XBIT 0x08 and preserves XBIT 0x10 only on towers.
        watered_tiles[index] = city.building_at(x, y) == 0xeb
            && city.tile_bits_at(x, y).is_some_and(TileBits::watered);
    }

    let mut roots: Vec<_> = (0..CITY_TILES)
        .filter_map(|index| {
            let x = index % CITY_WIDTH;
            let y = index / CITY_WIDTH;
            (matches!(city.building_at(x, y), 0xdc | 0xfa)
                && city.tile_bits_at(x, y).is_some_and(TileBits::powered))
            .then_some((x, y))
        })
        .collect();
    roots.sort_by_key(|&(x, y)| water_source_scan_key(city, EngineFidelity::Mac68k, x, y));

    let mut capacity_total = 0u32;
    let mut served_total = 0u32;
    for root in roots {
        let root_index = root.1 * CITY_WIDTH + root.0;
        if watered_tiles[root_index] {
            continue;
        }
        let order = saved_xbit_water_first_pass(city, root, &mut work);
        if order.is_empty() {
            continue;
        }
        let (component_mask, tower_carry) =
            water_worker_members_and_tower_carry(city, &order, &mut watered_tiles);
        let capacity = mac_connected_water_source_capacity_tiles(
            city,
            &component_mask,
            city.mac_selected_water_height().unwrap_or(0),
            city.mac_weather_humidity().unwrap_or(0),
        )
        .saturating_add(tower_carry);
        let mut generic_budget = order
            .iter()
            .filter(|&&(x, y)| pe_water_generic_counter_tile(city.building_at(x, y)))
            .count()
            .min(capacity as usize) as u32;
        let generic_budget_initial = generic_budget;
        let tower_tiles = order
            .iter()
            .filter(|&&(x, y)| city.building_at(x, y) == 0xeb)
            .count() as u32;
        let tower_capacity = capacity
            .saturating_sub(generic_budget)
            .min(tower_tiles.saturating_mul(100));
        let mut tower_refill_budget = tower_capacity.saturating_add(50) / 100;
        capacity_total = capacity_total.saturating_add(capacity);
        served_total = served_total.saturating_add(generic_budget_initial);

        // The binary performs a second queue walk over first-pass work marks.
        // Re-run the same bounded ring because overflow can change the subset
        // that actually receives the served bit.
        let service_order = saved_xbit_water_second_pass(root, &mut work);
        for &(x, y) in &service_order {
            let index = y * CITY_WIDTH + x;
            let building = city.building_at(x, y);
            let powered = city.tile_bits_at(x, y).is_some_and(TileBits::powered);
            let watered = if matches!(building, 0xdc | 0xf4 | 0xfa) {
                powered
            } else if building == 0xeb {
                let refill = powered && tower_refill_budget > 0;
                if refill {
                    tower_refill_budget = tower_refill_budget.saturating_sub(1);
                }
                refill
            } else if pe_water_generic_served_tile(building) {
                let refill = generic_budget > 0;
                if building >= 0x70 {
                    generic_budget = generic_budget.saturating_sub(1);
                }
                refill
            } else {
                false
            };
            watered_tiles[index] |= watered;
        }
    }

    WaterServiceMaps {
        pipe_tiles,
        watered_tiles,
        pe_worker_capacity_units: 0,
        pe_worker_served_cells: 0,
        pe_worker_pending_tiles: Vec::new(),
        pe_worker_contract_complete: false,
        mac_worker_capacity_units: capacity_total,
        mac_worker_served_cells: served_total,
        mac_worker_contract_complete: true,
    }
}

/// Both workers consume a tower marker from the live plane during the
/// first pass. A later root sees the result left by the preceding worker.
fn water_worker_members_and_tower_carry(
    city: &City,
    visits: &[(usize, usize)],
    watered: &mut [bool],
) -> (Vec<bool>, u32) {
    let mut members = vec![false; CITY_TILES];
    let mut carry = 0;
    for &(x, y) in visits {
        let index = y * CITY_WIDTH + x;
        members[index] = true;
        if city.building_at(x, y) == 0xeb && watered[index] {
            carry += 100;
            watered[index] = false;
        }
    }
    (members, carry)
}

fn saved_xbit_water_neighbors(x: usize, y: usize) -> [(usize, usize); 4] {
    [
        (x, y.wrapping_sub(1)),
        (x.wrapping_sub(1), y),
        (x, y + 1),
        (x + 1, y),
    ]
}

/// PE and Mac water workers share dequeue-marked XBIT 0x20 membership and a 511-pending
/// drop-oldest ring. The transient 0x08 mark is represented by `work` across
/// roots, so a source reached by an earlier worker is not traversed again.
pub(in crate::app) fn saved_xbit_water_first_pass(
    city: &City,
    root: (usize, usize),
    work: &mut [bool],
) -> Vec<(usize, usize)> {
    let mut queue = VecDeque::from([root]);
    let mut order = Vec::new();
    while let Some((x, y)) = queue.pop_front() {
        let index = y * CITY_WIDTH + x;
        if work[index] || !city.tile_bits_at(x, y).is_some_and(TileBits::piped) {
            continue;
        }
        work[index] = true;
        order.push((x, y));
        for (nx, ny) in saved_xbit_water_neighbors(x, y) {
            if nx >= CITY_WIDTH || ny >= CITY_HEIGHT {
                continue;
            }
            let next_index = ny * CITY_WIDTH + nx;
            // Native enqueue tests the transient visited bit, not conductive
            // membership; the worker rejects non-0x20 cells when dequeued.
            // Keep those queue entries because they affect the bounded ring.
            if !work[next_index] {
                if queue.len() >= 0x1ff {
                    let _ = queue.pop_front();
                }
                queue.push_back((nx, ny));
            }
        }
    }
    order
}

pub(in crate::app) fn saved_xbit_water_second_pass(
    root: (usize, usize),
    work: &mut [bool],
) -> Vec<(usize, usize)> {
    let mut queue = VecDeque::from([root]);
    let mut order = Vec::new();
    while let Some((x, y)) = queue.pop_front() {
        let index = y * CITY_WIDTH + x;
        if !work[index] {
            continue;
        }
        work[index] = false;
        order.push((x, y));
        for (nx, ny) in saved_xbit_water_neighbors(x, y) {
            if nx >= CITY_WIDTH || ny >= CITY_HEIGHT {
                continue;
            }
            let next_index = ny * CITY_WIDTH + nx;
            if work[next_index] {
                if queue.len() >= 0x1ff {
                    let _ = queue.pop_front();
                }
                queue.push_back((nx, ny));
            }
        }
    }
    order
}

/// PE `FUN_00464550`/`FUN_004649E0` water-service allocation follows saved
/// XBIT `0x20` membership, not CityGame's inferred underground geometry.
/// Keep the geometry map for display and editing; use the bounded native
/// queue and saved plane for simulation, including mismatched and large maps.
pub(in crate::app) fn pe_water_service_maps_from_saved_xbit(
    city: &City,
    pipe_tiles: Vec<bool>,
) -> WaterServiceMaps {
    let dim = CITY_WIDTH.min(CITY_HEIGHT).min(0x80);
    let mut work = vec![false; CITY_TILES];
    let mut watered_tiles = vec![false; CITY_TILES];
    for index in 0..CITY_TILES {
        let x = index % CITY_WIDTH;
        let y = index / CITY_WIDTH;
        watered_tiles[index] = city.building_at(x, y) == 0xeb
            && city.tile_bits_at(x, y).is_some_and(TileBits::watered);
    }

    // PE's dispatch table has four entries. Other saved WORDs skip the scan.
    let scan_tiles = if city.pe_w_city_rotation < 4 {
        dim * dim
    } else {
        0
    };
    let mut roots: Vec<_> = (0..scan_tiles)
        .filter_map(|index| {
            let x = index % dim;
            let y = index / dim;
            (matches!(city.building_at(x, y), 0xdc | 0xfa)
                && city.tile_bits_at(x, y).is_some_and(TileBits::powered))
            .then_some((x, y))
        })
        .collect();
    roots.sort_by_key(|&(x, y)| water_source_scan_key(city, EngineFidelity::Win95, x, y));

    let mut capacity_total = 0_u32;
    let mut served_total = 0_u32;
    for root in roots {
        let root_index = root.1 * CITY_WIDTH + root.0;
        if watered_tiles[root_index]
            || !city
                .tile_bits_at(root.0, root.1)
                .is_some_and(TileBits::piped)
        {
            continue;
        }

        let first_pass = saved_xbit_water_first_pass(city, root, &mut work);
        if first_pass.is_empty() {
            continue;
        }
        let (member_tiles, tower_capacity) =
            water_worker_members_and_tower_carry(city, &first_pass, &mut watered_tiles);
        let source_capacity = pe_connected_water_source_capacity_tiles(city, &member_tiles);
        let capacity = source_capacity.wrapping_add(tower_capacity);
        let mut generic_budget = first_pass
            .iter()
            .filter(|&&(x, y)| pe_water_generic_counter_tile(city.building_at(x, y)))
            .count()
            .min(capacity as usize) as u32;
        let generic_budget_initial = generic_budget;
        let tower_tiles = first_pass
            .iter()
            .filter(|&&(x, y)| city.building_at(x, y) == 0xeb)
            .count() as u32;
        let tower_budget =
            pe_water_tower_refill_budget(capacity.saturating_sub(generic_budget), tower_tiles);
        let mut tower_budget = tower_budget;
        capacity_total = capacity_total.wrapping_add(capacity);
        served_total = served_total.wrapping_add(generic_budget_initial);

        // Pass two revisits only the XBIT 0x08 marks produced by pass one.
        // It clears each mark on dequeue, then applies the PE type-specific
        // served-bit arm in the same N/W/E/S bounded-ring order.
        let service_order = saved_xbit_water_second_pass(root, &mut work);
        for (x, y) in service_order {
            let index = y * CITY_WIDTH + x;
            let building = city.building_at(x, y);
            let powered = city.tile_bits_at(x, y).is_some_and(TileBits::powered);
            let watered = if matches!(building, 0xdc | 0xf4 | 0xfa) {
                powered
            } else if building == 0xeb {
                let served = powered && tower_budget > 0;
                if served {
                    tower_budget = tower_budget.saturating_sub(1);
                }
                served
            } else if pe_water_generic_served_tile(building) {
                let served = generic_budget > 0;
                if building >= 0x70 {
                    generic_budget = generic_budget.saturating_sub(1);
                }
                served
            } else {
                false
            };
            watered_tiles[index] |= watered;
        }
    }

    WaterServiceMaps {
        pipe_tiles,
        watered_tiles,
        pe_worker_capacity_units: capacity_total,
        pe_worker_served_cells: served_total,
        pe_worker_pending_tiles: work,
        pe_worker_contract_complete: true,
        mac_worker_capacity_units: 0,
        mac_worker_served_cells: 0,
        mac_worker_contract_complete: false,
    }
}

/// PE water service starts at the first powered pump/desalination cell in the
/// rotation-selected scan, then assigns consumers in the worker's FIFO order.
/// Use it only when the saved XBIT membership flood exactly matches the
/// geometry-derived component; other networks retain the established fallback.
pub(in crate::app) fn pe_water_allocation_order(
    city: &City,
    component: &[(usize, usize)],
) -> Option<Vec<(usize, usize)>> {
    water_allocation_order_for_sources(city, component, &[220, 250], EngineFidelity::Win95)
}

/// Match the source scan's coordinate nesting without conflating the two
/// editions' row-pointer layouts. PE scans its own `wRotation` axes; Mac's
/// CODE 8 map worker establishes X as the row-table index and Y as the byte
/// offset, while the CODE 14 water wrapper uses D7=X and D6=Y. The Mac
/// selector is persisted in MISC (`saved_rotation`), independently of the
/// display-only compass. The Mac loader expands XBLD/XBIT bytes into X-major
/// rows, matching CityGame's `x * CITY_HEIGHT + y` plane index, so these keys
/// address native X/Y cells directly when the chunk is present. The allocator
/// does not clear the later XBIT buffer; absent-chunk behavior and original-Mac
/// output replay remain separate validation gaps.
fn water_source_scan_key(
    city: &City,
    fidelity: EngineFidelity,
    x: usize,
    y: usize,
) -> (usize, usize) {
    // Both executables index the row-pointer table by the first coordinate.
    let rotation = match fidelity {
        EngineFidelity::Win95 => city.pe_w_city_rotation,
        EngineFidelity::Mac68k => u16::from(city.saved_rotation),
    };
    match rotation & 3 {
        0 => (y, x),
        1 => (x, CITY_HEIGHT - 1 - y),
        2 => (CITY_HEIGHT - 1 - y, CITY_WIDTH - 1 - x),
        _ => (CITY_WIDTH - 1 - x, y),
    }
}

fn water_allocation_order_for_sources(
    city: &City,
    component: &[(usize, usize)],
    source_buildings: &[u8],
    fidelity: EngineFidelity,
) -> Option<Vec<(usize, usize)>> {
    if component
        .iter()
        .any(|&(x, y)| !city.tile_bits_at(x, y).is_some_and(TileBits::piped))
    {
        return None;
    }

    let mut sources: Vec<_> = component
        .iter()
        .copied()
        .filter(|&(x, y)| {
            if !city.tile_bits_at(x, y).is_some_and(TileBits::powered) {
                return false;
            }
            // FUN_00464550 enters FUN_004649E0 from this exact source cell
            // only when its saved XBIT 0x40 is set. Do not substitute the
            // geometry/capacity service map here: the PE worker has already
            // published the authoritative source-connected bit.
            source_buildings.contains(&city.building_at(x, y))
        })
        .collect();
    if sources.is_empty() {
        return None;
    }
    sources.sort_by_key(|&(x, y)| water_source_scan_key(city, fidelity, x, y));
    let root = sources[0];

    let mut in_component = vec![false; CITY_TILES];
    for &(x, y) in component {
        in_component[y * CITY_WIDTH + x] = true;
    }

    // The recovered worker walks XBIT membership directly, without
    // CityGame's directional pipe-connection predicate. Only use its order
    // when saved membership is exactly the geometry component counted above.
    let mut membership_component = vec![false; CITY_TILES];
    let mut membership_queue = VecDeque::from([root]);
    membership_component[root.1 * CITY_WIDTH + root.0] = true;
    while let Some((x, y)) = membership_queue.pop_front() {
        for (nx, ny) in saved_xbit_water_neighbors(x, y) {
            if nx >= CITY_WIDTH || ny >= CITY_HEIGHT {
                continue;
            }
            let index = ny * CITY_WIDTH + nx;
            if !membership_component[index]
                && city.tile_bits_at(nx, ny).is_some_and(TileBits::piped)
            {
                membership_component[index] = true;
                membership_queue.push_back((nx, ny));
            }
        }
    }
    if membership_component != in_component {
        return None;
    }

    let mut visited = vec![false; CITY_TILES];
    let mut queue = VecDeque::new();
    queue.push_back(root);
    let mut order = Vec::with_capacity(component.len());

    // Both recovered workers mark XBIT 0x08 at dequeue.
    // Pending duplicates are possible; the native 512-slot ring holds 511
    // records and drops its oldest item when a write collides with the read.
    // The binary's 5,001-dequeue callback does not stop this walk: it calls
    // relocated CODE 6 export 130, which refreshes timed palettes, resets the
    // counter, then continues until the queue is empty. Palette parity is a
    // rendering concern and does not change this water-allocation contract.
    while !queue.is_empty() {
        let Some((x, y)) = queue.pop_front() else {
            break;
        };
        let index = y * CITY_WIDTH + x;
        if visited[index] {
            continue;
        }
        visited[index] = true;
        order.push((x, y));
        for (nx, ny) in saved_xbit_water_neighbors(x, y) {
            if nx >= CITY_WIDTH || ny >= CITY_HEIGHT {
                continue;
            }
            let index = ny * CITY_WIDTH + nx;
            if in_component[index] && !visited[index] {
                if queue.len() >= 0x1ff {
                    let _ = queue.pop_front();
                }
                queue.push_back((nx, ny));
            }
        }
    }

    (order.len() == component.len()).then_some(order)
}

/// Mac `export_209` admits Pump/Desalination source tiles in one of four
/// orientation-selected scan orders, then `FUN_00026106` services the selected
/// source through its -Y/-X/+Y/+X FIFO. One worker flood processes each reached
/// source tile, so scan order chooses the root across multiple sources. Water
/// Towers participate in this component's carry/refill counters. Water
/// Treatment uses its separate saved-powered served-marker arm and does not
/// debit D3. Mismatched geometry remains on the fallback path.
pub(in crate::app) fn mac_water_allocation_order(
    city: &City,
    component: &[(usize, usize)],
) -> Option<Vec<(usize, usize)>> {
    if !component.iter().any(|&(x, y)| {
        matches!(city.building_at(x, y), 220 | 250)
            && city.tile_bits_at(x, y).is_some_and(TileBits::piped)
            && city.tile_bits_at(x, y).is_some_and(TileBits::powered)
    }) {
        return None;
    }
    // This chooses the orientation-first source root and verifies that the
    // saved XBIT 0x20 flood exactly matches the geometry component.
    water_allocation_order_for_sources(city, component, &[220, 250], EngineFidelity::Mac68k)
}

/// PE increments/decrements the generic water-service counter only for
/// developed XBLD values at or above `0x70`, except the four explicit worker
/// arms for Pump, Water Tower, Water Treatment, and Desalination. The indexed
/// `0xDC..=0xFA` dispatch table sends every other ID in that range to the
/// default generic arm. This is deliberately separate from CityGame's
/// zone-demand predicate, which also counts undeveloped zoned cells for its
/// geometry-based service model.
pub(in crate::app) fn pe_water_generic_counter_tile(building: u8) -> bool {
    building >= 0x70 && !matches!(building, 0xdc | 0xeb | 0xf4 | 0xfa)
}

/// In pass two the default water arm stamps bit `0x10` on any non-special
/// cell while generic budget remains. The four explicit special arms are
/// `0xDC`, `0xEB`, `0xF4`, and `0xFA`; other IDs, including the default slots
/// inside `0xDC..=0xFA`, use the generic served arm. Only XBLD `>=0x70`
/// consumes a counter; lower transport/empty cells are marked without a debit.
pub(in crate::app) fn pe_water_generic_served_tile(building: u8) -> bool {
    !matches!(building, 0xdc | 0xeb | 0xf4 | 0xfa)
}

pub(in crate::app) fn water_component_tiles(
    city: &City,
    pipe_tiles: &[bool],
    visited: &mut [bool],
    start_x: usize,
    start_y: usize,
) -> Vec<(usize, usize)> {
    let mut component = Vec::new();
    let mut pipe_queue = VecDeque::new();
    let start_index = start_y * CITY_WIDTH + start_x;
    visited[start_index] = true;
    component.push((start_x, start_y));
    pe_network_queue_push(&mut pipe_queue, (start_x, start_y));

    while let Some((x, y)) = pipe_queue.pop_front() {
        for (side, nx, ny) in directional_neighbor_tiles(x, y) {
            let index = ny * CITY_WIDTH + nx;
            if visited[index] || !pipe_tiles[index] {
                continue;
            }
            if water_pipe_tile_connects_to(city, x, y, side)
                && water_pipe_tile_connects_to(city, nx, ny, opposite_network_side(side))
            {
                visited[index] = true;
                component.push((nx, ny));
                pe_network_queue_push(&mut pipe_queue, (nx, ny));
            }
        }
    }

    let pipe_component: Vec<_> = component.clone();
    let mut zone_queue = VecDeque::new();
    for &(px, py) in &pipe_component {
        if !pipe_tiles[py * CITY_WIDTH + px] {
            continue;
        }
        for (side, nx, ny) in directional_neighbor_tiles(px, py) {
            let index = ny * CITY_WIDTH + nx;
            if visited[index] || !zone_serviceable_tile(city, nx, ny) {
                continue;
            }
            if water_pipe_tile_connects_to(city, px, py, side)
                && water_pipe_tile_connects_to(city, nx, ny, opposite_network_side(side))
            {
                visited[index] = true;
                component.push((nx, ny));
                pe_network_queue_push(&mut zone_queue, (nx, ny));
            }
        }
    }

    while let Some((x, y)) = zone_queue.pop_front() {
        for (_, nx, ny) in directional_neighbor_tiles(x, y) {
            let index = ny * CITY_WIDTH + nx;
            if visited[index] || !zone_serviceable_tile(city, nx, ny) {
                continue;
            }
            visited[index] = true;
            component.push((nx, ny));
            pe_network_queue_push(&mut zone_queue, (nx, ny));
        }
    }

    component
}

pub(in crate::app) fn tile_water_status_text(city: &City, x: usize, y: usize) -> &'static str {
    tile_water_status_text_with_entry_seed(city, x, y, pe_power_recalc_seed(city))
}

pub(in crate::app) fn tile_water_status_text_with_entry_seed(
    city: &City,
    x: usize,
    y: usize,
    entry_seed: u32,
) -> &'static str {
    water_service_maps_with_entry_seed(city, entry_seed).status_text_at(city, x, y)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::app) struct WaterServiceSummary {
    pub(in crate::app) served: u32,
    pub(in crate::app) demand: u32,
}

impl WaterServiceSummary {
    pub(in crate::app) fn shortage(self) -> u32 {
        self.demand.saturating_sub(self.served)
    }
}

pub(in crate::app) fn water_service_summary(city: &City) -> WaterServiceSummary {
    water_service_summary_with_entry_seed(city, pe_power_recalc_seed(city))
}

pub(in crate::app) fn water_service_summary_with_entry_seed(
    city: &City,
    entry_seed: u32,
) -> WaterServiceSummary {
    let maps = water_service_maps_with_entry_seed(city, entry_seed);
    WaterServiceSummary {
        served: maps.watered_consumers(city),
        demand: water_demand_tiles(city),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::app) struct WaterTreatmentSummary {
    pub(in crate::app) plants: u32,
    pub(in crate::app) required_plants: u32,
}

impl WaterTreatmentSummary {
    pub(in crate::app) fn missing_plants(self) -> u32 {
        self.required_plants.saturating_sub(self.plants)
    }

    pub(in crate::app) fn has_bonus(self) -> bool {
        self.missing_plants() == 0
    }
}

pub(in crate::app) fn water_treatment_summary(city: &City) -> WaterTreatmentSummary {
    const WATER_TREATMENT_TILE_INTERVAL: u32 = 2_000;
    let city_tiles = CITY_TILES as u32;
    WaterTreatmentSummary {
        plants: water_treatment_plant_count(city),
        required_plants: city_tiles.div_ceil(WATER_TREATMENT_TILE_INTERVAL),
    }
}

pub(in crate::app) fn water_treatment_plant_count(city: &City) -> u32 {
    let mut count = 0;
    let mut seen = vec![false; CITY_TILES];
    for y in 0..CITY_HEIGHT {
        for x in 0..CITY_WIDTH {
            let Some((origin_x, origin_y)) =
                building_footprint_origin_containing(city, x, y, 244, 2)
            else {
                continue;
            };
            let origin_index = origin_y * CITY_WIDTH + origin_x;
            if seen[origin_index] {
                continue;
            }
            seen[origin_index] = true;
            count += 1;
        }
    }
    count
}

pub(in crate::app) const SERVICE_CAPACITY_WARNING_PERCENT: u32 = 98;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::app) struct ServiceStatus {
    pub(in crate::app) water: WaterServiceSummary,
    pub(in crate::app) power: PowerServiceSummary,
    pub(in crate::app) treatment: WaterTreatmentSummary,
    pub(in crate::app) water_capacity_warning: bool,
    pub(in crate::app) power_capacity_warning: bool,
}

impl ServiceStatus {
    pub(in crate::app) fn for_city(city: &City) -> Self {
        Self::for_city_with_entry_seed(city, pe_power_recalc_seed(city))
    }

    pub(in crate::app) fn for_city_with_entry_seed(city: &City, entry_seed: u32) -> Self {
        let water = water_service_summary_with_entry_seed(city, entry_seed);
        let power = power_service_summary_with_entry_seed(city, entry_seed);
        let treatment = water_treatment_summary(city);
        let water_capacity_warning =
            water_capacity_warning_for_city_with_entry_seed(city, entry_seed);
        let power_capacity_warning =
            power_capacity_warning_for_city_with_entry_seed(city, entry_seed);
        Self {
            water,
            power,
            treatment,
            water_capacity_warning,
            power_capacity_warning,
        }
    }

    pub(in crate::app) fn water_shortage(self) -> u32 {
        self.water.shortage()
    }

    pub(in crate::app) fn power_shortage(self) -> u32 {
        self.power.shortage()
    }

    pub(in crate::app) fn treatment_missing(self) -> u32 {
        // SC2K-docs describes treatment as a city-wide bonus: one plant per 2,000 tiles.
        // Gate the warning on demand existing, not on whether current pipe capacity reaches it.
        if self.water.demand > 0 {
            self.treatment.missing_plants()
        } else {
            0
        }
    }

    pub(in crate::app) fn has_water_capacity_warning(self) -> bool {
        self.water_shortage() == 0 && self.water_capacity_warning
    }

    pub(in crate::app) fn has_power_capacity_warning(self) -> bool {
        self.power_shortage() == 0 && self.power_capacity_warning
    }

    pub(in crate::app) fn has_warnings(self) -> bool {
        self.water_shortage() > 0
            || self.has_water_capacity_warning()
            || self.power_shortage() > 0
            || self.has_power_capacity_warning()
            || self.treatment_missing() > 0
    }

    pub(in crate::app) fn warning_parts(self) -> Vec<String> {
        let mut parts = Vec::new();
        let water_shortage = self.water_shortage();
        let power_shortage = self.power_shortage();
        let treatment_missing = self.treatment_missing();
        if water_shortage > 0 {
            parts.push(format!("Water Shortage {water_shortage}"));
        } else if self.has_water_capacity_warning() {
            parts.push(format!(
                "Water Capacity {SERVICE_CAPACITY_WARNING_PERCENT}%"
            ));
        }
        if power_shortage > 0 {
            parts.push(format!("Power Shortage {power_shortage}"));
        } else if self.has_power_capacity_warning() {
            parts.push(format!(
                "Power Capacity {SERVICE_CAPACITY_WARNING_PERCENT}%"
            ));
        }
        if treatment_missing > 0 {
            parts.push(format!("Treatment Missing {treatment_missing}"));
        }
        parts
    }

    pub(in crate::app) fn status_line_suffix(self) -> String {
        if !self.has_warnings() {
            return String::new();
        }
        let parts = self.warning_parts();
        format!("  {}", parts.join("  "))
    }
}

pub(in crate::app) fn service_capacity_at_warning_threshold(capacity: u32, demand: u32) -> bool {
    demand > 0
        && capacity > 0
        && demand <= capacity
        && u64::from(demand) * 100
            >= u64::from(capacity) * u64::from(SERVICE_CAPACITY_WARNING_PERCENT)
}

pub(in crate::app) fn water_capacity_warning_for_city(city: &City) -> bool {
    water_capacity_warning_for_city_with_entry_seed(city, pe_power_recalc_seed(city))
}

pub(in crate::app) fn water_capacity_warning_for_city_with_entry_seed(
    city: &City,
    entry_seed: u32,
) -> bool {
    let (pipe_tiles, _) = water_pipe_connection_maps(city);
    let power_maps = power_service_maps_with_entry_seed(city, entry_seed);
    let mut visited = vec![false; CITY_TILES];

    for y in 0..CITY_HEIGHT {
        for x in 0..CITY_WIDTH {
            let start_index = y * CITY_WIDTH + x;
            if visited[start_index] || !pipe_tiles[start_index] {
                continue;
            }
            let component = water_component_tiles(city, &pipe_tiles, &mut visited, x, y);
            let demand = component
                .iter()
                .filter(|&&(tx, ty)| water_consuming_tile(city, tx, ty))
                .count() as u32;
            if demand == 0 {
                continue;
            }
            let mut component_mask = vec![false; CITY_TILES];
            for &(tx, ty) in &component {
                component_mask[ty * CITY_WIDTH + tx] = true;
            }
            let capacity = connected_water_supply_capacity_tiles_with_power(
                city,
                &component_mask,
                &power_maps,
            );
            if service_capacity_at_warning_threshold(capacity, demand) {
                return true;
            }
        }
    }

    false
}

pub(in crate::app) fn water_used_percent_for_city(city: &City) -> u32 {
    water_used_percent_for_city_with_entry_seed(city, pe_power_recalc_seed(city))
}

pub(in crate::app) fn water_used_percent_for_city_with_entry_seed(
    city: &City,
    entry_seed: u32,
) -> u32 {
    let sea_level = city.misc_water_table_level().unwrap_or(0);
    let precipitation = city.misc_humidity().unwrap_or(0);
    let capacity =
        water_supply_capacity_tiles_with_entry_seed(city, sea_level, precipitation, entry_seed);
    let used = water_service_summary_with_entry_seed(city, entry_seed).served;
    service_utilization_percent(used, capacity)
}

pub(in crate::app) fn water_demand_tiles(city: &City) -> u32 {
    let mut demand = 0;
    for y in 0..CITY_HEIGHT {
        for x in 0..CITY_WIDTH {
            if water_consuming_tile(city, x, y) {
                demand += 1;
            }
        }
    }
    demand
}

pub(in crate::app) fn water_consuming_tile(city: &City, x: usize, y: usize) -> bool {
    let building = city.building_at(x, y);
    (structural_internal_pipe_eligible(city, x, y) || zone_serviceable_tile(city, x, y))
        && !is_water_utility_building(building)
        && !is_power_plant_building(building)
        && !is_abandoned_building(building)
}

pub(in crate::app) fn desalination_output_tiles(
    city: &City,
    origin_x: usize,
    origin_y: usize,
) -> u32 {
    let Some(footprint) = building_footprint_from_origin(origin_x, origin_y, 3) else {
        return 0;
    };
    footprint
        .into_iter()
        .flat_map(|(x, y)| centered_tiles(x, y))
        .filter(|&(x, y)| pe_salt_surface_water(city, x, y))
        .count() as u32
        * 20
}

fn centered_tiles(x: usize, y: usize) -> impl Iterator<Item = (usize, usize)> {
    (y.saturating_sub(1)..=(y + 1).min(CITY_HEIGHT - 1)).flat_map(move |ty| {
        (x.saturating_sub(1)..=(x + 1).min(CITY_WIDTH - 1)).map(move |tx| (tx, ty))
    })
}

fn pe_fresh_surface_water(city: &City, x: usize, y: usize) -> bool {
    city.tile_bits_at(x, y)
        .is_some_and(|bits| bits.raw_bits() & 0x05 == 0x04)
}

fn pe_salt_surface_water(city: &City, x: usize, y: usize) -> bool {
    city.tile_bits_at(x, y)
        .is_some_and(|bits| bits.raw_bits() & 0x05 == 0x05)
}

pub(in crate::app) fn is_freshwater_tile(city: &City, x: usize, y: usize) -> bool {
    is_water_terrain(city.terrain_at(x, y))
        && !city
            .tile_bits_at(x, y)
            .is_some_and(|bits| bits.salt_water())
}

pub(in crate::app) fn is_saltwater_tile(city: &City, x: usize, y: usize) -> bool {
    is_water_terrain(city.terrain_at(x, y))
        && city
            .tile_bits_at(x, y)
            .is_some_and(|bits| bits.salt_water())
}

pub(in crate::app) fn is_water_utility_building(building: u8) -> bool {
    matches!(building, 220 | 235 | 244 | 250)
}

pub(in crate::app) fn water_source_connects_as_building_pipe(
    city: &City,
    x: usize,
    y: usize,
) -> bool {
    match city.building_at(x, y) {
        220 => structural_internal_pipe_eligible(city, x, y),
        250 => complete_building_footprint_containing(city, x, y, 250, 3),
        _ => false,
    }
}
