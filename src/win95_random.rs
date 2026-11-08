//! Win95 `SIMCITY.EXE` random helpers used by Things / disaster code.
//!
//! Three distinct generators appear in the PE:
//! - CRT `rand` thunk `0x48cdd8` — MSVC LCG (`seed*0x343FD+0x269EC3`, return `(seed>>16)&0x7FFF`)
//! - Game random helper `0x40116d` — 32-bit LCG state at `0x4cdb80`
//! - Game LFSR `0x401cad` / `0x401415` — word state at `0x4cdb7c`, poly `0x1BF5`

/// Win95 CRT `rand` (`0x48cdd8`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Win95CrtRand {
    seed: u32,
}

/// Win95 game-specific 32-bit LCG used by the helper at `0x47faa0`.
///
/// This is separate from both CRT `rand()` (`0x48cdd8`) and the Game_Random
/// LFSR (`0x47faf0`). The PE seeds its process-global state from
/// `GetTickCount() | 1` in the view constructor (`0x42541d..0x42542b`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Win95GameRand {
    state: u32,
}

impl Win95GameRand {
    pub const DEFAULT_STATE: u32 = 1;

    pub fn new(state: u32) -> Self {
        Self { state }
    }

    pub fn state(self) -> u32 {
        self.state
    }

    /// Win95 `0x40116d(modulus)`: advance `state = state*0x41c64e6d+0x3039`,
    /// return `((state >> 16) & 0x7fff) % modulus`.
    pub fn random_mod(&mut self, modulus: u16) -> u16 {
        self.state = self.state.wrapping_mul(0x41c6_4e6d).wrapping_add(0x3039);
        let value = ((self.state >> 16) & 0x7fff) as u16;
        value % modulus.max(1)
    }
}

impl Win95CrtRand {
    pub const DEFAULT_SEED: u32 = 1;

    pub fn new(seed: u32) -> Self {
        // `srand(0)` is valid and leaves the CRT state at zero. Keep the
        // default TLS value explicit at call sites instead of aliasing zero.
        Self { seed }
    }

    pub fn seed(self) -> u32 {
        self.seed
    }

    /// One `rand()` call — value in `0..=0x7FFF`.
    pub fn random_u15(&mut self) -> u16 {
        self.seed = self.seed.wrapping_mul(0x343F_D).wrapping_add(0x0026_9EC3);
        ((self.seed >> 16) & 0x7FFF) as u16
    }

    pub fn random_u16(&mut self) -> u16 {
        self.random_u15()
    }

    pub fn random_u16_mask(&mut self, mask: u16) -> u16 {
        self.random_u15() & mask
    }

    /// `rand() % modulus` (MSVC idiv remainder), for helpers like zoo `0x40116d`.
    pub fn random_mod(&mut self, modulus: u16) -> u16 {
        let modulus = modulus.max(1);
        self.random_u15() % modulus
    }

    /// Bounded percentage helper for callers that already normalized funding
    /// to 0..=100. The PE's raw active-document WORD path uses
    /// `roll_pe_underfunding_decay` instead.
    pub fn roll_underfunding_decay(&mut self, funding_percent: u32) -> bool {
        let funding = funding_percent.min(100);
        if funding >= 100 {
            return false;
        }
        u32::from(self.random_u15() % 100) >= funding
    }

    /// Bounded bridge helper for callers that already normalized funding.
    /// The PE's raw active-document WORD path uses
    /// `roll_pe_bridge_underfunding_decay` instead.
    pub fn roll_bridge_underfunding_decay(&mut self, funding_percent: u32, wind: u32) -> bool {
        let funding = funding_percent.min(100);
        if funding >= 100 {
            return false;
        }
        wind.saturating_add(u32::from(self.random_u15() % 50)) >= funding
    }

    /// Win95 surface and subway upkeep raw WORD behavior. Funding 100 exits
    /// before `rand()`; all other values consume one CRT draw and compare its
    /// nonnegative remainder against the sign-extended funding WORD with the
    /// PE's signed `JL` branch.
    pub fn roll_pe_underfunding_decay(&mut self, funding_word: u16) -> bool {
        if funding_word == 100 {
            return false;
        }
        let roll = i32::from(self.random_u15() % 100);
        roll >= i32::from(funding_word as i16)
    }

    /// Win95 bridge budget check: after the 100% early exit, consume one CRT
    /// draw and compare `wind + rand() % 50` against the sign-extended funding
    /// WORD using the PE's signed `JL` branch.
    pub fn roll_pe_bridge_underfunding_decay(&mut self, funding_word: u16, wind: u8) -> bool {
        if funding_word == 100 {
            return false;
        }
        let total = i32::from(wind) + i32::from(self.random_u15() % 50);
        total >= i32::from(funding_word as i16)
    }

    /// Win95 `GetRubbleTileID`: `(rand() & 3) + 1`.
    pub fn rubble_tile_id(&mut self) -> u8 {
        (self.random_u15() & 3) as u8 + 1
    }
}

/// Win95 game LFSR (`0x47faf0` / `0x47fc70`, state `0x4cdb7c`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Win95Lfsr {
    state: u16,
}

impl Win95Lfsr {
    pub const DEFAULT_STATE: u16 = 1;
    const POLY: u16 = 0x1BF5;

    pub fn new(state: u16) -> Self {
        Self {
            state: if state == 0 {
                Self::DEFAULT_STATE
            } else {
                state
            },
        }
    }

    pub fn from_u32_seed(seed: u32) -> Self {
        Self::new(seed as u16)
    }

    pub fn state(self) -> u16 {
        self.state
    }

    /// Advance the LFSR word once (`0x47faf0` / `0x47fc70` shared prelude).
    fn advance(&mut self) {
        let mut ax = self.state;
        if ax & 0x8000 != 0 {
            ax = ax.wrapping_shl(1) ^ Self::POLY;
        } else {
            ax = ax.wrapping_shl(1);
        }
        self.state = ax;
    }

    /// Win95 `0x401cad(modulus)` — returns `lfsr % modulus`.
    pub fn random_mod(&mut self, modulus: u16) -> u16 {
        let modulus = modulus.max(1);
        self.advance();
        self.state % modulus
    }

    /// Win95 `0x401415` — returns `lfsr & 0x3f`.
    pub fn random_mod64(&mut self) -> u16 {
        self.advance();
        u16::from(self.state & 0x3f)
    }

    /// Win95 `0x402b3f` → `0x47fcb0`: advance LFSR, return `state & 0x7f` (`RandomAnd7F`).
    pub fn random_mask_7f(&mut self) -> u16 {
        self.advance();
        self.state & 0x7f
    }

    /// Win95 `0x4025e5` → `0x47fbf0`: advance LFSR, return `state & 0xf`.
    pub fn random_mask_0f(&mut self) -> u16 {
        self.advance();
        self.state & 0xf
    }

    /// Win95 `0x402261` → `0x47fb70`: advance LFSR, return `state & 3` (`RandomAnd3`).
    pub fn random_mask_03(&mut self) -> u16 {
        self.advance();
        self.state & 3
    }

    /// Win95 `0x402946` → `0x47fb30`: advance LFSR, return low bit (`RandomWordLFSR & 1`).
    pub fn random_word_lsb(&mut self) -> u16 {
        self.advance();
        self.state & 1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crt_rand_matches_msvc_reference_sequence() {
        let mut rng = Win95CrtRand::new(1);
        // Hand-checked against `seed = seed*0x343FD+0x269EC3; (seed>>16)&0x7FFF`.
        let expected = [0x0029u16, 0x4823, 0x18BE, 0x6784, 0x4AE1];
        for want in expected {
            assert_eq!(rng.random_u15(), want);
        }
    }

    #[test]
    fn pe_transport_funding_uses_raw_signed_word_and_exact_draw_gate() {
        // PE @ 0x435b04 / 0x435a75: only the literal WORD 100 skips rand.
        // The CRT seed 1 produces 41 on the first draw.
        let mut full = Win95CrtRand::new(1);
        assert!(!full.roll_pe_underfunding_decay(100));
        assert_eq!(full.seed(), 1, "100% funding exits before CRT rand");

        let mut above_range = Win95CrtRand::new(1);
        assert!(!above_range.roll_pe_underfunding_decay(120));
        assert_eq!(
            above_range.seed(),
            1_u32.wrapping_mul(0x343f_d).wrapping_add(0x269e_c3)
        );

        let mut signed_word = Win95CrtRand::new(1);
        assert!(signed_word.roll_pe_underfunding_decay(0x8000));
        assert_eq!(
            signed_word.seed(),
            1_u32.wrapping_mul(0x343f_d).wrapping_add(0x269e_c3)
        );
    }

    #[test]
    fn pe_bridge_funding_uses_raw_signed_word_and_wind_threshold() {
        // PE @ 0x435c87..0x435cb6: literal 100 exits before rand; otherwise
        // compare the unsigned nonnegative sum against a signed funding WORD.
        let mut full = Win95CrtRand::new(1);
        assert!(!full.roll_pe_bridge_underfunding_decay(100, 255));
        assert_eq!(full.seed(), 1);

        let mut raw_120 = Win95CrtRand::new(1);
        // First rand is 41, so 60 + (41 % 50) = 101 < 120.
        assert!(!raw_120.roll_pe_bridge_underfunding_decay(120, 60));
        assert_eq!(
            raw_120.seed(),
            1_u32.wrapping_mul(0x343f_d).wrapping_add(0x269e_c3)
        );

        let mut equality = Win95CrtRand::new(1);
        assert!(equality.roll_pe_bridge_underfunding_decay(101, 60));

        let mut signed_word = Win95CrtRand::new(1);
        assert!(signed_word.roll_pe_bridge_underfunding_decay(0x8000, 0));
    }

    #[test]
    fn srand_zero_keeps_the_zero_seed() {
        let mut rng = Win95CrtRand::new(0);
        assert_eq!(rng.seed(), 0);
        assert_eq!(rng.random_u15(), (0x0026_9ec3_u32 >> 16) as u16);
    }

    #[test]
    fn lfsr_mod_matches_pe_shift_xor_poly() {
        let mut rng = Win95Lfsr::new(0x1234);
        // state 0x1234: bit15 clear -> <<1 = 0x2468; 0x2468 % 100 = 4
        assert_eq!(rng.random_mod(100), 0x2468 % 100);
        assert_eq!(rng.state(), 0x2468);

        // Next: 0x2468 bit15 clear -> 0x48D0
        assert_eq!(rng.random_mod(100), 0x48D0 % 100);

        // Force poly path: state with bit15 set.
        let mut hi = Win95Lfsr::new(0x8001);
        // (0x8001<<1) ^ 0x1BF5 = 0x0002 ^ 0x1BF5 = 0x1BF7
        assert_eq!(hi.random_mod(0x64), 0x1BF7 % 0x64);
        assert_eq!(hi.state(), 0x1BF7);
    }

    #[test]
    fn lfsr_mod64_masks_low_6_bits() {
        let mut rng = Win95Lfsr::new(0x00FF);
        // <<1 = 0x01FE; & 0x3f = 0x3E
        assert_eq!(rng.random_mod64(), 0x3E);
    }

    #[test]
    fn lfsr_random_word_lsb_returns_low_bit_after_advance() {
        let mut rng = Win95Lfsr::new(0x1234);
        // <<1 = 0x2468; lsb = 0
        assert_eq!(rng.random_word_lsb(), 0);
        assert_eq!(rng.state(), 0x2468);
        // <<1 = 0x48D0; lsb = 0
        assert_eq!(rng.random_word_lsb(), 0);
        // Force odd: start 0x8001 -> 0x1BF7; lsb = 1
        let mut hi = Win95Lfsr::new(0x8001);
        assert_eq!(hi.random_word_lsb(), 1);
        assert_eq!(hi.state(), 0x1BF7);
    }

    #[test]
    fn game_rand_matches_pe_sibling_lcg_and_is_independent_from_crt() {
        let mut game = Win95GameRand::new(1);
        let first_state = 1_u32.wrapping_mul(0x41c6_4e6d).wrapping_add(0x3039);
        let first_value = ((first_state >> 16) & 0x7fff) as u16;
        assert_eq!(game.random_mod(100), first_value % 100);
        assert_eq!(game.state(), first_state);

        let mut crt = Win95CrtRand::new(1);
        let crt_state = 1_u32.wrapping_mul(0x343f_d).wrapping_add(0x269e_c3);
        assert_eq!(crt.random_u15(), ((crt_state >> 16) & 0x7fff) as u16);
        assert_ne!(game.state(), crt.seed());
    }

    #[test]
    fn lfsr_random_mask_7f_matches_pe_402b3f() {
        let mut rng = Win95Lfsr::new(0x00FF);
        // <<1 = 0x01FE; & 0x7f = 0x7E
        assert_eq!(rng.random_mask_7f(), 0x7E);
    }

    #[test]
    fn lfsr_random_mask_0f_matches_pe_4025e5() {
        let mut rng = Win95Lfsr::new(0x00FF);
        // <<1 = 0x01FE; & 0xf = 0xE
        assert_eq!(rng.random_mask_0f(), 0xE);
        assert_eq!(rng.state(), 0x01FE);
    }

    #[test]
    fn lfsr_random_mask_03_matches_pe_402261() {
        let mut rng = Win95Lfsr::new(0x00FF);
        // <<1 = 0x01FE; & 3 = 2
        assert_eq!(rng.random_mask_03(), 2);
        assert_eq!(rng.state(), 0x01FE);
    }
}
