//! Newspaper story events and delivery adapters.
//!
//! Preserve native thresholds, condition priority, process CRT chronology and
//! pending-flag consumption. Saved queues/papers, options, page composition,
//! gift placement and calendar scheduling retain their existing owners.

use super::city_options::{newspaper_extra_enabled_for_city, set_city_newspaper_choice_with_crt};
use super::dialogs::{CityInfoDialog, NewspaperEdition};
use super::{
    AIRPORT_RUNWAY_CROSS_BUILDING_ID, BIG_PARK_BUILDING_ID, BUDGET_SERVICE_CURRENT_FIELD,
    CITY_HEIGHT, CITY_WIDTH, FIRE_STATION_BUILDING_ID, HOSPITAL_BUILDING_ID, MARINA_BUILDING_ID,
    RAIL_STATION_BUILDING_ID, RUNWAY_BUILDING_ID, SCHOOL_BUILDING_ID, SEAPORT_CRANE_BUILDING_ID,
    STADIUM_BUILDING_ID, SUBWAY_STATION_BUILDING_ID, ViewerState, ZOO_BUILDING_ID,
    budget_row_misc_offset_for_city, budget_service_rows_for_city, ordinance_flags_for_city,
    pe_city_has_ocean, pe_city_has_river, pe_fa_network_overlay_count_4cc4d8,
    pe_xbld_counts_fa_network_overlay_4cc4d8,
};
use crate::city::{City, CityFileKind, Technology};
use crate::win95_random::Win95CrtRand;

/// Durable XTXT `0xfa` census on XBLD outside the PE network bands, used by
/// the seaport condition story rather than the transient Win95 BSS counter.
/// The class split follows DestroyBuilding `0x45a150` and placement `0x44a207`.
pub(super) fn citygame_fa_non_network_neighbor_marker_count(city: &City) -> u32 {
    let mut count = 0_u32;
    for y in 0..CITY_HEIGHT {
        for x in 0..CITY_WIDTH {
            if city.text_overlay_at(x, y) != Some(0xfa) {
                continue;
            }
            if !pe_xbld_counts_fa_network_overlay_4cc4d8(city.building_at(x, y)) {
                count = count.saturating_add(1);
            }
        }
    }
    count
}

/// PE `FUN_00468150` sports-newspaper arm @ `0x4687f3`: when `dwTileCount[0xd7] > 0`
/// and bit `rand() % 5` is set in `wSportsTeams` (`0x4cc4e0`), queue story id `8`
/// with that bit index as the story param.
pub(super) fn pe_rci_demand_updates_sports_newspaper_4687f3(
    city: &mut City,
    crt: &mut Win95CrtRand,
) -> bool {
    if city.pe_dw_tile_count[STADIUM_BUILDING_ID as usize] == 0 {
        return false;
    }
    let bit = crt.random_mod(5);
    let mask = 1u16 << bit;
    if city.pe_w_sports_teams() & mask == 0 {
        return false;
    }
    city.queue_pe_newspaper_story(8, bit as u8);
    true
}

/// PE `FUN_00468150` attitude-newspaper arms @ `0x46881f`–`0x4688a0`.
///
/// For traffic (`*[0x4cc480]`), pollution (`*[0x4cc484]`), and crime (`*[0x4cc48c]`)
/// scores: `rand()&0x7f < score` queues stories `0x11`/`0x12`/`0x10`; `rand()&0xf > score`
/// queues `0x3e`/`0x3f`/`0x3d`. Land-value `*[0x4cc488]` is not in this block.
pub(super) fn pe_rci_demand_updates_attitude_newspaper_46881f(
    city: &mut City,
    crt: &mut Win95CrtRand,
) {
    let arms: [(usize, u16, u16); 3] = [(0, 0x11, 0x3e), (1, 0x12, 0x3f), (3, 0x10, 0x3d)];
    for (score_index, low_story, high_story) in arms {
        let score = city.pe_attitude_score_at(score_index);
        // PE: `rand(); and eax,0x7f; cmp eax,[score]; jge skip` → queue when `< score`.
        if u32::from(crt.random_u15() & 0x7f) < score {
            city.queue_pe_newspaper_story(low_story, 0);
        }
        // PE: `rand(); and eax,0xf; cmp eax,[score]; jle skip` → queue when `> score`.
        if u32::from(crt.random_u15() & 0xf) > score {
            city.queue_pe_newspaper_story(high_story, 0);
        }
    }
}

/// PE `FUN_00468150` unemployment-newspaper arms @ `0x4688a8`.
///
/// Reads BSS `dwUnemploymentPercent` @ `0x4ca5f8`: `rand()&0x3f < unemp` → story `0x15`;
/// `rand()&3 > unemp` → story `0x42`.
pub(super) fn pe_rci_demand_updates_unemployment_newspaper_4688a8(
    city: &mut City,
    crt: &mut Win95CrtRand,
) {
    let unemp = city.pe_unemployment_percent() as i32;
    if i32::from(crt.random_u15() & 0x3f) < unemp {
        city.queue_pe_newspaper_story(0x15, 0);
    }
    if i32::from(crt.random_u15() & 3) > unemp {
        city.queue_pe_newspaper_story(0x42, 0);
    }
}

/// PE `FUN_00468150` EQ-newspaper arms @ `0x468905`.
///
/// Reads BSS fertile EQ @ `0x4cc4b4`: when `EQ < 0x50`, `rand()%0x50 > EQ` queues
/// story `0x13`; when `EQ >= 0x50`, `rand()%0x50 < (EQ-0x50)` queues story `0x40`.
pub(super) fn pe_rci_demand_updates_eq_newspaper_468905(city: &mut City, crt: &mut Win95CrtRand) {
    let eq = city.pe_eq_education_average();
    let roll = u32::from(crt.random_mod(0x50));
    if eq < 0x50 {
        if roll > eq {
            city.queue_pe_newspaper_story(0x13, 0);
        }
    } else if roll < eq - 0x50 {
        city.queue_pe_newspaper_story(0x40, 0);
    }
}

/// PE `FUN_00468150` LE-newspaper arms @ `0x468960`.
///
/// Reads BSS fertile LE @ `0x4cb410`: when `LE < 0x3c`, `rand()%0x3c > LE` queues
/// story `0x14`; when `LE >= 0x3c`, `rand()%0x3c < (LE-0x3c)` queues story `0x41`.
pub(super) fn pe_rci_demand_updates_le_newspaper_468960(city: &mut City, crt: &mut Win95CrtRand) {
    let life = city.pe_eq_life_average();
    let roll = u32::from(crt.random_mod(0x3c));
    if life < 0x3c {
        if roll > life {
            city.queue_pe_newspaper_story(0x14, 0);
        }
    } else if roll < life - 0x3c {
        city.queue_pe_newspaper_story(0x41, 0);
    }
}

/// PE `FUN_00468150` neighbor-fame newspaper @ `0x4689c8`.
///
/// Gate: `rand() & 7 == 0`. Then scan WORD[17] @ `0x4cb430`; for each nonzero rank,
/// when `(i16)[0x4cb020] + (i16)year_founded >= (i16)rank`, queue story `4` (index `< 7`)
/// or `5` (param `index-7`), clear that rank slot, and stop.
pub(super) fn pe_rci_demand_updates_neighbor_fame_newspaper_4689c8(
    city: &mut City,
    crt: &mut Win95CrtRand,
) {
    if crt.random_u15() & 7 != 0 {
        return;
    }
    let threshold = i32::from(city.pe_w_local_fame_base() as i16)
        .wrapping_add(i32::from(city.pe_w_year_founded as i16));
    for index in 0..17 {
        let Some(rank) = city.pe_w_neighbor_fame_rank(index) else {
            break;
        };
        if rank == 0 {
            continue;
        }
        if threshold >= i32::from(rank as i16) {
            let (story_id, param) = if index < 7 {
                (4_u16, index as u8)
            } else {
                (5_u16, (index - 7) as u8)
            };
            city.queue_pe_newspaper_story(story_id, param);
            // PE @ `0x468a0d` zeros `[esi*2+0x4cb430]`; mirror MISC tech-year dword too.
            if let Some(technology) = Technology::ALL.get(index).copied() {
                city.set_technology_discovery_year(technology, 0);
            } else {
                city.set_pe_w_neighbor_fame_rank(index, 0);
            }
            // PE @ `0x468a1a` → tech-unlock rebuild after a slot clear.
            let _ = city.pe_rebuild_tech_unlock_mask_46a0be(ordinance_flags_for_city(city));
            break;
        }
    }
}

impl ViewerState {
    /// Presentation-only welcome edition: no PE paper-list writes in Mac-layout saves.
    pub(super) fn maybe_show_founding_newspaper(&mut self) {
        if !self.founding_newspaper_pending || self.mode != crate::city::AppMode::CityView {
            return;
        }
        self.founding_newspaper_pending = false;
        if self.menu_checks.citygame_newspaper {
            self.open_city_info_dialog(CityInfoDialog::Newspaper(NewspaperEdition::Founding));
        }
    }

    /// PE `FUN_00471BC0` condition-newspaper chooser (through recreation/park arm).
    pub(super) fn maybe_queue_pe_monthly_condition_newspaper_story(&mut self) {
        if !self.engine_fidelity.uses_win95() {
            return;
        }
        let weather_trend = u32::from(self.city.pe_b_weather_trend);
        // PE condition newspaper @ `0x471c01` reads BSS `dwPowerUsedPercentage` @ `0x4caa50`.
        let power_used = u32::from(self.city.pe_power_used_percent());
        if let Some(story_id) = pe_monthly_condition_newspaper_story_id(
            &self.city,
            weather_trend,
            power_used,
            &mut self.pe_process_crt,
        ) {
            self.city.queue_pe_newspaper_story(story_id, 0);
            self.maybe_open_pe_newspaper_extra_popup();
        }
    }

    /// Win95 Extra auto-open after newspaper story insert (`0x47b681`).
    pub(super) fn maybe_open_pe_newspaper_extra_popup(&mut self) {
        if !self.engine_fidelity.uses_win95() || self.city.kind == CityFileKind::MacCity {
            return;
        }
        if !self.city.take_pe_newspaper_extra_popup_pending() {
            return;
        }
        if !self.menu_checks.citygame_newspaper {
            return;
        }
        if !newspaper_extra_enabled_for_city(&self.city) {
            return;
        }
        self.open_city_info_dialog(CityInfoDialog::Newspaper(NewspaperEdition::Extra));
        self.set_sim_status_announcement("Newspaper Extra: breaking story");
    }

    /// OpenSC2K DisasterEnd: open newspaper paper 0 when the disaster finishes.
    pub(super) fn maybe_open_pe_disaster_end_newspaper(&mut self) {
        if !self.engine_fidelity.uses_win95() {
            return;
        }
        if !self.city.take_pe_disaster_end_newspaper_pending() {
            return;
        }
        if !self.menu_checks.citygame_newspaper {
            return;
        }
        if self.city.kind == CityFileKind::MacCity {
            return;
        }
        // Force paper 0 before open; Subscription open keeps that choice.
        set_city_newspaper_choice_with_crt(&mut self.city, 0, Some(&mut self.pe_process_crt));
        self.open_city_info_dialog(CityInfoDialog::Newspaper(NewspaperEdition::Subscription));
        self.set_sim_status_announcement("Newspaper: disaster summary");
    }
}

/// PE `FUN_00471BC0` condition-newspaper index → story id (`index + 0x2E`).
/// Arms: power `0x2E`, transit `0x2F`, police/prison `0x30`, fire `0x31`, water `0x32`,
/// hospital `0x33`, school `0x34`, seaport/desal vs industrial tax `0x35`/`0x3B`,
/// airport vs commercial tax `0x3C` / Free Clinics `0x36`, recreation
/// parks/stadium/zoo/marina vs residential tax `0x37..=0x3A`.
fn pe_monthly_condition_newspaper_story_id(
    city: &City,
    weather_trend: u32,
    power_used_percent: u32,
    crt: &mut Win95CrtRand,
) -> Option<u16> {
    // PE `cmp [0x4cb40c], 9` / `jc` power arm: severe trends take a different paper path.
    if weather_trend >= 9 {
        return None;
    }
    // @ 0x471c01: `dwPowerUsedPercentage` (`0x4caa50`) > `0x62` → index 0.
    if power_used_percent > 0x62 {
        return Some(0x2e);
    }
    // @ 0x471c13: `dwTileCount[0xe9]`+`[0xed]` (`0x4ca4c8+0x1d2`/`+0x1da`) + roads current
    // compared to `dwCityPopulation / 100`; if <= → index 1.
    let pop = city.pe_dw_city_population();
    let subway = u32::from(city.pe_dw_tile_count[SUBWAY_STATION_BUILDING_ID as usize]);
    let rail = u32::from(city.pe_dw_tile_count[RAIL_STATION_BUILDING_ID as usize]);
    let roads_current = budget_service_rows_for_city(city)[5].current;
    let transit_score = subway.saturating_add(rail).saturating_add(roads_current);
    if transit_score <= pop / 100 {
        return Some(0x2f);
    }
    // @ 0x471c50: pop < 1000 → no story.
    if pop < 0x3e8 {
        return None;
    }
    // @ 0x471c65: (`dwTileCount[0xd2]`/9 + `dwTileCount[0xd8]`/16) <= pop/0x4e20 → index 2.
    // `esi` keeps `pop/0x4e20` for fire and school reuse.
    let demand = pop / 0x4e20;
    let police_count = u32::from(city.pe_dw_tile_count[210]) / 9;
    let prison_count = u32::from(city.pe_dw_tile_count[216]) / 16;
    if police_count.saturating_add(prison_count) <= demand {
        return Some(0x30);
    }
    // @ 0x471cb6: `dwTileCount[0xd3]`/9 <= pop/0x4e20 → index 3.
    let fire_count = u32::from(city.pe_dw_tile_count[FIRE_STATION_BUILDING_ID as usize]) / 9;
    if fire_count <= demand {
        return Some(0x31);
    }
    // @ 0x471cdb: PE `0x4CC4B8` water coverage (`pe_water_coverage_percent`) > `0x62` → index 4.
    if u32::from(city.pe_water_coverage_percent()) > 0x62 {
        return Some(0x32);
    }
    // @ 0x471cee: pop < 3000 → no story.
    if pop < 0xbb8 {
        return None;
    }
    // @ 0x471d03: `dwTileCount[0xd1]` (hospital 209) / 9 <= pop/0x61a8 → index 5.
    let hospital_count = u32::from(city.pe_dw_tile_count[HOSPITAL_BUILDING_ID as usize]) / 9;
    if hospital_count <= pop / 0x61a8 {
        return Some(0x33);
    }
    // @ 0x471d36: `dwTileCount[0xd6]` (school 214) / 9 <= `esi` (pop/0x4e20) → index 6.
    let school_count = u32::from(city.pe_dw_tile_count[SCHOOL_BUILDING_ID as usize]) / 9;
    if school_count <= demand {
        return Some(0x34);
    }
    // @ 0x471d5b: pop < 8000 → no story.
    if pop < 0x1f40 {
        return None;
    }
    // @ 0x471d70: `dwTileCount[0xe0]` (crane 224)+word `0x4ca3f0` (XTXT 0xfa on non-network XBLD;
    // PE place/dec / DestroyBuilding else-arm — not desal building-250 tiles) vs
    // `(INDFUND current − arco_pop/12) / 0x2710`. Shortage → index 7 (`0x35`) with
    // coast/river, else index 13 (`0x3B`).
    let crane_tiles = u32::from(city.pe_dw_tile_count[SEAPORT_CRANE_BUILDING_ID as usize]);
    let fa_non_network = citygame_fa_non_network_neighbor_marker_count(city);
    let seaport_score = i64::from(crane_tiles.saturating_add(fa_non_network));
    let arco = i64::from(city.pe_dw_arcology_population());
    let industrial_tax = i64::from(
        budget_row_misc_offset_for_city(city, 2, BUDGET_SERVICE_CURRENT_FIELD)
            .and_then(|offset| city.budget_misc_u32_at(offset))
            .unwrap_or(0),
    );
    let tax_threshold = (industrial_tax - arco / 12) / 0x2710;
    if seaport_score < tax_threshold {
        if pe_city_has_ocean(city) || pe_city_has_river(city) {
            return Some(0x35);
        }
        return Some(0x3b);
    }
    // @ 0x471dc4: `dwTileCount[0xdd]`+`[0xde]` + word `0x4cc4d8` (XTXT 0xfa on
    // network-class XBLD; PE place/dec / load census) vs `(COMFUND current − arco/12) / 0x7d0`.
    // Shortage → Free Clinics (`0x4caa40` bit `0x40` / PE `0x4e9a30` bit2) → `0x36`, else `0x3C`.
    let runway_tiles = u32::from(city.pe_dw_tile_count[RUNWAY_BUILDING_ID as usize]);
    let runway_cross_tiles =
        u32::from(city.pe_dw_tile_count[AIRPORT_RUNWAY_CROSS_BUILDING_ID as usize]);
    let fa_network = pe_fa_network_overlay_count_4cc4d8(city);
    let airport_score = i64::from(
        runway_tiles
            .saturating_add(runway_cross_tiles)
            .saturating_add(fa_network),
    );
    let commercial_tax = i64::from(
        budget_row_misc_offset_for_city(city, 1, BUDGET_SERVICE_CURRENT_FIELD)
            .and_then(|offset| city.budget_misc_u32_at(offset))
            .unwrap_or(0),
    );
    let airport_threshold = (commercial_tax - arco / 12) / 0x7d0;
    if airport_score < airport_threshold {
        // PE `0x4e9a30` bit2 @ `0x471dfd`: set → story `0x36`, clear → `0x3C`.
        // Tech-unlock arm 6 ORs this bit when fame-rank[6] is zero (`0x46a116`).
        if city.pe_dw_tech_ui_4e9a30() & 2 != 0 {
            return Some(0x36);
        }
        return Some(0x3c);
    }
    // @ 0x471e13: `dwTileCount[0xd5]`/3 + `[0xd7]` + `[0xda]` + `[0xf8]` vs
    // `(RESFUND current − 2*(arco/12)) / 0x3e8`. Shortage → `rand()&3 + 9` →
    // stories `0x37..=0x3A`.
    let park_units = u32::from(city.pe_dw_tile_count[BIG_PARK_BUILDING_ID as usize]) / 3;
    let stadium_tiles = u32::from(city.pe_dw_tile_count[STADIUM_BUILDING_ID as usize]);
    let zoo_tiles = u32::from(city.pe_dw_tile_count[ZOO_BUILDING_ID as usize]);
    let marina_tiles = u32::from(city.pe_dw_tile_count[MARINA_BUILDING_ID as usize]);
    let recreation_score = i64::from(
        park_units
            .saturating_add(stadium_tiles)
            .saturating_add(zoo_tiles)
            .saturating_add(marina_tiles),
    );
    let residential_tax = i64::from(
        budget_row_misc_offset_for_city(city, 0, BUDGET_SERVICE_CURRENT_FIELD)
            .and_then(|offset| city.budget_misc_u32_at(offset))
            .unwrap_or(0),
    );
    let recreation_threshold = (residential_tax - (arco / 12) * 2) / 0x3e8;
    if recreation_score < recreation_threshold {
        let index = u16::from(crt.random_u15() & 3).saturating_add(9);
        return Some(0x2eu16.saturating_add(index));
    }
    None
}
