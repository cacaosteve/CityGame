//! Shared Mac/PE selected-water-height control, with edition-specific surface clearing.

use super::{EngineFidelity, pe_place_tile_441f00};
use crate::city::{CITY_HEIGHT, CITY_WIDTH, City};

fn next_level(current: i16, raise: bool) -> Option<i16> {
    if raise {
        (current <= 30).then(|| current.wrapping_add(1))
    } else {
        (current >= 1).then(|| current.wrapping_sub(1))
    }
}

/// Mac CODE 3 0x130ea and PE 0x4460b0 scan x-major/y-inner. Only the
/// flooded XBLD clear differs: Mac calls PlaceTile for every flooded cell;
/// PE does so only for 6..=13. This is not the nine-neighbor classifier.
fn apply(city: &mut City, engine: EngineFidelity, level: i16) -> usize {
    let mut affected = 0;
    for x in 0..CITY_WIDTH {
        for y in 0..CITY_HEIGHT {
            let altitude = city.altm_at(x, y);
            let terrain = city.terrain_at(x, y);
            let building = city.building_at(x, y);
            let bits = city.tile_bits_at(x, y).unwrap_or_default();
            let before = (altitude, terrain, building, bits);
            let land = i16::from(altitude.land & 0x1f);
            // Native WORD mask/shift must retain its out-of-range behavior:
            // a malformed signed level can OR into the high ALTM bits.
            let packed = u16::from(altitude.land & 0x1f)
                | (u16::from(altitude.tunnel_levels & 0x3f) << 10)
                | (level as u16).wrapping_shl(5);
            if land < level {
                city.set_altm_word(x, y, packed);
                if engine == EngineFidelity::Mac68k || (6..=13).contains(&building) {
                    pe_place_tile_441f00(city, x, y, 0);
                }
                city.set_tile_bits(x, y, bits.with_water_tile(true));
                let terrain = if terrain >= 0x30 {
                    0x10
                } else if land + 1 < level || terrain == 0 {
                    (terrain & 0x0f) + 0x10
                } else {
                    (terrain & 0x0f) + 0x20
                };
                city.set_terrain(x, y, terrain);
            } else if bits.water_tile() {
                city.set_tile_bits(x, y, bits.with_water_tile(false));
                if terrain < 0x30 {
                    city.set_altm_word(x, y, packed);
                    city.set_terrain(x, y, terrain & 0x0f);
                }
            }
            let after = (
                city.altm_at(x, y),
                city.terrain_at(x, y),
                city.building_at(x, y),
                city.tile_bits_at(x, y).unwrap_or_default(),
            );
            affected += usize::from(before != after);
        }
    }
    affected
}

pub(super) fn step(city: &mut City, engine: EngineFidelity, raise: bool) -> Option<(i16, usize)> {
    // Both original load/save cursors map this WORD to MISC slot 912.
    let current = city.mac_selected_water_height().unwrap_or(0) as u16 as i16;
    let next = next_level(current, raise)?;
    city.set_selected_water_height_word(next);
    Some((next, apply(city, engine, next)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_water_level_steps_preserve_signed_word_bounds_and_packing() {
        assert_eq!(next_level(30, true), Some(31));
        assert_eq!(next_level(31, true), None);
        assert_eq!(next_level(1, false), Some(0));
        assert_eq!(next_level(0, false), None);
        assert_eq!(next_level(i16::MIN, true), Some(i16::MIN + 1));
        assert_eq!(next_level(i16::MAX, false), Some(i16::MAX - 1));
        for engine in [EngineFidelity::Win95, EngineFidelity::Mac68k] {
            let mut city =
                City::new_blank("/tmp/native-water-word.sc2", "NativeWaterWord").unwrap();
            city.set_mac_state_misc_u32_at(0x0e40, 0x1234_fffe);
            city.set_altitude(20, 20, 4, 3);
            city.set_tile_bits(20, 20, crate::city::TileBits::new(4));
            city.set_terrain(20, 20, 0x13);
            assert_eq!(step(&mut city, engine, true), Some((-1, 1)));
            assert_eq!(city.mac_state_misc_u32_at(0x0e40), Some(u32::MAX));
            assert_eq!(city.altitude_at(20, 20), 4);
            assert_eq!(city.water_height_at(20, 20), 31);
            assert_eq!(city.altm_at(20, 20).tunnel_levels, 63);
            assert_eq!(city.terrain_at(20, 20), 3);
            assert!(!city.tile_bits_at(20, 20).unwrap().water_tile());
        }
    }
}
