//! Win95 SimCalendar and provisional Mac CODE 14 phase adapter: both advance
//! in 25-step monthly cycles, but the edition-specific phase contracts differ.
//!
//! `GameClock` ticks these days in real time. Live play dispatches each named
//! pass on the matching day. Month-batched simulation (tests / catch-up helper)
//! runs the same Win95 order: growth ticks (days 3–18) then traffic (day 19).
//! The loaded-city RCI growth restriction does not suppress the Win95 calendar:
//! power, density, transport upkeep, trips, port activity, and day-19 XTRF
//! decay/sum still run. The player-tile allowlist only blocks zone development.
//! This module is the Win95 simulation contract. Mac 68k also advances a
//! 25-phase counter, but its schedule differs: phase 1 runs monthly power,
//! phase 2 rebuilds derived maps, phases 3–18 partition a monthly map/object
//! worker, phase 19 decays traffic, phase 20 runs water, phase 21 publishes
//! demand/history, and phase 22 handles progression/scenario checks. In
//! particular, Mac phases 3–18 are gameplay work, not the Win95 4×4 growth
//! lattice and not the old ShowWindow-wrapper interpretation. CityGame has a
//! separate Mac phase adapter in its month-batched and live-day dispatch paths,
//! but many phase bodies remain partial or intentionally gated. Do not treat
//! this module's overall schedule as fully Mac-faithful.
//! Each growth tick applies one `SimulationGrowthTick` 4×4 lattice immediately
//! within each pass (trips, then decline/churches/develop/special), with a shared
//! monthly cap. Day 0 settles year-end cashflow (taxes − services ± ordinances −
//! bond interest), clears budget history, and opens the Budget window on January
//! year-rollover unless Auto-Budget is on. Day 19 is `SimulationUpdateMonthlyTrafficData`
//! (decay XTRF by `value-(value>>2)`, sum microsim cells into MISC traffic — not a
//! second trip generator pass). Day 20 is
//! `SimulationUpdateWaterConsumption` (XBIT pipe/water flood-fill + used%). Day 1 is
//! power consumption (used% snapshot; power stays on-demand, not XBIT). Demand is
//! day 21 only (`SimulationRCIDemandUpdates`); growth ticks use last month's
//! `wCityDemand`. Win95 `SimulationEQ_LE_Processing` (national, neighbors, industry mix, then EQ_LE tables), then graphs
//! are also day 21. Day 22 checks city progression
//! milestones (`dwCityProgressionRequirements`) then scenario / ambient disaster.
//! Day 21 also advances the Win95 weather Markov machine (heat/wind/humidity/trend).
//! Day 24 refreshes toolbar / city map / neighbors and may start hurricane/tornado
//! from the current weather trend (`UpdateWeatherOrDisasterState`). Growth ticks
//! also develop airport / seaport / military zones with Win95 flagged-tile growth
//! thresholds (military by active MISC base type), may spawn civilian planes /
//! helicopters from powered runways and ships from cranes, may spawn trains from
//! powered rail depots and sailboats from powered marinas (ambient actors then
//! step along their facing each overlay tick — spawn heading aims at nearby
//! water/rail, trains follow curves/junctions with car consists and subway
//! kind switches, ships may wander on open water, and blocked actors turn or
//! despawn), updates arco microsim stat0 from land value / crime / pollution,
//! provisions system/structure microsims on place (with arco steal when full),
//! seeds structure microsim capacity stats (power MW, arco design/built year,
//! school/hospital grade seed, police/fire funding), refreshes city-wide system
//! microsim counts each January and fills school/hospital/college/arco/police/
//! fire/stadium/prison/zoo/llama/city-hall/statue/mayor/water/desal occupancy
//! plus power-plant age, and may turn
//! underfunded unzoned transport into rubble
//! (`DoBudgetRoad/Rail/Highway/Tunnel/SubwayCheck`). Bridges use the Win95 wind
//! formula (`wind + rand%50 >= funding`) and announce collapses. Underfunded
//! tunnels use DestroyStructure-style clearing of the portal run.

use crate::city::{CITY_HEIGHT, CITY_WIDTH, City, ZONE_CORNER_ALL};
use crate::game_clock::GameClock;

pub(super) const SIM_CALENDAR_DAYS_PER_MONTH: u32 = 25;

pub(super) const SIM_DAY_BUDGET: u32 = 0;
pub(super) const SIM_DAY_POWER: u32 = 1;
pub(super) const SIM_DAY_DENSITY: u32 = 2;
pub(super) const SIM_DAY_GROWTH_FIRST: u32 = 3;
pub(super) const SIM_DAY_GROWTH_LAST: u32 = 18;
pub(super) const SIM_DAY_TRAFFIC: u32 = 19;
pub(super) const SIM_DAY_WATER: u32 = 20;
pub(super) const SIM_DAY_DEMAND: u32 = 21;
pub(super) const SIM_DAY_SCENARIO: u32 = 22;
pub(super) const SIM_DAY_STATS_DIALOGS: u32 = 23;
pub(super) const SIM_DAY_UI_CHROME: u32 = 24;

pub(super) fn pe_city_founded_year(city: &City, clock: GameClock) -> u16 {
    city.year_founded()
        .filter(|&year| year > 0)
        .and_then(|year| u16::try_from(year).ok())
        .or_else(|| city.saved_date().map(|date| date.year))
        .unwrap_or(clock.year())
}

/// Win95 `wGrowthPopulationWeight` @ `SIMCITY.EXE` `0x004E75B0`.
///
/// `SimulationGrowthTick` indexes this signed 16-bit table with the
/// populated-tile growth level. An eligible normal-growth candidate adds the
/// entry to aggregate slots 1..6; the tile-special branch adds it to slot 7.
/// Entries outside the table, including non-positive levels, contribute zero.
#[allow(dead_code)]
pub(super) const PE_GROWTH_AGGREGATE_WEIGHTS: [i16; 16] =
    [0, 1, 8, 12, 36, 0, 0, 0, 1, 0, -1, 0, 1, 0, -1, 0];

#[allow(dead_code)]
pub(super) fn pe_growth_aggregate_weight(level: i16) -> i32 {
    usize::try_from(level)
        .ok()
        .and_then(|index| PE_GROWTH_AGGREGATE_WEIGHTS.get(index))
        .copied()
        .unwrap_or(0)
        .into()
}

/// Win95 `wGrowthLevelByXbld` @ `SIMCITY.EXE` `0x004E7458`.
/// The growth routine indexes this table with normalized `XBLD - 0x70`.
#[allow(dead_code)]
pub(super) const PE_GROWTH_LEVELS_BY_XBLD: &[i16] = &[
    1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 2, 2, 2, 2,
    3, 3, 3, 3, 2, 2, 2, 2, 2, 3, 3, 3, 3, 3, 2, 2, 2, 2, 3, 3, 3, 3, 2, 2, 3, 3, 2, 2, 3, 3, 4, 4,
    4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 0, 0,
];

#[allow(dead_code)]
pub(super) fn pe_growth_level_for_xbld(tile: u8) -> i16 {
    tile.checked_sub(0x70)
        .map(usize::from)
        .and_then(|index| PE_GROWTH_LEVELS_BY_XBLD.get(index))
        .copied()
        .unwrap_or(0)
}

/// The PE growth candidate inputs derived from one normalized XBLD value:
/// positive growth level and the tile-special flag that selects aggregate
/// slot 7 instead of the normal category slot.
#[allow(dead_code)]
pub(super) fn pe_growth_tile_inputs(tile: u8) -> Option<(i16, bool)> {
    let level = pe_growth_level_for_xbld(tile);
    (level > 0).then(|| (level, pe_tile_has_special_growth_flag(tile)))
}

/// Win95 tile growth class @ `SIMCITY.EXE` `0x004E7508`, indexed by `XBLD - 0x70`.
/// Class `0` = GROWTH_CHANGE, `1` = BUILD_THINK / construction, `2` = BUILD_ABANDON.
#[allow(dead_code)]
pub(super) const PE_GROWTH_TILE_CLASS: &[u8] = &[
    // 0x70..0x87
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, // 0x88..0x8b
    1, 1, 2, 2, // 0x8c..0xa5
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    // 0xa6..0xad
    1, 1, 1, 1, 2, 2, 2, 2, // 0xae..0xc1
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, // 0xc2..0xc5
    1, 1, 2, 2,
];

#[allow(dead_code)]
pub(super) fn pe_growth_tile_class(tile: u8) -> u8 {
    tile.checked_sub(0x70)
        .map(usize::from)
        .and_then(|index| PE_GROWTH_TILE_CLASS.get(index))
        .copied()
        .unwrap_or(0xff)
}

/// Entries with class value 2 in the PE `0x4E7508` table (`XBLD - 0x70`).
#[allow(dead_code)]
pub(super) fn pe_tile_has_special_growth_flag(tile: u8) -> bool {
    pe_growth_tile_class(tile) == 2
}

/// Shared PE `FUN_004358B0` gates before class dispatch: zone `1..=6`, `XBLD >= 0x70`,
/// view-angle corner (`0x436056`), and positive growth level (`0x4360ea`).
#[allow(dead_code)]
pub(super) fn pe_growth_pre_roll_tile_inputs(
    city: &City,
    x: usize,
    y: usize,
) -> Option<(u8, u8, u8)> {
    let zone = city.zone_at(x, y)?;
    let zone_id = zone.zone_id();
    if !(1..=6).contains(&zone_id) {
        return None;
    }
    let tile = city.building_at(x, y);
    if tile < 0x70 {
        return None;
    }
    // PE @ `0x436056`: `(XZON & 0xf0) & wPositionAngle != 0` (same as XZONCornerCheck).
    let angle = [0x80u8, 0x10, 0x20, 0x40][usize::from(city.rotation() & 3)];
    if zone.raw_bits() & ZONE_CORNER_ALL & angle == 0 {
        return None;
    }
    if pe_growth_level_for_xbld(tile) <= 0 {
        return None;
    }
    Some((zone_id, tile, pe_growth_tile_class(tile)))
}

/// PE `FUN_004358B0` @ `0x0043602c..0x00436112`: class-0 pre-roll aggregate inputs.
///
/// Credits the XZON type nibble slot for the visited XBLD. Does not require building
/// kind to match the zone — PE writes before ChangeBuilding footprint / kind
/// validation. Multi-tile continuation tiles fail the corner check.
#[allow(dead_code)]
pub(super) fn pe_class0_growth_aggregate_inputs(
    city: &City,
    x: usize,
    y: usize,
) -> Option<(u8, u8)> {
    let (zone_id, tile, class) = pe_growth_pre_roll_tile_inputs(city, x, y)?;
    (class == 0).then_some((zone_id, tile))
}

/// PE `FUN_004358B0` @ `0x004361c9`: class-2 BUILD_ABANDON pre-roll slot-7 inputs.
///
/// Credits slot 7 with the visited XBLD weight before the abandon redevelop roll.
/// Same corner / level gates as class 0; does not require trip access, demand, or
/// a successful ChooseAndPlace.
#[allow(dead_code)]
pub(super) fn pe_class2_growth_aggregate_inputs(city: &City, x: usize, y: usize) -> Option<u8> {
    let (_zone_id, tile, class) = pe_growth_pre_roll_tile_inputs(city, x, y)?;
    (class == 2).then_some(tile)
}

/// The transient PE `0x4CA428` growth block: total slot 0, six normal
/// category slots, and the special-zone slot 7.
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct PeGrowthAggregate {
    slots: [i32; 8],
}

impl PeGrowthAggregate {
    #[allow(dead_code)]
    pub(super) fn reset(&mut self) {
        self.slots = [0; 8];
    }

    #[allow(dead_code)]
    pub(super) fn add_candidate(&mut self, category: u8, level: i16, tile_special: bool) {
        if (1..=6).contains(&category) {
            if tile_special {
                self.add_special_candidate(level);
            } else {
                self.add_normal_candidate(category, level);
            }
        }
    }

    #[allow(dead_code)]
    pub(super) fn add_xbld_candidate(&mut self, category: u8, tile: u8) {
        if let Some((level, tile_special)) = pe_growth_tile_inputs(tile) {
            self.add_candidate(category, level, tile_special);
        }
    }

    #[allow(dead_code)]
    pub(super) fn add_normal_candidate(&mut self, category: u8, level: i16) {
        if level > 0 && (1..=6).contains(&category) {
            let slot = usize::from(category);
            self.slots[slot] = self.slots[slot].wrapping_add(pe_growth_aggregate_weight(level));
        }
    }

    #[allow(dead_code)]
    pub(super) fn add_special_candidate(&mut self, level: i16) {
        if level > 0 {
            self.slots[7] = self.slots[7].wrapping_add(pe_growth_aggregate_weight(level));
        }
    }

    #[allow(dead_code)]
    pub(super) fn fold_normal_slots(&mut self) {
        self.slots[0] = self.slots[1..=6].iter().copied().fold(0, i32::wrapping_add);
    }

    #[allow(dead_code)]
    pub(super) fn zone_population_rows(&self) -> [i32; 3] {
        [
            self.slots[1] + self.slots[2],
            self.slots[3] + self.slots[4],
            self.slots[5] + self.slots[6],
        ]
    }

    #[allow(dead_code)]
    pub(super) fn scaled_city_population(&self) -> i32 {
        self.slots[0].wrapping_mul(10)
    }

    /// Win95 `FUN_00413E50`: 32-bit unsigned `100 * slot7 / (slot0 + slot7 + 1)`
    /// → `0x4CA5F8` (`EDX` is cleared before `DIV`).
    ///
    /// Call after [`Self::fold_normal_slots`]. Slot 7 is the flag-2/special weight
    /// accumulator; slot 0 is the folded normal contribution total. The native
    /// x86 code does not clamp the quotient, so preserve each accumulator's
    /// 32-bit bit pattern and the wrapping arithmetic used to form the operands.
    #[allow(dead_code)]
    pub(super) fn unemployment_ratio_percent(&self) -> u32 {
        let slot0 = self.slots[0] as u32;
        let slot7 = self.slots[7] as u32;
        let denom = slot0.wrapping_add(slot7).wrapping_add(1);
        slot7.wrapping_mul(100) / denom
    }

    #[allow(dead_code)]
    pub(super) fn slots(&self) -> &[i32; 8] {
        &self.slots
    }
}

/// MISC `wCityProgression` (sc2kfix / Win95).
pub(super) const MISC_CITY_PROGRESSION_OFFSET: usize = 0x0020;

/// Win95 `dwCityProgressionRequirements` @ `SIMCITY.EXE` `0x004E6984`.
/// Indexed by current progression; advance when `requirement < city_population`.
pub(super) const CITY_PROGRESSION_REQUIREMENTS: &[u32] = &[
    2_000, 10_000, 30_000, 60_000, 90_000, 120_000, 500_000, 1_000_000, 5_000_000, 10_000_000,
];

/// sc2kfix `Hook_Engine_SimulationProcessTick` growth-tick phase:
/// `iStep = ((dwMonDay - 3) / 4 % 4)`, `iSubStep = (dwMonDay + 1) % 4`.
pub(super) fn sim_calendar_growth_phase(mon_day: u32) -> Option<(u16, u16)> {
    if !(SIM_DAY_GROWTH_FIRST..=SIM_DAY_GROWTH_LAST).contains(&mon_day) {
        return None;
    }
    let step = ((mon_day - SIM_DAY_GROWTH_FIRST) / 4 % 4) as u16;
    let substep = ((mon_day + 1) % 4) as u16;
    Some((step, substep))
}

pub(super) fn sim_calendar_growth_phases() -> [(u16, u16); 16] {
    let mut phases = [(0u16, 0u16); 16];
    for (index, mon_day) in (SIM_DAY_GROWTH_FIRST..=SIM_DAY_GROWTH_LAST).enumerate() {
        phases[index] = sim_calendar_growth_phase(mon_day).expect("growth day");
    }
    phases
}

/// Win95 `SimulationGrowthTick`: `for (iX = iStep; iX < MAP; iX += 4)` /
/// `for (iY = iSubStep; iY < MAP; iY += 4)`.
pub(super) fn sim_calendar_growth_phase_tiles(
    step: u16,
    substep: u16,
) -> impl Iterator<Item = (usize, usize)> {
    let step = usize::from(step);
    let substep = usize::from(substep);
    (step..CITY_WIDTH)
        .step_by(4)
        .flat_map(move |x| (substep..CITY_HEIGHT).step_by(4).map(move |y| (x, y)))
}

pub(super) fn sim_calendar_all_growth_tiles() -> impl Iterator<Item = (usize, usize)> {
    sim_calendar_growth_phases()
        .into_iter()
        .flat_map(|(step, substep)| sim_calendar_growth_phase_tiles(step, substep))
}

#[cfg(test)]
mod calendar_unit_tests {
    use super::{
        CITY_PROGRESSION_REQUIREMENTS, PeGrowthAggregate, SIM_CALENDAR_DAYS_PER_MONTH,
        SIM_DAY_DEMAND, SIM_DAY_DENSITY, SIM_DAY_TRAFFIC, pe_class0_growth_aggregate_inputs,
        pe_class2_growth_aggregate_inputs, pe_growth_aggregate_weight, pe_growth_tile_class,
        pe_growth_tile_inputs, pe_tile_has_special_growth_flag, sim_calendar_all_growth_tiles,
        sim_calendar_growth_phase, sim_calendar_growth_phase_tiles, sim_calendar_growth_phases,
    };
    use crate::city::{CITY_TILES, CITY_WIDTH, City};

    #[test]
    fn progression_requirements_match_win95_table() {
        assert_eq!(
            CITY_PROGRESSION_REQUIREMENTS,
            &[
                2_000, 10_000, 30_000, 60_000, 90_000, 120_000, 500_000, 1_000_000, 5_000_000,
                10_000_000,
            ]
        );
    }

    #[test]
    fn growth_phases_cover_all_four_by_four_offsets() {
        let phases = sim_calendar_growth_phases();
        // PE calendar day cases 3..=18 pass this exact ordered pair list to
        // SimulationGrowthTick through thunk 0x4022fc.
        assert_eq!(
            phases,
            [
                (0, 0),
                (0, 1),
                (0, 2),
                (0, 3),
                (1, 0),
                (1, 1),
                (1, 2),
                (1, 3),
                (2, 0),
                (2, 1),
                (2, 2),
                (2, 3),
                (3, 0),
                (3, 1),
                (3, 2),
                (3, 3),
            ]
        );
        let mut seen = std::collections::BTreeSet::new();
        for phase in phases {
            assert!(seen.insert(phase), "duplicate growth phase {phase:?}");
        }
        assert_eq!(seen.len(), 16);
    }

    #[test]
    fn non_growth_days_have_no_phase() {
        assert_eq!(sim_calendar_growth_phase(SIM_DAY_DENSITY), None);
        assert_eq!(sim_calendar_growth_phase(SIM_DAY_TRAFFIC), None);
        assert_eq!(sim_calendar_growth_phase(SIM_DAY_DEMAND), None);
        assert_eq!(sim_calendar_growth_phase(SIM_CALENDAR_DAYS_PER_MONTH), None);
    }

    #[test]
    fn growth_lattices_cover_every_tile_once_in_win95_order() {
        let tiles: Vec<_> = sim_calendar_all_growth_tiles().collect();
        assert_eq!(tiles.len(), CITY_TILES);
        assert_eq!(tiles[0], (0, 0));
        assert_eq!(sim_calendar_growth_phase_tiles(1, 2).next(), Some((1, 2)));
        assert!(
            sim_calendar_growth_phase_tiles(1, 2).any(|(x, y)| x == 5 && y == 6),
            "(5,6) belongs to iStep=1 iSubStep=2"
        );
        assert_eq!(tiles[1], (0, 4));
        assert_eq!(tiles[32], (4, 0));
        let mut seen = vec![false; CITY_TILES];
        for (x, y) in tiles {
            let index = y * CITY_WIDTH + x;
            assert!(!seen[index], "duplicate growth tile ({x},{y})");
            seen[index] = true;
        }
        assert!(seen.iter().all(|visited| *visited));
    }

    #[test]
    fn pe_growth_tile_class_matches_pe_4e7508() {
        assert_eq!(pe_growth_tile_class(0x70), 0);
        assert_eq!(pe_growth_tile_class(0x88), 1);
        assert_eq!(pe_growth_tile_class(0x8a), 2);
        assert_eq!(pe_growth_tile_class(0x8c), 0);
        assert_eq!(pe_growth_tile_class(0xa6), 1);
        assert_eq!(pe_growth_tile_class(0xaa), 2);
        assert_eq!(pe_growth_tile_class(0xae), 0);
        assert_eq!(pe_growth_tile_class(0xc2), 1);
        assert_eq!(pe_growth_tile_class(0xc4), 2);
        assert!(pe_tile_has_special_growth_flag(0x8a));
        assert!(!pe_tile_has_special_growth_flag(0x70));
        assert!(!pe_tile_has_special_growth_flag(0x88));
    }

    #[test]
    fn pe_class0_aggregate_requires_corner_and_ignores_kind_mismatch() {
        let mut city = City::new_blank("/tmp/pe-class0-agg.sc2", "Class0").expect("blank city");
        // Residential XBLD on commercial zone — PE still credits the zone slot.
        city.set_zone(20, 20, 0x03 | 0xf0);
        city.set_building(20, 20, 0x70);
        assert_eq!(
            pe_class0_growth_aggregate_inputs(&city, 20, 20),
            Some((3, 0x70))
        );
        // Continuation tile: wrong corner for rot 0 (SW=0x80) → no credit.
        city.set_zone(21, 20, 0x03 | 0x10);
        city.set_building(21, 20, 0x70);
        assert_eq!(pe_class0_growth_aggregate_inputs(&city, 21, 20), None);
        // Construction class 1 → no class-0 credit.
        city.set_zone(22, 20, 0x01 | 0xf0);
        city.set_building(22, 20, 0x88);
        assert_eq!(pe_class0_growth_aggregate_inputs(&city, 22, 20), None);
    }

    #[test]
    fn pe_class2_aggregate_credits_abandoned_without_kind_match() {
        let mut city = City::new_blank("/tmp/pe-class2-agg.sc2", "Class2").expect("blank city");
        city.set_zone(20, 20, 0x04 | 0xf0);
        city.set_building(20, 20, 0x8a);
        assert_eq!(pe_class2_growth_aggregate_inputs(&city, 20, 20), Some(0x8a));
        assert_eq!(pe_class0_growth_aggregate_inputs(&city, 20, 20), None);
        // Wrong corner → no slot-7 credit.
        city.set_zone(21, 20, 0x04 | 0x10);
        city.set_building(21, 20, 0x8a);
        assert_eq!(pe_class2_growth_aggregate_inputs(&city, 21, 20), None);
    }

    #[test]
    fn pe_growth_aggregate_routes_flag2_xbld_to_slot_7() {
        let mut agg = PeGrowthAggregate::default();
        agg.add_xbld_candidate(1, 0x8A);
        assert_eq!(
            agg.slots()[7],
            pe_growth_aggregate_weight(1),
            "abandoned 0x8A is flag-2 → special slot 7"
        );
        assert_eq!(agg.slots()[1], 0);

        let mut normal = PeGrowthAggregate::default();
        normal.add_xbld_candidate(1, 0x70);
        assert_eq!(normal.slots()[1], pe_growth_aggregate_weight(1));
        assert_eq!(normal.slots()[7], 0);

        let mut vacant = PeGrowthAggregate::default();
        vacant.add_xbld_candidate(1, 0);
        assert_eq!(
            *vacant.slots(),
            [0; 8],
            "vacant XBLD contributes no pre-roll weight"
        );
    }

    #[test]
    fn pe_growth_aggregate_category_comes_from_xzon_low_nibble() {
        let mut city = City::new_blank("/tmp/pe-growth-xzon.sc2", "test").expect("blank city");
        let (x, y) = (17, 29);
        // PE `FUN_004358B0` uses `[0x4CA1F0 + 4*x][y] & 0x0f`; XZON's
        // corner bits occupy the high nibble and must not affect the category.
        city.set_zone(x, y, 0xf5);
        let zone = city.zone_at(x, y).expect("XZON tile");
        assert_eq!(zone.raw_bits(), 0xf5);
        assert_eq!(zone.zone_id(), 5);

        let mut aggregate = PeGrowthAggregate::default();
        aggregate.add_xbld_candidate(zone.zone_id(), 0x70);
        let weight = pe_growth_aggregate_weight(1);
        assert_eq!(aggregate.slots()[5], weight);
        assert_eq!(aggregate.slots()[4], 0);
        assert_eq!(aggregate.zone_population_rows(), [0, 0, weight]);
    }

    #[test]
    fn pe_growth_aggregate_unemployment_ratio_matches_fun_00413e50() {
        let mut agg = PeGrowthAggregate::default();
        agg.add_xbld_candidate(1, 0x70); // normal → slot 1
        agg.add_xbld_candidate(1, 0x8A); // flag-2 → slot 7
        agg.fold_normal_slots();
        let slot0 = pe_growth_aggregate_weight(1);
        let slot7 = pe_growth_aggregate_weight(1);
        assert_eq!(agg.slots()[0], slot0);
        assert_eq!(agg.slots()[7], slot7);
        assert_eq!(
            agg.unemployment_ratio_percent(),
            ((slot7 * 100) / (slot0 + slot7 + 1)) as u32,
            "PE writes 100 * slot7 / (slot0 + slot7 + 1)"
        );

        let empty = PeGrowthAggregate::default();
        assert_eq!(empty.unemployment_ratio_percent(), 0);
    }

    #[test]
    fn pe_growth_aggregate_unemployment_ratio_uses_unsigned_x86_division() {
        let mut agg = PeGrowthAggregate::default();
        agg.slots[0] = 1;
        agg.slots[7] = -1; // Native DWORD bit pattern 0xffff_ffff.

        assert_eq!(agg.unemployment_ratio_percent(), 0xffff_ff9c);
    }

    #[test]
    fn pe_growth_fold_and_population_match_original_instructions() {
        let fixture =
            include_str!("../../tests/fixtures/simulation/win95/pe_growth_population.csv");
        let mut city = City::new_blank("/tmp/pe-population-replay.sc2", "Replay").unwrap();
        let mut count = 0;
        for line in fixture.lines().skip(1) {
            let values: Vec<u32> = line.split(',').map(|v| v.parse().unwrap()).collect();
            assert_eq!(values.len(), 13);
            let mut aggregate = PeGrowthAggregate::default();
            for (slot, value) in aggregate.slots[1..=6].iter_mut().zip(&values[..6]) {
                *slot = *value as i32;
            }
            aggregate.fold_normal_slots();
            assert_eq!(aggregate.slots[0] as u32, values[8], "{line}");
            let scaled = aggregate.scaled_city_population() as u32;
            assert_eq!(scaled, values[9], "{line}");
            city.persist_pe_day22_population_leftover(values[6]);
            city.persist_pe_day22_population_leftover(scaled);
            assert_eq!(city.pe_eq_leftover_births(), values[10], "{line}");
            assert_eq!(city.pe_eq_leftover_deaths(), values[11], "{line}");
            assert_eq!(values[7].wrapping_add(scaled), values[12], "{line}");
            count += 1;
        }
        assert_eq!(count, 132);
    }

    #[test]
    fn pe_growth_aggregate_zone_population_rows_pair_slots() {
        let mut agg = PeGrowthAggregate::default();
        agg.add_normal_candidate(1, 1); // weight → slot 1
        agg.add_normal_candidate(2, 2); // weight → slot 2
        agg.add_normal_candidate(3, 1);
        agg.add_normal_candidate(5, 1);
        agg.fold_normal_slots();
        let w1 = pe_growth_aggregate_weight(1);
        let w2 = pe_growth_aggregate_weight(2);
        assert_eq!(
            agg.zone_population_rows(),
            [w1 + w2, w1, w1],
            "PE pZonePops rows are (1+2), (3+4), (5+6)"
        );
        assert_eq!(
            agg.scaled_city_population(),
            (w1 * 3 + w2) * 10,
            "PE slot 0 folds all six R/C/I slots, not only the residential pair"
        );
    }

    #[test]
    fn pe_growth_aggregate_ignores_plan_building_ids_outside_visited_xbld() {
        // Plan buildings such as fresh RCI shells must not be credited unless the
        // visited tile already holds that XBLD (PE indexes the pre-roll tile).
        assert!(pe_growth_tile_inputs(0).is_none());
        assert_eq!(pe_growth_tile_inputs(0x8A), Some((1, true)));
        assert_eq!(pe_growth_tile_inputs(0x70), Some((1, false)));
    }
}
