//! Disaster progression, actor-tick adapters and cleared-state publication.
//!
//! Idle/calendar schedulers retain their existing cadence and admission gates.
//! Native damage and actor workers retain ownership of simulation; these adapters
//! preserve random-stream handoff, sound/auto-goto order and redraw boundaries.
//! The historical monthly fallback keeps its existing test-only gate.

use super::{
    AppMode, CITY_HEIGHT, CITY_WIDTH, City, DISASTER_SPREAD_MAX_TILES_PER_MONTH, DisasterKind,
    EngineFidelity, MISC_CURRENT_DISASTER_TYPE_OFFSET, MISC_DISASTER_ACTIVE_OFFSET,
    POLICE_STATION_BUILDING_ID, PlacementSound, ViewerState, XTHG_FIRST_USABLE_SLOT,
    disaster_kind_misc_type, disaster_spread_offsets, ensure_budget_misc_defaults_before_extension,
    is_disaster_text_overlay, is_water_terrain, next_disaster_marker, offset_city_tile,
    pe_aircrash_tick_city_with_lfsr, pe_destroy_building_footprint,
    pe_disaster_overlay_tick_city_with_rng, pe_disaster_xthg_tick_slots_in_order,
    pe_earthquake_shake_delta, pe_explosion_tick_city_with_lfsr, pe_fb_overlay_scan_due,
    pe_fire_overlay_scan_due, pe_flood_destroy_tile_with_crt, pe_flood_overlay_scan_due,
    pe_hurricane_wind_tick_with_rng, pe_misc_mirror_storage_active, pe_monster_tick_city_with_rng,
    pe_riot_overlay_scan_due, pe_tornado_tick_city_with_rng, persist_pe_city_mode_misc,
    xthg_proxy_for_slot,
};
#[cfg(test)]
use super::{DISASTER_DAMAGE_MAX_TILES_PER_MONTH, active_disaster_tiles_for_city};
use crate::win95_random::Win95CrtRand;
use glam::Vec2;
use std::collections::HashSet;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) struct PeDisasterOverlaySeen {
    pub(super) fire: bool,
    pub(super) flood: bool,
    pub(super) riot: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) struct PeDisasterScanSounds {
    pub(super) play_riot: bool,
    pub(super) play_flood: bool,
    pub(super) play_fire: bool,
}

fn pe_disaster_scan_sound_seed(city: &City) -> u32 {
    // Offline/deterministic helper seed only. Live Win95 scan @ `0x4605a9`
    // draws CRT `rand()` (`0x48cdd8`) from the process stream for 1-in-8 SFX gates.
    let mut seed = 0x4605_A900u32;
    for byte in city.name.bytes() {
        seed = seed.wrapping_mul(33).wrapping_add(u32::from(byte));
    }
    seed
}

pub(super) fn pe_disaster_overlay_seen(city: &City) -> PeDisasterOverlaySeen {
    let mut seen = PeDisasterOverlaySeen::default();
    for y in 0..CITY_HEIGHT {
        for x in 0..CITY_WIDTH {
            match city.text_overlay_at(x, y) {
                Some(0xff) => seen.fire = true,
                Some(0xfc) => seen.flood = true,
                Some(0xfd | 0xfe) => seen.riot = true,
                _ => {}
            }
            if seen.fire && seen.flood && seen.riot {
                return seen;
            }
        }
    }
    seen
}

fn pe_disaster_scan_sounds(city: &City, seen: PeDisasterOverlaySeen) -> PeDisasterScanSounds {
    let mut crt = Win95CrtRand::new(pe_disaster_scan_sound_seed(city));
    pe_disaster_scan_sounds_with_rng(city, seen, &mut crt)
}

pub(super) fn pe_disaster_scan_sounds_with_rng(
    _city: &City,
    seen: PeDisasterOverlaySeen,
    crt: &mut Win95CrtRand,
) -> PeDisasterScanSounds {
    PeDisasterScanSounds {
        // Win95 `0x4605a9`: riot/flood are 1-in-8 SFX; fire-seen always calls `0x401e7e(0x1fb)`.
        play_riot: seen.riot && crt.random_u16() & 7 == 0,
        play_flood: seen.flood && crt.random_u16() & 7 == 0,
        play_fire: seen.fire,
    }
}

/// Win95 `dwDisasterActive` / `wCurrentDisasterType` / `wMonsterXTHGIndex`
/// → MISC `0x1048` / `0x1044` / `0x1040`, plus `wCityMode` → MISC `0x0004`.
pub(super) fn persist_pe_disaster_misc(
    city: &mut City,
    started: Option<DisasterKind>,
    mode: AppMode,
    fidelity: EngineFidelity,
) {
    let pe_mirrors_active = pe_misc_mirror_storage_active(city, fidelity);
    if pe_mirrors_active {
        ensure_budget_misc_defaults_before_extension(city);
    }
    let active = city.has_active_disaster();
    city.set_pe_dw_disaster_active(u32::from(active));
    if pe_mirrors_active {
        city.set_misc_u32_at(MISC_DISASTER_ACTIVE_OFFSET, u32::from(active));
    }
    if let Some(kind) = started {
        let disaster_type = disaster_kind_misc_type(kind) as u16;
        city.set_pe_w_current_disaster_type(disaster_type);
        if pe_mirrors_active {
            city.set_misc_u32_at(MISC_CURRENT_DISASTER_TYPE_OFFSET, u32::from(disaster_type));
        }
    } else if !active {
        // PE `0x45cf10` / OpenSC2K DisasterEnd: summary story + recall dispatch before
        // clearing `wCurrentDisasterType`.
        pe_disaster_end_45cf10(city);
        city.set_pe_w_current_disaster_type(0);
        if pe_mirrors_active {
            city.set_misc_u32_at(MISC_CURRENT_DISASTER_TYPE_OFFSET, 0);
        }
        city.clear_pe_aircrash_actors();
        city.clear_pe_monster_actors();
        city.clear_pe_tornado_actors();
        clear_pe_maxis_man_actors(city);
    }
    persist_pe_city_mode_misc(city, mode, fidelity);
}

/// PE disaster-end UI arm @ `0x45cf10` (OpenSC2K `DisasterEnd.finish`): when the last
/// marker/actor is gone, queue the type's summary newspaper story and clear police/
/// fire/military dispatch XTHG proxies (kind only), then leave type clear to the caller.
fn pe_disaster_end_45cf10(city: &mut City) {
    let disaster_type = city.pe_w_current_disaster_type();
    if disaster_type == 0 {
        return;
    }
    if let Some(story_id) = pe_disaster_end_summary_story_id(disaster_type) {
        // PE / OpenSC2K DisasterEnd: story param is the highest-weight damage class.
        let param = city.take_pe_disaster_damage_class_param();
        city.queue_pe_newspaper_story(story_id, param);
    } else {
        city.clear_pe_disaster_damage_class();
    }
    let _ = city.pe_maxis_daily_clear_dispatch_xtxt_proxies();
    // OpenSC2K DisasterEnd: newspaper_requested + NEWSPAPER_PAPER = 0.
    city.set_pe_disaster_end_newspaper_pending();
}

/// PE `0x45d179` switch / OpenSC2K `DisasterEnd.STORY_TYPES`.
fn pe_disaster_end_summary_story_id(disaster_type: u16) -> Option<u16> {
    Some(match disaster_type {
        0x01 | 0x0c => 0x16, // fire / firestorm
        0x02 | 0x0e => 0x17, // flood / blizzard flood
        0x03 | 0x0d => 0x23, // rioters / mass riot
        0x04 => 0x20,        // toxic spill
        0x05 | 0x12 => 0x18, // nop air-crash / plane crash
        0x06 => 0x1b,        // earthquake
        0x07 => 0x1a,        // tornado
        0x08 => 0x1c,        // monster
        0x09 => 0x1d,        // meltdown
        0x0a => 0x1e,        // microwave
        0x0b => 0x1f,        // volcano
        0x0f => 0x21,        // pollution accident
        0x10 => 0x22,        // hurricane
        0x11 => 0x19,        // helicopter crash
        _ => return None,
    })
}

pub(super) fn clear_pe_maxis_man_actors(city: &mut City) -> usize {
    let actors: Vec<(usize, usize, usize)> = city
        .traffic_overlays()
        .map(|table| {
            table
                .iter()
                .enumerate()
                .skip(XTHG_FIRST_USABLE_SLOT)
                .filter_map(|(slot, record)| {
                    (record.kind == crate::win95_overlay::XTHG_MAXIS_MAN).then_some((
                        slot,
                        record.tile_x as usize,
                        record.tile_y as usize,
                    ))
                })
                .collect()
        })
        .unwrap_or_default();
    let mut cleared = 0;
    for (slot, x, y) in actors {
        if let Some(proxy) = xthg_proxy_for_slot(slot)
            && city.text_overlay_at(x, y) == Some(proxy)
        {
            city.set_text_overlay(x, y, 0);
        }
        if let Some(overlays) = city.traffic_overlays_mut()
            && let Some(record) = overlays.get_mut(slot)
        {
            // PE `FUN_004507D0`: clear kind only; height/saved_b remain.
            record.kind = 0;
        }
        city.mark_render_dirty(x, y);
        cleared += 1;
    }
    cleared
}

impl ViewerState {
    #[cfg(test)]
    pub(super) fn advance_active_disasters(&mut self) -> usize {
        // Historical monthly fallback retained only for archaeology assertions.
        // Native marker scans belong to pending dispatch, never calendar day 0.
        let pe_flood = pe_flood_overlay_scan_due(&self.city);
        let pe_fire = pe_fire_overlay_scan_due(&self.city);
        let pe_riot = pe_riot_overlay_scan_due(&self.city);
        let pe_fb = pe_fb_overlay_scan_due(&self.city);
        let active_tiles: Vec<_> = active_disaster_tiles_for_city(&self.city)
            .into_iter()
            .filter(|&(_, _, marker)| {
                !(pe_flood && marker == 0xfc)
                    && !(pe_fire && marker == 0xff)
                    && !(pe_riot && matches!(marker, 0xfd | 0xfe))
                    && !(pe_fb && marker == 0xfb)
            })
            .collect();
        let mut damaged = 0;
        for &(x, y, _) in &active_tiles {
            if damaged >= DISASTER_DAMAGE_MAX_TILES_PER_MONTH {
                break;
            }
            if self.damage_disaster_tile(x, y) {
                damaged += 1;
            }
        }
        let spread = self.spread_active_disasters(&active_tiles);
        let cleared = if damaged == 0 && spread == 0 {
            self.clear_spent_disaster_markers(&active_tiles)
        } else {
            0
        };
        // Air-crash / tornado / monster finish via PE XTHG ticks, not monthly clear.
        let changed = damaged + spread + cleared;
        if cleared > 0 {
            self.persist_cleared_disaster_misc();
        }
        if changed > 0 {
            self.city.mark_all_dirty();
            self.set_sim_status_announcement(if damaged > 0 {
                "Disaster Damage Reported"
            } else if spread > 0 {
                "Disaster Spreading"
            } else {
                "Disaster Contained"
            });
        }
        changed
    }

    pub(super) fn damage_disaster_tile(&mut self, x: usize, y: usize) -> bool {
        let building = self.city.building_at(x, y);
        if building == 0 {
            return false;
        }

        // PE DestroyStructure (`0x401604` → `0x461330`); disaster then re-stamps active markers.
        let footprint = pe_destroy_building_footprint(&self.city, x, y);
        let saved: Vec<(usize, usize, u8, u8)> = footprint
            .iter()
            .map(|&(tx, ty)| {
                (
                    tx,
                    ty,
                    self.city.text_overlay_at(tx, ty).unwrap_or(0),
                    self.city.zone_at(tx, ty).map_or(0, |zone| zone.zone_id()),
                )
            })
            .collect();
        pe_flood_destroy_tile_with_crt(&mut self.city, &mut self.pe_process_crt, x, y);
        for (tx, ty, prior_overlay, zone_id) in saved {
            self.city.set_zone(tx, ty, zone_id);
            // Disaster monthly damage re-stamps the active marker (meltdown pattern @ `0x45f2b0`).
            if is_disaster_text_overlay(prior_overlay) {
                self.city.set_text_overlay(tx, ty, prior_overlay);
            }
            // PE DestroyStructure only tests XBIT water bit `0x04`; it does not clear watered/piped.
        }
        true
    }

    fn spread_active_disasters(&mut self, active_tiles: &[(usize, usize, u8)]) -> usize {
        let mut claimed: HashSet<(usize, usize)> =
            active_tiles.iter().map(|&(x, y, _)| (x, y)).collect();
        let mut spread_targets = Vec::new();
        'active: for &(x, y, marker) in active_tiles {
            for (dx, dy) in disaster_spread_offsets(marker) {
                let Some((tx, ty)) = offset_city_tile(x, y, dx, dy) else {
                    continue;
                };
                if claimed.contains(&(tx, ty)) || !self.can_disaster_spread_to_tile(tx, ty) {
                    continue;
                }
                claimed.insert((tx, ty));
                spread_targets.push((tx, ty, next_disaster_marker(marker)));
                if spread_targets.len() >= DISASTER_SPREAD_MAX_TILES_PER_MONTH {
                    break 'active;
                }
            }
        }

        for (x, y, marker) in spread_targets.iter().copied() {
            self.city.set_text_overlay(x, y, marker);
        }
        spread_targets.len()
    }

    fn can_disaster_spread_to_tile(&self, x: usize, y: usize) -> bool {
        if self.city.dispatch_actor_at(x, y).is_some()
            || is_water_terrain(self.city.terrain_at(x, y))
            || self.city.text_overlay_at(x, y).unwrap_or(0) != 0
        {
            return false;
        }
        self.city.building_at(x, y) != 0
    }

    fn clear_spent_disaster_markers(&mut self, active_tiles: &[(usize, usize, u8)]) -> usize {
        for &(x, y, _) in active_tiles {
            self.city.set_text_overlay(x, y, 0);
        }
        active_tiles.len()
    }

    pub(super) fn earthquake_shake_offset(&self) -> Vec2 {
        pe_earthquake_shake_delta(self.earthquake_shake_remaining, self.zoom)
    }

    pub(super) fn tick_earthquake_shake(&mut self) {
        self.earthquake_shake_remaining = self.earthquake_shake_remaining.saturating_sub(1);
    }

    pub(super) fn tick_pe_disaster_duration(&mut self) {
        self.pe_disaster_duration = self.pe_disaster_duration.saturating_sub(1);
    }

    fn tick_pe_disaster_duration_days(&mut self, days: u16) {
        for _ in 0..days {
            self.tick_pe_disaster_duration();
        }
    }

    fn tick_pe_hurricane_wind(&mut self) -> usize {
        let tick = pe_hurricane_wind_tick_with_rng(
            &mut self.city,
            self.pe_disaster_duration,
            self.pe_hurricane_wind_remaining,
            &mut self.pe_process_crt,
            &mut self.pe_process_lfsr,
        );
        self.pe_hurricane_wind_remaining = tick.wind_remaining;
        if tick.play_wind {
            self.queue_disaster_extra_sounds(PlacementSound::Wind, 1);
        }
        let Some((x, y)) = tick.destroyed else {
            return 0;
        };
        if self.auto_goto_enabled {
            self.pending_disaster_auto_goto = Some((x, y));
        }
        self.queue_disaster_extra_sounds(PlacementSound::Explode, 1);
        1
    }

    pub(super) fn tick_pe_disaster_clock(&mut self) -> usize {
        self.tick_pe_disaster_duration();
        // PE 0x45f760 runs once per pending disaster dispatch, separately from calendar.
        // The 200ms producer and mode-2 parity live in pe_idle_scheduler. There is no
        // synchronous repeat loop inside the native start wrapper.
        // Tornado/monster/air-crash XTHG run on the Things walker (`0x4509ba`) via realtime ticks.
        // Maxis Man is not daily: sole ctor caller is disaster-start `0x45d066` (DECLINED + RandomAnd3).
        let mut changed = self.tick_pe_disaster_overlays();
        changed += self.tick_pe_hurricane_wind();
        changed
    }

    pub(super) fn tick_pe_disaster_xthg_actors_in_slot_order(&mut self) -> usize {
        // PE 0x45096c..0x4509b5 rolls the ambient sound gates before slot 1.
        // The shared CRT advances even when effects playback is disabled.
        if self.city.pe_attitude_score_at(0) as i32 > 35
            && self.pe_process_crt.random_u15() & 0xff == 0
        {
            self.queue_disaster_extra_sounds(PlacementSound::Horns, 1);
        }
        if self.city.pe_dw_tile_count[POLICE_STATION_BUILDING_ID as usize] != 0
            && self.city.pe_attitude_score_at(3) as i32 > 40
            && self.pe_process_crt.random_u15() & 0xff == 0
        {
            self.queue_disaster_extra_sounds(PlacementSound::Police, 1);
        }

        let (aircraft, monster, tornado, explosion, maxis_man_changed, train_changed) =
            pe_disaster_xthg_tick_slots_in_order(
                &mut self.win95_overlay_sim,
                &mut self.city,
                &mut self.pe_process_crt,
                &mut self.pe_process_lfsr,
                &mut self.pe_process_game_rand,
            );
        let train_sounds = self.win95_overlay_sim.take_train_sfx_plays();
        self.queue_disaster_extra_sounds(PlacementSound::Train, train_sounds);
        if aircraft.mayday_sounds > 0 {
            self.queue_disaster_extra_sounds(PlacementSound::Mayday, aircraft.mayday_sounds);
        }
        if monster.attack_sounds > 0 {
            self.queue_disaster_extra_sounds(PlacementSound::PowerLine, monster.attack_sounds);
        }
        if explosion.explode_sounds > 0 {
            // Win95 `0x401096(0x1f8)` when kind-6 lane/`+1` is 0.
            self.queue_disaster_extra_sounds(PlacementSound::Explode, explosion.explode_sounds);
        }
        self.publish_pe_explosion_point(explosion.trigger_point);
        let disaster_changed = aircraft.changed()
            + monster.changed()
            + tornado.changed()
            + explosion.changed()
            + maxis_man_changed;
        let changed = disaster_changed + train_changed;
        if disaster_changed > 0 && !self.city.has_active_disaster() {
            self.persist_cleared_disaster_misc();
        }
        if disaster_changed > 0 {
            self.city.mark_all_dirty();
        }
        changed
    }

    fn tick_pe_explosion_actors(&mut self) -> usize {
        let stats = pe_explosion_tick_city_with_lfsr(
            &mut self.city,
            &mut self.pe_process_crt,
            &mut self.pe_process_lfsr,
        );
        if stats.explode_sounds > 0 {
            // Win95 `0x401096(0x1f8)` when kind-6 lane/`+1` is 0.
            self.queue_disaster_extra_sounds(PlacementSound::Explode, stats.explode_sounds);
        }
        self.publish_pe_explosion_point(stats.trigger_point);
        let changed = stats.changed();
        if changed > 0 && !self.city.has_active_disaster() {
            self.persist_cleared_disaster_misc();
        }
        if changed > 0 {
            self.city.mark_all_dirty();
        }
        changed
    }

    fn publish_pe_explosion_point(&mut self, point: Option<(usize, usize)>) {
        if let Some((x, y)) = point {
            self.pe_idle_scheduler.pending_disaster_point = Some((x as i16, y as i16));
            if self.auto_goto_enabled {
                self.pending_disaster_auto_goto = Some((x, y));
            }
        }
    }

    pub(super) fn tick_pe_tornado_actors(&mut self) -> usize {
        let stats = pe_tornado_tick_city_with_rng(&mut self.city, &mut self.pe_process_crt);
        let changed = stats.changed();
        if changed > 0 && !self.city.has_active_disaster() {
            self.persist_cleared_disaster_misc();
        }
        if changed > 0 {
            self.city.mark_all_dirty();
        }
        changed
    }

    pub(super) fn tick_pe_aircrash_actors(&mut self) -> usize {
        let stats = pe_aircrash_tick_city_with_lfsr(&mut self.city, &mut self.pe_process_lfsr);
        if stats.mayday_sounds > 0 {
            // Win95 `0x401096(0x203)` when diving height crosses 8.
            self.queue_disaster_extra_sounds(PlacementSound::Mayday, stats.mayday_sounds);
        }
        let changed = stats.changed();
        if changed > 0 && !self.city.has_active_disaster() {
            self.persist_cleared_disaster_misc();
        }
        if changed > 0 {
            self.city.mark_all_dirty();
        }
        changed
    }

    pub(super) fn tick_pe_monster_actors(&mut self) -> usize {
        let (center_x, center_y) = self.city.pe_city_center();
        let stats = pe_monster_tick_city_with_rng(
            &mut self.city,
            center_x,
            center_y,
            &mut self.pe_process_crt,
            &mut self.pe_process_lfsr,
        );
        if stats.attack_sounds > 0 {
            // Win95 `0x401096(0x202)` after an attack-marked move.
            self.queue_disaster_extra_sounds(PlacementSound::PowerLine, stats.attack_sounds);
        }
        let changed = stats.changed();
        if changed > 0 && !self.city.has_active_disaster() {
            self.persist_cleared_disaster_misc();
        }
        if changed > 0 {
            self.city.mark_all_dirty();
        }
        changed
    }

    pub(super) fn tick_pe_disaster_overlays(&mut self) -> usize {
        let pe_flood = pe_flood_overlay_scan_due(&self.city);
        let pe_fire = pe_fire_overlay_scan_due(&self.city);
        let pe_riot = pe_riot_overlay_scan_due(&self.city);
        let pe_fb = pe_fb_overlay_scan_due(&self.city);
        let pe_dispatch = self.city.has_active_disaster();
        if !(pe_flood || pe_fire || pe_riot || pe_fb || pe_dispatch) {
            return 0;
        }
        let seen = pe_disaster_overlay_seen(&self.city);
        let stats = pe_disaster_overlay_tick_city_with_rng(
            &mut self.city,
            self.pe_disaster_duration,
            pe_dispatch,
            &mut self.pe_process_crt,
            &mut self.pe_process_lfsr,
        );
        let changed = stats.changed();
        self.queue_disaster_extra_sounds(PlacementSound::Explode, stats.flood_explodes());
        self.queue_pe_disaster_scan_sounds(seen);
        if changed > 0 && !self.city.has_active_disaster() {
            self.persist_cleared_disaster_misc();
        }
        if changed > 0 {
            self.city.mark_all_dirty();
            self.set_sim_status_announcement(if self.city.has_active_disaster() {
                "Disaster Spreading"
            } else {
                "Disaster Contained"
            });
        }
        changed
    }

    fn queue_pe_disaster_scan_sounds(&mut self, seen: PeDisasterOverlaySeen) {
        let sounds = pe_disaster_scan_sounds_with_rng(&self.city, seen, &mut self.pe_process_crt);
        if sounds.play_riot {
            self.queue_disaster_extra_sounds(PlacementSound::Riot, 1);
        }
        if sounds.play_flood {
            self.queue_disaster_extra_sounds(PlacementSound::Flood, 1);
        }
        if sounds.play_fire {
            self.queue_disaster_extra_sounds(PlacementSound::Fire, 1);
        }
    }

    pub(super) fn persist_cleared_disaster_misc(&mut self) {
        if self.engine_fidelity.uses_win95() {
            persist_pe_disaster_misc(&mut self.city, None, self.mode, self.engine_fidelity);
            self.maybe_open_pe_disaster_end_newspaper();
        }
    }
}
