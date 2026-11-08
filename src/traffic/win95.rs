//! Win95 executable traffic overlays and saved-XTRF interpretation.

use super::*;

/// CityGame's active road traffic path: Win95/XTRF overlays for every city-file format.
pub fn win95_traffic_overlay_at(
    city: &City,
    x: usize,
    y: usize,
    congestion: impl Fn(usize, usize) -> u8,
    rng: &mut crate::mac_random::MacToolboxRng,
) -> Option<TrafficOverlay> {
    traffic_overlay_stamp_at(city, x, y, congestion, rng)
}

fn road_overlay_uses_draw_tile(city: &City, x: usize, y: usize, building: u8) -> bool {
    // 1418..1426 / 1441..1449 are complete 64-pixel highway canvases. Their
    // 1097..1107 surfaces draw once per 2x2 footprint, so their traffic must use
    // that same visible corner rather than four translated copies of the mask.
    // The 1073..1080 family uses 32-pixel per-tile art and must not be deduplicated.
    !matches!(building, 97..=107) || city.building_should_draw_at(x, y)
}

pub(super) fn traffic_overlay_stamp_at(
    city: &City,
    x: usize,
    y: usize,
    congestion: impl Fn(usize, usize) -> u8,
    rng: &mut crate::mac_random::MacToolboxRng,
) -> Option<TrafficOverlay> {
    if x >= CITY_WIDTH || y >= CITY_HEIGHT {
        return None;
    }
    let rotation = city.rotation();
    let building = rotated_building_id(city.building_at(x, y), rotation);
    if !has_decorative_traffic(building) || !road_overlay_uses_draw_tile(city, x, y, building) {
        return None;
    }
    let density = traffic_density_for_tile(city, x, y, building, &congestion)?;
    let sprite_id = win95_pe_zone_or_terrain_sprite(city, x, y)
        .or_else(|| traffic_sprite_for_building_density(building, density))?;
    Some(TrafficOverlay {
        sprite_id,
        density,
        lane: 0,
        blit_flip: rng.random_bit(),
    })
}

fn traffic_density_for_tile(
    city: &City,
    x: usize,
    y: usize,
    building: u8,
    congestion: impl Fn(usize, usize) -> u8,
) -> Option<TrafficDensity> {
    let value = congestion(x, y);
    if let Some(density) = density_from_xtrf_win95_draw(value) {
        return Some(density);
    }
    if city.has_traffic_density_map() {
        return None;
    }
    if value == 0
        && is_spawnable_traffic_road(building)
        && traffic_heuristic_byte(x, y) >= PORT_TRAFFIC_ACTOR_SPAWN_THRESHOLD
    {
        return Some(TrafficDensity::Light);
    }
    None
}

/// Stand-in congestion byte when no `XTRF` chunk is loaded (Win95 spawn gate uses `0xaa`).
fn traffic_heuristic_byte(x: usize, y: usize) -> u8 {
    let mixed = (x.wrapping_mul(41) + y.wrapping_mul(59) + 17) as u8;
    mixed.saturating_add(0x80)
}

const WIN95_PE_ZONE_SPRITE_BASE: u32 = 0x50a;
const WIN95_PE_TERRAIN_SPRITE_BASE: u32 = 0x514;
const WIN95_PE_POSITION_ANGLE: [u16; 4] = [0x80, 0x10, 0x20, 0x40];

// SIMCITY.EXE 0x004E77B8, indexed by the low terrain class nibble.
const WIN95_PE_TERRAIN_CLASS_OFFSETS: [u8; 16] = [3, 0, 0, 1, 1, 2, 2, 4, 4, 4, 4, 4, 4, 4, 4, 4];

/// PE `FUN_0043C900` zone branch for per-tile traffic/decoration selection.
///
/// The zone branch wins when a zone low nibble is present (`0x50A + nibble`).
/// The separate terrain branch is mask-gated by the runtime word at `0x4C7D18`
/// and is intentionally not selected here until that BSS value has a source. The
/// caller selects the executable-era contract; save-file kind is not a gate.
pub fn win95_pe_zone_or_terrain_sprite(city: &City, x: usize, y: usize) -> Option<u32> {
    let zone_nibble = city
        .zone_at(x, y)
        .map(|zone| zone.raw_bits() & 0x0f)
        .unwrap_or(0);
    if zone_nibble != 0 {
        return Some(WIN95_PE_ZONE_SPRITE_BASE + u32::from(zone_nibble));
    }
    win95_pe_terrain_sprite_for_mask(
        win95_pe_terrain_mask_for_rotation(city.rotation()),
        city.terrain_at(x, y),
    )
}

/// PE writes `0x004C7D18 = word[0x004DC4C8 + rotation * 2]` during view setup.
pub fn win95_pe_terrain_mask_for_rotation(rotation: u8) -> u16 {
    WIN95_PE_POSITION_ANGLE[usize::from(rotation & 3)]
}

/// PE terrain alternate branch (`0x004C7D18 & (XTER & 0xF0) != 0`).
///
/// Callers must provide the runtime terrain mask explicitly; the current City
/// model does not contain a proven source for the PE BSS word.
pub fn win95_pe_terrain_sprite_for_mask(terrain_mask: u16, terrain: u8) -> Option<u32> {
    let terrain_high = u16::from(terrain & 0xf0);
    if terrain_high == 0 || terrain_mask & terrain_high == 0 {
        return None;
    }
    let terrain_class = usize::from(terrain & 0x0f);
    WIN95_PE_TERRAIN_CLASS_OFFSETS
        .get(terrain_class)
        .map(|offset| WIN95_PE_TERRAIN_SPRITE_BASE + u32::from(*offset))
}

#[cfg(test)]
#[path = "win95_pe_terrain_tests.rs"]
mod terrain_tests;
