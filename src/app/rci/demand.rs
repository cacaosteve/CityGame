use super::*;
use crate::city::Technology;

pub(in crate::app) fn rci_demand_for_kind(city: &City, kind: RciKind) -> i32 {
    saved_rci_demand_for_kind(city, kind).unwrap_or(0)
}

pub(in crate::app) fn saved_rci_demand_for_kind(city: &City, kind: RciKind) -> Option<i32> {
    if !city.pe_rci_demand_initialized() {
        return None;
    }
    let offset = RCI_DEMAND_MISC_OFFSET + rci_kind_index(kind) * 4;
    city.misc_i32_at(offset)
}

pub(in crate::app) fn rci_kind_index(kind: RciKind) -> usize {
    match kind {
        RciKind::Residential => 0,
        RciKind::Commercial => 1,
        RciKind::Industrial => 2,
    }
}

pub(in crate::app) fn rci_demand_values_for_city(
    city: &City,
    population: PopulationStats,
    _outcomes: CityServiceOutcomes,
) -> [i32; 3] {
    pe_rci_demand_values_from_previous(
        city,
        population,
        [
            saved_rci_demand_for_kind(city, RciKind::Residential).unwrap_or(0),
            saved_rci_demand_for_kind(city, RciKind::Commercial).unwrap_or(0),
            saved_rci_demand_for_kind(city, RciKind::Industrial).unwrap_or(0),
        ],
    )
}

/// One PE `SimulationRCIDemandUpdates` write from an explicit prior `wCityDemand` triple.
pub(in crate::app) fn pe_rci_demand_values_from_previous(
    city: &City,
    population: PopulationStats,
    previous: [i32; 3],
) -> [i32; 3] {
    let scores = pe_rci_zone_balance_scores(city, population);
    [
        pe_rci_clamp_demand_word(
            previous[0],
            pe_rci_demand_monthly_delta(city, RciKind::Residential, scores[0]),
        ),
        pe_rci_clamp_demand_word(
            previous[1],
            pe_rci_demand_monthly_delta(city, RciKind::Commercial, scores[1]),
        ),
        pe_rci_clamp_demand_word(
            previous[2],
            pe_rci_demand_monthly_delta(city, RciKind::Industrial, scores[2]),
        ),
    ]
}

/// Win95 `SimulationRCIDemandUpdates` nature sample @ `0x468626`: one random tile.
/// - XBLD `5` + `rand() & 0x0f == 0` → PlaceTile(0) (`0x468669`), then fall through
///   with the original XBLD still in `ebx`.
/// - Water-tile sample (`XBIT & 0x04`) exits (`0x468697`) after the radio arm.
/// - Tree/park band `6..=0x0D` skips the 1-in-16 gate (`0x46869d`/`0x4686a5`); other
///   XBLD values need `rand() & 0x0f == 0` (`0x4686a7`).
/// - Source age is only `6..=0x0B` → PlaceTile(XBLD+1) (`0x4686b4`/`0x4686bc`);
///   `0x0C`/`0x0D` skip age but still NESW-spread.
/// - NESW neighbor (`0x4686d0`): skip water / `>=0x0C` / `5`; `>=6` → PlaceTile(n+1),
///   else PlaceTile(6).
pub(in crate::app) fn pe_rci_demand_updates_nature_sample(
    city: &mut City,
    crt: &mut Win95CrtRand,
) -> Option<(usize, usize, u8)> {
    let x = usize::from(crt.random_u16() & 0x7f);
    let y = usize::from(crt.random_u16() & 0x7f);
    // PE keeps the pre-PlaceTile XBLD in `ebx` through the tree/neighbor arms.
    let building = city.building_at(x, y);
    let mut result = None;
    if building == 5 && crt.random_u16() & 0x0f == 0 {
        pe_place_tile_441f00(city, x, y, 0);
        result = Some((x, y, 0));
    }
    if city
        .tile_bits_at(x, y)
        .is_some_and(|bits| bits.water_tile())
    {
        return result;
    }
    // PE @ `0x46869d`: `6..=0x0D` falls into age/neighbor without `rand&0xf`.
    if !(6..=0x0d).contains(&building) && crt.random_u16() & 0x0f != 0 {
        return result;
    }
    // PE @ `0x4686b4`/`0x4686bc`: age only while `6 <= XBLD < 0x0C`.
    if (6..=0x0b).contains(&building) {
        let next = building + 1;
        pe_place_tile_441f00(city, x, y, next);
        result = Some((x, y, next));
    }
    // PE @ 0x4686d0: rand&3 NESW neighbor tree plant/age.
    let dir = crt.random_u16() & 3;
    let (nx, ny) = match dir {
        0 if x < 0x7f => (x + 1, y),
        1 if x > 0 => (x - 1, y),
        2 if y < 0x7f => (x, y + 1),
        3 if y > 0 => (x, y - 1),
        _ => (x, y),
    };
    // PE `0x4686df..0x4686fb` leaves the coordinate unchanged at a map
    // edge, then re-reads and processes that same tile as the neighbor.
    if !city
        .tile_bits_at(nx, ny)
        .is_some_and(|bits| bits.water_tile())
    {
        let neighbor = city.building_at(nx, ny);
        if neighbor < 0x0c && neighbor != 5 {
            let planted = if neighbor >= 6 { neighbor + 1 } else { 6 };
            pe_place_tile_441f00(city, nx, ny, planted);
            if result.is_none() {
                result = Some((nx, ny, planted));
            }
        }
    }
    result
}

/// Mac CODE 8 export 237, `0x38bf0..0x38d12`: one nature sample.
/// This map-mutating slice is scheduled by the Mac phase-21 publisher. The
/// known alert/history/timer draws that follow it are also scheduled there;
/// story insertion/presentation effects and the earlier session RNG chronology
/// remain incomplete, so this does not imply whole-worker parity.
pub(in crate::app) fn mac_rci_nature_sample(city: &mut City, mut random_word: impl FnMut() -> u16) {
    let x = usize::from(random_word() & 0x7f);
    let y = usize::from(random_word() & 0x7f);
    let initial = city.building_at(x, y);
    if initial == 5 && random_word() & 0x0f == 0 {
        city.set_building(x, y, 0);
    }
    if city
        .tile_bits_at(x, y)
        .is_some_and(|bits| bits.water_tile())
    {
        return;
    }
    if !(6..=13).contains(&initial) && random_word() & 0x0f != 0 {
        return;
    }
    if (6..=11).contains(&initial) {
        city.set_building(x, y, initial + 1);
    }
    let (nx, ny) = match random_word() & 3 {
        0 if x < 127 => (x + 1, y),
        1 if x > 0 => (x - 1, y),
        2 if y < 127 => (x, y + 1),
        3 if y > 0 => (x, y - 1),
        _ => (x, y),
    };
    if city
        .tile_bits_at(nx, ny)
        .is_some_and(|bits| bits.water_tile())
    {
        return;
    }
    let neighbor = city.building_at(nx, ny);
    if neighbor < 12 && neighbor != 5 {
        city.set_building(nx, ny, if neighbor >= 6 { neighbor + 1 } else { 6 });
    }
}

/// Run the non-map, pre-weather QuickDraw draws in Mac CODE 8
/// `export_237` (`0x38d28..0x3905a`). A successful final gate scans the
/// 17 technology dates in ascending order, admits the Extra poll when enabled,
/// and clears the first due date. In `export_240`, date slots
/// 0..=11 add command choices, and slots 12..=15 contribute to the gated
/// arcology latch refreshed when a date clears.
pub(in crate::app) fn mac_consume_phase21_pre_weather_event_draws(
    city: &mut City,
    current_year: u16,
    mut random_word: impl FnMut() -> u16,
) -> Option<usize> {
    let selector = random_word() % 6;
    if selector == 0 {
        // The two selector-zero alert gates are sequential. The first result
        // controls only its story insert; the second draw always follows.
        let _first_alert = random_word() & 3;
        let _second_alert = random_word() & 3;
    }

    // A nonzero XBLD-0xd7 census admits one random stadium-mask choice.
    // Whether it inserts a story has no effect on the subsequent draw order.
    if city.mac_xbld_tile_count(0xd7) != 0 {
        let _team_choice = random_word() % 5;
    }

    // Four paired alert thresholds consume both draws unconditionally.
    for _ in 0..8 {
        let _alert_roll = random_word();
    }

    // Each history metric chooses one of two mutually exclusive sites; both
    // branches consume exactly one draw.
    let _first_history_roll = random_word();
    let _second_history_roll = random_word();

    // The 1-in-8 technology-date scan gate precedes the weather draw. Its
    // first-due date clear also refreshes the Mac arcology latch.
    let technology_scan_gate = random_word() & 7;
    if technology_scan_gate != 0 {
        return None;
    }

    for (index, technology) in Technology::ALL.iter().copied().enumerate() {
        let offset = technology.misc_offset();
        let Some(saved_date) = city.mac_state_misc_u32_at(offset) else {
            continue;
        };
        let date = saved_date as u16;
        if date == 0 || (date as i16) > (current_year as i16) {
            continue;
        }

        // CODE 8 emits selector 4 or 5 before clearing the due date.
        city.mac_poll_for_extra(&mut random_word);
        // Each technology date occupies its own DWORD slot in the decoded
        // MISC model; native CODE 4 writes its WORD value into that slot.
        city.set_mac_state_misc_u32_at(offset, 0);
        city.set_technology_discovery_year(technology, 0);
        city.refresh_mac_arcology_latch();
        return Some(index);
    }
    None
}

/// Win95 `SimulationRCIDemandUpdates` radioactivity arm @ `0x468669` (tests).
pub(in crate::app) fn pe_rci_demand_updates_radioactivity_sample(
    city: &mut City,
    crt: &mut Win95CrtRand,
) -> Option<(usize, usize)> {
    let x = usize::from(crt.random_u16() & 0x7f);
    let y = usize::from(crt.random_u16() & 0x7f);
    if city.building_at(x, y) != 5 {
        return None;
    }
    if crt.random_u16() & 0x0f != 0 {
        return None;
    }
    pe_place_tile_441f00(city, x, y, 0);
    Some((x, y))
}

/// PE `FUN_00468150` adds each FTOL result to `wCityDemand` with `ADDW`
/// (`0x4685f4`) before comparing the signed word against the demand limits.
fn pe_rci_clamp_demand_word(previous: i32, delta: i32) -> i32 {
    let value = (previous as i16).wrapping_add(delta as i16);
    i32::from(value.clamp(RCI_DEMAND_MIN as i16, RCI_DEMAND_MAX as i16))
}

/// Win95 `0x4E9328`: signed demand step indexed by (ordinance-adjusted) tax rate.
const PE_RCI_TAX_DEMAND_TABLE: [i16; 23] = [
    200, 160, 120, 100, 75, 50, 25, 0, -25, -50, -100, -150, -200, -250, -300, -350, -400, -450,
    -500, -550, -600, -650, -700,
];

/// Win95 `0x4E9538`: difficulty factor added into the industrial numerator.
const PE_RCI_DIFFICULTY_FACTORS: [f64; 4] = [0.0, 1.2, 1.1, 0.95];
const PE_RCI_ZONE_POP_DIVISOR: u32 = 10;
const PE_RCI_RESIDENTIAL_JOBS_DIVISOR: i32 = 50;
const PE_RCI_INDUSTRIAL_POP_BIAS: u32 = 50_000;
const PE_RCI_INDUSTRIAL_POP_SCALE: f64 = 1.0 / 150_000.0; // `0x4daed0`
const PE_RCI_INDUSTRIAL_SCORE_FLOOR: f64 = 15.0; // `0x4daee0`
const PE_RCI_RECREATION_BASE: i32 = 10;
const PE_RCI_RECREATION_SCALE: i32 = 150;
const PE_RCI_COMMERCIAL_JOBS_SCALE: i32 = 4;
const PE_RCI_COMMERCIAL_JOBS_BIAS: i32 = 500; // `0x1f4`
const PE_RCI_PORT_INFRA_SCALE: i32 = 1_500;

pub(in crate::app) fn pe_rci_tax_demand_delta(tax_rate: i32) -> i32 {
    let index = tax_rate.clamp(0, (PE_RCI_TAX_DEMAND_TABLE.len() - 1) as i32) as usize;
    i32::from(PE_RCI_TAX_DEMAND_TABLE[index])
}

/// Win95 `SimulationRCIDemandUpdates` @ `0x46854c`: ordinances nudge the tax-rate index
/// before the `0x4E9328` lookup (not a separate demand bonus).
pub(in crate::app) fn pe_rci_ordinance_adjusted_tax_rate(
    city: &City,
    kind: RciKind,
    tax_rate: i32,
) -> i32 {
    let flags = ordinance_flags_for_city(city);
    let bit = |index: usize| flags & (1_u32 << index) != 0;
    let mut rate = tax_rate;
    match kind {
        RciKind::Residential => {
            // PE: `test ebp,ecx` with ebp=2 → bit 1 (Income Tax) inc; bit 14 (Beautify) dec.
            if bit(ORDINANCE_INCOME_TAX) {
                rate += 1;
            }
            if bit(ORDINANCE_CITY_BEAUTIFICATION) {
                rate -= 1;
            }
        }
        RciKind::Commercial => {
            // PE: bit 0 Sales Tax inc; bits 12/15/18 Tourist/Carnival/Homeless dec.
            if bit(ORDINANCE_SALES_TAX) {
                rate += 1;
            }
            if bit(ORDINANCE_TOURIST_ADVERTISING) {
                rate -= 1;
            }
            if bit(ORDINANCE_ANNUAL_CARNIVAL) {
                rate -= 1;
            }
            if bit(ORDINANCE_HOMELESS_SHELTER) {
                rate -= 1;
            }
        }
        RciKind::Industrial => {
            // PE: bit 13 Business Ad dec; bit 19 Pollution Controls inc.
            if bit(ORDINANCE_BUSINESS_ADVERTISING) {
                rate -= 1;
            }
            if bit(ORDINANCE_POLLUTION_CONTROLS) {
                rate += 1;
            }
        }
    }
    rate.max(0)
}

fn pe_rci_zone_pop(people: u32) -> i32 {
    (people / PE_RCI_ZONE_POP_DIVISOR) as i32
}

/// A positive/negative normal x87 value represented as
/// `significand * 2^(exponent - 63)`. The PE uses the default 64-bit x87
/// significand precision for the rooted RCI expressions, then stores each
/// score to a qword before loading it for the final `* 600 + tax` sequence.
#[derive(Clone, Copy)]
struct PeX87Value {
    negative: bool,
    significand: u64,
    exponent: i32,
}

impl PeX87Value {
    const ZERO: Self = Self {
        negative: false,
        significand: 0,
        exponent: 0,
    };

    fn from_f64(value: f64) -> Self {
        if value == 0.0 {
            return Self::ZERO;
        }
        let bits = value.to_bits();
        let negative = bits >> 63 != 0;
        let exponent_bits = ((bits >> 52) & 0x7ff) as i32;
        let fraction = bits & ((1_u64 << 52) - 1);
        if exponent_bits == 0 {
            let top_bit = 63 - fraction.leading_zeros() as i32;
            let shift = 63 - top_bit;
            return Self {
                negative,
                significand: fraction << shift,
                exponent: top_bit - 1074,
            };
        }
        Self {
            negative,
            significand: (((1_u64 << 52) | fraction) << 11),
            exponent: exponent_bits - 1023,
        }
    }

    fn from_i32(value: i32) -> Self {
        if value == 0 {
            return Self::ZERO;
        }
        let magnitude = value.unsigned_abs();
        let mut positive = Self::from_u32(magnitude);
        positive.negative = value < 0;
        positive
    }

    fn from_u32(value: u32) -> Self {
        if value == 0 {
            return Self::ZERO;
        }
        let magnitude = value;
        let exponent = 31 - magnitude.leading_zeros() as i32;
        Self {
            negative: false,
            significand: u64::from(magnitude) << (63 - exponent),
            exponent,
        }
    }

    fn compare(self, other: Self) -> std::cmp::Ordering {
        use std::cmp::Ordering;
        match (self.significand == 0, other.significand == 0) {
            (true, true) => return Ordering::Equal,
            (true, false) => {
                return if other.negative {
                    Ordering::Greater
                } else {
                    Ordering::Less
                };
            }
            (false, true) => {
                return if self.negative {
                    Ordering::Less
                } else {
                    Ordering::Greater
                };
            }
            (false, false) => {}
        }
        if self.negative != other.negative {
            return if self.negative {
                Ordering::Less
            } else {
                Ordering::Greater
            };
        }
        let magnitude = (self.exponent, self.significand).cmp(&(other.exponent, other.significand));
        if self.negative {
            magnitude.reverse()
        } else {
            magnitude
        }
    }

    fn from_ratio(numerator: u128, denominator: u128) -> Self {
        if numerator == 0 || denominator == 0 {
            return Self::ZERO;
        }
        let mut exponent =
            (128 - numerator.leading_zeros() as i32) - (128 - denominator.leading_zeros() as i32);
        if exponent >= 0 {
            if numerator < (denominator << exponent as u32) {
                exponent -= 1;
            }
        } else if (numerator << (-exponent) as u32) < denominator {
            exponent -= 1;
        }

        let shift = u32::try_from(63 - exponent)
            .expect("the normalized PE RCI ratios fit the x87 significand");
        let scaled_numerator = numerator << shift;
        let scaled_denominator = denominator;
        let quotient = scaled_numerator / scaled_denominator;
        let remainder = scaled_numerator % scaled_denominator;
        let mut significand =
            pe_x87_round_ratio_nearest_even(quotient, remainder, scaled_denominator);
        if significand == (1_u128 << 64) {
            significand >>= 1;
            exponent += 1;
        }
        Self {
            negative: false,
            significand: significand as u64,
            exponent,
        }
    }

    fn divided_by_u32(self, denominator: u32) -> Self {
        if self.significand == 0 || denominator == 0 {
            return Self::ZERO;
        }
        let mut quotient = Self::from_ratio(u128::from(self.significand), u128::from(denominator));
        quotient.negative = self.negative;
        quotient.exponent += self.exponent - 63;
        quotient
    }

    fn divided_by(self, denominator: Self) -> Self {
        if self.significand == 0 || denominator.significand == 0 {
            return Self::ZERO;
        }
        let mut quotient = Self::from_ratio(
            u128::from(self.significand),
            u128::from(denominator.significand),
        );
        quotient.negative = self.negative ^ denominator.negative;
        quotient.exponent += self.exponent - denominator.exponent;
        quotient
    }

    fn multiply_u32(self, factor: u32) -> Self {
        if self.significand == 0 || factor == 0 {
            return Self::ZERO;
        }
        let product = u128::from(self.significand) * u128::from(factor);
        let product_bits = 128 - product.leading_zeros() as i32;
        // The normalized significand already has bit 63 set, and a nonzero
        // integer factor therefore makes product_bits at least 64.
        let discard = (product_bits - 64) as u32;
        let mut significand = pe_x87_round_ratio_nearest_even(
            product >> discard,
            product & ((1_u128 << discard) - 1),
            1_u128 << discard,
        );
        let mut exponent = self.exponent + discard as i32;
        if significand == (1_u128 << 64) {
            significand >>= 1;
            exponent += 1;
        }
        Self {
            negative: self.negative,
            significand: significand as u64,
            exponent,
        }
    }

    fn multiplied_by(self, other: Self) -> Self {
        if self.significand == 0 || other.significand == 0 {
            return Self::ZERO;
        }
        let product = u128::from(self.significand) * u128::from(other.significand);
        let product_bits = 128 - product.leading_zeros() as i32;
        let discard = (product_bits - 64) as u32;
        let mut significand = pe_x87_round_ratio_nearest_even(
            product >> discard,
            product & ((1_u128 << discard) - 1),
            1_u128 << discard,
        );
        let mut exponent = self.exponent + other.exponent - 63 + discard as i32;
        if significand == (1_u128 << 64) {
            significand >>= 1;
            exponent += 1;
        }
        Self {
            negative: self.negative ^ other.negative,
            significand: significand as u64,
            exponent,
        }
    }

    fn add(self, other: Self) -> Self {
        if self.significand == 0 {
            return other;
        }
        if other.significand == 0 {
            return self;
        }
        let (large, small) =
            if (self.exponent, self.significand) >= (other.exponent, other.significand) {
                (self, other)
            } else {
                (other, self)
            };
        let difference = (large.exponent - small.exponent) as u32;
        // RCI demand scores are bounded by the rooted numerator/denominator
        // limits (less than 3,000); the integer tax term is at most 700, so
        // their exact exponent alignment is within this 128-bit accumulator.
        if difference > 63 {
            return large;
        }
        let large_magnitude = u128::from(large.significand) << difference;
        let small_magnitude = u128::from(small.significand);
        let (negative, magnitude) = if large.negative == small.negative {
            (large.negative, large_magnitude + small_magnitude)
        } else if large_magnitude >= small_magnitude {
            (large.negative, large_magnitude - small_magnitude)
        } else {
            (small.negative, small_magnitude - large_magnitude)
        };
        Self::from_aligned_magnitude(negative, magnitude, small.exponent)
    }

    fn from_aligned_magnitude(negative: bool, magnitude: u128, base_exponent: i32) -> Self {
        if magnitude == 0 {
            return Self::ZERO;
        }
        let bits = 128 - magnitude.leading_zeros() as i32;
        let (significand, exponent) = if bits > 64 {
            let discard = (bits - 64) as u32;
            let rounded = pe_x87_round_ratio_nearest_even(
                magnitude >> discard,
                magnitude & ((1_u128 << discard) - 1),
                1_u128 << discard,
            );
            (rounded, base_exponent + discard as i32)
        } else {
            (magnitude << (64 - bits) as u32, base_exponent - (64 - bits))
        };
        let (significand, exponent) = if significand == (1_u128 << 64) {
            (significand >> 1, exponent + 1)
        } else {
            (significand, exponent)
        };
        Self {
            negative,
            significand: significand as u64,
            exponent,
        }
    }

    fn add_i32(self, value: i32) -> Self {
        self.add(Self::from_i32(value))
    }

    fn to_f64(self) -> f64 {
        if self.significand == 0 {
            return 0.0;
        }
        let mut significand = u128::from(self.significand >> 11);
        let discarded = self.significand & 0x7ff;
        if discarded > 0x400 || (discarded == 0x400 && significand & 1 != 0) {
            significand += 1;
        }
        let mut exponent = self.exponent;
        if significand == (1_u128 << 53) {
            significand >>= 1;
            exponent += 1;
        }
        if !(-1022..=1023).contains(&exponent) {
            let approximate = (self.significand as f64) * 2.0_f64.powi(self.exponent - 63);
            return if self.negative {
                -approximate
            } else {
                approximate
            };
        }
        let fraction = (significand as u64) & ((1_u64 << 52) - 1);
        let sign = u64::from(self.negative) << 63;
        let exponent_bits = (exponent + 1023) as u64;
        f64::from_bits(sign | (exponent_bits << 52) | fraction)
    }

    fn to_f32(self) -> f32 {
        if self.significand == 0 {
            return 0.0;
        }
        let mut significand = u128::from(self.significand >> 40);
        let discarded = self.significand & ((1_u64 << 40) - 1);
        let halfway = 1_u64 << 39;
        if discarded > halfway || (discarded == halfway && significand & 1 != 0) {
            significand += 1;
        }
        let mut exponent = self.exponent;
        if significand == (1_u128 << 24) {
            significand >>= 1;
            exponent += 1;
        }
        if !(-126..=127).contains(&exponent) {
            return self.to_f64() as f32;
        }
        let fraction = (significand as u32) & ((1_u32 << 23) - 1);
        let sign = u32::from(self.negative) << 31;
        let exponent_bits = (exponent + 127) as u32;
        f32::from_bits(sign | (exponent_bits << 23) | fraction)
    }

    fn truncate_to_i32(self) -> i32 {
        if self.significand == 0 {
            return 0;
        }
        let shift = 63 - self.exponent;
        let magnitude = if shift >= 128 {
            0
        } else if shift >= 0 {
            u128::from(self.significand) >> shift as u32
        } else {
            u128::from(self.significand)
                .checked_shl((-shift) as u32)
                .unwrap_or(u128::MAX)
        };
        let signed = if self.negative {
            -(magnitude as i128)
        } else {
            magnitude as i128
        };
        signed as i32
    }
}

fn pe_x87_round_ratio_nearest_even(quotient: u128, remainder: u128, denominator: u128) -> u128 {
    let twice_remainder = remainder * 2;
    if twice_remainder > denominator || (twice_remainder == denominator && quotient & 1 != 0) {
        quotient + 1
    } else {
        quotient
    }
}

fn pe_rci_x87_score(numerator: f64, denominator: i32) -> f64 {
    if denominator <= 0 {
        return numerator / (f64::from(denominator) + 1.0) - 1.0;
    }
    PeX87Value::from_f64(numerator)
        .divided_by_u32(denominator as u32)
        .add_i32(-1)
        .to_f64()
}

fn pe_rci_x87_demand_ftol(score: f64, tax_delta: i32) -> i32 {
    PeX87Value::from_f64(score)
        .multiply_u32(PE_RCI_DEMAND_BALANCE_SCALE as u32)
        .add_i32(tax_delta)
        .truncate_to_i32()
}

/// Mac CODE 8 export 237 publishes demand before its first QuickDraw sample.
/// The three demand words are shared saved fields, but the prior residential
/// snapshot and weighted scan counters belong to the selected engine.
pub(in crate::app) fn mac_publish_monthly_rci_demand(city: &mut City) {
    let pair_totals = city.mac_rci_pair_totals();
    let xbld_counts = std::array::from_fn(|id| city.mac_xbld_tile_count(id as u8));
    let [cap_modifier_1, cap_modifier_2] = city.mac_rci_overlay_cap_counts();
    let candidates = mac_rci_demand_candidates(MacRciCandidateInputs {
        pair_totals,
        previous_residential_total: city.mac_residential_pair_total(),
        current_scalar: city
            .mac_state_misc_u32_at(MISC_NORMAL_POPULATION_OFFSET)
            .unwrap_or(0) as i32,
        prior_industry_modifier: city
            .mac_state_misc_u32_at(MISC_INDUSTRIAL_MIX_BONUS_OFFSET)
            .unwrap_or(0) as i16,
        difficulty: usize::from(city.pe_w_city_difficulty),
        xbld_counts: &xbld_counts,
        cap_modifier_1,
        cap_modifier_2,
    });
    // 0x3868e stores the current pair after reading the old snapshot.
    city.set_mac_residential_pair_total(pair_totals[0]);

    let kinds = [
        RciKind::Residential,
        RciKind::Commercial,
        RciKind::Industrial,
    ];
    let demand = std::array::from_fn::<_, 3, _>(|index| {
        let previous = city
            .misc_u32_at(RCI_DEMAND_MISC_OFFSET + index * 4)
            .unwrap_or(0) as i16;
        let rate = raw_tax_rate_for_city(city, index) as i16;
        let adjusted_rate = pe_rci_ordinance_adjusted_tax_rate(city, kinds[index], i32::from(rate));
        let tax_adjustment = pe_rci_tax_demand_delta(adjusted_rate) as i16;
        let pair = pair_totals[index] as u32;
        let normalized = mac_rci_normalize_demand_candidate(candidates[index], pair);
        let word = mac_rci_demand_pre_clamp_word(normalized, tax_adjustment, previous);
        // Keep the saved word when the native SANE conversion would be
        // exceptional. Its out-of-range result is still unverified.
        let next = if pair == u32::MAX {
            previous
        } else {
            i16::try_from(word).map_or(previous, |word| word.clamp(-2000, 2000))
        };
        i32::from(next) as u32
    });
    city.set_misc_u32_values(RCI_DEMAND_MISC_OFFSET, &demand);
}

/// Mac CODE 8 `export_237` final RCI writer from the already capped candidate.
///
/// `0x38a3a..0x38a64` divides the candidate by its R/C/I pair total plus one,
/// subtracts one in SANE extended precision, and stores the result as single.
/// The final loop `0x38b6e..0x38bd0` multiplies that rounded single by 600,
/// adds the ordinance-table WORD and prior signed demand, truncates toward
/// zero, converts to a signed WORD, then clamps that stored WORD. This helper's
/// result is native-validated only when the pre-clamp value fits in i16; the
/// Mac SANE out-of-range conversion result is not yet pinned.
pub(in crate::app) fn mac_rci_demand_from_candidate(
    candidate: f32,
    pair_total: u32,
    tax_adjustment: i16,
    previous_demand: i16,
) -> i16 {
    mac_rci_demand_from_normalized_candidate(
        mac_rci_normalize_demand_candidate(candidate, pair_total),
        tax_adjustment,
        previous_demand,
    )
}

pub(in crate::app) fn mac_rci_normalize_demand_candidate(candidate: f32, pair_total: u32) -> f32 {
    PeX87Value::from_f64(f64::from(candidate))
        .divided_by_u32(pair_total.wrapping_add(1))
        .add_i32(-1)
        .to_f32()
}

pub(in crate::app) fn mac_rci_demand_from_normalized_candidate(
    normalized: f32,
    tax_adjustment: i16,
    previous_demand: i16,
) -> i16 {
    mac_rci_demand_pre_clamp_word(normalized, tax_adjustment, previous_demand).clamp(-2000, 2000)
        as i16
}

/// Reconstruct the integer value immediately before CODE 8's signed-WORD
/// conversion. Only values in i16 range are currently supported as a native
/// finalizer oracle; out-of-range SANE conversion is implementation-specific.
pub(in crate::app) fn mac_rci_demand_pre_clamp_word(
    normalized: f32,
    tax_adjustment: i16,
    previous_demand: i16,
) -> i32 {
    PeX87Value::from_f64(f64::from(normalized))
        .multiply_u32(600)
        .add_i32(i32::from(tax_adjustment))
        .add_i32(i32::from(previous_demand))
        .truncate_to_i32()
}

/// Inputs consumed by Mac CODE 8 `export_237` when it builds the three
/// pre-normalization R/C/I candidate singles.
pub(in crate::app) struct MacRciCandidateInputs<'a> {
    pub pair_totals: [i32; 3],
    pub previous_residential_total: i32,
    /// The weighted scalar published from this month's six scan counters
    /// before CODE 8 constructs the candidates; this is not the saved scalar
    /// that entered the monthly scan.
    pub current_scalar: i32,
    pub prior_industry_modifier: i16,
    pub difficulty: usize,
    pub xbld_counts: &'a [u16; 256],
    /// Raw unsigned WORD at `fffffe08`, used by CODE 8's commercial cap.
    pub cap_modifier_1: u16,
    /// Raw unsigned WORD at `fffffe06`, used by CODE 8's industrial cap.
    pub cap_modifier_2: u16,
}

/// Reconstruct the three capped CODE 8 demand candidates from explicit state
/// inputs. The original narrows selected intermediates to binary32 (`FX2S`),
/// while the intervening SANE operations retain extended precision; keeping
/// those boundaries explicit is important for the later common finalizer.
pub(in crate::app) fn mac_rci_demand_candidates(inputs: MacRciCandidateInputs<'_>) -> [f32; 3] {
    const DIFFICULTY_MULTIPLIERS: [f32; 4] = [0.0, 1.2, 1.1, 0.95];
    let [residential, commercial, industrial] = inputs.pair_totals;
    let commercial_plus_industrial = PeX87Value::from_i32(commercial)
        .add(PeX87Value::from_i32(industrial))
        .to_f32();
    let ratio_denominator = PeX87Value::from_f64(f64::from(commercial_plus_industrial)).add_i32(1);
    let residential_ratio = PeX87Value::from_i32(inputs.previous_residential_total)
        .divided_by(ratio_denominator)
        .to_f32();

    let residential_jobs = PeX87Value::from_f64(f64::from(commercial_plus_industrial))
        .add_i32(residential / 50)
        .to_f32();
    let count = |id: usize| i32::from(inputs.xbld_counts[id]);

    // Native count-table thresholds use signed integer division before the
    // integer result is loaded into SANE for comparison with each single.
    // The native WORD additions wrap before the cap is widened for the
    // multiply. Preserve that boundary for malformed or saturated saved
    // census values as well as ordinary counts.
    let candidate_0_cap_units = 10u16
        .wrapping_add(count(0xd5) as u16 / 3)
        .wrapping_add(count(0xd7) as u16)
        .wrapping_add(count(0xda) as u16)
        .wrapping_add(count(0xf8) as u16);
    let candidate_0_cap_a = i32::from(candidate_0_cap_units) * 150;
    let candidate_0_cap_b = commercial.saturating_mul(4).saturating_add(500);
    let candidate_0 = mac_x87_min(
        mac_x87_min(
            PeX87Value::from_f64(f64::from(residential_jobs)),
            PeX87Value::from_i32(candidate_0_cap_a),
        ),
        PeX87Value::from_i32(candidate_0_cap_b),
    )
    .to_f32();

    let growth_scale = PeX87Value::from_i32(inputs.current_scalar)
        .add_i32(50_000)
        .divided_by_u32(150_000)
        .to_f32();
    let candidate_1 = PeX87Value::from_i32(industrial)
        .multiplied_by(PeX87Value::from_f64(f64::from(residential_ratio)))
        .multiplied_by(PeX87Value::from_f64(f64::from(growth_scale)));
    let candidate_1_cap_units = inputs
        .cap_modifier_1
        .wrapping_add(count(0xdd) as u16)
        .wrapping_add(count(0xde) as u16);
    let candidate_1_cap = 1_500 * (1 + i32::from(candidate_1_cap_units / 5));
    let candidate_1 = mac_x87_min(candidate_1, PeX87Value::from_i32(candidate_1_cap)).to_f32();

    let difficulty = DIFFICULTY_MULTIPLIERS
        .get(inputs.difficulty)
        .copied()
        .unwrap_or(0.0);
    let industry_factor = PeX87Value::from_i32(i32::from(inputs.prior_industry_modifier))
        .divided_by_u32(100)
        .add(PeX87Value::from_f64(f64::from(difficulty)))
        .to_f32();
    let candidate_2 = PeX87Value::from_i32(industrial)
        .multiplied_by(PeX87Value::from_f64(f64::from(residential_ratio)))
        .multiplied_by(PeX87Value::from_f64(f64::from(industry_factor)));
    let candidate_2 = mac_x87_max(candidate_2, PeX87Value::from_i32(15));
    let candidate_2_cap_units = 1u16
        .wrapping_add(inputs.cap_modifier_2)
        .wrapping_add(count(0xe0) as u16);
    let candidate_2_cap = 1_500 * i32::from(candidate_2_cap_units);
    let candidate_2 = mac_x87_min(candidate_2, PeX87Value::from_i32(candidate_2_cap)).to_f32();

    [candidate_0, candidate_1, candidate_2]
}

fn mac_x87_min(left: PeX87Value, right: PeX87Value) -> PeX87Value {
    if left.compare(right) != std::cmp::Ordering::Greater {
        left
    } else {
        right
    }
}

fn mac_x87_max(left: PeX87Value, right: PeX87Value) -> PeX87Value {
    if left.compare(right) != std::cmp::Ordering::Less {
        left
    } else {
        right
    }
}

/// Win95 `FUN_00468150` zone-balance floats before `* 600` / tax / `_ftol`.
///
/// Prefer published growth-aggregate `pZonePops` rows when day-21 publish has
/// run; otherwise fall back to census people / 10.
pub(in crate::app) fn pe_rci_zone_balance_scores(
    city: &City,
    population: PopulationStats,
) -> [f64; 3] {
    let (r, c, i) = pe_rci_zone_pops_for_demand(city, population);
    let [r_num, c_num, i_num] = pe_rci_zone_balance_numerators(city, population);
    [
        pe_rci_x87_score(r_num, r.wrapping_add(1)),
        pe_rci_x87_score(c_num, c.wrapping_add(1)),
        pe_rci_x87_score(i_num, i.wrapping_add(1)),
    ]
}

pub(in crate::app) fn pe_rci_zone_balance_numerators(
    city: &City,
    population: PopulationStats,
) -> [f64; 3] {
    let (r, c, i) = pe_rci_zone_pops_for_demand(city, population);
    // PE `FUN_00468150` reads last month's residential `pZonePops[0]` from
    // BSS `dwResidentialPopulation` @ `0x4ca194` before replacing it with the
    // current row. That BSS value hydrates from MISC `+0x74` on city load.
    // The PE accumulates C + I in x87, adds 1, then divides the signed prior-R
    // dword. Both numerator builders consume this live extended intermediate.
    let ratio_denominator = PeX87Value::from_i32(c)
        .add(PeX87Value::from_i32(i))
        .add_i32(1);
    let ratio = PeX87Value::from_i32(city.pe_dw_residential_population as i32)
        .divided_by(ratio_denominator);
    // PE `FUN_00468150` @ `0x4682e9`: movsx from BSS `wCityDifficulty` @ `0x4cb404`.
    let difficulty = u32::from(city.pe_w_city_difficulty)
        .min((PE_RCI_DIFFICULTY_FACTORS.len() - 1) as u32) as usize;
    let mix = i32::from(industrial_mix_bonus_for_city(city));

    let r_jobs = r / PE_RCI_RESIDENTIAL_JOBS_DIVISOR + c + i;
    let r_num = r_jobs
        .min(pe_rci_residential_recreation_numerator(city))
        .min(c.saturating_mul(PE_RCI_COMMERCIAL_JOBS_SCALE) + PE_RCI_COMMERCIAL_JOBS_BIAS);

    // PE folds all six R/C/I growth slots into aggregate slot 0 and publishes
    // that total times ten at `0x4681e5`. The commercial numerator reads the
    // resulting `dwCityPopulation` directly, then adds 50,000.
    // At `0x4682a4..0x4682ed`, x87 then computes
    // `* (1/150000) * prior_R/(1+C+I) * I` in instruction order.
    let population_plus_bias = city
        .pe_dw_city_population()
        .wrapping_add(PE_RCI_INDUSTRIAL_POP_BIAS);
    let mut c_num = PeX87Value::from_u32(population_plus_bias)
        .multiplied_by(PeX87Value::from_f64(PE_RCI_INDUSTRIAL_POP_SCALE))
        .multiplied_by(ratio)
        .multiply_u32(i as u32)
        .to_f64();
    c_num = c_num.min(f64::from(pe_rci_commercial_infra_numerator(city)));

    // PE `0x4682dc..0x468300`: `(mix * 0.01 + difficulty) * ratio * I`.
    // The x87 value is stored as a qword only after both multiplications.
    let industrial_factor = PeX87Value::from_i32(mix)
        .multiplied_by(PeX87Value::from_f64(0.01))
        .add(PeX87Value::from_f64(PE_RCI_DIFFICULTY_FACTORS[difficulty]));
    let mut i_num = industrial_factor
        .multiplied_by(ratio)
        .multiply_u32(i as u32)
        .to_f64();
    if i_num < PE_RCI_INDUSTRIAL_SCORE_FLOOR {
        i_num = PE_RCI_INDUSTRIAL_SCORE_FLOOR;
    }
    i_num = i_num.min(f64::from(pe_rci_industrial_infra_numerator(city)));

    [f64::from(r_num), c_num, i_num]
}

pub(in crate::app) fn pe_rci_zone_pops_for_demand(
    city: &City,
    population: PopulationStats,
) -> (i32, i32, i32) {
    if city.pe_zone_pops_ready() {
        let [r, c, i] = city.pe_zone_pops();
        (r, c, i)
    } else {
        (
            pe_rci_zone_pop(population.residential_population),
            pe_rci_zone_pop(population.commercial_population),
            pe_rci_zone_pop(population.industrial_population),
        )
    }
}

/// Win95 `0x468321`: `(big_park/3 + stadium + zoo + marina + 10) * 150`.
/// Building IDs from BSS `dwTileCount` (same census as FUN_00471BC0 @ `0x471e13`).
pub(in crate::app) fn pe_rci_residential_recreation_numerator(city: &City) -> i32 {
    // PE reads WORDs, sign-extends each value, and uses signed IDIV for the
    // big-park third (`0x46832a..0x468336`). Its LEA chain at
    // `0x468339..0x468363` then performs wrapping dword adds and `* 150`.
    let tile_count_word = |building: u8| i32::from(city.pe_dw_tile_count[building as usize] as i16);
    let parks = tile_count_word(BIG_PARK_BUILDING_ID) / 3;
    let stadiums = tile_count_word(STADIUM_BUILDING_ID);
    let zoos = tile_count_word(ZOO_BUILDING_ID);
    let marinas = tile_count_word(MARINA_BUILDING_ID);
    parks
        .wrapping_add(stadiums)
        .wrapping_add(zoos)
        .wrapping_add(marinas)
        .wrapping_add(PE_RCI_RECREATION_BASE)
        .wrapping_mul(PE_RCI_RECREATION_SCALE)
}

/// PE word `0x4cc4d8`: network-band XBLD predicate for durable XTXT `0xfa` markers.
/// The bands match the DestroyBuilding and load-census class checks.
pub(in crate::app) fn pe_xbld_counts_fa_network_overlay_4cc4d8(building: u8) -> bool {
    matches!(
        building,
        0x1d..=0x2b | 0x3f..=0x46 | 0x4b | 0x4c | 0x5d..=0x60
    )
}

/// PE `DAT_004CC4D8` uses the saved map census plus live purchase adjustments.
pub(in crate::app) fn pe_fa_network_overlay_count_4cc4d8(city: &City) -> u32 {
    let mut count = city.pe_road_link_census_delta;
    for y in 0..CITY_HEIGHT {
        for x in 0..CITY_WIDTH {
            if city.text_overlay_at(x, y) != Some(0xfa) {
                continue;
            }
            if city.is_synthetic_trip_marker(x, y) {
                continue;
            }
            if pe_xbld_counts_fa_network_overlay_4cc4d8(city.building_at(x, y)) {
                count = count.wrapping_add(1);
            }
        }
    }
    u32::from(count)
}

/// Win95 `0x4683d5`: `1500 * ((runway + runway_cross + DAT_004CC4D8) / 5 + 1)`.
/// The PE rebuilds `DAT_004CC4D8` from network-class XTXT `0xfa` markers on city
/// open. This census is independent of the geometry edge scan used by traffic.
pub(in crate::app) fn pe_rci_commercial_infra_numerator(city: &City) -> i32 {
    // FUN_00468150 sign-extends both building-count WORDs, zero-extends the
    // network marker WORD, then uses signed IDIV by five and 32-bit LEA math.
    let runway = i32::from(city.pe_dw_tile_count[RUNWAY_BUILDING_ID as usize] as u16 as i16);
    let runway_cross =
        i32::from(city.pe_dw_tile_count[AIRPORT_RUNWAY_CROSS_BUILDING_ID as usize] as u16 as i16);
    let tiles = runway
        .wrapping_add(runway_cross)
        .wrapping_add(pe_fa_network_overlay_count_4cc4d8(city) as i32);
    (tiles / 5)
        .wrapping_add(1)
        .wrapping_mul(PE_RCI_PORT_INFRA_SCALE)
}

/// Win95 `0x468431`: `(crane + neighbor_count + 1) * 1500`.
/// `DAT_004CA3F0` credits accepted Rail and Highway purchases. Its live
/// value is separate from the edge geometry used to synthesize off-map trips.
pub(in crate::app) fn pe_rci_industrial_infra_numerator(city: &City) -> i32 {
    // The crane count is MOVSX WORD; DAT_004CA3F0 is MOVZX WORD.
    let cranes = i32::from(city.pe_dw_tile_count[SEAPORT_CRANE_BUILDING_ID as usize] as u16 as i16);
    cranes
        .wrapping_add(i32::from(city.pe_nonnetwork_neighbor_links))
        .wrapping_add(1)
        .wrapping_mul(PE_RCI_PORT_INFRA_SCALE)
}

/// Win95 adds the channel's general tax-table entry to the x87 score before `_ftol`.
pub(in crate::app) fn pe_rci_demand_monthly_delta(city: &City, kind: RciKind, score: f64) -> i32 {
    let rate = pe_rci_ordinance_adjusted_tax_rate(
        city,
        kind,
        tax_rate_for_city(city, rci_kind_index(kind)) as i32,
    );
    let tax_delta = pe_rci_tax_demand_delta(rate);
    pe_rci_x87_demand_ftol(score, tax_delta)
}

pub(in crate::app) fn commit_pe_rci_previous_residential_zone_pop(
    city: &mut City,
    population: PopulationStats,
) {
    let (r, _, _) = pe_rci_zone_pops_for_demand(city, population);
    // Native `FUN_00468150` writes the just-published residential row back to
    // `0x4ca194` after using its prior value; MISC `+0x74` persists that state.
    city.pe_dw_residential_population = r as u32;
}
