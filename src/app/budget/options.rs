//! Saved Auto-Budget options and budget-row default initialization.
//!
//! PE and Mac option fields use different codecs. Keep the shared UI-facing
//! selectors here while preserving their native saved locations and defaults.

use super::*;

const PE_NEW_CITY_BUDGET_DEFAULTS_4E6730: [u32; BUDGET_TOTAL_COUNT] = [
    7, 7, 7, 1, 0, 100, 100, 100, 100, 100, 100, 100, 100, 100, 100, 100,
];
const MAC_NEW_CITY_BUDGET_DEFAULTS_A5_51DC: [u32; BUDGET_TOTAL_COUNT] = [
    7, 7, 7, 1, 0, 100, 100, 100, 100, 100, 100, 100, 100, 100, 100, 100,
];

pub(in crate::app) const AUTO_BUDGET_MISC_OFFSET: usize = 0x0ff0;
pub(in crate::app) const AUTO_BUDGET_ENABLED_VALUE: u32 = 1;
pub(in crate::app) const AUTO_BUDGET_DISABLED_VALUE: u32 = 0;

pub(in crate::app) fn auto_budget_enabled_for_city(city: &City) -> bool {
    city.misc_u32_at(AUTO_BUDGET_MISC_OFFSET).unwrap_or(0) != AUTO_BUDGET_DISABLED_VALUE
}

/// Read the native Mac Auto-Budget option from CODE 4 slot 1020. Its low byte
/// is restored from `ffffd606` and the original Options menu labels item 1
/// "Auto-Budget". This option gates the January budget window; it is distinct
/// from CityGame's PE service-funding automation below.
pub(in crate::app) fn mac_auto_budget_enabled_for_city(city: &City) -> bool {
    // The native Mac reader consumes the low byte of MISC slot 1020. That
    // logical field is also present in Win-layout saves at the same offset;
    // the selected engine controls the annual-review behavior, while the
    // file layout controls how a toggle is written.
    city.mac_state_misc_u32_at(AUTO_BUDGET_MISC_OFFSET)
        .is_some_and(|value| value as u8 != 0)
}

/// Both recovered options gate annual review. File layout owns the saved
/// representation; the known Mac byte can also supply a Win95 engine session.
pub(in crate::app) fn auto_budget_enabled_for_engine(city: &City, engine: EngineFidelity) -> bool {
    match (engine, city.kind) {
        (EngineFidelity::Win95, CityFileKind::MacCity) => mac_auto_budget_enabled_for_city(city),
        (EngineFidelity::Win95, _) => auto_budget_enabled_for_city(city),
        (EngineFidelity::Mac68k, CityFileKind::MacCity) => mac_auto_budget_enabled_for_city(city),
        // Mac CODE 4 reads the shared slot's low byte. The Win-layout field is
        // a DWORD, but native game saves use the same 0/1 values.
        (EngineFidelity::Mac68k, _) => mac_auto_budget_enabled_for_city(city),
    }
}

pub(in crate::app) fn set_auto_budget_enabled_for_engine(
    city: &mut City,
    engine: EngineFidelity,
    enabled: bool,
) -> bool {
    match (engine, city.kind) {
        (_, CityFileKind::MacCity) => set_mac_auto_budget_enabled(city, enabled),
        (EngineFidelity::Win95, _) => set_city_auto_budget_enabled(city, enabled),
        // On a Win-layout save, MISC +0x0ff0 is the file-owned DWORD option;
        // write it through the PE-layout codec while keeping Mac annual-review
        // behavior selected by the engine.
        (EngineFidelity::Mac68k, _) => set_city_auto_budget_enabled(city, enabled),
    }
}

pub(in crate::app) fn pe_fiscal_crisis_after_settlement(
    funds: i32,
    auto_budget_enabled: bool,
) -> bool {
    funds < 0 && auto_budget_enabled
}

pub(in crate::app) fn set_mac_auto_budget_enabled(city: &mut City, enabled: bool) -> bool {
    if city.kind != CityFileKind::MacCity {
        return false;
    }
    let Some(current) = city.mac_state_misc_u32_at(AUTO_BUDGET_MISC_OFFSET) else {
        return false;
    };
    if (current as u8 != 0) == enabled {
        return false;
    }
    // CODE 4 saves this native byte by zero-extending it into slot 1020.
    city.set_mac_state_misc_u32_at(
        AUTO_BUDGET_MISC_OFFSET,
        if enabled {
            AUTO_BUDGET_ENABLED_VALUE
        } else {
            AUTO_BUDGET_DISABLED_VALUE
        },
    );
    true
}

pub(in crate::app) fn set_city_auto_budget_enabled(city: &mut City, enabled: bool) -> bool {
    if city.kind == CityFileKind::MacCity {
        // Mac CODE 4 slot 1020 needs the native Mac state codec. Keep this raw
        // PE writer safe even if a caller bypasses the layout selector above.
        return false;
    }
    let next = if enabled {
        AUTO_BUDGET_ENABLED_VALUE
    } else {
        AUTO_BUDGET_DISABLED_VALUE
    };
    if city.misc_u32_at(AUTO_BUDGET_MISC_OFFSET).unwrap_or(0) == next {
        return false;
    }
    ensure_budget_misc_defaults_before_extension(city);
    city.set_misc_u32_at(AUTO_BUDGET_MISC_OFFSET, next);
    true
}
pub(in crate::app) fn ensure_budget_misc_defaults_before_extension(city: &mut City) {
    for index in 0..BUDGET_TAX_COUNT {
        if budget_row_is_unset(city, index)
            && let Some(offset) = tax_rate_misc_offset_for_city(city, index)
        {
            city.set_budget_misc_u32_values(offset, &[DEFAULT_TAX_RATE]);
        }
    }
    for index in 0..BUDGET_SERVICE_COUNT {
        if budget_row_is_unset(city, BUDGET_SERVICE_ROW_START.saturating_add(index))
            && let Some(offset) = budget_service_funding_misc_offset_for_city(city, index)
        {
            city.set_budget_misc_u32_values(offset, &[100]);
        }
    }
}

pub(in crate::app) fn initialize_new_city_budget_rows(city: &mut City, fidelity: EngineFidelity) {
    // The two executables independently initialize the same 16-row book. The
    // accessor routes native Mac rows to the isolated Mac row-book buffer.
    let defaults = match fidelity {
        EngineFidelity::Win95 => PE_NEW_CITY_BUDGET_DEFAULTS_4E6730,
        EngineFidelity::Mac68k => MAC_NEW_CITY_BUDGET_DEFAULTS_A5_51DC,
    };
    for (row, value) in defaults.into_iter().enumerate() {
        if let Some(offset) = budget_row_misc_offset_for_city(city, row, 0) {
            city.set_budget_misc_u32_values(offset, &[value, value]);
        }
    }
}
