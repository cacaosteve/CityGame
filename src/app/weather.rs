//! Shared Win95 and Mac weather transition tables and state helpers.
//!
//! Both binaries use the same season×trend×8 transition table and per-trend
//! heat/wind/humidity targets. Season is `(month_index + 1) % 12 / 3` (0..3).
//! The transition math is shared. Mac's phase-21 worker models its local
//! `export_237` map, event, technology, and weather draws; the RNG input is
//! not session-aligned because the preceding natural CODE 2/6/8 schedule and
//! its shared-stream interleaving are not yet reproduced.

use super::*;
use crate::mac_random::MacToolboxRng;
use crate::win95_random::Win95CrtRand;

pub(super) const WEATHER_TREND_COUNT: usize = 12;
pub(super) const WEATHER_SEASON_COUNT: usize = 4;
pub(super) const WEATHER_TRANSITION_CHOICES: usize = 8;

pub(super) const WEATHER_TREND_COLD: u8 = 0;
pub(super) const WEATHER_TREND_CLEAR: u8 = 1;
pub(super) const WEATHER_TREND_HOT: u8 = 2;
pub(super) const WEATHER_TREND_FOGGY: u8 = 3;
pub(super) const WEATHER_TREND_CHILLY: u8 = 4;
pub(super) const WEATHER_TREND_OVERCAST: u8 = 5;
pub(super) const WEATHER_TREND_SNOW: u8 = 6;
pub(super) const WEATHER_TREND_RAIN: u8 = 7;
pub(super) const WEATHER_TREND_WINDY: u8 = 8;
pub(super) const WEATHER_TREND_BLIZZARD: u8 = 9;
pub(super) const WEATHER_TREND_HURRICANE: u8 = 10;
pub(super) const WEATHER_TREND_TORNADO: u8 = 11;

/// Default heat after new-city init (`FUN_0040e250`).
pub(super) const DEFAULT_WEATHER_HEAT: u32 = 0x96;
/// Default wind / PE `0x004CAE0C` after new-city init.
pub(super) const DEFAULT_WEATHER_WIND: u32 = 0x0a;
/// Default humidity / PE `0x004CB3D4` after new-city init (`FUN_0040e250`).
pub(super) const DEFAULT_WEATHER_HUMID: u32 = 0x0f;
/// Initial weather trend / PE global `0x004CB40C` after new-city init.
pub(super) const DEFAULT_WEATHER_TREND: u32 = WEATHER_TREND_CHILLY as u32;

/// Per-trend heat / wind / humidity targets @ Win95 `SIMCITY.EXE` `0x4dc038` / `048` / `058`.
/// Both engine variants use the same target arrays.
pub(super) const WEATHER_HEAT_TARGETS: [u8; WEATHER_TREND_COUNT] =
    [80, 165, 210, 100, 145, 175, 80, 150, 175, 60, 140, 175];
pub(super) const WEATHER_WIND_TARGETS: [u8; WEATHER_TREND_COUNT] =
    [15, 30, 0, 0, 15, 5, 10, 30, 60, 100, 100, 100];
pub(super) const WEATHER_HUMID_TARGETS: [u8; WEATHER_TREND_COUNT] =
    [0, 0, 0, 15, 15, 15, 30, 30, 30, 60, 60, 60];

/// Season×trend×8 next-trend table @ Win95 `0x4dbeb8` (row = season*12 + trend).
/// The Mac CODE 8 selector uses the same layout, and its DATA table is byte-identical.
pub(super) const WEATHER_TRANSITIONS: [[[u8; WEATHER_TRANSITION_CHOICES]; WEATHER_TREND_COUNT];
    WEATHER_SEASON_COUNT] = [
    [
        [0, 0, 0, 3, 4, 5, 1, 8],
        [1, 1, 1, 0, 0, 3, 4, 8],
        [2, 1, 1, 1, 8, 8, 5, 3],
        [3, 3, 0, 1, 4, 5, 8, 7],
        [4, 4, 0, 1, 4, 5, 7, 6],
        [5, 5, 1, 4, 3, 7, 7, 8],
        [6, 6, 7, 7, 5, 4, 0, 9],
        [7, 7, 7, 6, 8, 8, 4, 3],
        [8, 8, 8, 7, 7, 5, 4, 4],
        [9, 6, 6, 6, 6, 7, 7, 3],
        [7, 7, 6, 6, 7, 7, 7, 8],
        [8, 8, 8, 8, 8, 7, 6, 4],
    ],
    [
        [0, 0, 0, 1, 1, 1, 3, 4],
        [1, 1, 1, 1, 2, 2, 0, 4],
        [2, 2, 2, 1, 1, 5, 4, 3],
        [3, 3, 0, 8, 1, 1, 4, 7],
        [4, 4, 4, 1, 8, 0, 5, 3],
        [5, 5, 5, 7, 2, 4, 1, 8],
        [6, 7, 7, 7, 3, 8, 5, 5],
        [7, 7, 7, 6, 3, 5, 8, 10],
        [8, 8, 8, 7, 5, 4, 1, 1],
        [7, 6, 6, 7, 7, 8, 8, 4],
        [10, 7, 7, 7, 8, 8, 4, 4],
        [8, 8, 8, 8, 8, 4, 1, 2],
    ],
    [
        [0, 0, 0, 1, 1, 1, 3, 4],
        [1, 1, 1, 1, 2, 2, 0, 4],
        [2, 2, 2, 1, 1, 5, 4, 3],
        [3, 3, 0, 8, 1, 1, 4, 7],
        [4, 4, 4, 1, 8, 0, 5, 3],
        [5, 5, 5, 7, 2, 4, 1, 8],
        [6, 7, 7, 7, 3, 8, 5, 4],
        [7, 7, 7, 6, 3, 5, 8, 8],
        [8, 8, 8, 7, 5, 4, 1, 11],
        [7, 6, 6, 7, 7, 8, 8, 4],
        [6, 7, 7, 7, 8, 8, 4, 4],
        [11, 8, 8, 8, 8, 4, 1, 2],
    ],
    [
        [0, 0, 0, 3, 4, 5, 1, 8],
        [1, 1, 1, 0, 0, 3, 4, 8],
        [2, 1, 1, 1, 8, 8, 5, 3],
        [3, 3, 0, 1, 4, 5, 8, 7],
        [4, 4, 0, 1, 4, 5, 7, 6],
        [5, 5, 1, 4, 3, 7, 7, 8],
        [6, 6, 7, 7, 5, 4, 0, 0],
        [7, 7, 7, 6, 8, 4, 3, 10],
        [8, 8, 8, 7, 7, 5, 4, 5],
        [6, 6, 6, 6, 7, 7, 7, 3],
        [10, 7, 6, 6, 7, 7, 7, 8],
        [8, 8, 8, 8, 8, 7, 6, 4],
    ],
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct WeatherState {
    pub heat: u32,
    pub wind: u32,
    pub humidity: u32,
    pub trend: u8,
    pub season: u8,
}

pub(super) fn weather_season_for_month(month: u8) -> u8 {
    // Win95: `wCityCurrentSeason = (wCityCurrentMonth + 1) % 12 / 3`
    // with `wCityCurrentMonth` in 0..11.
    let month_index = u32::from(month.saturating_sub(1)) % 12;
    (((month_index + 1) % 12) / 3) as u8
}

pub(super) fn weather_state_for_city(city: &City, month: u8) -> WeatherState {
    // PE weather @ `0x468a28` reads season BSS `0x4cb3e8`; fall back to calendar month.
    let season = if city.pe_w_season() != 0 || city.pe_dw_city_age > 0 {
        city.pe_w_season().min(3) as u8
    } else {
        weather_season_for_month(month)
    };
    WeatherState {
        heat: u32::from(city.pe_b_weather_heat).min(255),
        wind: u32::from(city.pe_b_weather_wind).min(255),
        humidity: u32::from(city.pe_b_weather_humid).min(255),
        trend: city.pe_b_weather_trend.min(WEATHER_TREND_TORNADO),
        season,
    }
}

/// PE disaster/newspaper arms read BSS `bWeatherTrend` @ `0x4cb40c`.
pub(super) fn pe_weather_trend(city: &City) -> u8 {
    city.pe_b_weather_trend.min(WEATHER_TREND_TORNADO)
}

pub(super) fn weather_status_label(city: &City) -> &'static str {
    [
        "Cold weather",
        "Clear skies",
        "Hot weather",
        "Foggy weather",
        "Chilly weather",
        "Overcast skies",
        "Snow expected",
        "Rain expected",
        "Windy weather",
        "Blizzard warning",
        "Hurricane warning",
        "Tornado warning",
    ][usize::from(pe_weather_trend(city))]
}

/// PE disaster/newspaper arms read BSS `dwCityHasRiver` @ `0x4cb3f8` (MISC `0x0e48`).
pub(super) fn pe_city_has_river(city: &City) -> bool {
    city.pe_dw_city_has_river() != 0
}

/// PE disaster/newspaper arms read BSS `dwCityHasOcean` @ `0x4c94c0` (MISC `0x0e44`).
pub(super) fn pe_city_has_ocean(city: &City) -> bool {
    city.pe_dw_city_has_ocean() != 0
}

/// Read the Mac CODE 8 weather bytes from their native saved slots. Those
/// fields occupy only the low byte of MISC dwords 24..27; PE's cached weather
/// fields interpret the whole dword and can therefore include unrelated upper
/// bytes preserved by Mac saves.
pub(super) fn mac_weather_state_for_city(city: &City, month: u8) -> WeatherState {
    let low_byte = |slot: usize| {
        city.mac_state_misc_u32_at(slot * 4)
            .map(|value| (value & 0xff) as u8)
            .unwrap_or(0)
    };
    WeatherState {
        heat: u32::from(low_byte(24)),                  // F084
        wind: u32::from(low_byte(25)),                  // F085
        humidity: u32::from(low_byte(26)),              // F086
        trend: low_byte(27).min(WEATHER_TREND_TORNADO), // F087
        // CODE 3 derives the Mac season from the current calendar month.
        season: weather_season_for_month(month),
    }
}

fn ease_toward(current: u32, target: u8) -> u32 {
    ((current + u32::from(target)) / 2).min(255)
}

fn advance_weather_state_with_choice(state: WeatherState, choice: usize) -> WeatherState {
    let season = usize::from(state.season.min((WEATHER_SEASON_COUNT - 1) as u8));
    let trend = usize::from(state.trend.min(WEATHER_TREND_TORNADO));
    let next_trend = WEATHER_TRANSITIONS[season][trend][choice].min(WEATHER_TREND_TORNADO);
    let next = usize::from(next_trend);
    WeatherState {
        heat: ease_toward(state.heat, WEATHER_HEAT_TARGETS[next]),
        wind: ease_toward(state.wind, WEATHER_WIND_TARGETS[next]),
        humidity: ease_toward(state.humidity, WEATHER_HUMID_TARGETS[next]),
        trend: next_trend,
        season: state.season,
    }
}

/// Mac CODE 8 `export_237` weather-tail transition: consume its final
/// `_Random` result and apply the shared table. This helper does not reproduce
/// the publisher's preceding conditional draw schedule.
pub(super) fn advance_weather_state(state: WeatherState, rng: &mut MacToolboxRng) -> WeatherState {
    let choice = (rng.random_i16() as u16 & 7) as usize;
    advance_weather_state_with_choice(state, choice)
}

/// Win95 `SimulationRCIDemandUpdates` weather transition at `0x468a34`:
/// `rand()` (`0x48cdd8`) masked with `7` selects the next-trend table entry.
pub(super) fn advance_weather_state_win95(
    state: WeatherState,
    rng: &mut Win95CrtRand,
) -> WeatherState {
    let choice = usize::from(rng.random_u15() & 7);
    advance_weather_state_with_choice(state, choice)
}

pub(super) fn persist_weather_state_to_city(city: &mut City, state: WeatherState) {
    if city.kind == CityFileKind::MacCity {
        // Mac CODE 8 stores these four weather values in the low byte of MISC
        // dwords 24..27. A Win95 simulation over a Mac-layout file shares those
        // byte fields, but must preserve the unrelated upper bytes.
        persist_mac_weather_state_to_city(city, state);
        city.pe_b_weather_heat = state.heat.min(255) as u8;
        city.pe_b_weather_wind = state.wind.min(255) as u8;
        city.pe_b_weather_humid = state.humidity.min(255) as u8;
        city.pe_b_weather_trend = state.trend.min(WEATHER_TREND_TORNADO);
        return;
    }
    city.set_misc_u32_at(super::MISC_HEAT_OFFSET, state.heat);
    city.set_misc_u32_at(super::MISC_WIND_OFFSET, state.wind);
    city.set_misc_u32_at(super::MISC_HUMID_OFFSET, state.humidity);
    city.set_misc_u32_at(super::MISC_WEATHER_TREND_OFFSET, u32::from(state.trend));
}

/// Mac CODE 8 stores F084..F087 as byte state in four consecutive MISC dwords.
/// Keep the upper bytes intact; the PE save mirror writes full dwords instead.
pub(super) fn persist_mac_weather_state_to_city(city: &mut City, state: WeatherState) {
    for (offset, value) in [
        (24 * 4, state.heat),
        (25 * 4, state.wind),
        (26 * 4, state.humidity),
        (27 * 4, u32::from(state.trend)),
    ] {
        city.set_mac_state_misc_low_byte_at(offset, value.min(255) as u8);
    }
    // For Win-layout files, City::save mirrors the PE weather cache back as
    // full DWORDs. Keep those caches aligned with the Mac-owned low bytes so
    // saving under Mac fidelity does not restore stale PE weather values.
    if city.kind.is_win95() {
        city.pe_b_weather_heat = state.heat.min(255) as u8;
        city.pe_b_weather_wind = state.wind.min(255) as u8;
        city.pe_b_weather_humid = state.humidity.min(255) as u8;
        city.pe_b_weather_trend = state.trend.min(WEATHER_TREND_TORNADO);
    }
}

pub(super) fn weather_advance_seed(city: &City, clock: GameClock) -> u32 {
    let mut seed = 0x5745_4154u32; // "WEAT"
    for byte in city.name.bytes() {
        seed = seed.wrapping_mul(33).wrapping_add(u32::from(byte));
    }
    seed ^= u32::from(clock.year()).wrapping_mul(0x0100_0193);
    seed ^= u32::from(clock.month()) << 16;
    seed ^= u32::from(clock.day()) << 8;
    // Weather seed folds BSS `bWeatherTrend` @ `0x4cb40c` (same live source as gates).
    seed ^= u32::from(city.pe_b_weather_trend).wrapping_mul(0x9E37_79B9);
    seed
}

/// Win95 day-24 hurricane/tornado gate when the matching weather trend is active (~15%).
pub(super) fn weather_disaster_roll_succeeds(rng: &mut Win95CrtRand) -> bool {
    rng.random_u15() % 100 < 15
}
