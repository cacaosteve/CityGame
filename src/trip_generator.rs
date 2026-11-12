//! Multi-mode trip generator grounded in sc2kfix `L_RunTripGenerator`.
//!
//! Finds transit within 3 tiles, walks road / highway / rail / subway toward
//! complementary zone destinations, and on success bumps microsim `XTRF` cells
//! along road/highway/bridge segments (`x/2`, `y/2` semantics).

use crate::city::{CITY_HEIGHT, CITY_TILES, CITY_WIDTH, City, ZONE_CORNER_ALL};
use crate::win95_random::Win95CrtRand;

/// Win95 `GROWTH_TILE_MAX_TRIP_STEPS`.
pub const TRIP_MAX_STEPS: u16 = 100;

/// Win95 `wPositionAngle` (`GetPopulatedTileAndLevel` / `XZONCornerCheck`).
const PE_POSITION_ANGLE: [u8; 4] = [0x80, 0x10, 0x20, 0x40];

/// sc2kfix `dwTripStartingCoords` — search ring within 3 tiles.
const TRIP_START_COORDS: [(i32, i32); 24] = [
    (0, 1),
    (1, 0),
    (0, -1),
    (-1, 0),
    (0, 2),
    (2, 0),
    (0, -2),
    (-2, 0),
    (0, 3),
    (3, 0),
    (0, -3),
    (-3, 0),
    (1, 1),
    (-1, 1),
    (1, -1),
    (-1, -1),
    (2, 1),
    (-2, 1),
    (2, -1),
    (-2, -1),
    (1, 2),
    (-1, 2),
    (1, -2),
    (-1, -2),
];

/// Cardinal steps matching Win95 `wTripX` / `wTripY` (N, E, S, W).
const TRIP_DIRS: [(i32, i32); 4] = [(0, -1), (1, 0), (0, 1), (-1, 0)];

/// Win95 `wArrZoneDestinations` @ `0x4E8570` — bit `z` set means zone id `z` is
/// a valid destination. Indexed by origin zone type (1..=6 used for RCI).
const ZONE_DEST_MASKS: [u16; 16] = [
    0xffff, 0xfff8, 0xfff8, 0xffe6, 0xffe6, 0xff9e, 0xff9e, 0x0000, 0x0001, 0x0001, 0x0003, 0x0006,
    0x000c, 0x0000, 0x070b, 0x0d0e,
];

/// Win95 `byte_4E858C`: remaining dir bits after traveling N/E/S/W (no reverse).
/// `{0x0B, 0x07, 0x0E, 0x0D}` — N keeps N|E|W, E keeps N|E|S, etc.
const TRIP_REMAINING_AFTER_DIR: [u8; 4] = [0x0b, 0x07, 0x0e, 0x0d];

/// Isolation key for the CRT draws within one trip.
const TRIP_PREFER_DIR_KEY: u8 = 0x87;

fn trip_isolated_rng(x: usize, y: usize, origin_zone: u8, pop_level: u8, key: u8) -> Win95CrtRand {
    Win95CrtRand::new(
        Win95CrtRand::DEFAULT_SEED
            .wrapping_mul(0x9E37_79B9)
            .wrapping_add(x as u32)
            .wrapping_add((y as u32) << 8)
            .wrapping_add(u32::from(pop_level) << 16)
            .wrapping_add(u32::from(origin_zone) << 24)
            .wrapping_add(u32::from(key) << 12),
    )
}

fn trip_prefer_rng(x: usize, y: usize, origin_zone: u8, pop_level: u8) -> Win95CrtRand {
    trip_isolated_rng(x, y, origin_zone, pop_level, TRIP_PREFER_DIR_KEY)
}

fn trip_roll_outer_facing(rng: &mut Win95CrtRand) -> usize {
    // Win95 outer walk: `iMaybeDirection = rand() & 3` (not the last successful dir).
    usize::from(rng.random_u16_mask(3))
}

fn trip_rotate_dir(dir: usize, rotation_step: usize) -> usize {
    // Win95 inner walk: `dir = (dir + wDunno3) & 3` before each try.
    (dir + rotation_step) & 3
}

fn trip_prefer_dir(x: usize, y: usize, origin_zone: u8, pop_level: u8) -> usize {
    let mut rng = trip_prefer_rng(x, y, origin_zone, pop_level);
    let _ = trip_rotation_step(&mut rng);
    trip_roll_outer_facing(&mut rng)
}

fn trip_rotation_step(rng: &mut Win95CrtRand) -> usize {
    // Win95: `wDunno3 = 2 * (rand() & 1) + 1` → 1 or 3 (clockwise vs counterclockwise).
    2 * usize::from(rng.random_u16_mask(1)) + 1
}

/// Win95 neighbor-connection XTXT overlay (`bTextOverlay == 250`).
/// `L_RunTripGenerator` treats an out-of-bounds step from this tile as success.
pub const TRIP_NEIGHBOR_CONNECTION_OVERLAY: u8 = 250;

/// Win95 tunnel-walk XBIT test in `RunTripGenerator` (`0x457a4b`, `0x457d14`):
/// any of bits 2..=6 keeps the walk in its current tunnel mode.
const TRIP_TUNNEL_XBIT_MASK: u8 = 0x7c;

/// Win95 desal building id; road/pedestrian/rail treat `XBLD > 0xFA` (arcos / llama)
/// as an immediate trip success.
const TRIP_DESAL_BUILDING: u8 = 250;

/// Project tile IDs (Mac/OpenCity2k catalog used by this viewer).
const SUBWAY_STATION: u8 = 233;
const BUS_DEPOT: u8 = 236;
const RAIL_STATION: u8 = 237;

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TransitMode {
    Road,
    Highway,
    Tunnel,
    RoadBridge,
    Pedestrian,
    PedestrianHighway,
    PedestrianTunnel,
    PedestrianBridge,
    Bus,
    TransitTransfer,
    RailEnter,
    SubwayEnter,
    Rail,
    Subway,
}

impl TransitMode {
    /// Win95 only writes XTRF for road, highway, and road-bridge steps.
    fn bumps_xtrf(self) -> bool {
        matches!(self, Self::Road | Self::Highway | Self::RoadBridge)
    }

    /// Win95 passenger flags: `TRANSIT_TYPE_BUS` / `RAIL_ENTER` / `SUBWAY_ENTER`.
    fn marks_bus(self) -> bool {
        matches!(self, Self::Bus)
    }

    fn marks_rail(self) -> bool {
        matches!(self, Self::RailEnter)
    }

    fn marks_subway(self) -> bool {
        matches!(self, Self::SubwayEnter)
    }
}

#[derive(Clone, Debug, Default)]
pub struct TripResult {
    pub success: bool,
    pub path: Vec<(usize, usize)>,
    /// Road / highway / bridge tiles that should congest XTRF.
    pub traffic_path: Vec<(usize, usize)>,
    /// Win95 `bUsedBus` / `bUsedRail` / `bUsedSubway` on a completed trip.
    pub used_bus: bool,
    pub used_rail: bool,
    pub used_subway: bool,
}

#[derive(Clone, Debug, Default)]
pub struct MonthlyTripStats {
    pub trips_attempted: usize,
    pub trips_succeeded: usize,
    pub tiles_bumped: usize,
    /// Win95 `dwBusPassengers` / `dwRailPassengers` / `dwSubwayPassengers` for this month.
    pub bus_passengers: u32,
    pub rail_passengers: u32,
    pub subway_passengers: u32,
}

fn is_road_tile(building: u8) -> bool {
    // PE route-walk road admission: 0x1d..=0x2b, 0x3f..=0x46,
    // 0x4b..=0x4c, and 0x5d..=0x60. Callers handle the tunnel, bridge,
    // and ramp mode transitions before using this shared road fallback.
    matches!(building, 0x1d..=0x2b | 0x3f..=0x46 | 0x4b..=0x4c | 0x5d..=0x60)
}

fn is_tunnel_tile(building: u8) -> bool {
    matches!(building, 63..=66)
}

fn is_road_bridge_tile(building: u8) -> bool {
    matches!(building, 0x51..=0x5c | 0x6a..=0x6b)
}

/// Win95 highway-walk range: `0x49..0x51` and `0x61..0x6C`.
fn is_highway_walkable(building: u8) -> bool {
    matches!(building, 73..=80 | 97..=107)
}

/// Win95 onramp/exit-to-road range while on highway: `0x5D..0x61`.
fn is_highway_to_road(building: u8) -> bool {
    matches!(building, 93..=96)
}

fn is_onramp_tile(building: u8) -> bool {
    matches!(building, 0x5d..=0x60)
}

fn is_rail_tile(building: u8) -> bool {
    matches!(building, 0x2c..=0x3e | 0x45..=0x48 | 0x4d..=0x4e | 0x6c..=0x6f)
}

fn is_subway_underground(tile: u8) -> bool {
    // sc2kfix: non-zero and < UNDER_TILE_PIPES_LR (16), plus crossovers / entrance / silo.
    (tile > 0 && tile < 16) || matches!(tile, 31 | 32 | 34 | 35)
}

fn start_mode_for_building(building: u8) -> Option<TransitMode> {
    match building {
        BUS_DEPOT => Some(TransitMode::Bus),
        RAIL_STATION => Some(TransitMode::RailEnter),
        SUBWAY_STATION => Some(TransitMode::SubwayEnter),
        b if is_valid_transit_item_tile(b) => Some(TransitMode::Road),
        _ => None,
    }
}

fn zone_accepts_destination(origin_zone: u8, dest_zone: u8) -> bool {
    let Some(mask) = ZONE_DEST_MASKS.get(usize::from(origin_zone)) else {
        return false;
    };
    (*mask & (1u16 << (dest_zone & 0x0f))) != 0
}

fn in_bounds(x: i32, y: i32) -> bool {
    x >= 0 && y >= 0 && (x as usize) < CITY_WIDTH && (y as usize) < CITY_HEIGHT
}

fn find_trip_start(city: &City, x: usize, y: usize) -> Option<(usize, usize, TransitMode)> {
    for &(dx, dy) in &TRIP_START_COORDS {
        let nx = x as i32 + dx;
        let ny = y as i32 + dy;
        if !in_bounds(nx, ny) {
            continue;
        }
        let (nx, ny) = (nx as usize, ny as usize);
        let building = city.building_at(nx, ny);
        if let Some(mode) = start_mode_for_building(building) {
            return Some((nx, ny, mode));
        }
    }
    None
}

fn neighbor_is_destination(city: &City, origin_zone: u8, x: usize, y: usize) -> bool {
    city.zone_at(x, y)
        .map(|zone| {
            let dest = zone.zone_id();
            dest != 0 && zone_accepts_destination(origin_zone, dest)
        })
        .unwrap_or(false)
}

/// Try to advance one step under the current transit mode.
/// Returns `(new_mode, step_cost)` when the neighbor is traversable.
fn try_step(
    city: &City,
    mode: TransitMode,
    origin_zone: u8,
    nx: usize,
    ny: usize,
) -> Option<(TransitMode, u16, bool)> {
    let building = city.building_at(nx, ny);
    let underground = city.underground_at(nx, ny);

    if matches!(
        mode,
        TransitMode::Road
            | TransitMode::RoadBridge
            | TransitMode::Pedestrian
            | TransitMode::PedestrianBridge
            | TransitMode::Bus
            | TransitMode::TransitTransfer
    ) && neighbor_is_destination(city, origin_zone, nx, ny)
    {
        return Some((mode, 0, true));
    }

    match mode {
        TransitMode::Road | TransitMode::Pedestrian => {
            let pedestrian = mode == TransitMode::Pedestrian;
            if is_tunnel_tile(building) {
                return Some((
                    if pedestrian {
                        TransitMode::PedestrianTunnel
                    } else {
                        TransitMode::Tunnel
                    },
                    if pedestrian { 2 } else { 3 },
                    false,
                ));
            }
            if is_road_bridge_tile(building) {
                return Some((
                    if pedestrian {
                        TransitMode::PedestrianBridge
                    } else {
                        TransitMode::RoadBridge
                    },
                    if pedestrian { 2 } else { 3 },
                    false,
                ));
            }
            if is_onramp_tile(building) {
                return Some((
                    if pedestrian {
                        TransitMode::PedestrianHighway
                    } else {
                        TransitMode::Highway
                    },
                    2,
                    false,
                ));
            }
            if is_road_tile(building) {
                return Some((mode, if pedestrian { 2 } else { 3 }, false));
            }
            match building {
                BUS_DEPOT if pedestrian => Some((TransitMode::TransitTransfer, 4, false)),
                BUS_DEPOT => Some((TransitMode::Bus, 4, false)),
                RAIL_STATION => Some((TransitMode::RailEnter, 4, false)),
                SUBWAY_STATION => Some((TransitMode::SubwayEnter, 4, false)),
                b if b > TRIP_DESAL_BUILDING => Some((mode, 0, true)),
                _ => None,
            }
        }
        TransitMode::Highway => {
            if is_highway_walkable(building) {
                Some((TransitMode::Highway, 1, false))
            } else if is_highway_to_road(building) {
                Some((TransitMode::Road, 1, false))
            } else {
                None
            }
        }
        TransitMode::PedestrianHighway => {
            if is_highway_walkable(building) {
                Some((TransitMode::PedestrianHighway, 1, false))
            } else if is_highway_to_road(building) {
                Some((TransitMode::Pedestrian, 1, false))
            } else {
                None
            }
        }
        TransitMode::Tunnel => {
            if city
                .tile_bits_at(nx, ny)
                .is_some_and(|bits| bits.raw_bits() & TRIP_TUNNEL_XBIT_MASK != 0)
            {
                Some((TransitMode::Tunnel, 3, false))
            } else if is_road_tile(building) {
                Some((TransitMode::Road, 3, false))
            } else {
                None
            }
        }
        TransitMode::PedestrianTunnel => {
            if city
                .tile_bits_at(nx, ny)
                .is_some_and(|bits| bits.raw_bits() & TRIP_TUNNEL_XBIT_MASK != 0)
            {
                Some((TransitMode::PedestrianTunnel, 2, false))
            } else if is_road_tile(building) {
                Some((TransitMode::Pedestrian, 2, false))
            } else {
                None
            }
        }
        TransitMode::RoadBridge => {
            if is_road_bridge_tile(building) {
                Some((TransitMode::RoadBridge, 3, false))
            } else if is_road_tile(building) {
                Some((TransitMode::Road, 3, false))
            } else {
                None
            }
        }
        TransitMode::PedestrianBridge => {
            if is_road_bridge_tile(building) {
                Some((TransitMode::PedestrianBridge, 2, false))
            } else if is_road_tile(building) {
                Some((TransitMode::Pedestrian, 2, false))
            } else {
                None
            }
        }
        TransitMode::Bus => {
            if building == BUS_DEPOT {
                Some((TransitMode::Bus, 4, false))
            } else if is_road_tile(building) {
                // Win95 drops to pedestrian after leaving the depot onto road.
                Some((TransitMode::Pedestrian, 2, false))
            } else {
                None
            }
        }
        TransitMode::TransitTransfer => {
            if building == BUS_DEPOT || building == RAIL_STATION {
                Some((TransitMode::TransitTransfer, 4, false))
            } else if is_road_tile(building) {
                // Win95 @ 0x457f47 resumes vehicle traffic after a transfer.
                Some((TransitMode::Road, 3, false))
            } else {
                None
            }
        }
        TransitMode::RailEnter => {
            if building == RAIL_STATION {
                Some((TransitMode::RailEnter, 4, false))
            } else if is_rail_tile(building) {
                Some((TransitMode::Rail, 1, false))
            } else {
                None
            }
        }
        TransitMode::SubwayEnter => {
            // The entry arm at 0x457fd6 reads only XUND.
            if is_subway_underground(underground) {
                Some((TransitMode::Subway, 1, false))
            } else {
                None
            }
        }
        TransitMode::Rail => {
            if building == RAIL_STATION {
                Some((TransitMode::TransitTransfer, 4, false))
            } else if is_rail_tile(building) {
                Some((TransitMode::Rail, 1, false))
            } else if building > TRIP_DESAL_BUILDING {
                Some((mode, 0, true))
            } else {
                None
            }
        }
        TransitMode::Subway => {
            if building == SUBWAY_STATION {
                Some((TransitMode::TransitTransfer, 4, false))
            } else if is_subway_underground(underground) {
                Some((TransitMode::Subway, 1, false))
            } else {
                None
            }
        }
    }
}

/// Multi-mode trip walk from an RCI origin tile.
///
/// Uses a Win95-style direction-mask stack: when a branch dead-ends, pop and try
/// remaining exits from earlier tiles instead of failing the whole trip.
pub fn run_trip(city: &City, x: usize, y: usize, origin_zone: u8, pop_level: u8) -> TripResult {
    let mut trip_rng = trip_prefer_rng(x, y, origin_zone, pop_level);
    run_trip_with_rng(city, x, y, origin_zone, pop_level, &mut trip_rng)
}

/// Win95 trip walk using the caller's process-wide CRT stream.
///
/// `FUN_004358B0` invokes `FUN_00457620` from the monthly growth scan, and the
/// route walker calls CRT `rand()` directly. Production callers must pass the
/// same stream used by the surrounding tile sequence.
pub fn run_trip_with_rng(
    city: &City,
    x: usize,
    y: usize,
    origin_zone: u8,
    pop_level: u8,
    trip_rng: &mut Win95CrtRand,
) -> TripResult {
    let Some((sx, sy, start_mode)) = find_trip_start(city, x, y) else {
        return TripResult::default();
    };

    let mut max_steps = TRIP_MAX_STEPS;
    if pop_level == 1 {
        max_steps -= max_steps / 4;
    }

    #[derive(Clone)]
    struct Frame {
        x: usize,
        y: usize,
        mode: TransitMode,
        steps: u16,
        /// Bitmask of cardinal dirs still to try from this tile (N=1,E=2,S=4,W=8).
        remaining: u8,
        path_len: usize,
        traffic_len: usize,
        used_bus: bool,
        used_rail: bool,
        used_subway: bool,
    }

    // Win95 after a successful step: road and pedestrian bridges keep only
    // the travel dir, subway-enter resets to all four, otherwise
    // `byte_4E858C[dir]`.
    let child_remaining = |mode: TransitMode, d: usize| -> u8 {
        if matches!(
            mode,
            TransitMode::RoadBridge | TransitMode::PedestrianBridge
        ) {
            1u8 << d
        } else if matches!(mode, TransitMode::SubwayEnter) {
            0b1111
        } else {
            TRIP_REMAINING_AFTER_DIR[d]
        }
    };

    let mut path = vec![(sx, sy)];
    let mut traffic_path = Vec::new();
    if start_mode.bumps_xtrf() {
        traffic_path.push((sx, sy));
    }

    // PE keeps only the active route stack (coordinate ring plus per-depth
    // mode/direction/cost arrays). Do not globally prune a coordinate/mode:
    // backtracking restores the parent frame and may revisit that state.
    let mut stack = vec![Frame {
        x: sx,
        y: sy,
        mode: start_mode,
        steps: 0,
        remaining: 0b1111,
        path_len: 1,
        traffic_len: traffic_path.len(),
        used_bus: start_mode.marks_bus(),
        used_rail: start_mode.marks_rail(),
        used_subway: start_mode.marks_subway(),
    }];

    // Win95: each outer walk step `iMaybeDirection = rand() & 3`, then inner
    // `dir = (dir + wDunno3) & 3` with `wDunno3 = 2*(rand&1)+1` (add before first try).
    let rotation_step = trip_rotation_step(trip_rng);
    let mut used_bus = start_mode.marks_bus();
    let mut used_rail = start_mode.marks_rail();
    let mut used_subway = start_mode.marks_subway();

    while let Some(frame) = stack.last_mut() {
        // PE compares the accumulated weighted cost against the budget before
        // selecting another facing. Exhaustion returns failure for the whole
        // route; it does not backtrack to a parent with unused directions.
        if frame.steps >= max_steps {
            return TripResult {
                success: false,
                path,
                traffic_path,
                used_bus: frame.used_bus,
                used_rail: frame.used_rail,
                used_subway: frame.used_subway,
            };
        }
        if frame.remaining == 0 {
            stack.pop();
            if let Some(parent) = stack.last() {
                path.truncate(parent.path_len);
                traffic_path.truncate(parent.traffic_len);
                used_bus = parent.used_bus;
                used_rail = parent.used_rail;
                used_subway = parent.used_subway;
            }
            continue;
        }

        let mut advanced = false;
        let mut prefer_dir = trip_roll_outer_facing(trip_rng);
        for _ in 0..4 {
            prefer_dir = trip_rotate_dir(prefer_dir, rotation_step);
            let d = prefer_dir;
            let bit = 1u8 << d;
            if frame.remaining & bit == 0 {
                continue;
            }
            frame.remaining &= !bit;

            let (dx, dy) = TRIP_DIRS[d];
            let nx = frame.x as i32 + dx;
            let ny = frame.y as i32 + dy;
            if !in_bounds(nx, ny) {
                // Win95: next x/y `>= 128` (LOWORD wrap makes west/north -1 look huge)
                // succeeds when the current tile's XTXT overlay is 250.
                if city.text_overlay_at(frame.x, frame.y) == Some(TRIP_NEIGHBOR_CONNECTION_OVERLAY)
                {
                    return TripResult {
                        success: true,
                        path,
                        traffic_path,
                        used_bus: frame.used_bus,
                        used_rail: frame.used_rail,
                        used_subway: frame.used_subway,
                    };
                }
                continue;
            }
            let (nx, ny) = (nx as usize, ny as usize);
            let Some((next_mode, cost, success)) = try_step(city, frame.mode, origin_zone, nx, ny)
            else {
                continue;
            };
            if success {
                path.push((nx, ny));
                return TripResult {
                    success: true,
                    path,
                    traffic_path,
                    used_bus: frame.used_bus,
                    used_rail: frame.used_rail,
                    used_subway: frame.used_subway,
                };
            }

            let next_steps = frame.steps.saturating_add(cost.max(1));

            path.push((nx, ny));
            if next_mode.bumps_xtrf() {
                traffic_path.push((nx, ny));
            }
            let next_bus = frame.used_bus || next_mode.marks_bus();
            let next_rail = frame.used_rail || next_mode.marks_rail();
            let next_subway = frame.used_subway || next_mode.marks_subway();

            let parent = Frame {
                x: frame.x,
                y: frame.y,
                mode: frame.mode,
                steps: frame.steps,
                remaining: frame.remaining,
                path_len: frame.path_len,
                traffic_len: frame.traffic_len,
                used_bus: frame.used_bus,
                used_rail: frame.used_rail,
                used_subway: frame.used_subway,
            };
            *frame = parent;

            stack.push(Frame {
                x: nx,
                y: ny,
                mode: next_mode,
                steps: next_steps,
                remaining: child_remaining(next_mode, d),
                path_len: path.len(),
                traffic_len: traffic_path.len(),
                used_bus: next_bus,
                used_rail: next_rail,
                used_subway: next_subway,
            });
            used_bus = next_bus;
            used_rail = next_rail;
            used_subway = next_subway;
            advanced = true;
            break;
        }

        if !advanced {
            stack.pop();
            if let Some(parent) = stack.last() {
                path.truncate(parent.path_len);
                traffic_path.truncate(parent.traffic_len);
                used_bus = parent.used_bus;
                used_rail = parent.used_rail;
                used_subway = parent.used_subway;
            }
        }
    }

    TripResult {
        success: false,
        path,
        traffic_path,
        used_bus,
        used_rail,
        used_subway,
    }
}

mod simulation;
pub use simulation::*;
