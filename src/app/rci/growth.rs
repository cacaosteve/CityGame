use super::*;

pub(in crate::app) fn rci_growth_current_demand(demand: i32) -> i32 {
    demand.saturating_add(2000).clamp(0, 4000)
}

/// PE growth-tick demand pair after the powered-trip gate in the tile-growth hook.
///
/// Init before the trip: `(iCurrentDemand, iRemainderDemand) = (0, 4000)`.
/// On powered trip success: current = `wCityDemand[RCI] + 2000`, remainder =
/// `4000 - current`. Failed or unpowered trips leave the init pair in place.
///
/// StartBuilding (`3 * current / (pop+1)`) and abandon-redevelop
/// (`15 * current / pop`) consume current; BUILD_START ChangeBuilding consumes
/// `remainder / pop`. Live StartBuilding/abandon paths hard-gate on trip access,
/// which is equivalent to PE's `current == 0` thresholds when the trip fails.
pub(in crate::app) fn rci_growth_trip_demand_pair(demand: i32, trip_succeeded: bool) -> (i32, u32) {
    if trip_succeeded {
        let current = rci_growth_current_demand(demand);
        (current, 4000u32.saturating_sub(current as u32))
    } else {
        (0, 4000)
    }
}

pub(in crate::app) fn rci_growth_start_threshold(demand: i32, pop: u8) -> u32 {
    let current = rci_growth_current_demand(demand) as u32;
    3 * current / (u32::from(pop) + 1)
}

pub(in crate::app) fn rci_construction_complete_threshold(pop: u8) -> u32 {
    16384 / u32::from(pop.max(1))
}

pub(in crate::app) fn rci_place_church_on_construction_complete(
    place_church_tick: bool,
    zone_id: u8,
    pop: u8,
) -> bool {
    // Win95 BUILD_THINK: `bPlaceChurch && (BP & 2) && zone < 3`.
    // For valid RCI zones, only residential (1/2) pop-2/3 construction qualifies.
    place_church_tick && (pop & 2) != 0 && zone_id < ZONE_COMMERCIAL_LIGHT
}

/// Win95 `IsTileMultipliedThresholdReached(CHURCH, dwCityPopulation, LESS, 2500)`:
/// `church_tiles * 2500 < dwCityPopulation` (`0x4caa74`). Tile count, not buildings.
pub(in crate::app) fn pe_b_place_church(church_tiles: u32, dw_city_population: u32) -> bool {
    church_tiles.saturating_mul(CHURCH_POPULATION_STEP) < dw_city_population
}

pub(in crate::app) fn church_tile_count_for_city(city: &City) -> u32 {
    let mut tiles = 0_u32;
    for y in 0..CITY_HEIGHT {
        for x in 0..CITY_WIDTH {
            if city.building_at(x, y) == CHURCH_BUILDING_ID {
                tiles = tiles.saturating_add(1);
            }
        }
    }
    tiles
}

pub(in crate::app) fn rci_abandoned_redevelop_threshold(demand: i32, pop: u8) -> u32 {
    let current = rci_growth_current_demand(demand) as u32;
    15 * current / u32::from(pop.max(1))
}

/// Win95 `iRemainderDemand` before GROWTH_CHANGE: see [`rci_growth_trip_demand_pair`].
pub(in crate::app) fn rci_growth_remainder_demand(demand: i32, trip_succeeded: bool) -> u32 {
    rci_growth_trip_demand_pair(demand, trip_succeeded).1
}

/// Win95 GROWTH_CHANGE: `iDemandThreshold = iRemainderDemand / iTilePopLevel`.
pub(in crate::app) fn rci_growth_change_threshold(remainder: u32, pop: u8) -> u32 {
    remainder / u32::from(pop.max(1))
}

/// Isolation key for vacant StartBuilding `rand() < 3*current/(pop+1)` (pop NONE).
pub(in crate::app) const GROWTH_ROLL_START_VACANT_KEY: u8 = 0x89;
/// Isolation key for occupied StartBuilding size-up `rand() < 3*current/(pop+1)`.
pub(in crate::app) const GROWTH_ROLL_START_EXISTING_KEY: u8 = 0x8A;
/// Isolation key for BUILD_THINK complete `rand() < 16384/pop`.
pub(in crate::app) const GROWTH_ROLL_CONSTRUCTION_KEY: u8 = 0x8B;
/// Isolation key for BUILD_ABANDON redevelop `rand() < 15*current/pop`.
pub(in crate::app) const GROWTH_ROLL_ABANDON_REDEVELOP_KEY: u8 = 0x8C;
/// Isolation key for BUILD_START ChangeBuilding `rand() < iRemainderDemand/pop`.
pub(in crate::app) const GROWTH_ROLL_CHANGE_KEY: u8 = 0x8D;

pub(in crate::app) fn rci_growth_roll_crt(x: usize, y: usize, table_key: u8) -> u16 {
    rci_choose_and_place_isolated_rng(x, y, table_key, 0).random_u16()
}

#[cfg(test)]
thread_local! {
    static USE_LIVE_GROWTH_RNG: std::cell::Cell<bool> =
        const { std::cell::Cell::new(false) };
    static FAIL_CONSTRUCTION_COMPLETE_ROLL: std::cell::Cell<bool> =
        const { std::cell::Cell::new(false) };
    static FAIL_GROWTH_CHANGE_ROLL: std::cell::Cell<bool> =
        const { std::cell::Cell::new(false) };
    static FORCED_GROWTH_CHANGE_ROLL_USED: std::cell::Cell<bool> =
        const { std::cell::Cell::new(false) };
}

/// Use the production CRT stream when comparing a whole growth tick to the PE.
#[cfg(test)]
pub(in crate::app) struct LiveGrowthRng(bool);

#[cfg(test)]
impl LiveGrowthRng {
    pub(in crate::app) fn enable() -> Self {
        Self(USE_LIVE_GROWTH_RNG.with(|cell| cell.replace(true)))
    }
}

#[cfg(test)]
impl Drop for LiveGrowthRng {
    fn drop(&mut self) {
        USE_LIVE_GROWTH_RNG.with(|cell| cell.set(self.0));
    }
}

#[cfg(test)]
pub(in crate::app) fn use_isolated_growth_rng() -> bool {
    !USE_LIVE_GROWTH_RNG.with(std::cell::Cell::get)
}

/// Tests force growth commits (`roll = 0`). Arm this to exercise failed BUILD_THINK fall-through.
#[cfg(test)]
pub(in crate::app) struct FailConstructionCompleteRolls;

#[cfg(test)]
impl FailConstructionCompleteRolls {
    pub(in crate::app) fn arm() -> Self {
        FAIL_CONSTRUCTION_COMPLETE_ROLL.with(|cell| cell.set(true));
        Self
    }
}

#[cfg(test)]
impl Drop for FailConstructionCompleteRolls {
    fn drop(&mut self) {
        FAIL_CONSTRUCTION_COMPLETE_ROLL.with(|cell| cell.set(false));
    }
}

#[cfg(test)]
pub(in crate::app) struct FailGrowthChangeRolls;

#[cfg(test)]
impl FailGrowthChangeRolls {
    pub(in crate::app) fn arm() -> Self {
        FAIL_GROWTH_CHANGE_ROLL.with(|cell| cell.set(true));
        FORCED_GROWTH_CHANGE_ROLL_USED.with(|cell| cell.set(false));
        Self
    }

    pub(in crate::app) fn was_used(&self) -> bool {
        FORCED_GROWTH_CHANGE_ROLL_USED.with(std::cell::Cell::get)
    }
}

#[cfg(test)]
impl Drop for FailGrowthChangeRolls {
    fn drop(&mut self) {
        FAIL_GROWTH_CHANGE_ROLL.with(|cell| cell.set(false));
        FORCED_GROWTH_CHANGE_ROLL_USED.with(|cell| cell.set(false));
    }
}

pub(in crate::app) fn rci_growth_roll(x: usize, y: usize, table_key: u8) -> u16 {
    // Unit tests force commits; production matches Win95 growth CRT `rand` draws.
    #[cfg(test)]
    {
        if table_key == GROWTH_ROLL_CONSTRUCTION_KEY
            && FAIL_CONSTRUCTION_COMPLETE_ROLL.with(|cell| cell.get())
        {
            return u16::MAX;
        }
        if table_key == GROWTH_ROLL_CHANGE_KEY && FAIL_GROWTH_CHANGE_ROLL.with(|cell| cell.get()) {
            FORCED_GROWTH_CHANGE_ROLL_USED.with(|cell| cell.set(true));
            return u16::MAX;
        }
        return 0;
    }

    #[cfg(not(test))]
    {
        rci_growth_roll_crt(x, y, table_key)
    }
}

pub(in crate::app) fn rci_growth_roll_succeeds(
    threshold: u32,
    x: usize,
    y: usize,
    table_key: u8,
) -> bool {
    threshold > 0 && u32::from(rci_growth_roll(x, y, table_key)) < threshold
}

/// Live PE growth gate using the shared process CRT stream (`0x48cdd8`).
pub(in crate::app) fn pe_rci_growth_roll_succeeds_with_rng(
    threshold: u32,
    rng: &mut Win95CrtRand,
) -> bool {
    threshold > 0 && u32::from(rng.random_u16()) < threshold
}

/// These PE growth branches call `rand()` before comparing, even at threshold zero.
pub(in crate::app) fn pe_rci_growth_roll_succeeds_with_rng_always_draw(
    threshold: u32,
    rng: &mut Win95CrtRand,
) -> bool {
    u32::from(rng.random_u16()) < threshold
}

/// Tests keep isolation/zero-roll; live callers pass `Some(pe_process_crt)`.
pub(in crate::app) fn pe_rci_growth_roll_succeeds_optional(
    threshold: u32,
    x: usize,
    y: usize,
    table_key: u8,
    process_crt: Option<&mut Win95CrtRand>,
) -> bool {
    #[cfg(test)]
    if use_isolated_growth_rng() {
        return rci_growth_roll_succeeds(threshold, x, y, table_key);
    }
    match process_crt {
        Some(rng) => pe_rci_growth_roll_succeeds_with_rng(threshold, rng),
        None => rci_growth_roll_succeeds(threshold, x, y, table_key),
    }
}

/// BUILD_ABANDON's live roll consumes CRT `rand()` even for zero current demand.
pub(in crate::app) fn pe_rci_abandoned_redevelop_roll_succeeds_optional(
    threshold: u32,
    x: usize,
    y: usize,
    table_key: u8,
    process_crt: Option<&mut Win95CrtRand>,
) -> bool {
    #[cfg(test)]
    if use_isolated_growth_rng() {
        return rci_growth_roll_succeeds(threshold, x, y, table_key);
    }
    match process_crt {
        Some(rng) => pe_rci_growth_roll_succeeds_with_rng_always_draw(threshold, rng),
        None => rci_growth_roll_succeeds(threshold, x, y, table_key),
    }
}

/// BUILD_START also consumes its process CRT draw when current demand gives threshold zero.
pub(in crate::app) fn pe_rci_startbuilding_growth_roll_succeeds_optional(
    threshold: u32,
    x: usize,
    y: usize,
    table_key: u8,
    process_crt: Option<&mut Win95CrtRand>,
) -> bool {
    #[cfg(test)]
    if use_isolated_growth_rng() {
        return rci_growth_roll_succeeds(threshold, x, y, table_key);
    }
    match process_crt {
        Some(rng) => pe_rci_growth_roll_succeeds_with_rng_always_draw(threshold, rng),
        None => rci_growth_roll_succeeds(threshold, x, y, table_key),
    }
}

/// GROWTH_CHANGE draws for its threshold roll even at zero; only a pass draws `iReplaceTile`.
pub(in crate::app) fn pe_rci_growth_change_roll_then_replace_with_rng(
    threshold: u32,
    rng: &mut Win95CrtRand,
) -> Option<bool> {
    if !pe_rci_growth_roll_succeeds_with_rng_always_draw(threshold, rng) {
        return None;
    }
    Some(rng.random_u16_mask(1) != 0)
}

pub(in crate::app) fn pe_rci_growth_change_roll_then_replace_optional(
    threshold: u32,
    x: usize,
    y: usize,
    table_key: u8,
    process_crt: Option<&mut Win95CrtRand>,
) -> Option<bool> {
    #[cfg(test)]
    if use_isolated_growth_rng() {
        if !rci_growth_roll_succeeds(threshold, x, y, table_key) {
            return None;
        }
        return Some(rci_change_building_replace_tile(x, y));
    }
    match process_crt {
        Some(rng) => pe_rci_growth_change_roll_then_replace_with_rng(threshold, rng),
        None => {
            if !rci_growth_roll_succeeds(threshold, x, y, table_key) {
                return None;
            }
            Some(rci_change_building_replace_tile(x, y))
        }
    }
}

pub(in crate::app) fn rci_decline_building(size: usize, x: usize, y: usize) -> Option<u8> {
    abandoned_building_for_pop(size_to_default_pop_level(size)?, x, y)
}

/// Isolation key for growth-tick `iReplaceTile = rand() & 1` before ChangeBuilding.
pub(in crate::app) const CHANGEBUILDING_REPLACE_KEY: u8 = 0x85;
/// Isolation key for ChangeBuilding pop4 `rand() & 3` scatter origin.
pub(in crate::app) const CHANGEBUILDING_SCATTER_KEY: u8 = 0x86;

pub(in crate::app) fn rci_change_building_replace_tile(x: usize, y: usize) -> bool {
    // Win95 growth tick: `iReplaceTile = rand() & 1` then ChangeBuilding(..., iReplaceTile).
    rci_choose_and_place_isolated_rng(x, y, CHANGEBUILDING_REPLACE_KEY, 0).random_u16_mask(1) != 0
}

/// Live PE `iReplaceTile = rand() & 1` using process CRT (`0x48cdd8`).
pub(in crate::app) fn rci_change_building_replace_tile_optional(
    x: usize,
    y: usize,
    process_crt: Option<&mut Win95CrtRand>,
) -> bool {
    #[cfg(test)]
    if use_isolated_growth_rng() {
        return rci_change_building_replace_tile(x, y);
    }
    match process_crt {
        Some(rng) => rng.random_u16_mask(1) != 0,
        None => rci_change_building_replace_tile(x, y),
    }
}

fn rci_change_building_scatter_origin(x: usize, y: usize) -> (usize, usize) {
    // Win95 `0x437eb8`: r = rand()&3; PE origin is (x+(r&1), y-(r>>1)) from a max-y
    // 3×3 corner. Port footprints use min-y origin, so y+(r>>1) keeps the 2×2 inside.
    let r =
        rci_choose_and_place_isolated_rng(x, y, CHANGEBUILDING_SCATTER_KEY, 4).random_u16_mask(3);
    (x + usize::from(r & 1), y + usize::from(r >> 1))
}

fn rci_change_building_scatter_roll_optional(
    x: usize,
    y: usize,
    process_crt: Option<&mut Win95CrtRand>,
) -> u16 {
    #[cfg(test)]
    if use_isolated_growth_rng() {
        return rci_choose_and_place_isolated_rng(x, y, CHANGEBUILDING_SCATTER_KEY, 4)
            .random_u16_mask(3);
    }
    match process_crt {
        Some(rng) => rng.random_u16_mask(3),
        None => rci_choose_and_place_isolated_rng(x, y, CHANGEBUILDING_SCATTER_KEY, 4)
            .random_u16_mask(3),
    }
}

fn rci_change_building_scatter_origin_optional(
    x: usize,
    y: usize,
    process_crt: Option<&mut Win95CrtRand>,
) -> (usize, usize) {
    #[cfg(test)]
    if use_isolated_growth_rng() {
        return rci_change_building_scatter_origin(x, y);
    }
    match process_crt {
        Some(rng) => {
            let r = rng.random_u16_mask(3);
            (x + usize::from(r & 1), y + usize::from(r >> 1))
        }
        None => rci_change_building_scatter_origin(x, y),
    }
}

pub(in crate::app) fn rci_decline_change_building(
    building: u8,
    footprint: &[(usize, usize)],
    x: usize,
    y: usize,
    replace_tile: bool,
) -> Option<Vec<(usize, usize, u8)>> {
    rci_decline_change_building_optional(building, footprint, x, y, replace_tile, None)
}

pub(in crate::app) fn rci_decline_change_building_optional(
    building: u8,
    footprint: &[(usize, usize)],
    x: usize,
    y: usize,
    replace_tile: bool,
    mut process_crt: Option<&mut Win95CrtRand>,
) -> Option<Vec<(usize, usize, u8)>> {
    let pop = rci_building_pop_level(building)?;
    if !replace_tile || pop <= 1 {
        let abandoned = abandoned_building_for_pop_optional(pop, x, y, process_crt.as_deref_mut())?;
        return Some(vec![(x, y, abandoned)]);
    }
    match pop {
        2 => {
            // Split 2×2 into four abandon 1×1.
            let mut placements = Vec::with_capacity(4);
            for &(tx, ty) in footprint {
                let abandoned =
                    abandoned_building_for_pop_optional(1, tx, ty, process_crt.as_deref_mut())?;
                placements.push((tx, ty, abandoned));
            }
            Some(placements)
        }
        3 => {
            // Downgrade to abandon pop2 (same 2×2 footprint).
            let abandoned =
                abandoned_building_for_pop_optional(2, x, y, process_crt.as_deref_mut())?;
            Some(vec![(x, y, abandoned)])
        }
        4 => {
            // Scatter: 1×1 abandons on the 3×3 ring, then a pop3 abandon 2×2 overlay.
            let mut placements = Vec::with_capacity(9);
            for &(tx, ty) in footprint {
                if tx == x + 1 && ty == y + 1 {
                    continue; // center filled by the pop3 overlay
                }
                let abandoned =
                    abandoned_building_for_pop_optional(1, tx, ty, process_crt.as_deref_mut())?;
                placements.push((tx, ty, abandoned));
            }
            let (ox, oy) =
                rci_change_building_scatter_origin_optional(x, y, process_crt.as_deref_mut());
            let abandoned =
                abandoned_building_for_pop_optional(3, ox, oy, process_crt.as_deref_mut())?;
            placements.push((ox, oy, abandoned));
            Some(placements)
        }
        _ => {
            let abandoned =
                abandoned_building_for_pop_optional(pop, x, y, process_crt.as_deref_mut())?;
            Some(vec![(x, y, abandoned)])
        }
    }
}

/// Win95 `ChangeBuilding` pop-2 replacement calls, in PE order, from its raw
/// `GetPopulatedTileAndLevel` tile. Coordinates stay signed so edge calls still
/// consume their picker draw before ChooseAndPlace rejects an out-of-map tile.
pub(in crate::app) fn pop2_change_building_replace_call_coords(
    x: usize,
    y: usize,
) -> [(isize, isize); 4] {
    let x = x as isize;
    let y = y as isize;
    [(x, y), (x + 1, y), (x + 1, y - 1), (x, y - 1)]
}

pub(in crate::app) fn pe_changebuilding_pop2_replace_placements_with_picker(
    call_x: usize,
    call_y: usize,
    mut choose_building: impl FnMut(isize, isize) -> Option<u8>,
) -> Option<Vec<(usize, usize, u8)>> {
    let mut placements = Vec::with_capacity(4);
    for (x, y) in pop2_change_building_replace_call_coords(call_x, call_y) {
        let abandoned = choose_building(x, y)?;
        if (0..CITY_WIDTH as isize).contains(&x) && (0..CITY_HEIGHT as isize).contains(&y) {
            placements.push((x as usize, y as usize, abandoned));
        }
    }
    Some(placements)
}

/// Win95 pop-4 replacement's eight pop-1 ChooseAndPlace calls, in PE order,
/// relative to its raw GetPopulatedTileAndLevel coordinate.
pub(in crate::app) fn pop4_change_building_replace_call_coords(
    x: usize,
    y: usize,
) -> [(isize, isize); 8] {
    let x = x as isize;
    let y = y as isize;
    [
        (x, y),
        (x + 1, y),
        (x + 2, y),
        (x + 2, y - 1),
        (x + 2, y - 2),
        (x + 1, y - 2),
        (x, y - 2),
        (x, y - 1),
    ]
}

pub(in crate::app) fn pop4_change_building_scatter_call_coords(
    x: isize,
    y: isize,
    roll: u16,
) -> (isize, isize) {
    let roll = roll & 3;
    (
        x + isize::from((roll & 1) as u8),
        y - isize::from((roll >> 1) as u8),
    )
}

fn pop3_choose_and_place_stamp_origin_from_call(
    call_x: isize,
    call_y: isize,
) -> Option<(usize, usize)> {
    if !(0..CITY_WIDTH as isize).contains(&call_x)
        || !(0..CITY_HEIGHT as isize).contains(&call_y)
        || call_y == 0
    {
        return None;
    }
    Some((call_x as usize, (call_y - 1) as usize))
}

fn pop4_choose_and_place_stamp_origin_from_call(
    call_x: usize,
    call_y: usize,
) -> Option<(usize, usize)> {
    if call_x >= CITY_WIDTH || call_y >= CITY_HEIGHT {
        return None;
    }
    Some((call_x, call_y.checked_sub(2)?))
}

pub(in crate::app) fn pe_changebuilding_pop1_direct_placement_with_picker(
    call_x: usize,
    call_y: usize,
    choose_building: impl FnOnce() -> Option<u8>,
) -> Option<Vec<(usize, usize, u8)>> {
    let abandoned = choose_building()?;
    if call_x >= CITY_WIDTH || call_y >= CITY_HEIGHT {
        return Some(Vec::new());
    }
    Some(vec![(call_x, call_y, abandoned)])
}

pub(in crate::app) fn pe_changebuilding_pop2_direct_placement_with_picker(
    call_x: usize,
    call_y: usize,
    choose_building: impl FnOnce() -> Option<u8>,
) -> Option<Vec<(usize, usize, u8)>> {
    let abandoned = choose_building()?;
    let Some((stamp_x, stamp_y)) = pop2_choose_and_place_stamp_origin(call_x, call_y) else {
        return Some(Vec::new());
    };
    Some(
        pe_choose_and_place_then_origin_bounds(|| Some(abandoned), stamp_x, stamp_y, 2)
            .map(|building| vec![(stamp_x, stamp_y, building)])
            .unwrap_or_default(),
    )
}

fn pe_changebuilding_pop3_signed_direct_placement_with_picker(
    call_x: isize,
    call_y: isize,
    choose_building: impl FnOnce() -> Option<u8>,
) -> Option<Vec<(usize, usize, u8)>> {
    let abandoned = choose_building()?;
    let Some((stamp_x, stamp_y)) = pop3_choose_and_place_stamp_origin_from_call(call_x, call_y)
    else {
        return Some(Vec::new());
    };
    Some(
        pe_choose_and_place_then_origin_bounds(|| Some(abandoned), stamp_x, stamp_y, 3)
            .map(|building| vec![(stamp_x, stamp_y, building)])
            .unwrap_or_default(),
    )
}

pub(in crate::app) fn pe_changebuilding_pop3_direct_placement_with_picker(
    call_x: usize,
    call_y: usize,
    choose_building: impl FnOnce() -> Option<u8>,
) -> Option<Vec<(usize, usize, u8)>> {
    pe_changebuilding_pop3_signed_direct_placement_with_picker(
        call_x as isize,
        call_y as isize,
        choose_building,
    )
}

pub(in crate::app) fn pe_changebuilding_pop4_direct_placement_with_picker(
    call_x: usize,
    call_y: usize,
    choose_building: impl FnOnce() -> Option<u8>,
) -> Option<Vec<(usize, usize, u8)>> {
    let abandoned = choose_building()?;
    let Some((stamp_x, stamp_y)) = pop4_choose_and_place_stamp_origin_from_call(call_x, call_y)
    else {
        return Some(Vec::new());
    };
    Some(
        pe_choose_and_place_then_origin_bounds(|| Some(abandoned), stamp_x, stamp_y, 4)
            .map(|building| vec![(stamp_x, stamp_y, building)])
            .unwrap_or_default(),
    )
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::app) enum PeChangeBuildingPop4ReplaceStep {
    ChooseBuilding { pop: u8, x: isize, y: isize },
    ScatterRoll { x: usize, y: usize },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::app) enum PeChangeBuildingPop4ReplaceResult {
    Building(u8),
    ScatterRoll(u16),
}

pub(in crate::app) fn pe_changebuilding_pop4_replace_placements_with_picker(
    call_x: usize,
    call_y: usize,
    mut choose: impl FnMut(PeChangeBuildingPop4ReplaceStep) -> Option<PeChangeBuildingPop4ReplaceResult>,
) -> Option<Vec<(usize, usize, u8)>> {
    let mut placements = Vec::with_capacity(9);
    for (x, y) in pop4_change_building_replace_call_coords(call_x, call_y) {
        let PeChangeBuildingPop4ReplaceResult::Building(abandoned) =
            choose(PeChangeBuildingPop4ReplaceStep::ChooseBuilding { pop: 1, x, y })?
        else {
            return None;
        };
        if (0..CITY_WIDTH as isize).contains(&x) && (0..CITY_HEIGHT as isize).contains(&y) {
            placements.push((x as usize, y as usize, abandoned));
        }
    }

    let PeChangeBuildingPop4ReplaceResult::ScatterRoll(roll) =
        choose(PeChangeBuildingPop4ReplaceStep::ScatterRoll {
            x: call_x,
            y: call_y,
        })?
    else {
        return None;
    };
    let (scatter_call_x, scatter_call_y) =
        pop4_change_building_scatter_call_coords(call_x as isize, call_y as isize, roll);
    let scatter = pe_changebuilding_pop3_signed_direct_placement_with_picker(
        scatter_call_x,
        scatter_call_y,
        || match choose(PeChangeBuildingPop4ReplaceStep::ChooseBuilding {
            pop: 3,
            x: scatter_call_x,
            y: scatter_call_y,
        })? {
            PeChangeBuildingPop4ReplaceResult::Building(building) => Some(building),
            PeChangeBuildingPop4ReplaceResult::ScatterRoll(_) => None,
        },
    )?;
    placements.extend(scatter);
    Some(placements)
}

/// Use the PE call tile for pop 2/3 while keeping the resolved NW origin for other sizes.
pub(in crate::app) fn rci_decline_change_building_at_call_optional(
    building: u8,
    footprint: &[(usize, usize)],
    call_x: usize,
    call_y: usize,
    origin_x: usize,
    origin_y: usize,
    replace_tile: bool,
    mut process_crt: Option<&mut Win95CrtRand>,
) -> Option<Vec<(usize, usize, u8)>> {
    match rci_building_pop_level(building)? {
        1 => {
            // ChooseAndPlace(pop 1) picks at raw (x,y) and skips the multi-tile bounds gate.
            pe_changebuilding_pop1_direct_placement_with_picker(call_x, call_y, || {
                abandoned_building_for_pop_optional(1, call_x, call_y, process_crt.as_deref_mut())
            })
        }
        2 => {
            if replace_tile {
                pe_changebuilding_pop2_replace_placements_with_picker(call_x, call_y, |x, y| {
                    // Each pop-1 ChooseAndPlace picks before validating its raw call tile.
                    abandoned_building_for_pop_optional(
                        1,
                        usize::try_from(x).unwrap_or_default(),
                        usize::try_from(y).unwrap_or_default(),
                        process_crt.as_deref_mut(),
                    )
                })
            } else {
                // ChooseAndPlace(pop 2) picks at raw (x,y), then stamps from (x, y - 1).
                pe_changebuilding_pop2_direct_placement_with_picker(call_x, call_y, || {
                    abandoned_building_for_pop_optional(
                        2,
                        call_x,
                        call_y,
                        process_crt.as_deref_mut(),
                    )
                })
            }
        }
        3 => {
            // ChangeBuilding(pop 3) chooses pop 3, or pop 2 when replacing, at raw (x,y).
            let abandoned_pop = if replace_tile { 2 } else { 3 };
            pe_changebuilding_pop3_direct_placement_with_picker(call_x, call_y, || {
                abandoned_building_for_pop_optional(
                    abandoned_pop,
                    call_x,
                    call_y,
                    process_crt.as_deref_mut(),
                )
            })
        }
        4 if replace_tile => {
            pe_changebuilding_pop4_replace_placements_with_picker(call_x, call_y, |step| match step
            {
                PeChangeBuildingPop4ReplaceStep::ChooseBuilding { pop, x, y } => {
                    abandoned_building_for_pop_optional(
                        pop,
                        usize::try_from(x).unwrap_or_default(),
                        usize::try_from(y).unwrap_or_default(),
                        process_crt.as_deref_mut(),
                    )
                    .map(PeChangeBuildingPop4ReplaceResult::Building)
                }
                PeChangeBuildingPop4ReplaceStep::ScatterRoll { x, y } => {
                    Some(PeChangeBuildingPop4ReplaceResult::ScatterRoll(
                        rci_change_building_scatter_roll_optional(x, y, process_crt.as_deref_mut()),
                    ))
                }
            })
        }
        4 => {
            // Direct ChooseAndPlace(pop 4) picks at the raw call tile, then stamps from (x, y - 2).
            pe_changebuilding_pop4_direct_placement_with_picker(call_x, call_y, || {
                abandoned_building_for_pop_optional(4, call_x, call_y, process_crt.as_deref_mut())
            })
        }
        _ => rci_decline_change_building_optional(
            building,
            footprint,
            origin_x,
            origin_y,
            replace_tile,
            process_crt,
        ),
    }
}
