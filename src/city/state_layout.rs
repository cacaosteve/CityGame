//! Saved-state byte offsets and simulation constants shared across City modules.

/// Win95 MISC city mode word @ `+0x04` (PE BSS mirror `0x4ca42c`).
pub(super) const MISC_CITY_MODE_OFFSET: usize = 0x0004;
/// Win95 `FUN_00431330` reads a fixed-size, versioned MISC record.
pub(super) const WIN95_MISC_RECORD_SIZE: usize = 0x12c0;
pub(super) const WIN95_MISC_RECORD_VERSION: u32 = 0x122;
/// Mac CODE 14 calendar counter units recovered from MCRP save/restore: 25
/// dispatcher phases per month and 300 per year.
pub(super) const MAC_CALENDAR_PHASES_PER_MONTH: u32 = 25;
pub(super) const MAC_CALENDAR_PHASES_PER_YEAR: u32 = 300;
/// Win95 MISC city rotation word @ `+0x08` (PE BSS mirror `0x4c942c`).
pub(super) const MISC_CITY_ROTATION_OFFSET: usize = 0x0008;
/// Win95 MISC year founded word @ `+0x0c` (PE BSS mirror `0x4ca5f4`).
pub(super) const MISC_YEAR_FOUNDED_OFFSET: usize = 0x0c;
/// Win95 MISC city age (days) dword @ `+0x10` (PE BSS mirror `0x4cae04`).
pub(super) const MISC_CITY_AGE_OFFSET: usize = 0x10;
/// Win95 MISC city funds dword @ `+0x14` (PE BSS mirror `0x4ca444`).
pub(super) const MISC_CITY_FUNDS_OFFSET: usize = 0x0014;
/// Win95 MISC outstanding bond count dword @ `+0x18` (PE BSS mirror `0x4cc4e8`).
pub(super) const MISC_CITY_BONDS_OFFSET: usize = 0x0018;
/// Shared 50-entry per-bond rate table, consumed as low WORDs by both editions.
pub(super) const MISC_BOND_RATE_TABLE_OFFSET: usize = 0x0610;
/// Mac-only cached signed sum of the active bond rates (MISC slot 1070).
pub(super) const MAC_BOND_RATE_SUM_MISC_OFFSET: usize = 0x10b8;
pub(super) const BOND_RATE_TABLE_CAPACITY: usize = 50;
pub(super) const MISC_DIFFICULTY_OFFSET: usize = 0x001c;
pub(super) const MISC_RCI_DEMAND_OFFSET: usize = 0x0718;
// Win95 new-city defaults from `FUN_00479610` / `FUN_0047998e`.
pub(super) const MISC_AUTO_GOTO_OFFSET: usize = 0x0ff4;
pub(super) const MISC_SOUND_EFFECTS_OFFSET: usize = 0x0ff8;
pub(super) const MISC_MUSIC_OFFSET: usize = 0x0ffc;
pub(super) const MISC_NEWSPAPER_EXTRA_OFFSET: usize = 0x1008;
pub(super) const MISC_NEWSPAPER_SUBSCRIPTION_OFFSET: usize = 0x1004;
/// Win95 MISC city progression / newspaper choice word @ `+0x20` (PE BSS `0x4cb010`).
pub(super) const MISC_CITY_PROGRESSION_OFFSET: usize = 0x0020;
/// Win95 MISC city value dword @ `+0x24` (PE BSS mirror `0x4ca4d0`).
pub(super) const MISC_CITY_VALUE_OFFSET: usize = 0x0024;
/// Win95 MISC land value dword @ `+0x28` (PE BSS mirror `0x4ca440`).
pub(super) const MISC_LAND_VALUE_OFFSET: usize = 0x0028;
/// Win95 MISC garbage dword @ `+0x40` (PE BSS mirror `dwCityGarbage` `0x4ca5f0`).
pub(super) const MISC_GARBAGE_OFFSET: usize = 0x0040;
/// Win95 MISC city fame dword @ `+0x38` (PE BSS mirror `dwCityFame` `0x4cad28`).
pub(super) const MISC_CITY_FAME_OFFSET: usize = 0x0038;
/// Win95 MISC advertising dword @ `+0x3c` (PE BSS mirror `dwCityAdvertising` `0x4cb018`).
pub(super) const MISC_CITY_ADVERTISING_OFFSET: usize = 0x003c;
/// Win95 MISC weather trend byte @ `+0x6c` (PE BSS mirror `bWeatherTrend` `0x4cb40c`).
pub(super) const MISC_WEATHER_TREND_OFFSET: usize = 0x006c;
/// Win95 MISC weather heat byte @ `+0x60` (PE BSS mirror `bWeatherHeat` `0x4cade0`).
pub(super) const MISC_HEAT_OFFSET: usize = 0x0060;
/// Win95 MISC weather wind byte @ `+0x64` (PE BSS mirror `bWeatherWind` `0x4cae0c`).
pub(super) const MISC_WIND_OFFSET: usize = 0x0064;
/// Win95 ordinance bitmask (`0x4caa40` / MISC `0x0fa0`).
pub(super) const MISC_ORDINANCE_FLAGS_OFFSET: usize = 0x0fa0;
pub(super) const MISC_CITY_UNEMPLOYMENT_OFFSET: usize = 0x0fa4;
/// Mac CODE 4 slot 1000 (`ffffeff2`) also stores the ordinance mask at `+0x0fa0`.
pub(super) const MAC_ORDINANCE_FLAGS_MISC_OFFSET: usize = 0x0fa0;
pub(super) const ORDINANCE_FLAGS_MASK: u32 = (1 << 20) - 1;
/// Win95 `0x4ca934` PlaceTile military census WORD[16] / MISC `0xfa8` (16 dwords).
pub(super) const MISC_MILITARY_TILE_COUNTS_OFFSET: usize = 0x0fa8;
pub(super) const MILITARY_TILE_COUNT_VALUES: usize = 16;
/// Win95 `0x4ca41c` PlaceXUND occupancy / MISC `0x0fe8`. After ordinance `0xfa0`,
/// unemployment `0xfa4`, and 16 military tile-count dwords `0xfa8..0xfe7`.
pub(super) const MISC_XUND_OCCUPANCY_OFFSET: usize = 0x0fe8;
/// Win95 packed saved camera target word at MISC `0x1010`.
pub(super) const MISC_SAVED_VIEW_TARGET_OFFSET: usize = 0x1010;
/// Win95 saved camera scale selector at MISC `0x1014`.
pub(super) const MISC_SAVED_VIEW_MODE_OFFSET: usize = 0x1014;
/// PE PlaceTile military case map @ `0x442208`/`0x442268` (XBLD−`0xdd` → case).
pub(super) const PE_MILITARY_TILE_CASE_MAP: [u8; 0x1d] = [
    0x00, 0x01, 0x0f, 0x02, 0x0f, 0x03, 0x04, 0x05, 0x06, 0x0f, 0x07, 0x08, 0x0f, 0x09, 0x0f, 0x0f,
    0x0f, 0x0f, 0x0a, 0x0f, 0x0b, 0x0c, 0x0f, 0x0f, 0x0f, 0x0d, 0x0f, 0x0f, 0x0e,
];
/// PE jump-table case → word index in `0x4ca934` (case `0x0f` / out-of-range → word 0).
pub(super) const PE_MILITARY_TILE_CASE_WORD: [u8; 16] =
    [1, 2, 10, 11, 6, 7, 8, 12, 13, 5, 3, 9, 4, 14, 15, 0];
pub(super) const MISC_CITY_CRIME_OFFSET: usize = 0x002c;
pub(super) const MISC_WORK_FORCE_PERCENT_OFFSET: usize = 0x0044;
pub(super) const MISC_CITY_LIFE_EXPECTANCY_OFFSET: usize = 0x0048;
pub(super) const MISC_CITY_EDUCATION_QUOTIENT_OFFSET: usize = 0x004c;
/// Win95 `dwNationalPopulation` (`0x4CA928`) / MISC `0x50`.
pub(super) const MISC_NATIONAL_POPULATION_OFFSET: usize = 0x0050;
/// Win95 `dwNationalValue` (`0x4CB014`) / MISC `0x54`.
pub(super) const MISC_NATIONAL_VALUE_OFFSET: usize = 0x0054;
/// Win95 `wNationalTax` (`0x4CA938`) / MISC `0x58`.
pub(super) const MISC_NATIONAL_TAX_OFFSET: usize = 0x0058;
/// Win95 `wNationalEconomyTrend` (`0x4CA1BC`) / MISC `0x5c`.
pub(super) const MISC_NATIONAL_TREND_OFFSET: usize = 0x005c;
/// Win95 neighbors packed by `FUN_0043153a` after bonds: `[name, pop, value, fame] × 4`.
/// RAM is `wNeighborNameIdx` / `dwNeighborPopulation` / `dwNeighborValue` / `dwNeighborFame`.
pub(super) const MISC_NEIGHBOR_TABLE_OFFSET: usize = 0x06d8;
/// Win95 neighbor-fame rank table WORD[17] @ `0x4cb430` (init `0x41229e`, newspaper `0x4689c8`).
/// MISC base `0x0738` (17 dwords) — same slots as [`Technology`] discovery years.
pub(super) const PE_NEIGHBOR_FAME_RANK_COUNT: usize = 17;
pub(super) const MISC_NEIGHBOR_FAME_RANKS_OFFSET: usize = 0x0738;
/// Win95 EQ_LE 20-bucket packed tables at MISC dword index `0x1f` (`FUN_0043153a`).
/// Each bucket is `[cohort, aux, life]` (`0x4c94b4` / `0x4c94bc` / `0x4caa70`).
/// Industry `[demand, tax, pop] × 11` follows at `0x16c`; `dwTileCount` at `0x1F0`.
pub(super) const MISC_EQ_LE_TABLES_OFFSET: usize = 0x007c;
pub(super) const PE_EQ_LE_BUCKETS: usize = 20;
pub(super) const PE_EQ_LE_PACKED_DWORDS: usize = PE_EQ_LE_BUCKETS * 3;
/// Win95 `wIndustrialMixBonus` (`0x4CA1E8`) / MISC `0x1030`.
pub(super) const MISC_INDUSTRIAL_MIX_BONUS_OFFSET: usize = 0x1030;
/// Win95 `wIndustrialMixPollutionBonus` (`0x4C9428`) / MISC `0x1034`.
pub(super) const MISC_INDUSTRIAL_MIX_POLLUTION_BONUS_OFFSET: usize = 0x1034;
/// Win95 `wOldArrests` (`0x4CA430`) / MISC `0x1038`.
pub(super) const MISC_OLD_ARRESTS_OFFSET: usize = 0x1038;
/// Win95 `wPrisonBonus` (`0x4CA1DC`) / MISC `0x103C`.
pub(super) const MISC_PRISON_BONUS_OFFSET: usize = 0x103C;
/// Win95 `wSewerBonus` (`0x4CB3DC`) / MISC `0x1050`.
pub(super) const MISC_SEWER_BONUS_OFFSET: usize = 0x1050;
/// Win95 `wMonsterXTHGIndex` / `wDisasterObject` (`0x4CA81C`) / MISC `0x1040`.
pub(super) const MISC_MONSTER_XTHG_INDEX_OFFSET: usize = 0x1040;
/// Win95 `wCurrentDisasterType` (`0x4CA93C`) / MISC `0x1044`.
pub(super) const MISC_CURRENT_DISASTER_TYPE_OFFSET: usize = 0x1044;
/// Win95 `dwDisasterActive` (`0x4C9EE8`) / MISC `0x1048`.
pub(super) const MISC_DISASTER_ACTIVE_OFFSET: usize = 0x1048;
pub(super) const MISC_CITY_TRAFFIC_OFFSET: usize = 0x0030;
pub(super) const MISC_CITY_POLLUTION_OFFSET: usize = 0x0034;
/// Win95 MISC weather humid byte @ `+0x68` (PE BSS mirror `bWeatherHumid` `0x4cb3d4`).
pub(super) const MISC_HUMIDITY_OFFSET: usize = 0x0068;
/// Win95 MISC trigger-disaster word @ `+0x70` (PE BSS mirror `wSetTriggerDisasterType` `0x4ca420`).
pub(super) const MISC_TRIGGER_DISASTER_OFFSET: usize = 0x0070;
/// Mac CODE 4 pending-action selector `ffffd604`, saved at MISC `+0x70`.
pub(super) const MAC_PENDING_ACTION_MISC_OFFSET: usize = 0x0070;
/// Win95 MISC residential population dword @ `+0x74` (PE BSS mirror `0x4ca194`).
pub(super) const MISC_RESIDENTIAL_POPULATION_OFFSET: usize = 0x0074;
/// Win95 MISC rewards/available-buildings dword @ `+0x78` (PE BSS mirror `0x4e9a24`).
pub(super) const MISC_REWARDS_AVAILABLE_OFFSET: usize = 0x0078;
pub(super) const MISC_WATER_TABLE_LEVEL_OFFSET: usize = 0x0e40;
/// Win95 `dwCityHasOcean` (`0x4c94c0`) / MISC `0x0e44` (same dword as Edit Map Coast).
/// Hydrate `0x43189a`, persist `0x433ed8`.
pub(super) const MISC_CITY_HAS_OCEAN_OFFSET: usize = 0x0e44;
/// Win95 `dwCityHasRiver` (`0x4cb3f8`) / MISC `0x0e48` (same dword as Edit Map River).
/// Hydrate `0x4318ab`, persist `0x433eec`.
pub(super) const MISC_CITY_HAS_RIVER_OFFSET: usize = 0x0e48;
/// Win95 military base type / BSS `0x4cc4e4` (MISC `0x0e4c`); tech-unlock epilogue reads it.
pub(super) const MISC_MILITARY_BASE_TYPE_OFFSET: usize = 0x0e4c;
/// Mac CODE 4 slot 915 persists `ffffe0c2`, shared by the military capacity,
/// zone-7 growth, and phase-21 Dispatch Military availability selectors.
pub(super) const MAC_DISPATCH_SELECTOR_MISC_OFFSET: usize = 915 * 4;
/// Mac CODE 3 `export_163` table at `ffffaa32`, indexed by `ffffe0c2`.
pub(super) const MAC_DISPATCH_MILITARY_CAPACITY_BY_SELECTOR: [usize; 6] = [0, 0, 5, 2, 3, 0];
/// Win95 `dwArcologyPopulation` (`0x4c94c4`) / MISC `0x1020`.
pub(super) const MISC_ARCOLOGY_POPULATION_OFFSET: usize = 0x1020;
/// Win95 `wSportsTeams` (`0x4cc4e0`) / MISC `0x1028`. Bitmask `1/3/7/0xf/0x1f`, not stadium count.
pub(super) const MISC_SPORTS_TEAMS_OFFSET: usize = 0x1028;
/// Win95 `dwCityPopulation` (`0x4caa74`) / MISC `0x102c`. Day 22 snapshots
/// `pZonePops[0]*10` here; day 23 census rewrites the same MISC slot.
pub(super) const MISC_NORMAL_POPULATION_OFFSET: usize = 0x102c;
/// Win95 `0x4CC4D4` / MISC No Disasters option (same slot as the Options menu flag).
pub(super) const MISC_NO_DISASTERS_OFFSET: usize = 0x1000;
/// Win95 newspaper story records @ `0x4CA1C0`, saved as MISC `0x0EC8`.
pub(super) const MISC_NEWSPAPER_STORY_LIST_OFFSET: usize = 0x0EC8;
pub(super) const NEWSPAPER_STORY_RECORD_COUNT: usize = 9;
pub(super) const NEWSPAPER_STORY_MISC_FIELDS: usize = 6;
pub(super) const NEWSPAPER_STORY_LIST_MISC_DWORDS: usize =
    NEWSPAPER_STORY_RECORD_COUNT * NEWSPAPER_STORY_MISC_FIELDS;
/// Viewer extension snapshot of Win95 `dwBusPassengers` / `dwRailPassengers` /
/// `dwSubwayPassengers` (`0x4C85A0` / `0x4C85A4` / `0x4C8600`).
pub(super) const MISC_BUS_PASSENGERS_OFFSET: usize = 0x1120;
pub(super) const MISC_RAIL_PASSENGERS_OFFSET: usize = 0x1124;
pub(super) const MISC_SUBWAY_PASSENGERS_OFFSET: usize = 0x1128;
/// Win95 `0x4CAA50` / MISC power-used% (viewer extension slot matches day-1 persist).
pub(super) const MISC_POWER_USED_PERCENT_OFFSET: usize = 0x112c;
/// Win95 budget book in MISC (`TAX_MISC_OFFSET`). Service row 5 = police, 6 = fire.
pub(super) const MISC_BUDGET_BOOK_OFFSET: usize = 0x077c;
/// Mac save cursor slot 479: `FUN_0002e008` restores the 16×27-dword budget rows here.
pub(super) const MAC_MISC_BUDGET_BOOK_OFFSET: usize = 0x077c;
/// Mac CODE 4 saved WORD counters for export-80 XTXT-0xfa removal.
pub(super) const MAC_XTXT_FA_NON_NETWORK_COUNT_MISC_OFFSET: usize = 0x1024;
pub(super) const MAC_XTXT_FA_NETWORK_COUNT_MISC_OFFSET: usize = 0x1050;
/// Developed-city center words shared by Win95 and Mac save layouts.
pub(super) const MISC_DEVELOPED_CENTER_X_OFFSET: usize = 1030 * MISC_FIELD_LEN;
pub(super) const MISC_DEVELOPED_CENTER_Y_OFFSET: usize = 1031 * MISC_FIELD_LEN;

/// Mac `fffff00a` power summary, whose low WORD aliases `fffff00c`.
pub(super) const MAC_POWER_SUMMARY_MISC_OFFSET: usize = 1048 * MISC_FIELD_LEN;
/// Mac `FUN_0002e008` restores selected terrain/water height from packed MISC slot 912.
/// This is the final cursor-derived field after the budget rows, not fixed slot 53.
pub(super) const MAC_SELECTED_WATER_HEIGHT_STATE_OFFSET: usize = 912 * 4;
/// Mac `fffffdf0` scenario-active byte is the low byte of saved state slot 1050.
pub(super) const MAC_SCENARIO_ACTIVE_STATE_OFFSET: usize = 1050 * 4;
/// Mac CODE 8 weather state F084..F087 is stored in the low byte of MISC slots 24..27.
pub(super) const MAC_WEATHER_STATE_BASE_OFFSET: usize = 24 * MISC_FIELD_LEN;
/// Mac `FUN_0002e008` restores `fffff086` from MISC dword 26; `FUN_0002eb8e`
/// widens that byte back into the same saved dword.
pub(super) const MAC_WEATHER_HUMIDITY_STATE_OFFSET: usize = MAC_WEATHER_STATE_BASE_OFFSET + 2 * 4;
pub(super) const MISC_BUDGET_ROW_DWORDS: usize = 27;
pub(super) const MISC_BUDGET_ROW_COUNT: usize = 16;
pub(super) const MISC_BUDGET_FUNDING_FIELD: usize = 1;
pub(super) const MISC_BUDGET_SERVICE_ROW_START: usize = 5;
pub(super) const DEFAULT_SERVICE_FUNDING: u32 = 100;
/// Win95 budget tax-rate fields in MISC (`TAX_MISC_OFFSET` + row stride).
pub(super) const MISC_TAX_RATE_R_OFFSET: usize = 0x0780;
pub(super) const MISC_TAX_RATE_C_OFFSET: usize = 0x07ec;
pub(super) const MISC_TAX_RATE_I_OFFSET: usize = 0x0858;
pub(super) const DEFAULT_TAX_RATE: u32 = 7;
pub(super) const MISC_FIELD_LEN: usize = 4;
pub(super) const DAYS_PER_YEAR: u32 = 300;
pub(super) const DAYS_PER_MONTH: u32 = 25;
