//! Main-view Maps / data overlays (OpenSC2K `CityDataView` colors as oracle).
//!
//! Tints existing isometric sprites; power/water are 3-state XBIT readouts,
//! traffic/land value/pollution/crime/density are XTRF/XVAL/XPLT/XCRM/XPOP gradients,
//! growth is a 3-state XROG sign (decline / steady / growth), height is ALTM land HSV.

use crate::city::TileBits;

/// Diagnose-the-city overlay mode for the main isometric view.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum CityDataMapMode {
    #[default]
    City,
    Power,
    Water,
    Traffic,
    LandValue,
    Pollution,
    Crime,
    Police,
    Fire,
    Density,
    Growth,
    Height,
}

impl CityDataMapMode {
    pub fn label(self) -> &'static str {
        match self {
            Self::City => "City",
            Self::Power => "Power Supply",
            Self::Water => "Water Supply",
            Self::Traffic => "Traffic",
            Self::LandValue => "Land Value",
            Self::Pollution => "Pollution",
            Self::Crime => "Crime",
            Self::Police => "Police Power",
            Self::Fire => "Fire Power",
            Self::Density => "Density",
            Self::Growth => "Rate of Growth",
            Self::Height => "Heightmap",
        }
    }

    pub fn is_overlay(self) -> bool {
        self != Self::City
    }
}

/// OpenSC2K: no link / no supply / supplied (`CityDataView.color` for POWER/WATER).
const STATE_NO_LINK: [f32; 4] = [0.40784314, 0.45098040, 0.50588240, 1.0]; // #687381
const STATE_NO_SUPPLY: [f32; 4] = [0.88627450, 0.36078432, 0.27450982, 1.0]; // #e25c46
const POWER_SUPPLIED: [f32; 4] = [0.95686275, 0.82745098, 0.36862746, 1.0]; // #f4d35e
const WATER_SUPPLIED: [f32; 4] = [0.25882354, 0.74117650, 0.90980390, 1.0]; // #42bde8

/// OpenSC2K TRAFFIC gradient ends.
const TRAFFIC_LOW: [f32; 4] = [0.11764706, 0.22745098, 0.18431373, 1.0]; // #1e3a2f
const TRAFFIC_HIGH: [f32; 4] = [1.0, 0.41176470, 0.29803922, 1.0]; // #ff694c

/// OpenSC2K LAND_VALUE gradient ends.
const LAND_VALUE_LOW: [f32; 4] = [0.15294118, 0.23529412, 0.39607844, 1.0]; // #273c65
const LAND_VALUE_HIGH: [f32; 4] = [0.34509805, 0.84313726, 0.63137255, 1.0]; // #58d7a1

/// OpenSC2K `DEFAULT_GRADIENT` for POLLUTION / CRIME (and other nuisance maps).
const NUISANCE_LOW: [f32; 4] = [0.16862746, 0.32156864, 0.37647060, 1.0]; // #2b5260
const NUISANCE_HIGH: [f32; 4] = [1.0, 0.41176470, 0.29803922, 1.0]; // #ff694c

/// OpenSC2K POLICE_POWER / FIRE_POWER gradient ends.
const POLICE_LOW: [f32; 4] = [0.10980392, 0.14117648, 0.25098040, 1.0]; // #1c2440
const POLICE_HIGH: [f32; 4] = [0.43529412, 0.65882355, 1.0, 1.0]; // #6fa8ff
const FIRE_LOW: [f32; 4] = [0.22745098, 0.12156863, 0.10980392, 1.0]; // #3a1f1c
const FIRE_HIGH: [f32; 4] = [1.0, 0.64705884, 0.32156864, 1.0]; // #ffa552

/// OpenSC2K DENSITY gradient ends.
const DENSITY_LOW: [f32; 4] = [0.12156863, 0.17647059, 0.24705882, 1.0]; // #1f2d3f
const DENSITY_HIGH: [f32; 4] = [1.0, 0.81960785, 0.40000000, 1.0]; // #ffd166

/// OpenSC2K GROWTH sign colors (decline / steady / growth).
const GROWTH_DECLINE_COLOR: [f32; 4] = [0.88627450, 0.27058825, 0.23529412, 1.0]; // #e2453c
const GROWTH_STEADY_COLOR: [f32; 4] = [0.72549020, 0.76078430, 0.80392160, 1.0]; // #b9c2cd
const GROWTH_INCREASE_COLOR: [f32; 4] = [0.20784314, 0.81960785, 0.41568628, 1.0]; // #35d16a

/// OpenSC2K `GROWTH_DECLINE` / `GROWTH_INCREASE` XROG band edges.
pub const GROWTH_DECLINE: u8 = 0x7d;
pub const GROWTH_INCREASE: u8 = 0x83;

const WHITE: [f32; 4] = [1.0, 1.0, 1.0, 1.0];

/// Power: 2 if powered, else 1 if powerable, else 0 (OpenSC2K `CityDataView.value`).
pub fn power_state(bits: Option<TileBits>) -> u8 {
    let Some(bits) = bits else {
        return 0;
    };
    if bits.powered() {
        2
    } else if bits.powerable() {
        1
    } else {
        0
    }
}

/// Water: 2 if watered, else 1 if piped, else 0.
pub fn water_state(bits: Option<TileBits>) -> u8 {
    let Some(bits) = bits else {
        return 0;
    };
    if bits.watered() {
        2
    } else if bits.piped() {
        1
    } else {
        0
    }
}

fn lerp_rgba(low: [f32; 4], high: [f32; 4], t: f32) -> [f32; 4] {
    let t = t.clamp(0.0, 1.0);
    [
        low[0] + (high[0] - low[0]) * t,
        low[1] + (high[1] - low[1]) * t,
        low[2] + (high[2] - low[2]) * t,
        low[3] + (high[3] - low[3]) * t,
    ]
}

fn utility_state_tint(state: u8, supplied: [f32; 4]) -> [f32; 4] {
    match state {
        2 => supplied,
        1 => STATE_NO_SUPPLY,
        _ => STATE_NO_LINK,
    }
}

fn gradient_tint(low: [f32; 4], high: [f32; 4], amount: Option<u8>) -> [f32; 4] {
    lerp_rgba(low, high, f32::from(amount.unwrap_or(0)) / 255.0)
}

/// Per-tile samples for gradient Maps modes.
#[derive(Debug, Clone, Copy, Default)]
pub struct DataMapSamples {
    pub traffic: Option<u8>,
    pub land_value: Option<u8>,
    pub pollution: Option<u8>,
    pub crime: Option<u8>,
    pub police: Option<u8>,
    pub fire: Option<u8>,
    pub density: Option<u8>,
    pub growth: Option<u8>,
    /// ALTM land nibble `0..=0x1f` (OpenSC2K Heightmap).
    pub height: Option<u8>,
}

/// OpenSC2K `CityDataView.growth_state`: decline / steady / growth around the mid band.
pub fn growth_state(value: u8) -> u8 {
    if value < GROWTH_DECLINE {
        0
    } else if value < GROWTH_INCREASE {
        1
    } else {
        2
    }
}

fn growth_tint(amount: Option<u8>) -> [f32; 4] {
    match growth_state(amount.unwrap_or(0x7f)) {
        0 => GROWTH_DECLINE_COLOR,
        2 => GROWTH_INCREASE_COLOR,
        _ => GROWTH_STEADY_COLOR,
    }
}

/// OpenSC2K `CityDataView.color` HEIGHT: HSV `((31-level)/31)*0.75`, sat 0.85, val 0.95.
pub fn height_tint(land_altitude: Option<u8>) -> [f32; 4] {
    let level = land_altitude.unwrap_or(0).min(31);
    let hue = f32::from(31 - level) / 31.0 * 0.75;
    hsv_to_rgba(hue, 0.85, 0.95)
}

fn hsv_to_rgba(h: f32, s: f32, v: f32) -> [f32; 4] {
    let h = h.rem_euclid(1.0);
    let s = s.clamp(0.0, 1.0);
    let v = v.clamp(0.0, 1.0);
    let i = (h * 6.0).floor();
    let f = h * 6.0 - i;
    let p = v * (1.0 - s);
    let q = v * (1.0 - f * s);
    let t = v * (1.0 - (1.0 - f) * s);
    let (r, g, b) = match i as i32 % 6 {
        0 => (v, t, p),
        1 => (q, v, p),
        2 => (p, v, t),
        3 => (p, q, v),
        4 => (t, p, v),
        _ => (v, p, q),
    };
    [r, g, b, 1.0]
}

/// Per-tile tint for the active data map mode. `City` returns white (no overlay).
pub fn tile_tint(
    mode: CityDataMapMode,
    bits: Option<TileBits>,
    samples: DataMapSamples,
) -> [f32; 4] {
    match mode {
        CityDataMapMode::City => WHITE,
        CityDataMapMode::Power => utility_state_tint(power_state(bits), POWER_SUPPLIED),
        CityDataMapMode::Water => utility_state_tint(water_state(bits), WATER_SUPPLIED),
        CityDataMapMode::Traffic => gradient_tint(TRAFFIC_LOW, TRAFFIC_HIGH, samples.traffic),
        CityDataMapMode::LandValue => {
            gradient_tint(LAND_VALUE_LOW, LAND_VALUE_HIGH, samples.land_value)
        }
        CityDataMapMode::Pollution | CityDataMapMode::Crime => {
            let amount = match mode {
                CityDataMapMode::Pollution => samples.pollution,
                _ => samples.crime,
            };
            gradient_tint(NUISANCE_LOW, NUISANCE_HIGH, amount)
        }
        CityDataMapMode::Police => gradient_tint(POLICE_LOW, POLICE_HIGH, samples.police),
        CityDataMapMode::Fire => gradient_tint(FIRE_LOW, FIRE_HIGH, samples.fire),
        CityDataMapMode::Density => gradient_tint(DENSITY_LOW, DENSITY_HIGH, samples.density),
        CityDataMapMode::Growth => growth_tint(samples.growth),
        CityDataMapMode::Height => height_tint(samples.height),
    }
}

pub fn multiply_rgba(a: [f32; 4], b: [f32; 4]) -> [f32; 4] {
    [a[0] * b[0], a[1] * b[1], a[2] * b[2], a[3] * b[3]]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::city::TileBits;

    #[test]
    fn power_and_water_three_states_match_opensc2k() {
        assert_eq!(power_state(None), 0);
        assert_eq!(power_state(Some(TileBits::new(0))), 0);
        assert_eq!(power_state(Some(TileBits::new(0x80))), 1); // powerable
        assert_eq!(power_state(Some(TileBits::new(0x40))), 2); // powered
        assert_eq!(power_state(Some(TileBits::new(0xc0))), 2);

        assert_eq!(water_state(None), 0);
        assert_eq!(water_state(Some(TileBits::new(0))), 0);
        assert_eq!(water_state(Some(TileBits::new(0x20))), 1); // piped
        assert_eq!(water_state(Some(TileBits::new(0x10))), 2); // watered
        assert_eq!(water_state(Some(TileBits::new(0x30))), 2);
    }

    #[test]
    fn city_mode_is_identity_white() {
        assert_eq!(
            tile_tint(
                CityDataMapMode::City,
                Some(TileBits::new(0xc0)),
                DataMapSamples {
                    traffic: Some(255),
                    land_value: Some(255),
                    pollution: Some(255),
                    crime: Some(255),
                    ..Default::default()
                },
            ),
            WHITE
        );
    }

    #[test]
    fn power_supplied_uses_opensc2k_yellow() {
        let tint = tile_tint(
            CityDataMapMode::Power,
            Some(TileBits::new(0x40)),
            DataMapSamples::default(),
        );
        assert!((tint[0] - POWER_SUPPLIED[0]).abs() < 1e-5);
        assert!((tint[1] - POWER_SUPPLIED[1]).abs() < 1e-5);
        assert!((tint[2] - POWER_SUPPLIED[2]).abs() < 1e-5);
    }

    #[test]
    fn water_no_supply_uses_opensc2k_red() {
        let tint = tile_tint(
            CityDataMapMode::Water,
            Some(TileBits::new(0x20)),
            DataMapSamples::default(),
        );
        assert!((tint[0] - STATE_NO_SUPPLY[0]).abs() < 1e-5);
        assert!((tint[1] - STATE_NO_SUPPLY[1]).abs() < 1e-5);
        assert!((tint[2] - STATE_NO_SUPPLY[2]).abs() < 1e-5);
    }

    #[test]
    fn traffic_and_land_value_lerp_ends() {
        let t0 = tile_tint(
            CityDataMapMode::Traffic,
            None,
            DataMapSamples {
                traffic: Some(0),
                ..Default::default()
            },
        );
        let t1 = tile_tint(
            CityDataMapMode::Traffic,
            None,
            DataMapSamples {
                traffic: Some(255),
                ..Default::default()
            },
        );
        assert!((t0[0] - TRAFFIC_LOW[0]).abs() < 1e-5);
        assert!((t1[0] - TRAFFIC_HIGH[0]).abs() < 1e-5);

        let l0 = tile_tint(
            CityDataMapMode::LandValue,
            None,
            DataMapSamples {
                land_value: Some(0),
                ..Default::default()
            },
        );
        let l1 = tile_tint(
            CityDataMapMode::LandValue,
            None,
            DataMapSamples {
                land_value: Some(255),
                ..Default::default()
            },
        );
        assert!((l0[2] - LAND_VALUE_LOW[2]).abs() < 1e-5);
        assert!((l1[1] - LAND_VALUE_HIGH[1]).abs() < 1e-5);
    }

    #[test]
    fn pollution_and_crime_use_opensc2k_default_gradient() {
        let p0 = tile_tint(
            CityDataMapMode::Pollution,
            None,
            DataMapSamples {
                pollution: Some(0),
                ..Default::default()
            },
        );
        let p1 = tile_tint(
            CityDataMapMode::Pollution,
            None,
            DataMapSamples {
                pollution: Some(255),
                ..Default::default()
            },
        );
        assert!((p0[0] - NUISANCE_LOW[0]).abs() < 1e-5);
        assert!((p1[0] - NUISANCE_HIGH[0]).abs() < 1e-5);

        let c1 = tile_tint(
            CityDataMapMode::Crime,
            None,
            DataMapSamples {
                crime: Some(255),
                ..Default::default()
            },
        );
        assert!((c1[0] - NUISANCE_HIGH[0]).abs() < 1e-5);
    }

    #[test]
    fn police_and_fire_use_opensc2k_gradients() {
        let p1 = tile_tint(
            CityDataMapMode::Police,
            None,
            DataMapSamples {
                police: Some(255),
                ..Default::default()
            },
        );
        assert!((p1[2] - POLICE_HIGH[2]).abs() < 1e-5);
        let f1 = tile_tint(
            CityDataMapMode::Fire,
            None,
            DataMapSamples {
                fire: Some(255),
                ..Default::default()
            },
        );
        assert!((f1[0] - FIRE_HIGH[0]).abs() < 1e-5);
    }

    #[test]
    fn density_uses_opensc2k_gradient() {
        let low = tile_tint(
            CityDataMapMode::Density,
            None,
            DataMapSamples {
                density: Some(0),
                ..Default::default()
            },
        );
        let high = tile_tint(
            CityDataMapMode::Density,
            None,
            DataMapSamples {
                density: Some(255),
                ..Default::default()
            },
        );
        assert!((low[0] - DENSITY_LOW[0]).abs() < 1e-5);
        assert!((high[0] - DENSITY_HIGH[0]).abs() < 1e-5);
    }

    #[test]
    fn growth_uses_opensc2k_three_state_sign() {
        assert_eq!(growth_state(0x7c), 0);
        assert_eq!(growth_state(0x7d), 1);
        assert_eq!(growth_state(0x82), 1);
        assert_eq!(growth_state(0x83), 2);

        let decline = tile_tint(
            CityDataMapMode::Growth,
            None,
            DataMapSamples {
                growth: Some(0x10),
                ..Default::default()
            },
        );
        let steady = tile_tint(
            CityDataMapMode::Growth,
            None,
            DataMapSamples {
                growth: Some(0x7f),
                ..Default::default()
            },
        );
        let growing = tile_tint(
            CityDataMapMode::Growth,
            None,
            DataMapSamples {
                growth: Some(0xa0),
                ..Default::default()
            },
        );
        assert!((decline[0] - GROWTH_DECLINE_COLOR[0]).abs() < 1e-5);
        assert!((steady[0] - GROWTH_STEADY_COLOR[0]).abs() < 1e-5);
        assert!((growing[1] - GROWTH_INCREASE_COLOR[1]).abs() < 1e-5);
    }

    #[test]
    fn height_uses_opensc2k_hsv_ramp() {
        let low = height_tint(Some(0));
        let high = height_tint(Some(31));
        // Level 0 → hue 0.75 (blue-ish); level 31 → hue 0 (red).
        assert!(low[2] > low[0], "low altitude should be cooler/bluer");
        assert!(high[0] > high[2], "high altitude should be warmer/redder");
        let mid = tile_tint(
            CityDataMapMode::Height,
            None,
            DataMapSamples {
                height: Some(16),
                ..Default::default()
            },
        );
        assert!((mid[0] - height_tint(Some(16))[0]).abs() < 1e-5);
    }

    #[test]
    fn multiply_rgba_scales_channels() {
        assert_eq!(
            multiply_rgba([1.0, 0.5, 0.25, 1.0], [0.5, 1.0, 2.0, 1.0]),
            [0.5, 0.5, 0.5, 1.0]
        );
    }
}
