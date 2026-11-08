//! PE and Mac XTXT resource cleanup, XMIC placement, and system-row refresh.

use super::*;

/// Win95 `0x4e7c9a` microsim class. Provision (`0x44cf60`) also requires `tile >= 0xc6`.
pub(super) const PE_MICROSIM_CLASS: [u8; 58] = [
    21, 21, 20, 1, 1, 1, 1, 1, 1, 1, 2, 3, 4, 5, 23, 22, 6, 7, 8, 9, 10, 11, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 19, 0, 0, 17, 18, 0, 0, 0, 0, 0, 12, 13, 24, 0, 0, 25, 0, 14, 15, 15, 15, 15,
    16,
];

/// Mac CODE 3 `export_157` A5-relative allocation class for XBLD `0xc6..=0xff`.
/// The recovered 58 bytes currently match PE's table, but keep their sources
/// separate so future edition-specific evidence cannot silently cross over.
pub(super) const MAC_MICROSIM_CLASS: [u8; 58] = [
    21, 21, 20, 1, 1, 1, 1, 1, 1, 1, 2, 3, 4, 5, 23, 22, 6, 7, 8, 9, 10, 11, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 19, 0, 0, 17, 18, 0, 0, 0, 0, 0, 12, 13, 24, 0, 0, 25, 0, 14, 15, 15, 15, 15,
    16,
];

pub(super) fn mac_microsim_class(tile_id: u8) -> u8 {
    usize::from(tile_id)
        .checked_sub(0xc6)
        .and_then(|index| MAC_MICROSIM_CLASS.get(index).copied())
        .unwrap_or(0)
}

fn apply_mac_fixed_microsim_placement(record: &mut MicrosimRecord, tile_id: u8) {
    match tile_id {
        0xc6 | 0xc7 => {
            record.stat1 = record.stat1.wrapping_add(1);
            record.stat2 = record.stat2.wrapping_add(20);
        }
        0xc8 => {
            record.stat1 = record.stat1.wrapping_add(1);
            record.stat2 = record.stat2.wrapping_add(4);
        }
        0xd5 => record.stat2 = record.stat2.wrapping_add(9),
        0xe9 | 0xec | 0xed => record.stat1 = record.stat1.wrapping_add(1),
        _ => {}
    }
}

fn mac_initial_structure_microsim_record(
    city: &City,
    tile_id: u8,
    current_year: u16,
    rng: &mut MacToolboxRng,
) -> MicrosimRecord {
    let mut record = MicrosimRecord {
        tile_id,
        ..MicrosimRecord::default()
    };
    match tile_id {
        0xc9..=0xcf => record.stat1 = mac_initial_plant_parameter(tile_id),
        0xd0 => {
            record.stat1 = mac_xmic_capacity_helper(city, 200, 900);
            record.stat2 = current_year;
        }
        0xd1 | 0xd6 | 0xd9 => record.stat0 = 6,
        0xd2 => {
            let funding = mac_raw_service_funding(city, 0) as u16;
            record.stat1 = mac_xmic_capacity_helper(city, funding.wrapping_mul(2), 90);
        }
        0xd3 => {
            let funding = mac_raw_service_funding(city, 1) as i32;
            record.stat1 = mac_xmic_capacity_helper(city, (funding >> 1) as u16, 70);
            record.stat2 = 4;
        }
        0xdb => record.stat1 = current_year,
        0xf3 => {
            record.stat1 = current_year;
            record.stat2 = rng.random_u16() % 30 + 10;
            record.stat3 = rng.random_u16() % 60;
        }
        0xfb..=0xfe => {
            record.stat0 = 5;
            record.stat1 = mac_initial_arcology_parameter(tile_id);
            record.stat3 = current_year;
        }
        0xff => record.stat3 = current_year,
        _ => {}
    }
    record
}

/// Literal WORDs from the Mac CODE 3 initializer; their units are not proven.
fn mac_initial_plant_parameter(tile_id: u8) -> u16 {
    match tile_id {
        0xc9 => 50,
        0xca => 220,
        0xcb => 500,
        0xcc => 50,
        0xcd => 1_600,
        0xce => 2_500,
        0xcf => 200,
        _ => 0,
    }
}

/// Literal `+2` WORDs from the Mac `0xfb..=0xfe` branches; units are unknown.
fn mac_initial_arcology_parameter(tile_id: u8) -> u16 {
    match tile_id {
        0xfb => 55,
        0xfc => 30,
        0xfd => 45,
        0xfe => 65,
        _ => 0,
    }
}

/// Mac `FUN_0001238a`: unsigned scalar division and low-WORD narrowing, then
/// a signed WORD minimum against the caller's candidate.
fn mac_xmic_capacity_helper(city: &City, candidate: u16, divisor: u16) -> u16 {
    let divisor = if divisor == 0 {
        100_u32
    } else {
        i32::from(divisor as i16) as u32
    };
    let quotient = (city.mac_state_misc_u32_at(0x102c).unwrap_or(0) / divisor) as u16;
    if (quotient as i16) < (candidate as i16) {
        quotient
    } else {
        candidate
    }
}

fn mac_raw_service_funding(city: &City, service_index: usize) -> u32 {
    let row = MISC_BUDGET_SERVICE_ROW_START.saturating_add(service_index);
    let Some(start) = budget_row_misc_offset_for_city(city, row, 0) else {
        return DEFAULT_SERVICE_FUNDING;
    };
    city.budget_misc_u32_at(start.saturating_add(MISC_BUDGET_FUNDING_FIELD * MISC_FIELD_LEN))
        .unwrap_or(DEFAULT_SERVICE_FUNDING)
}

pub(super) fn pe_microsim_class(tile_id: u8) -> u8 {
    usize::from(tile_id)
        .checked_sub(0xc6)
        .and_then(|index| PE_MICROSIM_CLASS.get(index).copied())
        .unwrap_or(0)
}

fn system_microsim_index_for_building(tile_id: u8) -> Option<usize> {
    let class = pe_microsim_class(tile_id);
    (class >= 0x11).then(|| usize::from(class - 0x10))
}

/// Fallback XLAB copy when `pTileNames[tile]` is null (`0x44d0d6` / `0x4c6a8a`).
fn pe_provision_xlabel(tile_id: u8) -> Option<&'static str> {
    Some(match tile_id {
        198 | 199 => "Hydro Power",
        200 => "Wind Power",
        201 => "Gas Power",
        202 => "Oil Power",
        203 => "Nuclear Power",
        204 => "Solar Power",
        205 => "Microwave Power",
        206 => "Fusion Power",
        207 => "Coal Power",
        208 => "City Hall",
        209 => "Hospital",
        210 => "Police Station",
        211 => "Fire Station",
        212 => "Museum",
        213 => "SimPark System",
        214 => "School",
        215 => "Stadium",
        216 => "Prison",
        217 => "College",
        218 => "Zoo",
        219 => "Statue",
        233 => "Subway Station",
        236 => "Bus Depot",
        237 => "Rail Depot",
        243 => "Mayor's House",
        244 => "Water Treatment",
        245 => "Library",
        248 => "Marina",
        250 => "Desalinization",
        251 => "Plymouth Arco",
        252 => "Forest Arco",
        253 => "Darco",
        254 => "Launch Arco",
        255 => "Braun Llama Dome",
        _ => return None,
    })
}

/// Win95 `SimulationInitializeMicrosim` (`0x44ddc0`) for system slots (class `>= 0x11`).
/// The PE increments the shared row rather than reseeding it.
fn apply_pe_initialize_system_microsim(record: &mut MicrosimRecord, tile_id: u8) {
    match tile_id {
        198 | 199 => {
            record.stat1 = record.stat1.wrapping_add(1);
            record.stat2 = record.stat2.wrapping_add(0x14);
        }
        200 => {
            record.stat1 = record.stat1.wrapping_add(1);
            record.stat2 = record.stat2.wrapping_add(4);
        }
        213 => {
            record.stat2 = record.stat2.wrapping_add(9);
        }
        233 | 236 | 237 => {
            record.stat1 = record.stat1.wrapping_add(1);
        }
        _ => {}
    }
}

/// Win95 `FUN_0044f7d0` pop load at place: leftover `0x4caa74` + arco residents
/// (`0x4c94c4`) + tile-count bonus.
fn pe_initialize_capacity_pop(city: &City) -> u32 {
    pe_microsim_population_load(
        city.pe_dw_city_population(),
        city.pe_dw_arcology_population(),
        pe_arco_tile_pop_bonus(pe_occupancy_arco_footprints(city)),
    )
}

pub(super) fn budget_row_misc_offset_for_city(
    city: &City,
    row_index: usize,
    field: usize,
) -> Option<usize> {
    let base = match city.kind {
        CityFileKind::MacCity => MAC_MISC_BUDGET_BOOK_OFFSET,
        CityFileKind::WinSc2 | CityFileKind::WinScenario => MISC_BUDGET_BOOK_OFFSET,
    };
    row_index
        .checked_mul(MISC_BUDGET_ROW_DWORDS)
        .and_then(|base| base.checked_add(field))
        .and_then(|field| field.checked_mul(4))
        .and_then(|bytes| base.checked_add(bytes))
}

/// Win95 `*(0x4cc4cc)+0x294` police / `+0x304` fire: MISC budget-book percent.
/// Unset (all-zero) rows behave as 100, matching `budget_service_funding_percent_for_city`.
fn misc_budget_service_funding(city: &City, service_index: usize) -> u32 {
    let row = MISC_BUDGET_SERVICE_ROW_START.saturating_add(service_index);
    let Some(start) = budget_row_misc_offset_for_city(city, row, 0) else {
        return DEFAULT_SERVICE_FUNDING;
    };
    let values = city.budget_misc_u32_values(start, MISC_BUDGET_ROW_DWORDS);
    if !values.is_empty() {
        if values.iter().all(|&value| value == 0) {
            return DEFAULT_SERVICE_FUNDING;
        }
        return values
            .get(MISC_BUDGET_FUNDING_FIELD)
            .copied()
            .unwrap_or(DEFAULT_SERVICE_FUNDING)
            .min(100);
    }
    city.budget_misc_u32_at(
        budget_row_misc_offset_for_city(city, row, MISC_BUDGET_FUNDING_FIELD).unwrap_or(start),
    )
    .map(|funding| funding.min(100))
    .unwrap_or(DEFAULT_SERVICE_FUNDING)
}

/// Seed stats from Win95 `SimulationInitializeMicrosim` (`0x44de8c` structure path).
/// Occupancy (`FUN_0044e390`) still fills yearly school/hospital/college/stadium/
/// police/fire/prison/zoo/llama/city-hall/statue/mayor/water/desal/power/arco fields.
pub(super) fn initial_structure_microsim_record(
    city: &City,
    tile_id: u8,
    crt: &mut Win95CrtRand,
) -> MicrosimRecord {
    let built_year = city.saved_date().map(|date| date.year).unwrap_or(1900);
    let pop_load = pe_initialize_capacity_pop(city);
    let mut record = MicrosimRecord {
        tile_id,
        ..MicrosimRecord::default()
    };
    match tile_id {
        201..=207 => {
            record.stat1 = power_plant_max_mw(tile_id);
        }
        208 => {
            // `push 0x384; push 0xc8`: helper sees divisor 900, cap 200.
            record.stat1 =
                pe_microsim_scaled_capacity(pop_load, 900, 200).min(u32::from(u16::MAX)) as u16;
            record.stat2 = built_year;
        }
        209 | 214 | 217 => {
            record.stat0 = 6;
        }
        210 => {
            // PE `0x44dfa7`: `FUN_0044f7d0(pop, 90, funding*2)` from budget `+0x294`.
            let funding = misc_budget_service_funding(city, 0);
            record.stat1 = pe_microsim_scaled_capacity(pop_load, 90, funding.saturating_mul(2))
                .min(u32::from(u16::MAX)) as u16;
        }
        211 => {
            // PE `0x44dfce`: `FUN_0044f7d0(pop, 70, funding/2)` from budget `+0x304`.
            let funding = misc_budget_service_funding(city, 1);
            record.stat1 = pe_microsim_scaled_capacity(pop_load, 70, funding / 2)
                .min(u32::from(u16::MAX)) as u16;
            record.stat2 = 4;
        }
        219 => {
            // Tile DB selects ordinary initializer action 0x0e -> `0x44e00e`.
            // It writes only stat1 = current_year_offset + year_founded.
            record.stat1 = built_year;
        }
        243 => {
            record.stat1 = built_year;
            // PE `0x44e041..0x44e073`: two shared CRT draws, stored as
            // `rand()%30 + 10` and `rand()%60`.
            record.stat2 = crt.random_mod(0x1e).saturating_add(0xa);
            record.stat3 = crt.random_mod(0x3c);
        }
        251..=254 => {
            record.stat0 = 5;
            record.stat1 = arco_design_capacity_thousands(tile_id);
            record.stat3 = built_year;
        }
        255 => {
            // strcmp vs `"aus"` (`0x4e807c`); else `rand() & 0x3f`.
            record.stat3 = if city.name == "aus" {
                built_year
            } else {
                crt.random_u16_mask(0x3f)
            };
        }
        _ => {}
    }
    record
}

fn power_plant_max_mw(tile_id: u8) -> u16 {
    match tile_id {
        201 => 50,    // gas
        202 => 220,   // oil
        203 => 500,   // nuclear
        204 => 50,    // solar
        205 => 1_600, // microwave
        206 => 2_500, // fusion
        207 => 200,   // coal
        _ => 0,
    }
}

fn arco_design_capacity_thousands(tile_id: u8) -> u16 {
    match tile_id {
        251 => 55, // Plymouth
        252 => 30, // Forest
        253 => 45, // Darco
        254 => 65, // Launch
        _ => 0,
    }
}

impl City {
    /// Win95 `0x401dca` / `FUN_0044ced0`: free XLAB / XMIC / XTHG resources for an XTXT id.
    /// Signs `1..=0x32` clear XLAB; system `0x33..=0x3c` no-op; structures `0x3d..=0xc8`
    /// clear XMIC `tile_id` then XLAB; actor proxies `0xc9..=0xf0` clear XTHG `saved_a`.
    pub fn pe_401dca_clear_xtxt_resources(&mut self, xtxt: u8) {
        self.pe_401dca_clear_xtxt_resources_inner(xtxt, None);
    }

    /// CODE 5 export 80's generic per-cell map edits. XTER chooses the
    /// PlaceTile replacement; the later XTXT load owns resource cleanup.
    /// Keep this shared by terrain/transport removal and plant expiry.
    pub(crate) fn mac_export80_replace_generic_cell(
        &mut self,
        x: usize,
        y: usize,
        root_building: u8,
        rng: &mut MacToolboxRng,
    ) {
        if x >= CITY_WIDTH || y >= CITY_HEIGHT {
            return;
        }
        let replacement = if self.terrain_at(x, y) == 0 {
            (rng.random_u16() as u8 & 3) + 1
        } else {
            0
        };
        // Keep the compatibility census as well as set_building's native Mac
        // census; engine choice does not depend on serialized city layout.
        let old = self.building_at(x, y);
        if self.zone_at(x, y).is_some_and(|zone| zone.zone_id() == 7) {
            self.pe_adjust_military_tile_count_441f86(old, replacement);
        } else {
            self.pe_dw_tile_count[usize::from(old)] =
                self.pe_dw_tile_count[usize::from(old)].wrapping_sub(1);
            self.pe_dw_tile_count[usize::from(replacement)] =
                self.pe_dw_tile_count[usize::from(replacement)].wrapping_add(1);
        }
        self.set_building(x, y, replacement);
        let bits = self.tile_bits_at(x, y).unwrap_or_default();
        self.set_tile_bits(x, y, TileBits::new(bits.raw_bits() & 0x3d));
        let zone = self.zone_at(x, y).map_or(0, |zone| zone.raw_bits());
        self.set_zone(x, y, zone & 0x0f);

        let xtxt = self.text_overlay_at(x, y).unwrap_or(0);
        if xtxt != 0 && (xtxt < 0xf1 || xtxt == 0xfa) {
            if xtxt < 0xc9 || xtxt == 0xfa {
                self.set_text_overlay(x, y, 0);
            }
            self.mac_export_161_clear_xtxt_resources(xtxt);
            if xtxt == 0xfa {
                self.mac_export80_decrement_xtxt_fa_counter(root_building);
            }
        }
    }

    /// CODE 5 export 80's generic XTXT-0xfa saved-counter decrement. The
    /// classification is from the original root XBLD, not the per-cell tile.
    pub(crate) fn mac_export80_decrement_xtxt_fa_counter(&mut self, root_building: u8) {
        let network =
            matches!(root_building, 0x1d..=0x2b | 0x3f..=0x46 | 0x4b..=0x4c | 0x5d..=0x60);
        let offset = if network {
            MAC_XTXT_FA_NETWORK_COUNT_MISC_OFFSET
        } else {
            MAC_XTXT_FA_NON_NETWORK_COUNT_MISC_OFFSET
        };
        let counter = self.mac_state_misc_u32_at(offset).unwrap_or(0) as u16;
        self.set_mac_state_misc_u32_at(offset, u32::from(counter.wrapping_sub(1)));
    }

    /// Mac CODE 3 `export_161` XTXT cleanup. The Mac caller owns its map-marker
    /// clearing decision; this helper only releases the referenced resource:
    /// labels for `1..=0x32`, no-op system IDs `0x33..=0x3c`, XMIC `+0` and
    /// XLAB for `0x3d..=0xc8`, and XTHG byte `+0x0a` for `0xc9..=0xf0`.
    pub(crate) fn mac_export_161_clear_xtxt_resources(&mut self, xtxt: u8) {
        match xtxt {
            0x01..=0x32 => self.set_label(xtxt, ""),
            0x33..=0x3c => {}
            0x3d..=0xc8 => {
                let index = usize::from(xtxt.saturating_sub(MIN_SIM_TEXT_ENTRIES));
                if self.microsims.is_none() {
                    self.microsims = Some(vec![MicrosimRecord::default(); MICROSIM_COUNT]);
                }
                if let Some(record) = self
                    .microsims
                    .as_mut()
                    .and_then(|records| records.get_mut(index))
                {
                    record.tile_id = 0;
                    self.modified = true;
                }
                self.set_label(xtxt, "");
            }
            0xc9..=0xf0 => {
                let slot = usize::from(xtxt - 0xc9);
                if let Some(record) = self.mac_object_slot_records_mut().get_mut(slot) {
                    record.saved_a = 0;
                    self.modified = true;
                }
            }
            _ => {}
        }
    }

    pub(super) fn pe_401dca_clear_xtxt_resources_with_microsims(
        &mut self,
        xtxt: u8,
        microsims: &mut [MicrosimRecord],
    ) {
        self.pe_401dca_clear_xtxt_resources_inner(xtxt, Some(microsims));
    }

    fn pe_401dca_clear_xtxt_resources_inner(
        &mut self,
        xtxt: u8,
        mut microsims: Option<&mut [MicrosimRecord]>,
    ) {
        match xtxt {
            0x01..=0x32 => self.set_label(xtxt, ""),
            0x33..=0x3c => {}
            0x3d..=0xc8 => {
                let index = usize::from(xtxt.saturating_sub(MIN_SIM_TEXT_ENTRIES));
                let record = if let Some(records) = microsims.as_deref_mut() {
                    records.get_mut(index)
                } else {
                    if self.microsims.is_none() {
                        self.microsims = Some(vec![MicrosimRecord::default(); MICROSIM_COUNT]);
                    }
                    self.microsims
                        .as_mut()
                        .and_then(|records| records.get_mut(index))
                };
                if let Some(record) = record {
                    record.tile_id = 0;
                    self.modified = true;
                }
                self.set_label(xtxt, "");
            }
            0xc9..=0xf0 => {
                let slot = usize::from(xtxt.saturating_sub(0xc9));
                if let Some(table) = self.traffic_overlays.as_mut()
                    && let Some(record) = table.get_mut(slot)
                {
                    record.saved_a = 0;
                    self.modified = true;
                }
            }
            _ => {}
        }
    }

    pub fn set_microsim_stat0(&mut self, index: usize, stat0: u8) -> bool {
        if self.microsims.is_none() {
            self.microsims = Some(vec![MicrosimRecord::default(); MICROSIM_COUNT]);
        }
        let Some(record) = self
            .microsims
            .as_mut()
            .and_then(|microsims| microsims.get_mut(index))
        else {
            return false;
        };
        record.stat0 = stat0;
        self.modified = true;
        true
    }

    /// Set the map/object class byte in one XMIC record.
    pub fn set_microsim_tile_id(&mut self, index: usize, tile_id: u8) -> bool {
        if self.microsims.is_none() {
            self.microsims = Some(vec![MicrosimRecord::default(); MICROSIM_COUNT]);
        }
        let Some(record) = self
            .microsims
            .as_mut()
            .and_then(|microsims| microsims.get_mut(index))
        else {
            return false;
        };
        record.tile_id = tile_id;
        self.modified = true;
        true
    }

    pub fn set_microsim_stat1(&mut self, index: usize, stat1: u16) -> bool {
        if self.microsims.is_none() {
            self.microsims = Some(vec![MicrosimRecord::default(); MICROSIM_COUNT]);
        }
        let Some(record) = self
            .microsims
            .as_mut()
            .and_then(|microsims| microsims.get_mut(index))
        else {
            return false;
        };
        record.stat1 = stat1;
        self.modified = true;
        true
    }

    pub fn set_microsim_stat2(&mut self, index: usize, stat2: u16) -> bool {
        if self.microsims.is_none() {
            self.microsims = Some(vec![MicrosimRecord::default(); MICROSIM_COUNT]);
        }
        let Some(record) = self
            .microsims
            .as_mut()
            .and_then(|microsims| microsims.get_mut(index))
        else {
            return false;
        };
        record.stat2 = stat2;
        self.modified = true;
        true
    }

    pub fn set_microsim_stat3(&mut self, index: usize, stat3: u16) -> bool {
        if self.microsims.is_none() {
            self.microsims = Some(vec![MicrosimRecord::default(); MICROSIM_COUNT]);
        }
        let Some(record) = self
            .microsims
            .as_mut()
            .and_then(|microsims| microsims.get_mut(index))
        else {
            return false;
        };
        record.stat3 = stat3;
        self.modified = true;
        true
    }

    /// Win95 `SimulationProvisionMicrosim` (`0x44cf60`). Tile `< 0xc6` or class 0
    /// returns nothing. System class `>= 0x11` uses slot `class-0x10`; structures
    /// scan from index 10. Arcologies/llama (`tile >= 0xfb`) steal the first
    /// non-arco structure slot and clear matching XTXT. XLAB `[51+slot]` is then
    /// filled from the default name table (`0x4c6a8a` / `pTileNames` fallback).
    pub fn provision_microsim_for_building(&mut self, tile_id: u8) -> Option<u8> {
        self.provision_microsim_for_building_with_crt(
            tile_id,
            &mut Win95CrtRand::new(Win95CrtRand::DEFAULT_SEED),
        )
    }

    /// Live PE placement variant: structure initializer `FUN_0044de8c` uses
    /// the process CRT stream for the Mayor's House and non-`aus` Llama Dome.
    pub fn provision_microsim_for_building_with_crt(
        &mut self,
        tile_id: u8,
        crt: &mut Win95CrtRand,
    ) -> Option<u8> {
        let class = pe_microsim_class(tile_id);
        if class == 0 {
            return None;
        }
        if self.microsims.is_none() {
            self.microsims = Some(vec![MicrosimRecord::default(); MICROSIM_COUNT]);
        }
        let is_system = class >= 0x11;
        let mut stolen_overlay = None;
        let index = if let Some(slot) = system_microsim_index_for_building(tile_id) {
            slot
        } else {
            let empty = {
                let microsims = self.microsims.as_ref()?;
                microsims
                    .iter()
                    .enumerate()
                    .skip(MICROSIM_STRUCTURE_FIRST)
                    .find_map(|(index, record)| (record.tile_id == 0).then_some(index))
            };
            if let Some(index) = empty {
                index
            } else if tile_id >= 0xfb {
                let steal = {
                    let microsims = self.microsims.as_ref()?;
                    microsims
                        .iter()
                        .enumerate()
                        .skip(MICROSIM_STRUCTURE_FIRST)
                        .find_map(|(index, record)| (record.tile_id < 0xfb).then_some(index))
                };
                let steal = steal?;
                stolen_overlay = Some(MIN_SIM_TEXT_ENTRIES.saturating_add(steal as u8));
                steal
            } else {
                return None;
            }
        };
        if index >= MICROSIM_COUNT {
            return None;
        }
        if let Some(overlay) = stolen_overlay {
            self.clear_text_overlays_matching(overlay);
        }
        if is_system {
            let microsims = self.microsims.as_mut()?;
            microsims[index].tile_id = tile_id;
            apply_pe_initialize_system_microsim(&mut microsims[index], tile_id);
            self.modified = true;
        } else {
            let seeded = initial_structure_microsim_record(self, tile_id, crt);
            let microsims = self.microsims.as_mut()?;
            microsims[index] = seeded;
            self.modified = true;
        }
        let text = MIN_SIM_TEXT_ENTRIES.saturating_add(index as u8);
        let xlab_empty = self.label_at(text).is_none_or(str::is_empty);
        if (!is_system || xlab_empty)
            && let Some(name) = pe_provision_xlabel(tile_id)
        {
            self.set_label(text, name);
        }
        Some(text)
    }

    /// Mac CODE 3 export 157 XMIC/XTXT allocation. The class table and row
    /// initialization are edition-specific; the fixed rows update in place,
    /// while ordinary rows start at slot 10. QuickDraw draws are consumed only
    /// by the recovered Mayor's House initializer.
    pub(crate) fn provision_mac_microsim_for_building(
        &mut self,
        tile_id: u8,
        rng: &mut MacToolboxRng,
    ) -> Option<u8> {
        let class = mac_microsim_class(tile_id);
        if class == 0 {
            return None;
        }
        if self.microsims.is_none() {
            self.microsims = Some(vec![MicrosimRecord::default(); MICROSIM_COUNT]);
        }

        let is_fixed = class >= 0x11;
        let mut stolen_overlay = None;
        let index = if is_fixed {
            usize::from(class - 0x10)
        } else {
            let empty = self
                .microsims
                .as_ref()?
                .iter()
                .enumerate()
                .skip(MICROSIM_STRUCTURE_FIRST)
                .find_map(|(index, record)| (record.tile_id == 0).then_some(index));
            if let Some(index) = empty {
                index
            } else if tile_id >= 0xfb {
                let recycle = self
                    .microsims
                    .as_ref()?
                    .iter()
                    .enumerate()
                    .skip(MICROSIM_STRUCTURE_FIRST)
                    .find_map(|(index, record)| (record.tile_id <= 0xfa).then_some(index))?;
                stolen_overlay = Some(MIN_SIM_TEXT_ENTRIES.saturating_add(recycle as u8));
                recycle
            } else {
                return None;
            }
        };
        if index >= MICROSIM_COUNT {
            return None;
        }

        if let Some(overlay) = stolen_overlay {
            self.clear_text_overlays_matching(overlay);
        }
        let current_year = self.saved_date.map(|date| date.year).unwrap_or(1900);
        if is_fixed {
            let record = self.microsims.as_mut()?.get_mut(index)?;
            record.tile_id = tile_id;
            apply_mac_fixed_microsim_placement(record, tile_id);
        } else {
            let record = mac_initial_structure_microsim_record(self, tile_id, current_year, rng);
            self.microsims
                .as_mut()?
                .get_mut(index)
                .map(|slot| *slot = record)?;
        }

        let text = MIN_SIM_TEXT_ENTRIES.saturating_add(index as u8);
        let xlab_empty = self.label_at(text).is_none_or(str::is_empty);
        if (!is_fixed || xlab_empty)
            && let Some(name) = pe_provision_xlabel(tile_id)
        {
            // The Mac copies 16 bytes from its object text resource. That
            // source-string table is still unresolved, so retain CityGame's
            // existing localized-name fallback while matching XMIC/XTXT.
            self.set_label(text, name);
        }
        self.modified = true;
        Some(text)
    }

    /// Mac CODE 3 `export_143` → `FUN_0001007a` New City XMIC reset.
    /// Keep this separate from the PE reset even though both editions seed
    /// the same fixed row tags: Mac explicitly clears all row tags, then
    /// clears the statistic fields only for rows 1 through 9.
    pub(crate) fn initialize_mac_new_city_microsims(&mut self) {
        let Some(microsims) = self.microsims.as_mut() else {
            return;
        };
        for record in microsims.iter_mut() {
            record.tile_id = 0;
        }
        for (index, tile_id) in [236, 236, 237, 233, 200, 198, 213, 212, 245, 248]
            .into_iter()
            .enumerate()
        {
            microsims[index].tile_id = tile_id;
        }
        for record in microsims.iter_mut().take(10).skip(1) {
            record.stat0 = 0;
            record.stat1 = 0;
            record.stat2 = 0;
            record.stat3 = 0;
        }
        self.modified = true;
    }

    /// Win95 New City reset helper at `0x44cb7f`: seed fixed city-wide XMIC
    /// rows before the first annual dispatch. Save layout does not select this;
    /// callers gate it on Win95 engine fidelity.
    pub(crate) fn initialize_pe_new_city_microsims(&mut self) {
        let Some(microsims) = self.microsims.as_mut() else {
            return;
        };
        microsims.fill(MicrosimRecord::default());
        // Native slots 0..=9: slot 0 is a sentinel bus tag; slots 1..=9
        // are the fixed system rows selected by yearly XMIC dispatch.
        for (index, tile_id) in [236, 236, 237, 233, 200, 198, 213, 212, 245, 248]
            .into_iter()
            .enumerate()
        {
            microsims[index].tile_id = tile_id;
        }
        for (index, tile_id) in [236, 237, 233, 200, 198, 213, 212, 245, 248]
            .into_iter()
            .enumerate()
        {
            let text_id = MIN_SIM_TEXT_ENTRIES.saturating_add((index + 1) as u8);
            if let Some(label) = pe_provision_xlabel(tile_id) {
                self.set_label(text_id, label);
            }
        }
        self.modified = true;
    }

    /// Backward-compatible alias used by arco-focused call sites.
    pub fn provision_structure_microsim(&mut self, tile_id: u8) -> Option<u8> {
        self.provision_microsim_for_building(tile_id)
    }

    /// Refresh city-wide system microsim `stat1` with unit counts (January / yearly).
    pub fn refresh_system_microsim_counts(&mut self) {
        let counts = [
            (MICROSIM_SYSTEM_WIND, count_building_tiles(self, &[200])),
            // PE hydro case sums tile counts 198+199 (`0x44e4db`).
            (
                MICROSIM_SYSTEM_HYDRO,
                count_building_tiles(self, &[198, 199]),
            ),
            // Bus/rail/subway occupancy is FUN_0044e390 (`0x44ee19` / `0x44ee5f` /
            // `0x44edf3`). Park/marina (`0x44e8e6` / `0x44efd2`) and museum/library
            // (`0x44e887` / `0x44ef17`) are the same yearly pass.
        ];
        if let Some(microsims) = self.microsims.as_mut() {
            for (index, count) in counts {
                if let Some(record) = microsims.get_mut(index) {
                    // PE `FUN_0044E390` dispatches these yearly arms only when
                    // the saved XMIC tile byte is C6/C7 (hydro) or C8 (wind).
                    // Placement provisions that tag; a census refresh must not
                    // synthesize an otherwise absent fixed system row.
                    let active = match index {
                        MICROSIM_SYSTEM_WIND => record.tile_id == 200,
                        MICROSIM_SYSTEM_HYDRO => matches!(record.tile_id, 198 | 199),
                        _ => false,
                    };
                    if !active {
                        continue;
                    }
                    let count = count.min(u32::from(u16::MAX)) as u16;
                    record.stat1 = count;
                    // PE wind/hydro cases also write MW into `stat2`.
                    match index {
                        MICROSIM_SYSTEM_WIND => {
                            // PE `shl ax, 2` writes a WORD, so overflow wraps.
                            record.stat2 = count.wrapping_mul(4);
                        }
                        MICROSIM_SYSTEM_HYDRO => {
                            // PE `imul cx, cx, 0x14` also truncates to a WORD.
                            record.stat2 = count.wrapping_mul(20);
                        }
                        _ => {}
                    }
                }
            }
            self.modified = true;
        }
    }
}
