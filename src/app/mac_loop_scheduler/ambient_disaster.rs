//! CODE 7 export 231, called on city open and at the end of calendar phase 24.

use super::*;

impl ViewerState {
    pub(in crate::app) fn run_mac_disaster_chooser(&mut self) {
        let scheduler = &mut self.mac_loop_scheduler;
        let selector = propose(
            &mut self.city,
            self.no_disasters_enabled,
            &mut self.mac_process_rng,
            &mut self.mac_app_lfsr,
            scheduler.disaster_point,
            &mut scheduler.pending_action_point,
            &mut scheduler.status_message,
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
    city: &mut City,
    no_disasters: bool,
    rng: &mut MacToolboxRng,
    lfsr: &mut MacAppLfsrRng,
    retained_point: (i16, i16),
    point: &mut Option<(i16, i16)>,
    status: &mut Option<MacStatusMessage>,
) -> Option<u16> {
    let trend = city
        .mac_state_misc_u32_at(MISC_WEATHER_TREND_OFFSET)
        .unwrap_or(0) as u8;
    let growth = city
        .mac_state_misc_u32_at(MAC_MASS_RIOT_GROWTH_STATE_OFFSET)
        .unwrap_or(0);
    let story = status_story_selector(city, trend, growth, rng);
    *status = if (9..=11).contains(&trend) {
        Some(MacStatusMessage::Weather(trend))
    } else {
        story.map(MacStatusMessage::Story)
    };
    if let Some(selector) = story {
        city.queue_mac_newspaper_story(selector, 0);
    }

    let misc = |offset| city.mac_state_misc_u32_at(offset).unwrap_or(0);
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
            let (x, y) = native_toxic_spill_target_tile(city, || usize::from(lfsr.bounded(10)))?;
            Some((x as i16, y as i16))
        }
        6 => Some(interior(rng)),
        7 if trend >= WEATHER_TREND_WINDY => Some(interior(rng)),
        8 if growth >= 45_000 => Some(jitter(rng, 15)),
        facility_disaster::SCATTER_ACTION if city.mac_xbld_tile_count(0xcb) != 0 => {
            return Some(selector);
        }
        facility_disaster::WALK_ACTION if city.mac_xbld_tile_count(0xcd) != 0 => {
            return Some(selector);
        }
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

/// CODE 7's ordered status gates at 0x35652..0x358de.
fn status_story_selector(
    city: &City,
    trend: u8,
    growth: u32,
    rng: &mut MacToolboxRng,
) -> Option<u16> {
    if trend >= 9 {
        return None;
    }
    if city.mac_power_service_summary() > 98 {
        return Some(46);
    }
    let count = |tile| city.mac_xbld_tile_count(tile);
    let transit = u32::from(count(0xe9))
        .wrapping_add(u32::from(count(0xed)))
        .wrapping_add(budget_current(city, 10) as u32);
    if growth / 100 >= transit {
        return Some(47);
    }
    if growth < 1_000 {
        return None;
    }
    if growth / 20_000 >= u32::from(count(0xd2) / 9 + count(0xd8) / 16) {
        return Some(48);
    }
    if growth / 20_000 >= u32::from(count(0xd3) / 9) {
        return Some(49);
    }
    if city.mac_water_service_summary() > 98 {
        return Some(50);
    }
    if growth < 3_000 {
        return None;
    }
    if growth / 25_000 >= u32::from(count(0xd1) / 9) {
        return Some(51);
    }
    if growth / 20_000 >= u32::from(count(0xd6) / 9) {
        return Some(52);
    }
    if growth >= 8_000 {
        let arcology = city.mac_arcology_resident_total() as i32 / 12;
        let [_, non_network] = city.mac_rci_overlay_cap_counts();
        let seaport = i32::from(count(0xe0).wrapping_add(non_network));
        if seaport < budget_current(city, 2).wrapping_sub(arcology) / 10_000 {
            let coast = city
                .mac_state_misc_u32_at(EDIT_MAP_COAST_MISC_OFFSET)
                .unwrap_or(0) as u8
                != 0;
            let river = city
                .mac_state_misc_u32_at(EDIT_MAP_RIVER_MISC_OFFSET)
                .unwrap_or(0) as u8
                != 0;
            return Some(if coast || river { 53 } else { 59 });
        }
        let [network, _] = city.mac_rci_overlay_cap_counts();
        let airport = i32::from(count(0xdd).wrapping_add(count(0xde)).wrapping_add(network));
        if airport < budget_current(city, 1).wrapping_sub(arcology) / 2_000 {
            // CODE 8 export 240 copies command 8's default mask (1), then
            // sets bit 1 when the airport technology WORD is zero.
            let date = city
                .mac_state_misc_u32_at(crate::city::Technology::Airport.misc_offset())
                .unwrap_or(0) as u16;
            return Some(if date == 0 { 54 } else { 60 });
        }
    }
    if recreation_story_due(city, trend, growth) {
        return Some(55 + (rng.random_u16() & 3));
    }
    None
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::tests::{blank_city_layout_fixture, test_state};

    #[test]
    fn natural_tornado_shortcut_replays_captured_native_seed_and_point() {
        for path in ["natural-tornado.city", "natural-tornado.sc2"] {
            let mut city = blank_city_layout_fixture(path, "Natural Tornado");
            city.set_mac_state_misc_u32_at(MISC_CITY_AGE_OFFSET, 0x15b8e);
            city.set_mac_state_misc_u32_at(MISC_DIFFICULTY_OFFSET, 1);
            city.set_mac_state_misc_u32_at(MISC_WEATHER_TREND_OFFSET, 11);

            // At native age 0x15b8e the QuickDraw stream enters CODE 7 with
            // seed 0x2ebe18c2. The chance result advances to 0x424b8087
            // (low word % 100 == 3), followed by the captured X/Y results.
            let mut rng = MacToolboxRng::new(0x2ebe18c2);
            let mut lfsr = MacAppLfsrRng::new(MacAppLfsrRng::INITIAL_STATE);
            let mut point = None;
            let mut status = None;
            let selector = propose(
                &mut city,
                false,
                &mut rng,
                &mut lfsr,
                (0, 0),
                &mut point,
                &mut status,
            );

            assert_eq!(selector, Some(7), "{path}");
            assert_eq!(point, Some((26, 96)), "{path}");
            assert_eq!(rng.seed(), 0x6dc4f9e9, "{path}");
            assert_eq!(status, Some(MacStatusMessage::Weather(11)), "{path}");
            assert_eq!(lfsr.state(), MacAppLfsrRng::INITIAL_STATE, "{path}");
        }
    }

    #[test]
    fn captured_natural_tornado_runs_through_citygame_dispatch_and_constructor() {
        for path in [
            "natural-tornado-dispatch.city",
            "natural-tornado-dispatch.sc2",
        ] {
            let mut city = blank_city_layout_fixture(path, "Natural Tornado Dispatch");
            city.set_mac_state_misc_u32_at(MISC_CITY_AGE_OFFSET, 0x15b8e);
            city.set_mac_state_misc_u32_at(MISC_DIFFICULTY_OFFSET, 1);
            city.set_mac_state_misc_u32_at(MISC_WEATHER_TREND_OFFSET, 11);
            city.set_mac_state_misc_u32_at(MISC_CITY_MODE_OFFSET, CITY_MODE_GAMEPLAY);
            city.set_altm_word(26, 96, 0x02ab);
            city.set_text_overlay(26, 96, 0x22);
            city.mac_object_slot_records_mut()[1].saved_b = 0x5a;

            let mut state = test_state(city);
            state.engine_fidelity = EngineFidelity::Mac68k;
            state.traffic.set_speed(GameSpeed::Paused);
            state.no_disasters_enabled = false;
            state.mac_process_rng = MacToolboxRng::new(0x2ebe18c2);
            state.mac_app_lfsr = MacAppLfsrRng::new(MacAppLfsrRng::INITIAL_STATE);

            // Reproduce the captured CODE 7 proposal through ViewerState,
            // then let the ordinary paused-mode pending-action poll dispatch it.
            state.run_mac_disaster_chooser();
            assert_eq!(state.city.mac_pending_action(), 7, "{path}");
            assert_eq!(
                state.mac_loop_scheduler.pending_action_point,
                Some((26, 96)),
                "{path}"
            );
            assert_eq!(state.mac_process_rng.seed(), 0x6dc4f9e9, "{path}");

            assert_eq!(state.advance_frame_simulation(Duration::ZERO).days, 0);
            assert_eq!(state.city.mac_pending_action(), 0, "{path}");
            assert!(
                scenario_action::mac_scenario_action_active(&state.city),
                "{path}"
            );
            assert_eq!(state.city.mac_state_misc_u32_at(0x1044), Some(7), "{path}");
            assert_eq!(
                state.city.traffic_overlays().unwrap()[1].to_bytes(),
                [15, 4, 0, 26, 96, 11, 8, 8, 115, 72, 0x22, 0x5a],
                "{path}"
            );
            assert_eq!(state.city.text_overlay_at(26, 96), Some(0xca), "{path}");
            assert_eq!(state.city.mac_tornado_actor_counter, 1, "{path}");
            assert_eq!(state.mac_process_rng.seed(), 0x3c60a1c8, "{path}");
            assert_eq!(
                state.mac_app_lfsr.state(),
                MacAppLfsrRng::INITIAL_STATE,
                "{path}"
            );
        }
    }

    #[test]
    fn captured_live_quadra_tornado_constructor_matches_published_actor_record() {
        for path in ["live-natural-tornado.city", "live-natural-tornado.sc2"] {
            let mut city = blank_city_layout_fixture(path, "Live Natural Tornado");
            city.set_altm_word(103, 33, 4);
            city.set_text_overlay(103, 33, 0);
            // The native allocator reuses slot 1 without clearing byte +11.
            city.mac_object_slot_records_mut()[1].saved_b = 1;

            // Captured at the natural CODE 10 constructor entry on the Quadra.
            // This constructor makes three direct QuickDraw calls (direction,
            // target X, target Y); the native view helper accounts for two
            // more before the captured constructor return.
            let mut rng = MacToolboxRng::new(0x5035_0296);
            assert!(scenario_action::seed_tornado(
                &mut city,
                (103, 33),
                &mut rng,
            ));

            assert_eq!(
                city.traffic_overlays().unwrap()[1].to_bytes(),
                [0x0f, 5, 0, 103, 33, 4, 8, 8, 24, 47, 0, 1],
                "{path}"
            );
            assert_eq!(city.text_overlay_at(103, 33), Some(0xca), "{path}");
            assert_eq!(city.mac_tornado_actor_counter, 1, "{path}");
            assert_eq!(city.mac_state_misc_u32_at(0x1044), Some(7), "{path}");
            assert_eq!(rng.seed(), 0x7080_d32f, "{path}");
        }
    }

    #[test]
    fn captured_natural_tornado_dispatch_matches_native_seed_through_autogoto_redraw() {
        for path in [
            "live-natural-tornado-dispatch.city",
            "live-natural-tornado-dispatch.sc2",
        ] {
            let mut city = blank_city_layout_fixture(path, "Live Natural Tornado Dispatch");
            city.set_mac_state_misc_u32_at(MISC_CITY_MODE_OFFSET, CITY_MODE_GAMEPLAY);
            city.set_altm_word(103, 33, 4);
            city.set_text_overlay(103, 33, 0);
            city.mac_object_slot_records_mut()[1].saved_b = 1;

            let mut state = test_state(city);
            state.engine_fidelity = EngineFidelity::Mac68k;
            state.traffic.set_speed(GameSpeed::Paused);
            state.auto_goto_enabled = true;
            state.mac_loop_scheduler.set_viewport_size((640, 480));
            state.mac_loop_scheduler.pending_action_point = Some((103, 33));
            state
                .city
                .set_mac_pending_action(u32::from(scenario_action::TORNADO_ACTION));
            state.mac_process_rng = MacToolboxRng::new(0x5035_0296);

            let mut now = 0;
            let mut music_callback = |_state: &mut ViewerState| {};
            assert!(
                state.consume_mac_pending_action(&mut now, &mut music_callback),
                "{path}"
            );
            assert_eq!(state.city.mac_pending_action(), 0, "{path}");
            assert_eq!(
                state.city.traffic_overlays().unwrap()[1].to_bytes(),
                [0x0f, 5, 0, 103, 33, 4, 8, 8, 24, 47, 0, 1],
                "{path}"
            );
            assert_eq!(state.city.text_overlay_at(103, 33), Some(0xca), "{path}");
            assert_eq!(state.city.mac_tornado_actor_counter, 1, "{path}");
            let markers = (0..128)
                .flat_map(|y| (0..128).map(move |x| (x, y)))
                .filter_map(|(x, y)| {
                    state
                        .city
                        .text_overlay_at(x, y)
                        .filter(|marker| {
                            (0xc9..=0xf1).contains(marker) || (0xfb..=0xff).contains(marker)
                        })
                        .map(|marker| (x, y, marker))
                })
                .collect::<Vec<_>>();
            // The constructor consumes three draws. The real Auto-Goto view
            // walk sees this Tornado marker and consumes the remaining pair.
            assert_eq!(
                state.mac_process_rng.seed(),
                0x2135_dad4,
                "{path} markers={markers:?}"
            );
            assert_eq!(
                state.mac_app_lfsr.state(),
                MacAppLfsrRng::INITIAL_STATE,
                "{path}"
            );
        }
    }

    #[test]
    fn captured_natural_earthquake_proposal_uses_shared_rng_and_dispatches() {
        for path in ["natural-earthquake.city", "natural-earthquake.sc2"] {
            let mut city = blank_city_layout_fixture(path, "Natural Earthquake");
            city.set_mac_state_misc_u32_at(MISC_CITY_AGE_OFFSET, 0x20c9b);
            city.set_mac_state_misc_u32_at(MISC_DIFFICULTY_OFFSET, 1);
            city.set_mac_state_misc_u32_at(MISC_WEATHER_TREND_OFFSET, 1);
            city.set_mac_state_misc_u32_at(MISC_CITY_MODE_OFFSET, CITY_MODE_GAMEPLAY);

            let mut state = test_state(city);
            state.engine_fidelity = EngineFidelity::Mac68k;
            state.traffic.set_speed(GameSpeed::Paused);
            state.no_disasters_enabled = false;
            state.mac_process_rng = MacToolboxRng::new(0x62a4c2b8);
            state.mac_app_lfsr = MacAppLfsrRng::new(0x4067);
            // The live Point before proposal was (x=-1, y=41).
            state.mac_loop_scheduler.disaster_point = (-1, 41);

            // The native chance, selector, and X/Y calls returned low words
            // f0a0, b071, ccc5, and c0c6. CityGame draws them itself from the
            // captured entry seed and proposes the same selector-6 point.
            state.run_mac_disaster_chooser();
            assert_eq!(state.city.mac_pending_action(), 6, "{path}");
            assert_eq!(
                state.mac_loop_scheduler.pending_action_point,
                Some((6, 85)),
                "{path}"
            );
            assert_eq!(state.mac_loop_scheduler.disaster_point, (6, 85), "{path}");
            assert_eq!(state.mac_process_rng.seed(), 0x717ec0c6, "{path}");
            assert_eq!(state.mac_app_lfsr.state(), 0x4067, "{path}");

            // The native capture reached CODE 10 and cleared pending 6. Exercise
            // that dispatcher entry directly so the fixture does not advance a
            // separate frame/action pass beyond the captured event boundary.
            let mut now = 0;
            assert!(
                state.consume_mac_pending_action(&mut now, &mut |_| {}),
                "{path}"
            );
            assert_eq!(state.city.mac_pending_action(), 0, "{path}");
            assert_eq!(
                state.mac_loop_scheduler.pending_action_point, None,
                "{path}"
            );
            assert_eq!(state.mac_loop_scheduler.disaster_point, (6, 85), "{path}");
            // CODE 10's selector-6 branch begins the shake prelude here. The
            // worker writes the active-disaster field only after the prelude,
            // so the native pending-clear boundary still has no active type.
            assert_eq!(state.city.mac_state_misc_u32_at(0x1044), Some(0), "{path}");
            assert_eq!(state.mac_process_rng.seed(), 0x717ec0c6, "{path}");
            // A blank fixture cannot establish the later scan/action lifetime
            // or damage result; those require the captured live map state.
        }
    }

    #[test]
    fn captured_natural_air_crash_selector_rejects_without_runway() {
        for path in ["natural-air-crash.city", "natural-air-crash.sc2"] {
            let mut city = blank_city_layout_fixture(path, "Natural Air Crash");
            city.set_mac_state_misc_u32_at(MISC_CITY_AGE_OFFSET, 0x21885);
            city.set_mac_state_misc_u32_at(MISC_DIFFICULTY_OFFSET, 1);
            city.set_mac_state_misc_u32_at(MISC_WEATHER_TREND_OFFSET, 0);
            city.set_mac_state_misc_u32_at(MISC_CITY_MODE_OFFSET, CITY_MODE_GAMEPLAY);
            assert_eq!(city.mac_xbld_tile_count(0xdd), 0);

            let mut state = test_state(city);
            state.engine_fidelity = EngineFidelity::Mac68k;
            state.traffic.set_speed(GameSpeed::Paused);
            state.no_disasters_enabled = false;
            state.mac_process_rng = MacToolboxRng::new(0x02f4b54c);
            state.mac_app_lfsr = MacAppLfsrRng::new(0x2770);
            // The previous native Point was the natural Earthquake point.
            state.mac_loop_scheduler.disaster_point = (6, 85);

            state.run_mac_disaster_chooser();

            // Captured native returns are 0x9218 (chance) and 0x6db1
            // (selector 18). With XBLD[0xdd] == 0, native publishes no
            // pending Air Crash and makes no coordinate calls.
            assert_eq!(state.mac_process_rng.seed(), 0x075d6db1, "{path}");
            assert_eq!(state.city.mac_pending_action(), 0, "{path}");
            assert_eq!(
                state.mac_loop_scheduler.pending_action_point, None,
                "{path}"
            );
            assert_eq!(state.mac_loop_scheduler.disaster_point, (6, 85), "{path}");
            assert_eq!(state.mac_app_lfsr.state(), 0x2770, "{path}");
        }
    }

    #[test]
    fn ambient_walk_selector_publishes_without_replacing_point() {
        for path in ["ambient-walk-proposal.city", "ambient-walk-proposal.sc2"] {
            let mut city = blank_city_layout_fixture(path, "Ambient Walk Proposal");
            city.set_mac_state_misc_u32_at(MISC_CITY_AGE_OFFSET, 0x21885);
            city.set_mac_state_misc_u32_at(MISC_DIFFICULTY_OFFSET, 1);
            city.set_mac_state_misc_u32_at(
                MISC_WEATHER_TREND_OFFSET,
                u32::from(WEATHER_TREND_BLIZZARD),
            );
            city.set_mac_state_misc_u32_at(MISC_CITY_MODE_OFFSET, CITY_MODE_GAMEPLAY);
            city.prepare_mac_fidelity_from_saved_city();
            city.set_building(20, 20, 0xcd);
            assert_eq!(city.mac_xbld_tile_count(0xcd), 1);

            let mut state = test_state(city);
            state.engine_fidelity = EngineFidelity::Mac68k;
            state.traffic.set_speed(GameSpeed::Paused);
            state.no_disasters_enabled = false;
            state.mac_process_rng = MacToolboxRng::new(1884);
            state.mac_app_lfsr = MacAppLfsrRng::new(0x2770);
            state.mac_loop_scheduler.disaster_point = (13, 14);
            assert_eq!(state.city.mac_pending_action(), 0, "{path} before chooser");
            assert_eq!(
                state.city.mac_xbld_tile_count(0xcd),
                1,
                "{path} in ViewerState"
            );

            state.run_mac_disaster_chooser();

            // Native CODE 7 returns 10 from 0x35ce4 when XBLD[0xcd] != 0,
            // publishes it without making coordinate draws, and leaves Point
            // untouched. Seed 1884 supplies chance remainder 0, then slot 10.
            assert_eq!(state.city.mac_pending_action(), 10, "{path}");
            assert_eq!(state.mac_process_rng.seed(), 0x6899c693, "{path}");
            assert_eq!(
                state.mac_loop_scheduler.pending_action_point, None,
                "{path}"
            );
            assert_eq!(state.mac_loop_scheduler.disaster_point, (13, 14), "{path}");
            assert_eq!(state.mac_app_lfsr.state(), 0x2770, "{path}");
        }
    }

    #[test]
    fn ambient_scatter_selector_publishes_without_replacing_point() {
        for path in [
            "ambient-scatter-proposal.city",
            "ambient-scatter-proposal.sc2",
        ] {
            let mut city = blank_city_layout_fixture(path, "Ambient Scatter Proposal");
            city.set_mac_state_misc_u32_at(MISC_CITY_AGE_OFFSET, 0x21885);
            city.set_mac_state_misc_u32_at(MISC_DIFFICULTY_OFFSET, 1);
            city.set_mac_state_misc_u32_at(
                MISC_WEATHER_TREND_OFFSET,
                u32::from(WEATHER_TREND_BLIZZARD),
            );
            city.set_mac_state_misc_u32_at(MISC_CITY_MODE_OFFSET, CITY_MODE_GAMEPLAY);
            city.prepare_mac_fidelity_from_saved_city();
            city.set_building(20, 20, 0xcb);
            assert_eq!(city.mac_xbld_tile_count(0xcb), 1);

            let mut state = test_state(city);
            state.engine_fidelity = EngineFidelity::Mac68k;
            state.traffic.set_speed(GameSpeed::Paused);
            state.no_disasters_enabled = false;
            state.mac_process_rng = MacToolboxRng::new(992);
            state.mac_loop_scheduler.disaster_point = (13, 14);

            state.run_mac_disaster_chooser();

            // CODE 7 selector 9 publishes when XBLD[0xcb] is nonzero and
            // leaves Point alone; seed 992 selects slot 9 after chance zero.
            assert_eq!(state.city.mac_pending_action(), 9, "{path}");
            assert_eq!(state.mac_process_rng.seed(), 0x3e246662, "{path}");
            assert_eq!(
                state.mac_loop_scheduler.pending_action_point, None,
                "{path}"
            );
            assert_eq!(state.mac_loop_scheduler.disaster_point, (13, 14), "{path}");
        }
    }

    #[test]
    fn captured_natural_walk_selector_rejects_without_source() {
        for path in ["natural-walk-miss.city", "natural-walk-miss.sc2"] {
            let mut city = blank_city_layout_fixture(path, "Natural Walk Miss");
            city.set_mac_state_misc_u32_at(MISC_CITY_AGE_OFFSET, 0x21ec5);
            city.set_mac_state_misc_u32_at(MISC_DIFFICULTY_OFFSET, 1);
            city.set_mac_state_misc_u32_at(
                MISC_WEATHER_TREND_OFFSET,
                u32::from(WEATHER_TREND_BLIZZARD),
            );
            city.set_mac_state_misc_u32_at(MISC_CITY_MODE_OFFSET, CITY_MODE_GAMEPLAY);
            assert_eq!(city.mac_xbld_tile_count(0xcd), 0);

            let mut state = test_state(city);
            state.engine_fidelity = EngineFidelity::Mac68k;
            state.traffic.set_speed(GameSpeed::Paused);
            state.no_disasters_enabled = false;
            state.mac_process_rng = MacToolboxRng::new(0x7d9b43e4);
            state.mac_loop_scheduler.disaster_point = (6, 85);

            state.run_mac_disaster_chooser();

            // The live slot-10 draw was 0x338c (%19 == 10), with XBLD[0xcd]
            // zero. Native CODE 7 returns without publishing or changing Point.
            assert_eq!(state.mac_process_rng.seed(), 0x7daa338c, "{path}");
            assert_eq!(state.city.mac_pending_action(), 0, "{path}");
            assert_eq!(
                state.mac_loop_scheduler.pending_action_point, None,
                "{path}"
            );
            assert_eq!(state.mac_loop_scheduler.disaster_point, (6, 85), "{path}");
        }
    }

    #[test]
    fn captured_natural_scatter_selector_rejects_without_source() {
        for path in ["natural-scatter-miss.city", "natural-scatter-miss.sc2"] {
            let mut city = blank_city_layout_fixture(path, "Natural Scatter Miss");
            city.set_mac_state_misc_u32_at(MISC_CITY_AGE_OFFSET, 0x22bc2);
            city.set_mac_state_misc_u32_at(MISC_DIFFICULTY_OFFSET, 1);
            city.set_mac_state_misc_u32_at(
                MISC_WEATHER_TREND_OFFSET,
                u32::from(WEATHER_TREND_BLIZZARD),
            );
            city.set_mac_state_misc_u32_at(MISC_CITY_MODE_OFFSET, CITY_MODE_GAMEPLAY);
            assert_eq!(city.mac_xbld_tile_count(0xcb), 0);

            let mut state = test_state(city);
            state.engine_fidelity = EngineFidelity::Mac68k;
            state.traffic.set_speed(GameSpeed::Paused);
            state.no_disasters_enabled = false;
            state.mac_process_rng = MacToolboxRng::new(0x15bb9109);
            state.mac_loop_scheduler.disaster_point = (6, 85);

            state.run_mac_disaster_chooser();

            // The live selector return 0x6532 selects slot 9. With XBLD[0xcb]
            // zero, native CODE 7 returns without publishing or changing Point.
            assert_eq!(state.mac_process_rng.seed(), 0x44d16532, "{path}");
            assert_eq!(state.city.mac_pending_action(), 0, "{path}");
            assert_eq!(
                state.mac_loop_scheduler.pending_action_point, None,
                "{path}"
            );
            assert_eq!(state.mac_loop_scheduler.disaster_point, (6, 85), "{path}");
        }
    }

    #[test]
    fn captured_natural_riot_selector_rejects_below_weather_threshold() {
        for path in ["natural-riot-miss.city", "natural-riot-miss.sc2"] {
            let mut city = blank_city_layout_fixture(path, "Natural Riot Miss");
            city.set_mac_state_misc_u32_at(MISC_CITY_AGE_OFFSET, 0x221e5);
            city.set_mac_state_misc_u32_at(MISC_DIFFICULTY_OFFSET, 1);
            city.set_mac_state_misc_u32_at(
                MISC_WEATHER_TREND_OFFSET,
                u32::from(WEATHER_TREND_BLIZZARD),
            );
            city.set_mac_state_misc_u32_at(MISC_CITY_MODE_OFFSET, CITY_MODE_GAMEPLAY);
            city.set_mac_state_misc_u32_at(MAC_RIOT_UNEMPLOYMENT_STATE_OFFSET, 10);
            city.set_mac_state_misc_u32_at(MAC_RIOT_WEATHER_HEAT_STATE_OFFSET, 0x9e);

            let mut state = test_state(city);
            state.engine_fidelity = EngineFidelity::Mac68k;
            state.traffic.set_speed(GameSpeed::Paused);
            state.no_disasters_enabled = false;
            state.mac_process_rng = MacToolboxRng::new(0x0d7094b8);
            state.mac_loop_scheduler.disaster_point = (6, 85);

            state.run_mac_disaster_chooser();

            // The live selector return 0x4237 selects slot 3. Its saved heat
            // byte was 0x9e, below the native 0xaa gate, so no point or pending
            // selector is written after chance and selector draws.
            assert_eq!(state.mac_process_rng.seed(), 0x7c424237, "{path}");
            assert_eq!(state.city.mac_pending_action(), 0, "{path}");
            assert_eq!(
                state.mac_loop_scheduler.pending_action_point, None,
                "{path}"
            );
            assert_eq!(state.mac_loop_scheduler.disaster_point, (6, 85), "{path}");
        }
    }

    #[test]
    fn captured_natural_flood_selector_rejects_below_weather_threshold() {
        for path in ["natural-flood-miss.city", "natural-flood-miss.sc2"] {
            let mut city = blank_city_layout_fixture(path, "Natural Flood Miss");
            city.set_mac_state_misc_u32_at(MISC_CITY_AGE_OFFSET, 0x22d9d);
            city.set_mac_state_misc_u32_at(MISC_DIFFICULTY_OFFSET, 1);
            city.set_mac_state_misc_u32_at(MISC_WEATHER_TREND_OFFSET, 8);
            city.set_mac_state_misc_u32_at(MISC_CITY_MODE_OFFSET, CITY_MODE_GAMEPLAY);

            let mut state = test_state(city);
            state.engine_fidelity = EngineFidelity::Mac68k;
            state.traffic.set_speed(GameSpeed::Paused);
            state.no_disasters_enabled = false;
            state.mac_process_rng = MacToolboxRng::new(0x0619be91);
            state.mac_loop_scheduler.disaster_point = (6, 85);

            state.run_mac_disaster_chooser();

            // The live selector return 0xae2c selects slot 14. Trend 8 is one
            // below the Flood wrapper's native trend-9 threshold.
            assert_eq!(state.mac_process_rng.seed(), 0x716cae2c, "{path}");
            assert_eq!(state.city.mac_pending_action(), 0, "{path}");
            assert_eq!(
                state.mac_loop_scheduler.pending_action_point, None,
                "{path}"
            );
            assert_eq!(state.mac_loop_scheduler.disaster_point, (6, 85), "{path}");
        }
    }

    #[test]
    fn captured_natural_selector_8_rejects_below_growth_threshold() {
        for path in ["natural-growth-miss.city", "natural-growth-miss.sc2"] {
            let mut city = blank_city_layout_fixture(path, "Natural Growth Gate");
            city.set_mac_state_misc_u32_at(MISC_CITY_AGE_OFFSET, 0x704d);
            city.set_mac_state_misc_u32_at(MISC_DIFFICULTY_OFFSET, 1);
            city.set_mac_state_misc_u32_at(MISC_WEATHER_TREND_OFFSET, 8);
            city.set_mac_state_misc_u32_at(MISC_CITY_MODE_OFFSET, CITY_MODE_GAMEPLAY);
            city.set_mac_state_misc_u32_at(MAC_MASS_RIOT_GROWTH_STATE_OFFSET, 0x1446);
            let row2 =
                budget_row_misc_offset_for_city(&city, 2, BUDGET_SERVICE_CURRENT_FIELD).unwrap();
            city.set_budget_misc_u32_values(row2, &[0x096a]);

            let mut state = test_state(city);
            state.engine_fidelity = EngineFidelity::Mac68k;
            state.traffic.set_speed(GameSpeed::Paused);
            state.no_disasters_enabled = false;
            state.mac_process_rng = MacToolboxRng::new(0x57282f24);
            state.mac_app_lfsr = MacAppLfsrRng::new(0x0da1);
            state.mac_loop_scheduler.disaster_point = (0, 0);

            state.run_mac_disaster_chooser();

            // Native CODE 7 at age 0x704d returned chance seed 0x0f2f1130,
            // then selector seed 0x5b116e19. Its low word selects slot 8;
            // live growth 0x1446 (5,190) is below the 45,000 gate, so there
            // are no coordinate draws or pending publication.
            assert_eq!(state.mac_process_rng.seed(), 0x5b116e19, "{path}");
            assert_eq!(state.city.mac_pending_action(), 0, "{path}");
            assert_eq!(
                state.mac_loop_scheduler.pending_action_point, None,
                "{path}"
            );
            assert_eq!(state.mac_loop_scheduler.disaster_point, (0, 0), "{path}");
            assert_eq!(state.mac_app_lfsr.state(), 0x0da1, "{path}");
        }
    }

    #[test]
    fn natural_2047_selector_8_rejects_with_live_growth_and_budget_state() {
        for path in [
            "natural-2047-growth-miss.city",
            "natural-2047-growth-miss.sc2",
        ] {
            let mut city = blank_city_layout_fixture(path, "Natural 2047 Growth Gate");
            city.set_mac_state_misc_u32_at(MISC_CITY_AGE_OFFSET, 0xae82);
            city.set_mac_state_misc_u32_at(MISC_DIFFICULTY_OFFSET, 1);
            city.set_mac_state_misc_u32_at(MISC_WEATHER_TREND_OFFSET, 8);
            city.set_mac_state_misc_u32_at(MISC_CITY_MODE_OFFSET, CITY_MODE_GAMEPLAY);
            city.set_mac_state_misc_u32_at(MAC_MASS_RIOT_GROWTH_STATE_OFFSET, 0x0140);
            let row2 =
                budget_row_misc_offset_for_city(&city, 2, BUDGET_SERVICE_CURRENT_FIELD).unwrap();
            city.set_budget_misc_u32_values(row2, &[0x0078]);

            let mut state = test_state(city);
            state.engine_fidelity = EngineFidelity::Mac68k;
            state.traffic.set_speed(GameSpeed::Paused);
            state.no_disasters_enabled = false;
            state.mac_process_rng = MacToolboxRng::new(0x1a57_9a4a);
            state.mac_app_lfsr = MacAppLfsrRng::new(0x2330);
            state.mac_loop_scheduler.disaster_point = (0, 0);

            state.run_mac_disaster_chooser();

            // At age 0xae82 the chance return 0x7dc8 passes `% 100`, then
            // selector return 0x0d8a chooses slot 8. Growth 320 is below its
            // 45,000 gate; budget row 2 is 120 and remains untouched.
            assert_eq!(state.mac_process_rng.seed(), 0x42c0_0d8a, "{path}");
            assert_eq!(state.city.mac_pending_action(), 0, "{path}");
            assert_eq!(
                state.mac_loop_scheduler.pending_action_point, None,
                "{path}"
            );
            assert_eq!(state.mac_loop_scheduler.disaster_point, (0, 0), "{path}");
            assert_eq!(budget_current(&state.city, 2), 120, "{path}");
            assert_eq!(state.mac_app_lfsr.state(), 0x2330, "{path}");
        }
    }

    #[test]
    fn captured_natural_selector_13_rejects_below_growth_threshold() {
        for path in [
            "natural-riot-growth-miss.city",
            "natural-riot-growth-miss.sc2",
        ] {
            let mut city = blank_city_layout_fixture(path, "Natural Riot Growth Gate");
            city.set_mac_state_misc_u32_at(MISC_CITY_AGE_OFFSET, 0x98ed);
            city.set_mac_state_misc_u32_at(MISC_DIFFICULTY_OFFSET, 1);
            city.set_mac_state_misc_u32_at(MISC_WEATHER_TREND_OFFSET, 0);
            city.set_mac_state_misc_u32_at(MISC_CITY_MODE_OFFSET, CITY_MODE_GAMEPLAY);
            city.set_mac_state_misc_u32_at(MAC_MASS_RIOT_GROWTH_STATE_OFFSET, 0x131a);
            let row2 =
                budget_row_misc_offset_for_city(&city, 2, BUDGET_SERVICE_CURRENT_FIELD).unwrap();
            city.set_budget_misc_u32_values(row2, &[0x079e]);

            let mut state = test_state(city);
            state.engine_fidelity = EngineFidelity::Mac68k;
            state.traffic.set_speed(GameSpeed::Paused);
            state.no_disasters_enabled = false;
            state.mac_process_rng = MacToolboxRng::new(0x223c1c2f);
            state.mac_app_lfsr = MacAppLfsrRng::new(0x995f);
            state.mac_loop_scheduler.disaster_point = (0, 0);

            state.run_mac_disaster_chooser();

            // The 2026 natural capture drew chance state 0x185e6338, then
            // selector state 0x5cc3fe07 (slot 13). Decision-time growth was
            // 0x131a (4,890), below selector 13's 30,000 gate, so no actor
            // coordinate draw, Point update, or pending publication followed.
            assert_eq!(state.mac_process_rng.seed(), 0x5cc3fe07, "{path}");
            assert_eq!(state.city.mac_pending_action(), 0, "{path}");
            assert_eq!(
                state.mac_loop_scheduler.pending_action_point, None,
                "{path}"
            );
            assert_eq!(state.mac_loop_scheduler.disaster_point, (0, 0), "{path}");
            assert_eq!(state.mac_app_lfsr.state(), 0x995f, "{path}");
        }
    }

    #[test]
    fn captured_natural_selector_15_rejects_below_budget_threshold() {
        for path in ["natural-budget-miss.city", "natural-budget-miss.sc2"] {
            let mut city = blank_city_layout_fixture(path, "Natural Budget Gate");
            city.set_mac_state_misc_u32_at(MISC_CITY_AGE_OFFSET, 0x720f);
            city.set_mac_state_misc_u32_at(MISC_DIFFICULTY_OFFSET, 1);
            city.set_mac_state_misc_u32_at(MISC_WEATHER_TREND_OFFSET, 7);
            city.set_mac_state_misc_u32_at(MISC_CITY_MODE_OFFSET, CITY_MODE_GAMEPLAY);
            city.set_mac_state_misc_u32_at(MAC_MASS_RIOT_GROWTH_STATE_OFFSET, 0x155e);
            let row2 =
                budget_row_misc_offset_for_city(&city, 2, BUDGET_SERVICE_CURRENT_FIELD).unwrap();
            city.set_budget_misc_u32_values(row2, &[0x0a3c]);

            let mut state = test_state(city);
            state.engine_fidelity = EngineFidelity::Mac68k;
            state.traffic.set_speed(GameSpeed::Paused);
            state.no_disasters_enabled = false;
            state.mac_process_rng = MacToolboxRng::new(0x440e7e44);
            state.mac_app_lfsr = MacAppLfsrRng::new(0x61eb);
            state.mac_loop_scheduler.disaster_point = (0, 0);

            state.run_mac_disaster_chooser();

            // Native CODE 7 at age 0x720f returned chance state 0x1383c544,
            // then selector state 0x300bfd5e; the selector low word selects
            // slot 15. Live row-2 current was 0x0a3c (2,620), below 10,000,
            // so the original made no jitter calls or pending publication.
            assert_eq!(state.mac_process_rng.seed(), 0x300bfd5e, "{path}");
            assert_eq!(budget_current(&state.city, 2), 0x0a3c, "{path}");
            assert_eq!(state.city.mac_pending_action(), 0, "{path}");
            assert_eq!(
                state.mac_loop_scheduler.pending_action_point, None,
                "{path}"
            );
            assert_eq!(state.mac_loop_scheduler.disaster_point, (0, 0), "{path}");
            assert_eq!(state.mac_app_lfsr.state(), 0x61eb, "{path}");
        }
    }

    #[test]
    fn captured_natural_selector_2_rejects_without_coast_or_river() {
        for path in [
            "natural-water-option-miss.city",
            "natural-water-option-miss.sc2",
        ] {
            let mut city = blank_city_layout_fixture(path, "Natural Water Option Gate");
            city.set_mac_state_misc_u32_at(MISC_CITY_AGE_OFFSET, 0x7179);
            city.set_mac_state_misc_u32_at(MISC_DIFFICULTY_OFFSET, 1);
            city.set_mac_state_misc_u32_at(MISC_WEATHER_TREND_OFFSET, 7);
            city.set_mac_state_misc_u32_at(MISC_CITY_MODE_OFFSET, CITY_MODE_GAMEPLAY);
            city.set_mac_state_misc_u32_at(EDIT_MAP_COAST_MISC_OFFSET, 0);
            city.set_mac_state_misc_u32_at(EDIT_MAP_RIVER_MISC_OFFSET, 0);

            let mut state = test_state(city);
            state.engine_fidelity = EngineFidelity::Mac68k;
            state.traffic.set_speed(GameSpeed::Paused);
            state.no_disasters_enabled = false;
            state.mac_process_rng = MacToolboxRng::new(0x6dd29070);
            state.mac_app_lfsr = MacAppLfsrRng::new(0xf2e0);
            state.mac_loop_scheduler.disaster_point = (0, 0);

            state.run_mac_disaster_chooser();

            // The live chance result was 0x1b08e164, followed by selector
            // 0x64057a19 (low word %19 == 2). Trend 7 clears the weather
            // threshold, but Maxis Land has neither saved water option; no
            // coordinate calls or action publication follow.
            assert_eq!(state.mac_process_rng.seed(), 0x64057a19, "{path}");
            assert_eq!(state.city.mac_pending_action(), 0, "{path}");
            assert_eq!(
                state.mac_loop_scheduler.pending_action_point, None,
                "{path}"
            );
            assert_eq!(state.mac_loop_scheduler.disaster_point, (0, 0), "{path}");
            assert_eq!(state.mac_app_lfsr.state(), 0xf2e0, "{path}");
        }
    }

    fn status_fixture(
        path: &str,
        date: u32,
        commercial: u32,
        residential: u32,
        arcology_share: u32,
        runway: u16,
        cross: u16,
        network: u16,
    ) -> City {
        let mut city = blank_city_layout_fixture(path, "Airport status");
        for (tile, count) in [
            (0xd2, 9),
            (0xd3, 9),
            (0xd1, 9),
            (0xd6, 9),
            (0xdd, runway),
            (0xde, cross),
        ] {
            city.set_mac_state_misc_u32_at(0x01f0 + tile * 4, u32::from(count));
        }
        city.prepare_mac_fidelity_from_saved_city();
        city.set_mac_power_service_summary(0);
        city.set_mac_water_service_summary(0);
        city.set_mac_arcology_resident_total(arcology_share * 12);
        city.set_mac_state_misc_u32_at(0x1050, u32::from(network));
        city.set_mac_state_misc_u32_at(crate::city::Technology::Airport.misc_offset(), date);
        for (row, amount) in [(0, residential), (1, commercial), (2, 0), (10, 81)] {
            let offset =
                budget_row_misc_offset_for_city(&city, row, BUDGET_SERVICE_CURRENT_FIELD).unwrap();
            city.set_budget_misc_u32_values(offset, &[amount]);
        }
        city
    }

    #[test]
    fn airport_story_precedes_recreation_and_uses_native_technology_word() {
        for path in ["airport-status.city", "airport-status.sc2"] {
            for (date, expected) in [(0, 54), (1999, 60), (0x10000, 54)] {
                let city = status_fixture(path, date, 2000, 1000, 0, 0, 0, 0);
                let mut rng = MacToolboxRng::new(2);
                assert_eq!(
                    status_story_selector(&city, 0, 8000, &mut rng),
                    Some(expected)
                );
                assert_eq!(rng.seed(), 2);
            }
        }
    }

    #[test]
    fn water_status_uses_strict_unsigned_threshold() {
        for path in ["water-status.city", "water-status.sc2"] {
            for (water, expected) in [(98, None), (99, Some(50)), (u32::MAX, Some(50))] {
                let mut city = status_fixture(path, 0, 0, 0, 0, 0, 0, 0);
                city.set_mac_water_service_summary(water);
                let mut rng = MacToolboxRng::new(2);
                assert_eq!(status_story_selector(&city, 0, 1000, &mut rng), expected);
                assert_eq!(rng.seed(), 2);
            }
        }
    }

    #[test]
    fn status_window_tracks_chooser_and_clears_stale_messages() {
        for path in ["status-window.city", "status-window.sc2"] {
            let city = status_fixture(path, 0, 2000, 0, 0, 0, 0, 0);
            let mut state = test_state(city);
            state.engine_fidelity = EngineFidelity::Mac68k;
            state.traffic.set_speed(GameSpeed::Turtle);
            state.no_disasters_enabled = true;
            state
                .city
                .set_mac_state_misc_u32_at(MAC_MASS_RIOT_GROWTH_STATE_OFFSET, 8000);
            state
                .city
                .set_mac_state_misc_u32_at(MISC_WEATHER_TREND_OFFSET, 0);
            state.run_mac_disaster_chooser();
            assert_eq!(
                state.mac_loop_scheduler.status_message,
                Some(MacStatusMessage::Story(54))
            );
            assert_eq!(
                state.status_panel_notice(),
                super::super::super::newspaper_stories::pe_health_amenities_newspaper_label(54)
                    .unwrap()
            );

            state
                .city
                .set_mac_state_misc_u32_at(MISC_WEATHER_TREND_OFFSET, 9);
            state.run_mac_disaster_chooser();
            assert_eq!(
                state.mac_loop_scheduler.status_message,
                Some(MacStatusMessage::Weather(9))
            );
            assert_eq!(state.status_panel_notice(), "Blizzard conditions");

            state
                .city
                .set_mac_state_misc_u32_at(MISC_WEATHER_TREND_OFFSET, 0);
            state
                .city
                .set_mac_state_misc_u32_at(MAC_MASS_RIOT_GROWTH_STATE_OFFSET, 999);
            state.run_mac_disaster_chooser();
            assert_eq!(state.mac_loop_scheduler.status_message, None);
            assert_eq!(state.status_panel_notice(), "");
        }
    }

    #[test]
    #[ignore = "requires private native Mac airport status fixtures"]
    fn airport_status_matches_original_worker() {
        let path = std::env::var_os("CITYGAME_MAC_AIRPORT_STATUS").unwrap();
        let cases: serde_json::Value =
            serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        for row in cases.as_array().unwrap() {
            let value = |key: &str| row[key].as_u64().unwrap() as u32;
            for path in ["airport-status.city", "airport-status.sc2"] {
                let city = status_fixture(
                    path,
                    value("date"),
                    value("commercial"),
                    value("residential"),
                    value("arcology_share"),
                    value("runway") as u16,
                    value("cross") as u16,
                    value("network") as u16,
                );
                let mut rng = MacToolboxRng::new(2);
                let expected = row["selector"].as_u64().map(|value| value as u16);
                assert_eq!(
                    status_story_selector(&city, 0, 8000, &mut rng),
                    expected,
                    "{row}"
                );
                assert_eq!(
                    rng.seed(),
                    if matches!(expected, Some(55..=58)) {
                        33614
                    } else {
                        2
                    }
                );
            }
        }
    }

    #[test]
    #[ignore = "requires private native Mac status chooser fixtures"]
    fn status_chooser_matches_original_decision_tree() {
        let path = std::env::var_os("CITYGAME_MAC_STATUS_CHOOSER").unwrap();
        let cases: serde_json::Value =
            serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        for row in cases.as_array().unwrap() {
            let value = |key: &str| row[key].as_u64().unwrap() as u32;
            for path in ["status-chooser.city", "status-chooser.sc2"] {
                let mut city = blank_city_layout_fixture(path, "Status chooser");
                for tile in [
                    0xd1, 0xd2, 0xd3, 0xd5, 0xd6, 0xd7, 0xd8, 0xda, 0xdd, 0xde, 0xe0, 0xe9, 0xed,
                    0xf8,
                ] {
                    let count = row["counts"][tile.to_string()].as_u64().unwrap_or(0);
                    city.set_mac_state_misc_u32_at(0x01f0 + tile * 4, count as u32);
                }
                city.prepare_mac_fidelity_from_saved_city();
                city.set_mac_power_service_summary(value("power"));
                city.set_mac_water_service_summary(value("water"));
                city.set_mac_arcology_resident_total(value("arcology"));
                for (offset, key) in [
                    (0x1050, "network"),
                    (0x1024, "non_network"),
                    (EDIT_MAP_COAST_MISC_OFFSET, "coast"),
                    (EDIT_MAP_RIVER_MISC_OFFSET, "river"),
                    (crate::city::Technology::Airport.misc_offset(), "date"),
                ] {
                    city.set_mac_state_misc_u32_at(offset, value(key));
                }
                for (row_index, key) in [
                    (0, "residential"),
                    (1, "commercial"),
                    (2, "industrial"),
                    (10, "transit"),
                ] {
                    let offset = budget_row_misc_offset_for_city(
                        &city,
                        row_index,
                        BUDGET_SERVICE_CURRENT_FIELD,
                    )
                    .unwrap();
                    city.set_budget_misc_u32_values(offset, &[value(key)]);
                }
                let mut rng = MacToolboxRng::new(value("seed"));
                let actual =
                    status_story_selector(&city, value("trend") as u8, value("growth"), &mut rng);
                assert_eq!(
                    actual,
                    row["selector"].as_u64().map(|value| value as u16),
                    "{path} {row}"
                );
                assert_eq!(rng.seed(), value("seed_after"), "{path} {row}");
                if let Some(native_message) = row.get("message_index") {
                    let mut state = test_state(city);
                    state.engine_fidelity = EngineFidelity::Mac68k;
                    state.no_disasters_enabled = true;
                    state.mac_process_rng = MacToolboxRng::new(value("seed"));
                    state
                        .city
                        .set_mac_state_misc_u32_at(MISC_WEATHER_TREND_OFFSET, value("trend"));
                    state.city.set_mac_state_misc_u32_at(
                        MAC_MASS_RIOT_GROWTH_STATE_OFFSET,
                        value("growth"),
                    );
                    state.run_mac_disaster_chooser();
                    let actual = match state.mac_loop_scheduler.status_message {
                        Some(MacStatusMessage::Story(selector)) => Some(u64::from(selector - 46)),
                        Some(MacStatusMessage::Weather(trend)) => Some(u64::from(trend + 7)),
                        None => None,
                    };
                    assert_eq!(actual, native_message.as_u64(), "{path} {row}");
                    assert_eq!(
                        state.mac_process_rng.seed(),
                        value("seed_after"),
                        "{path} {row}"
                    );
                }
            }
        }
    }
}
