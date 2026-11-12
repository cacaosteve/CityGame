//! City loading and blank-city construction.

use super::layers::infer_corner_key_bits;
use super::*;

pub(super) fn city_name_from_chunks(
    path: &Path,
    chunks: &[CityChunk],
    kind: CityFileKind,
) -> String {
    let name = chunks
        .iter()
        .find(|chunk| chunk.name == "CNAM")
        .map(|chunk| cnam_text(&chunk.data));
    if kind.is_win95() {
        // The Win95 loader resets the city-name string to empty before dispatching
        // chunks. A missing or zero-length CNAM therefore stays empty; it does not
        // inherit the source filename.
        name.unwrap_or_default()
    } else {
        name.filter(|name| !name.is_empty()).unwrap_or_else(|| {
            path.file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("Untitled")
                .to_owned()
        })
    }
}

impl City {
    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        let bytes = fs::read(&path).with_context(|| format!("reading city {}", path.display()))?;
        let kind = detect_city_kind(&path, &bytes)?;
        let chunks = parse_scdh_chunks(&bytes)?;
        let mac_budget_book = if kind == CityFileKind::MacCity {
            let start = MAC_MISC_BUDGET_BOOK_OFFSET;
            let len = MISC_BUDGET_ROW_DWORDS * MISC_BUDGET_ROW_COUNT * MISC_FIELD_LEN;
            let raw = decode_misc_bytes_for_kind(&chunks, kind).unwrap_or_default();
            let mut book = vec![0; len];
            if let Some(saved) = raw.get(start..start.saturating_add(len)) {
                book.copy_from_slice(saved);
            } else if let Some(saved) = raw.get(start..) {
                book[..saved.len().min(len)].copy_from_slice(&saved[..saved.len().min(len)]);
            }
            Some(book)
        } else {
            None
        };
        let chunk_index = chunks
            .iter()
            .enumerate()
            .map(|(index, chunk)| (chunk.name.clone(), index))
            .collect::<HashMap<_, _>>();
        // Win95's loader dispatches these buffers through size-aware readers. ALTM is
        // 0x8000 bytes, MISC's versioned record is 0x12c0 bytes, and XGRP is 0xd00.
        // Keep absent optional chunks absent, but reject a present payload that cannot
        // satisfy the PE read contract instead of silently dropping its decoded state.
        decode_optional_chunk_layer(&chunks, "MISC", kind, |chunks| {
            decode_win95_misc_record(chunks).map(|_| ())
        })?;
        for (name, expected) in [("XGRP", 0xd00)] {
            decode_optional_chunk_layer(&chunks, name, kind, |chunks| {
                decode_expected_chunk_for_kind(chunks, name, expected, kind).map(|_| ())
            })?;
        }
        let altm =
            decode_optional_chunk_layer(&chunks, "ALTM", kind, |chunks| decode_altm(chunks, kind))?;
        let name = city_name_from_chunks(&path, &chunks, kind);
        let terrain = decode_optional_chunk_layer(&chunks, "XTER", kind, |chunks| {
            decode_expected_chunk_for_kind(chunks, "XTER", CITY_TILES, kind)
        })?;
        let buildings = decode_optional_chunk_layer(&chunks, "XBLD", kind, |chunks| {
            decode_expected_chunk_for_kind(chunks, "XBLD", CITY_TILES, kind)
        })?;
        let underground = decode_optional_chunk_layer(&chunks, "XUND", kind, |chunks| {
            decode_expected_chunk_for_kind(chunks, "XUND", CITY_TILES, kind)
        })?;
        let zones = decode_optional_chunk_layer(&chunks, "XZON", kind, |chunks| {
            decode_zones(chunks, kind)
        })?;
        let text_overlays = decode_optional_chunk_layer(&chunks, "XTXT", kind, |chunks| {
            decode_expected_chunk_for_kind(chunks, "XTXT", CITY_TILES, kind)
        })?;
        let labels = decode_optional_chunk_layer(&chunks, "XLAB", kind, |chunks| {
            decode_labels(chunks, kind)
        })?;
        let microsims =
            decode_optional_chunk_layer(&chunks, "XMIC", kind, |chunks| decode_xmic(chunks, kind))?;
        let tile_bits = decode_optional_chunk_layer(&chunks, "XBIT", kind, |chunks| {
            decode_tile_bits(chunks, kind)
        })?;
        let (traffic_density, xtrf_storage) = if chunks.iter().any(|chunk| chunk.name == "XTRF") {
            let (data, format) = decode_xtrf(&chunks, kind).context("loading XTRF chunk")?;
            (Some(data), format)
        } else {
            (None, XtrfStorageFormat::PerTile128)
        };
        let land_value_density = decode_optional_chunk_layer(&chunks, "XVAL", kind, |chunks| {
            decode_microsim64_chunk(chunks, "XVAL", kind)
        })?;
        let crime_density = decode_optional_chunk_layer(&chunks, "XCRM", kind, |chunks| {
            decode_microsim64_chunk(chunks, "XCRM", kind)
        })?;
        // Pollution is `XPLT` (64×64). Older builds of this port mistakenly wrote pollution
        // into `XPLC` (police); accept that as a fallback when `XPLT` is missing.
        // Preserve the legacy port format that stored its full-resolution pollution map in
        // XPLC when XPLT was absent; do not misclassify that payload as malformed coverage.
        let legacy_xplc_pollution = !chunks.iter().any(|chunk| chunk.name == "XPLT")
            && decode_expected_chunk_for_kind(&chunks, "XPLC", CITY_TILES, kind).is_ok();
        let police_coverage = if legacy_xplc_pollution {
            None
        } else {
            decode_optional_microsim32_chunk(&chunks, "XPLC", kind)?
        };
        let fire_coverage = decode_optional_microsim32_chunk(&chunks, "XFIR", kind)?;
        let population_density = decode_optional_microsim32_chunk(&chunks, "XPOP", kind)?;
        let rate_of_growth = decode_optional_microsim32_chunk(&chunks, "XROG", kind)?;
        let pollution_density = if chunks.iter().any(|chunk| chunk.name == "XPLT") {
            Some(decode_microsim64_chunk(&chunks, "XPLT", kind).context("loading XPLT chunk")?)
        } else if police_coverage.is_none() {
            decode_expected_chunk_for_kind(&chunks, "XPLC", CITY_TILES, kind).ok()
        } else {
            None
        };
        let traffic_overlays =
            decode_optional_chunk_layer(&chunks, "XTHG", kind, |chunks| decode_xthg(chunks, kind))?;
        let traffic_overlays_from_file = traffic_overlays.is_some();
        let saved_rotation = decode_rotation_for_kind(&chunks, kind).unwrap_or(0);
        let misc_crime_count = decode_misc_u32_for_kind(&chunks, MISC_CITY_CRIME_OFFSET, kind).ok();
        let misc_traffic_count =
            decode_misc_u32_for_kind(&chunks, MISC_CITY_TRAFFIC_OFFSET, kind).ok();
        let misc_pollution_count =
            decode_misc_u32_for_kind(&chunks, MISC_CITY_POLLUTION_OFFSET, kind).ok();
        let misc_humidity = decode_misc_u32_for_kind(&chunks, MISC_HUMIDITY_OFFSET, kind)
            .ok()
            .map(|value| u32::from(weather_cache_byte_from_misc(value)));
        let misc_water_table_level =
            decode_misc_u32_for_kind(&chunks, MISC_WATER_TABLE_LEVEL_OFFSET, kind).ok();
        let saved_date = decode_city_date(&chunks, kind).ok();
        let saved_view = decode_city_view(&chunks, kind).ok();
        let corner_key_bits = infer_corner_key_bits(buildings.as_deref(), zones.as_deref());
        let pe_rci_demand_initialized = chunks.iter().any(|chunk| chunk.name == "MISC");
        // CGEX carries selected-engine state where native MISC bytes belong
        // to the other edition. Decode it for either container layout.
        let citygame_extension = decode_citygame_extension(&chunks);
        let mac_saved_state = citygame_extension
            .as_ref()
            .filter(|_| kind.is_win95())
            .and_then(|extension| extension.mac_saved_state.as_ref());
        let mac_pending_action_runtime = citygame_extension
            .as_ref()
            .filter(|_| kind.is_win95())
            .map(|extension| extension.mac_pending_action)
            .unwrap_or(0);

        let mut city = Self {
            path,
            kind,
            name,
            chunks,
            chunk_index,
            mac_budget_book,
            terrain,
            buildings,
            underground,
            altm,
            zones,
            text_overlays,
            labels,
            microsims,
            tile_bits,
            traffic_density,
            land_value_density,
            crime_density,
            pollution_density,
            police_coverage,
            fire_coverage,
            population_density,
            rate_of_growth,
            xtrf_storage,
            traffic_overlays,
            traffic_overlays_from_file,
            pe_active_train_count_override: None,
            pe_tornado_actor_counter: 0,
            mac_marina_actor_counter: 0,
            mac_kind3_actor_counter: 0,
            mac_kind1_actor_counter: 0,
            mac_kind2_actor_counter: 0,
            mac_kind5_actor_counter: 0,
            mac_kind16_actor_counter: 0,
            mac_tornado_actor_counter: 0,
            mac_rail_actor_counter: 0,
            mac_developed_cell_snapshot: mac_saved_state.and_then(|state| state.developed_cells),
            mac_transit_accumulators_runtime: mac_saved_state
                .map(|state| state.transit_accumulators)
                .unwrap_or([0; 3]),
            mac_pending_action_runtime,
            mac_scenario_active_runtime: mac_saved_state
                .and_then(|state| state.mac_scenario_active),
            mac_scenario_runtime: mac_saved_state.and_then(|state| state.scenario),
            mac_rci_counters_runtime: mac_saved_state
                .map(|state| state.rci_counters)
                .unwrap_or([0; 8]),
            mac_population_change_balances: [0; 2],
            pe_cross_layout_active: false,
            pe_cross_layout_saved_neighbor_fame_ranks: citygame_extension
                .as_ref()
                .and_then(|extension| extension.pe_neighbor_fame_ranks),
            pe_cross_layout_saved_current_disaster_type: citygame_extension
                .as_ref()
                .and_then(|extension| extension.pe_current_disaster_type),
            pe_cross_layout_saved_disaster_active: citygame_extension
                .as_ref()
                .and_then(|extension| extension.pe_disaster_active),
            pe_cross_layout_saved_old_arrests: citygame_extension
                .as_ref()
                .and_then(|extension| extension.pe_old_arrests),
            pe_cross_layout_saved_prison_bonus: citygame_extension
                .as_ref()
                .and_then(|extension| extension.pe_prison_bonus),
            pe_cross_layout_saved_sewer_bonus: citygame_extension
                .as_ref()
                .and_then(|extension| extension.pe_sewer_bonus),
            pe_cross_layout_saved_city_has_ocean: citygame_extension
                .as_ref()
                .and_then(|extension| extension.pe_city_has_ocean),
            pe_cross_layout_saved_city_has_river: citygame_extension
                .as_ref()
                .and_then(|extension| extension.pe_city_has_river),
            pe_cross_layout_saved_xund_occupancy: citygame_extension
                .as_ref()
                .and_then(|extension| extension.pe_xund_occupancy),
            pe_cross_layout_saved_city_progression: citygame_extension
                .as_ref()
                .and_then(|extension| extension.pe_city_progression),
            pe_cross_layout_saved_rewards_available: citygame_extension
                .as_ref()
                .and_then(|extension| extension.pe_rewards_available),
            pe_cross_layout_saved_industrial_mix_bonus: citygame_extension
                .as_ref()
                .and_then(|extension| extension.pe_industrial_mix_bonus),
            pe_cross_layout_saved_industrial_mix_pollution_bonus: citygame_extension
                .as_ref()
                .and_then(|extension| extension.pe_industrial_mix_pollution_bonus),
            pe_cross_layout_saved_traffic_count: citygame_extension
                .as_ref()
                .and_then(|extension| extension.pe_traffic_count),
            pe_cross_layout_saved_monster_xthg_index: citygame_extension
                .as_ref()
                .and_then(|extension| extension.pe_monster_xthg_index),
            mac_cross_layout_active: false,
            mac_road_neighbor_link_count_runtime: mac_saved_state
                .and_then(|state| state.road_neighbor_links),
            mac_residential_total_runtime: mac_saved_state
                .and_then(|state| state.residential_total),
            mac_cross_layout_saved_civic_latch: citygame_extension
                .as_ref()
                .and_then(|extension| extension.mac_civic_latch),
            mac_xbld_census_runtime: mac_saved_state
                .map(|state| state.xbld_census)
                .unwrap_or([0; 256]),
            mac_cross_layout_state_restored: mac_saved_state.is_some(),
            mac_kind3_boundary_target: (0, 0),
            mac_kind2_hotspot_runtime: mac_saved_state.and_then(|state| state.developed_hotspot),
            pe_sailboat_budget: 0,
            pe_xund_occupancy: 0,
            pe_dw_tile_count: [0; 256],
            pe_nonnetwork_neighbor_links: 0,
            pe_road_link_census_delta: 0,
            pe_synthetic_trip_markers: vec![false; CITY_TILES],
            pe_military_tile_count: [0; MILITARY_TILE_COUNT_VALUES],
            pe_city_value: 0,
            pe_cargo_spawn_tile: (0, 0),
            dispatch_cursors: [0; 3],
            mac_dispatch: Default::default(),
            pe_rci_demand_initialized,
            pe_water_coverage_percent: 100,
            pe_power_used_percent: 100,
            pe_dw_bus_passengers: 0,
            pe_dw_rail_passengers: 0,
            pe_dw_subway_passengers: 0,
            mac_monthly_arrest_total: 0,
            mac_mayor_approval: 0,
            mac_newspaper_suppressed: false,
            mac_police_gate_runtime: mac_saved_state.map(|state| state.police_gate).unwrap_or(0),
            mac_arcology_total_runtime: mac_saved_state.and_then(|state| state.arcology_total),
            mac_power_summary_runtime: mac_saved_state
                .map(|state| state.power_summary)
                .unwrap_or(0),
            mac_water_summary_runtime: mac_saved_state
                .map(|state| state.water_summary)
                .unwrap_or(0),
            mac_treatment_gate_runtime: mac_saved_state.and_then(|state| state.treatment_gate),
            pe_age_cohorts: [0; 20],
            pe_eq_life_points: [0; 20],
            pe_eq_aux_points: [0; 20],
            pe_eq_deaths: 0,
            pe_eq_births: 0,
            pe_dw_city_population: 0,
            pe_dw_arcology_population: 0,
            pe_w_sports_teams: 0,
            pe_w_industrial_mix_bonus: 0,
            pe_w_industrial_mix_pollution_bonus: 0,
            pe_w_old_arrests: 0,
            pe_w_prison_bonus: 0,
            pe_w_sewer_bonus: 0,
            pe_w_monster_xthg_index: 0xffff,
            pe_w_current_disaster_type: 0,
            pe_disaster_damage_class: -1,
            pe_dw_disaster_active: 0,
            pe_dw_city_has_ocean: 0,
            pe_dw_city_has_river: 0,
            pe_dw_national_population: 0,
            pe_dw_national_value: 0,
            pe_w_national_tax: 0,
            pe_w_national_trend: 0,
            pe_eq_growth_slot7: 0,
            pe_zone_pops: [0; 3],
            pe_zone_pops_ready: false,
            pe_eq_leftover_deaths: 0,
            pe_eq_leftover_births: 0,
            pe_eq_life_average: 0,
            pe_eq_education_average: 0,
            pe_eq_workforce_percent: 0,
            pe_eq_averages_ready: false,
            pe_life_expectancy: 70,
            pe_mayor_approval: 0,
            pe_unemployment_percent: 0,
            pe_city_crime: 0,
            pe_city_pollution: 0,
            pe_traffic_count: 0,
            pe_attitude_tax_bases: [0; 3],
            pe_w_city_developed_tiles: 0,
            pe_dw_city_pollution: 0,
            pe_dw_city_land_value: 0,
            pe_i32_city_funds: 0,
            pe_dw_city_age: 0,
            pe_w_city_difficulty: 0,
            pe_w_year_founded: 0,
            pe_dw_city_bonds: 0,
            pe_w_city_mode: 0,
            pe_w_city_rotation: 0,
            pe_w_city_progression: 0,
            pe_w_military_base_type: 0,
            pe_w_city_garbage: 0,
            pe_dw_city_fame: 0,
            pe_dw_city_advertising: 0,
            pe_b_weather_trend: 0,
            pe_b_weather_heat: 0,
            pe_b_weather_wind: 0,
            pe_b_weather_humid: 0,
            pe_w_trigger_disaster: 0,
            pe_w_last_cheat_sound_id: 0,
            pe_pending_effect_sound_ids: Vec::new(),
            pe_pending_auto_goto: Vec::new(),
            pe_dw_residential_population: 0,
            pe_dw_rewards_available: 0,
            pe_dw_city_crime: 0,
            pe_attitude_totals_ready: false,
            pe_attitude_scores: [0; 4],
            pe_w_local_fame_base: 0,
            pe_w_calendar_month_mod: 0,
            pe_w_season: 0,
            pe_w_tech_unlock_mask: 0,
            pe_w_tech_unlock_high_zero_count: 0,
            pe_dw_tech_ui_4e9a10: [0; 18],
            pe_w_neighbor_fame_ranks: [0; PE_NEIGHBOR_FAME_RANK_COUNT],
            pe_newspaper_extra_popup_pending: false,
            pe_newspaper_storage: PeNewspaperStorage::default(),
            pe_disaster_end_newspaper_pending: false,
            misc_crime_count,
            misc_traffic_count,
            misc_pollution_count,
            pe_scenario_deadline_expired: false,
            pe_scenario_active: kind == CityFileKind::WinScenario,
            misc_humidity,
            misc_water_table_level,
            saved_date,
            saved_view,
            corner_key_bits,
            rotation: 0,
            saved_rotation,
            dirty_tiles: HashSet::new(),
            modified: false,
        };
        city.hydrate_pe_eq_le_from_misc();
        // PE MISC→BSS arco pop @ `0x431b8e` (`[MISC+0x1020]` → `[0x4c94c4]`).
        // Mac CODE 4 restores `fffffe0a` from the same slot, so a CityGame
        // Mac-state extension is also a valid newer value for this shared role.
        city.pe_dw_arcology_population = if kind.is_win95() {
            mac_saved_state
                .and_then(|state| state.arcology_total)
                .or_else(|| city.misc_u32_at(MISC_ARCOLOGY_POPULATION_OFFSET))
                .unwrap_or(0)
        } else {
            0
        };
        // PE MISC→BSS sports teams @ `0x431ba9` (word `[MISC+0x1028]` → `[0x4cc4e0]`).
        // Mac uses the same saved slot for its stadium choice mask; it is
        // hydrated into the PE cache if Win95 fidelity is selected.
        city.pe_w_sports_teams = if kind.is_win95() {
            city.misc_u32_at(MISC_SPORTS_TEAMS_OFFSET).unwrap_or(0) as u16
        } else {
            0
        };
        // PE MISC→BSS neighbor-fame ranks @ `0x43179f` (WORD[17] `[MISC+0x738]` → `[0x4cb430]`).
        // Mac CODE 4 uses the same saved technology-date table, but its
        // engine-owned timeline can differ. Do not hydrate PE's cache from a
        // Mac-layout file; PE cross-layout continuation comes from CGEX.
        if kind.is_win95() {
            city.hydrate_pe_w_neighbor_fame_ranks_from_misc_43179f();
        }
        // On Mac files, PE's runtime view is prepared at Win95 engine open;
        // Mac sessions keep their own native producer for this saved scalar.
        city.pe_dw_city_population = if kind.is_win95() {
            city.misc_u32_at(MISC_NORMAL_POPULATION_OFFSET).unwrap_or(0)
        } else {
            0
        };
        // PE reads these low WORDs on first Win95-fidelity open. On a Mac
        // container they overlap Mac-owned demand and pollution state, so keep
        // the PE cache neutral until that engine is explicitly selected.
        let mix = city
            .misc_u32_at(MISC_INDUSTRIAL_MIX_BONUS_OFFSET)
            .unwrap_or(0);
        city.pe_w_industrial_mix_bonus = if kind.is_win95() {
            mix.min(u32::from(u16::MAX)) as u16
        } else {
            0
        };
        city.pe_w_industrial_mix_pollution_bonus = if kind.is_win95() {
            city.misc_u32_at(MISC_INDUSTRIAL_MIX_POLLUTION_BONUS_OFFSET)
                .unwrap_or(0) as i16
        } else {
            0
        };
        // These other PE caches still overlap distinct Mac state.
        if kind.is_win95() {
            // PE MISC→BSS old arrests @ `0x431bf5` (word `[MISC+0x1038]` → `[0x4ca430]`).
            city.pe_w_old_arrests = city
                .misc_u32_at(MISC_OLD_ARRESTS_OFFSET)
                .unwrap_or(0)
                .min(u32::from(u16::MAX)) as u16;
            // PE MISC→BSS prison bonus @ `0x431c04` (word `[MISC+0x103C]` → `[0x4ca1dc]`).
            city.pe_w_prison_bonus = city
                .misc_u32_at(MISC_PRISON_BONUS_OFFSET)
                .unwrap_or(0)
                .min(u32::from(u16::MAX)) as u16;
            // PE MISC→BSS monster XTHG index @ `0x431c0e` (word `[MISC+0x1040]` → `[0x4ca81c]`).
            city.pe_w_monster_xthg_index = city
                .misc_u32_at(MISC_MONSTER_XTHG_INDEX_OFFSET)
                .unwrap_or(0xffff)
                .min(u32::from(u16::MAX)) as u16;
            // PE MISC→BSS current disaster type @ `0x431c2c` (word `[MISC+0x1044]` → `[0x4ca93c]`).
            city.pe_w_current_disaster_type = city
                .misc_u32_at(MISC_CURRENT_DISASTER_TYPE_OFFSET)
                .unwrap_or(0)
                .min(u32::from(u16::MAX)) as u16;
            // PE MISC→BSS disaster active @ `0x431c3b` (`[MISC+0x1048]` → `[0x4c9ee8]`).
            city.pe_dw_disaster_active = city.misc_u32_at(MISC_DISASTER_ACTIVE_OFFSET).unwrap_or(0);
            // PE MISC→BSS sewer bonus @ `0x431c45` (word `[MISC+0x1050]` → `[0x4cb3dc]`).
            city.pe_w_sewer_bonus = city
                .misc_u32_at(MISC_SEWER_BONUS_OFFSET)
                .unwrap_or(0)
                .min(u32::from(u16::MAX)) as u16;
        }
        // PE MISC→BSS ocean/river @ `0x43189a`/`0x4318ab` (`[MISC+0x0e44]`/`[MISC+0x0e48]`).
        // Mac CODE 4 saves its native coast/river options in the same slots.
        // Defer that PE interpretation until Win95 fidelity is selected.
        if kind.is_win95() {
            city.pe_dw_city_has_ocean = city.misc_u32_at(MISC_CITY_HAS_OCEAN_OFFSET).unwrap_or(0);
            city.pe_dw_city_has_river = city.misc_u32_at(MISC_CITY_HAS_RIVER_OFFSET).unwrap_or(0);
            // PE MISC→BSS xund occupancy @ `0x431a40` (word `[MISC+0xfe8]` → `[0x4ca41c]`).
            city.pe_xund_occupancy =
                city.misc_u32_at(MISC_XUND_OCCUPANCY_OFFSET).unwrap_or(0) as u16;
            // PE MISC→BSS military tile counts @ `0x431a0b` (16 words `[MISC+0xfa8]` → `[0x4ca934]`).
            for index in 0..MILITARY_TILE_COUNT_VALUES {
                city.pe_military_tile_count[index] = city
                    .misc_u32_at(MISC_MILITARY_TILE_COUNTS_OFFSET + index * 4)
                    .unwrap_or(0) as u16;
            }
        }
        // PE MISC→BSS national pop @ `0x4314d6` (`[MISC+0x50]` → `[0x4ca928]`).
        city.pe_dw_national_population = city
            .misc_u32_at(MISC_NATIONAL_POPULATION_OFFSET)
            .unwrap_or(0);
        // PE MISC→BSS national value @ `0x4314df` (`[MISC+0x54]` → `[0x4cb014]`).
        city.pe_dw_national_value = city.misc_u32_at(MISC_NATIONAL_VALUE_OFFSET).unwrap_or(0);
        // PE MISC→BSS national tax @ `0x4314e8` (word `[MISC+0x58]` → `[0x4ca938]`).
        city.pe_w_national_tax = city
            .misc_u32_at(MISC_NATIONAL_TAX_OFFSET)
            .map(|value| misc_word_from_saved_dword(value, kind))
            .unwrap_or(0);
        // PE MISC→BSS national trend @ `0x4314f2` (word `[MISC+0x5c]` → `[0x4ca1bc]`).
        city.pe_w_national_trend = city
            .misc_u32_at(MISC_NATIONAL_TREND_OFFSET)
            .map(|value| misc_word_from_saved_dword(value, kind))
            .unwrap_or(0);
        // PE MISC→BSS city value @ `0x430878` (`[MISC+0x24]` → `[0x4ca4d0]`).
        city.pe_city_value = city
            .misc_u32_at(MISC_CITY_VALUE_OFFSET)
            .map(|v| v as i32)
            .unwrap_or(0);
        // PE MISC→BSS land value @ `0x43147f` (`[MISC+0x28]` → `[0x4ca440]`).
        city.pe_dw_city_land_value = city.misc_u32_at(MISC_LAND_VALUE_OFFSET).unwrap_or(0);
        // PE MISC→BSS city funds @ `0x43144d` (`[MISC+0x14]` → `[0x4ca444]`).
        city.pe_i32_city_funds = city.misc_i32_at(MISC_CITY_FUNDS_OFFSET).unwrap_or(0);
        // PE MISC→BSS city age @ `0x431444` (`[MISC+0x10]` → `[0x4cae04]`).
        // Mac CODE 14 counts one persisted tick per in-game calendar day:
        // 25 per month and 300 per year, the same numeric scale used by the
        // PE age-in-days field. Keep the shared date/age cache hydration.
        city.pe_dw_city_age = city.misc_u32_at(MISC_CITY_AGE_OFFSET).unwrap_or(0);
        city.pe_publish_graph_calendar_words_from_city_age_41354e();
        // PE MISC→BSS difficulty @ `0x43145f` (word `[MISC+0x1c]` → `[0x4cb404]`).
        city.pe_w_city_difficulty = city
            .misc_u32_at(MISC_DIFFICULTY_OFFSET)
            .map(|value| misc_word_from_saved_dword(value, kind))
            .unwrap_or(0);
        // PE MISC→BSS year founded @ `0x43143a` (word `[MISC+0x0c]` → `[0x4ca5f4]`).
        city.pe_w_year_founded = city
            .misc_u32_at(MISC_YEAR_FOUNDED_OFFSET)
            .map(|value| misc_year_founded_word(value, kind))
            .unwrap_or(0);
        // PE MISC→BSS bond count @ `0x431456` (`[MISC+0x18]` → `[0x4cc4e8]`).
        city.pe_dw_city_bonds = city.misc_u32_at(MISC_CITY_BONDS_OFFSET).unwrap_or(0);
        // PE MISC→BSS city mode @ `0x431426` (word `[MISC+0x04]` → `[0x4ca42c]`).
        // Mac CODE 4 owns this field as its run-state gate, not PE editor mode.
        if kind.is_win95() {
            city.pe_w_city_mode = city
                .misc_u32_at(MISC_CITY_MODE_OFFSET)
                .unwrap_or(0)
                .min(u32::from(u16::MAX)) as u16;
        }
        // PE MISC→BSS city rotation @ `0x431430` (word `[MISC+0x08]` → `[0x4c942c]`).
        city.pe_w_city_rotation = city.misc_u32_at(MISC_CITY_ROTATION_OFFSET).unwrap_or(0) as u16;
        // PE MISC→BSS city progression @ `0x431469` (word `[MISC+0x20]` → `[0x4cb010]`).
        city.pe_w_city_progression =
            city.misc_u32_at(MISC_CITY_PROGRESSION_OFFSET).unwrap_or(0) as u16;
        // PE loads the low byte of this shared selector; Mac engine sessions
        // leave PE-specific caches unprepared until the engine is selected.
        city.pe_w_military_base_type = if city.kind == CityFileKind::MacCity {
            0
        } else {
            u32::from(
                city.misc_u32_at(MISC_MILITARY_BASE_TYPE_OFFSET)
                    .unwrap_or(0) as u8,
            )
        };
        // PE MISC→BSS garbage @ `0x4314b5` (`[MISC+0x40]` → `[0x4ca5f0]`).
        // Mac CODE 4 owns +0x40 as its monthly growth scalar.
        if kind.is_win95() {
            city.pe_w_city_garbage = city.misc_u32_at(MISC_GARBAGE_OFFSET).unwrap_or(0);
        }
        // PE MISC→BSS fame @ `0x4314a5` (`[MISC+0x38]` → `[0x4cad28]`).
        // Mac CODE 4 preserves +0x38/+0x3c as opaque state; PE fame and
        // advertising do not have a proven Mac mapping.
        city.pe_dw_city_fame = if kind.is_win95() {
            city.misc_u32_at(MISC_CITY_FAME_OFFSET).unwrap_or(0)
        } else {
            0
        };
        // PE MISC→BSS advertising @ `0x4314ae` (`[MISC+0x3c]` → `[0x4cb018]`).
        city.pe_dw_city_advertising = if kind.is_win95() {
            city.misc_u32_at(MISC_CITY_ADVERTISING_OFFSET).unwrap_or(0)
        } else {
            0
        };
        // PE MISC→BSS weather trend @ `0x43151e` (`[MISC+0x6c]` → `[0x4cb40c]`).
        // Mac layout stores only the low byte of this weather-state dword.
        let weather_trend = city.misc_u32_at(MISC_WEATHER_TREND_OFFSET).unwrap_or(0);
        city.pe_b_weather_trend = weather_cache_byte_from_misc(weather_trend);
        // PE MISC→BSS weather heat @ `0x431505` (`[MISC+0x60]` → `[0x4cade0]`).
        let weather_heat = city.misc_u32_at(MISC_HEAT_OFFSET).unwrap_or(0);
        city.pe_b_weather_heat = weather_cache_byte_from_misc(weather_heat);
        // PE MISC→BSS weather wind @ `0x43150d` (`[MISC+0x64]` → `[0x4cae0c]`).
        let weather_wind = city.misc_u32_at(MISC_WIND_OFFSET).unwrap_or(0);
        city.pe_b_weather_wind = weather_cache_byte_from_misc(weather_wind);
        // PE MISC→BSS weather humid @ `0x431516` (`[MISC+0x68]` → `[0x4cb3d4]`).
        let weather_humidity = city.misc_u32_at(MISC_HUMIDITY_OFFSET).unwrap_or(0);
        city.pe_b_weather_humid = weather_cache_byte_from_misc(weather_humidity);
        // PE MISC→BSS trigger disaster @ `0x431527` (`[MISC+0x70]` → word `[0x4ca420]`).
        // Mac CODE 7 owns this byte range as its pending-action selector.
        if kind.is_win95() {
            city.pe_w_trigger_disaster = city
                .misc_u32_at(MISC_TRIGGER_DISASTER_OFFSET)
                .unwrap_or(0)
                .min(u32::from(u16::MAX)) as u16;
        } else {
            city.pe_w_trigger_disaster = citygame_extension
                .as_ref()
                .and_then(|extension| extension.pe_trigger_disaster)
                .unwrap_or(0);
        }
        // PE MISC→BSS residential pop @ `0x431530` (`[MISC+0x74]` → `[0x4ca194]`).
        // Mac owns +0x74 for a different phase-21 value; on that layout CGEX
        // carries the dormant PE engine's previous-month demand input.
        city.pe_dw_residential_population = if kind.is_win95() {
            city.misc_u32_at(MISC_RESIDENTIAL_POPULATION_OFFSET)
                .unwrap_or(0)
        } else {
            citygame_extension
                .as_ref()
                .and_then(|extension| extension.pe_residential_population)
                .unwrap_or(0)
        };
        // PE `0x4e9a24` reads MISC +0x78 as its rewards mask. Mac CODE 4
        // assigns that same slot to its command-strip latch; when Win95
        // fidelity is selected, prepare_pe_fidelity_from_saved_city prefers
        // the separate CityGame extension snapshot on Mac-layout files.
        city.pe_dw_rewards_available = city.misc_u32_at(MISC_REWARDS_AVAILABLE_OFFSET).unwrap_or(0);
        // These viewer snapshots belong to the PE MISC layout. Mac CODE 4
        // zero-fills this tail, so reading it as a saved power percentage
        // would replace the PE cache's no-demand default of 100 with zero.
        if kind.is_win95() {
            city.pe_power_used_percent = city
                .misc_u32_at(MISC_POWER_USED_PERCENT_OFFSET)
                .unwrap_or(100);
            city.pe_dw_bus_passengers = city.misc_u32_at(MISC_BUS_PASSENGERS_OFFSET).unwrap_or(0);
            city.pe_dw_rail_passengers = city.misc_u32_at(MISC_RAIL_PASSENGERS_OFFSET).unwrap_or(0);
            city.pe_dw_subway_passengers =
                city.misc_u32_at(MISC_SUBWAY_PASSENGERS_OFFSET).unwrap_or(0);
        }
        // PE MISC→BSS traffic @ `0x430893` (`[MISC+0x30]` → `[0x4cc6f4]`).
        city.set_pe_traffic_count(city.misc_u32_at(MISC_CITY_TRAFFIC_OFFSET).unwrap_or(0));
        // PE MISC→BSS crime @ `0x43088a` (`[MISC+0x2c]` → `[0x4cb454]`).
        city.pe_dw_city_crime = city.misc_u32_at(MISC_CITY_CRIME_OFFSET).unwrap_or(0);
        // PE MISC→BSS pollution @ `0x43089c` (`[MISC+0x34]` → `[0x4cc910]`).
        city.pe_dw_city_pollution = city.misc_u32_at(MISC_CITY_POLLUTION_OFFSET).unwrap_or(0);
        // PE MISC→BSS pollution rate @ `0x4308c9` (`[MISC+0x48]` → `[0x4cb410]`).
        // Same MISC dword as life expectancy; PE load copies it into the rate word.
        // Mac CODE 4 owns this offset as native event-history state.
        if city.kind.is_win95() {
            // PE 0x431a02 loads the unemployment DWORD from MISC +0xfa4.
            city.pe_unemployment_percent =
                city.misc_u32_at(MISC_CITY_UNEMPLOYMENT_OFFSET).unwrap_or(0);
            city.pe_city_pollution = city
                .misc_u32_at(MISC_CITY_LIFE_EXPECTANCY_OFFSET)
                .unwrap_or(0);
            // PE MISC→BSS crime rate @ `0x4308d2` (`[MISC+0x4c]` → `[0x4cc4b4]`).
            // Same Win95 MISC dword as education quotient; Mac CODE 4 owns this slot too.
            city.pe_city_crime = city
                .misc_u32_at(MISC_CITY_EDUCATION_QUOTIENT_OFFSET)
                .unwrap_or(0);
        }
        // PE post-open/load census rebuild @ `0x434f80` (thunk `0x402ef5`).
        city.pe_rebuild_dw_tile_count_434f80();
        Ok(city)
    }

    pub fn new_blank(path: impl AsRef<Path>, name: impl Into<String>) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        let name = name.into();
        let terrain = vec![0u8; CITY_TILES];
        let buildings = vec![0u8; CITY_TILES];
        let underground = vec![0u8; CITY_TILES];
        let altm = vec![
            AltmSample {
                land: BLANK_MAP_BASE_HEIGHT,
                water: BLANK_MAP_BASE_HEIGHT,
                tunnel_levels: 0,
            };
            CITY_TILES
        ];
        let zones = vec![ZoneFlags::new(0); CITY_TILES];
        let text_overlays = vec![0u8; CITY_TILES];
        let labels = vec![String::new(); CITY_LABEL_COUNT];
        let tile_bits = vec![TileBits::new(0); CITY_TILES];
        let traffic_density = vec![0u8; CITY_TILES];
        let mut city = Self {
            path,
            kind: CityFileKind::WinSc2,
            name: name.clone(),
            chunks: Vec::new(),
            chunk_index: HashMap::new(),
            mac_budget_book: None,
            terrain: Some(terrain),
            buildings: Some(buildings),
            underground: Some(underground),
            altm: Some(altm),
            zones: Some(zones),
            text_overlays: Some(text_overlays),
            labels: Some(labels),
            microsims: Some(vec![MicrosimRecord::default(); MICROSIM_COUNT]),
            tile_bits: Some(tile_bits),
            traffic_density: Some(traffic_density),
            land_value_density: None,
            crime_density: None,
            pollution_density: None,
            police_coverage: None,
            fire_coverage: None,
            population_density: None,
            rate_of_growth: None,
            xtrf_storage: XtrfStorageFormat::Microsim64,
            traffic_overlays: Some(empty_overlay_table()),
            traffic_overlays_from_file: false,
            pe_active_train_count_override: None,
            pe_tornado_actor_counter: 0,
            mac_marina_actor_counter: 0,
            mac_kind3_actor_counter: 0,
            mac_kind1_actor_counter: 0,
            mac_kind2_actor_counter: 0,
            mac_kind5_actor_counter: 0,
            mac_kind16_actor_counter: 0,
            mac_tornado_actor_counter: 0,
            mac_rail_actor_counter: 0,
            mac_developed_cell_snapshot: None,
            mac_transit_accumulators_runtime: [0; 3],
            mac_pending_action_runtime: 0,
            mac_scenario_active_runtime: None,
            mac_scenario_runtime: None,
            mac_rci_counters_runtime: [0; 8],
            mac_population_change_balances: [0; 2],
            pe_cross_layout_active: false,
            pe_cross_layout_saved_neighbor_fame_ranks: None,
            pe_cross_layout_saved_current_disaster_type: None,
            pe_cross_layout_saved_disaster_active: None,
            pe_cross_layout_saved_old_arrests: None,
            pe_cross_layout_saved_prison_bonus: None,
            pe_cross_layout_saved_sewer_bonus: None,
            pe_cross_layout_saved_city_has_ocean: None,
            pe_cross_layout_saved_city_has_river: None,
            pe_cross_layout_saved_xund_occupancy: None,
            pe_cross_layout_saved_city_progression: None,
            pe_cross_layout_saved_rewards_available: None,
            pe_cross_layout_saved_industrial_mix_bonus: None,
            pe_cross_layout_saved_industrial_mix_pollution_bonus: None,
            pe_cross_layout_saved_traffic_count: None,
            pe_cross_layout_saved_monster_xthg_index: None,
            mac_cross_layout_active: false,
            mac_road_neighbor_link_count_runtime: None,
            mac_residential_total_runtime: None,
            mac_cross_layout_saved_civic_latch: None,
            mac_xbld_census_runtime: [0; 256],
            mac_cross_layout_state_restored: false,
            mac_kind3_boundary_target: (0, 0),
            mac_kind2_hotspot_runtime: None,
            pe_sailboat_budget: 0,
            pe_xund_occupancy: 0,
            pe_dw_tile_count: [0; 256],
            pe_nonnetwork_neighbor_links: 0,
            pe_road_link_census_delta: 0,
            pe_synthetic_trip_markers: vec![false; CITY_TILES],
            pe_military_tile_count: [0; MILITARY_TILE_COUNT_VALUES],
            pe_city_value: 0,
            pe_cargo_spawn_tile: (0, 0),
            dispatch_cursors: [0; 3],
            mac_dispatch: Default::default(),
            pe_rci_demand_initialized: false,
            pe_water_coverage_percent: 100,
            pe_power_used_percent: 100,
            pe_dw_bus_passengers: 0,
            pe_dw_rail_passengers: 0,
            pe_dw_subway_passengers: 0,
            mac_monthly_arrest_total: 0,
            mac_mayor_approval: 0,
            mac_newspaper_suppressed: false,
            mac_police_gate_runtime: 0,
            mac_arcology_total_runtime: None,
            mac_power_summary_runtime: 0,
            mac_water_summary_runtime: 0,
            mac_treatment_gate_runtime: None,
            pe_age_cohorts: [0; 20],
            pe_eq_life_points: [0; 20],
            pe_eq_aux_points: [0; 20],
            pe_eq_deaths: 0,
            pe_eq_births: 0,
            pe_dw_city_population: 0,
            pe_dw_arcology_population: 0,
            pe_w_sports_teams: 0,
            pe_w_industrial_mix_bonus: 0,
            pe_w_industrial_mix_pollution_bonus: 0,
            pe_w_old_arrests: 0,
            pe_w_prison_bonus: 0,
            pe_w_sewer_bonus: 0,
            pe_w_monster_xthg_index: 0xffff,
            pe_w_current_disaster_type: 0,
            pe_disaster_damage_class: -1,
            pe_dw_disaster_active: 0,
            pe_dw_city_has_ocean: 0,
            pe_dw_city_has_river: 0,
            pe_dw_national_population: 0,
            pe_dw_national_value: 0,
            pe_w_national_tax: 0,
            pe_w_national_trend: 0,
            pe_eq_growth_slot7: 0,
            pe_zone_pops: [0; 3],
            pe_zone_pops_ready: false,
            pe_eq_leftover_deaths: 0,
            pe_eq_leftover_births: 0,
            pe_eq_life_average: 0,
            pe_eq_education_average: 0,
            pe_eq_workforce_percent: 0,
            pe_eq_averages_ready: false,
            pe_life_expectancy: 70,
            pe_mayor_approval: 0,
            pe_unemployment_percent: 0,
            pe_city_crime: 0,
            pe_city_pollution: 0,
            pe_traffic_count: 0,
            pe_attitude_tax_bases: [0; 3],
            pe_w_city_developed_tiles: 0,
            pe_dw_city_pollution: 0,
            pe_dw_city_land_value: 0,
            pe_i32_city_funds: 0,
            pe_dw_city_age: 0,
            pe_w_city_difficulty: 0,
            pe_w_year_founded: 0,
            pe_dw_city_bonds: 0,
            pe_w_city_mode: 0,
            pe_w_city_rotation: 0,
            pe_w_city_progression: 0,
            pe_w_military_base_type: 0,
            pe_w_city_garbage: 0,
            pe_dw_city_fame: 0,
            pe_dw_city_advertising: 0,
            pe_b_weather_trend: 0,
            pe_b_weather_heat: 0,
            pe_b_weather_wind: 0,
            pe_b_weather_humid: 0,
            pe_w_trigger_disaster: 0,
            pe_w_last_cheat_sound_id: 0,
            pe_pending_effect_sound_ids: Vec::new(),
            pe_pending_auto_goto: Vec::new(),
            pe_dw_residential_population: 0,
            pe_dw_rewards_available: 0,
            pe_dw_city_crime: 0,
            pe_attitude_totals_ready: false,
            pe_attitude_scores: [0; 4],
            pe_w_local_fame_base: 0,
            pe_w_calendar_month_mod: 0,
            pe_w_season: 0,
            pe_w_tech_unlock_mask: 0,
            pe_w_tech_unlock_high_zero_count: 0,
            pe_dw_tech_ui_4e9a10: [0; 18],
            pe_w_neighbor_fame_ranks: [0; PE_NEIGHBOR_FAME_RANK_COUNT],
            pe_newspaper_extra_popup_pending: false,
            pe_newspaper_storage: PeNewspaperStorage::default(),
            pe_disaster_end_newspaper_pending: false,
            misc_crime_count: None,
            misc_traffic_count: None,
            misc_pollution_count: None,
            pe_scenario_deadline_expired: false,
            pe_scenario_active: false,
            misc_humidity: Some(0),
            misc_water_table_level: Some(BLANK_MAP_BASE_HEIGHT as u32),
            saved_date: Some(CityDate {
                year: 1900,
                month: 1,
            }),
            saved_view: None,
            corner_key_bits: DEFAULT_CORNER_KEY_BITS,
            rotation: 0,
            saved_rotation: 0,
            dirty_tiles: HashSet::new(),
            modified: true,
        };
        let mut misc_record = vec![0; WIN95_MISC_RECORD_SIZE];
        misc_record[..4].copy_from_slice(&WIN95_MISC_RECORD_VERSION.to_be_bytes());
        for offset in [
            MISC_AUTO_GOTO_OFFSET,
            MISC_SOUND_EFFECTS_OFFSET,
            MISC_MUSIC_OFFSET,
            MISC_NEWSPAPER_EXTRA_OFFSET,
        ] {
            misc_record[offset..offset + 4].copy_from_slice(&1u32.to_be_bytes());
        }
        upsert_chunk(
            &mut city.chunks,
            &mut city.chunk_index,
            "MISC",
            &misc_record,
        );
        // PE new-city setup writes 64 into both center WORDs at 0x4349ec/0x4349f8.
        city.set_pe_city_center(64, 64);
        city.rebuild_corner_metadata();
        city.mark_all_dirty();
        // PE new-city @ `0x40e2e7`–`0x40e337`: zero `0x4ca4c8`, then `dwTileCount[0] = 0x4000`
        // (full vacant census without a map walk). Also clears `0x4ca41c` (already 0 here).
        city.pe_dw_tile_count[0] = 0x4000;
        Ok(city)
    }
}
