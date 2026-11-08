//! Win95 ambient actor cadence and PE XTHG slot scheduler.
//!
//! The road-decoration traffic scheduler remains in the parent adapter.

use super::{
    City, GameSpeed, TrainEngineStep, WIN95_OVERLAY_SIM_TICKS_PER_SECOND, Win95CrtRand,
    Win95GameRand, Win95Lfsr, Win95OverlaySim, first_cargo_ship_position,
    pe_clear_ambient_xtxt_occupancy, pe_clear_xthg_kind, pe_train_consist_slots,
    pe_train_ed_neighbor_defers_tick, pe_train_merge_finish_explosion,
    pe_train_merge_tile_eligible, pe_zero_tile_xtxt, reverse_pe_train_consist, step_ambient_actor,
    step_ambient_train_car, step_ambient_train_engine, step_pe_cargo_actor_with_streams,
};
use crate::win95_overlay::{
    XTHG_AIRPLANE, XTHG_CARGO_SHIP, XTHG_FIRST_USABLE_SLOT, XTHG_HELICOPTER, XTHG_RECORD_COUNT,
    XTHG_SAILBOAT,
};

impl Win95OverlaySim {
    /// Step ambient planes / ships / trains / sailboats without the road-traffic path.
    ///
    /// Safe to call when Win95 road overlay sim is disabled (Mac-everywhere default).
    /// Train ED-neighbor defer shares live process LFSR+game-LCG.
    pub fn tick_ambient(
        &mut self,
        city: &mut City,
        speed: GameSpeed,
        delta_seconds: f32,
        crt: &mut Win95CrtRand,
        lfsr: &mut Win95Lfsr,
    ) -> bool {
        let mut game_rand = Win95GameRand::new(Win95GameRand::DEFAULT_STATE);
        self.tick_ambient_with_game_rand(city, speed, delta_seconds, crt, lfsr, &mut game_rand)
    }

    /// Ambient PE actors with all three process RNG streams kept distinct.
    pub fn tick_ambient_with_game_rand(
        &mut self,
        city: &mut City,
        speed: GameSpeed,
        delta_seconds: f32,
        crt: &mut Win95CrtRand,
        lfsr: &mut Win95Lfsr,
        game_rand: &mut Win95GameRand,
    ) -> bool {
        self.tick_ambient_with_game_rand_policy(
            city,
            speed,
            delta_seconds,
            crt,
            lfsr,
            game_rand,
            false,
        )
    }

    /// Ambient pass for the app's selected PE slot walk; extracted actors and
    /// train consists are left for their native slots to preserve shared draws.
    pub fn tick_ambient_with_game_rand_defer_xthg_slot_actors(
        &mut self,
        city: &mut City,
        speed: GameSpeed,
        delta_seconds: f32,
        crt: &mut Win95CrtRand,
        lfsr: &mut Win95Lfsr,
        game_rand: &mut Win95GameRand,
    ) -> bool {
        self.tick_ambient_with_game_rand_policy(
            city,
            speed,
            delta_seconds,
            crt,
            lfsr,
            game_rand,
            true,
        )
    }

    fn tick_ambient_with_game_rand_policy(
        &mut self,
        city: &mut City,
        speed: GameSpeed,
        delta_seconds: f32,
        crt: &mut Win95CrtRand,
        lfsr: &mut Win95Lfsr,
        game_rand: &mut Win95GameRand,
        defer_slot_actors: bool,
    ) -> bool {
        if !city.has_traffic_overlays() {
            return false;
        }
        let multiplier = speed.tick_multiplier();
        if multiplier <= 0.0 {
            return false;
        }
        self.ambient_tick_accumulator +=
            delta_seconds * multiplier * WIN95_OVERLAY_SIM_TICKS_PER_SECOND;
        let mut changed = false;
        while self.ambient_tick_accumulator >= 1.0 {
            self.ambient_tick_accumulator -= 1.0;
            changed |= self.tick_ambient_once_with_game_rand_policy(
                city,
                crt,
                lfsr,
                game_rand,
                defer_slot_actors,
            );
        }
        changed
    }
    pub(super) fn tick_ambient_once(
        &mut self,
        city: &mut City,
        crt: &mut Win95CrtRand,
        lfsr: &mut Win95Lfsr,
    ) -> bool {
        let mut game_rand = Win95GameRand::new(Win95GameRand::DEFAULT_STATE);
        self.tick_ambient_once_with_game_rand(city, crt, lfsr, &mut game_rand)
    }

    pub(super) fn tick_ambient_once_with_game_rand(
        &mut self,
        city: &mut City,
        crt: &mut Win95CrtRand,
        lfsr: &mut Win95Lfsr,
        game_rand: &mut Win95GameRand,
    ) -> bool {
        self.tick_ambient_once_with_game_rand_policy(city, crt, lfsr, game_rand, false)
    }

    pub(super) fn tick_ambient_once_with_game_rand_policy(
        &mut self,
        city: &mut City,
        crt: &mut Win95CrtRand,
        lfsr: &mut Win95Lfsr,
        game_rand: &mut Win95GameRand,
        defer_slot_actors: bool,
    ) -> bool {
        let table_snapshot = city
            .traffic_overlays()
            .map(|table| table.to_vec())
            .unwrap_or_default();
        let old_first_cargo_ship = first_cargo_ship_position(&table_snapshot);
        let mut changed = false;
        let mut handled_train_slots = [false; XTHG_RECORD_COUNT];
        // PE `0x4e99d0`: reset each overlay tick; sailboat handler incs and despawns when `>4`.
        city.pe_sailboat_budget = 0;
        for slot in XTHG_FIRST_USABLE_SLOT..XTHG_RECORD_COUNT {
            if handled_train_slots[slot] {
                continue;
            }
            let Some(record) = table_snapshot.get(slot).copied() else {
                continue;
            };
            if !record.is_ambient_port_actor() || record.is_pe_disaster_xthg_actor() {
                continue;
            }
            if defer_slot_actors
                && (matches!(
                    record.kind,
                    XTHG_AIRPLANE | XTHG_HELICOPTER | XTHG_CARGO_SHIP | XTHG_SAILBOAT
                ) || record.is_ambient_train_family())
            {
                continue;
            }
            let old_x = record.tile_x as usize;
            let old_y = record.tile_y as usize;
            let mut next = record;
            if record.is_ambient_train_engine() {
                let train_consist = pe_train_consist_slots(slot, &record, &table_snapshot);
                // PE `FUN_00455500`: kind-0x0A ED neighbor probe can defer the whole tick.
                if pe_train_ed_neighbor_defers_tick(&record, city, lfsr, game_rand) {
                    continue;
                }
                // PE `0x402E6E` success → clear consist + kind-6 @ `0x4557bb`.
                if pe_train_merge_tile_eligible(city, old_x, old_y) {
                    let mut clear_slots = vec![slot];
                    if let Some((first_slot, _, second_slot, _)) = train_consist {
                        clear_slots.push(first_slot);
                        clear_slots.push(second_slot);
                        handled_train_slots[first_slot] = true;
                        handled_train_slots[second_slot] = true;
                    }
                    let _ = pe_train_merge_finish_explosion(city, &clear_slots, old_x, old_y);
                    city.mark_render_dirty(old_x, old_y);
                    changed = true;
                    continue;
                }
                match step_ambient_train_engine(
                    &mut next,
                    city,
                    crt,
                    game_rand,
                    train_consist.map(|(_, _, tail_slot, _)| tail_slot),
                ) {
                    TrainEngineStep::Advanced => {
                        if let Some((first_slot, first_car, second_slot, second_car)) =
                            train_consist
                        {
                            next.saved_a = city.advance_ambient_train_xthg_proxies(
                                slot,
                                record,
                                first_slot,
                                first_car,
                                second_slot,
                                second_car,
                                next.tile_x as usize,
                                next.tile_y as usize,
                            );
                        } else if (old_x, old_y) != (next.tile_x as usize, next.tile_y as usize) {
                            next.saved_a = city.move_ambient_train_xthg_proxy(
                                slot,
                                old_x,
                                old_y,
                                record.saved_a,
                                next.tile_x as usize,
                                next.tile_y as usize,
                            );
                        }
                    }
                    TrainEngineStep::NeedsTurnaround => {
                        if let Some((first_slot, first_car, second_slot, second_car)) =
                            train_consist
                        {
                            let mut next_first_car = first_car;
                            let mut next_second_car = second_car;
                            let _ = step_ambient_train_car(&mut next_first_car, &table_snapshot);
                            let _ = step_ambient_train_car(&mut next_second_car, &table_snapshot);
                            next.saved_a = city.advance_ambient_train_xthg_proxies(
                                slot,
                                record,
                                first_slot,
                                first_car,
                                second_slot,
                                second_car,
                                next.tile_x as usize,
                                next.tile_y as usize,
                            );
                            reverse_pe_train_consist(&mut next, &mut next_second_car);
                            if let Some(overlays) = city.traffic_overlays_mut() {
                                overlays[slot] = next;
                                overlays[first_slot] = next_first_car;
                                overlays[second_slot] = next_second_car;
                            }
                            handled_train_slots[first_slot] = true;
                            handled_train_slots[second_slot] = true;
                            for old in [record, first_car, second_car] {
                                city.mark_render_dirty(old.tile_x as usize, old.tile_y as usize);
                            }
                            for new in [next, next_first_car, next_second_car] {
                                city.mark_render_dirty(new.tile_x as usize, new.tile_y as usize);
                            }
                            changed = true;
                            continue;
                        } else {
                            pe_clear_xthg_kind(&mut next);
                            // PE clear arms use `FUN_00401528` → zero XTXT, not `+A` restore.
                            pe_zero_tile_xtxt(city, old_x, old_y);
                        }
                    }
                    TrainEngineStep::Blocked => continue,
                    TrainEngineStep::Cleared => {}
                }
            } else if record.is_ambient_train_car() {
                if !step_ambient_train_car(&mut next, &table_snapshot) {
                    continue;
                }
            } else if record.kind == XTHG_CARGO_SHIP {
                if defer_slot_actors {
                    continue;
                }
                // PE FUN_00456E10 XTXT handoff is not gated on loaded chunks —
                // blank/new cities still paint `0x4CA600` occupancy.
                // FUN_00456760: CRT SFX gate then LFSR (`0x401cad`) state rolls.
                let (ok, play_sfx) =
                    step_pe_cargo_actor_with_streams(&mut next, city, crt, lfsr, slot);
                if play_sfx {
                    self.cargo_sfx_plays = self.cargo_sfx_plays.saturating_add(1);
                }
                if !ok {
                    continue;
                }
            } else if record.kind == XTHG_SAILBOAT {
                city.pe_sailboat_budget = city.pe_sailboat_budget.saturating_add(1);
                if city.pe_sailboat_budget > 4 {
                    // PE `FUN_004562e0` @ `0x4562e8`: after inc, count `>4` → `FUN_00401528`.
                    pe_clear_ambient_xtxt_occupancy(city, &next);
                    pe_clear_xthg_kind(&mut next);
                } else if !step_ambient_actor(&mut next, city, crt, lfsr, slot) {
                    continue;
                }
            } else if !step_ambient_actor(&mut next, city, crt, lfsr, slot) {
                continue;
            }
            if record.is_ambient_train_engine() && !next.is_ambient_train_engine() {
                // PE `FUN_00401528` on Cleared / failed turnaround: zero XTXT.
                pe_zero_tile_xtxt(city, old_x, old_y);
            }
            if let Some(overlays) = city.traffic_overlays_mut() {
                overlays[slot] = next;
            }
            city.mark_render_dirty(old_x, old_y);
            if next.kind != 0 {
                city.mark_render_dirty(next.tile_x as usize, next.tile_y as usize);
            }
            changed = true;
        }
        // PE rebuilds `wActiveTrains` while walking usable XTHG slots. Slot-zero
        // constructor anomalies are omitted from that next sweep's count.
        city.clear_pe_active_train_count_override();
        if changed {
            let new_first_cargo_ship = city
                .traffic_overlays()
                .map(first_cargo_ship_position)
                .flatten();
            if old_first_cargo_ship != new_first_cargo_ship {
                for position in [old_first_cargo_ship, new_first_cargo_ship]
                    .into_iter()
                    .flatten()
                {
                    city.mark_win95_pe_raising_bridge_render_dirty_near(position.0, position.1);
                }
            }
            city.mark_traffic_layers_dirty();
        }
        changed
    }
}
