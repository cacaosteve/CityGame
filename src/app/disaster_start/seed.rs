use super::*;

impl ViewerState {
    pub(in crate::app) fn seed_disaster_footprint(
        &mut self,
        kind: DisasterKind,
        x: usize,
        y: usize,
    ) -> usize {
        let was_active = self.city.has_active_disaster();
        let (x, y) = if kind == DisasterKind::Meltdown {
            let Some(tile) = pe_meltdown_plant_tile(&self.city, (x, y)) else {
                return 0;
            };
            tile
        } else {
            (x, y)
        };
        let mut seeded = 0;
        if kind == DisasterKind::PollutionAccident {
            seeded = self.seed_pe_pollution_accident_clouds(x, y);
        } else if kind == DisasterKind::Volcano {
            seeded = self.seed_pe_volcano_scatter(x, y);
        } else if kind == DisasterKind::Microwave {
            seeded = self.seed_pe_microwave_walk();
        } else if kind == DisasterKind::Firestorm {
            seeded = self.seed_pe_firestorm_spiral(x, y);
        } else if kind == DisasterKind::Meltdown {
            seeded = self.seed_pe_meltdown_scatter(x, y);
        } else if kind == DisasterKind::Rioters {
            seeded = self.seed_pe_rioters(x, y);
        } else if kind == DisasterKind::MassRiot {
            seeded = self.seed_pe_mass_riot(x, y);
        } else if kind == DisasterKind::Hurricane {
            seeded = self.seed_pe_hurricane();
        } else if kind == DisasterKind::Flood {
            seeded = self.seed_pe_flood(x, y);
        } else if kind == DisasterKind::BlizzardFlood {
            seeded = self.seed_pe_blizzard_flood(x, y);
        } else if kind == DisasterKind::Fire {
            seeded = self.seed_pe_fire();
        } else if kind == DisasterKind::Earthquake {
            seeded = self.seed_pe_earthquake(x, y);
        } else if kind == DisasterKind::Tornado {
            seeded = self.seed_pe_tornado(x, y);
        } else if kind == DisasterKind::Monster {
            seeded = self.seed_pe_monster(x, y);
        } else if kind == DisasterKind::AirCrash {
            seeded = self.seed_pe_aircrash();
        } else {
            for (step, &(dx, dy)) in disaster_footprint_for_city(&self.city, kind)
                .iter()
                .enumerate()
            {
                if let Some((tile_x, tile_y)) = offset_city_tile(x, y, dx, dy) {
                    self.city.set_text_overlay(
                        tile_x,
                        tile_y,
                        disaster_marker_for_step(kind, step),
                    );
                    seeded += 1;
                }
            }
        }
        persist_pe_disaster_misc(&mut self.city, Some(kind), self.mode, self.engine_fidelity);
        if !was_active {
            self.reduce_fastest_speed_on_pe_disaster_entry();
        }
        // PE `0x45d066` after siren: DECLINED + RandomAnd3 → sole `FUN_00452FA0` caller.
        let _ = self.tick_pe_maxis_man_spawn();
        seeded
    }

    /// PE `0x45d066`→`0x45d080` / `FUN_00452FA0` Maxis Man constructor boundary.
    ///
    /// Admission (disaster-start only in PE): `0x4cc4e4 == 1` (military DECLINED), then
    /// `0x402261` RandomAnd3 (`0x4cdb7c` LFSR) must be 0. Sole PE caller is `0x45d080`.
    ///
    /// CRT order inside the ctor: spawn-tile `rand()&3` ±0x10 jitter, then (no CRT)
    /// `0x402dbf` direction from jittered tile toward the event tile, then IDs `3|13`
    /// `rand()&1` for `0xFD`/`0xFE`. IDs `5|8..=12` pass through
    /// `wMonsterXTHGIndex` (`0x4CA81C`) low byte (`0xFF` when cleared).
    pub(in crate::app) fn tick_pe_maxis_man_spawn(&mut self) -> usize {
        if !self.city.has_active_disaster() {
            return 0;
        }
        // PE `0x45d066`: `cmp byte [0x4cc4e4], 1` before RandomAnd3 — no LFSR on refuse.
        if military_base_type_for_city(&self.city) != MILITARY_BASE_DECLINED {
            return 0;
        }
        // PE `0x45d06f` → `0x402261` → `0x47fb70`: admit only when `(LFSR & 3) == 0`.
        if self.pe_process_lfsr.random_mask_03() != 0 {
            return 0;
        }
        let Some(disaster_id) = u8::try_from(self.city.pe_w_current_disaster_type()).ok() else {
            return 0;
        };
        let (event_x, event_y) = self.disaster_target_tile();
        if event_x >= CITY_WIDTH || event_y >= CITY_HEIGHT {
            return 0;
        }
        let (spawn_x, spawn_y) =
            pe_maxis_man_spawn_tile_jitter(event_x, event_y, &mut self.pe_process_crt);
        let direction = pe_xthg_direction_toward(
            spawn_x as i16,
            spawn_y as i16,
            event_x as i16,
            event_y as i16,
        );
        let random_low_bit = if matches!(disaster_id, 3 | 13) {
            (self.pe_process_crt.random_u16() & 1) as u8
        } else {
            0
        };
        // PE `wMonsterXTHGIndex` @ `0x4CA81C`: active monster slot, or `0xFFFF`
        // when cleared (`FUN_0045d144`) so the low byte read at `0x453151` is `0xFF`.
        let global_payload = self.city.pe_w_monster_xthg_index() as u8;
        let Some(saved_payload) =
            pe_maxis_man_saved_payload(disaster_id, random_low_bit, global_payload)
        else {
            return 0;
        };
        let inputs = PeMaxisManSpawnInputs {
            tile_x: spawn_x.min(255) as u8,
            tile_y: spawn_y.min(255) as u8,
            direction,
            altm: self.city.altitude_at(spawn_x, spawn_y),
            target_x: event_x.min(255) as u8,
            target_y: event_y.min(255) as u8,
            // PE `FUN_00452FA0` @ `0x4530d9`: save spawn-tile XTXT (`0x4CA600`) to `+A`.
            saved_xtxt: self.city.text_overlay_at(spawn_x, spawn_y).unwrap_or(0),
            saved_payload,
        };
        let Some(overlays) = self.city.traffic_overlays_mut() else {
            return 0;
        };
        let Some(slot) = spawn_pe_maxis_man(overlays, inputs) else {
            return 0;
        };
        if let Some(proxy) = xthg_proxy_for_slot(slot) {
            self.city.set_text_overlay(spawn_x, spawn_y, proxy);
        }
        // PE `0x45d230` epilogue after a successful constructor path.
        self.city.pe_maxis_daily_clear_dispatch_xtxt_proxies();
        self.city.mark_all_dirty();
        1
    }

    /// Win95 type-7 start @ `0x45f090`: one XTHG tornado + XTXT `slot-0x37`.
    pub(in crate::app) fn seed_pe_tornado(&mut self, x: usize, y: usize) -> usize {
        let (lane, height, target_x, target_y, saved_a) =
            pe_tornado_xthg_fields_with_rng(&self.city, x, y, &mut self.pe_process_crt);
        self.city
            .place_pe_tornado_actor(x, y, lane, height, target_x, target_y, saved_a)
            .map(|_| 1)
            .unwrap_or(0)
    }

    /// Win95 type-8 start @ `0x45ee10`: one XTHG monster + XTXT `slot-0x37`.
    pub(in crate::app) fn seed_pe_monster(&mut self, x: usize, y: usize) -> usize {
        let (target_x, target_y, saved_a, saved_b) =
            pe_monster_xthg_fields_with_rng(&self.city, x, y, &mut self.pe_process_crt);
        self.city
            .place_pe_monster_actor(x, y, target_x, target_y, saved_a, saved_b)
            .map(|_| 1)
            .unwrap_or(0)
    }

    /// Win95 type-0x12 start @ `0x45e7d0`: random inner empty XTXT, airplane + proxy.
    pub(in crate::app) fn seed_pe_aircrash(&mut self) -> usize {
        let Some((x, y)) = pe_aircrash_tile_with_lfsr(&self.city, &mut self.pe_process_lfsr) else {
            return 0;
        };
        let placed = self
            .city
            .place_pe_aircrash_actor(x, y)
            .map(|_| 1)
            .unwrap_or(0);
        if placed > 0 && self.auto_goto_enabled {
            self.pending_disaster_auto_goto = Some((x, y));
        }
        placed
    }

    /// Win95 type-0xF start @ `0x45d8b0`: `dwCityPopulation/0x2710+5` XTXT `0xfb` clouds, `rand&7-4`.
    pub(in crate::app) fn seed_pe_pollution_accident_clouds(
        &mut self,
        x: usize,
        y: usize,
    ) -> usize {
        let clouds = pe_pollution_accident_cloud_tiles_with_rng(
            &mut self.city,
            x,
            y,
            &mut self.pe_process_crt,
        );
        // Markers already painted live during scatter; re-stamp is idempotent.
        for &(tile_x, tile_y) in &clouds {
            self.city.set_text_overlay(tile_x, tile_y, 0xfb);
        }
        clouds.len()
    }

    /// Win95 type-0xE start @ `0x45d9d0`: `dwCityPopulation/0x2710+5` type-2 floods, `rand&0x1f-0x10`.
    pub(in crate::app) fn seed_pe_blizzard_flood(&mut self, x: usize, y: usize) -> usize {
        let overlays = pe_blizzard_flood_overlays_with_rng(
            &mut self.city,
            x,
            y,
            &mut self.pe_process_crt,
            &mut self.pe_process_lfsr,
        );
        if !overlays.is_empty() {
            self.pe_disaster_duration = PE_FLOOD_HURRICANE_DURATION;
        }
        // Markers already painted live during nested type-2 calls; re-stamp is idempotent.
        for (&(tile_x, tile_y), &marker) in &overlays {
            self.city.set_text_overlay(tile_x, tile_y, marker);
        }
        overlays.len()
    }

    /// Win95 type-2 start @ `0x45db00`: XTER `0x20..0x30` neighbors, else random XTER `0`.
    pub(in crate::app) fn seed_pe_flood(&mut self, x: usize, y: usize) -> usize {
        // PE @ `0x45db00`: type-2 paints `0xfc` on the map live during the start.
        let overlays = pe_flood_paint_with_lfsr(&mut self.city, x, y, &mut self.pe_process_lfsr);
        if !overlays.is_empty() {
            self.pe_disaster_duration = PE_FLOOD_HURRICANE_DURATION;
        }
        // Markers already painted live; re-stamp is idempotent.
        for (&(tile_x, tile_y), &marker) in &overlays {
            self.city.set_text_overlay(tile_x, tile_y, marker);
        }
        if self.auto_goto_enabled {
            if let Some(source) = pe_flood_source_tile(&self.city, x, y) {
                self.pending_disaster_auto_goto = Some(source);
            } else if let Some((&tile, _)) = overlays.iter().next() {
                self.pending_disaster_auto_goto = Some(tile);
            }
        }
        overlays.len()
    }

    /// Win95 type-0x10 start @ `0x45d4e0`: destroy XBLD `>= 0x0D`, then coast XTXT `0xfc`.
    pub(in crate::app) fn seed_pe_hurricane(&mut self) -> usize {
        self.pe_disaster_duration = PE_FLOOD_HURRICANE_DURATION;
        self.pe_hurricane_wind_remaining = PE_HURRICANE_WIND_REMAINING;
        let (overlays, hits) = pe_hurricane_overlays_with_lfsr(
            &mut self.city,
            &mut self.pe_process_crt,
            &mut self.pe_process_lfsr,
        );
        let arm = pe_hurricane_arm(self.city.rotation());
        if pe_hurricane_destroy_extra_explode(arm) {
            self.queue_disaster_extra_sounds(PlacementSound::Explode, hits);
        }
        for (&(tile_x, tile_y), &marker) in &overlays {
            self.city.set_text_overlay(tile_x, tile_y, marker);
        }
        overlays.len()
    }

    /// Win95 type-0xD start @ `0x45e180`: `dwCityPopulation/0x2710+5` type-3 riots, `rand&0x1f-0x10`.
    pub(in crate::app) fn seed_pe_mass_riot(&mut self, x: usize, y: usize) -> usize {
        let (overlays, hits) =
            pe_mass_riot_start_with_rng(&mut self.city, x, y, &mut self.pe_process_crt);
        self.queue_disaster_extra_sounds(PlacementSound::Riot, hits.saturating_sub(1));
        // Markers already painted live during type-3 calls; re-stamp is idempotent.
        for (&(tile_x, tile_y), &marker) in &overlays {
            self.city.set_text_overlay(tile_x, tile_y, marker);
        }
        overlays.len()
    }

    /// Win95 type-3 start @ `0x45e5f0`: NESW spiral to one transit tile, `0xfd`/`0xfe`.
    pub(in crate::app) fn seed_pe_rioters(&mut self, x: usize, y: usize) -> usize {
        let Some((tile_x, tile_y)) = pe_rioters_target_tile(&self.city, x, y) else {
            return 0;
        };
        let marker = pe_rioters_marker(&mut self.pe_process_crt);
        self.city.set_text_overlay(tile_x, tile_y, marker);
        1
    }

    /// Win95 type-9 start @ `0x45f2b0`: destroy nuclear footprint, then ±32 1-in-32 scatter.
    pub(in crate::app) fn seed_pe_meltdown_scatter(&mut self, x: usize, y: usize) -> usize {
        let overlays = pe_meltdown_overlays_with_rng(
            &mut self.city,
            x,
            y,
            &mut self.pe_process_crt,
            &mut self.pe_process_lfsr,
        );
        for (&(tile_x, tile_y), &marker) in &overlays {
            self.city.set_text_overlay(tile_x, tile_y, marker);
        }
        overlays.len()
    }

    /// Win95 type-6 start @ `0x45eb20`: ±32 1-in-64 ignite or `0x401604` destroy XBLD `>= 0x0E`.
    pub(in crate::app) fn seed_pe_earthquake(&mut self, x: usize, y: usize) -> usize {
        self.earthquake_shake_remaining = PE_EARTHQUAKE_SHAKE_ITERS as u8;
        self.queue_disaster_extra_sounds(
            PlacementSound::Explode,
            PE_EARTHQUAKE_SHAKE_ITERS.saturating_sub(1),
        );
        let overlays = pe_earthquake_overlays_with_rng(
            &mut self.city,
            x,
            y,
            &mut self.pe_process_crt,
            &mut self.pe_process_lfsr,
        );
        for (&(tile_x, tile_y), &marker) in &overlays {
            self.city.set_text_overlay(tile_x, tile_y, marker);
        }
        overlays.len()
    }

    /// Win95 type-1 start @ `0x45e920`: CRT center jitter spiral (XBLD `>= 0x70` + FUN flag 0),
    /// then RandomAnd7F fallback calling FUN until paint success or `0xc8` attempts.
    pub(in crate::app) fn seed_pe_fire(&mut self) -> usize {
        let (cx, cy) = land_value_city_center(&self.city);
        let mut x = cx as isize + pe_fire_center_jitter(&mut self.pe_process_crt);
        let mut y = cy as isize + pe_fire_center_jitter(&mut self.pe_process_crt);
        let mut dir = 0_usize;
        let mut arm_len = 1_isize;
        let mut steps_on_arm = 0_isize;
        while arm_len < PE_FIRE_SPIRAL_ARM_LIMIT {
            x += PE_FIRESTORM_DX[dir];
            y += PE_FIRESTORM_DY[dir];
            if (0..CITY_WIDTH as isize).contains(&x) && (0..CITY_HEIGHT as isize).contains(&y) {
                let (tx, ty) = (x as usize, y as usize);
                // PE @ `0x45e9aa`: XBLD `>= 0x70` then `push 0; call 0x401bf4`.
                if self.city.building_at(tx, ty) >= 0x70
                    && pe_fun_00460e10_ignite(
                        &mut self.city,
                        Some(&mut self.pe_process_crt),
                        &mut self.pe_process_lfsr,
                        tx,
                        ty,
                        0,
                    )
                {
                    if self.auto_goto_enabled {
                        self.pending_disaster_auto_goto = Some((tx, ty));
                    }
                    return 1;
                }
            }
            steps_on_arm += 1;
            if steps_on_arm >= arm_len {
                steps_on_arm = 0;
                if dir & 1 != 0 {
                    arm_len += 1;
                }
                dir = (dir + 1) & 3;
            }
        }
        // PE @ `0x45e9e9`: RandomAnd7F + FUN flag 0 until success or `0xc8` attempts.
        for _ in 0..PE_FIRE_FALLBACK_ATTEMPTS {
            let tx = usize::from(self.pe_process_lfsr.random_mask_7f());
            let ty = usize::from(self.pe_process_lfsr.random_mask_7f());
            if tx < CITY_WIDTH
                && ty < CITY_HEIGHT
                && pe_fun_00460e10_ignite(
                    &mut self.city,
                    Some(&mut self.pe_process_crt),
                    &mut self.pe_process_lfsr,
                    tx,
                    ty,
                    0,
                )
            {
                if self.auto_goto_enabled {
                    self.pending_disaster_auto_goto = Some((tx, ty));
                }
                return 1;
            }
        }
        0
    }

    /// Win95 firestorm start @ `0x45e030`: NESW spiral calls FUN_00460e10 flag 1 on every
    /// in-bounds step; only paint successes decrement the `0x41` budget (PE `test eax`).
    /// Each success also updates the auto-goto tile (`0x40174e` → last paint).
    pub(in crate::app) fn seed_pe_firestorm_spiral(&mut self, x: usize, y: usize) -> usize {
        let mut bp = x as isize;
        let mut si = y as isize;
        let mut dir = 0_usize;
        let mut arm_len = 1_isize;
        let mut steps_on_arm = 0_isize;
        let mut remaining = PE_FIRESTORM_PAINT_COUNT;
        let mut painted = 0usize;
        let mut last_success = None;
        while remaining > 0 && arm_len < 0x80 {
            bp += PE_FIRESTORM_DX[dir];
            si += PE_FIRESTORM_DY[dir];
            if (0..CITY_WIDTH as isize).contains(&bp) && (0..CITY_HEIGHT as isize).contains(&si) {
                // PE @ `0x45e0a4`: `push 1; call 0x401bf4` — no outer water/XTXT prefilter.
                if pe_fun_00460e10_ignite(
                    &mut self.city,
                    Some(&mut self.pe_process_crt),
                    &mut self.pe_process_lfsr,
                    bp as usize,
                    si as usize,
                    1,
                ) {
                    remaining -= 1;
                    painted += 1;
                    last_success = Some((bp as usize, si as usize));
                }
            }
            steps_on_arm += 1;
            if steps_on_arm >= arm_len {
                steps_on_arm = 0;
                if dir & 1 != 0 {
                    arm_len += 1;
                }
                dir = (dir + 1) & 3;
            }
        }
        if self.auto_goto_enabled {
            // PE @ `0x45e10a`: auto-goto uses the spiral cursor after the last successful paint.
            self.pending_disaster_auto_goto = last_success;
        }
        painted
    }

    /// Win95 type-0xA start @ `0x45f5c0`: 0x27-step process CRT `rand&7` walk from XBLD `0xcd`.
    pub(in crate::app) fn seed_pe_microwave_walk(&mut self) -> usize {
        let Some(walk) = pe_microwave_walk_with_rng(
            &mut self.city,
            &mut self.pe_process_crt,
            &mut self.pe_process_lfsr,
        ) else {
            return 0;
        };
        let extra = walk.zap_count.saturating_sub(1);
        self.queue_disaster_extra_sounds(PlacementSound::PowerLine, extra);
        for (&(tile_x, tile_y), &marker) in &walk.overlays {
            // Land `0xff` already applied by FUN_00460e10; water `0xfb` still needs stamp.
            self.city.set_text_overlay(tile_x, tile_y, marker);
        }
        if self.auto_goto_enabled {
            self.pending_disaster_auto_goto = Some(walk.periodic_goto);
        }
        walk.overlays.len()
    }

    /// Win95 volcano start @ `0x45e2a0`: dummy-funds near/far scatter plus flatten.
    pub(in crate::app) fn seed_pe_volcano_scatter(&mut self, x: usize, y: usize) -> usize {
        let (overlays, explodes) =
            pe_volcano_scatter_with_rng(&mut self.city, x, y, &mut self.pe_process_crt);
        self.queue_disaster_extra_sounds(PlacementSound::Explode, explodes.saturating_sub(1));
        overlays.len()
    }
}
