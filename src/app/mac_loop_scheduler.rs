use super::*;
use crate::mac_random::{MacAppLfsrRng, MacGameLcgRng};
use crate::win95_overlay::{
    XTHG_FIRST_USABLE_SLOT, XTHG_MAXIS_MAN, XTHG_RECORD_COUNT, XTHG_SAILBOAT,
};

mod maxis_man;
pub(super) mod rail_consist;
mod rail_routes;

const MAC_KIND3_CANDIDATE_ROW_DELTA: [i16; 8] = [0, 3, 3, 3, 0, -3, -3, -3];
const MAC_KIND3_CANDIDATE_COL_DELTA: [i16; 8] = [-3, -3, 0, 3, 3, 3, 0, -3];
const MAC_KIND3_ALLOWED_XBLD: [u8; 9] = [0x51, 0x52, 0x54, 0x55, 0x58, 0x59, 0x5b, 0x5c, 0x6b];
const MAC_KIND3_MOVE_ROW_DELTA: [i16; 8] = [0, 3, 4, 3, 0, -3, -4, -3];
const MAC_KIND3_MOVE_COL_DELTA: [i16; 8] = [-4, -3, 0, 3, 4, 3, 0, -3];
const MAC_KIND3_HAZARD_ROW_DELTA: [i16; 4] = [2, 0, -2, 0];
const MAC_KIND3_HAZARD_COL_DELTA: [i16; 4] = [0, 2, 0, -2];
const MAC_KIND3_PHASE2_ZERO_WINDOW: [i16; 9] = [0, 7, 6, 5, 4, 3, 2, 1, 2];
const MAC_KIND3_FORWARD_DIRECTION_WINDOW: [i16; 9] = [0, 1, 2, 3, 4, 5, 6, 7, 0];
const MAC_KIND1_MOVE_ROW_UNIT: [i16; 8] = [0, 1, 1, 1, 0, -1, -1, -1];
const MAC_KIND1_MOVE_COL_UNIT: [i16; 8] = [-1, -1, 0, 1, 1, 1, 0, -1];
const MAC_EXPORT80_RENDER_WAIT_TICKS_PER_LOOP: u32 = 12;

/// CODE 2's post-event 0x26..0xb2 admission. The adaptive actor timer and
/// native action state are separate workers, not the shared road renderer.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct MacLoopScheduler {
    elapsed: Duration,
    deadline: u32,
    /// Native TickCount deadline `ffff8cb2` for CODE 6 export 109.
    actor_deadline: u32,
    /// Native signed WORD `ffff8c82` used to select the next 30/45-tick interval.
    actor_smoother: i16,
    /// Native WORD ffff8c80, accumulated after normal calendar dispatches.
    dispatch_counter: u16,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MacLoopDispatch {
    Calendar,
    Action,
}

impl MacLoopScheduler {
    fn advance_elapsed(&mut self, elapsed: Duration) -> u32 {
        self.elapsed = self.elapsed.saturating_add(elapsed);
        // A virtual 60 Hz TickCount, initialized with the deadline at epoch 0.
        // Keep fractional ticks through pause/editor frames; no float backlog.
        (self.elapsed.as_nanos() * 60 / 1_000_000_000) as u32
    }

    fn admit_at(
        &mut self,
        mode: i16,
        speed: i16,
        check: u32,
        refresh: u32,
    ) -> Option<MacLoopDispatch> {
        if !matches!(mode, 1 | 2) || speed <= 1 || check <= self.deadline {
            return None;
        }
        let interval = match speed {
            2 => 36,
            3 => 12,
            4 => 0,
            // Invalid selectors would address outside the native menu table.
            _ => return None,
        };
        self.deadline = refresh.wrapping_add(interval);
        Some(if mode == 1 {
            MacLoopDispatch::Calendar
        } else {
            MacLoopDispatch::Action
        })
    }

    fn finish_dispatch(&mut self) {
        // The native WORD increment occurs after the worker returns.
        self.dispatch_counter = self.dispatch_counter.wrapping_add(1);
    }

    fn due_actor_sweep(&mut self, run_state: i16, speed: i16, now: u32) -> bool {
        if speed <= 1 {
            return false;
        }
        if now <= self.actor_deadline {
            return false;
        }

        // CODE 2 `0x336..0x360`: scale the WORD counter by four with WORD
        // wrap, sign-extend it through ADDA.W, smooth, and store the result.
        let scaled_counter = self.dispatch_counter.wrapping_mul(4) as i16;
        let sum = 7i32 * i32::from(self.actor_smoother) + i32::from(scaled_counter);
        self.actor_smoother = (sum / 8) as i16;
        self.dispatch_counter = 0;
        let interval = if run_state == 2 || self.actor_smoother >= 6 {
            30
        } else {
            45
        };
        self.actor_deadline = now.wrapping_add(interval);
        true
    }
}

impl ViewerState {
    pub(super) fn advance_mac_frame_simulation(&mut self, elapsed: Duration) -> GameClockAdvance {
        let mut now = self.mac_loop_scheduler.advance_elapsed(elapsed);
        // TickCount continues while the map editor is open, but the Mac
        // simulation callback belongs to the live city view. Keep editor
        // frames out of both the actor sweep and calendar/action dispatcher.
        if self.mode != AppMode::CityView {
            return GameClockAdvance::default();
        }
        let mode = 1;
        let speed = match self.traffic.speed() {
            GameSpeed::Paused => 1,
            GameSpeed::Turtle => 2,
            GameSpeed::Llama => 3,
            GameSpeed::Cheetah | GameSpeed::AfricanSwallow => 4,
        };
        // Mac CODE 4 owns MISC +0x04 as its run-state gate for either saved
        // layout. On Win-layout cities this is also the shared persisted city
        // mode word; do not silently force gameplay while Mac fidelity is in
        // editor/action state.
        let run_state = self
            .city
            .mac_state_misc_u32_at(MISC_CITY_MODE_OFFSET)
            .unwrap_or(mode as u32) as i16;
        // Native CODE 2 checks the independent actor timer before CODE 14's
        // calendar deadline. The Mac actor pass includes kind-1/2/3/5/6/9/16,
        // kind-10/12 trains, sailboat, and tornado handlers. It remains a
        // bounded port: mode-2 action dispatch, draw-time cleanup, and
        // original-session QuickDraw chronology are not established. The
        // rail-head counter is rebuilt here and shared with its constructor
        // and monthly producer. This does not enter the PE actor or
        // road-traffic path.
        if self
            .mac_loop_scheduler
            .due_actor_sweep(run_state, speed, now)
        {
            // Zoom gates the native redraw helper, not the actor update.
            let native_zoom = (crate::traffic::traffic_shape_scale_band(self.zoom) / 500) as u16;
            let sound_requests = tick_mac_actors_with_rng_and_clock(
                &mut self.city,
                &mut self.mac_process_rng,
                &mut self.mac_game_rng,
                &mut self.mac_kind2_sound_deadline,
                native_zoom,
                &mut now,
            );
            for sound in sound_requests {
                self.queue_disaster_extra_sounds(sound, 1);
            }
        }
        // Native pending selectors and the CODE 10 mode-2 action worker are
        // not connected to ViewerState; the existing PE manual-action adapter
        // is not that state. Still admit/count a due mode-2 dispatch so it
        // suppresses the calendar and contributes to the native actor smoother.
        // Do not infer Mac run-state from a PE active-disaster flag.
        match self.mac_loop_scheduler.admit_at(run_state, speed, now, now) {
            Some(MacLoopDispatch::Calendar) => {}
            Some(MacLoopDispatch::Action) => {
                self.mac_loop_scheduler.finish_dispatch();
                return GameClockAdvance::default();
            }
            None => return GameClockAdvance::default(),
        }
        let start = self.game_clock;
        self.game_clock = start.wrapping_add_days(1);
        let advance = GameClockAdvance {
            days: 1,
            months: u32::from(start.month() != self.game_clock.month()),
        };
        self.apply_game_clock_advance(start, advance);
        self.mac_loop_scheduler.finish_dispatch();
        advance
    }
}

/// Rooted Mac CODE 6 export-109 kind-9 path under the reconstructed zero
/// A5-2 LFSR. State-zero actors on water activate, consume one QuickDraw draw
/// for their heading turn, and become state 1; the next due sweep removes them.
/// Kind 3 contributes its rooted visit counter and unconditional QuickDraw
/// draw; active phase-0/1 records route and move with the native Mac XTXT
/// marker rules, and phases 2–4 run their native candidate search/return path.
/// Other phases remain open. CODE 6 applies the zoom thresholds to redraws
/// before and after handlers, not to the handlers themselves.
#[cfg(test)]
fn tick_mac_marina_actors_and_count_kind3(
    city: &mut City,
    quickdraw: &mut MacToolboxRng,
    _native_zoom: u16,
) -> Vec<PlacementSound> {
    let mut game_rng = MacGameLcgRng::new(MacGameLcgRng::INITIAL_STATE);
    tick_mac_actors_with_rng(city, quickdraw, &mut game_rng, _native_zoom)
}

fn tick_mac_actors_with_rng(
    city: &mut City,
    quickdraw: &mut MacToolboxRng,
    game_rng: &mut MacGameLcgRng,
    _native_zoom: u16,
) -> Vec<PlacementSound> {
    let mut kind2_sound_deadline = 0;
    tick_mac_actors_with_rng_at(
        city,
        quickdraw,
        game_rng,
        &mut kind2_sound_deadline,
        _native_zoom,
        0,
    )
}

fn tick_mac_actors_with_rng_at(
    city: &mut City,
    quickdraw: &mut MacToolboxRng,
    game_rng: &mut MacGameLcgRng,
    kind2_sound_deadline: &mut u32,
    _native_zoom: u16,
    now: u32,
) -> Vec<PlacementSound> {
    let mut tick_count = now;
    tick_mac_actors_with_rng_and_clock(
        city,
        quickdraw,
        game_rng,
        kind2_sound_deadline,
        _native_zoom,
        &mut tick_count,
    )
}

fn tick_mac_actors_with_rng_and_clock(
    city: &mut City,
    quickdraw: &mut MacToolboxRng,
    game_rng: &mut MacGameLcgRng,
    kind2_sound_deadline: &mut u32,
    _native_zoom: u16,
    tick_count: &mut u32,
) -> Vec<PlacementSound> {
    // This is called only by the Mac-fidelity scheduler. Keep the simulation
    // engine's actor semantics independent from the saved city-file layout.
    // Native export 109 always owns a 40×12 runtime table even when the
    // optional serialized chunk was absent.
    let _ = city.mac_object_slot_records_mut();
    let mut now = *tick_count;
    city.mac_marina_actor_counter = 0;
    city.mac_kind3_actor_counter = 0;
    city.mac_kind1_actor_counter = 0;
    city.mac_kind2_actor_counter = 0;
    city.mac_kind5_actor_counter = 0;
    city.mac_kind16_actor_counter = 0;
    city.mac_tornado_actor_counter = 0;
    city.mac_rail_actor_counter = 0;
    // The native pre-slot sound probes consume the same QuickDraw stream as
    // the later actor handlers, even when playback is disabled.
    let mut sound_requests =
        if let Some((traffic, crime, police)) = mac_actor_sweep_history_inputs(city) {
            mac_actor_sweep_prelude(traffic, crime, police, || quickdraw.random_u16())
        } else {
            Vec::new()
        };
    for slot in XTHG_FIRST_USABLE_SLOT..XTHG_RECORD_COUNT {
        let Some(mut record) = city
            .traffic_overlays()
            .and_then(|records| records.get(slot))
            .copied()
        else {
            continue;
        };
        if !matches!(
            record.kind,
            1 | 2 | 3 | 5 | 6 | 10 | 12 | XTHG_SAILBOAT | XTHG_TORNADO | XTHG_MAXIS_MAN
        ) {
            continue;
        }
        if record.kind == XTHG_MAXIS_MAN {
            city.mac_kind16_actor_counter = city.mac_kind16_actor_counter.wrapping_add(1);
            maxis_man::tick(city, slot, &mut record, quickdraw, &mut sound_requests);
            city.set_mac_object_slot_record(slot, record);
            continue;
        }
        if record.kind == XTHG_TORNADO {
            // CODE 6 FUN_0000e380 counts this visit, destructs the actor's
            // current developed tile through export 80, then makes one or two
            // independent heading/movement attempts from the saved heading.
            city.mac_tornado_actor_counter = city.mac_tornado_actor_counter.wrapping_add(1);
            let x = usize::from(record.tile_x);
            let y = usize::from(record.tile_y);
            let origin_building = city.building_at(x, y);
            if origin_building >= 6 {
                let wait_loops =
                    super::budget::mac_destroy_building_export80(city, x, y, quickdraw, true);
                now = now
                    .wrapping_add(wait_loops.wrapping_mul(MAC_EXPORT80_RENDER_WAIT_TICKS_PER_LOOP));
            }

            let first_a = quickdraw.random_u16();
            let first_b = quickdraw.random_u16();
            let first_heading = mac_tornado_heading_mod3(record.lane, first_a, first_b);
            mac_move_one_step_with_heading(city, slot, &mut record, 8, first_heading);
            // Native BFTST (SP){8:8} selects the low byte of the big-endian
            // QuickDraw WORD; cleanup is gated by that byte being zero.
            if quickdraw.random_u16() & 0x00ff == 0 {
                mac_export121_clear_actor(city, slot, &mut record);
            }

            // The native condition uses the XBLD value saved before export 80.
            // Its second turn is relative to the original record heading, not
            // the first computed movement heading; export 121 does not abort
            // this handler's remaining branch.
            if origin_building < 0x0d {
                let second_a = quickdraw.random_u16();
                let second_b = quickdraw.random_u16();
                let second_heading = mac_tornado_heading_bit(record.lane, second_a, second_b);
                mac_move_one_step_with_heading(city, slot, &mut record, 8, second_heading);
                if quickdraw.random_u16() & 0x00ff == 0 {
                    mac_export121_clear_actor(city, slot, &mut record);
                }
            }
            city.set_mac_object_slot_record(slot, record);
            continue;
        }
        if record.kind == 1 {
            // CODE 6 increments ffffa092 on handler entry. The early sprite-
            // height redirect precedes the phase switch.
            city.mac_kind1_actor_counter = city.mac_kind1_actor_counter.wrapping_add(1);
            if mac_kind1_height_redirect(city, &mut record) {
                city.set_mac_object_slot_record(slot, record);
                continue;
            }
            if record.state & 0x0f == 0 {
                mac_kind1_move_one_step(city, slot, &mut record);
                if record.height == 0 {
                    sound_requests.push(PlacementSound::MacTakeoff);
                }
                if record.height >= 0x0e {
                    record.state = 2;
                } else {
                    record.height = record.height.wrapping_add(1);
                }
                city.set_mac_object_slot_record(slot, record);
            } else if record.state & 0x0f == 1 {
                // CODE 6 d20c moves before decrementing +5. Reaching zero
                // requests Land, restores the proxy underlay, then either
                // retires on runway XBLD 0xdd or enters the phase-5 marker.
                mac_kind1_move_one_step(city, slot, &mut record);
                record.height = record.height.wrapping_sub(1);
                if record.height == 0 {
                    sound_requests.push(PlacementSound::MacLand);
                    let x = usize::from(record.tile_x);
                    let y = usize::from(record.tile_y);
                    if x < CITY_WIDTH && y < CITY_HEIGHT {
                        city.set_text_overlay(x, y, record.saved_a);
                        city.mark_render_dirty(x, y);
                        if city.building_at(x, y) == 0xdd {
                            city.set_text_overlay(x, y, 0);
                            record.kind = 0;
                        } else {
                            city.set_text_overlay(x, y, 6);
                            record.state = 5;
                            record.saved_b = 1;
                            record.lane = 0;
                        }
                    }
                }
                city.set_mac_object_slot_record(slot, record);
            } else if record.state & 0x0f == 2 {
                // CODE 6 d354 perturbs +1 with FUN_0000b2c6(5), checks the
                // adjacent XBLD arcology band through FUN_0000dbc8, then moves.
                record.lane = mac_kind1_perturb_heading(record.lane, quickdraw);
                mac_kind1_avoid_arcology_neighbor(city, &mut record);
                mac_kind1_move_one_step(city, slot, &mut record);
                city.set_mac_object_slot_record(slot, record);
            } else if record.state & 0x0f == 3 {
                // CODE 6 d3ac selects a sign-octant heading with FUN_0000b354,
                // moves, then turns toward the state high nibble once within
                // Manhattan distance 1. That transition enters phase 4 and
                // moves the target six tiles along cardinal headings.
                record.lane = mac_kind1_sign_heading(&record);
                mac_kind1_move_one_step(city, slot, &mut record);
                if mac_kind1_target_distance(&record) < 2 {
                    let target_heading = record.state >> 4;
                    record.lane = mac_kind3_heading_turn(record.lane, target_heading);
                    record.state = (target_heading << 4) | 4;
                    match target_heading {
                        0 => record.target_y = record.target_y.wrapping_sub(6),
                        2 => record.target_x = record.target_x.wrapping_add(6),
                        4 => record.target_y = record.target_y.wrapping_add(6),
                        6 => record.target_x = record.target_x.wrapping_sub(6),
                        // Native odd-heading table entries return directly;
                        // heading 7 reaches a shared guard that exits for all
                        // usable (nonzero) actor slots.
                        _ => {}
                    }
                }
                city.set_mac_object_slot_record(slot, record);
            } else if record.state & 0x0f == 4 {
                // CODE 6 d522 calls FUN_0000b3b6 (FUN_0000b41e's slope-aware
                // heading plus one-octant turn), moves, then finishes the
                // alignment once it is within Manhattan distance 1.
                let desired = crate::mac_traffic_geometry::mac_direction_to_tile(
                    usize::from(record.tile_x),
                    usize::from(record.tile_y),
                    usize::from(record.target_x),
                    usize::from(record.target_y),
                );
                record.lane = mac_kind3_turn_one_step(record.lane, desired);
                mac_kind1_move_one_step(city, slot, &mut record);
                if mac_kind1_target_distance(&record) < 2 {
                    let target_heading = record.state >> 4;
                    record.lane = target_heading;
                    record.state = 1;
                    match target_heading {
                        0 | 4 => record.tile_x = record.target_x,
                        2 | 6 => record.tile_y = record.target_y,
                        // The native heading-7 arm clears +1; odd nonterminal
                        // arms leave coordinates as they are.
                        7 => record.lane = 0,
                        _ => {}
                    }
                }
                city.set_mac_object_slot_record(slot, record);
            } else if record.state & 0x0f == 7 {
                // CODE 6 d678 decrements +5. A zero count leaves the marker-6
                // phase-5 record; otherwise a resulting count of eight
                // requests MAYDAY5 before incrementing +1 and moving.
                if record.height == 0 {
                    let x = usize::from(record.tile_x);
                    let y = usize::from(record.tile_y);
                    if x < CITY_WIDTH && y < CITY_HEIGHT {
                        city.set_text_overlay(x, y, 6);
                        city.mark_render_dirty(x, y);
                    }
                    record.state = 5;
                    record.saved_b = 1;
                    record.lane = 0;
                } else {
                    record.height = record.height.wrapping_sub(1);
                    if record.height == 8 {
                        sound_requests.push(PlacementSound::Mayday);
                    }
                    record.lane = record.lane.wrapping_add(1) & 7;
                    mac_kind1_move_one_step(city, slot, &mut record);
                }
                city.set_mac_object_slot_record(slot, record);
            }
            continue;
        }
        if record.kind == 2 {
            // CODE 6 increments ffffa094 on kind-2 handler entry. Its rooted
            // states 0, 2, 3, 4, and 5 are integrated below. State 2 uses the
            // native kind-2 stride, shared hotspot, XTRF threshold and ordered
            // QuickDraw draws independently of the saved file layout.
            city.mac_kind2_actor_counter = city.mac_kind2_actor_counter.wrapping_add(1);
            match record.state {
                0 => {
                    record.lane = record.lane.wrapping_add(1) & 7;
                    if record.height < 10 {
                        record.height = record.height.wrapping_add(1);
                    } else {
                        record.state = 2;
                    }
                    city.set_mac_object_slot_record(slot, record);
                }
                2 => {
                    let desired = crate::mac_traffic_geometry::mac_direction_to_tile(
                        usize::from(record.tile_x),
                        usize::from(record.tile_y),
                        usize::from(record.target_x),
                        usize::from(record.target_y),
                    );
                    record.lane = mac_kind3_turn_one_step(record.lane, desired);
                    mac_kind1_avoid_arcology_neighbor(city, &mut record);
                    mac_move_one_step_with_stride(city, slot, &mut record, 8);

                    let x = usize::from(record.tile_x);
                    let y = usize::from(record.tile_y);
                    if x < CITY_WIDTH
                        && y < CITY_HEIGHT
                        && city.traffic_density_at(x, y).unwrap_or(0) >= 0xaa
                        && *kind2_sound_deadline < now
                    {
                        sound_requests.push(PlacementSound::MacKind2Traffic);
                        *kind2_sound_deadline = now.wrapping_add(5000);
                    }

                    if mac_kind1_target_distance(&record) < 2 {
                        let (hotspot_x, hotspot_y) =
                            city.mac_kind2_hotspot().unwrap_or_else(|| {
                                let (x, y) = super::density::mac_developed_tile_anchor(city);
                                city.set_mac_kind2_hotspot(x as u16, y as u16);
                                (x as u16, y as u16)
                            });
                        record.target_x = mac_kind2_random_target_axis(hotspot_x, quickdraw);
                        record.target_y = mac_kind2_random_target_axis(hotspot_y, quickdraw);
                    }

                    let x = usize::from(record.tile_x);
                    let y = usize::from(record.tile_y);
                    let unoccupied = x < CITY_WIDTH
                        && y < CITY_HEIGHT
                        && city.building_at(x, y) == 0
                        && city
                            .tile_bits_at(x, y)
                            .map_or(true, |bits| bits.raw_bits() == 0);
                    if unoccupied && quickdraw.random_u16() & 1 != 0 {
                        record.state = 3;
                    }
                    city.set_mac_object_slot_record(slot, record);
                }
                3 => {
                    record.lane = record.lane.wrapping_add(1) & 7;
                    if record.height > 2 {
                        if record.height == 4 {
                            sound_requests.push(PlacementSound::Mayday);
                        }
                        record.height = record.height.wrapping_sub(1);
                    } else {
                        record.kind = 6;
                        record.state = 0x11;
                        record.saved_b = 1;
                        record.lane = 0;
                    }
                    city.set_mac_object_slot_record(slot, record);
                }
                4 => {
                    // CODE 6's state-4 arm consumes one QuickDraw `_Random`
                    // value and clears the phase only when its signed
                    // remainder modulo 20 is zero.
                    if quickdraw.random_i16() % 20 == 0 {
                        record.state = 0;
                        city.set_mac_object_slot_record(slot, record);
                    }
                }
                5 => {
                    record.lane = record.lane.wrapping_add(2) & 7;
                    if record.height == 4 {
                        sound_requests.push(PlacementSound::Mayday);
                    }
                    if record.height <= 2 {
                        record.kind = 6;
                        record.state = 0x11;
                        record.saved_b = 1;
                        record.lane = 0;
                    } else {
                        record.height = record.height.wrapping_sub(1);
                    }
                    city.set_mac_object_slot_record(slot, record);
                }
                _ => {}
            }
            continue;
        }
        if record.kind == 5 {
            // CODE 6 increments ffffa09c at kind-5 handler entry. That
            // sweep-associated gate is shared with the manual creator.
            city.mac_kind5_actor_counter = city.mac_kind5_actor_counter.wrapping_add(1);
            // CODE 6 FUN_0000deba first applies the cross-actor state writes,
            // then runs the Monster's own update. The native scans are
            // ordered kind-1 then kind-2 and cover usable slots 1..=39.
            // Counters include constructors and handler visits since the
            // sweep reset. This Rust slice models visits already reached in
            // this actor walk; monthly constructor interleave is not yet wired.
            if city.mac_kind1_actor_counter != 0 {
                for target_slot in XTHG_FIRST_USABLE_SLOT..XTHG_RECORD_COUNT {
                    if let Some(mut target) = city
                        .traffic_overlays()
                        .and_then(|records| records.get(target_slot))
                        .copied()
                        && target.kind == 1
                    {
                        target.state = 7;
                        city.set_mac_object_slot_record(target_slot, target);
                    }
                }
            }
            if city.mac_kind2_actor_counter != 0 {
                for target_slot in XTHG_FIRST_USABLE_SLOT..XTHG_RECORD_COUNT {
                    if let Some(mut target) = city
                        .traffic_overlays()
                        .and_then(|records| records.get(target_slot))
                        .copied()
                        && target.kind == 2
                    {
                        target.state = 5;
                        city.set_mac_object_slot_record(target_slot, target);
                    }
                }
            }
            // States 0, 1, and 2 enter the common kind-5 movement tail; state
            // 3 owns its bounded proxy-cleanup branch below. Direct Monster
            // drawing remains separate.
            if record.state == 0 {
                if record.height <= 8 {
                    record.state = 1;
                } else {
                    record.height = record.height.wrapping_sub(1);
                }
                record.target_x = 0;
                record.target_y = 0;

                let (target_x, target_y) = city.mac_kind2_hotspot().unwrap_or((0, 0));
                let direction = crate::mac_traffic_geometry::mac_direction_to_tile(
                    usize::from(record.tile_x),
                    usize::from(record.tile_y),
                    usize::from(target_x),
                    usize::from(target_y),
                );
                if mac_kind5_candidate_is_kind14(city, &record, direction) {
                    record.state = 3;
                }
                // Native 0xe2ac..0xe352 still calls the shared mover after a
                // kind-14 hit. The mover walks through occupied XTXT actor
                // proxies, so the collision changes state but does not stop
                // this visit's movement attempt.
                record.lane = direction;
                mac_move_one_step_with_stride(city, slot, &mut record, 8);
                city.set_mac_object_slot_record(slot, record);
            } else if record.state == 2 {
                // CODE 6 FUN_0000deba @ 0xe190 waits for +5 to reach 15.
                // It then uses QuickDraw _Random modulo 3 to either clear the
                // state or enter state 3. Both paths continue into the shared
                // target/movement tail during this same visit.
                if record.height < 15 {
                    record.height = record.height.wrapping_add(1);
                } else {
                    record.state = if quickdraw.random_u16() % 3 == 0 {
                        3
                    } else {
                        0
                    };
                }

                // A second _Random supplies the shared 0..=7 target, which
                // is stored in 9-pixel/tile units on both axes.
                let target = (quickdraw.random_u16() & 7).wrapping_mul(9) as u8;
                record.target_x = target;
                record.target_y = target;
                let direction = mac_kind1_perturb_heading(record.lane, quickdraw);
                if record.state != 3 && mac_kind5_candidate_is_kind14(city, &record, direction) {
                    record.state = 3;
                }
                record.lane = direction;
                mac_move_one_step_with_stride(city, slot, &mut record, 8);
                city.set_mac_object_slot_record(slot, record);
            } else if record.state == 1 {
                // CODE 6 FUN_0000deba @ 0xdfe6 uses three independent
                // QuickDraw _Random calls: one modulo-25 state gate and two
                // modulo-128 target coordinates. These are not TickCount
                // samples and must advance the shared Mac Random stream.
                if quickdraw.random_u16() % 25 == 0 {
                    record.state = 2;
                }
                record.target_x = (quickdraw.random_u16() % 128) as u8;
                record.target_y = (quickdraw.random_u16() % 128) as u8;
                let direction = mac_kind1_perturb_heading(record.lane, quickdraw);
                // State 1 tests the actor's current row/column. The heading
                // update affects the common mover, not the map-effect cell.
                let x = i16::from(record.tile_x);
                let y = i16::from(record.tile_y);

                // The native candidate test rejects row/column >= 127 before
                // translating them to one-based PlaceTile helper coordinates.
                if (0..127).contains(&x) && (0..127).contains(&y) {
                    let x = x as usize;
                    let y = y as usize;
                    if record.saved_b == 0 {
                        if city.building_at(x, y) >= 6 {
                            let mut app_lfsr = MacAppLfsrRng::new(MacAppLfsrRng::INITIAL_STATE);
                            if mac_export247_candidate(city, x, y, true, &mut app_lfsr, quickdraw) {
                                record.target_x |= 0x80;
                            }
                        }
                    } else if city
                        .tile_bits_at(x, y)
                        .is_some_and(|bits| bits.raw_bits() & 0x04 == 0)
                        && city.building_at(x, y) >= 0x0e
                        && city.building_at(x, y) != 0xc8
                    {
                        // Export 80 is called with option 0xff. Its render
                        // branch consumes selector-specific QuickDraw draws
                        // from the shared Toolbox RNG even though CityGame
                        // does not reproduce those sprite callbacks here.
                        // Preserve that RNG cadence with the map edits, then
                        // reread +0xb as the native handler does.
                        let render_wait_loops = super::budget::mac_destroy_building_export80(
                            city, x, y, quickdraw, true,
                        );
                        // Export 80 has selector-specific +12 wait loops. Its
                        // generic outer extent repeats the wait once per row;
                        // connected extents have one terminal wait, and
                        // classifier-triggered nested export-80 calls add
                        // their own waits. The static helper returns this
                        // count while presentation callbacks remain omitted.
                        now = now.wrapping_add(
                            render_wait_loops.wrapping_mul(MAC_EXPORT80_RENDER_WAIT_TICKS_PER_LOOP),
                        );
                        // Native reloads XTHG +0x0b after the helper. Keep
                        // that memory boundary visible instead of branching
                        // on only the pre-call local record copy.
                        let status = city
                            .traffic_overlays()
                            .and_then(|records| records.get(slot))
                            .map_or(record.saved_b, |after| after.saved_b);
                        record.saved_b = status;
                        match status as i8 {
                            1 => {
                                // CODE 6 `0xe12a` selects this tile with
                                // QuickDraw `_Random & 3` after export 80.
                                let tile = 9 + (quickdraw.random_u16() as u8 & 3);
                                city.set_building(x, y, tile);
                            }
                            2 => {
                                super::terrain_generation::mac68k_repair_water_component(city, x, y)
                            }
                            3 => mac_kind5_place_c8(city, x, y, quickdraw),
                            _ => {}
                        }
                        // CODE 6's nonzero-status epilogue sets this regardless
                        // of whether export 80 or the selected helper changed
                        // the map.
                        record.target_x |= 0x80;
                    }
                }
                record.lane = direction;
                mac_move_one_step_with_stride(city, slot, &mut record, 8);
                city.set_mac_object_slot_record(slot, record);
            } else if record.state == 3 {
                // CODE 6 `FUN_0000deba @ 0xe236` clears +8/+9 each visit,
                // sets both bytes to 0x24 when QuickDraw `_Random & 1` is
                // nonzero, then calls CODE 13 export 203 with bound 100. Its
                // process LFSR starts at zero and zero is a fixed point, so
                // the result is always zero and export 121 clears the proxy.
                record.target_x = 0;
                record.target_y = 0;
                if quickdraw.random_u16() & 1 != 0 {
                    record.target_x = 0x24;
                    record.target_y = 0x24;
                }
                let mut app_lfsr = MacAppLfsrRng::new(MacAppLfsrRng::INITIAL_STATE);
                if app_lfsr.bounded(100) == 0 {
                    let x = usize::from(record.tile_x);
                    let y = usize::from(record.tile_y);
                    if x < CITY_WIDTH && y < CITY_HEIGHT {
                        // Export 121 clears the tile byte directly; it does
                        // not retire or rewrite the XTHG record.
                        city.set_text_overlay(x, y, 0);
                    }
                }
                city.set_mac_object_slot_record(slot, record);
            }
            continue;
        }
        if matches!(record.kind, 10 | 12) {
            // CODE 6 export 109 counts each enabled head before dispatch and
            // visits heads in this same slot order. The count is pre-handler:
            // teardown does not subtract it until the next sweep. CODE 13
            // export 201 and Toolbox _Random remain separate streams.
            city.mac_rail_actor_counter = city.mac_rail_actor_counter.wrapping_add(1);
            if let Some((_, Some(sound_requested))) = rail_consist::mac_rail_handler(
                city,
                slot,
                || game_rng.bounded(2),
                || quickdraw.random_u16(),
            ) {
                if sound_requested {
                    sound_requests.push(PlacementSound::Train);
                }
            }
            continue;
        }
        // The native kind-3 handler increments ffffa096 and calls QuickDraw
        // _Random before checking the actor's XBIT or phase. Preserve this
        // visit count and slot-ordered draw on every handler path.
        if record.kind == 3 {
            city.mac_kind3_actor_counter = city.mac_kind3_actor_counter.wrapping_add(1);
            if mac_kind3_sound_requested(quickdraw.random_u16()) {
                sound_requests.push(PlacementSound::Cargo);
            }
            // CODE 8 establishes X as the row-pointer index and Y as the
            // within-row byte offset. Kind-3 +3/+4 preserve that raw X/Y
            // order, as does export 116's boundary constructor.
            let x = usize::from(record.tile_x);
            let y = usize::from(record.tile_y);
            if !city
                .tile_bits_at(x, y)
                .is_some_and(|bits| bits.raw_bits() & 0x04 != 0)
            {
                // CODE 6 FUN_0000f970 does not free the record or clear its
                // XTXT proxy here. It changes the kind and clears only +1,+2,
                // and +0x0b; the enabled kind-6 handler owns later cleanup.
                record.kind = 6;
                record.lane = 0;
                record.state = 0;
                record.saved_b = 0;
                city.set_mac_object_slot_record(slot, record);
                continue;
            }
            if record.state == 0 {
                let original_x = i16::from(record.tile_x);
                let original_y = i16::from(record.tile_y);
                let mut app_lfsr = MacAppLfsrRng::new(MacAppLfsrRng::INITIAL_STATE);
                if app_lfsr.bounded(10) == 0 {
                    let desired = crate::mac_traffic_geometry::mac_direction_to_tile(
                        usize::from(record.tile_x),
                        usize::from(record.tile_y),
                        usize::from(record.target_x),
                        usize::from(record.target_y),
                    );
                    let proposed = mac_kind3_turn_one_step(record.lane, desired);
                    if mac_kind3_route_candidate(city, &record, proposed) {
                        record.lane = proposed;
                    }
                }
                if mac_kind3_route_candidate(city, &record, record.lane) {
                    mac_kind3_move_one_step(city, slot, &mut record);
                } else {
                    record.state = 1;
                }
                // FUN_0000f970 checks these four offsets from the coordinates
                // loaded before movement; XBLD 0xdf forces phase 3.
                for index in 0..4 {
                    let x = original_x + MAC_KIND3_HAZARD_ROW_DELTA[index];
                    let y = original_y + MAC_KIND3_HAZARD_COL_DELTA[index];
                    if (0..128).contains(&x)
                        && (0..128).contains(&y)
                        && city.building_at(x as usize, y as usize) == 0xdf
                    {
                        record.state = 3;
                    }
                }
                city.set_mac_object_slot_record(slot, record);
                continue;
            }
            if record.state == 1 {
                // CODE 6 FUN_0000b41e receives the raw +3/+4 and +8/+9
                // coordinate pairs in that order; the shared helper preserves
                // its half-slope octant calculation. FUN_0000b30a then turns
                // one step on the eight-direction ring.
                let desired = crate::mac_traffic_geometry::mac_direction_to_tile(
                    usize::from(record.tile_x),
                    usize::from(record.tile_y),
                    usize::from(record.target_x),
                    usize::from(record.target_y),
                );
                let turned = mac_kind3_heading_turn(record.lane, desired);
                record.lane = turned;
                if turned == desired {
                    record.state = if mac_kind3_route_candidate(city, &record, turned) {
                        0
                    } else {
                        2
                    };
                }
                city.set_mac_object_slot_record(slot, record);
                continue;
            }
            if record.state == 2 {
                let mut app_lfsr = MacAppLfsrRng::new(MacAppLfsrRng::INITIAL_STATE);
                let offsets = if app_lfsr.bounded(2) == 0 {
                    &MAC_KIND3_PHASE2_ZERO_WINDOW
                } else {
                    &MAC_KIND3_FORWARD_DIRECTION_WINDOW
                };
                let mut direction = record.lane & 7;
                for (index, offset) in offsets.iter().copied().enumerate() {
                    direction = (i16::from(record.lane) + offset).rem_euclid(8) as u8;
                    if mac_kind3_route_candidate(city, &record, direction) {
                        if index == 8 {
                            // Keep the binary's call-before-write order. With
                            // unchanged initialized tables and read-only
                            // candidate checks, index 8 repeats index 6 and
                            // cannot be the first accepted candidate.
                            let x = usize::from(record.tile_x);
                            let y = usize::from(record.tile_y);
                            record.kind = 0;
                            if x < CITY_WIDTH && y < CITY_HEIGHT {
                                city.set_text_overlay(x, y, 0);
                            }
                        }
                        break;
                    }
                }
                record.lane = direction;
                record.state = 0;
                city.set_mac_object_slot_record(slot, record);
                continue;
            }
            if record.state == 3 {
                let mut app_lfsr = MacAppLfsrRng::new(MacAppLfsrRng::INITIAL_STATE);
                if app_lfsr.bounded(30) == 0 {
                    record.state = 4;
                    (record.target_x, record.target_y) = city.mac_kind3_boundary_target;
                }
                city.set_mac_object_slot_record(slot, record);
                continue;
            }
            if record.state == 4 {
                if mac_kind3_route_candidate(city, &record, record.lane) {
                    mac_kind3_move_one_step(city, slot, &mut record);
                } else {
                    let mut app_lfsr = MacAppLfsrRng::new(MacAppLfsrRng::INITIAL_STATE);
                    let start = usize::from(app_lfsr.bounded(2));
                    let mut direction = record.lane & 7;
                    for offset in MAC_KIND3_FORWARD_DIRECTION_WINDOW.iter().skip(start) {
                        direction = (i16::from(record.lane) + offset).rem_euclid(8) as u8;
                        if mac_kind3_route_candidate(city, &record, direction) {
                            break;
                        }
                    }
                    // The native state-4 search stores the last candidate even
                    // when every route fails and leaves phase 4 unchanged.
                    record.lane = direction;
                }
                city.set_mac_object_slot_record(slot, record);
            }
            // Native FUN_0000f970 has no phase arm above 4. Signed-negative
            // phase bytes also skip its nonnegative dispatch; every such visit
            // still performs the common counter and QuickDraw prelude above.
            continue;
        }
        if record.kind == 6 {
            // FUN_0000dd56 advances byte +1 for every kind-6 record, including
            // variants with +0x0b set. Its zero-countdown path plays selector
            // 0x1f8 before the first increment. The optional search follows
            // cleanup and is handled below for rooted export-247 cases.
            if record.lane <= 1 {
                if record.lane == 0 {
                    sound_requests.push(PlacementSound::Explode);
                }
                record.lane = record.lane.wrapping_add(1);
                city.set_mac_object_slot_record(slot, record);
                continue;
            }
            let x = usize::from(record.tile_x);
            let y = usize::from(record.tile_y);
            record.kind = 0;
            city.set_mac_object_slot_record(slot, record);
            if x < 128 && y < 128 {
                city.set_text_overlay(x, y, 0);
                city.mark_render_dirty(x, y);
            }
            if record.saved_b != 0
                && (!no_disasters_enabled_for_engine(city, EngineFidelity::Mac68k)
                    || city.mac_scenario_active())
            {
                // FUN_0000dd56 makes four pairs of CODE 13 export-203 calls
                // with bound 5. The recovered A5-2 state is zero and fixed, so
                // all candidates are the saved origin minus two on both axes.
                // Keep all eight draws in this separate app-LFSR stream.
                let mut app_lfsr = MacAppLfsrRng::new(MacAppLfsrRng::INITIAL_STATE);
                for _ in 0..4 {
                    let x_offset = app_lfsr.bounded(5) as i16 - 2;
                    let y_offset = app_lfsr.bounded(5) as i16 - 2;
                    let x = i16::from(record.tile_x) + x_offset;
                    let y = i16::from(record.tile_y) + y_offset;
                    if (0..128).contains(&x) && (0..128).contains(&y) {
                        let _ = mac_export247_candidate(
                            city,
                            x as usize,
                            y as usize,
                            false,
                            &mut app_lfsr,
                            quickdraw,
                        );
                    }
                }
                // The native success tail also writes the transient global
                // active point and, outside run state 2, queues the record's
                // selector. Those app-level globals/action dispatch remain
                // unmodeled; do not synthesize a partial queued action here.
            }
            continue;
        }
        if record.kind != XTHG_SAILBOAT {
            continue;
        }

        city.mac_marina_actor_counter = city.mac_marina_actor_counter.wrapping_add(1);
        // Mac CODE 6 indexes its map as tile_x * 128 + tile_y, matching the
        // x-major City planes. XTHG bytes +3/+4 therefore map directly to the
        // Rust record's tile_x/tile_y fields; do not apply the old transposing
        // rail adapter here.
        let old_x = usize::from(record.tile_x);
        let old_y = usize::from(record.tile_y);
        let mut app_lfsr = MacAppLfsrRng::new(MacAppLfsrRng::INITIAL_STATE);
        let remove = if city.mac_marina_actor_counter > 4 {
            true
        } else if record.state == 0 {
            // Export 203's zero-fixed-point stream admits this branch.
            let _ = app_lfsr.bounded(4);
            if !city
                .tile_bits_at(old_x, old_y)
                .is_some_and(|bits| bits.raw_bits() & 0x04 != 0)
            {
                true
            } else {
                let _ = app_lfsr.bounded(4000);
                record.state = 1;
                // The reconstructed Mac A5-2 stream is zero-fixed, so the
                // native second bound check succeeds on this activation arm.
                // CODE 6 requests SND 527 (Roar1) at the same transition.
                sound_requests.push(PlacementSound::MacSailboatRoar);
                record.lane = mac_kind9_turn(record.lane, quickdraw.random_u16());
                false
            }
        } else {
            // Every nonzero state takes export 203(bound=5); zero selects
            // export-121 cleanup in the reconstructed original state.
            app_lfsr.bounded(5) == 0
        };

        if remove {
            record.kind = 0;
            city.set_mac_object_slot_record(slot, record);
            if old_x < CITY_WIDTH && old_y < CITY_HEIGHT {
                city.set_text_overlay(old_x, old_y, 0);
            }
        } else {
            city.set_mac_object_slot_record(slot, record);
            city.mark_render_dirty(old_x, old_y);
        }
    }
    *tick_count = now;
    sound_requests
}

/// CODE 6 FUN_0000d02a's pre-switch redirect for XBLD above 0x70. The original
/// bypasses zone 8, uses the app-LFSR/status branch above 0xfa, and otherwise
/// compares unsigned +5 with the base-zoom sprite height divided by three.
/// This precedes the integrated kind-1 phase handlers.
fn mac_kind1_height_redirect(
    city: &mut City,
    record: &mut crate::win95_overlay::Win95OverlayRecord,
) -> bool {
    let x = usize::from(record.tile_x);
    let y = usize::from(record.tile_y);
    if x >= CITY_WIDTH || y >= CITY_HEIGHT {
        return false;
    }
    let building = city.building_at(x, y);
    if !(building > 0x70)
        || city
            .zone_at(x, y)
            .is_some_and(|zone| zone.raw_bits() & 0x0f == 8)
    {
        return false;
    }
    if building >= 0xfb {
        // CODE 6's A5+0x67a call is CODE 13 export 203, not the same-numbered
        // local Ghidra function. Its app-owned LFSR starts at zero and stays
        // zero, so bound 16 returns zero and +0x0b becomes 1.
        let mut app_lfsr = MacAppLfsrRng::new(MacAppLfsrRng::INITIAL_STATE);
        record.saved_b = u8::from(app_lfsr.bounded(16) == 0);
        record.state = 5;
        record.lane = 0;
        city.set_text_overlay(x, y, 6);
        city.mark_render_dirty(x, y);
        return true;
    }
    let Some(threshold) = crate::mac_shape_table::kind1_redirect_height_threshold(building) else {
        return false;
    };
    if record.height >= threshold {
        return false;
    }
    city.set_text_overlay(x, y, 6);
    city.mark_render_dirty(x, y);
    record.state = 5;
    record.saved_b = 1;
    record.lane = 0;
    true
}

/// CODE 6 `FUN_0000d02a` phases 0/1 and their shared `FUN_0000b4fa` mover.
/// The reconstructed A5 tables give kind 1 a stride of 16 and eight signed
/// cardinal/diagonal direction vectors. The shared mover uses strict `> 16`
/// and `< 0` accumulator carries, restores XTXT underlay before stepping, then
/// skips occupied actor-proxy cells along the carried direction.
fn mac_kind1_move_one_step(
    city: &mut City,
    slot: usize,
    record: &mut crate::win95_overlay::Win95OverlayRecord,
) {
    mac_move_one_step_with_stride(city, slot, record, 16);
}

/// CODE 6 tornado's first turn: two unsigned QuickDraw results are reduced
/// with DIVU.W 3, then combined with the original heading and masked to 3 bits.
fn mac_tornado_heading_mod3(heading: u8, first: u16, second: u16) -> u8 {
    (i16::from(heading) + 8 + (first % 3) as i16 - (second % 3) as i16) as u8 & 7
}

/// CODE 6 tornado's optional second turn uses the low bit of each result.
fn mac_tornado_heading_bit(heading: u8, first: u16, second: u16) -> u8 {
    (i16::from(heading) + 8 + (first & 1) as i16 - (second & 1) as i16) as u8 & 7
}

fn mac_move_one_step_with_heading(
    city: &mut City,
    slot: usize,
    record: &mut crate::win95_overlay::Win95OverlayRecord,
    stride: i16,
    heading: u8,
) {
    let saved_heading = record.lane;
    record.lane = heading;
    mac_move_one_step_with_stride(city, slot, record, stride);
    // FUN_0000b4fa uses the heading as an argument and does not persist it.
    record.lane = saved_heading;
}

/// CODE 6 export 121 clears the record kind and its current XTXT proxy byte;
/// it does not restore the saved underlay.
fn mac_export121_clear_actor(
    city: &mut City,
    slot: usize,
    record: &mut crate::win95_overlay::Win95OverlayRecord,
) {
    record.kind = 0;
    city.set_mac_object_slot_record(slot, *record);
    let x = usize::from(record.tile_x);
    let y = usize::from(record.tile_y);
    if x < CITY_WIDTH && y < CITY_HEIGHT {
        city.set_text_overlay(x, y, 0);
        city.mark_render_dirty(x, y);
    }
}

/// CODE 6 kind 5 checks the next tile before movement. A kind-14 XTXT proxy
/// changes the Monster to state 3 and defers movement until its next visit.
fn mac_kind5_candidate_is_kind14(
    city: &City,
    record: &crate::win95_overlay::Win95OverlayRecord,
    direction: u8,
) -> bool {
    let direction = usize::from(direction & 7);
    let x = i16::from(record.tile_x) + MAC_KIND1_MOVE_ROW_UNIT[direction];
    let y = i16::from(record.tile_y) + MAC_KIND1_MOVE_COL_UNIT[direction];
    if !(0..CITY_WIDTH as i16).contains(&x) || !(0..CITY_HEIGHT as i16).contains(&y) {
        return false;
    }
    let proxy = city.text_overlay_at(x as usize, y as usize).unwrap_or(0);
    let Some(target_slot) = crate::win95_overlay::xthg_slot_for_proxy(proxy) else {
        return false;
    };
    city.traffic_overlays()
        .and_then(|records| records.get(target_slot))
        .is_some_and(|target| target.kind == 14)
}

/// CODE 6 `FUN_0000b4fa` shared subtile mover. Kinds 1 and 2 supply distinct
/// native strides (16 and 8); accumulator carries and proxy-cell handling are
/// otherwise identical.
fn mac_move_one_step_with_stride(
    city: &mut City,
    slot: usize,
    record: &mut crate::win95_overlay::Win95OverlayRecord,
    stride: i16,
) {
    let direction = usize::from(record.lane & 7);
    let (sub_row, row_carry) =
        mac_kind1_advance_subtile(record.sub_x, MAC_KIND1_MOVE_ROW_UNIT[direction] * stride);
    let (sub_col, col_carry) =
        mac_kind1_advance_subtile(record.sub_y, MAC_KIND1_MOVE_COL_UNIT[direction] * stride);
    record.sub_x = sub_row;
    record.sub_y = sub_col;
    if row_carry == 0 && col_carry == 0 {
        return;
    }

    let old_x = i16::from(record.tile_x);
    let old_y = i16::from(record.tile_y);
    if (0..CITY_WIDTH as i16).contains(&old_x) && (0..CITY_HEIGHT as i16).contains(&old_y) {
        city.set_text_overlay(old_x as usize, old_y as usize, record.saved_a);
    }

    let mut next_x = old_x + row_carry;
    let mut next_y = old_y + col_carry;
    loop {
        if !(0..CITY_WIDTH as i16).contains(&next_x)
            || !(0..CITY_HEIGHT as i16).contains(&next_y)
        {
            // CODE 6 export 121 clears the current slot and its in-bounds
            // record-coordinate XTXT cell. Before the first collision that is
            // the original tile; after a collision it is the last proxy tile.
            if (0..CITY_WIDTH as i16).contains(&i16::from(record.tile_x))
                && (0..CITY_HEIGHT as i16).contains(&i16::from(record.tile_y))
            {
                city.set_text_overlay(usize::from(record.tile_x), usize::from(record.tile_y), 0);
            }
            record.kind = 0;
            return;
        }

        let x = next_x as usize;
        let y = next_y as usize;
        let overlay = city.text_overlay_at(x, y).unwrap_or(0);
        if overlay >= 0xc9 {
            // The native helper updates the record coordinate to this occupied
            // proxy before attempting the next carried cell.
            record.tile_x = next_x as u8;
            record.tile_y = next_y as u8;
            next_x += row_carry;
            next_y += col_carry;
            continue;
        }

        record.tile_x = next_x as u8;
        record.tile_y = next_y as u8;
        record.saved_a = overlay;
        city.set_text_overlay(x, y, 0xc9u8.wrapping_add(slot as u8));
        city.mark_render_dirty(x, y);
        break;
    }
    if (0..CITY_WIDTH as i16).contains(&old_x) && (0..CITY_HEIGHT as i16).contains(&old_y) {
        city.mark_render_dirty(old_x as usize, old_y as usize);
    }
}

/// Mac kind-2 state-2 retargeting around the persisted developed-map hotspot.
/// The native handler first tries `center + (Random & 0x3f) - 0x20`; an
/// out-of-map result consumes a second Random and substitutes `32 + (Random & 0x3f)`.
fn mac_kind2_random_target_axis(center: u16, quickdraw: &mut MacToolboxRng) -> u8 {
    let candidate = i32::from(center) + i32::from(quickdraw.random_u16() & 0x3f) - 0x20;
    if (0..CITY_WIDTH as i32).contains(&candidate) {
        candidate as u8
    } else {
        (32 + (quickdraw.random_u16() & 0x3f)) as u8
    }
}

fn mac_kind1_advance_subtile(value: u8, delta: i16) -> (u8, i16) {
    let mut next = i16::from(value) + delta;
    let carry = if next > 0x10 {
        next -= 0x10;
        1
    } else if next < 0 {
        next += 0x10;
        -1
    } else {
        0
    };
    (next as u8, carry)
}

/// CODE 6's `FUN_0000b2c6(5)` heading update. The first QuickDraw result is
/// interpreted as an unsigned word; a zero remainder alone spends the second
/// draw and applies `(Random % 3) - 1` around the eight-heading ring.
fn mac_kind1_perturb_heading(heading: u8, quickdraw: &mut MacToolboxRng) -> u8 {
    if quickdraw.random_u16() % 5 == 0 {
        (heading
            .wrapping_add((quickdraw.random_u16() % 3) as u8)
            .wrapping_sub(1))
            & 7
    } else {
        heading
    }
}

/// CODE 6 `FUN_0000b354`: choose the eight-way heading from row/column signs.
/// This intentionally differs from the slope-aware `FUN_0000b41e` helper used
/// by phase 4.
fn mac_kind1_sign_heading(record: &crate::win95_overlay::Win95OverlayRecord) -> u8 {
    let row_delta = i16::from(record.target_x) - i16::from(record.tile_x);
    let col_delta = i16::from(record.target_y) - i16::from(record.tile_y);
    match (row_delta.signum(), col_delta.signum()) {
        (-1, -1) => 7,
        (-1, 0) => 6,
        (-1, 1) => 5,
        (0, -1) => 0,
        (0, 0 | 1) => 4,
        (1, -1) => 1,
        (1, 0) => 2,
        (1, 1) => 3,
        _ => unreachable!("coordinate signum is in -1..=1"),
    }
}

/// CODE 6 export 112's Manhattan distance between current +3/+4 and target
/// +8/+9 coordinates. City coordinates are bounded to 0..128.
fn mac_kind1_target_distance(record: &crate::win95_overlay::Win95OverlayRecord) -> u16 {
    u16::from(record.tile_x.abs_diff(record.target_x))
        + u16::from(record.tile_y.abs_diff(record.target_y))
}

const MAC_KIND1_NEIGHBOR_ROW: [i16; 8] = [0, 3, 3, 3, 0, -3, -3, -3];
const MAC_KIND1_NEIGHBOR_COL: [i16; 8] = [-3, -3, 0, 3, 3, 3, 0, -3];
const MAC_KIND1_NEIGHBOR_TURNS: [u8; 7] = [1, 7, 2, 6, 3, 5, 4];

/// CODE 6 `FUN_0000dbc8`: rotate away from the first adjacent XBLD >= 0xfb.
/// Off-map candidates count as clear; if all eight headings are blocked the
/// final stored heading is the opposite direction.
fn mac_kind1_avoid_arcology_neighbor(
    city: &City,
    record: &mut crate::win95_overlay::Win95OverlayRecord,
) {
    let heading = usize::from(record.lane);
    if heading >= MAC_KIND1_NEIGHBOR_ROW.len()
        || !mac_kind1_neighbor_is_arcology(city, record, heading)
    {
        return;
    }

    let mut candidate = record.lane;
    for offset in MAC_KIND1_NEIGHBOR_TURNS {
        candidate = record.lane.wrapping_add(offset) & 7;
        if !mac_kind1_neighbor_is_arcology(city, record, usize::from(candidate)) {
            break;
        }
    }
    record.lane = candidate;
}

fn mac_kind1_neighbor_is_arcology(
    city: &City,
    record: &crate::win95_overlay::Win95OverlayRecord,
    heading: usize,
) -> bool {
    let x = i16::from(record.tile_x) + MAC_KIND1_NEIGHBOR_ROW[heading];
    let y = i16::from(record.tile_y) + MAC_KIND1_NEIGHBOR_COL[heading];
    if !(0..CITY_WIDTH as i16).contains(&x) || !(0..CITY_HEIGHT as i16).contains(&y) {
        return false;
    }
    city.building_at(x as usize, y as usize) >= 0xfb
}

/// CODE 6 `export_109` sound probes before the actor slot loop. Inputs are
/// the native report-history traffic/crime values and police-cell census.
fn mac_actor_sweep_prelude(
    traffic_history: u32,
    crime_history: u32,
    police_cells: u16,
    mut quickdraw: impl FnMut() -> u16,
) -> Vec<PlacementSound> {
    let mut requests = Vec::new();
    if traffic_history > 40 && quickdraw() & 0xff == 0 {
        requests.push(PlacementSound::Horns);
    }
    if police_cells != 0 && crime_history > 40 && quickdraw() & 0xff == 0 {
        requests.push(PlacementSound::Police);
    }
    requests
}

/// Read the two newest native XGRP report-history lanes and police census.
/// The four environment rows are published by the Mac phase-21 adapter.
fn mac_actor_sweep_history_inputs(city: &City) -> Option<(u32, u32, u16)> {
    const VALUES_PER_SERIES: usize = 52;
    const TRAFFIC_SERIES: usize = 4;
    const CRIME_SERIES: usize = 7;
    let span = (CRIME_SERIES - TRAFFIC_SERIES) * VALUES_PER_SERIES;
    let values = city.chunk_u32_values("XGRP", TRAFFIC_SERIES * VALUES_PER_SERIES * 4, span + 1);
    Some((
        *values.first()?,
        *values.get(span)?,
        city.mac_xbld_tile_count(0xd2),
    ))
}

/// CODE 6 `0xf6b8..0xf6d4` zero-extends QuickDraw's word before DIVU.W.
fn mac_kind9_turn(lane: u8, random_word: u16) -> u8 {
    (i16::from(lane) + (random_word % 3) as i16 - 1).rem_euclid(4) as u8
}

/// CODE 6 `0xf986` tests the low byte of the QuickDraw result for zero.
fn mac_kind3_sound_requested(random_word: u16) -> bool {
    random_word & 0xff == 0
}

/// Rooted export-247 effects for dry candidates in the empty, label, object,
/// rubble, and `0xfa` XTXT classes. `require_developed_xbld` distinguishes the
/// kind-5 option-zero caller from the kind-6 option-`0xff` caller.
/// The object class enters FUN_0003dc90's
/// zero-render export-80 removal, except the native `0x3f..=0x42` bypass.
/// Its `ffffceda` affected-class publication is still outside this actor port.
fn mac_export247_candidate(
    city: &mut City,
    x: usize,
    y: usize,
    require_developed_xbld: bool,
    app_lfsr: &mut MacAppLfsrRng,
    quickdraw: &mut MacToolboxRng,
) -> bool {
    if x >= CITY_WIDTH
        || y >= CITY_HEIGHT
        || (require_developed_xbld && city.building_at(x, y) < 6)
        || !city
            .tile_bits_at(x, y)
            .is_some_and(|bits| bits.raw_bits() & 0x04 == 0)
    {
        return false;
    }
    let Some(xtxt) = city.text_overlay_at(x, y) else {
        return false;
    };
    match xtxt {
        0 => {}
        1..=0x32 => city.mac_export_161_clear_xtxt_resources(xtxt),
        0x33..=0xc8 => {
            if !matches!(city.building_at(x, y), 0x3f..=0x42) {
                super::budget::mac_destroy_building_export80(city, x, y, quickdraw, false);
            }
        }
        0xf1..=0xf9 => {
            let rubble = app_lfsr.bounded(4) as u8 + 1;
            // CODE 5 export 105 writes XBLD and its Mac census but preserves
            // XTXT, XBIT, XZON, and the Mac half-resolution density plane.
            // This branch's native export-247 return is still zero: rubble
            // placement alone does not signal the caller's success bit.
            city.set_building(x, y, rubble);
            return false;
        }
        0xfa => {
            let building = city.building_at(x, y);
            city.set_text_overlay(x, y, 0);
            city.mac_export80_decrement_xtxt_fa_counter(building);
        }
        // The native export returns zero for actor-proxy bytes and for the
        // reserved high marker tail. XTXT 0xff is handled by its caller's
        // marker worker, not by this candidate helper.
        0xc9..=0xf0 | 0xfb..=0xff => return false,
    }
    city.set_text_overlay(x, y, 0xff);
    // Mac export 247 clears the corresponding ffffe740 half-resolution cell.
    city.set_traffic_density(x, y, 0);
    true
}

/// CODE 5 export 107 as reached by the Mac kind-5 state-1 status-3 arm:
/// PlaceTile `0xc8` at extent 1, restore XZON's high marker while preserving
/// its zone nibble, then provision the fixed XMIC row and its XTXT marker.
fn mac_kind5_place_c8(city: &mut City, x: usize, y: usize, quickdraw: &mut MacToolboxRng) {
    if x >= CITY_WIDTH || y >= CITY_HEIGHT {
        return;
    }
    let bits = city.tile_bits_at(x, y).unwrap_or_default();
    city.set_tile_bits(x, y, TileBits::new((bits.raw_bits() & 0x1f) | 0xe0));
    city.set_building(x, y, 0xc8);
    let zone = city.zone_at(x, y).map_or(0, |zone| zone.raw_bits());
    city.set_zone(x, y, (zone & 0x0f) | 0xf0);
    if let Some(text) = city.provision_mac_microsim_for_building(0xc8, quickdraw) {
        city.set_text_overlay(x, y, text);
    }
}

fn mac_kind3_turn_one_step(current: u8, desired: u8) -> u8 {
    let current = current & 7;
    let desired = desired & 7;
    if current == desired {
        return current;
    }
    if current > desired {
        if current - desired < 5 {
            current.wrapping_sub(1) & 7
        } else {
            current.wrapping_add(1) & 7
        }
    } else if desired - current < 5 {
        current.wrapping_add(1) & 7
    } else {
        current.wrapping_sub(1) & 7
    }
}

/// FUN_0000b30a turns even an aligned heading one octant clockwise: its
/// zero-difference case falls through the incrementing branch. Keep this
/// distinct from FUN_0000b3b6, which composes heading calculation and turn.
fn mac_kind3_heading_turn(current: u8, desired: u8) -> u8 {
    let current = current & 7;
    let desired = desired & 7;
    if current > desired {
        if current - desired < 5 {
            current.wrapping_sub(1) & 7
        } else {
            current.wrapping_add(1) & 7
        }
    } else if desired - current < 5 {
        current.wrapping_add(1) & 7
    } else {
        current.wrapping_sub(1) & 7
    }
}

fn mac_kind3_route_candidate(
    city: &City,
    record: &crate::win95_overlay::Win95OverlayRecord,
    direction: u8,
) -> bool {
    let direction = usize::from(direction & 7);
    let x = i16::from(record.tile_x) + MAC_KIND3_CANDIDATE_ROW_DELTA[direction];
    let y = i16::from(record.tile_y) + MAC_KIND3_CANDIDATE_COL_DELTA[direction];
    if !(0..128).contains(&x) || !(0..128).contains(&y) {
        // FUN_0000fd8a accepts an out-of-bounds direction as a terminal edge.
        return true;
    }
    let (x, y) = (x as usize, y as usize);
    if !city
        .tile_bits_at(x, y)
        .is_some_and(|bits| bits.raw_bits() & 0x04 != 0)
    {
        return false;
    }
    if !(0x10..=0x1f).contains(&city.terrain_at(x, y)) {
        return false;
    }
    let building = city.building_at(x, y);
    if building != 0 && !MAC_KIND3_ALLOWED_XBLD.contains(&building) {
        return false;
    }
    city.text_overlay_at(x, y).unwrap_or(0) < 0xc9
}

fn mac_kind3_move_one_step(
    city: &mut City,
    slot: usize,
    record: &mut crate::win95_overlay::Win95OverlayRecord,
) {
    let direction = usize::from(record.lane & 7);
    let (sub_row, carry_row) =
        mac_kind3_advance_subtile(record.sub_x, MAC_KIND3_MOVE_ROW_DELTA[direction]);
    let (sub_col, carry_col) =
        mac_kind3_advance_subtile(record.sub_y, MAC_KIND3_MOVE_COL_DELTA[direction]);
    record.sub_x = sub_row;
    record.sub_y = sub_col;
    if carry_row == 0 && carry_col == 0 {
        return;
    }

    let old_x = i16::from(record.tile_x);
    let old_y = i16::from(record.tile_y);
    city.set_text_overlay(old_x as usize, old_y as usize, record.saved_a);
    let next_x = old_x + carry_row;
    let next_y = old_y + carry_col;
    if !(0..128).contains(&next_x) || !(0..128).contains(&next_y) {
        // The native caller restores the old underlay before export 121 clears
        // both the slot kind and that same XTXT cell at the terminal edge.
        record.kind = 0;
        city.set_text_overlay(old_x as usize, old_y as usize, 0);
        return;
    }

    record.tile_x = next_x as u8;
    record.tile_y = next_y as u8;
    let x = next_x as usize;
    let y = next_y as usize;
    record.saved_a = city.text_overlay_at(x, y).unwrap_or(0);
    city.set_text_overlay(x, y, 0xc9u8.wrapping_add(slot as u8));
}

fn mac_kind3_advance_subtile(value: u8, delta: i16) -> (u8, i16) {
    let mut next = i16::from(value) + delta;
    let carry = if next > 0x10 {
        next -= 0x10;
        1
    } else if next < 0 {
        next += 0x10;
        -1
    } else {
        0
    };
    (next as u8, carry)
}

#[cfg(test)]
#[path = "mac_loop_scheduler/tests.rs"]
mod tests;
