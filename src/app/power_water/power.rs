//! Power supply, connectivity, allocation, and service maps.

use super::*;

#[cfg(test)]
thread_local! {
    static MAC_POWER_SOURCE_TRACE:
        std::cell::RefCell<Option<Vec<(usize, usize, u8, u32, u32, u32)>>> = const {
            std::cell::RefCell::new(None)
        };
    static MAC_POWER_FLOOD_TRACE:
        std::cell::RefCell<Option<Vec<(usize, usize, usize, usize, u32, u8, u8)>>> = const {
            std::cell::RefCell::new(None)
        };
    static MAC_POWER_COMPONENT_TRACE:
        std::cell::RefCell<Option<Vec<(usize, usize, u32, u32, u32, bool)>>> = const {
            std::cell::RefCell::new(None)
        };
    static MAC_POWER_FIRST_PASS_TRACE:
        std::cell::RefCell<Option<Vec<(usize, usize, usize, usize, u8, u8)>>> = const {
            std::cell::RefCell::new(None)
        };
}

#[cfg(test)]
pub(in crate::app) fn mac_power_source_trace_begin() {
    MAC_POWER_SOURCE_TRACE.with(|trace| *trace.borrow_mut() = Some(Vec::new()));
}

#[cfg(test)]
pub(in crate::app) fn mac_power_source_trace_take() -> Vec<(usize, usize, u8, u32, u32, u32)> {
    MAC_POWER_SOURCE_TRACE.with(|trace| trace.borrow_mut().take().unwrap_or_default())
}

#[cfg(test)]
pub(in crate::app) fn mac_power_flood_trace_begin() {
    MAC_POWER_FLOOD_TRACE.with(|trace| *trace.borrow_mut() = Some(Vec::new()));
}

#[cfg(test)]
pub(in crate::app) fn mac_power_flood_trace_take() -> Vec<(usize, usize, usize, usize, u32, u8, u8)>
{
    MAC_POWER_FLOOD_TRACE.with(|trace| trace.borrow_mut().take().unwrap_or_default())
}

#[cfg(test)]
pub(in crate::app) fn mac_power_component_trace_begin() {
    MAC_POWER_COMPONENT_TRACE.with(|trace| *trace.borrow_mut() = Some(Vec::new()));
}

#[cfg(test)]
pub(in crate::app) fn mac_power_component_trace_take() -> Vec<(usize, usize, u32, u32, u32, bool)> {
    MAC_POWER_COMPONENT_TRACE.with(|trace| trace.borrow_mut().take().unwrap_or_default())
}

#[cfg(test)]
pub(in crate::app) fn mac_power_first_pass_trace_begin() {
    MAC_POWER_FIRST_PASS_TRACE.with(|trace| *trace.borrow_mut() = Some(Vec::new()));
}

#[cfg(test)]
pub(in crate::app) fn mac_power_first_pass_trace_take() -> Vec<(usize, usize, usize, usize, u8, u8)>
{
    MAC_POWER_FIRST_PASS_TRACE.with(|trace| trace.borrow_mut().take().unwrap_or_default())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::app) struct PowerSupplySummary {
    pub(in crate::app) capacity: u32,
    pub(in crate::app) demand: u32,
}

impl PowerSupplySummary {
    pub(in crate::app) fn shortage(self) -> u32 {
        self.demand.saturating_sub(self.capacity)
    }

    pub(in crate::app) fn has_shortage(self) -> bool {
        self.shortage() > 0
    }
}

pub(in crate::app) fn power_supply_summary(city: &City) -> PowerSupplySummary {
    PowerSupplySummary {
        capacity: power_supply_capacity_tiles(city),
        demand: power_demand_tiles(city),
    }
}

/// Win95 `dwPowerUsedPercentage`: powered consumers × 100 / plant capacity (100 if no capacity).
pub(in crate::app) fn power_used_percent_for_city(city: &City) -> u32 {
    power_used_percent_for_city_at_clock(city, pe_power_recalc_clock_from_city(city))
}

pub(in crate::app) fn power_used_percent_for_city_at_clock(city: &City, clock: GameClock) -> u32 {
    let mut rng = pe_power_recalc_rng_for_clock(city, clock);
    power_used_percent_with_rng(city, &mut rng)
}

/// Live sim day-1 power: reuse the shared process CRT stream (`0x48cdd8`).
pub(in crate::app) fn power_used_percent_for_city_with_process_crt(
    city: &City,
    crt: &mut Win95CrtRand,
) -> u32 {
    power_used_percent_with_rng(city, crt)
}

fn power_used_percent_with_rng(city: &City, crt: &mut Win95CrtRand) -> u32 {
    let entry_seed = crt.seed();
    let capacity = power_supply_capacity_tiles_with_rng(city, crt);
    let used = power_service_maps_with_entry_seed(city, entry_seed).pe_powered_consumers(city);
    service_utilization_percent(used, capacity)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::app) struct PowerServiceSummary {
    pub(in crate::app) served: u32,
    pub(in crate::app) demand: u32,
}

impl PowerServiceSummary {
    pub(in crate::app) fn shortage(self) -> u32 {
        self.demand.saturating_sub(self.served)
    }
}

pub(in crate::app) fn power_service_summary(city: &City) -> PowerServiceSummary {
    power_service_summary_with_entry_seed(city, pe_power_recalc_seed(city))
}

pub(in crate::app) fn power_service_summary_with_entry_seed(
    city: &City,
    entry_seed: u32,
) -> PowerServiceSummary {
    power_service_maps_with_entry_seed(city, entry_seed).summary(city)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::app) struct PowerServiceMaps {
    pub(in crate::app) conductive_tiles: Vec<bool>,
    pub(in crate::app) energized_tiles: Vec<bool>,
    /// Saved XBIT `0x40` published by Win95 `FUN_00467860` or Mac CODE 14 `export_218`.
    pub(in crate::app) saved_xbit_powered_tiles: Vec<bool>,
    pub(in crate::app) powerable_tiles: Vec<bool>,
    pub(in crate::app) powered_tiles: Vec<bool>,
}

impl Default for PowerServiceMaps {
    fn default() -> Self {
        let empty_plane = || vec![false; CITY_TILES];
        Self {
            conductive_tiles: empty_plane(),
            energized_tiles: empty_plane(),
            saved_xbit_powered_tiles: empty_plane(),
            powerable_tiles: empty_plane(),
            powered_tiles: empty_plane(),
        }
    }
}

/// Win95 `FUN_00436D00` and Mac CODE 8 `FUN_00036c8c`: growth reads saved
/// XBIT `0x40` on the visited tile or a cardinal neighbor, not derived geometry.
pub(in crate::app) fn growth_powered_cross_for_engine(
    power_maps: &PowerServiceMaps,
    x: usize,
    y: usize,
) -> bool {
    growth_powered_cross_from_plane(&power_maps.saved_xbit_powered_tiles, x, y)
}

fn growth_powered_cross_from_plane(plane: &[bool], x: usize, y: usize) -> bool {
    growth_powered_cross_with(x, y, |tx, ty| plane[ty * CITY_WIDTH + tx])
}

fn growth_powered_cross_with(
    x: usize,
    y: usize,
    mut is_powered: impl FnMut(usize, usize) -> bool,
) -> bool {
    // PE west/north probes use strict `> 1`; east/south use `< 0x7f`.
    let mut tile_powered = |tx: usize, ty: usize| is_powered(tx, ty);
    let mut powered = tile_powered(x, y);
    if x > 1 {
        powered |= tile_powered(x - 1, y);
    }
    if x < CITY_WIDTH - 1 {
        powered |= tile_powered(x + 1, y);
    }
    if y > 1 {
        powered |= tile_powered(x, y - 1);
    }
    if y < CITY_HEIGHT - 1 {
        powered |= tile_powered(x, y + 1);
    }
    powered
}

impl PowerServiceMaps {
    pub(in crate::app) fn powered_consumers(&self, city: &City) -> u32 {
        let mut count = 0;
        for y in 0..CITY_HEIGHT {
            for x in 0..CITY_WIDTH {
                let index = y * CITY_WIDTH + x;
                if self.powered_tiles[index] && power_consuming_tile(city, x, y) {
                    count += 1;
                }
            }
        }
        count
    }

    /// Powered tiles in the PE flood-fill consumer classes.
    pub(in crate::app) fn pe_powered_consumers(&self, city: &City) -> u32 {
        let mut count = 0;
        for y in 0..CITY_HEIGHT {
            for x in 0..CITY_WIDTH {
                let index = y * CITY_WIDTH + x;
                // PE's first worker pass counts source-eligible consumers, then
                // publishes min(component capacity, consumer count) to the global
                // used-percentage numerator. The later XBIT-0x40 second pass is a
                // separate supply-limited traversal and must not feed that counter.
                if self.energized_tiles[index]
                    && pe_power_flood_tile_accepted(city, x, y)
                    && pe_power_flood_fill_consumer_tile(city, x, y)
                {
                    count += 1;
                }
            }
        }
        count
    }

    pub(in crate::app) fn summary(&self, city: &City) -> PowerServiceSummary {
        PowerServiceSummary {
            served: self.powered_consumers(city),
            demand: power_demand_tiles(city),
        }
    }

    /// PE-faithful served/demand for shortage math (excludes bare zones).
    pub(in crate::app) fn pe_summary(&self, city: &City) -> PowerServiceSummary {
        PowerServiceSummary {
            served: self.pe_powered_consumers(city),
            demand: pe_power_demand_tiles(city),
        }
    }

    pub(in crate::app) fn status_text_at(&self, x: usize, y: usize) -> &'static str {
        let index = y * CITY_WIDTH + x;
        if self.powerable_tiles[index] {
            // PE growth/source paths read XBIT `0x40`. PE's second queue pass
            // stamps that bit only while remaining component supply is nonzero;
            // City's energized map is the geometry-connected view, while its
            // powered map rations consumers in the port's ordering. Exact PE
            // per-tile publication order remains to be mirrored here.
            if self.energized_tiles[index] {
                " pwr=on"
            } else {
                " pwr=off"
            }
        } else if self.conductive_tiles[index] {
            if self.energized_tiles[index] {
                " pwr=on"
            } else {
                " pwr=off"
            }
        } else {
            ""
        }
    }
}

pub(in crate::app) fn power_service_maps(city: &City) -> PowerServiceMaps {
    power_service_maps_with_entry_seed(city, pe_power_recalc_seed(city))
}

/// Build power maps using plant CRT rolls from a fixed entry seed (per-component restart).
pub(in crate::app) fn power_service_maps_with_entry_seed(
    city: &City,
    entry_seed: u32,
) -> PowerServiceMaps {
    let (power_tiles, _) = power_connection_maps(city);
    let mut visited = vec![false; CITY_TILES];
    let conductive_tiles = power_tiles.clone();
    let mut energized_tiles = vec![false; CITY_TILES];
    let mut saved_xbit_powered_tiles = vec![false; CITY_TILES];
    let mut powerable_tiles = vec![false; CITY_TILES];
    let mut powered_tiles = vec![false; CITY_TILES];

    for y in 0..CITY_HEIGHT {
        for x in 0..CITY_WIDTH {
            let start_index = y * CITY_WIDTH + x;
            saved_xbit_powered_tiles[start_index] = city
                .tile_bits_at(x, y)
                .is_some_and(|bits| bits.raw_bits() & 0x40 != 0);
            if visited[start_index] || !power_tiles[start_index] {
                continue;
            }
            let component = power_component_tiles(city, &power_tiles, &mut visited, x, y);
            let mut component_mask = vec![false; CITY_TILES];
            let mut consumers = Vec::new();
            for &(tx, ty) in &component {
                component_mask[ty * CITY_WIDTH + tx] = true;
                if power_consuming_tile(city, tx, ty) {
                    consumers.push((tx, ty));
                }
            }
            let mut plant_rng = Win95CrtRand::new(entry_seed);
            let component_capacity = connected_power_supply_capacity_tiles_with_rng(
                city,
                &component_mask,
                &mut plant_rng,
            );
            // CityGame's derived connected-state view is separate from its
            // capacity-rationed consumer view. A dynamic plant can have zero
            // current CRT/XVAL contribution while the geometry component still
            // has an eligible source. PE's separate XBIT-0x40 pass is supply-
            // and queue-order-limited; it is not this whole-component view. The
            // zero-output fixture below characterizes only this geometry map.
            let has_eligible_source = component.iter().any(|&(tx, ty)| {
                power_plant_footprint_origin(city, tx, ty)
                    .is_some_and(|(ox, oy)| pe_power_tile_source_eligible(city, ox, oy))
            });
            if has_eligible_source {
                for &(tx, ty) in &component {
                    energized_tiles[ty * CITY_WIDTH + tx] = true;
                }
            }
            for (index, (tx, ty)) in consumers.into_iter().enumerate() {
                let tile_index = ty * CITY_WIDTH + tx;
                powerable_tiles[tile_index] = true;
                powered_tiles[tile_index] = (index as u32) < component_capacity;
            }
        }
    }

    PowerServiceMaps {
        conductive_tiles,
        energized_tiles,
        saved_xbit_powered_tiles,
        powerable_tiles,
        powered_tiles,
    }
}

pub(in crate::app) fn power_capacity_warning_for_city(city: &City) -> bool {
    power_capacity_warning_for_city_with_entry_seed(city, pe_power_recalc_seed(city))
}

pub(in crate::app) fn power_capacity_warning_for_city_with_entry_seed(
    city: &City,
    entry_seed: u32,
) -> bool {
    let (power_tiles, _) = power_connection_maps(city);
    let mut visited = vec![false; CITY_TILES];

    for y in 0..CITY_HEIGHT {
        for x in 0..CITY_WIDTH {
            let start_index = y * CITY_WIDTH + x;
            if visited[start_index] || !power_tiles[start_index] {
                continue;
            }
            let component = power_component_tiles(city, &power_tiles, &mut visited, x, y);
            let demand = component
                .iter()
                .filter(|&&(tx, ty)| power_consuming_tile(city, tx, ty))
                .count() as u32;
            if demand == 0 {
                continue;
            }
            let mut component_mask = vec![false; CITY_TILES];
            for &(tx, ty) in &component {
                component_mask[ty * CITY_WIDTH + tx] = true;
            }
            let mut rng = Win95CrtRand::new(entry_seed);
            let capacity =
                connected_power_supply_capacity_tiles_with_rng(city, &component_mask, &mut rng);
            if service_capacity_at_warning_threshold(capacity, demand) {
                return true;
            }
        }
    }

    false
}

pub(in crate::app) fn tile_power_status_text(city: &City, x: usize, y: usize) -> &'static str {
    tile_power_status_text_with_entry_seed(city, x, y, pe_power_recalc_seed(city))
}

pub(in crate::app) fn tile_power_status_text_with_entry_seed(
    city: &City,
    x: usize,
    y: usize,
    entry_seed: u32,
) -> &'static str {
    power_service_maps_with_entry_seed(city, entry_seed).status_text_at(x, y)
}

pub(in crate::app) fn power_supply_capacity_tiles(city: &City) -> u32 {
    power_supply_capacity_tiles_at_clock(city, pe_power_recalc_clock_from_city(city))
}

pub(in crate::app) fn power_supply_capacity_tiles_at_clock(city: &City, clock: GameClock) -> u32 {
    let mut rng = pe_power_recalc_rng_for_clock(city, clock);
    power_supply_capacity_tiles_with_rng(city, &mut rng)
}

pub(in crate::app) fn power_supply_capacity_tiles_with_rng(
    city: &City,
    rng: &mut Win95CrtRand,
) -> u32 {
    let mut seen = vec![false; CITY_TILES];
    let mut capacity = 0;
    for y in 0..CITY_HEIGHT {
        for x in 0..CITY_WIDTH {
            let Some((origin_x, origin_y)) = power_plant_footprint_origin(city, x, y) else {
                continue;
            };
            let origin_index = origin_y * CITY_WIDTH + origin_x;
            if seen[origin_index] {
                continue;
            }
            seen[origin_index] = true;
            capacity += power_plant_output_tiles_with_rng(city, origin_x, origin_y, rng);
        }
    }
    capacity
}

pub(in crate::app) fn connected_power_supply_capacity_tiles(
    city: &City,
    connected_tiles: &[bool],
) -> u32 {
    let mut rng = pe_power_recalc_rng(city);
    connected_power_supply_capacity_tiles_with_rng(city, connected_tiles, &mut rng)
}

pub(in crate::app) fn connected_power_supply_capacity_tiles_with_rng(
    city: &City,
    connected_tiles: &[bool],
    rng: &mut Win95CrtRand,
) -> u32 {
    let mut seen = vec![false; CITY_TILES];
    let mut capacity = 0;
    for y in 0..CITY_HEIGHT {
        for x in 0..CITY_WIDTH {
            let Some((origin_x, origin_y)) = power_plant_footprint_origin(city, x, y) else {
                continue;
            };
            let origin_index = origin_y * CITY_WIDTH + origin_x;
            if seen[origin_index] {
                continue;
            }
            seen[origin_index] = true;
            let building = city.building_at(origin_x, origin_y);
            if !complete_connected_building_footprint_at_origin(
                city,
                connected_tiles,
                origin_x,
                origin_y,
                building,
                usize::from(xbld_tile_size(building)),
            ) {
                continue;
            }
            capacity += power_plant_output_tiles_with_rng(city, origin_x, origin_y, rng);
        }
    }
    capacity
}

pub(in crate::app) fn power_connection_maps(city: &City) -> (Vec<bool>, Vec<bool>) {
    let mut power_tiles = vec![false; CITY_TILES];
    let mut connected_tiles = vec![false; CITY_TILES];
    let mut queue = VecDeque::new();
    let mut seen = vec![false; CITY_TILES];

    for y in 0..CITY_HEIGHT {
        for x in 0..CITY_WIDTH {
            let index = y * CITY_WIDTH + x;
            power_tiles[index] = power_network_tile(city, x, y);
        }
    }

    for y in 0..CITY_HEIGHT {
        for x in 0..CITY_WIDTH {
            let Some((origin_x, origin_y)) = power_plant_footprint_origin(city, x, y) else {
                continue;
            };
            let origin_index = origin_y * CITY_WIDTH + origin_x;
            if seen[origin_index] {
                continue;
            }
            seen[origin_index] = true;
            let building = city.building_at(origin_x, origin_y);
            let Some(footprint) = building_footprint_from_origin(
                origin_x,
                origin_y,
                usize::from(xbld_tile_size(building)),
            ) else {
                continue;
            };
            for (tx, ty) in footprint {
                let index = ty * CITY_WIDTH + tx;
                if power_tiles[index] && !connected_tiles[index] {
                    connected_tiles[index] = true;
                    pe_network_queue_push(&mut queue, (tx, ty));
                }
            }
        }
    }

    while let Some((x, y)) = queue.pop_front() {
        for (side, nx, ny) in directional_neighbor_tiles(x, y) {
            let index = ny * CITY_WIDTH + nx;
            if connected_tiles[index] || !power_tiles[index] {
                continue;
            }
            if power_tile_connects_to(city, x, y, side)
                && power_tile_connects_to(city, nx, ny, opposite_network_side(side))
            {
                connected_tiles[index] = true;
                pe_network_queue_push(&mut queue, (nx, ny));
            }
        }
    }

    (power_tiles, connected_tiles)
}

pub(in crate::app) fn power_plant_footprint_origin(
    city: &City,
    x: usize,
    y: usize,
) -> Option<(usize, usize)> {
    let building = city.building_at(x, y);
    if !is_power_plant_building(building) {
        return None;
    }
    let size = usize::from(xbld_tile_size(building).max(1));
    building_footprint_origin_containing(city, x, y, building, size)
}

pub(in crate::app) fn power_component_tiles(
    city: &City,
    power_tiles: &[bool],
    visited: &mut [bool],
    start_x: usize,
    start_y: usize,
) -> Vec<(usize, usize)> {
    let mut component = Vec::new();
    let mut queue = VecDeque::new();
    let start_index = start_y * CITY_WIDTH + start_x;
    visited[start_index] = true;
    pe_network_queue_push(&mut queue, (start_x, start_y));

    while let Some((x, y)) = queue.pop_front() {
        component.push((x, y));
        for (side, nx, ny) in directional_neighbor_tiles(x, y) {
            let index = ny * CITY_WIDTH + nx;
            if visited[index] || !power_tiles[index] {
                continue;
            }
            if power_tile_connects_to(city, x, y, side)
                && power_tile_connects_to(city, nx, ny, opposite_network_side(side))
            {
                visited[index] = true;
                pe_network_queue_push(&mut queue, (nx, ny));
            }
        }
    }

    component
}

pub(in crate::app) fn power_network_tile(city: &City, x: usize, y: usize) -> bool {
    let building = city.building_at(x, y);
    network_building_connects_to(building, NetworkKind::Power, NETWORK_TOP)
        || network_building_connects_to(building, NetworkKind::Power, NETWORK_RIGHT)
        || network_building_connects_to(building, NetworkKind::Power, NETWORK_BOTTOM)
        || network_building_connects_to(building, NetworkKind::Power, NETWORK_LEFT)
        || is_power_plant_building(building)
        || structural_powerable_tile(city, x, y)
}

/// Rust network-geometry predicate used by the live flood.
///
/// PE `FUN_004679B0` does **not** call this style of side-matched check; it expands
/// through any dequeued tile that already carries XBIT `0x80` (see
/// [`pe_power_flood_tile_accepted`]). Keep the PE membership bit and this connector
/// helper distinct until placement sites that stamp `0x80` are fully mapped.
pub(in crate::app) fn power_tile_connects_to(city: &City, x: usize, y: usize, side: u8) -> bool {
    let building = city.building_at(x, y);
    if is_power_plant_building(building) || structural_powerable_tile(city, x, y) {
        return true;
    }
    network_building_connects_to(building, NetworkKind::Power, side)
}

pub(in crate::app) fn power_plant_anchor_at(city: &City, x: usize, y: usize) -> bool {
    let building = city.building_at(x, y);
    if !matches!(building, 198 | 199 | 200 | 201..=207) {
        return false;
    }
    let size = usize::from(xbld_tile_size(building));
    building_footprint_origin_containing(city, x, y, building, size).is_some()
}

/// PE globals persisted by `FUN_00433a40` into the city save blob (MISC dword slots).
pub(in crate::app) fn pe_sim_wind_param(city: &City) -> u32 {
    // PE wind/power arms read BSS `bWeatherWind` @ `0x4cae0c`.
    u32::from(city.pe_b_weather_wind)
}

/// PE `0x004CB3D4`; stored in MISC humidity slot but drives `0xCC` solar rand span too.
pub(in crate::app) fn pe_sim_fusion_humid_param(city: &City) -> u32 {
    // PE humidity/fusion arms read BSS `bWeatherHumid` @ `0x4cb3d4`.
    u32::from(city.pe_b_weather_humid)
}

/// PE `0xC8` random modulus: `(0x004CAE0C >> 3) + 1`.
pub(in crate::app) fn pe_power_c8_random_modulus(city: &City) -> u32 {
    (pe_sim_wind_param(city) >> 3).saturating_add(1)
}

/// PE `0xCC` random span: `(100 - 0x004CB3D4) / 10` for ordinary humidity.
pub(in crate::app) fn pe_power_cc_random_span(city: &City) -> u32 {
    100_u32.saturating_sub(pe_sim_fusion_humid_param(city)) / 10
}

/// PE `FUN_004679b0` `0xC8` worker contribution (pre network tile scaling).
pub(in crate::app) fn pe_power_c8_pe_units(
    city: &City,
    x: usize,
    y: usize,
    rand_remainder: u32,
) -> u32 {
    // 0x467afe reads the ALTM row and 0x467b0c masks off water/tunnel fields.
    let altitude = u32::from(city.altitude_at(x, y));
    let modulus = pe_power_c8_random_modulus(city);
    (altitude + rand_remainder % modulus) / 2
}

/// PE `FUN_004679b0` `0xCC` worker contribution (pre network tile scaling).
pub(in crate::app) fn pe_power_cc_pe_units(city: &City, rand_remainder: u32) -> u32 {
    // The PE's signed divisor becomes zero for humidity 91..=109, trapping in
    // its solar-source arm. Native new-city weather starts at 15 and eases
    // toward targets no higher than 60, so keep non-native/custom saves
    // playable instead of reproducing that out-of-domain divide-by-zero.
    let span = pe_power_cc_random_span(city).max(1);
    5_u32.saturating_add(rand_remainder % span)
}

/// Mac CODE 14 `FUN_00025a7c`, XBLD `0xC8` source contribution. The caller's
/// process-wide QuickDraw stream is not reconstructed yet, so keep the exact
/// one-draw operation explicit and require the caller to supply that stream.
pub(in crate::app) fn mac_power_c8_units(
    city: &City,
    x: usize,
    y: usize,
    rng: &mut crate::mac_random::MacToolboxRng,
) -> u32 {
    let random = u32::from(rng.random_u16());
    let wind_input = city.mac_state_misc_u32_at(0x64).unwrap_or(0) as u8;
    let modulus = u32::from((wind_input >> 3) + 1);
    // Mac CODE 14's A5-0xebc row table is ALTM (terrain height), not XVAL.
    let altitude = u32::from(city.altitude_at(x, y) & 0x1f);
    (altitude + random % modulus) >> 1
}

/// Mac CODE 14 `FUN_00025a7c`, XBLD `0xCC` source contribution. The native
/// code computes a signed `(100 - F086) / 10`, then uses that WORD as the
/// divisor of an unsigned QuickDraw result. `None` represents the native
/// divide-by-zero trap for humidity bytes 91 through 109.
pub(in crate::app) fn mac_power_cc_units(
    city: &City,
    rng: &mut crate::mac_random::MacToolboxRng,
) -> Option<u32> {
    let random = u32::from(rng.random_u16());
    let humidity = city.mac_weather_humidity().unwrap_or(0) as u8;
    let signed_span = (100_i16 - i16::from(humidity)) / 10;
    let divisor = u32::from(signed_span as u16);
    (divisor != 0).then(|| 5 + random % divisor)
}

/// Fixed Mac CODE 14 `FUN_00025a7c` source contributions. The dispatch table
/// at CODE 14 resource offset `0x4e6` is indexed by `XBLD - 0xc6`; in
/// particular, `0xc9` and `0xca` target 11 and 48 respectively. These are
/// map-source contributions, independent of the Mac XMIC initializer's
/// per-record `+2` WORDs.
const MAC_POWER_FIXED_SOURCE_UNITS: [(u8, u32); 8] = [
    (0xC6, 40),
    (0xC7, 40),
    (0xC9, 11),
    (0xCA, 48),
    (0xCB, 111),
    (0xCD, 0x163),
    (0xCE, 0x22B),
    (0xCF, 44),
];

pub(in crate::app) fn mac_power_fixed_units_for_building(building: u8) -> Option<u32> {
    MAC_POWER_FIXED_SOURCE_UNITS
        .iter()
        .find_map(|&(id, units)| (id == building).then_some(units))
}

fn mac_power_source_units_at(
    city: &City,
    x: usize,
    y: usize,
    rng: &mut crate::mac_random::MacToolboxRng,
) -> Option<u32> {
    let building = city.building_at(x, y);
    #[cfg(test)]
    let seed_before = rng.seed();
    let units = match building {
        0xC8 => Some(mac_power_c8_units(city, x, y, rng)),
        0xCC => mac_power_cc_units(city, rng),
        building => mac_power_fixed_units_for_building(building),
    };
    #[cfg(test)]
    let units_for_trace = units.unwrap_or(u32::MAX);
    #[cfg(test)]
    MAC_POWER_SOURCE_TRACE.with(|trace| {
        if let Some(trace) = trace.borrow_mut().as_mut() {
            trace.push((x, y, building, seed_before, rng.seed(), units_for_trace));
        }
    });
    units
}

/// Mac CODE 14 `FUN_00025a7c` first component flood. The native worker only
/// admits XBIT `0x80` cells, marks them in `0x08`, counts powered loads at
/// XBLD `0x70..=0xff` except the plant-source band `0xc6..=0xcf`, and adds
/// connected plant contributions. Its CODE 14 queue
/// helper is the same 512-slot ring used by the other bounded map walkers
/// (511 pending entries, dropping the oldest on collision).
pub(super) fn mac_power_first_pass(
    city: &mut City,
    root: (usize, usize),
    rng: &mut crate::mac_random::MacToolboxRng,
) -> Option<(u32, u32)> {
    let mut queue = std::collections::VecDeque::from([root]);
    let (mut capacity, mut consumers) = (0_u32, 0_u32);
    while let Some((x, y)) = queue.pop_front() {
        let Some(bits) = city.tile_bits_at(x, y) else {
            continue;
        };
        if bits.raw_bits() & 0x08 != 0 || !bits.powerable() {
            continue;
        }
        city.set_tile_bits(x, y, TileBits::new(bits.raw_bits() | 0x08));

        let building = city.building_at(x, y);
        #[cfg(test)]
        if root == (26, 23) {
            MAC_POWER_FIRST_PASS_TRACE.with(|trace| {
                if let Some(trace) = trace.borrow_mut().as_mut() {
                    trace.push((root.0, root.1, x, y, building, bits.raw_bits()));
                }
            });
        }
        if pe_power_source_xbld(building) {
            capacity = capacity.saturating_add(mac_power_source_units_at(city, x, y, rng)?);
        } else if mac_power_load_xbld(building) {
            consumers = consumers.saturating_add(1);
        }

        // CODE 14 queues north, west, south, east. It marks on dequeue, so
        // duplicate pending coordinates are allowed and filtered by XBIT 0x08.
        for neighbor in [
            y.checked_sub(1).map(|ny| (x, ny)),
            x.checked_sub(1).map(|nx| (nx, y)),
            (y + 1 < 0x80).then_some((x, y + 1)),
            (x + 1 < 0x80).then_some((x + 1, y)),
        ]
        .into_iter()
        .flatten()
        {
            let (nx, ny) = neighbor;
            if nx >= CITY_WIDTH || ny >= CITY_HEIGHT || nx >= 0x80 || ny >= 0x80 {
                continue;
            }
            if city
                .tile_bits_at(nx, ny)
                .is_some_and(|neighbor_bits| neighbor_bits.raw_bits() & 0x08 == 0)
            {
                power_queue_push_511(&mut queue, neighbor);
            }
        }
    }
    Some((capacity, consumers))
}

/// Mac CODE 14 treats the connected plant IDs as capacity sources and other
/// XBLD IDs from `0x70` upward as one load each. The upper facility band is
/// part of the load path too; the native phase-1/phase-2 join specifically
/// exercises the D2 police station that the former C6 cutoff omitted.
pub(super) fn mac_power_load_xbld(building: u8) -> bool {
    building >= 0x70 && !(0xC6..=0xCF).contains(&building)
}

/// Mac CODE 14 `FUN_00025a7c` second flood. The first-pass scratch markers
/// constrain this walk; reached cells receive powered XBIT `0x40` while
/// capacity remains, with one unit consumed at every XBLD `>= 0x70` cell.
fn mac_power_second_pass(city: &mut City, root: (usize, usize), mut remaining: u32) {
    let mut queue = std::collections::VecDeque::from([root]);
    while let Some((x, y)) = queue.pop_front() {
        let Some(bits) = city.tile_bits_at(x, y) else {
            continue;
        };
        if bits.raw_bits() & 0x08 == 0 {
            continue;
        }
        #[cfg(test)]
        MAC_POWER_FLOOD_TRACE.with(|trace| {
            if let Some(trace) = trace.borrow_mut().as_mut() {
                trace.push((
                    root.0,
                    root.1,
                    x,
                    y,
                    remaining,
                    city.building_at(x, y),
                    bits.raw_bits(),
                ));
            }
        });
        if remaining != 0 {
            if city.building_at(x, y) >= 0x70 {
                remaining -= 1;
            }
            city.set_tile_bits(x, y, TileBits::new(bits.raw_bits() | 0x40));
        }
        let after_power = city.tile_bits_at(x, y).unwrap_or(bits);
        city.set_tile_bits(x, y, TileBits::new(after_power.raw_bits() & !0x08));

        for neighbor in [
            y.checked_sub(1).map(|ny| (x, ny)),
            x.checked_sub(1).map(|nx| (nx, y)),
            (y + 1 < 0x80).then_some((x, y + 1)),
            (x + 1 < 0x80).then_some((x + 1, y)),
        ]
        .into_iter()
        .flatten()
        {
            let (nx, ny) = neighbor;
            if nx >= CITY_WIDTH || ny >= CITY_HEIGHT || nx >= 0x80 || ny >= 0x80 {
                continue;
            }
            if city
                .tile_bits_at(nx, ny)
                .is_some_and(|neighbor_bits| neighbor_bits.raw_bits() & 0x08 != 0)
            {
                power_queue_push_511(&mut queue, neighbor);
            }
        }
    }
}

/// Mac CODE 14 `export_218 @ 0x259ba`: clear transient XBIT `0x40|0x08`,
/// visit plant roots x-major, allocate each component, and return the native
/// `fffff00a` summary. The caller supplies the process-wide QuickDraw stream;
/// no seed surrogate is created here. `None` represents the native divide by
/// zero trap in source `0xCC`, and the caller must not publish a partial pass.
pub(in crate::app) fn mac_export_218_power_with_rng(
    city: &mut City,
    rng: &mut crate::mac_random::MacToolboxRng,
) -> Option<u32> {
    // The native 0xCC case consumes Random before DIVU traps. Run only this
    // exceptional input on a scratch city so we can preserve the caller's map
    // while still advancing the process stream through preceding sources and
    // the trapping draw in native traversal order.
    let humidity = city.mac_weather_humidity().unwrap_or(0) as u8;
    if (91..=109).contains(&humidity)
        && (0..0x80).any(|x| {
            (0..0x80).any(|y| {
                city.building_at(x, y) == 0xCC
                    && city.tile_bits_at(x, y).is_some_and(TileBits::powerable)
            })
        })
    {
        let mut trial_city = city.clone();
        let mut trial_rng = *rng;
        let result = mac_export_218_power_inner(&mut trial_city, &mut trial_rng);
        *rng = trial_rng;
        debug_assert!(result.is_none(), "a powerable 0xCC source must reach DIVU");
        return None;
    }

    mac_export_218_power_inner(city, rng)
}

fn mac_export_218_power_inner(
    city: &mut City,
    rng: &mut crate::mac_random::MacToolboxRng,
) -> Option<u32> {
    let dim = 0x80.min(CITY_WIDTH).min(CITY_HEIGHT);
    for x in 0..dim {
        for y in 0..dim {
            if let Some(bits) = city.tile_bits_at(x, y) {
                city.set_tile_bits(x, y, TileBits::new(bits.raw_bits() & !0x48));
            }
        }
    }

    let mut raw_capacity = 0_u32;
    let mut served = 0_u32;
    // Mac CODE 14 tests bit 0 of byte `ffffeff3`. The ordinance mask begins at
    // `ffffeff2` and is saved at MISC slot 1000 (+0x0fa0); because MISC dwords
    // are big-endian, that byte bit is ordinance bit 16 (Energy Conservation).
    let energy_conservation_enabled = super::ordinances::ordinance_enabled_for_city_with_fidelity(
        city,
        super::ordinances::ORDINANCE_ENERGY_CONSERVATION,
        EngineFidelity::Mac68k,
    );
    for x in 0..dim {
        for y in 0..dim {
            if !pe_power_source_xbld(city.building_at(x, y))
                || city
                    .tile_bits_at(x, y)
                    .is_some_and(|bits| bits.raw_bits() & 0x40 != 0)
            {
                continue;
            }
            let Some((component_capacity, consumers)) = mac_power_first_pass(city, (x, y), rng)
            else {
                return None;
            };
            raw_capacity = raw_capacity.saturating_add(component_capacity);
            let available = if energy_conservation_enabled {
                component_capacity.saturating_add(component_capacity / 12)
            } else {
                component_capacity
            };
            #[cfg(test)]
            MAC_POWER_COMPONENT_TRACE.with(|trace| {
                if let Some(trace) = trace.borrow_mut().as_mut() {
                    trace.push((
                        x,
                        y,
                        component_capacity,
                        consumers,
                        available,
                        energy_conservation_enabled,
                    ));
                }
            });
            served = served.saturating_add(available.min(consumers));
            mac_power_second_pass(city, (x, y), available);
        }
    }

    Some(if raw_capacity == 0 {
        100
    } else {
        (u64::from(served) * 100 / u64::from(raw_capacity)) as u32
    })
}

/// Fixed PE plant capacities from jump table `0x467F98` (service units before Rust scaling).
pub(in crate::app) const PE_POWER_FIXED_PE_UNITS: [(u8, u32); 8] = [
    (198, 0x28),  // `0xC6` — Rust uses PE units directly as tiles
    (199, 0x28),  // `0xC7` hydro orientation twin
    (201, 0x0b),  // `0xC9` gas
    (202, 0x30),  // `0xCA` oil
    (203, 0x6f),  // `0xCB` nuclear
    (205, 0x163), // `0xCD` microwave
    (206, 0x22b), // `0xCE` fusion
    (207, 0x2c),  // `0xCF`
];

pub(in crate::app) fn pe_power_fixed_pe_units_for_building(building: u8) -> Option<u32> {
    PE_POWER_FIXED_PE_UNITS
        .iter()
        .find_map(|&(id, units)| (id == building).then_some(units))
}

/// Map Rust power-plant XBLD ids to PE jump-table XBLD ids (`0x467F98`).
pub(in crate::app) fn pe_power_xbld_for_building(building: u8) -> Option<u8> {
    match building {
        198 => Some(0xC6),
        199 => Some(0xC7),
        200 => Some(0xC8),
        201 => Some(0xC9),
        202 => Some(0xCA),
        203 => Some(0xCB),
        204 => Some(0xCC),
        205 => Some(0xCD),
        206 => Some(0xCE),
        207 => Some(0xCF),
        _ => None,
    }
}

/// CRT seed for PE power recalc: weather MISC inputs plus monthly weather stream entry roll.
pub(in crate::app) fn pe_power_recalc_seed(city: &City) -> u32 {
    pe_power_recalc_seed_for_clock(city, pe_power_recalc_clock_from_city(city))
}

pub(in crate::app) fn pe_power_recalc_clock_from_city(city: &City) -> GameClock {
    city.saved_date()
        .map(|date| GameClock::new_with_day(date.year, date.month, 1))
        .unwrap_or_else(GameClock::default_start)
}

pub(in crate::app) fn pe_power_recalc_seed_for_clock(city: &City, clock: GameClock) -> u32 {
    let base = Win95CrtRand::DEFAULT_SEED
        .wrapping_add(pe_sim_wind_param(city))
        .wrapping_add(pe_sim_fusion_humid_param(city).wrapping_mul(0x0001_0001))
        .wrapping_add(weather_advance_seed(city, clock));
    let mut rng = Win95CrtRand::new(base);
    base.wrapping_add(u32::from(rng.random_u16()))
}

/// CRT stream entry for PE `FUN_004679B0` plant scans at the given calendar month.
pub(in crate::app) fn pe_power_recalc_rng_for_clock(city: &City, clock: GameClock) -> Win95CrtRand {
    Win95CrtRand::new(pe_power_recalc_seed_for_clock(city, clock))
}

pub(in crate::app) fn pe_power_recalc_rng(city: &City) -> Win95CrtRand {
    pe_power_recalc_rng_for_clock(city, pe_power_recalc_clock_from_city(city))
}

/// Synthetic CRT entry when no live Win95 process stream is available.
/// Runtime Win95 startup supplies a tick-derived stream; this remains a fallback
/// for tests/offline and the unrooted Mac compatibility path.
pub(in crate::app) fn init_pe_process_crt(city: &City, clock: GameClock) -> Win95CrtRand {
    pe_power_recalc_rng_for_clock(city, clock)
}

/// Deterministic Game_Random LFSR fallback for tests/offline construction.
///
/// The live PE initializes process-global `0x4cdb7c` from
/// `GetTickCount() | 1` in the view constructor; runtime passes its
/// SDL-tick-seeded equivalent explicitly. City saves do not persist the word,
/// and live city opens preserve the running stream.
pub(in crate::app) fn init_pe_process_lfsr(_city: &City, _clock: GameClock) -> Win95Lfsr {
    Win95Lfsr::new(Win95Lfsr::DEFAULT_STATE)
}

/// PE `FUN_004679B0` dequeue gate: XBIT `0x80` must be set before capacity or neighbor
/// expansion. Cardinal enqueue via `FUN_00413CF0` only rejects already-visited
/// (`XBIT 0x08`) neighbors — there is no side-matched connector predicate in this worker.
pub(in crate::app) fn pe_power_flood_tile_accepted(city: &City, x: usize, y: usize) -> bool {
    city.tile_bits_at(x, y).is_some_and(TileBits::powerable)
}

/// PE `FUN_004679B0` requires `XBIT_POWERABLE` before counting plant output.
pub(in crate::app) fn pe_power_tile_source_eligible(city: &City, x: usize, y: usize) -> bool {
    if pe_power_flood_tile_accepted(city, x, y) {
        return true;
    }
    // Until plant commit mirrors XBIT_POWERABLE, complete plant origins still source.
    matches!(
        power_plant_footprint_origin(city, x, y),
        Some((origin_x, origin_y)) if origin_x == x && origin_y == y
    )
}

pub(in crate::app) fn pe_power_crt_rand_u16(rng: &mut Win95CrtRand) -> u32 {
    u32::from(rng.random_u16())
}

/// Clear `XBIT_POWERABLE` (`0x80`) on one tile.
///
/// PE budget road/rail decay (`0x435dd6`) and sc2kfix `DoBudgetRoadCheck` /
/// `DoBudgetRailCheck` clear this after replacing the surface with rubble.
pub(in crate::app) fn clear_xbit_powerable(city: &mut City, x: usize, y: usize) {
    let Some(bits) = city.tile_bits_at(x, y) else {
        return;
    };
    city.set_tile_bits(x, y, bits.with_powerable(false));
}

// The PE and Mac native helpers use a 512-record ring with one empty slot;
// on collision, enqueue advances the read cursor and drops the oldest item.
pub(super) const POWER_QUEUE_PENDING_CAPACITY: usize = 0x1ff;

pub(super) fn power_queue_push_511(
    queue: &mut std::collections::VecDeque<(usize, usize)>,
    coordinate: (usize, usize),
) {
    if queue.len() >= POWER_QUEUE_PENDING_CAPACITY {
        let _ = queue.pop_front();
    }
    queue.push_back(coordinate);
}

fn pe_power_source_units_at(city: &City, x: usize, y: usize, crt: &mut Win95CrtRand) -> u32 {
    match city.building_at(x, y) {
        198 | 199 => 0x28,
        200 => pe_power_c8_pe_units(city, x, y, pe_power_crt_rand_u16(crt)),
        201 => 0x0b,
        202 => 0x30,
        203 => 0x6f,
        204 => pe_power_cc_pe_units(city, pe_power_crt_rand_u16(crt)),
        205 => 0x163,
        206 => 0x22b,
        207 => 0x2c,
        _ => 0,
    }
}

pub(super) fn pe_power_first_pass(
    city: &mut City,
    root: (usize, usize),
    crt: &mut Win95CrtRand,
) -> (u32, u32) {
    let mut queue = std::collections::VecDeque::from([root]);
    let (mut supply, mut consumers) = (0_u32, 0_u32);
    let mut dequeues_since_reset = 0_u32;

    while let Some((x, y)) = queue.pop_front() {
        let Some(bits) = city.tile_bits_at(x, y) else {
            continue;
        };
        if bits.raw_bits() & 0x08 != 0 {
            continue;
        }
        dequeues_since_reset += 1;
        if dequeues_since_reset > 5000 {
            // PE calls a RET-only diagnostic thunk and resets this counter; it does not
            // terminate or truncate the allocation walk.
            dequeues_since_reset = 0;
        }
        if !bits.powerable() {
            continue;
        }

        // PE marks the accepted dequeued tile itself before it considers neighbors.
        city.set_tile_bits(x, y, TileBits::new(bits.raw_bits() | 0x08));

        let building = city.building_at(x, y);
        if pe_power_source_xbld(building) {
            supply = supply.saturating_add(pe_power_source_units_at(city, x, y, crt));
        } else if pe_power_consumer_xbld(building) {
            consumers = consumers.saturating_add(1);
        }

        // The native worker pushes N, W, S, E in that order. Unlike common BFS
        // implementations, it marks on dequeue; duplicate pending entries remain
        // possible and are discarded by the visited-bit check when they are popped.
        for neighbor in [
            y.checked_sub(1).map(|ny| (x, ny)),
            x.checked_sub(1).map(|nx| (nx, y)),
            (y + 1 < 0x80).then_some((x, y + 1)),
            (x + 1 < 0x80).then_some((x + 1, y)),
        ]
        .into_iter()
        .flatten()
        {
            let (nx, ny) = neighbor;
            if nx >= CITY_WIDTH || ny >= CITY_HEIGHT || nx >= 0x80 || ny >= 0x80 {
                continue;
            }
            let neighbor_bits = city.tile_bits_at(nx, ny).unwrap_or(TileBits::new(0));
            if neighbor_bits.raw_bits() & 0x08 == 0 {
                power_queue_push_511(&mut queue, neighbor);
            }
        }
    }

    (supply, consumers)
}

fn pe_power_second_pass(city: &mut City, root: (usize, usize), mut remaining: u32) {
    let mut queue = std::collections::VecDeque::from([root]);
    while let Some((x, y)) = queue.pop_front() {
        let Some(bits) = city.tile_bits_at(x, y) else {
            continue;
        };
        if bits.raw_bits() & 0x08 == 0 {
            continue;
        }

        if remaining != 0 {
            if city.building_at(x, y) >= 0x70 {
                remaining -= 1;
            }
            // The tile is stamped even when its own decrement exhausts supply.
            city.set_tile_bits(x, y, TileBits::new(bits.raw_bits() | 0x40));
        }
        let after_power = city.tile_bits_at(x, y).unwrap_or(bits);
        city.set_tile_bits(x, y, TileBits::new(after_power.raw_bits() & !0x08));

        for neighbor in [
            y.checked_sub(1).map(|ny| (x, ny)),
            x.checked_sub(1).map(|nx| (nx, y)),
            (y + 1 < 0x80).then_some((x, y + 1)),
            (x + 1 < 0x80).then_some((x + 1, y)),
        ]
        .into_iter()
        .flatten()
        {
            let (nx, ny) = neighbor;
            if nx >= CITY_WIDTH || ny >= CITY_HEIGHT || nx >= 0x80 || ny >= 0x80 {
                continue;
            }
            if city
                .tile_bits_at(nx, ny)
                .is_some_and(|b| b.raw_bits() & 0x08 != 0)
            {
                power_queue_push_511(&mut queue, neighbor);
            }
        }
    }
}

/// PE `FUN_00467860` clears powered/work bits, scans XBLD `0xC6..=0xCF` in x-major
/// order, and runs `FUN_004679B0` for each unpowered source. The worker's two bounded
/// queue walks publish the PE XBIT `0x40` allocation and update the saved service ratio.
pub(in crate::app) fn pe_fun_00467860(city: &mut City, crt: &mut Win95CrtRand) {
    let dim = 0x80.min(CITY_WIDTH).min(CITY_HEIGHT);
    for y in 0..dim {
        for x in 0..dim {
            let Some(bits) = city.tile_bits_at(x, y) else {
                continue;
            };
            city.set_tile_bits(x, y, TileBits::new(bits.raw_bits() & 0xb7));
        }
    }

    let mut capacity = 0_u32;
    let mut served = 0_u32;
    let ordinance_flags = city.ordinance_flags_for_saved_layout();
    for x in 0..dim {
        for y in 0..dim {
            let building = city.building_at(x, y);
            if !pe_power_source_xbld(building)
                || city
                    .tile_bits_at(x, y)
                    .is_some_and(|bits| bits.raw_bits() & 0x40 != 0)
            {
                continue;
            }

            let (raw_supply, consumers) = pe_power_first_pass(city, (x, y), crt);
            capacity = capacity.saturating_add(raw_supply);
            let supply_for_consumers = if ordinance_flags & 0x1_0000 != 0 {
                raw_supply.saturating_add(raw_supply / 12)
            } else {
                raw_supply
            };
            served = served.saturating_add(supply_for_consumers.min(consumers));
            pe_power_second_pass(city, (x, y), supply_for_consumers);
        }
    }

    let percent = if capacity == 0 {
        100
    } else {
        served
            .saturating_mul(100)
            .checked_div(capacity)
            .unwrap_or(100)
            .min(100)
    };
    city.set_pe_power_used_percent(percent);
}

/// PE `FUN_00464550` @ `0x464550` entry: map-wide on `0..0x80` — if XBLD ≠ `0xeb` then
/// `XBIT &= 0xef` (drop watered `0x10`); always `XBIT &= 0xf7` (drop work `0x08`).
/// The root-level clear is followed by the saved-XBIT source scan and bounded
/// queue worker, independent of CityGame's geometry-derived display map.
pub(in crate::app) fn pe_fun_00464550(city: &mut City) -> WaterServiceMaps {
    let maps = pe_water_service_maps_from_saved_xbit(city, Vec::new());
    for y in 0..CITY_HEIGHT {
        for x in 0..CITY_WIDTH {
            let Some(bits) = city.tile_bits_at(x, y) else {
                continue;
            };
            let index = y * CITY_WIDTH + x;
            let work = if maps.pe_worker_pending_tiles[index] {
                0x08
            } else {
                0
            };
            let updated = TileBits::new((bits.raw_bits() & !0x08) | work)
                .with_watered(maps.watered_tiles[index]);
            if bits != updated {
                city.set_tile_bits(x, y, updated);
            }
        }
    }
    city.set_pe_water_coverage_percent(maps.pe_worker_utilization_percent());
    city.publish_pe_sewer_bonus(maps.pe_worker_served_cells, city.pe_dw_tile_count[0xf4]);
    maps
}

/// Stamp XBIT piped (`0x20`) on one tile.
///
/// PE `PlacePipesAtCoordinates` (`0x44b829`) ORs this after committing an underground
/// pipe tile for water-pump / empty-land pipe placement.
pub(in crate::app) fn stamp_xbit_piped(city: &mut City, x: usize, y: usize) {
    // PE OR of XBIT 0x20 only — leave watered/powerable/XTRF alone.
    let bits = city
        .tile_bits_at(x, y)
        .unwrap_or(TileBits::new(0))
        .with_piped(true);
    city.set_tile_bits(x, y, bits);
}

/// Stamp `XBIT_POWERABLE` (`0x80`) on one tile.
///
/// PE `FUN_0044A8A0` / sc2kfix `PlacePowerLineTile` set this when committing power
/// lines and power crossovers; plant placement (`0x441640`) sets it on plant tiles.
pub(in crate::app) fn stamp_xbit_powerable(city: &mut City, x: usize, y: usize) {
    // PE OR of XBIT 0x80 only — leave watered/piped/XTRF alone.
    let bits = city
        .tile_bits_at(x, y)
        .unwrap_or(TileBits::new(0))
        .with_powerable(true);
    city.set_tile_bits(x, y, bits);
}

/// Stamp `XBIT_POWERABLE` on a committed power-plant footprint (PE source-side gate).
pub(in crate::app) fn stamp_power_plant_xbit_powerable(
    city: &mut City,
    footprint: &[(usize, usize)],
) {
    for &(x, y) in footprint {
        stamp_xbit_powerable(city, x, y);
    }
}

#[cfg(test)]
pub(in crate::app) fn pe_seed_power_plant_altitude_for_test(
    city: &mut City,
    x: usize,
    y: usize,
    altitude: u8,
) {
    city.set_altitude(x, y, altitude, city.water_height_at(x, y));
}

/// PE `0xC8` plant output in Rust tile units (`PE units × 16`).
pub(in crate::app) fn pe_power_c8_output_tiles(
    city: &City,
    x: usize,
    y: usize,
    rand_remainder: u32,
) -> u32 {
    pe_power_c8_pe_units(city, x, y, rand_remainder).saturating_mul(16)
}

/// PE `0xCC` plant output in Rust tile units (`PE units × 16`).
pub(in crate::app) fn pe_power_cc_output_tiles(city: &City, rand_remainder: u32) -> u32 {
    pe_power_cc_pe_units(city, rand_remainder).saturating_mul(16)
}

pub(in crate::app) fn power_plant_output_tiles(city: &City, x: usize, y: usize) -> u32 {
    let mut rng = pe_power_recalc_rng(city);
    power_plant_output_tiles_in_scan_order(city, x, y, &mut rng)
}

/// PE scans plants in map order; earlier `0xC8`/`0xCC` plants consume CRT rolls first.
pub(in crate::app) fn power_plant_output_tiles_in_scan_order(
    city: &City,
    target_x: usize,
    target_y: usize,
    rng: &mut Win95CrtRand,
) -> u32 {
    let mut seen = vec![false; CITY_TILES];
    for y in 0..CITY_HEIGHT {
        for x in 0..CITY_WIDTH {
            let Some((origin_x, origin_y)) = power_plant_footprint_origin(city, x, y) else {
                continue;
            };
            let origin_index = origin_y * CITY_WIDTH + origin_x;
            if seen[origin_index] {
                continue;
            }
            seen[origin_index] = true;
            let output = power_plant_output_tiles_with_rng(city, origin_x, origin_y, rng);
            if origin_x == target_x && origin_y == target_y {
                return output;
            }
        }
    }
    0
}

pub(in crate::app) fn power_plant_output_tiles_with_rng(
    city: &City,
    x: usize,
    y: usize,
    rng: &mut Win95CrtRand,
) -> u32 {
    if !pe_power_tile_source_eligible(city, x, y) {
        return 0;
    }
    match city.building_at(x, y) {
        198 => 40,
        199 => 40,
        200 => pe_power_c8_output_tiles(city, x, y, pe_power_crt_rand_u16(rng)),
        201 => 176,
        202 => 768,
        203 => 1_776,
        204 => pe_power_cc_output_tiles(city, pe_power_crt_rand_u16(rng)),
        205 => 5_680,
        206 => 8_880,
        207 => 704,
        _ => 0,
    }
}

/// PE 0x467b6f counts developed XBLD below the plant band. The
/// out-of-table branch at 0x467ac2 also counts XBLD above it.
pub(in crate::app) fn pe_power_consumer_xbld(building: u8) -> bool {
    matches!(building, 0x70..=0xc5 | 0xd0..=0xff)
}

/// PE flood-fill consumer predicate: XBLD band only (zones use building `0`).
pub(in crate::app) fn pe_power_flood_fill_consumer_tile(city: &City, x: usize, y: usize) -> bool {
    let building = city.building_at(x, y);
    pe_power_consumer_xbld(building)
        && !NetworkKind::Power.contains(building)
        && !is_abandoned_building(building)
}

/// PE `FUN_00467860` source scan range: `0xC6..=0xCF`.
pub(in crate::app) fn pe_power_source_xbld(building: u8) -> bool {
    (0xC6..=0xCF).contains(&building)
}

pub(in crate::app) fn power_demand_tiles(city: &City) -> u32 {
    let mut demand = 0;
    for y in 0..CITY_HEIGHT {
        for x in 0..CITY_WIDTH {
            if power_consuming_tile(city, x, y) {
                demand += 1;
            }
        }
    }
    demand
}

/// PE flood-fill consumer demand from XBLD, excluding bare zones.
pub(in crate::app) fn pe_power_demand_tiles(city: &City) -> u32 {
    let mut demand = 0;
    for y in 0..CITY_HEIGHT {
        for x in 0..CITY_WIDTH {
            if pe_power_flood_fill_consumer_tile(city, x, y) {
                demand += 1;
            }
        }
    }
    demand
}

pub(in crate::app) fn power_consuming_tile(city: &City, x: usize, y: usize) -> bool {
    let building = city.building_at(x, y);
    structural_powerable_tile(city, x, y)
        && !NetworkKind::Power.contains(building)
        && !is_power_plant_building(building)
        && !is_abandoned_building(building)
}

pub(in crate::app) fn structural_powerable_tile(city: &City, x: usize, y: usize) -> bool {
    let building = city.building_at(x, y);
    if is_tree_building(building) {
        return zone_serviceable_tile(city, x, y);
    }
    if is_road_tile(building) || is_rail_tile(building) || is_highway_tile(building) {
        return false;
    }
    building != 0 || zone_serviceable_tile(city, x, y)
}

pub(in crate::app) fn zone_serviceable_tile(city: &City, x: usize, y: usize) -> bool {
    !is_water_terrain(city.terrain_at(x, y))
        && city
            .zone_at(x, y)
            .is_some_and(|zone| matches!(zone.zone_id(), 1..=9))
}

pub(in crate::app) fn is_power_plant_building(building: u8) -> bool {
    matches!(building, 198 | 199 | 200 | 201..=207)
}

pub(in crate::app) fn complete_building_footprint_at_origin(
    city: &City,
    origin_x: usize,
    origin_y: usize,
    building: u8,
    size: usize,
) -> bool {
    origin_x + size <= CITY_WIDTH
        && origin_y + size <= CITY_HEIGHT
        && (origin_y..origin_y + size)
            .all(|y| (origin_x..origin_x + size).all(|x| city.building_at(x, y) == building))
}

pub(in crate::app) fn complete_connected_building_footprint_at_origin(
    city: &City,
    connected_tiles: &[bool],
    origin_x: usize,
    origin_y: usize,
    building: u8,
    size: usize,
) -> bool {
    complete_building_footprint_at_origin(city, origin_x, origin_y, building, size)
        && (origin_y..origin_y + size)
            .all(|y| (origin_x..origin_x + size).all(|x| connected_tiles[y * CITY_WIDTH + x]))
}

pub(in crate::app) fn complete_building_footprint_containing(
    city: &City,
    x: usize,
    y: usize,
    building: u8,
    size: usize,
) -> bool {
    let min_x = x.saturating_sub(size.saturating_sub(1));
    let min_y = y.saturating_sub(size.saturating_sub(1));
    (min_y..=y).any(|origin_y| {
        (min_x..=x).any(|origin_x| {
            complete_building_footprint_at_origin(city, origin_x, origin_y, building, size)
        })
    })
}

pub(in crate::app) fn building_footprint_origin_containing(
    city: &City,
    x: usize,
    y: usize,
    building: u8,
    size: usize,
) -> Option<(usize, usize)> {
    let min_x = x.saturating_sub(size.saturating_sub(1));
    let min_y = y.saturating_sub(size.saturating_sub(1));
    for origin_y in min_y..=y {
        for origin_x in min_x..=x {
            if complete_building_footprint_at_origin(city, origin_x, origin_y, building, size) {
                return Some((origin_x, origin_y));
            }
        }
    }
    None
}

pub(in crate::app) fn building_requires_saltwater(building: u8) -> bool {
    building == 250
}

pub(in crate::app) fn building_requires_mixed_shoreline(building: u8) -> bool {
    building == 248
}

pub(in crate::app) fn building_requires_flat_land(building: u8) -> bool {
    !building_requires_mixed_shoreline(building)
}

pub(in crate::app) fn building_has_internal_pipes(building: u8) -> bool {
    // PE subway-station place clears XBIT piped; stations are transit access, not pipe nodes.
    if matches!(building, 108..=111 | 233) {
        return false;
    }
    // Hydro generators (198/199) use the dedicated MapTool path: it sets the
    // powerable bit and does not stamp the generic building-plumbing bit.
    if matches!(building, 198 | 199) {
        return false;
    }
    building != 0
        && !is_tree_building(building)
        && !is_power_tile(building)
        && !is_road_tile(building)
        && !is_rail_tile(building)
        && !is_highway_tile(building)
}
