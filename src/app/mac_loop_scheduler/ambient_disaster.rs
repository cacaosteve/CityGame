//! CODE 7 export 231, called at the end of calendar phase 24.

use super::*;

impl ViewerState {
    pub(in crate::app) fn run_mac_phase24_disaster_chooser(&mut self) {
        let scheduler = &mut self.mac_loop_scheduler;
        let selector = propose(
            &self.city,
            self.no_disasters_enabled,
            &mut self.mac_process_rng,
            scheduler.disaster_point,
            &mut scheduler.pending_action_point,
        );
        if let Some(point) = scheduler.pending_action_point {
            scheduler.disaster_point = point;
        }
        if let Some(selector) = selector {
            // The native pending selector is a single word. The actor sweep
            // may have written it earlier in this same CODE 2 iteration.
            self.city.set_mac_pending_action(u32::from(selector));
        }
    }
}

fn propose(
    city: &City,
    no_disasters: bool,
    rng: &mut MacToolboxRng,
    retained_point: (i16, i16),
    point: &mut Option<(i16, i16)>,
) -> Option<u16> {
    let misc = |offset| city.mac_state_misc_u32_at(offset).unwrap_or(0);
    let trend = misc(MISC_WEATHER_TREND_OFFSET) as u8;
    let growth = misc(MAC_MASS_RIOT_GROWTH_STATE_OFFSET);
    if recreation_story_due(city, trend, growth) {
        // 0x358ac selects one of four recreation stories. Its draw precedes
        // both No Disasters and the age gate.
        rng.random_u16();
    }

    let difficulty = misc(MISC_DIFFICULTY_OFFSET) as u16;
    let months = *[0, 100, 60, 30].get(usize::from(difficulty))?;
    let city_months = misc(MISC_CITY_AGE_OFFSET) as i32 / 25;
    if no_disasters || months == 0 || city_months < i32::from(months) {
        return None;
    }
    let chance = rng.random_u16() % months;
    let coast = misc(EDIT_MAP_COAST_MISC_OFFSET) as u8 != 0;
    let river = misc(EDIT_MAP_RIVER_MISC_OFFSET) as u8 != 0;
    let selector = match (trend, chance) {
        (WEATHER_TREND_HURRICANE, 0..=14) if coast => 16,
        (WEATHER_TREND_TORNADO, 0..=14) => 7,
        (_, 0) => rng.random_u16() % 19,
        _ => return None,
    };

    let heat = u16::from(misc(MAC_RIOT_WEATHER_HEAT_STATE_OFFSET) as u8);
    let interior = |rng: &mut MacToolboxRng| {
        let (x, y) = mac_random_interior_map_tile(rng);
        (x as i16, y as i16)
    };
    let jitter = |rng: &mut MacToolboxRng, offset| {
        let (x, y) = city.mac_kind2_hotspot().unwrap_or_default();
        let dx = (rng.random_u16() & 31) as i16 - offset;
        let dy = (rng.random_u16() & 31) as i16 - offset;
        ((x as i16).wrapping_add(dx), (y as i16).wrapping_add(dy))
    };
    let proposed_point = match selector {
        1 if (rng.random_u16() & 127) + 127 <= heat => Some(interior(rng)),
        2 if (coast || river) && trend >= WEATHER_TREND_FOGGY => Some(interior(rng)),
        3 | 13 if (selector == 3 || growth >= 30_000) && mac_riot_disaster_ready(city) => {
            Some(jitter(rng, 16))
        }
        4 => {
            // The scan clears the Point's X word even if no cell qualifies.
            *point = Some((-1, point.unwrap_or(retained_point).1));
            let mut lfsr = MacAppLfsrRng::new(MacAppLfsrRng::INITIAL_STATE);
            let (x, y) = native_toxic_spill_target_tile(city, || usize::from(lfsr.bounded(10)))?;
            Some((x as i16, y as i16))
        }
        6 => Some(interior(rng)),
        7 if trend >= WEATHER_TREND_WINDY => Some(interior(rng)),
        8 if growth >= 45_000 => Some(jitter(rng, 15)),
        facility_disaster::SCATTER_ACTION if city.mac_xbld_tile_count(0xcb) != 0 => None,
        facility_disaster::WALK_ACTION if city.mac_xbld_tile_count(0xcd) != 0 => None,
        14 if (coast || river) && trend >= WEATHER_TREND_BLIZZARD => Some(interior(rng)),
        15 if budget_current(city, 2) >= 10_000 => {
            // The native global pair at ffffe4b4/e4b6 is the developed-map
            // hotspot refreshed by the Mac derived-map pass. It is distinct
            // from the scheduler's retained action Point, so use the same
            // hotspot source as selectors 3 and 8.
            Some(jitter(rng, 15))
        }
        16 if coast && trend >= WEATHER_TREND_WINDY => None,
        18 if city.mac_xbld_tile_count(0xdd) != 0 => Some(interior(rng)),
        _ => return None,
    };
    if let Some(proposed_point) = proposed_point {
        *point = Some(proposed_point);
    }
    Some(selector)
}

fn budget_current(city: &City, row: usize) -> i32 {
    budget_row_misc_offset_for_city(city, row, BUDGET_SERVICE_CURRENT_FIELD)
        .and_then(|offset| city.budget_misc_u32_at(offset))
        .unwrap_or(0) as i32
}

/// Only the recreation-shortage branch in the story prelude draws _Random.
/// Earlier branches produce presentation state without advancing the stream.
fn recreation_story_due(city: &City, trend: u8, growth: u32) -> bool {
    let count = |tile| city.mac_xbld_tile_count(tile);
    let transit = u32::from(count(0xe9))
        .wrapping_add(u32::from(count(0xed)))
        .wrapping_add(budget_current(city, 10) as u32);
    if trend >= 9
        || city.mac_power_service_summary() > 98
        || transit <= growth / 100
        || growth < 1_000
        || u32::from(count(0xd2) / 9 + count(0xd8) / 16) <= growth / 20_000
        || u32::from(count(0xd3) / 9) <= growth / 20_000
        || city.mac_water_service_summary() > 98
        || growth < 3_000
        || u32::from(count(0xd1) / 9) <= growth / 25_000
        || u32::from(count(0xd6) / 9) <= growth / 20_000
        || growth < 8_000
    {
        return false;
    }

    let arcology_share = city.mac_arcology_resident_total() as i32 / 12;
    let [network, non_network] = city.mac_rci_overlay_cap_counts();
    let seaport = i32::from(count(0xe0).wrapping_add(non_network));
    let airport = i32::from(count(0xdd).wrapping_add(count(0xde)).wrapping_add(network));
    let recreation = i32::from(
        count(0xd7)
            .wrapping_add(count(0xf8))
            .wrapping_add(count(0xda))
            .wrapping_add(count(0xd5) / 3),
    );
    seaport >= budget_current(city, 2).wrapping_sub(arcology_share) / 10_000
        && airport >= budget_current(city, 1).wrapping_sub(arcology_share) / 2_000
        && recreation < budget_current(city, 0).wrapping_sub(arcology_share * 2) / 1_000
}
