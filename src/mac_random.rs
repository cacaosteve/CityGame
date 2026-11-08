//! Classic Mac Toolbox `_Random` trap (`0xA861`).
//!
//! SimCity 2000 traffic blits consume sequential `Random` values during the
//! `export_80` dash pass (`Random & 3` for shape variant, `Random & 1` for flip).

/// Mac Toolbox random number generator.
///
/// Authoritative QuickDraw source: `supermario/QuickDraw/QDUtil.a`, `Random FUNC EXPORT`.
/// The ROM trap advances `randSeed = (randSeed * 16807) mod 2147483647`, then
/// returns the signed low word, with `-32768` remapped to `0`.
#[derive(Debug, Clone, Copy)]
pub struct MacToolboxRng {
    seed: u32,
}

impl MacToolboxRng {
    /// QuickDraw `InitGraf` initializes its low-memory `randSeed` global to 1.
    /// Later `_Random` calls share and advance that process-global state.
    pub const QUICKDRAW_INIT_GRAF_SEED: u32 = 1;

    /// Deterministic CityGame seed used where the original stream position is
    /// unavailable. This is not the original post-startup QuickDraw state.
    pub const DEFAULT_SEED: u32 = 0x9a3e_b231;
    const MODULUS: u32 = 2_147_483_647;
    const MULTIPLIER: u64 = 16_807;

    /// Construct from a QuickDraw `RandSeed` value, reduced modulo `2^31 - 1`.
    /// Zero is a valid fixed point of `_Random` and remains zero.
    pub fn new(seed: u32) -> Self {
        let seed = seed % Self::MODULUS;
        Self { seed }
    }

    /// Seed for one map draw pass.
    pub fn for_traffic_draw(city_name: &str, rotation: u8, redraw_epoch: u32) -> Self {
        let mut seed = 0x5A5A_1234u32;
        for byte in city_name.bytes() {
            seed = seed.wrapping_mul(33).wrapping_add(u32::from(byte));
        }
        seed ^= u32::from(rotation) << 8;
        seed ^= redraw_epoch.wrapping_mul(0x9E37_79B9);
        Self::new(seed)
    }

    pub fn seed(self) -> u32 {
        self.seed
    }

    /// One `_Random` trap call — signed 16-bit result, seed advanced.
    pub fn random_i16(&mut self) -> i16 {
        self.seed = ((u64::from(self.seed) * Self::MULTIPLIER) % u64::from(Self::MODULUS)) as u32;
        let value = self.seed as u16 as i16;
        if value == i16::MIN { 0 } else { value }
    }

    /// `Random & 3` dash variant index (`export_80` @ `0x6c78`).
    pub fn random_u3(&mut self) -> u8 {
        (self.random_i16() as u16 & 3) as u8
    }

    /// `Random & 1` blit flip / heavy phase (`jsr 0x55a` arg 4).
    pub fn random_bit(&mut self) -> bool {
        self.random_i16() & 1 != 0
    }

    pub fn random_u16(&mut self) -> u16 {
        self.random_i16() as u16
    }

    /// Approximate Win95 1-in-128 probe via Toolbox `Random` (not the PE LFSR at `0x4cdb7c`).
    pub fn roll_mod_128_is_zero(&mut self) -> bool {
        self.random_u16() % 128 == 0
    }

    /// Approximate Win95 1-in-4 probe via Toolbox `Random` (not the PE LFSR at `0x4cdb7c`).
    pub fn roll_mod_4_is_zero(&mut self) -> bool {
        self.random_u16() % 4 == 0
    }

    /// Win95 `(unsigned rand() % 100) >= funding` underfunding decay.
    pub fn roll_underfunding_decay(&mut self, funding_percent: u32) -> bool {
        let funding = funding_percent.min(100);
        if funding >= 100 {
            return false;
        }
        u32::from(self.random_u16() % 100) >= funding
    }

    /// Mac CODE 8 bridge upkeep: `funding_word <= wind + Random % 50`.
    /// The caller skips only an exact funding word of 100; other words consume
    /// this draw even when the comparison cannot pass.
    pub fn roll_bridge_underfunding_decay(&mut self, funding_word: u16, wind: u32) -> bool {
        let roll = u32::from(self.random_u16() % 50);
        u32::from(funding_word) <= wind.saturating_add(roll)
    }

    /// Win95 `GetRubbleTileID`: `(rand() & 3) + 1`.
    pub fn rubble_tile_id(&mut self) -> u8 {
        self.random_u3() + 1
    }

    /// Win95 `(rand() & 3) != 0` spawn-vs-growth gate (75% spawn branch).
    pub fn roll_ambient_spawn_branch(&mut self) -> bool {
        (self.random_u16() & 3) != 0
    }

    /// Win95 `rand() % modulus == 0` (modulus must be > 0).
    pub fn roll_mod_is_zero(&mut self, modulus: u16) -> bool {
        let modulus = modulus.max(1);
        self.random_u16() % modulus == 0
    }

    /// Win95 `rand() % modulus < threshold`.
    pub fn roll_mod_less_than(&mut self, modulus: u16, threshold: u16) -> bool {
        let modulus = modulus.max(1);
        self.random_u16() % modulus < threshold
    }
}

/// SimCity's separate app-owned Mac game-LCG (`CODE 13` export 201).
///
/// This is not QuickDraw `_Random`: it advances a full 32-bit state and returns
/// a bounded value from the high 15 bits. The reconstructed A5 data image starts
/// the process state at 1, though live startup and prior-call chronology remain
/// outside this type's scope.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MacGameLcgRng {
    state: u32,
}

impl MacGameLcgRng {
    pub const INITIAL_STATE: u32 = 1;

    /// Process state immediately after the Mac application's CODE 15 startup
    /// playlist selects one of its five SONGs through CODE 13 export 201.
    /// CityGame may play replacement music, but that startup selector still
    /// advances the native app-owned stream before Mac gameplay begins.
    pub fn after_startup_playlist_choice() -> Self {
        let mut rng = Self::new(Self::INITIAL_STATE);
        rng.bounded(5);
        rng
    }

    pub const fn new(state: u32) -> Self {
        Self { state }
    }

    pub const fn state(self) -> u32 {
        self.state
    }

    /// One CODE 13 export 201 call: advance the LCG, then divide its high
    /// 15-bit sample by the caller's nonzero bound.
    pub fn bounded(&mut self, bound: u16) -> u16 {
        assert_ne!(bound, 0, "Mac game-LCG bound must be nonzero");
        self.state = self.state.wrapping_mul(0x41c6_4e6d).wrapping_add(0x3039);
        (((self.state >> 16) & 0x7fff) as u16) % bound
    }
}

/// SimCity 2000 Mac CODE 13 exports 203–208 share this app-owned 16-bit LFSR
/// at A5-2. The reconstructed Mac 1.2 A5 world initializes it to zero, and
/// the loaded CODE census finds only these exports reading or writing it, so
/// the original stream remains at zero. Keep this type separate from the PE
/// LFSR and QuickDraw `_Random` state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MacAppLfsrRng {
    state: u16,
}

impl MacAppLfsrRng {
    pub const INITIAL_STATE: u16 = 0;
    const POLY: u16 = 0x1bf5;

    pub const fn new(state: u16) -> Self {
        Self { state }
    }

    pub const fn state(self) -> u16 {
        self.state
    }

    /// CODE 13 export 203: advance, then return the unsigned word modulo bound.
    pub fn bounded(&mut self, bound: u16) -> u16 {
        assert_ne!(bound, 0, "Mac app-LFSR bound must be nonzero");
        self.advance();
        self.state % bound
    }

    /// CODE 13 export 205: advance, then return the low two bits.
    pub fn low_two_bits(&mut self) -> u16 {
        self.advance();
        self.state & 3
    }

    /// CODE 13 export 208: advance, then return the low seven bits.
    pub fn low_seven_bits(&mut self) -> u16 {
        self.advance();
        self.state & 0x7f
    }

    fn advance(&mut self) {
        let high_bit = self.state & 0x8000 != 0;
        self.state = self.state.wrapping_shl(1);
        if high_bit {
            self.state ^= Self::POLY;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mac_game_lcg_matches_relocated_export_201_sequence() {
        let mut rng = MacGameLcgRng::new(MacGameLcgRng::INITIAL_STATE);
        assert_eq!(rng.bounded(100), 38);
        assert_eq!(rng.state(), 0x41c6_7ea6);
        assert_eq!(rng.bounded(100), 58);
        assert_eq!(rng.state(), 0x967e_b0e7);
    }

    #[test]
    fn mac_startup_playlist_advances_the_process_game_lcg_once() {
        let mut expected = MacGameLcgRng::new(MacGameLcgRng::INITIAL_STATE);
        let song_index = expected.bounded(5);

        assert_eq!(song_index, 3);
        assert_eq!(
            MacGameLcgRng::after_startup_playlist_choice().state(),
            expected.state()
        );
    }

    #[test]
    fn mac_app_lfsr_matches_export_203_and_zero_initialized_stream() {
        let mut from_one = MacAppLfsrRng::new(1);
        assert_eq!(from_one.bounded(20), 2);
        assert_eq!(from_one.state(), 2);
        let mut from_high_bit = MacAppLfsrRng::new(0x8000);
        assert_eq!(from_high_bit.low_two_bits(), 0x1bf5 & 3);
        assert_eq!(from_high_bit.state(), 0x1bf5);
        let mut export_208_from_high_bit = MacAppLfsrRng::new(0x8000);
        assert_eq!(export_208_from_high_bit.low_seven_bits(), 0x1bf5 & 0x7f);
        assert_eq!(export_208_from_high_bit.state(), 0x1bf5);

        // DATA/ZERO + DREL + CREL reconstruction gives A5-2 = 0. No loaded
        // CODE writer seeds it, so every native LFSR helper result is zero.
        let mut original_initial_state = MacAppLfsrRng::new(MacAppLfsrRng::INITIAL_STATE);
        for _ in 0..16 {
            assert_eq!(original_initial_state.bounded(20), 0);
            assert_eq!(original_initial_state.low_two_bits(), 0);
            assert_eq!(original_initial_state.low_seven_bits(), 0);
        }
        assert_eq!(original_initial_state.state(), 0);
    }

    #[test]
    fn toolbox_rng_is_deterministic() {
        let mut a = MacToolboxRng::new(0x1234_5678);
        let mut b = MacToolboxRng::new(0x1234_5678);
        for _ in 0..8 {
            assert_eq!(a.random_i16(), b.random_i16());
        }
    }

    #[test]
    fn random_u3_stays_in_range() {
        let mut rng = MacToolboxRng::new(42);
        for _ in 0..64 {
            assert!(rng.random_u3() < 4);
        }
    }

    #[test]
    fn toolbox_rng_matches_quickdraw_reference_sequence() {
        // Golden sequence verified BIT-EXACT against the real 68k QuickDraw `Random`
        // (`supermario/QuickDraw/QDUtil.a`, Schrage's method) executed in
        // `resource_dasm`'s `m68kexec` emulator (seed = 1). See research note
        // `mac_random_bitexact_verification_2026_06_23.md`.
        let mut rng = MacToolboxRng::new(MacToolboxRng::QUICKDRAW_INIT_GRAF_SEED);
        let expected = [
            16_807, 15_089, -21_287, 3_114, -18_558, -9_528, -28_968, 2_558, 12_099, 1_101,
            -26_472, 15_445, 4_748, -9_246, -11_085, 14_151, 14_615, 16_657, -15_464, 18_772,
        ];
        for value in expected {
            assert_eq!(rng.random_i16(), value);
        }
    }

    #[test]
    fn toolbox_rng_handles_minus_32768_edge_like_quickdraw() {
        // QDUtil `Random`: `CMP.W #$8000,D0 / BNE / CLR D0` — a low word of 0x8000
        // (-32768) is returned as 0. With seed 0x8000 the first advance yields
        // 0x8000*16807 = 0x20D38000 (< 2^31-1, so no modular reduction), whose low
        // word is 0x8000 -> result 0. Confirmed in m68kexec.
        let mut rng = MacToolboxRng::new(0x8000);
        assert_eq!(rng.random_i16(), 0);
    }

    #[test]
    fn toolbox_rng_preserves_zero_seed_like_quickdraw() {
        // `_Random` multiplies RandSeed by 16807 modulo 2^31-1. A zero seed
        // remains zero; it is not replaced by an application-selected default.
        let mut from_zero = MacToolboxRng::new(0);
        let mut from_modulus = MacToolboxRng::new(MacToolboxRng::MODULUS);
        for _ in 0..4 {
            assert_eq!(from_zero.random_i16(), 0);
            assert_eq!(from_modulus.random_i16(), 0);
        }
        assert_eq!(from_zero.seed(), 0);
        assert_eq!(from_modulus.seed(), 0);
    }

    #[test]
    fn toolbox_rng_ambient_spawn_branch_uses_rand_and_3_mask() {
        let seed = 0x1234_5678;
        let mut raw = MacToolboxRng::new(seed);
        let mut gated = MacToolboxRng::new(seed);
        for _ in 0..128 {
            let value = raw.random_i16();
            let expected = (value as u16 & 3) != 0;
            assert_eq!(gated.roll_ambient_spawn_branch(), expected);
        }
    }

    #[test]
    fn dash_variant_and_flip_consume_in_export80_order() {
        // export_80 @ 0x6c7a..0x6cee: D4 = (Random & 3) + 0x188 (variant first),
        // then D0 = (Random & 1) (flip second). For seed 1: 16807&3=3 -> variant 3,
        // 15089&1=1 -> flip true.
        let mut rng = MacToolboxRng::new(1);
        assert_eq!(rng.random_u3(), 3);
        assert!(rng.random_bit());
    }
}
