use super::*;

/// Mac `export_231` type-0xF arm @ `0x35baa`: after the shared tax gate, place once at
/// `e4b4/e4b6` city center with `Random & 0x1F - 0x0F` on each axis (not PE's later
/// `0x45d8b0` multi-cloud `rand&7-4` scatter around a land-probe tile).
pub(in crate::app) fn mac_pollution_accident_target_tile<R: AmbientDisasterRandom>(
    city: &City,
    rng: &mut R,
) -> (usize, usize) {
    mac_city_center_rand_1f_minus_0f_tile(city, rng)
}

/// Mac `export_231` type-0x8 monster arm @ `0x35c98`: after pop `>= 0xAFC8`, same
/// city-center `Random & 0x1F - 0x0F` placement as pollution (PE still land-probes).
pub(in crate::app) fn mac_monster_target_tile<R: AmbientDisasterRandom>(
    city: &City,
    rng: &mut R,
) -> (usize, usize) {
    mac_city_center_rand_1f_minus_0f_tile(city, rng)
}

/// Mac disaster arms that jitter from `ffffe4b4`/`ffffe4b6`: `(Random & 0x1F) - 0x0F`.
pub(in crate::app) fn mac_city_center_rand_1f_minus_0f_tile<R: AmbientDisasterRandom>(
    city: &City,
    rng: &mut R,
) -> (usize, usize) {
    let (cx, cy) = mac_developed_tile_anchor(city);
    let x = (cx as isize + (rng.random_u16() & 0x1f) as isize - 0x0f)
        .clamp(0, CITY_WIDTH as isize - 1) as usize;
    let y = (cy as isize + (rng.random_u16() & 0x1f) as isize - 0x0f)
        .clamp(0, CITY_HEIGHT as isize - 1) as usize;
    (x, y)
}

pub(in crate::app) fn pollution_accident_ambient_target_tile<R: AmbientDisasterRandom>(
    city: &City,
    rng: &mut R,
    fidelity: EngineFidelity,
) -> (usize, usize) {
    if fidelity.uses_win95() {
        ambient_disaster_target_tile(city, rng)
    } else {
        mac_pollution_accident_target_tile(city, rng)
    }
}

pub(in crate::app) fn monster_ambient_target_tile<R: AmbientDisasterRandom>(
    city: &City,
    rng: &mut R,
    fidelity: EngineFidelity,
) -> (usize, usize) {
    if fidelity.uses_win95() {
        ambient_disaster_target_tile(city, rng)
    } else {
        mac_monster_target_tile(city, rng)
    }
}

/// Mac `export_231` type-0xD mass-riot arm @ `0x35a70`: after pop/unemp/heat gates,
/// place once at city center with `Random % 0x20 - 0x10` (same span as PE start
/// `rand&0x1F-0x10`, but Mac proposes from center rather than a land probe).
pub(in crate::app) fn mac_mass_riot_target_tile<R: AmbientDisasterRandom>(
    city: &City,
    rng: &mut R,
) -> (usize, usize) {
    mac_city_center_rand_mod20_minus_10_tile(city, rng)
}

/// Mac `export_231` type-3 rioters entry @ `0x35a7e`: skips the mass-riot pop gate,
/// then falls into the same city-center `Random % 0x20 - 0x10` placement as type 0xD.
pub(in crate::app) fn mac_rioters_target_tile<R: AmbientDisasterRandom>(
    city: &City,
    rng: &mut R,
) -> (usize, usize) {
    mac_city_center_rand_mod20_minus_10_tile(city, rng)
}

pub(in crate::app) fn mac_city_center_rand_mod20_minus_10_tile<R: AmbientDisasterRandom>(
    city: &City,
    rng: &mut R,
) -> (usize, usize) {
    let (cx, cy) = mac_developed_tile_anchor(city);
    let x = (cx as isize + toolbox_random_range(rng, 0x20) as isize - 0x10)
        .clamp(0, CITY_WIDTH as isize - 1) as usize;
    let y = (cy as isize + toolbox_random_range(rng, 0x20) as isize - 0x10)
        .clamp(0, CITY_HEIGHT as isize - 1) as usize;
    (x, y)
}

pub(in crate::app) fn mass_riot_ambient_target_tile<R: AmbientDisasterRandom>(
    city: &City,
    rng: &mut R,
    fidelity: EngineFidelity,
) -> (usize, usize) {
    if fidelity.uses_win95() {
        ambient_disaster_target_tile(city, rng)
    } else {
        mac_mass_riot_target_tile(city, rng)
    }
}

pub(in crate::app) fn rioters_ambient_target_tile<R: AmbientDisasterRandom>(
    city: &City,
    rng: &mut R,
    fidelity: EngineFidelity,
) -> (usize, usize) {
    if fidelity.uses_win95() {
        ambient_disaster_target_tile(city, rng)
    } else {
        mac_rioters_target_tile(city, rng)
    }
}

/// Mac `export_231` shared map pick: each axis is the zero-extended `_Random` word
/// `% 0x7E + 1` (tiles `1..=0x7E`). The assembly clears D1 before moving the random
/// result into D1w, so these coordinates use unsigned low-word modulo; this differs
/// from helper `0x67a`'s signed QuickDraw bounded-random path used by toxic spill.
/// Used by fire @ `0x359e6`, flood family @ `0x35a40`, earthquake @ `0x35c2e`, and
/// tornado @ `0x35c74` — not the PE land-probe helper and not city-center jitter.
pub(in crate::app) fn mac_random_interior_map_tile<R: AmbientDisasterRandom>(
    rng: &mut R,
) -> (usize, usize) {
    let x = usize::from(rng.random_u16()) % 0x7e + 1;
    let y = usize::from(rng.random_u16()) % 0x7e + 1;
    (x, y)
}

pub(in crate::app) fn mac_mod7e_plus_one_ambient_target_tile<R: AmbientDisasterRandom>(
    city: &City,
    rng: &mut R,
    fidelity: EngineFidelity,
) -> (usize, usize) {
    if fidelity.uses_win95() {
        ambient_disaster_target_tile(city, rng)
    } else {
        let _ = city;
        mac_random_interior_map_tile(rng)
    }
}

/// Mac weather-first hurricane @ `0x3594a` and jump-table type-0x10 @ `0x35cf2`:
/// weather+ocean gates only — no Random / no `a880` tile. Start ignores the tile
/// (`seed_pe_hurricane`). Win95 keeps the shared land-probe placeholder.
pub(in crate::app) fn hurricane_ambient_target_tile<R: AmbientDisasterRandom>(
    city: &City,
    rng: &mut R,
    fidelity: EngineFidelity,
) -> (usize, usize) {
    if fidelity.uses_win95() {
        ambient_disaster_target_tile(city, rng)
    } else {
        let _ = (city, rng);
        (CITY_WIDTH / 2, CITY_HEIGHT / 2)
    }
}

/// Both executables draw two interior coordinates for Tornado. The PE pushes
/// Y before X into its point setter at `0x42b4d0`.
pub(in crate::app) fn tornado_ambient_target_tile<R: AmbientDisasterRandom>(
    rng: &mut R,
    fidelity: EngineFidelity,
) -> (usize, usize) {
    let (first, second) = mac_random_interior_map_tile(rng);
    if fidelity.uses_win95() {
        (second, first)
    } else {
        (first, second)
    }
}

/// Mac hurricane sets the type alone. Win95 hurricane retains the land probe.
pub(in crate::app) fn weather_disaster_ambient_target_tile<R: AmbientDisasterRandom>(
    city: &City,
    rng: &mut R,
    fidelity: EngineFidelity,
    kind: DisasterKind,
) -> (usize, usize) {
    match kind {
        DisasterKind::Hurricane => hurricane_ambient_target_tile(city, rng, fidelity),
        DisasterKind::Tornado => tornado_ambient_target_tile(rng, fidelity),
        _ => ambient_disaster_target_tile(city, rng),
    }
}
