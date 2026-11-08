//! Saved newspaper selection and PE MISC paper/story-list initialization.
//! Event conditions and authored story presentation live in separate modules.

use super::*;

pub(super) fn newspaper_open_choice_for_edition(city: &City, edition: NewspaperEdition) -> u32 {
    let max_choice = newspaper_max_choice_for_city(city);
    match edition {
        NewspaperEdition::Courier if max_choice >= NEWSPAPER_COURIER_CHOICE => {
            NEWSPAPER_COURIER_CHOICE
        }
        _ => newspaper_choice_for_city(city).min(max_choice),
    }
}

pub(super) fn newspaper_paper_name(choice: u32) -> &'static str {
    NEWSPAPER_PAPER_NAMES[(choice.min(NEWSPAPER_CHOICE_MAX)) as usize]
}

/// Win95 `0x479736`: paper record `i` is `[i, i%3, i%3, i, i]`.
pub(super) fn pe_newspaper_paper_list_identity_bytes() -> [u8; NEWSPAPER_LIST_MISC_DWORDS] {
    let mut recs = [0u8; NEWSPAPER_LIST_MISC_DWORDS];
    for index in 0..NEWSPAPER_LIST_RECORD_COUNT as u8 {
        let rec = usize::from(index) * NEWSPAPER_LIST_RECORD_BYTES;
        recs[rec] = index;
        recs[rec + 1] = index % 3;
        recs[rec + 2] = index % 3;
        recs[rec + 3] = index;
        recs[rec + 4] = index;
    }
    recs
}

fn pe_newspaper_shuffle_seed(city: &City) -> u32 {
    let mut seed = 0x4797_3000u32;
    for byte in city.name.bytes() {
        seed = seed.wrapping_mul(33).wrapping_add(u32::from(byte));
    }
    seed
}

pub(super) trait PeNewspaperRandom {
    fn random_u16(&mut self) -> u16;
}

impl PeNewspaperRandom for MacToolboxRng {
    fn random_u16(&mut self) -> u16 {
        MacToolboxRng::random_u16(self)
    }
}

impl PeNewspaperRandom for Win95CrtRand {
    fn random_u16(&mut self) -> u16 {
        Win95CrtRand::random_u16(self)
    }
}

pub(super) fn pe_newspaper_random_mod<R: PeNewspaperRandom>(rng: &mut R, modulus: u16) -> usize {
    usize::from(rng.random_u16() & 0x7fff) % usize::from(modulus)
}

/// Win95 `0x47977c`: `_Random` pairwise byte swaps via `0x402a90`.
pub(super) fn pe_newspaper_paper_list_shuffled_bytes<R: PeNewspaperRandom>(
    rng: &mut R,
) -> [u8; NEWSPAPER_LIST_MISC_DWORDS] {
    let mut recs = pe_newspaper_paper_list_identity_bytes();
    let bit = usize::from(rng.random_u16() & 1);
    recs.swap(3, 0x12 + bit * NEWSPAPER_LIST_RECORD_BYTES);
    if rng.random_u16() & 1 != 0 {
        recs.swap(1, 6);
    }
    for _ in 0..PE_NEWSPAPER_SHUFFLE_ROUNDS {
        let a = pe_newspaper_random_mod(rng, 6) * NEWSPAPER_LIST_RECORD_BYTES;
        let b = pe_newspaper_random_mod(rng, 6) * NEWSPAPER_LIST_RECORD_BYTES;
        recs.swap(a, b);
        let a = pe_newspaper_random_mod(rng, 5) * NEWSPAPER_LIST_RECORD_BYTES + 6;
        let b = pe_newspaper_random_mod(rng, 5) * NEWSPAPER_LIST_RECORD_BYTES + 6;
        recs.swap(a, b);
        let a = pe_newspaper_random_mod(rng, 6) * NEWSPAPER_LIST_RECORD_BYTES + 2;
        let b = pe_newspaper_random_mod(rng, 6) * NEWSPAPER_LIST_RECORD_BYTES + 2;
        recs.swap(a, b);
        let a = pe_newspaper_random_mod(rng, 5) * NEWSPAPER_LIST_RECORD_BYTES + 8;
        let b = pe_newspaper_random_mod(rng, 5) * NEWSPAPER_LIST_RECORD_BYTES + 8;
        recs.swap(a, b);
        let a = pe_newspaper_random_mod(rng, 6) * NEWSPAPER_LIST_RECORD_BYTES + 4;
        let b = pe_newspaper_random_mod(rng, 6) * NEWSPAPER_LIST_RECORD_BYTES + 4;
        recs.swap(a, b);
    }
    recs
}

pub(super) fn pe_newspaper_paper_list_misc_values_with_rng(
    crt: &mut Win95CrtRand,
) -> [u32; NEWSPAPER_LIST_MISC_DWORDS] {
    let recs = pe_newspaper_paper_list_shuffled_bytes(crt);
    let mut values = [0u32; NEWSPAPER_LIST_MISC_DWORDS];
    for (index, byte) in recs.iter().copied().enumerate() {
        values[index] = u32::from(byte);
    }
    values
}

pub(super) fn pe_newspaper_paper_list_misc_values(
    city: &City,
) -> [u32; NEWSPAPER_LIST_MISC_DWORDS] {
    let mut rng = Win95CrtRand::new(pe_newspaper_shuffle_seed(city));
    pe_newspaper_paper_list_misc_values_with_rng(&mut rng)
}

/// Win95 `0x479938`: story record `i` is word `i+0x0B`, word `0`, then `0, 0xFF, 0xFF, 0xFF`.
pub(super) fn pe_newspaper_story_list_misc_values() -> [u32; NEWSPAPER_STORY_LIST_MISC_DWORDS] {
    let mut values = [0u32; NEWSPAPER_STORY_LIST_MISC_DWORDS];
    for index in 0..NEWSPAPER_STORY_RECORD_COUNT as u32 {
        let rec = index as usize * NEWSPAPER_STORY_MISC_FIELDS;
        values[rec] = index + 0x0B;
        values[rec + 1] = 0;
        values[rec + 2] = 0;
        values[rec + 3] = 0xFF;
        values[rec + 4] = 0xFF;
        values[rec + 5] = 0xFF;
    }
    values
}
