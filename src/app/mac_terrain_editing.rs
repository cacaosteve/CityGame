//! Mac CODE 5 height-drag arithmetic and ordered slope/classifier calls.

use crate::city::{CITY_HEIGHT, CITY_WIDTH, City, TileBits};
use crate::mac_random::MacToolboxRng;

/// CODE 5 export 84. Failed validation leaves any already-written marks in
/// place; an existing mark succeeds after the center's military-zone check.
#[allow(dead_code)]
pub(super) fn mac_mark_terrain_raise(city: &mut City, x: usize, y: usize) -> bool {
    if x >= CITY_WIDTH || y >= CITY_HEIGHT { return true; }
    if city.zone_at(x, y).is_some_and(|z| z.zone_id() == 7) { return false; }
    let bits = city.tile_bits_at(x, y).unwrap_or(TileBits::new(0));
    if bits.flatten_mark() { return true; }
    city.set_tile_bits(x, y, bits.with_flatten_mark(true));
    let land = city.altitude_at(x, y);
    for (dx, dy) in NEIGHBORHOOD.iter().copied().take(8) {
        if let (Some(nx), Some(ny)) = (x.checked_add_signed(dx), y.checked_add_signed(dy))
            && nx < CITY_WIDTH && ny < CITY_HEIGHT
            && city.zone_at(nx, ny).is_some_and(|z| z.zone_id() == 7)
        { return false; }
    }
    for (dx, dy) in [(-1, 0), (0, -1), (1, 0), (0, 1)] {
        if let (Some(nx), Some(ny)) = (x.checked_add_signed(dx), y.checked_add_signed(dy))
            && nx < CITY_WIDTH && ny < CITY_HEIGHT && city.altitude_at(nx, ny) < land
            && !mac_mark_terrain_raise(city, nx, ny)
        { return false; }
    }
    true
}

/// CODE 5 export 85. Its WORD OR allows height 31+1 to spill into water.
#[allow(dead_code)]
pub(super) fn mac_apply_marked_terrain_raise(
    city: &mut City, x: usize, y: usize, funds: &mut i32, rng: &mut MacToolboxRng,
) {
    mac_apply_marked_terrain_raise_with_destroy(city, x, y, funds, rng, &mut |city, x, y, rng| {
        super::budget::mac_destroy_building_export80(city, x, y, rng, true)
    });
}

/// Variant for callers that own the native destruction presentation tail.
pub(super) fn mac_apply_marked_terrain_raise_with_destroy(
    city: &mut City, x: usize, y: usize, funds: &mut i32, rng: &mut MacToolboxRng,
    destroy: &mut impl FnMut(&mut City, usize, usize, &mut MacToolboxRng) -> u32,
) {
    mac_apply_marked_terrain_raise_with_callbacks(city, x, y, funds, rng, destroy, &mut |_, _, _| {});
}

/// The original neighborhood walk marks tile bounds before and after each
/// classifier call; presentation state and shape dimensions belong to caller.
pub(super) fn mac_apply_marked_terrain_raise_with_callbacks(
    city: &mut City, x: usize, y: usize, funds: &mut i32, rng: &mut MacToolboxRng,
    destroy: &mut impl FnMut(&mut City, usize, usize, &mut MacToolboxRng) -> u32,
    mark_bounds: &mut impl FnMut(&City, usize, usize),
) {
    if x >= CITY_WIDTH || y >= CITY_HEIGHT { return; }
    let bits = city.tile_bits_at(x, y).unwrap_or(TileBits::new(0));
    if !bits.flatten_mark() { return; }
    city.set_tile_bits(x, y, bits.with_flatten_mark(false));
    let land = city.altitude_at(x, y);
    for (dx, dy) in [(-1, 0), (0, -1), (1, 0), (0, 1)] {
        if let (Some(nx), Some(ny)) = (x.checked_add_signed(dx), y.checked_add_signed(dy))
            && nx < CITY_WIDTH && ny < CITY_HEIGHT && city.altitude_at(nx, ny) < land
        { mac_apply_marked_terrain_raise_with_callbacks(city, nx, ny, funds, rng, destroy, mark_bounds); }
    }
    if *funds < 25 { return; }
    *funds -= 25;
    let altm = city.altm_at(x, y);
    let word = u16::from(altm.land) | (u16::from(altm.water) << 5) | (u16::from(altm.tunnel_levels) << 10);
    city.set_altm_word(x, y, (word & !31) | (u16::from(land) + 1));
    if let Some(zone) = city.zone_at(x, y) { city.set_zone(x, y, zone.raw_bits() & 0xf0); }
    for (dx, dy) in NEIGHBORHOOD {
        if let Some(nx) = x.checked_add_signed(dx)
            && let Some(ny) = y.checked_add_signed(dy)
        {
            mark_bounds(city, nx, ny);
            mac_classify_terrain_tile_with_destroy(city, nx, ny, rng, destroy);
            mark_bounds(city, nx, ny);
        }
    }
}

/// CODE 3 export 138: rendered surface removal, mode-sensitive PlaceTile,
/// XUND cleanup, then the height-neighbor/contour/water writes.
pub(super) fn mac_classify_terrain_tile(
    city: &mut City,
    x: usize,
    y: usize,
    rng: &mut MacToolboxRng,
) -> u32 {
    mac_classify_terrain_tile_with_destroy(city, x, y, rng, &mut |city, x, y, rng| {
        super::budget::mac_destroy_building_export80(city, x, y, rng, true)
    })
}

fn mac_classify_terrain_tile_with_destroy(
    city: &mut City,
    x: usize,
    y: usize,
    rng: &mut MacToolboxRng,
    destroy: &mut impl FnMut(&mut City, usize, usize, &mut MacToolboxRng) -> u32,
) -> u32 {
    const DX: [isize; 8] = [0, 1, 1, 1, 0, -1, -1, -1];
    const DY: [isize; 8] = [-1, -1, 0, 1, 1, 1, 0, -1];
    const WEIGHT: [u8; 8] = [0x03, 0x02, 0x06, 0x04, 0x0c, 0x08, 0x09, 0x01];
    const CLASS: [u8; 16] = [
        0x00, 0x09, 0x0a, 0x02, 0x0b, 0x0d, 0x03, 0x06, 0x0c, 0x01, 0x0d, 0x05, 0x04, 0x08, 0x07,
        0x32,
    ];

    if x >= CITY_WIDTH || y >= CITY_HEIGHT {
        return 0;
    }
    let original_building = city.building_at(x, y);
    let nested_export80_waits = if original_building >= 13 {
        destroy(city, x, y, rng)
    } else {
        0
    };
    if city
        .mac_state_misc_u32_at(super::MISC_CITY_MODE_OFFSET)
        .unwrap_or(0) as u16
        != 0
        && original_building != 5
    {
        super::pe_place_tile_441f00(city, x, y, 0);
    }

    // CODE 3 export 138 @ 0x139e4..0x139fe clears any nonzero XUND
    // through A5+0x372 (CODE 5 export 106) before classifying terrain.
    // City's writer maintains the saved Mac WORD census with no zone gate.
    if city.underground_at(x, y) != 0 {
        city.set_underground(x, y, 0);
    }
    let initial = city.altm_at(x, y);
    let initial_land = initial.land & 0x1f;
    let mut mask = 0u8;
    for direction in 0..8 {
        let nx = x as isize + DX[direction];
        let ny = y as isize + DY[direction];
        if nx >= 0
            && ny >= 0
            && nx < CITY_WIDTH as isize
            && ny < CITY_HEIGHT as isize
            && city.altitude_at(nx as usize, ny as usize) & 0x1f > initial_land
        {
            mask |= WEIGHT[direction];
        }
    }
    let class = CLASS[usize::from(mask & 0x0f)];
    if class != 0 {
        let zone = city.zone_at(x, y).map_or(0, |zone| zone.raw_bits());
        city.set_zone(x, y, zone & 0xf0);
    }

    let compared_land = if class == 0x32 {
        u16::from(initial_land) + 1
    } else {
        u16::from(initial_land)
    };
    let land = compared_land as u8 & 0x1f;
    if class == 0x32 {
        city.set_altitude(x, y, land, initial.water);
    }
    let sea_level = city.mac_selected_water_height().unwrap_or(0) as u16;
    let mut bits = city.tile_bits_at(x, y).unwrap_or_default().raw_bits();
    if compared_land >= sea_level {
        bits &= !0x04;
        city.set_tile_bits(x, y, TileBits::new(bits));
        city.set_terrain(x, y, if class == 0x32 { 0 } else { class });
    } else {
        bits |= 0x04;
        city.set_tile_bits(x, y, TileBits::new(bits));
        city.set_altitude(x, y, land, sea_level as u8);
        if city.building_at(x, y) != 5 {
            super::pe_place_tile_441f00(city, x, y, 0);
        }
        let terrain = if class == 0x32 {
            0x10
        } else if u16::from(initial_land) + 1 == sea_level {
            class.wrapping_add(0x20)
        } else {
            class.wrapping_add(0x10)
        };
        city.set_terrain(x, y, terrain);
    }
    nested_export80_waits
}
const CARDINAL: [(isize, isize); 4] = [(0, -1), (1, 0), (0, 1), (-1, 0)];
const NEIGHBORHOOD: [(isize, isize); 9] = [
    (0, -1),
    (1, -1),
    (1, 0),
    (1, 1),
    (0, 1),
    (-1, 1),
    (-1, 0),
    (-1, -1),
    (0, 0),
];

/// CODE 5 selector 4 owns reserved XTHG slot zero and starts its previous
/// coordinate at (0,0), independently of the selected press tile.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct LevelDrag {
    height: u8,
    previous: (usize, usize),
}

impl LevelDrag {
    pub(super) fn begin(city: &mut City, x: usize, y: usize) -> Option<Self> {
        if x >= CITY_WIDTH || y >= CITY_HEIGHT {
            return None;
        }
        let height = city.altitude_at(x, y) & 0x1f;
        let record = city.mac_object_slot_records_mut().first_mut()?;
        record.kind = 4;
        record.lane = 0;
        record.height = 0;
        record.sub_x = 0;
        record.sub_y = 0;
        record.target_x = 0;
        record.target_y = 0;
        Some(Self {
            height,
            previous: (0, 0),
        })
    }

    pub(super) fn sample(
        &mut self,
        city: &mut City,
        x: usize,
        y: usize,
        rng: &mut MacToolboxRng,
    ) -> bool {
        if x >= CITY_WIDTH || y >= CITY_HEIGHT || self.previous == (x, y) {
            return false;
        }
        let dx = x as isize - self.previous.0 as isize;
        let dy = y as isize - self.previous.1 as isize;
        let direction = if dx.abs() > dy.abs() {
            if dx > 0 { 1 } else { 3 }
        } else if dy > 0 {
            2
        } else {
            0
        };
        for (dx, dy) in CARDINAL {
            let Some(nx) = x.checked_add_signed(dx).filter(|&x| x < CITY_WIDTH) else {
                continue;
            };
            let Some(ny) = y.checked_add_signed(dy).filter(|&y| y < CITY_HEIGHT) else {
                continue;
            };
            let altitude = city.altm_at(nx, ny);
            city.set_altitude(nx, ny, self.height, altitude.water);
            relax_slopes(city, nx, ny, rng);
            classify_neighborhood(city, nx, ny, rng);
        }
        // Original direct byte writes are unconditional, including the first
        // previous=(0,0) clear and overwriting an existing current marker.
        city.set_text_overlay(self.previous.0, self.previous.1, 0);
        city.set_text_overlay(x, y, 0xc9);
        if let Some(record) = city.mac_object_slot_records_mut().first_mut() {
            record.lane = direction;
            record.tile_x = x as u8;
            record.tile_y = y as u8;
            record.height = 0;
        }
        self.previous = (x, y);
        true
    }

    pub(super) fn finish(self, city: &mut City) {
        city.set_text_overlay(self.previous.0, self.previous.1, 0);
        // A5+0x39a is CODE 6 export 111: scene bounds only. It does not
        // deactivate, zero, or restore the reserved actor record.
    }
}

/// One valid held-pointer sample from CODE 5 0x8d54/0x8c3e. Both spend
/// four coordinate draws before rejecting a candidate; neither recurses
/// through Stretch's slope worker or charges city-view terraforming costs.
pub(super) fn sample_height_step(
    city: &mut City,
    x: usize,
    y: usize,
    raise: bool,
    rng: &mut MacToolboxRng,
) -> bool {
    if x >= CITY_WIDTH || y >= CITY_HEIGHT {
        return false;
    }
    let nx = x as isize + (rng.random_u16() & 3) as isize - (rng.random_u16() & 3) as isize;
    let ny = y as isize + (rng.random_u16() & 3) as isize - (rng.random_u16() & 3) as isize;
    if !(0..CITY_WIDTH as isize).contains(&nx) || !(0..CITY_HEIGHT as isize).contains(&ny) {
        return false;
    }
    let (x, y) = (nx as usize, ny as usize);
    let altitude = city.altm_at(x, y);
    let height = altitude.land & 0x1f;
    if (raise && height == 31) || (!raise && height == 0) {
        return false;
    }
    for (dx, dy) in CARDINAL {
        let Some(nx) = x.checked_add_signed(dx).filter(|&x| x < CITY_WIDTH) else {
            continue;
        };
        let Some(ny) = y.checked_add_signed(dy).filter(|&y| y < CITY_HEIGHT) else {
            continue;
        };
        let neighbor = city.altitude_at(nx, ny) & 0x1f;
        if (raise && neighbor < height) || (!raise && neighbor > height) {
            return false;
        }
    }
    city.set_altitude(
        x,
        y,
        if raise { height + 1 } else { height - 1 },
        altitude.water,
    );
    classify_neighborhood(city, x, y, rng);
    true
}

/// CODE 5 0x943a: WORD pointer subtraction and signed division truncate
/// toward zero. Compare against the last requested height, not a height
/// subsequently changed by the contour classifier.
pub(super) fn drag_target_height(
    initial_height: u8,
    last_requested_height: u8,
    press_y: i16,
    mouse_y: i16,
    native_zoom: i16,
) -> Option<u8> {
    let divisor = match native_zoom {
        0 => 3,
        1 => 6,
        2 => 12,
        _ => 1,
    };
    let initial = i16::from(initial_height & 0x1f);
    let height = initial.wrapping_add(press_y.wrapping_sub(mouse_y) / divisor);
    ((1..31).contains(&height) && height >= initial - 10 && height as u8 != last_requested_height)
        .then_some(height as u8)
}

/// CODE 3 export 135 visits N,NE,E,SE,S,SW,W,NW,center. City tracks its own
/// dirty cells; original scene/actor callback chronology remains separate.
fn classify_neighborhood(city: &mut City, x: usize, y: usize, rng: &mut MacToolboxRng) {
    for (dx, dy) in NEIGHBORHOOD {
        if let Some(nx) = x.checked_add_signed(dx)
            && let Some(ny) = y.checked_add_signed(dy)
        {
            mac_classify_terrain_tile(city, nx, ny, rng);
        }
    }
}

/// CODE 5 0x9fa0 recurses immediately after each neighbor height write and
/// classifies that neighbor's nine cells only on return. Explicit frames keep
/// that depth-first order without consuming the host thread's call stack.
fn relax_slopes(city: &mut City, x: usize, y: usize, rng: &mut MacToolboxRng) {
    struct Frame {
        x: usize,
        y: usize,
        height: u8,
        next_direction: usize,
    }
    let mut stack = vec![Frame {
        x,
        y,
        height: city.altitude_at(x, y) & 0x1f,
        next_direction: 0,
    }];
    while let Some(frame) = stack.last_mut() {
        if frame.next_direction == CARDINAL.len() {
            let finished = stack.pop().expect("slope frame");
            if !stack.is_empty() {
                classify_neighborhood(city, finished.x, finished.y, rng);
            }
            continue;
        }
        let (dx, dy) = CARDINAL[frame.next_direction];
        frame.next_direction += 1;
        let Some(nx) = frame.x.checked_add_signed(dx).filter(|&x| x < CITY_WIDTH) else {
            continue;
        };
        let Some(ny) = frame.y.checked_add_signed(dy).filter(|&y| y < CITY_HEIGHT) else {
            continue;
        };
        if city.building_at(nx, ny) >= 13 {
            continue;
        }
        let altitude = city.altm_at(nx, ny);
        let difference = i16::from(frame.height) - i16::from(altitude.land & 0x1f);
        if difference.abs() <= 1 {
            continue;
        }
        let height = if difference > 1 {
            frame.height - 1
        } else {
            frame.height + 1
        };
        city.set_altitude(nx, ny, height, altitude.water);
        stack.push(Frame {
            x: nx,
            y: ny,
            height,
            next_direction: 0,
        });
    }
}

pub(super) fn apply_drag_height(
    city: &mut City,
    x: usize,
    y: usize,
    height: u8,
    rng: &mut MacToolboxRng,
) {
    if x >= CITY_WIDTH || y >= CITY_HEIGHT {
        return;
    }
    let altitude = city.altm_at(x, y);
    city.set_altitude(x, y, height, altitude.water);
    relax_slopes(city, x, y, rng);
    classify_neighborhood(city, x, y, rng);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::city::CityFileKind;

    #[test]
    fn mac_raise_validation_keeps_failed_marks_and_checks_center_first() {
        let mut city = City::new_blank("/tmp/mac-raise-marks", "Marks").unwrap();
        city.kind = CityFileKind::MacCity;
        city.set_zone(11, 10, 7);
        assert!(!mac_mark_terrain_raise(&mut city, 10, 10));
        assert!(city.tile_bits_at(10, 10).unwrap().flatten_mark());
        assert!(mac_mark_terrain_raise(&mut city, 10, 10));
        city.set_zone(10, 10, 7);
        assert!(!mac_mark_terrain_raise(&mut city, 10, 10));
    }

    #[test]
    fn mac_marked_raise_spills_height_word_and_respects_remaining_funds() {
        let mut city = City::new_blank("/tmp/mac-raise-spill", "Spill").unwrap();
        city.kind = CityFileKind::MacCity;
        city.set_mac_selected_water_height(0);
        for x in 9..=11 { for y in 9..=11 { city.set_altm_word(x, y, 0); } }
        city.set_altm_word(10, 10, 31);
        city.set_tile_bits(10, 10, TileBits::new(8));
        let mut rng = MacToolboxRng::new(1);
        let mut funds = 24;
        mac_apply_marked_terrain_raise(&mut city, 10, 10, &mut funds, &mut rng);
        assert_eq!(city.altitude_at(10, 10), 31);
        assert!(!city.tile_bits_at(10, 10).unwrap().flatten_mark());
        city.set_tile_bits(10, 10, TileBits::new(8));
        funds = 25;
        mac_apply_marked_terrain_raise(&mut city, 10, 10, &mut funds, &mut rng);
        assert_eq!(funds, 0);
        assert_eq!((city.altitude_at(10, 10), city.water_height_at(10, 10)), (0, 1));
    }

    #[test]
    fn mac_level_drag_direction_ties_and_map_edges() {
        let mut city =
            City::new_blank("/tmp/mac-level-directions.sc2", "MacLevelDirections").unwrap();
        city.set_mac_selected_water_height(0);
        let mut drag = LevelDrag::begin(&mut city, 64, 64).unwrap();
        let mut rng = MacToolboxRng::new(42);
        for (x, y, direction) in [(5, 5, 2), (7, 6, 1), (5, 5, 3), (4, 4, 0), (0, 127, 2)] {
            assert!(drag.sample(&mut city, x, y, &mut rng));
            let record = city.traffic_overlays().unwrap()[0];
            assert_eq!(record.lane, direction);
            assert_eq!((record.tile_x, record.tile_y), (x as u8, y as u8));
        }
        assert!(!drag.sample(&mut city, 0, 127, &mut rng));
        assert!(!drag.sample(&mut city, CITY_WIDTH, 127, &mut rng));
        assert_eq!(city.text_overlay_at(0, 127), Some(0xc9));
        drag.finish(&mut city);
        assert_eq!(city.text_overlay_at(0, 127), Some(0));
        assert_eq!(rng.seed(), 42);
        assert!(LevelDrag::begin(&mut city, CITY_WIDTH, 0).is_none());
    }

    #[test]
    fn mac_height_step_checks_all_cardinals_and_native_limits() {
        for raise in [false, true] {
            for (dx, dy) in CARDINAL {
                let mut city =
                    City::new_blank("/tmp/mac-height-gate.sc2", "MacHeightGate").unwrap();
                let (nx, ny) = (
                    64usize.checked_add_signed(dx).unwrap(),
                    64usize.checked_add_signed(dy).unwrap(),
                );
                city.set_altitude(nx, ny, if raise { 7 } else { 9 }, 0);
                city.set_underground(64, 64, 0x23);
                city.set_text_overlay(64, 64, 0xc9);
                let mut rng = MacToolboxRng::new(0); // Zero jitter isolates the slope predicate.
                assert!(!sample_height_step(&mut city, 64, 64, raise, &mut rng));
                assert_eq!(city.altitude_at(64, 64), 8);
                assert_eq!(city.underground_at(64, 64), 0x23);
                assert_eq!(city.text_overlay_at(64, 64), Some(0xc9));
            }
            let mut city = City::new_blank("/tmp/mac-height-limit.sc2", "MacHeightLimit").unwrap();
            city.set_altitude(0, 0, if raise { 31 } else { 0 }, 7);
            let mut rng = MacToolboxRng::new(0);
            assert!(!sample_height_step(&mut city, 0, 0, raise, &mut rng));
            assert_eq!(city.water_height_at(0, 0), 7);
            let mut rng = MacToolboxRng::new(42);
            assert!(!sample_height_step(
                &mut city, CITY_WIDTH, 0, raise, &mut rng
            ));
            assert_eq!(rng.seed(), 42); // Invalid pointer conversion, not candidate refusal.
        }
    }

    #[test]
    fn mac_height_drag_word_arithmetic_and_native_limits() {
        for (zoom, divisor) in [(0, 3), (1, 6), (2, 12)] {
            assert_eq!(
                drag_target_height(15, 15, 100, 100 - divisor, zoom),
                Some(16)
            );
            assert_eq!(
                drag_target_height(15, 15, 100, 100 + divisor, zoom),
                Some(14)
            );
            assert_eq!(drag_target_height(15, 15, 100, 101, zoom), None);
            assert_eq!(
                drag_target_height(15, 15, 100, 100 + 10 * divisor, zoom),
                Some(5)
            );
            assert_eq!(
                drag_target_height(15, 15, 100, 100 + 11 * divisor, zoom),
                None
            );
            assert_eq!(drag_target_height(30, 30, 100, 100 - divisor, zoom), None);
            assert_eq!(drag_target_height(1, 1, 100, 100 + divisor, zoom), None);
            assert_eq!(drag_target_height(15, 16, 100, 100 - divisor, zoom), None);
        }
        // The original SUB.W wraps before division; it is not an i32 delta.
        assert_eq!(drag_target_height(10, 10, i16::MIN, i16::MAX, 3), Some(11));
    }

    #[test]
    fn mac_height_drag_matches_independent_recursive_plane_fixtures() {
        // Independent scalar transcription of 0x9fa0 and export 135/138.
        // The lower request is refilled by the native special contour arm;
        // hashing both complete planes pins the return-time update order.
        for (height, expected) in [(10, 0x9f35_d19a_9d75_63cf), (6, 0x654a_3d0c_f256_2325)] {
            for kind in [CityFileKind::MacCity, CityFileKind::WinSc2] {
                let mut city = City::new_blank("/tmp/sc2000-mac-height-planes.sc2", "MacHeight")
                    .expect("blank city");
                city.kind = kind;
                city.set_mac_selected_water_height(0);
                let seed = 0x943a_1234;
                let mut rng = MacToolboxRng::new(seed);
                let initial_seed = rng.seed();
                apply_drag_height(&mut city, 64, 64, height, &mut rng);
                let mut digest = 0xcbf2_9ce4_8422_2325u64;
                for x in 0..CITY_WIDTH {
                    for y in 0..CITY_HEIGHT {
                        for byte in [city.altitude_at(x, y), city.terrain_at(x, y)] {
                            digest = (digest ^ u64::from(byte)).wrapping_mul(0x100_0000_01b3);
                        }
                    }
                }
                assert_eq!(digest, expected, "height={height}, kind={kind:?}");
                assert_eq!(rng.seed(), initial_seed);
            }
        }
    }
}
