//! City option persistence and default initialization.
//!
//! Engine/container storage guards, Mac low-byte semantics, missing-value defaults
//! and initialization/RNG order retain their existing behavior. Menu routing,
//! playback, native newspaper tables and simulation decisions remain with their
//! existing callers and workers.

use super::{
    CITY_DIFFICULTY_HARD, CITY_DIFFICULTY_MEDIUM, MISC_DIFFICULTY_OFFSET,
    MISC_NEWSPAPER_LIST_OFFSET, MISC_NEWSPAPER_STORY_LIST_OFFSET, NEWSPAPER_CHOICE_MAX,
    NEWSPAPER_LIST_MISC_DWORDS, NEWSPAPER_STORY_LIST_MISC_DWORDS,
    ensure_budget_misc_defaults_before_extension, ensure_industry_misc_defaults_before_extension,
    pe_misc_mirror_storage_active, pe_newspaper_paper_list_misc_values,
    pe_newspaper_paper_list_misc_values_with_rng, pe_newspaper_story_list_misc_values,
};
use crate::city::{City, CityFileKind};
use crate::engine_fidelity::EngineFidelity;
use crate::win95_random::Win95CrtRand;

pub(super) const AUTO_GOTO_MISC_OFFSET: usize = 0x0ff4;

pub(super) const SOUND_EFFECTS_MISC_OFFSET: usize = 0x0ff8;

pub(super) const MUSIC_MISC_OFFSET: usize = 0x0ffc;

pub(super) const NO_DISASTERS_MISC_OFFSET: usize = 0x1000;

pub(super) fn city_difficulty_for_city(city: &City) -> u32 {
    match u32::from(city.pe_w_city_difficulty) {
        0 => CITY_DIFFICULTY_MEDIUM,
        other => other.min(CITY_DIFFICULTY_HARD),
    }
}

/// Stock SC2K `bNewspaperSubscription` (`0x4CB3D0`).
pub(super) const MISC_NEWSPAPER_SUBSCRIPTION_OFFSET: usize = 0x1004;

/// Stock SC2K `bNewspaperExtra` (`0x4CC4BC`).
pub(super) const MISC_NEWSPAPER_EXTRA_OFFSET: usize = 0x1008;

/// Stock SC2K `wNewspaperChoice` (`0x4CA808`).
pub(super) const MISC_NEWSPAPER_CHOICE_OFFSET: usize = 0x100C;

pub(super) const CITY_OPTION_ENABLED_VALUE: u32 = 1;

pub(super) const CITY_OPTION_DISABLED_VALUE: u32 = 0;

fn city_option_enabled_for_city(city: &City, offset: usize, default_enabled: bool) -> bool {
    if city.kind == CityFileKind::MacCity {
        return default_enabled;
    }
    city.misc_u32_at(offset)
        .map_or(default_enabled, |value| value != CITY_OPTION_DISABLED_VALUE)
}

fn set_city_option_enabled(
    city: &mut City,
    offset: usize,
    enabled: bool,
    default_enabled: bool,
) -> bool {
    set_city_option_enabled_with_crt(city, offset, enabled, default_enabled, None)
}

fn set_city_option_enabled_with_crt(
    city: &mut City,
    offset: usize,
    enabled: bool,
    default_enabled: bool,
    crt: Option<&mut Win95CrtRand>,
) -> bool {
    if city.kind == CityFileKind::MacCity {
        return false;
    }
    let next = if enabled {
        CITY_OPTION_ENABLED_VALUE
    } else {
        CITY_OPTION_DISABLED_VALUE
    };
    if city_option_enabled_for_city(city, offset, default_enabled) == enabled {
        return false;
    }
    ensure_budget_misc_defaults_before_extension(city);
    ensure_newspaper_list_misc_defaults_before_extension(city, crt);
    city.set_misc_u32_at(offset, next);
    true
}

pub(super) fn auto_goto_enabled_for_city(city: &City) -> bool {
    city_option_enabled_for_city(city, AUTO_GOTO_MISC_OFFSET, true)
}

pub(super) fn auto_goto_enabled_for_engine(city: &City, fidelity: EngineFidelity) -> bool {
    option_enabled_for_engine(city, fidelity, AUTO_GOTO_MISC_OFFSET, true)
}

pub(super) fn set_city_auto_goto_enabled(city: &mut City, enabled: bool) -> bool {
    set_city_option_enabled(city, AUTO_GOTO_MISC_OFFSET, enabled, true)
}

pub(super) fn set_auto_goto_enabled_for_engine(
    city: &mut City,
    fidelity: EngineFidelity,
    enabled: bool,
) -> bool {
    set_option_enabled_for_engine(city, fidelity, AUTO_GOTO_MISC_OFFSET, enabled, true)
}

pub(super) fn sound_effects_enabled_for_city(city: &City) -> bool {
    city_option_enabled_for_city(city, SOUND_EFFECTS_MISC_OFFSET, true)
}

pub(super) fn sound_effects_enabled_for_engine(city: &City, fidelity: EngineFidelity) -> bool {
    option_enabled_for_engine(city, fidelity, SOUND_EFFECTS_MISC_OFFSET, true)
}

pub(super) fn set_city_sound_effects_enabled(city: &mut City, enabled: bool) -> bool {
    set_city_option_enabled(city, SOUND_EFFECTS_MISC_OFFSET, enabled, true)
}

pub(super) fn set_sound_effects_enabled_for_engine(
    city: &mut City,
    fidelity: EngineFidelity,
    enabled: bool,
) -> bool {
    set_option_enabled_for_engine(city, fidelity, SOUND_EFFECTS_MISC_OFFSET, enabled, true)
}

pub(super) fn music_enabled_for_city(city: &City) -> bool {
    city_option_enabled_for_city(city, MUSIC_MISC_OFFSET, true)
}

pub(super) fn music_enabled_for_engine(city: &City, fidelity: EngineFidelity) -> bool {
    option_enabled_for_engine(city, fidelity, MUSIC_MISC_OFFSET, true)
}

pub(super) fn set_city_music_enabled(city: &mut City, enabled: bool) -> bool {
    set_city_option_enabled(city, MUSIC_MISC_OFFSET, enabled, true)
}

pub(super) fn set_music_enabled_for_engine(
    city: &mut City,
    fidelity: EngineFidelity,
    enabled: bool,
) -> bool {
    set_option_enabled_for_engine(city, fidelity, MUSIC_MISC_OFFSET, enabled, true)
}

fn option_enabled_for_engine(
    city: &City,
    fidelity: EngineFidelity,
    offset: usize,
    default_enabled: bool,
) -> bool {
    match (fidelity, city.kind) {
        (EngineFidelity::Mac68k, CityFileKind::MacCity) => city
            .mac_state_misc_u32_at(offset)
            .map(|value| value as u8 != 0)
            .unwrap_or(default_enabled),
        (EngineFidelity::Win95, _) => city_option_enabled_for_city(city, offset, default_enabled),
        (EngineFidelity::Mac68k, _) => default_enabled,
    }
}

fn set_option_enabled_for_engine(
    city: &mut City,
    fidelity: EngineFidelity,
    offset: usize,
    enabled: bool,
    default_enabled: bool,
) -> bool {
    match (fidelity, city.kind) {
        (EngineFidelity::Mac68k, CityFileKind::MacCity) => {
            if option_enabled_for_engine(city, fidelity, offset, default_enabled) == enabled {
                return false;
            }
            // CODE 4 restores each option's low byte and zero-extends it on save.
            city.set_mac_state_misc_u32_at(offset, u32::from(enabled));
            true
        }
        (EngineFidelity::Win95, _) => {
            set_city_option_enabled(city, offset, enabled, default_enabled)
        }
        (EngineFidelity::Mac68k, _) => false,
    }
}

pub(super) fn no_disasters_enabled_for_city(city: &City) -> bool {
    if city.kind == CityFileKind::MacCity {
        // Mac CODE 4 restores only the low byte into `ffffd60a`.
        city.mac_state_misc_u32_at(NO_DISASTERS_MISC_OFFSET)
            .is_some_and(|value| value as u8 != CITY_OPTION_DISABLED_VALUE as u8)
    } else {
        city.misc_u32_at(NO_DISASTERS_MISC_OFFSET)
            .is_some_and(|value| value != CITY_OPTION_DISABLED_VALUE)
    }
}

/// Read the shared No Disasters option with the selected engine's native
/// width. Mac CODE 4 restores only the low byte even when the city is stored
/// in the Win-layout container; Win95 reads its saved DWORD.
pub(super) fn no_disasters_enabled_for_engine(city: &City, fidelity: EngineFidelity) -> bool {
    if fidelity == EngineFidelity::Mac68k {
        city.mac_state_misc_u32_at(NO_DISASTERS_MISC_OFFSET)
            .is_some_and(|value| value as u8 != CITY_OPTION_DISABLED_VALUE as u8)
    } else {
        no_disasters_enabled_for_city(city)
    }
}

pub(super) fn set_city_no_disasters_enabled(city: &mut City, enabled: bool) -> bool {
    set_no_disasters_enabled_for_engine(city, EngineFidelity::Win95, enabled)
}

pub(super) fn set_no_disasters_enabled_for_engine(
    city: &mut City,
    fidelity: EngineFidelity,
    enabled: bool,
) -> bool {
    if no_disasters_enabled_for_engine(city, fidelity) == enabled {
        return false;
    }

    if city.kind == CityFileKind::MacCity && city.misc_u32_at(NO_DISASTERS_MISC_OFFSET).is_none() {
        return false;
    }

    let value = if enabled {
        CITY_OPTION_ENABLED_VALUE
    } else {
        CITY_OPTION_DISABLED_VALUE
    };
    if city.kind == CityFileKind::MacCity {
        city.set_mac_state_misc_u32_at(NO_DISASTERS_MISC_OFFSET, value);
    } else {
        // This option is shared by both editions. Writing it must not seed the
        // unrelated PE budget/newspaper extension tables: those same offsets
        // carry native Mac state when Mac fidelity is selected.
        city.set_misc_u32_at(NO_DISASTERS_MISC_OFFSET, value);
    }
    true
}

pub(super) fn newspaper_subscription_enabled_for_city(city: &City) -> bool {
    city_option_enabled_for_city(city, MISC_NEWSPAPER_SUBSCRIPTION_OFFSET, false)
}

pub(super) fn set_city_newspaper_subscription_enabled(city: &mut City, enabled: bool) -> bool {
    set_city_newspaper_subscription_enabled_with_crt(city, enabled, None)
}

pub(super) fn set_city_newspaper_subscription_enabled_with_crt(
    city: &mut City,
    enabled: bool,
    crt: Option<&mut Win95CrtRand>,
) -> bool {
    set_city_option_enabled_with_crt(
        city,
        MISC_NEWSPAPER_SUBSCRIPTION_OFFSET,
        enabled,
        false,
        crt,
    )
}

pub(super) fn newspaper_extra_enabled_for_city(city: &City) -> bool {
    city_option_enabled_for_city(city, MISC_NEWSPAPER_EXTRA_OFFSET, false)
}

pub(super) fn set_city_newspaper_extra_enabled(city: &mut City, enabled: bool) -> bool {
    set_city_newspaper_extra_enabled_with_crt(city, enabled, None)
}

pub(super) fn set_city_newspaper_extra_enabled_with_crt(
    city: &mut City,
    enabled: bool,
    crt: Option<&mut Win95CrtRand>,
) -> bool {
    set_city_option_enabled_with_crt(city, MISC_NEWSPAPER_EXTRA_OFFSET, enabled, false, crt)
}

pub(super) fn newspaper_choice_for_city(city: &City) -> u32 {
    if city.kind == CityFileKind::MacCity {
        return 0;
    }
    city.misc_u32_at(MISC_NEWSPAPER_CHOICE_OFFSET)
        .unwrap_or(0)
        .min(newspaper_max_choice_for_city(city))
}

pub(super) fn set_city_newspaper_choice(city: &mut City, choice: u32) -> bool {
    set_city_newspaper_choice_with_crt(city, choice, None)
}

pub(super) fn set_city_newspaper_choice_with_crt(
    city: &mut City,
    choice: u32,
    crt: Option<&mut Win95CrtRand>,
) -> bool {
    if city.kind == CityFileKind::MacCity {
        return false;
    }
    let next = choice.min(newspaper_max_choice_for_city(city));
    if city.misc_u32_at(MISC_NEWSPAPER_CHOICE_OFFSET) == Some(next) {
        return false;
    }
    ensure_budget_misc_defaults_before_extension(city);
    ensure_newspaper_list_misc_defaults_before_extension(city, crt);
    city.set_misc_u32_at(MISC_NEWSPAPER_CHOICE_OFFSET, next);
    true
}

pub(super) fn newspaper_max_choice_for_city(city: &City) -> u32 {
    // Win95 newspaper menu @ `0x406dd7`: max index is `min(5, wCityProgression)`.
    u32::from(city.pe_w_city_progression).min(NEWSPAPER_CHOICE_MAX)
}

pub(super) fn ensure_newspaper_list_misc_defaults_before_extension(
    city: &mut City,
    crt: Option<&mut Win95CrtRand>,
) {
    if city.kind == CityFileKind::MacCity {
        return;
    }
    // A fresh city carries a full-sized, zero-filled MISC record. The PE paper
    // table is a permutation of 30 nonzero bytes, so all-zero means it has not
    // been initialized; any nonzero saved table remains authoritative.
    let paper_list = city.misc_u32_values(MISC_NEWSPAPER_LIST_OFFSET, NEWSPAPER_LIST_MISC_DWORDS);
    if paper_list.iter().all(|&value| value == 0) {
        let values = match crt {
            Some(crt) => pe_newspaper_paper_list_misc_values_with_rng(crt),
            None => pe_newspaper_paper_list_misc_values(city),
        };
        city.set_misc_u32_values(MISC_NEWSPAPER_LIST_OFFSET, &values);
    }
    let story_list = city.misc_u32_values(
        MISC_NEWSPAPER_STORY_LIST_OFFSET,
        NEWSPAPER_STORY_LIST_MISC_DWORDS,
    );
    if story_list.iter().all(|&value| value == 0) {
        city.set_misc_u32_values(
            MISC_NEWSPAPER_STORY_LIST_OFFSET,
            &pe_newspaper_story_list_misc_values(),
        );
    }
}

pub(super) fn ensure_default_city_options(city: &mut City) {
    ensure_default_city_options_with_crt(city, None)
}

pub(super) fn ensure_default_city_options_with_crt(
    city: &mut City,
    crt: Option<&mut Win95CrtRand>,
) {
    ensure_default_city_options_for_engine(city, EngineFidelity::Win95, crt)
}

pub(super) fn ensure_default_city_options_for_engine(
    city: &mut City,
    engine_fidelity: EngineFidelity,
    crt: Option<&mut Win95CrtRand>,
) {
    // These defaults model Win95 MISC extensions. Mac CODE 4 serializes native
    // state through many of the same offsets (including the industry block,
    // newspaper tables, and the late option slots), so never seed them while
    // operating in Mac fidelity.
    if !pe_misc_mirror_storage_active(city, engine_fidelity) {
        return;
    }
    ensure_budget_misc_defaults_before_extension(city);
    ensure_industry_misc_defaults_before_extension(city);
    ensure_newspaper_list_misc_defaults_before_extension(city, crt);
    for offset in [
        AUTO_GOTO_MISC_OFFSET,
        SOUND_EFFECTS_MISC_OFFSET,
        MUSIC_MISC_OFFSET,
    ] {
        if city.misc_u32_at(offset).is_none() {
            city.set_misc_u32_at(offset, CITY_OPTION_ENABLED_VALUE);
        }
    }
    // Win95 newspaper init @ `0x479610` / `0x47998e`: Extra=1, Subscription=0, choice=0.
    if city.misc_u32_at(MISC_NEWSPAPER_EXTRA_OFFSET).is_none() {
        city.set_misc_u32_at(MISC_NEWSPAPER_EXTRA_OFFSET, CITY_OPTION_ENABLED_VALUE);
    }
    if city
        .misc_u32_at(MISC_NEWSPAPER_SUBSCRIPTION_OFFSET)
        .is_none()
    {
        city.set_misc_u32_at(
            MISC_NEWSPAPER_SUBSCRIPTION_OFFSET,
            CITY_OPTION_DISABLED_VALUE,
        );
    }
    if city.misc_u32_at(MISC_NEWSPAPER_CHOICE_OFFSET).is_none() {
        city.set_misc_u32_at(MISC_NEWSPAPER_CHOICE_OFFSET, 0);
    }
    // PE difficulty BSS `wCityDifficulty` @ `0x4cb404`.
    if u32::from(city.pe_w_city_difficulty) == 0 {
        city.set_misc_u32_at(MISC_DIFFICULTY_OFFSET, CITY_DIFFICULTY_MEDIUM);
    }
}
