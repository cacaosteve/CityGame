//! FORM/SCDH framing, generic decoded-chunk access, and SC2K RLE codec.
//!
//! Simulation-specific chunk schemas and updates remain in the parent module.

use super::scenario_state::MacScenarioContinuation;
use super::{City, CityChunk, CityFileKind, MISC_FIELD_LEN};
use anyhow::{Result, anyhow, bail};
use std::collections::{HashMap, HashSet};
use std::path::Path;

pub(super) fn upsert_chunk(
    chunks: &mut Vec<CityChunk>,
    chunk_index: &mut HashMap<String, usize>,
    name: &str,
    raw: &[u8],
) {
    let encoded = sc2_rle_encode(raw);
    upsert_encoded_chunk(chunks, chunk_index, name, encoded);
}

pub(super) fn upsert_chunk_for_kind(
    chunks: &mut Vec<CityChunk>,
    chunk_index: &mut HashMap<String, usize>,
    name: &str,
    raw: &[u8],
    kind: CityFileKind,
) {
    let encoded = if kind == CityFileKind::MacCity {
        mac_rle_encode(raw)
    } else {
        sc2_rle_encode(raw)
    };
    upsert_encoded_chunk(chunks, chunk_index, name, encoded);
}

fn upsert_encoded_chunk(
    chunks: &mut Vec<CityChunk>,
    chunk_index: &mut HashMap<String, usize>,
    name: &str,
    encoded: Vec<u8>,
) {
    let existing_index = chunk_index
        .get(name)
        .copied()
        .filter(|index| chunks.get(*index).is_some_and(|chunk| chunk.name == name))
        .or_else(|| chunks.iter().position(|chunk| chunk.name == name));
    if let Some(index) = existing_index {
        let len = encoded.len();
        chunks[index].data = encoded;
        chunks[index].raw_len = len;
        chunk_index.insert(name.to_owned(), index);
    } else {
        let index = chunks.len();
        chunks.push(CityChunk {
            name: name.to_owned(),
            offset: 0,
            raw_len: encoded.len(),
            data: encoded,
        });
        chunk_index.insert(name.to_owned(), index);
    }
}

/// CityGame-owned state that has no safe slot in every native city layout.
/// The original loaders skip unknown chunks, so this does not alias Win95 MISC.
pub(super) const CITYGAME_EXTENSION_CHUNK: &str = "CGEX";
const CITYGAME_EXTENSION_VERSION: u32 = 1;
const CITYGAME_EXTENSION_VERSION_WITH_MAC_STATE: u32 = 2;
const CITYGAME_EXTENSION_VERSION_WITH_PE_STATE: u32 = 3;
const CITYGAME_EXTENSION_VERSION_WITH_PE_TRIGGER: u32 = 4;
const CITYGAME_EXTENSION_VERSION_WITH_PE_NEIGHBOR_RANKS: u32 = 5;
const CITYGAME_EXTENSION_VERSION_WITH_PE_DISASTER_STATE: u32 = 6;
const CITYGAME_EXTENSION_VERSION_WITH_PE_POLICE_STATE: u32 = 7;
const CITYGAME_EXTENSION_VERSION_WITH_PE_SEWER_STATE: u32 = 8;
const CITYGAME_EXTENSION_VERSION_WITH_PE_GEOGRAPHY_STATE: u32 = 9;
const CITYGAME_EXTENSION_VERSION_WITH_PE_XUND_STATE: u32 = 10;
const CITYGAME_EXTENSION_VERSION_WITH_PE_PROGRESSION_STATE: u32 = 11;
const CITYGAME_EXTENSION_VERSION_WITH_CROSS_LAYOUT_CIVIC_STATE: u32 = 12;
const CITYGAME_EXTENSION_VERSION_WITH_PE_INDUSTRY_MIX_STATE: u32 = 13;
const CITYGAME_EXTENSION_VERSION_WITH_PE_TRAFFIC_STATE: u32 = 14;
const CITYGAME_EXTENSION_VERSION_WITH_MAC_SCENARIO_STATE: u32 = 15;
const CITYGAME_EXTENSION_VERSION_WITH_PE_MONSTER_INDEX_STATE: u32 = 16;
const CITYGAME_EXTENSION_VERSION_WITH_MAC_RESIDENTIAL_TOTAL: u32 = 17;
const CITYGAME_EXTENSION_VERSION_WITH_MAC_SCENARIO_CONTINUATION: u32 = 18;
const CITYGAME_EXTENSION_VERSION_WITH_MAC_ROAD_LINKS: u32 = 19;
const MAC_SCENARIO_CONTINUATION_TAIL_LEN: usize = 4 * (1 + 16 + 5 + 1);
const CGEX_MAC_RESIDENTIAL_PRESENT: u32 = 1 << 25;
const CGEX_MAC_GOALS_PRESENT: u32 = 1 << 26;
const CGEX_MAC_SCENARIO_PRESENT: u32 = 1 << 27;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct MacCrossLayoutSavedState {
    pub rci_counters: [u32; 8],
    pub transit_accumulators: [u32; 3],
    pub xbld_census: [u16; 256],
    pub developed_hotspot: Option<(u16, u16)>,
    pub arcology_total: Option<u32>,
    pub police_gate: i16,
    pub power_summary: u32,
    pub water_summary: u32,
    pub treatment_gate: Option<i16>,
    pub developed_cells: Option<u16>,
    pub mac_scenario_active: Option<bool>,
    pub residential_total: Option<u32>,
    pub scenario: Option<MacScenarioContinuation>,
    pub road_neighbor_links: Option<u16>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct CityGameExtension {
    pub version: u32,
    pub mac_pending_action: u32,
    pub mac_saved_state: Option<MacCrossLayoutSavedState>,
    pub pe_residential_population: Option<u32>,
    pub pe_trigger_disaster: Option<u16>,
    pub pe_neighbor_fame_ranks: Option<[u16; 17]>,
    pub pe_current_disaster_type: Option<u16>,
    pub pe_disaster_active: Option<u32>,
    pub pe_old_arrests: Option<u16>,
    pub pe_prison_bonus: Option<u16>,
    pub pe_sewer_bonus: Option<u16>,
    pub pe_city_has_ocean: Option<u32>,
    pub pe_city_has_river: Option<u32>,
    pub pe_xund_occupancy: Option<u16>,
    pub pe_city_progression: Option<u16>,
    pub pe_rewards_available: Option<u32>,
    pub mac_civic_latch: Option<u32>,
    pub pe_industrial_mix_bonus: Option<u16>,
    pub pe_industrial_mix_pollution_bonus: Option<i16>,
    pub pe_traffic_count: Option<u32>,
    pub pe_monster_xthg_index: Option<u16>,
}

pub(super) fn encode_citygame_extension(
    mac_pending_action: u32,
    mac_saved_state: Option<&MacCrossLayoutSavedState>,
    pe_residential_population: Option<u32>,
    pe_trigger_disaster: Option<u16>,
    pe_neighbor_fame_ranks: Option<&[u16; 17]>,
    pe_current_disaster_type: Option<u16>,
    pe_disaster_active: Option<u32>,
    pe_old_arrests: Option<u16>,
    pe_prison_bonus: Option<u16>,
    pe_sewer_bonus: Option<u16>,
    pe_city_has_ocean: Option<u32>,
    pe_city_has_river: Option<u32>,
    pe_xund_occupancy: Option<u16>,
    pe_city_progression: Option<u16>,
    pe_rewards_available: Option<u32>,
    mac_civic_latch: Option<u32>,
    pe_industrial_mix_bonus: Option<u16>,
    pe_industrial_mix_pollution_bonus: Option<i16>,
    pe_traffic_count: Option<u32>,
    pe_monster_xthg_index: Option<u16>,
) -> Vec<u8> {
    if mac_saved_state.is_none()
        && pe_residential_population.is_none()
        && pe_trigger_disaster.is_none()
        && pe_neighbor_fame_ranks.is_none()
        && pe_current_disaster_type.is_none()
        && pe_disaster_active.is_none()
        && pe_old_arrests.is_none()
        && pe_prison_bonus.is_none()
        && pe_sewer_bonus.is_none()
        && pe_city_has_ocean.is_none()
        && pe_city_has_river.is_none()
        && pe_xund_occupancy.is_none()
        && pe_city_progression.is_none()
        && pe_rewards_available.is_none()
        && mac_civic_latch.is_none()
        && pe_industrial_mix_bonus.is_none()
        && pe_industrial_mix_pollution_bonus.is_none()
        && pe_traffic_count.is_none()
        && pe_monster_xthg_index.is_none()
    {
        let mut raw = Vec::with_capacity(8);
        raw.extend_from_slice(&CITYGAME_EXTENSION_VERSION.to_be_bytes());
        raw.extend_from_slice(&mac_pending_action.to_be_bytes());
        return raw;
    }

    // v2 snapshots Mac-native state whose offsets belong to PE fields in
    // Win-layout files. v3 adds PE's prior-residential demand input for
    // Mac-layout files. v4 adds PE's pending disaster trigger, whose MISC
    // slot is Mac's native pending-action selector. v6 extends the v5 payload
    // with PE's active-disaster continuation state; v7 adds the annual police
    // worker's previous-arrest and prison-bonus inputs; v8 adds PE's sewer
    // pollution-blur bonus; v9 adds PE ocean/river disaster geography; v10
    // separates PE XUND occupancy from the Mac count sharing MISC +0xfe8; v11
    // separates PE progression from Mac-owned MISC +0x20; v12 keeps PE's
    // rewards mask and Mac's command-strip latch separate at the colliding
    // MISC +0x78 slot. v13 separately preserves PE industry-mix values whose
    // offsets collide with Mac's demand and pollution-blur state. v14 adds
    // PE traffic count; v15 adds the Mac scenario-active flag while retaining
    // the v14 payload shape. v16 preserves PE's monster XTHG index when the
    // Mac layout owns its native MISC slot. v17 appends Mac's residential
    // total when Win95 owns the native MISC slot. v18 appends Mac scenario
    // goals, action state and any queued scenario point. v19 appends Mac's
    // road-link tally and allows the scenario continuation to be absent.
    let mac_residential_total = mac_saved_state.and_then(|state| state.residential_total);
    let mac_scenario = mac_saved_state.and_then(|state| state.scenario);
    let mac_road_links = mac_saved_state.and_then(|state| state.road_neighbor_links);
    let version = if pe_monster_xthg_index.is_some()
        || mac_residential_total.is_some()
        || mac_scenario.is_some()
        || mac_road_links.is_some()
    {
        CITYGAME_EXTENSION_VERSION_WITH_PE_MONSTER_INDEX_STATE
    } else if mac_saved_state.is_some_and(|state| state.mac_scenario_active.is_some()) {
        CITYGAME_EXTENSION_VERSION_WITH_MAC_SCENARIO_STATE
    } else if pe_traffic_count.is_some() {
        CITYGAME_EXTENSION_VERSION_WITH_PE_TRAFFIC_STATE
    } else if pe_industrial_mix_bonus.is_some() || pe_industrial_mix_pollution_bonus.is_some() {
        CITYGAME_EXTENSION_VERSION_WITH_PE_INDUSTRY_MIX_STATE
    } else if pe_rewards_available.is_some() || mac_civic_latch.is_some() {
        CITYGAME_EXTENSION_VERSION_WITH_CROSS_LAYOUT_CIVIC_STATE
    } else if pe_city_progression.is_some() {
        CITYGAME_EXTENSION_VERSION_WITH_PE_PROGRESSION_STATE
    } else if pe_xund_occupancy.is_some() {
        CITYGAME_EXTENSION_VERSION_WITH_PE_XUND_STATE
    } else if pe_city_has_ocean.is_some() || pe_city_has_river.is_some() {
        CITYGAME_EXTENSION_VERSION_WITH_PE_GEOGRAPHY_STATE
    } else if pe_sewer_bonus.is_some() {
        CITYGAME_EXTENSION_VERSION_WITH_PE_SEWER_STATE
    } else if pe_old_arrests.is_some() || pe_prison_bonus.is_some() {
        CITYGAME_EXTENSION_VERSION_WITH_PE_POLICE_STATE
    } else if pe_current_disaster_type.is_some() || pe_disaster_active.is_some() {
        CITYGAME_EXTENSION_VERSION_WITH_PE_DISASTER_STATE
    } else if pe_neighbor_fame_ranks.is_some() {
        CITYGAME_EXTENSION_VERSION_WITH_PE_NEIGHBOR_RANKS
    } else if pe_trigger_disaster.is_some() {
        CITYGAME_EXTENSION_VERSION_WITH_PE_TRIGGER
    } else if pe_residential_population.is_some() {
        CITYGAME_EXTENSION_VERSION_WITH_PE_STATE
    } else {
        CITYGAME_EXTENSION_VERSION_WITH_MAC_STATE
    };
    let mut raw = Vec::with_capacity(match version {
        CITYGAME_EXTENSION_VERSION_WITH_PE_TRAFFIC_STATE => 688,
        CITYGAME_EXTENSION_VERSION_WITH_MAC_SCENARIO_STATE => 688,
        CITYGAME_EXTENSION_VERSION_WITH_PE_MONSTER_INDEX_STATE => 692,
        CITYGAME_EXTENSION_VERSION_WITH_CROSS_LAYOUT_CIVIC_STATE => 676,
        CITYGAME_EXTENSION_VERSION_WITH_PE_INDUSTRY_MIX_STATE => 684,
        CITYGAME_EXTENSION_VERSION_WITH_PE_PROGRESSION_STATE => 668,
        CITYGAME_EXTENSION_VERSION_WITH_PE_XUND_STATE => 664,
        CITYGAME_EXTENSION_VERSION_WITH_PE_GEOGRAPHY_STATE => 660,
        CITYGAME_EXTENSION_VERSION_WITH_PE_SEWER_STATE => 652,
        CITYGAME_EXTENSION_VERSION_WITH_PE_POLICE_STATE => 648,
        CITYGAME_EXTENSION_VERSION_WITH_PE_DISASTER_STATE => 640,
        CITYGAME_EXTENSION_VERSION_WITH_PE_NEIGHBOR_RANKS => 632,
        CITYGAME_EXTENSION_VERSION_WITH_PE_TRIGGER => 598,
        CITYGAME_EXTENSION_VERSION_WITH_PE_STATE => 594,
        _ => 590,
    });
    let header_version = if mac_road_links.is_some() {
        CITYGAME_EXTENSION_VERSION_WITH_MAC_ROAD_LINKS
    } else if mac_scenario.is_some() {
        CITYGAME_EXTENSION_VERSION_WITH_MAC_SCENARIO_CONTINUATION
    } else if mac_residential_total.is_some() {
        CITYGAME_EXTENSION_VERSION_WITH_MAC_RESIDENTIAL_TOTAL
    } else {
        version
    };
    raw.extend_from_slice(&header_version.to_be_bytes());
    raw.extend_from_slice(&mac_pending_action.to_be_bytes());
    let flags = mac_saved_state.map_or(0, |state| {
        u32::from(state.developed_hotspot.is_some())
            | (u32::from(state.arcology_total.is_some()) << 1)
            | (u32::from(state.treatment_gate.is_some()) << 2)
            | (u32::from(state.developed_cells.is_some()) << 3)
            | (1 << 4)
            | (u32::from(state.mac_scenario_active.is_some()) << 22)
            | (u32::from(state.mac_scenario_active.unwrap_or(false)) << 23)
    }) | (u32::from(pe_residential_population.is_some()) << 5)
        | (u32::from(pe_trigger_disaster.is_some()) << 6)
        | (u32::from(pe_neighbor_fame_ranks.is_some()) << 7)
        | (u32::from(pe_current_disaster_type.is_some()) << 8)
        | (u32::from(pe_disaster_active.is_some()) << 9)
        | (u32::from(pe_old_arrests.is_some()) << 10)
        | (u32::from(pe_prison_bonus.is_some()) << 11)
        | (u32::from(pe_sewer_bonus.is_some()) << 12)
        | (u32::from(pe_city_has_ocean.is_some()) << 13)
        | (u32::from(pe_city_has_river.is_some()) << 14)
        | (u32::from(pe_xund_occupancy.is_some()) << 15)
        | (u32::from(pe_city_progression.is_some()) << 16);
    let flags = flags
        | (u32::from(pe_rewards_available.is_some()) << 17)
        | (u32::from(mac_civic_latch.is_some()) << 18)
        | (u32::from(pe_industrial_mix_bonus.is_some()) << 19)
        | (u32::from(pe_industrial_mix_pollution_bonus.is_some()) << 20)
        | (u32::from(pe_traffic_count.is_some()) << 21)
        | (u32::from(pe_monster_xthg_index.is_some()) << 24);
    let flags = if mac_scenario.is_some() || mac_road_links.is_some() {
        flags
            | if mac_residential_total.is_some() {
                CGEX_MAC_RESIDENTIAL_PRESENT
            } else {
                0
            }
            | if mac_scenario.is_some_and(|scenario| scenario.goals.is_some()) {
                CGEX_MAC_GOALS_PRESENT
            } else {
                0
            }
    } else {
        flags
    };
    let flags = if mac_road_links.is_some() && mac_scenario.is_some() {
        flags | CGEX_MAC_SCENARIO_PRESENT
    } else {
        flags
    };
    raw.extend_from_slice(&flags.to_be_bytes());
    let state = mac_saved_state;
    for value in state.map_or([0; 8], |state| state.rci_counters) {
        raw.extend_from_slice(&value.to_be_bytes());
    }
    for value in state.map_or([0; 3], |state| state.transit_accumulators) {
        raw.extend_from_slice(&value.to_be_bytes());
    }
    for value in state.map_or([0; 256], |state| state.xbld_census) {
        raw.extend_from_slice(&value.to_be_bytes());
    }
    let (hotspot_x, hotspot_y) = state
        .and_then(|state| state.developed_hotspot)
        .unwrap_or_default();
    raw.extend_from_slice(&hotspot_x.to_be_bytes());
    raw.extend_from_slice(&hotspot_y.to_be_bytes());
    raw.extend_from_slice(
        &state
            .and_then(|state| state.arcology_total)
            .unwrap_or_default()
            .to_be_bytes(),
    );
    raw.extend_from_slice(&state.map_or(0, |state| state.police_gate).to_be_bytes());
    raw.extend_from_slice(&state.map_or(0, |state| state.power_summary).to_be_bytes());
    raw.extend_from_slice(&state.map_or(0, |state| state.water_summary).to_be_bytes());
    raw.extend_from_slice(
        &state
            .and_then(|state| state.treatment_gate)
            .unwrap_or_default()
            .to_be_bytes(),
    );
    raw.extend_from_slice(
        &state
            .and_then(|state| state.developed_cells)
            .unwrap_or_default()
            .to_be_bytes(),
    );
    if matches!(
        version,
        CITYGAME_EXTENSION_VERSION_WITH_PE_STATE
            | CITYGAME_EXTENSION_VERSION_WITH_PE_TRIGGER
            | CITYGAME_EXTENSION_VERSION_WITH_PE_NEIGHBOR_RANKS
            | CITYGAME_EXTENSION_VERSION_WITH_PE_DISASTER_STATE
            | CITYGAME_EXTENSION_VERSION_WITH_PE_POLICE_STATE
            | CITYGAME_EXTENSION_VERSION_WITH_PE_SEWER_STATE
            | CITYGAME_EXTENSION_VERSION_WITH_PE_GEOGRAPHY_STATE
            | CITYGAME_EXTENSION_VERSION_WITH_PE_XUND_STATE
            | CITYGAME_EXTENSION_VERSION_WITH_PE_PROGRESSION_STATE
            | CITYGAME_EXTENSION_VERSION_WITH_CROSS_LAYOUT_CIVIC_STATE
            | CITYGAME_EXTENSION_VERSION_WITH_PE_INDUSTRY_MIX_STATE
            | CITYGAME_EXTENSION_VERSION_WITH_PE_TRAFFIC_STATE
            | CITYGAME_EXTENSION_VERSION_WITH_MAC_SCENARIO_STATE
            | CITYGAME_EXTENSION_VERSION_WITH_PE_MONSTER_INDEX_STATE
    ) {
        raw.extend_from_slice(&pe_residential_population.unwrap_or_default().to_be_bytes());
    }
    if matches!(
        version,
        CITYGAME_EXTENSION_VERSION_WITH_PE_TRIGGER
            | CITYGAME_EXTENSION_VERSION_WITH_PE_NEIGHBOR_RANKS
            | CITYGAME_EXTENSION_VERSION_WITH_PE_DISASTER_STATE
            | CITYGAME_EXTENSION_VERSION_WITH_PE_POLICE_STATE
            | CITYGAME_EXTENSION_VERSION_WITH_PE_SEWER_STATE
            | CITYGAME_EXTENSION_VERSION_WITH_PE_GEOGRAPHY_STATE
            | CITYGAME_EXTENSION_VERSION_WITH_PE_XUND_STATE
            | CITYGAME_EXTENSION_VERSION_WITH_PE_PROGRESSION_STATE
            | CITYGAME_EXTENSION_VERSION_WITH_CROSS_LAYOUT_CIVIC_STATE
            | CITYGAME_EXTENSION_VERSION_WITH_PE_INDUSTRY_MIX_STATE
            | CITYGAME_EXTENSION_VERSION_WITH_PE_TRAFFIC_STATE
            | CITYGAME_EXTENSION_VERSION_WITH_MAC_SCENARIO_STATE
            | CITYGAME_EXTENSION_VERSION_WITH_PE_MONSTER_INDEX_STATE
    ) {
        raw.extend_from_slice(&u32::from(pe_trigger_disaster.unwrap_or_default()).to_be_bytes());
    }
    if matches!(
        version,
        CITYGAME_EXTENSION_VERSION_WITH_PE_NEIGHBOR_RANKS
            | CITYGAME_EXTENSION_VERSION_WITH_PE_DISASTER_STATE
            | CITYGAME_EXTENSION_VERSION_WITH_PE_POLICE_STATE
            | CITYGAME_EXTENSION_VERSION_WITH_PE_SEWER_STATE
            | CITYGAME_EXTENSION_VERSION_WITH_PE_GEOGRAPHY_STATE
            | CITYGAME_EXTENSION_VERSION_WITH_PE_XUND_STATE
            | CITYGAME_EXTENSION_VERSION_WITH_PE_PROGRESSION_STATE
            | CITYGAME_EXTENSION_VERSION_WITH_CROSS_LAYOUT_CIVIC_STATE
            | CITYGAME_EXTENSION_VERSION_WITH_PE_INDUSTRY_MIX_STATE
            | CITYGAME_EXTENSION_VERSION_WITH_PE_TRAFFIC_STATE
            | CITYGAME_EXTENSION_VERSION_WITH_MAC_SCENARIO_STATE
            | CITYGAME_EXTENSION_VERSION_WITH_PE_MONSTER_INDEX_STATE
    ) {
        for rank in pe_neighbor_fame_ranks.copied().unwrap_or([0; 17]) {
            raw.extend_from_slice(&rank.to_be_bytes());
        }
    }
    if matches!(
        version,
        CITYGAME_EXTENSION_VERSION_WITH_PE_DISASTER_STATE
            | CITYGAME_EXTENSION_VERSION_WITH_PE_POLICE_STATE
            | CITYGAME_EXTENSION_VERSION_WITH_PE_SEWER_STATE
            | CITYGAME_EXTENSION_VERSION_WITH_PE_GEOGRAPHY_STATE
            | CITYGAME_EXTENSION_VERSION_WITH_PE_XUND_STATE
            | CITYGAME_EXTENSION_VERSION_WITH_PE_PROGRESSION_STATE
            | CITYGAME_EXTENSION_VERSION_WITH_CROSS_LAYOUT_CIVIC_STATE
            | CITYGAME_EXTENSION_VERSION_WITH_PE_INDUSTRY_MIX_STATE
            | CITYGAME_EXTENSION_VERSION_WITH_PE_TRAFFIC_STATE
            | CITYGAME_EXTENSION_VERSION_WITH_MAC_SCENARIO_STATE
            | CITYGAME_EXTENSION_VERSION_WITH_PE_MONSTER_INDEX_STATE
    ) {
        raw.extend_from_slice(
            &u32::from(pe_current_disaster_type.unwrap_or_default()).to_be_bytes(),
        );
        raw.extend_from_slice(&pe_disaster_active.unwrap_or_default().to_be_bytes());
    }
    if matches!(
        version,
        CITYGAME_EXTENSION_VERSION_WITH_PE_POLICE_STATE
            | CITYGAME_EXTENSION_VERSION_WITH_PE_SEWER_STATE
            | CITYGAME_EXTENSION_VERSION_WITH_PE_GEOGRAPHY_STATE
            | CITYGAME_EXTENSION_VERSION_WITH_PE_XUND_STATE
            | CITYGAME_EXTENSION_VERSION_WITH_PE_PROGRESSION_STATE
            | CITYGAME_EXTENSION_VERSION_WITH_CROSS_LAYOUT_CIVIC_STATE
            | CITYGAME_EXTENSION_VERSION_WITH_PE_INDUSTRY_MIX_STATE
            | CITYGAME_EXTENSION_VERSION_WITH_PE_TRAFFIC_STATE
            | CITYGAME_EXTENSION_VERSION_WITH_MAC_SCENARIO_STATE
            | CITYGAME_EXTENSION_VERSION_WITH_PE_MONSTER_INDEX_STATE
    ) {
        raw.extend_from_slice(&u32::from(pe_old_arrests.unwrap_or_default()).to_be_bytes());
        raw.extend_from_slice(&u32::from(pe_prison_bonus.unwrap_or_default()).to_be_bytes());
    }
    if matches!(
        version,
        CITYGAME_EXTENSION_VERSION_WITH_PE_SEWER_STATE
            | CITYGAME_EXTENSION_VERSION_WITH_PE_GEOGRAPHY_STATE
            | CITYGAME_EXTENSION_VERSION_WITH_PE_XUND_STATE
            | CITYGAME_EXTENSION_VERSION_WITH_PE_PROGRESSION_STATE
            | CITYGAME_EXTENSION_VERSION_WITH_CROSS_LAYOUT_CIVIC_STATE
            | CITYGAME_EXTENSION_VERSION_WITH_PE_INDUSTRY_MIX_STATE
            | CITYGAME_EXTENSION_VERSION_WITH_PE_TRAFFIC_STATE
            | CITYGAME_EXTENSION_VERSION_WITH_MAC_SCENARIO_STATE
            | CITYGAME_EXTENSION_VERSION_WITH_PE_MONSTER_INDEX_STATE
    ) {
        raw.extend_from_slice(&u32::from(pe_sewer_bonus.unwrap_or_default()).to_be_bytes());
    }
    if matches!(
        version,
        CITYGAME_EXTENSION_VERSION_WITH_PE_GEOGRAPHY_STATE
            | CITYGAME_EXTENSION_VERSION_WITH_PE_XUND_STATE
            | CITYGAME_EXTENSION_VERSION_WITH_PE_PROGRESSION_STATE
            | CITYGAME_EXTENSION_VERSION_WITH_CROSS_LAYOUT_CIVIC_STATE
            | CITYGAME_EXTENSION_VERSION_WITH_PE_INDUSTRY_MIX_STATE
            | CITYGAME_EXTENSION_VERSION_WITH_PE_TRAFFIC_STATE
            | CITYGAME_EXTENSION_VERSION_WITH_MAC_SCENARIO_STATE
            | CITYGAME_EXTENSION_VERSION_WITH_PE_MONSTER_INDEX_STATE
    ) {
        raw.extend_from_slice(&pe_city_has_ocean.unwrap_or_default().to_be_bytes());
        raw.extend_from_slice(&pe_city_has_river.unwrap_or_default().to_be_bytes());
    }
    if matches!(
        version,
        CITYGAME_EXTENSION_VERSION_WITH_PE_XUND_STATE
            | CITYGAME_EXTENSION_VERSION_WITH_PE_PROGRESSION_STATE
            | CITYGAME_EXTENSION_VERSION_WITH_CROSS_LAYOUT_CIVIC_STATE
            | CITYGAME_EXTENSION_VERSION_WITH_PE_INDUSTRY_MIX_STATE
            | CITYGAME_EXTENSION_VERSION_WITH_PE_TRAFFIC_STATE
            | CITYGAME_EXTENSION_VERSION_WITH_MAC_SCENARIO_STATE
            | CITYGAME_EXTENSION_VERSION_WITH_PE_MONSTER_INDEX_STATE
    ) {
        raw.extend_from_slice(&u32::from(pe_xund_occupancy.unwrap_or_default()).to_be_bytes());
    }
    if matches!(
        version,
        CITYGAME_EXTENSION_VERSION_WITH_PE_PROGRESSION_STATE
            | CITYGAME_EXTENSION_VERSION_WITH_CROSS_LAYOUT_CIVIC_STATE
            | CITYGAME_EXTENSION_VERSION_WITH_PE_INDUSTRY_MIX_STATE
            | CITYGAME_EXTENSION_VERSION_WITH_PE_TRAFFIC_STATE
            | CITYGAME_EXTENSION_VERSION_WITH_MAC_SCENARIO_STATE
            | CITYGAME_EXTENSION_VERSION_WITH_PE_MONSTER_INDEX_STATE
    ) {
        raw.extend_from_slice(&u32::from(pe_city_progression.unwrap_or_default()).to_be_bytes());
    }
    if matches!(
        version,
        CITYGAME_EXTENSION_VERSION_WITH_CROSS_LAYOUT_CIVIC_STATE
            | CITYGAME_EXTENSION_VERSION_WITH_PE_INDUSTRY_MIX_STATE
            | CITYGAME_EXTENSION_VERSION_WITH_PE_TRAFFIC_STATE
            | CITYGAME_EXTENSION_VERSION_WITH_MAC_SCENARIO_STATE
            | CITYGAME_EXTENSION_VERSION_WITH_PE_MONSTER_INDEX_STATE
    ) {
        raw.extend_from_slice(&pe_rewards_available.unwrap_or_default().to_be_bytes());
        raw.extend_from_slice(&mac_civic_latch.unwrap_or_default().to_be_bytes());
    }
    if matches!(
        version,
        CITYGAME_EXTENSION_VERSION_WITH_PE_INDUSTRY_MIX_STATE
            | CITYGAME_EXTENSION_VERSION_WITH_PE_TRAFFIC_STATE
            | CITYGAME_EXTENSION_VERSION_WITH_MAC_SCENARIO_STATE
            | CITYGAME_EXTENSION_VERSION_WITH_PE_MONSTER_INDEX_STATE
    ) {
        raw.extend_from_slice(
            &u32::from(pe_industrial_mix_bonus.unwrap_or_default()).to_be_bytes(),
        );
        raw.extend_from_slice(
            &i32::from(pe_industrial_mix_pollution_bonus.unwrap_or_default()).to_be_bytes(),
        );
    }
    if matches!(
        version,
        CITYGAME_EXTENSION_VERSION_WITH_PE_TRAFFIC_STATE
            | CITYGAME_EXTENSION_VERSION_WITH_MAC_SCENARIO_STATE
            | CITYGAME_EXTENSION_VERSION_WITH_PE_MONSTER_INDEX_STATE
    ) {
        raw.extend_from_slice(&pe_traffic_count.unwrap_or_default().to_be_bytes());
    }
    if version == CITYGAME_EXTENSION_VERSION_WITH_PE_MONSTER_INDEX_STATE {
        raw.extend_from_slice(&u32::from(pe_monster_xthg_index.unwrap_or_default()).to_be_bytes());
    }
    if mac_scenario.is_some() || mac_road_links.is_some() {
        let scenario = mac_scenario.unwrap_or_default();
        raw.extend_from_slice(&mac_residential_total.unwrap_or(0).to_be_bytes());
        for value in scenario
            .goals
            .unwrap_or_default()
            .into_iter()
            .chain(scenario.action)
        {
            raw.extend_from_slice(&value.to_be_bytes());
        }
        let point = scenario
            .pending_point
            .map_or(u32::MAX, |(x, y)| u32::from(u16::from_be_bytes([x, y])));
        raw.extend_from_slice(&point.to_be_bytes());
    } else if let Some(total) = mac_residential_total {
        raw.extend_from_slice(&total.to_be_bytes());
    }

    if let Some(count) = mac_road_links {
        raw.extend_from_slice(&count.to_be_bytes());
    }
    raw
}

pub(super) fn decode_citygame_extension(chunks: &[CityChunk]) -> Option<CityGameExtension> {
    let mut payload = chunks
        .iter()
        .find(|chunk| chunk.name == CITYGAME_EXTENSION_CHUNK)
        .map(|chunk| sc2_rle_decode(&chunk.data).unwrap_or_else(|_| chunk.data.clone()))?;
    let header_version = u32::from_be_bytes(payload.get(0..4)?.try_into().ok()?);
    let mac_road_links = if header_version == CITYGAME_EXTENSION_VERSION_WITH_MAC_ROAD_LINKS {
        let (base, count) = payload.split_last_chunk::<2>()?;
        let count = u16::from_be_bytes(*count);
        payload.truncate(base.len());
        Some(count)
    } else {
        None
    };
    let (version, base_len, mac_residential_total, mac_scenario) = match header_version {
        CITYGAME_EXTENSION_VERSION_WITH_MAC_SCENARIO_CONTINUATION
        | CITYGAME_EXTENSION_VERSION_WITH_MAC_ROAD_LINKS => {
            let base_len = payload
                .len()
                .checked_sub(MAC_SCENARIO_CONTINUATION_TAIL_LEN)?;
            let flags = u32::from_be_bytes(payload.get(8..12)?.try_into().ok()?);
            if flags & (1 << 4) == 0 {
                return None;
            }
            let mut cursor = base_len;
            let residential_total = read_u32_be(&payload, &mut cursor)?;
            let mut goals = [0; 16];
            let mut action = [0; 5];
            for value in goals.iter_mut().chain(&mut action) {
                *value = read_u32_be(&payload, &mut cursor)?;
            }
            let point = read_u32_be(&payload, &mut cursor)?;
            let pending_point = if point == u32::MAX {
                None
            } else {
                let [x, y] = u16::try_from(point).ok()?.to_be_bytes();
                Some((x, y))
            };
            (
                CITYGAME_EXTENSION_VERSION_WITH_PE_MONSTER_INDEX_STATE,
                base_len,
                (flags & CGEX_MAC_RESIDENTIAL_PRESENT != 0).then_some(residential_total),
                (mac_road_links.is_none() || flags & CGEX_MAC_SCENARIO_PRESENT != 0).then_some(
                    MacScenarioContinuation {
                        goals: (flags & CGEX_MAC_GOALS_PRESENT != 0).then_some(goals),
                        action,
                        pending_point,
                    },
                ),
            )
        }
        CITYGAME_EXTENSION_VERSION_WITH_MAC_RESIDENTIAL_TOTAL => {
            // v17 extends the complete v16 payload with the Mac-only total.
            let base_len = payload.len().checked_sub(4)?;
            let total = u32::from_be_bytes(payload.get(base_len..)?.try_into().ok()?);
            (
                CITYGAME_EXTENSION_VERSION_WITH_PE_MONSTER_INDEX_STATE,
                base_len,
                Some(total),
                None,
            )
        }
        _ => (header_version, payload.len(), None, None),
    };
    let mac_pending_action = u32::from_be_bytes(payload.get(4..8)?.try_into().ok()?);
    match (version, base_len) {
        (CITYGAME_EXTENSION_VERSION, 8) => Some(CityGameExtension {
            version,
            mac_pending_action,
            mac_saved_state: None,
            pe_residential_population: None,
            pe_trigger_disaster: None,
            pe_neighbor_fame_ranks: None,
            pe_current_disaster_type: None,
            pe_disaster_active: None,
            pe_old_arrests: None,
            pe_prison_bonus: None,
            pe_sewer_bonus: None,
            pe_city_has_ocean: None,
            pe_city_has_river: None,
            pe_xund_occupancy: None,
            pe_city_progression: None,
            pe_rewards_available: None,
            mac_civic_latch: None,
            pe_industrial_mix_bonus: None,
            pe_industrial_mix_pollution_bonus: None,
            pe_traffic_count: None,
            pe_monster_xthg_index: None,
        }),
        (CITYGAME_EXTENSION_VERSION_WITH_MAC_STATE, 590)
        | (CITYGAME_EXTENSION_VERSION_WITH_PE_STATE, 594)
        | (CITYGAME_EXTENSION_VERSION_WITH_PE_TRIGGER, 598)
        | (CITYGAME_EXTENSION_VERSION_WITH_PE_NEIGHBOR_RANKS, 632)
        | (CITYGAME_EXTENSION_VERSION_WITH_PE_DISASTER_STATE, 640)
        | (CITYGAME_EXTENSION_VERSION_WITH_PE_POLICE_STATE, 648)
        | (CITYGAME_EXTENSION_VERSION_WITH_PE_SEWER_STATE, 652)
        | (CITYGAME_EXTENSION_VERSION_WITH_PE_GEOGRAPHY_STATE, 660)
        | (CITYGAME_EXTENSION_VERSION_WITH_PE_XUND_STATE, 664)
        | (CITYGAME_EXTENSION_VERSION_WITH_PE_PROGRESSION_STATE, 668)
        | (CITYGAME_EXTENSION_VERSION_WITH_CROSS_LAYOUT_CIVIC_STATE, 676)
        | (CITYGAME_EXTENSION_VERSION_WITH_PE_INDUSTRY_MIX_STATE, 684)
        | (CITYGAME_EXTENSION_VERSION_WITH_PE_TRAFFIC_STATE, 688)
        | (CITYGAME_EXTENSION_VERSION_WITH_MAC_SCENARIO_STATE, 688)
        | (CITYGAME_EXTENSION_VERSION_WITH_PE_MONSTER_INDEX_STATE, 692) => {
            let flags = u32::from_be_bytes(payload.get(8..12)?.try_into().ok()?);
            let mut cursor = 12;
            let mut rci_counters = [0; 8];
            for value in &mut rci_counters {
                *value = read_u32_be(&payload, &mut cursor)?;
            }
            let mut transit_accumulators = [0; 3];
            for value in &mut transit_accumulators {
                *value = read_u32_be(&payload, &mut cursor)?;
            }
            let mut xbld_census = [0; 256];
            for value in &mut xbld_census {
                *value = read_u16_be(&payload, &mut cursor)?;
            }
            let hotspot = (
                read_u16_be(&payload, &mut cursor)?,
                read_u16_be(&payload, &mut cursor)?,
            );
            let arcology_total = read_u32_be(&payload, &mut cursor)?;
            let police_gate = read_i16_be(&payload, &mut cursor)?;
            let power_summary = read_u32_be(&payload, &mut cursor)?;
            let water_summary = read_u32_be(&payload, &mut cursor)?;
            let treatment_gate = read_i16_be(&payload, &mut cursor)?;
            let developed_cells = read_u16_be(&payload, &mut cursor)?;
            let pe_residential_population = if matches!(
                version,
                CITYGAME_EXTENSION_VERSION_WITH_PE_STATE
                    | CITYGAME_EXTENSION_VERSION_WITH_PE_TRIGGER
                    | CITYGAME_EXTENSION_VERSION_WITH_PE_NEIGHBOR_RANKS
                    | CITYGAME_EXTENSION_VERSION_WITH_PE_DISASTER_STATE
                    | CITYGAME_EXTENSION_VERSION_WITH_PE_POLICE_STATE
                    | CITYGAME_EXTENSION_VERSION_WITH_PE_SEWER_STATE
                    | CITYGAME_EXTENSION_VERSION_WITH_PE_GEOGRAPHY_STATE
                    | CITYGAME_EXTENSION_VERSION_WITH_PE_XUND_STATE
                    | CITYGAME_EXTENSION_VERSION_WITH_PE_PROGRESSION_STATE
                    | CITYGAME_EXTENSION_VERSION_WITH_CROSS_LAYOUT_CIVIC_STATE
                    | CITYGAME_EXTENSION_VERSION_WITH_PE_INDUSTRY_MIX_STATE
                    | CITYGAME_EXTENSION_VERSION_WITH_PE_TRAFFIC_STATE
                    | CITYGAME_EXTENSION_VERSION_WITH_MAC_SCENARIO_STATE
                    | CITYGAME_EXTENSION_VERSION_WITH_PE_MONSTER_INDEX_STATE
            ) {
                let value = read_u32_be(&payload, &mut cursor)?;
                (flags & (1 << 5) != 0).then_some(value)
            } else {
                None
            };
            let pe_trigger_disaster = if matches!(
                version,
                CITYGAME_EXTENSION_VERSION_WITH_PE_TRIGGER
                    | CITYGAME_EXTENSION_VERSION_WITH_PE_NEIGHBOR_RANKS
                    | CITYGAME_EXTENSION_VERSION_WITH_PE_DISASTER_STATE
                    | CITYGAME_EXTENSION_VERSION_WITH_PE_POLICE_STATE
                    | CITYGAME_EXTENSION_VERSION_WITH_PE_SEWER_STATE
                    | CITYGAME_EXTENSION_VERSION_WITH_PE_GEOGRAPHY_STATE
                    | CITYGAME_EXTENSION_VERSION_WITH_PE_XUND_STATE
                    | CITYGAME_EXTENSION_VERSION_WITH_PE_PROGRESSION_STATE
                    | CITYGAME_EXTENSION_VERSION_WITH_CROSS_LAYOUT_CIVIC_STATE
                    | CITYGAME_EXTENSION_VERSION_WITH_PE_INDUSTRY_MIX_STATE
                    | CITYGAME_EXTENSION_VERSION_WITH_PE_TRAFFIC_STATE
                    | CITYGAME_EXTENSION_VERSION_WITH_MAC_SCENARIO_STATE
                    | CITYGAME_EXTENSION_VERSION_WITH_PE_MONSTER_INDEX_STATE
            ) {
                let value = read_u32_be(&payload, &mut cursor)?.min(u32::from(u16::MAX)) as u16;
                (flags & (1 << 6) != 0).then_some(value)
            } else {
                None
            };
            let pe_neighbor_fame_ranks = if matches!(
                version,
                CITYGAME_EXTENSION_VERSION_WITH_PE_NEIGHBOR_RANKS
                    | CITYGAME_EXTENSION_VERSION_WITH_PE_DISASTER_STATE
                    | CITYGAME_EXTENSION_VERSION_WITH_PE_POLICE_STATE
                    | CITYGAME_EXTENSION_VERSION_WITH_PE_SEWER_STATE
                    | CITYGAME_EXTENSION_VERSION_WITH_PE_GEOGRAPHY_STATE
                    | CITYGAME_EXTENSION_VERSION_WITH_PE_XUND_STATE
                    | CITYGAME_EXTENSION_VERSION_WITH_PE_PROGRESSION_STATE
                    | CITYGAME_EXTENSION_VERSION_WITH_CROSS_LAYOUT_CIVIC_STATE
                    | CITYGAME_EXTENSION_VERSION_WITH_PE_INDUSTRY_MIX_STATE
                    | CITYGAME_EXTENSION_VERSION_WITH_PE_TRAFFIC_STATE
                    | CITYGAME_EXTENSION_VERSION_WITH_MAC_SCENARIO_STATE
                    | CITYGAME_EXTENSION_VERSION_WITH_PE_MONSTER_INDEX_STATE
            ) {
                let mut ranks = [0; 17];
                for rank in &mut ranks {
                    *rank = read_u16_be(&payload, &mut cursor)?;
                }
                (flags & (1 << 7) != 0).then_some(ranks)
            } else {
                None
            };
            let pe_current_disaster_type = if matches!(
                version,
                CITYGAME_EXTENSION_VERSION_WITH_PE_DISASTER_STATE
                    | CITYGAME_EXTENSION_VERSION_WITH_PE_POLICE_STATE
                    | CITYGAME_EXTENSION_VERSION_WITH_PE_SEWER_STATE
                    | CITYGAME_EXTENSION_VERSION_WITH_PE_GEOGRAPHY_STATE
                    | CITYGAME_EXTENSION_VERSION_WITH_PE_XUND_STATE
                    | CITYGAME_EXTENSION_VERSION_WITH_PE_PROGRESSION_STATE
                    | CITYGAME_EXTENSION_VERSION_WITH_CROSS_LAYOUT_CIVIC_STATE
                    | CITYGAME_EXTENSION_VERSION_WITH_PE_INDUSTRY_MIX_STATE
                    | CITYGAME_EXTENSION_VERSION_WITH_PE_TRAFFIC_STATE
                    | CITYGAME_EXTENSION_VERSION_WITH_MAC_SCENARIO_STATE
                    | CITYGAME_EXTENSION_VERSION_WITH_PE_MONSTER_INDEX_STATE
            ) {
                let value = read_u32_be(&payload, &mut cursor)?.min(u32::from(u16::MAX)) as u16;
                (flags & (1 << 8) != 0).then_some(value)
            } else {
                None
            };
            let pe_disaster_active = if matches!(
                version,
                CITYGAME_EXTENSION_VERSION_WITH_PE_DISASTER_STATE
                    | CITYGAME_EXTENSION_VERSION_WITH_PE_POLICE_STATE
                    | CITYGAME_EXTENSION_VERSION_WITH_PE_SEWER_STATE
                    | CITYGAME_EXTENSION_VERSION_WITH_PE_GEOGRAPHY_STATE
                    | CITYGAME_EXTENSION_VERSION_WITH_PE_XUND_STATE
                    | CITYGAME_EXTENSION_VERSION_WITH_PE_PROGRESSION_STATE
                    | CITYGAME_EXTENSION_VERSION_WITH_CROSS_LAYOUT_CIVIC_STATE
                    | CITYGAME_EXTENSION_VERSION_WITH_PE_INDUSTRY_MIX_STATE
                    | CITYGAME_EXTENSION_VERSION_WITH_PE_TRAFFIC_STATE
                    | CITYGAME_EXTENSION_VERSION_WITH_MAC_SCENARIO_STATE
                    | CITYGAME_EXTENSION_VERSION_WITH_PE_MONSTER_INDEX_STATE
            ) {
                let value = read_u32_be(&payload, &mut cursor)?;
                (flags & (1 << 9) != 0).then_some(value)
            } else {
                None
            };
            let pe_old_arrests = if matches!(
                version,
                CITYGAME_EXTENSION_VERSION_WITH_PE_POLICE_STATE
                    | CITYGAME_EXTENSION_VERSION_WITH_PE_SEWER_STATE
                    | CITYGAME_EXTENSION_VERSION_WITH_PE_GEOGRAPHY_STATE
                    | CITYGAME_EXTENSION_VERSION_WITH_PE_XUND_STATE
                    | CITYGAME_EXTENSION_VERSION_WITH_PE_PROGRESSION_STATE
                    | CITYGAME_EXTENSION_VERSION_WITH_CROSS_LAYOUT_CIVIC_STATE
                    | CITYGAME_EXTENSION_VERSION_WITH_PE_INDUSTRY_MIX_STATE
                    | CITYGAME_EXTENSION_VERSION_WITH_PE_TRAFFIC_STATE
                    | CITYGAME_EXTENSION_VERSION_WITH_MAC_SCENARIO_STATE
                    | CITYGAME_EXTENSION_VERSION_WITH_PE_MONSTER_INDEX_STATE
            ) {
                let value = read_u32_be(&payload, &mut cursor)?.min(u32::from(u16::MAX)) as u16;
                (flags & (1 << 10) != 0).then_some(value)
            } else {
                None
            };
            let pe_prison_bonus = if matches!(
                version,
                CITYGAME_EXTENSION_VERSION_WITH_PE_POLICE_STATE
                    | CITYGAME_EXTENSION_VERSION_WITH_PE_SEWER_STATE
                    | CITYGAME_EXTENSION_VERSION_WITH_PE_GEOGRAPHY_STATE
                    | CITYGAME_EXTENSION_VERSION_WITH_PE_XUND_STATE
                    | CITYGAME_EXTENSION_VERSION_WITH_PE_PROGRESSION_STATE
                    | CITYGAME_EXTENSION_VERSION_WITH_CROSS_LAYOUT_CIVIC_STATE
                    | CITYGAME_EXTENSION_VERSION_WITH_PE_INDUSTRY_MIX_STATE
                    | CITYGAME_EXTENSION_VERSION_WITH_PE_TRAFFIC_STATE
                    | CITYGAME_EXTENSION_VERSION_WITH_MAC_SCENARIO_STATE
                    | CITYGAME_EXTENSION_VERSION_WITH_PE_MONSTER_INDEX_STATE
            ) {
                let value = read_u32_be(&payload, &mut cursor)?.min(u32::from(u16::MAX)) as u16;
                (flags & (1 << 11) != 0).then_some(value)
            } else {
                None
            };
            let pe_sewer_bonus = if matches!(
                version,
                CITYGAME_EXTENSION_VERSION_WITH_PE_SEWER_STATE
                    | CITYGAME_EXTENSION_VERSION_WITH_PE_GEOGRAPHY_STATE
                    | CITYGAME_EXTENSION_VERSION_WITH_PE_XUND_STATE
                    | CITYGAME_EXTENSION_VERSION_WITH_PE_PROGRESSION_STATE
                    | CITYGAME_EXTENSION_VERSION_WITH_CROSS_LAYOUT_CIVIC_STATE
                    | CITYGAME_EXTENSION_VERSION_WITH_PE_INDUSTRY_MIX_STATE
                    | CITYGAME_EXTENSION_VERSION_WITH_PE_TRAFFIC_STATE
                    | CITYGAME_EXTENSION_VERSION_WITH_MAC_SCENARIO_STATE
                    | CITYGAME_EXTENSION_VERSION_WITH_PE_MONSTER_INDEX_STATE
            ) {
                let value = read_u32_be(&payload, &mut cursor)?.min(u32::from(u16::MAX)) as u16;
                (flags & (1 << 12) != 0).then_some(value)
            } else {
                None
            };
            let pe_city_has_ocean = if matches!(
                version,
                CITYGAME_EXTENSION_VERSION_WITH_PE_GEOGRAPHY_STATE
                    | CITYGAME_EXTENSION_VERSION_WITH_PE_XUND_STATE
                    | CITYGAME_EXTENSION_VERSION_WITH_PE_PROGRESSION_STATE
                    | CITYGAME_EXTENSION_VERSION_WITH_CROSS_LAYOUT_CIVIC_STATE
                    | CITYGAME_EXTENSION_VERSION_WITH_PE_INDUSTRY_MIX_STATE
                    | CITYGAME_EXTENSION_VERSION_WITH_PE_TRAFFIC_STATE
                    | CITYGAME_EXTENSION_VERSION_WITH_MAC_SCENARIO_STATE
                    | CITYGAME_EXTENSION_VERSION_WITH_PE_MONSTER_INDEX_STATE
            ) {
                let value = read_u32_be(&payload, &mut cursor)?;
                (flags & (1 << 13) != 0).then_some(value)
            } else {
                None
            };
            let pe_city_has_river = if matches!(
                version,
                CITYGAME_EXTENSION_VERSION_WITH_PE_GEOGRAPHY_STATE
                    | CITYGAME_EXTENSION_VERSION_WITH_PE_XUND_STATE
                    | CITYGAME_EXTENSION_VERSION_WITH_PE_PROGRESSION_STATE
                    | CITYGAME_EXTENSION_VERSION_WITH_CROSS_LAYOUT_CIVIC_STATE
                    | CITYGAME_EXTENSION_VERSION_WITH_PE_INDUSTRY_MIX_STATE
                    | CITYGAME_EXTENSION_VERSION_WITH_PE_TRAFFIC_STATE
                    | CITYGAME_EXTENSION_VERSION_WITH_MAC_SCENARIO_STATE
                    | CITYGAME_EXTENSION_VERSION_WITH_PE_MONSTER_INDEX_STATE
            ) {
                let value = read_u32_be(&payload, &mut cursor)?;
                (flags & (1 << 14) != 0).then_some(value)
            } else {
                None
            };
            let pe_xund_occupancy = if matches!(
                version,
                CITYGAME_EXTENSION_VERSION_WITH_PE_XUND_STATE
                    | CITYGAME_EXTENSION_VERSION_WITH_PE_PROGRESSION_STATE
                    | CITYGAME_EXTENSION_VERSION_WITH_CROSS_LAYOUT_CIVIC_STATE
                    | CITYGAME_EXTENSION_VERSION_WITH_PE_INDUSTRY_MIX_STATE
                    | CITYGAME_EXTENSION_VERSION_WITH_PE_TRAFFIC_STATE
                    | CITYGAME_EXTENSION_VERSION_WITH_MAC_SCENARIO_STATE
                    | CITYGAME_EXTENSION_VERSION_WITH_PE_MONSTER_INDEX_STATE
            ) {
                let value = read_u32_be(&payload, &mut cursor)?.min(u32::from(u16::MAX)) as u16;
                (flags & (1 << 15) != 0).then_some(value)
            } else {
                None
            };
            let pe_city_progression = if matches!(
                version,
                CITYGAME_EXTENSION_VERSION_WITH_PE_PROGRESSION_STATE
                    | CITYGAME_EXTENSION_VERSION_WITH_CROSS_LAYOUT_CIVIC_STATE
                    | CITYGAME_EXTENSION_VERSION_WITH_PE_INDUSTRY_MIX_STATE
                    | CITYGAME_EXTENSION_VERSION_WITH_PE_TRAFFIC_STATE
                    | CITYGAME_EXTENSION_VERSION_WITH_MAC_SCENARIO_STATE
                    | CITYGAME_EXTENSION_VERSION_WITH_PE_MONSTER_INDEX_STATE
            ) {
                let value = read_u32_be(&payload, &mut cursor)?.min(u32::from(u16::MAX)) as u16;
                (flags & (1 << 16) != 0).then_some(value)
            } else {
                None
            };
            let pe_rewards_available = if matches!(
                version,
                CITYGAME_EXTENSION_VERSION_WITH_CROSS_LAYOUT_CIVIC_STATE
                    | CITYGAME_EXTENSION_VERSION_WITH_PE_INDUSTRY_MIX_STATE
                    | CITYGAME_EXTENSION_VERSION_WITH_PE_TRAFFIC_STATE
                    | CITYGAME_EXTENSION_VERSION_WITH_MAC_SCENARIO_STATE
                    | CITYGAME_EXTENSION_VERSION_WITH_PE_MONSTER_INDEX_STATE
            ) {
                let value = read_u32_be(&payload, &mut cursor)?;
                (flags & (1 << 17) != 0).then_some(value)
            } else {
                None
            };
            let mac_civic_latch = if matches!(
                version,
                CITYGAME_EXTENSION_VERSION_WITH_CROSS_LAYOUT_CIVIC_STATE
                    | CITYGAME_EXTENSION_VERSION_WITH_PE_INDUSTRY_MIX_STATE
                    | CITYGAME_EXTENSION_VERSION_WITH_PE_TRAFFIC_STATE
                    | CITYGAME_EXTENSION_VERSION_WITH_MAC_SCENARIO_STATE
                    | CITYGAME_EXTENSION_VERSION_WITH_PE_MONSTER_INDEX_STATE
            ) {
                let value = read_u32_be(&payload, &mut cursor)?;
                (flags & (1 << 18) != 0).then_some(value)
            } else {
                None
            };
            let pe_industrial_mix_bonus = if matches!(
                version,
                CITYGAME_EXTENSION_VERSION_WITH_PE_INDUSTRY_MIX_STATE
                    | CITYGAME_EXTENSION_VERSION_WITH_PE_TRAFFIC_STATE
                    | CITYGAME_EXTENSION_VERSION_WITH_MAC_SCENARIO_STATE
                    | CITYGAME_EXTENSION_VERSION_WITH_PE_MONSTER_INDEX_STATE
            ) {
                let value = read_u32_be(&payload, &mut cursor)?.min(u32::from(u16::MAX)) as u16;
                (flags & (1 << 19) != 0).then_some(value)
            } else {
                None
            };
            let pe_industrial_mix_pollution_bonus = if matches!(
                version,
                CITYGAME_EXTENSION_VERSION_WITH_PE_INDUSTRY_MIX_STATE
                    | CITYGAME_EXTENSION_VERSION_WITH_PE_TRAFFIC_STATE
                    | CITYGAME_EXTENSION_VERSION_WITH_MAC_SCENARIO_STATE
                    | CITYGAME_EXTENSION_VERSION_WITH_PE_MONSTER_INDEX_STATE
            ) {
                let value = read_u32_be(&payload, &mut cursor)? as i32 as i16;
                (flags & (1 << 20) != 0).then_some(value)
            } else {
                None
            };
            let pe_traffic_count = if matches!(
                version,
                CITYGAME_EXTENSION_VERSION_WITH_PE_TRAFFIC_STATE
                    | CITYGAME_EXTENSION_VERSION_WITH_MAC_SCENARIO_STATE
                    | CITYGAME_EXTENSION_VERSION_WITH_PE_MONSTER_INDEX_STATE
            ) {
                let value = read_u32_be(&payload, &mut cursor)?;
                (flags & (1 << 21) != 0).then_some(value)
            } else {
                None
            };
            let pe_monster_xthg_index =
                if version == CITYGAME_EXTENSION_VERSION_WITH_PE_MONSTER_INDEX_STATE {
                    let value = read_u32_be(&payload, &mut cursor)?.min(u32::from(u16::MAX)) as u16;
                    (flags & (1 << 24) != 0).then_some(value)
                } else {
                    None
                };
            let has_mac_state =
                version == CITYGAME_EXTENSION_VERSION_WITH_MAC_STATE || flags & (1 << 4) != 0;
            Some(CityGameExtension {
                version: header_version,
                mac_pending_action,
                mac_saved_state: has_mac_state.then(|| MacCrossLayoutSavedState {
                    rci_counters,
                    transit_accumulators,
                    xbld_census,
                    developed_hotspot: (flags & 1 != 0).then_some(hotspot),
                    arcology_total: (flags & 2 != 0).then_some(arcology_total),
                    police_gate,
                    power_summary,
                    water_summary,
                    treatment_gate: (flags & 4 != 0).then_some(treatment_gate),
                    developed_cells: (flags & 8 != 0).then_some(developed_cells),
                    mac_scenario_active: (flags & (1 << 22) != 0).then_some(flags & (1 << 23) != 0),
                    residential_total: mac_residential_total,
                    scenario: mac_scenario,
                    road_neighbor_links: mac_road_links,
                }),
                pe_residential_population,
                pe_trigger_disaster,
                pe_neighbor_fame_ranks,
                pe_current_disaster_type,
                pe_disaster_active,
                pe_old_arrests,
                pe_prison_bonus,
                pe_sewer_bonus,
                pe_city_has_ocean,
                pe_city_has_river,
                pe_xund_occupancy,
                pe_city_progression,
                pe_rewards_available,
                mac_civic_latch,
                pe_industrial_mix_bonus,
                pe_industrial_mix_pollution_bonus,
                pe_traffic_count,
                pe_monster_xthg_index,
            })
        }
        _ => None,
    }
}

fn read_u16_be(payload: &[u8], cursor: &mut usize) -> Option<u16> {
    let value = u16::from_be_bytes(payload.get(*cursor..*cursor + 2)?.try_into().ok()?);
    *cursor += 2;
    Some(value)
}

fn read_i16_be(payload: &[u8], cursor: &mut usize) -> Option<i16> {
    read_u16_be(&payload, cursor).map(|value| value as i16)
}

fn read_u32_be(payload: &[u8], cursor: &mut usize) -> Option<u32> {
    let value = u32::from_be_bytes(payload.get(*cursor..*cursor + 4)?.try_into().ok()?);
    *cursor += 4;
    Some(value)
}

pub fn detect_city_kind(path: &Path, bytes: &[u8]) -> Result<CityFileKind> {
    if bytes.len() < 12 || &bytes[0..4] != b"FORM" || &bytes[8..12] != b"SCDH" {
        bail!(
            "{} is not a SimCity 2000™ FORM/SCDH city file",
            path.display()
        );
    }

    let ext = path
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();

    Ok(match ext.as_str() {
        "scn" => CityFileKind::WinScenario,
        "sc2" => CityFileKind::WinSc2,
        _ => CityFileKind::MacCity,
    })
}

pub fn parse_scdh_chunks(bytes: &[u8]) -> Result<Vec<CityChunk>> {
    if bytes.len() < 12 || &bytes[0..4] != b"FORM" || &bytes[8..12] != b"SCDH" {
        bail!("missing FORM/SCDH header");
    }

    let mut chunks = Vec::new();
    let mut offset = 12;

    while offset + 8 <= bytes.len() {
        let name = clean_chunk_name(&bytes[offset..offset + 4]);
        let size = u32::from_be_bytes(bytes[offset + 4..offset + 8].try_into().unwrap()) as usize;
        let data_start = offset + 8;
        let data_end = data_start
            .checked_add(size)
            .ok_or_else(|| anyhow!("chunk {name} overflows file size"))?;
        if data_end > bytes.len() {
            bail!("chunk {name} at {offset} runs past end of file");
        }

        chunks.push(CityChunk {
            name,
            offset,
            raw_len: size,
            data: bytes[data_start..data_end].to_vec(),
        });

        let unpadded = data_end;
        let padded = data_end + 1;
        if valid_chunk_header(bytes, unpadded) {
            offset = unpadded;
        } else if valid_chunk_header(bytes, padded) {
            offset = padded;
        } else {
            break;
        }
    }

    Ok(chunks)
}

pub fn sc2_rle_decode(input: &[u8]) -> Result<Vec<u8>> {
    let mut output = Vec::new();
    let mut index = 0;

    while index < input.len() {
        let marker = input[index];
        index += 1;

        match marker {
            1..=127 => {
                let count = marker as usize;
                if index + count > input.len() {
                    bail!("literal RLE run exceeds input");
                }
                output.extend_from_slice(&input[index..index + count]);
                index += count;
            }
            129..=255 => {
                if index >= input.len() {
                    bail!("repeat RLE run missing byte");
                }
                let count = marker as usize - 127;
                let value = input[index];
                index += 1;
                output.extend(std::iter::repeat_n(value, count));
            }
            0 | 128 => bail!("invalid SC2K RLE marker {marker}"),
        }
    }

    Ok(output)
}

/// Decode the Mac 68k chunk codec. Its literal controls store count minus one,
/// unlike the Win95 decoder's direct literal count. Repeat controls match the
/// Win95 repeat length.
pub fn mac_rle_decode(input: &[u8]) -> Result<Vec<u8>> {
    let mut output = Vec::new();
    let mut index = 0;

    while index < input.len() {
        let control = input[index];
        index += 1;
        if control & 0x80 == 0 {
            let count = usize::from(control) + 1;
            let end = index
                .checked_add(count)
                .filter(|end| *end <= input.len())
                .ok_or_else(|| anyhow!("Mac literal RLE run exceeds input"))?;
            output.extend_from_slice(&input[index..end]);
            index = end;
        } else {
            let count = usize::from(control & 0x7f) + 1;
            let value = *input
                .get(index)
                .ok_or_else(|| anyhow!("Mac repeat RLE run missing byte"))?;
            index += 1;
            output.extend(std::iter::repeat_n(value, count));
        }
    }

    Ok(output)
}

/// Encode the Mac 68k count-minus-one RLE format. Repeated runs are emitted
/// only when they save space; all other bytes use literal packets of 1..=128.
pub fn mac_rle_encode(input: &[u8]) -> Vec<u8> {
    let mut encoded = Vec::with_capacity(input.len());
    let mut index = 0;
    while index < input.len() {
        let mut repeat_len = 1;
        while input[index] < 0x80
            && index + repeat_len < input.len()
            && input[index + repeat_len] == input[index]
            && repeat_len < 128
        {
            repeat_len += 1;
        }
        if repeat_len >= 2 {
            encoded.push(0x80 | ((repeat_len - 1) as u8));
            encoded.push(input[index]);
            index += repeat_len;
            continue;
        }

        let literal_start = index;
        index += repeat_len;
        while index < input.len() && index - literal_start < 128 {
            let mut next_repeat = 1;
            while input[index] < 0x80
                && index + next_repeat < input.len()
                && input[index + next_repeat] == input[index]
                && next_repeat < 128
            {
                next_repeat += 1;
            }
            if next_repeat >= 2 {
                break;
            }
            let remaining = 128 - (index - literal_start);
            let take = next_repeat.min(remaining);
            index += take;
            if take < next_repeat {
                break;
            }
        }
        let literal_len = index - literal_start;
        encoded.push((literal_len - 1) as u8);
        encoded.extend_from_slice(&input[literal_start..index]);
    }
    encoded
}

pub(super) fn city_rle_decode(input: &[u8], kind: CityFileKind) -> Result<Vec<u8>> {
    if kind == CityFileKind::MacCity {
        mac_rle_decode(input)
    } else {
        sc2_rle_decode(input)
    }
}

fn clean_chunk_name(raw: &[u8]) -> String {
    raw.iter()
        .copied()
        .filter(|byte| *byte != 0)
        .map(char::from)
        .collect()
}

fn valid_chunk_header(bytes: &[u8], offset: usize) -> bool {
    if offset == bytes.len() {
        return false;
    }
    if offset + 8 > bytes.len() {
        return false;
    }
    let name = &bytes[offset..offset + 4];
    if !name
        .iter()
        .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit() || *byte == 0)
    {
        return false;
    }
    let size = u32::from_be_bytes(bytes[offset + 4..offset + 8].try_into().unwrap()) as usize;
    offset + 8 + size <= bytes.len()
}

/// Win95 save order from `FUN_004323ac`, corroborated against bundled stock cities.
pub const SAVE_CHUNK_ORDER: &[&str] = &[
    "MISC", "ALTM", "XTER", "XBLD", "XZON", "XUND", "XTXT", "XLAB", "XMIC", "XTHG", "XBIT", "XTRF",
    "XPLT", "XVAL", "XCRM", "XPLC", "XFIR", "XPOP", "XROG", "XGRP", "CNAM",
];

/// Chunk tag order as written in a `FORM SCDH` file (for save-path tests).
pub fn chunk_tags_in_file(bytes: &[u8]) -> Result<Vec<String>> {
    let chunks = parse_scdh_chunks(bytes)?;
    Ok(chunks.into_iter().map(|c| c.name).collect())
}

pub fn encode_scdh_file(chunks: &[CityChunk]) -> Result<Vec<u8>> {
    let mut body = Vec::new();
    let mut written = HashSet::new();
    for name in SAVE_CHUNK_ORDER {
        if let Some(chunk) = chunks.iter().find(|chunk| chunk.name == *name) {
            write_chunk(&mut body, chunk);
            written.insert(chunk.name.as_str());
        }
    }
    for chunk in chunks {
        if written.insert(chunk.name.as_str()) {
            write_chunk(&mut body, chunk);
        }
    }

    let form_size = 4 + body.len();
    let mut file = Vec::with_capacity(8 + form_size);
    file.extend_from_slice(b"FORM");
    file.extend_from_slice(&(form_size as u32).to_be_bytes());
    file.extend_from_slice(b"SCDH");
    file.extend_from_slice(&body);
    Ok(file)
}

fn write_chunk(out: &mut Vec<u8>, chunk: &CityChunk) {
    let mut name = [0u8; 4];
    for (index, byte) in chunk.name.bytes().take(4).enumerate() {
        name[index] = byte;
    }
    out.extend_from_slice(&name);
    out.extend_from_slice(&(chunk.data.len() as u32).to_be_bytes());
    out.extend_from_slice(&chunk.data);
    if chunk.data.len() % 2 == 1 {
        out.push(0);
    }
}

pub fn sc2_rle_encode(input: &[u8]) -> Vec<u8> {
    let mut encoded = Vec::new();
    let mut index = 0;
    while index < input.len() {
        if index + 2 < input.len()
            && input[index] == input[index + 1]
            && input[index + 1] == input[index + 2]
        {
            let value = input[index];
            let mut run = 1usize;
            while index + run < input.len() && input[index + run] == value && run < 128 {
                run += 1;
            }
            encoded.push((127 + run) as u8);
            encoded.push(value);
            index += run;
            continue;
        }

        let literal_start = index;
        let mut literal_len = 0usize;
        while index < input.len() && literal_len < 127 {
            if index + 2 < input.len()
                && input[index] == input[index + 1]
                && input[index + 1] == input[index + 2]
            {
                break;
            }
            literal_len += 1;
            index += 1;
        }
        encoded.push(literal_len as u8);
        encoded.extend_from_slice(&input[literal_start..literal_start + literal_len]);
    }

    if encoded.len() < input.len() {
        encoded
    } else {
        input.to_vec()
    }
}

impl City {
    /// Read a contiguous sequence of big-endian unsigned values from any decoded chunk.
    pub fn chunk_u32_values(&self, name: &str, offset: usize, count: usize) -> Vec<u32> {
        self.chunks
            .iter()
            .find(|chunk| chunk.name == name)
            .map(|chunk| sc2_rle_decode(&chunk.data).unwrap_or_else(|_| chunk.data.clone()))
            .and_then(|data| {
                let byte_len = count.checked_mul(MISC_FIELD_LEN)?;
                let end = offset.checked_add(byte_len)?;
                data.get(offset..end).map(|bytes| {
                    bytes
                        .chunks_exact(MISC_FIELD_LEN)
                        .map(|chunk| u32::from_be_bytes(chunk.try_into().expect("four bytes")))
                        .collect()
                })
            })
            .unwrap_or_default()
    }

    /// Write a contiguous sequence of big-endian unsigned values into any decoded chunk.
    pub fn set_chunk_u32_values(&mut self, name: &str, offset: usize, values: &[u32]) {
        let Some(byte_len) = values.len().checked_mul(MISC_FIELD_LEN) else {
            return;
        };
        let Some(end) = offset.checked_add(byte_len) else {
            return;
        };

        let mut raw = self
            .chunks
            .iter()
            .find(|chunk| chunk.name == name)
            .map(|chunk| sc2_rle_decode(&chunk.data).unwrap_or_else(|_| chunk.data.clone()))
            .unwrap_or_default();
        raw.resize(raw.len().max(end), 0);
        for (index, value) in values.iter().copied().enumerate() {
            let field_offset = offset + index * MISC_FIELD_LEN;
            raw[field_offset..field_offset + MISC_FIELD_LEN].copy_from_slice(&value.to_be_bytes());
        }
        upsert_chunk(&mut self.chunks, &mut self.chunk_index, name, &raw);
        self.modified = true;
    }
}
