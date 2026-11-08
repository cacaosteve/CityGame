//! Runtime congestion state for cities without a saved XTRF plane.

use super::*;

pub struct TrafficSystem {
    /// Heuristic congestion when the city file has no `XTRF` chunk.
    congestion: Vec<u8>,
    speed: GameSpeed,
    sim_accumulator: f32,
    sim_rng: crate::mac_random::MacToolboxRng,
}

impl TrafficSystem {
    pub fn new(city: &City) -> Self {
        let mut congestion = vec![0u8; CITY_TILES];
        if let Some(map) = city.traffic_density_slice() {
            congestion.copy_from_slice(map);
        }
        Self {
            congestion,
            speed: GameSpeed::Paused,
            sim_accumulator: 0.0,
            sim_rng: crate::mac_random::MacToolboxRng::new(0xC0FF_EE01),
        }
    }

    pub fn speed(&self) -> GameSpeed {
        self.speed
    }

    pub fn draw_rng(
        &self,
        city_name: &str,
        rotation: u8,
        redraw_epoch: u32,
    ) -> crate::mac_random::MacToolboxRng {
        crate::mac_random::MacToolboxRng::for_traffic_draw(city_name, rotation, redraw_epoch)
    }

    pub fn set_speed(&mut self, speed: GameSpeed) {
        self.speed = speed;
    }

    pub fn congestion_at(&self, x: usize, y: usize) -> u8 {
        self.congestion.get(city_index(x, y)).copied().unwrap_or(0)
    }

    pub fn congestion_slice(&self) -> &[u8] {
        &self.congestion
    }

    pub fn sync_from_city(&mut self, city: &City) {
        if let Some(map) = city.traffic_density_slice() {
            self.congestion.copy_from_slice(map);
        }
    }

    /// Mac `FUN_00002cb2` congestion bands.
    ///
    /// Saved `XTRF` is renderer input and is never mutated on tick. Only cities with no `XTRF`
    /// map synthesize congestion into the internal heuristic buffer; saved city data stays frozen.
    pub fn tick_traffic(&mut self, city: &mut City, delta_seconds: f32) {
        if !Self::should_simulate_congestion(city) {
            return;
        }
        let multiplier = self.speed.tick_multiplier();
        if multiplier <= 0.0 {
            return;
        }
        self.sim_accumulator += delta_seconds * multiplier * TRAFFIC_SIM_TICKS_PER_SECOND;
        while self.sim_accumulator >= 1.0 {
            self.sim_accumulator -= 1.0;
            self.tick_once(city);
        }
    }

    fn should_simulate_congestion(city: &City) -> bool {
        // Mac: saved XTRF is frozen; only synthesize congestion when the city has no XTRF map.
        !city.has_traffic_density_map()
    }

    fn tick_once(&mut self, city: &mut City) {
        let rotation = city.rotation();
        for y in 0..CITY_HEIGHT {
            for x in 0..CITY_WIDTH {
                let building = rotated_building_id(city.building_at(x, y), rotation);
                if !has_decorative_traffic(building) {
                    continue;
                }
                let index = city_index(x, y);
                let byte = self.congestion[index];
                let updated = step_congestion_byte(byte, &mut self.sim_rng);
                self.congestion[index] = updated;
            }
        }
    }
}

fn step_congestion_byte(byte: u8, rng: &mut crate::mac_random::MacToolboxRng) -> u8 {
    let updated = if byte < 0x80 {
        byte.saturating_add(3)
    } else if (MAC_TRAFFIC_VISIBLE_BYTE_MIN..MAC_TRAFFIC_HEAVY_BYTE_MIN).contains(&byte) {
        byte.saturating_add(2)
    } else if byte >= MAC_TRAFFIC_HEAVY_BYTE_MIN {
        byte.saturating_sub(2)
    } else {
        byte
    };
    if (PORT_TRAFFIC_ACTOR_SPAWN_THRESHOLD..MAC_TRAFFIC_HEAVY_BYTE_MIN).contains(&updated)
        && (rng.random_i16() as u8) < 0x80
    {
        updated.saturating_sub(0x37)
    } else {
        updated
    }
}

fn city_index(x: usize, y: usize) -> usize {
    x * CITY_HEIGHT + y
}
