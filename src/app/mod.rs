#[cfg(test)]
use crate::city::QUERY_SPECIFIC_MAYOR_STATS_OVERLAY;
use crate::city::pe_choose_and_place_corner_bits;
use crate::city::{
    AppMode, CITY_HEIGHT, CITY_TILES, CITY_WIDTH, City, CityDate, CityFileKind, MICROSIM_COUNT,
    MICROSIM_STRUCTURE_FIRST, MICROSIM_SYSTEM_BUS, MICROSIM_SYSTEM_HYDRO, MICROSIM_SYSTEM_WIND,
    TileBits, XBIT_CHOOSE_AND_PLACE, ZONE_CORNER_ALL,
};
#[cfg(test)]
use crate::city::{BLANK_MAP_BASE_HEIGHT, Technology};
#[cfg(test)]
use crate::city::{
    MAX_SIM_TEXT_ENTRIES, MAX_USER_TEXT_ENTRIES, MIN_SIM_TEXT_ENTRIES, MIN_USER_TEXT_ENTRIES,
    ZONE_CORNER_TRIGHT,
};
use crate::data_map::CityDataMapMode;
use crate::engine_fidelity::EngineFidelity;
use crate::game_clock::{DAYS_PER_MONTH, GameClock, GameClockAdvance};
use crate::mac_menu;
use crate::mac_random::{MacAppLfsrRng, MacGameLcgRng, MacToolboxRng};
use crate::win95_random::{Win95CrtRand, Win95GameRand, Win95Lfsr};

use crate::menu_model::MenuCheckState;
use crate::menubar::MenubarUi;
use crate::palette::{ANIMATION_FPS, MAC_PALETTE_ANIMATION_FPS};
#[cfg(test)]
use crate::palette::{is_smoke_sprite, is_water_sprite, should_animate_sprite};
#[cfg(test)]
use crate::paths;
use crate::renderer::{RenderSprite, Renderer, UiImage, UiSolid};
use crate::tiles::rotated_building_id;
use crate::traffic::{
    GameSpeed, ROAD_SPRITE_H, ROAD_SPRITE_W, TRAFFIC_ROAD_OVERLAY_SPRITE_OFFSET, TrafficSystem,
    count_traffic_overlays, has_decorative_traffic, mac_export80_ordinary_road_building,
    mac_export80_special_stamp_building, mac_export80_stamp_building, resolve_traffic_sprite,
    road_tile_overlay_flip_x, traffic_anim_frame_for_tile, traffic_dash_screen_scale,
    traffic_render_sprite_id, win95_traffic_overlay_at,
};
use crate::trip_generator::{self, game_is_valid_transit_items};
#[cfg(test)]
use crate::ui::UiAction;
use crate::ui::{self, EditTool};
#[cfg(test)]
use crate::win95_overlay::DispatchKind;
use crate::win95_overlay::{
    OVERLAY_KIND_DISPATCH_FIRE, OVERLAY_KIND_DISPATCH_MILITARY, OVERLAY_KIND_DISPATCH_POLICE,
    OVERLAY_KIND_TRAFFIC, PeMaxisManSpawnInputs, Win95OverlayRecord, XTHG_AIRPLANE,
    XTHG_CARGO_SHIP, XTHG_EXPLOSION, XTHG_FIRST_USABLE_SLOT, XTHG_HELICOPTER, XTHG_MAXIS_MAN,
    XTHG_MONSTER, XTHG_RECORD_COUNT, XTHG_SAILBOAT, XTHG_SUBWAY_TRAIN_ENGINE, XTHG_TORNADO,
    XTHG_TRAIN_CAR, XTHG_TRAIN_ENGINE, pe_maxis_man_saved_payload, spawn_pe_maxis_man,
    xthg_direction_delta, xthg_proxy_for_slot, xthg_slot_for_proxy,
};
#[cfg(test)]
use crate::win95_overlay::{XTHG_DIRECTION_EAST, XTHG_DIRECTION_NORTH};
use crate::win95_overlay_sim::{Win95OverlaySim, ambient_spawn_direction};
use crate::win95_tile_draw::uses_win95_overlay_draw;
use glam::Vec2;
#[cfg(test)]
use sdl3::keyboard::Keycode;
use sdl3::keyboard::Mod;
#[cfg(test)]
use sdl3::mouse::MouseWheelDirection;
use std::collections::{HashSet, VecDeque};
#[cfg(test)]
use std::fs;
use std::time::Duration;

mod budget;
mod budget_accounting;
mod budget_calendar;
mod budget_controls;
mod budget_window;
mod calendar;
mod calendar_dispatch;
mod calendar_publication;
mod city_attitude;
mod city_options;
mod city_publication;
mod city_valuation;
mod command_bus;
mod coverage;
mod cursors;
mod demolition;
mod density;
mod dialog_content;
mod dialog_input;
mod dialog_layout;
mod dialog_render;
mod dialogs;
mod disaster_actors;
mod disaster_model;
mod disaster_progression;
mod disaster_start;
mod disasters;
mod effect_sound;
mod graph_history;
mod growth_actors;
mod growth_bookkeeping;
mod growth_dispatch;
mod industry;
mod mac_loop_scheduler;
mod mac_terrain_editing;
mod map_interaction;
mod map_rendering;
mod military_base;
mod network;
mod newspaper;
mod newspaper_events;
mod newspaper_gifts;
mod newspaper_state;
mod newspaper_stories;
mod ordinances;
mod panels;
mod pe_idle_scheduler;
mod power_water;
mod power_water_refresh;
mod rci;

/// Evaluate the isolated Mac demand-candidate model for native-oracle
/// comparisons. This is not connected to live monthly simulation.
pub(super) fn mac_rci_candidate_probe(
    pair_totals: [i32; 3],
    previous_residential_total: i32,
    current_scalar: i32,
    prior_industry_modifier: i16,
    difficulty: usize,
    xbld_counts: &[u16; 256],
    cap_modifier_1: u16,
    cap_modifier_2: u16,
) -> [u32; 3] {
    rci::mac_rci_demand_candidates(rci::MacRciCandidateInputs {
        pair_totals,
        previous_residential_total,
        current_scalar,
        prior_industry_modifier,
        difficulty,
        xbld_counts,
        cap_modifier_1,
        cap_modifier_2,
    })
    .map(f32::to_bits)
}

/// Finalize the isolated Mac demand candidates with explicit tax and previous
/// demand inputs, for comparison with CODE 8's published WORDs.
pub(super) fn mac_rci_candidate_finalizer_probe(
    pair_totals: [i32; 3],
    previous_residential_total: i32,
    current_scalar: i32,
    prior_industry_modifier: i16,
    difficulty: usize,
    xbld_counts: &[u16; 256],
    cap_modifier_1: u16,
    cap_modifier_2: u16,
    tax_adjustments: [i16; 3],
    previous_demands: [i16; 3],
) -> anyhow::Result<[i16; 3]> {
    let candidates = rci::mac_rci_demand_candidates(rci::MacRciCandidateInputs {
        pair_totals,
        previous_residential_total,
        current_scalar,
        prior_industry_modifier,
        difficulty,
        xbld_counts,
        cap_modifier_1,
        cap_modifier_2,
    });
    let mut finalized = [0; 3];
    for index in 0..3 {
        let normalized =
            rci::mac_rci_normalize_demand_candidate(candidates[index], pair_totals[index] as u32);
        let pre_clamp_word = rci::mac_rci_demand_pre_clamp_word(
            normalized,
            tax_adjustments[index],
            previous_demands[index],
        );
        anyhow::ensure!(
            i16::MIN as i32 <= pre_clamp_word && pre_clamp_word <= i16::MAX as i32,
            "cannot claim native FX2I result for out-of-range value {pre_clamp_word} on channel {index}; Mac SANE overflow behavior is unresolved"
        );
        finalized[index] = pre_clamp_word.clamp(-2000, 2000) as i16;
    }
    Ok(finalized)
}
mod runtime;
mod scenario;
mod scenario_lifecycle;
mod session;
mod special_building_growth;
mod special_zone_growth;
mod startup_assets;
mod status_display;
mod terrain_generation;
mod terrain_water_level;
#[cfg(test)]
mod tests;
mod tile_classification;
mod title_flow;
mod ui_format;

mod building_placement;
mod inspection;
mod nature_tools;
mod terrain_editing;
mod title_screen;
mod tool_controller;
mod traffic_rendering;
mod transport_maintenance;
mod underground;
mod underground_model;
mod view_navigation;
mod weather;
mod window_support;
mod zoning_signs;

pub use runtime::run;

use zoning_signs::{ZoneDrag, is_user_sign, zone_id_for_tool, zone_placement_sound};
#[cfg(test)]
use zoning_signs::{tile_rectangle, zone_build_cost};

use budget::*;
use building_placement::{
    building_footprint_from_origin, building_zone_bits_for_tile, footprint_origin,
    pe_place_tile_size_440c50, pe_place_tile_size_440c50_ex, pe_place_tile_size_click_to_nw,
    placeable_building_for_tool, reward_mask_for_tool,
};
#[cfg(test)]
use building_placement::{
    pe_citytool_nimby_residential_count_440648, pe_citytool_post_place_water_recalc_44094e,
    pe_hydro_building_id_441610,
};
use calendar::*;
use city_attitude::*;
#[cfg(test)]
use city_options::{
    AUTO_GOTO_MISC_OFFSET, MISC_NEWSPAPER_CHOICE_OFFSET, MISC_NEWSPAPER_EXTRA_OFFSET,
    MISC_NEWSPAPER_SUBSCRIPTION_OFFSET, MUSIC_MISC_OFFSET, NO_DISASTERS_MISC_OFFSET,
    SOUND_EFFECTS_MISC_OFFSET, auto_goto_enabled_for_city, ensure_default_city_options,
    ensure_newspaper_list_misc_defaults_before_extension, music_enabled_for_city,
    set_city_auto_goto_enabled, set_city_music_enabled, set_city_sound_effects_enabled,
    sound_effects_enabled_for_city,
};
use city_options::{
    CITY_OPTION_DISABLED_VALUE, CITY_OPTION_ENABLED_VALUE, auto_goto_enabled_for_engine,
    city_difficulty_for_city, ensure_default_city_options_for_engine, music_enabled_for_engine,
    newspaper_choice_for_city, newspaper_extra_enabled_for_city, newspaper_max_choice_for_city,
    newspaper_subscription_enabled_for_city, no_disasters_enabled_for_city,
    no_disasters_enabled_for_engine, set_auto_goto_enabled_for_engine, set_city_newspaper_choice,
    set_city_newspaper_extra_enabled, set_city_newspaper_subscription_enabled,
    set_city_no_disasters_enabled, set_music_enabled_for_engine,
    set_no_disasters_enabled_for_engine, set_sound_effects_enabled_for_engine,
    sound_effects_enabled_for_engine,
};
#[cfg(test)]
use city_publication::pe_year_end_flag;
#[cfg(test)]
use city_publication::{building_tile_counts_for_city, military_tile_counts_for_city};
use city_publication::{
    pe_zone_pops_scaled_population, persist_pe_garbage_day22, persist_pe_year_end_misc,
};
use city_valuation::{
    mac_bond_city_value_ratio_3a714, mac_bond_issue_city_value_allows_3a714,
    mac_bond_issue_city_value_would_allow_3a714, mac_bond_issue_rate_scaled_3a714,
    mac_city_value_total_3a714, pe_bond_city_value_ratio_score_would_41d9a7,
    pe_bond_credit_rating_caption_4e6a78, pe_bond_issue_city_value_allows_41a679,
    pe_bond_issue_city_value_would_allow_41a679, sc2kfix_pe_recalculate_city_value,
};
#[cfg(test)]
use city_valuation::{
    mac_bond_city_value_ratio_allows_issue_3a714, pe_bond_city_value_ratio_score_41d9a7,
    pe_bond_city_value_ratio_score_from, pe_city_value_building_contrib_46a34a,
    pe_city_value_infra_subtotal_46a270, pe_recalculate_city_value_46a270,
    sc2kfix_pe_city_value_total,
};
use command_bus::{
    AppCommand, app_command_for_menu_command, command_queue, menu_callback, pop_command,
    push_command, windows_menu_shortcut,
};

pub(crate) extern "C" fn document_open_callback(path: *const std::ffi::c_char) {
    if path.is_null() {
        return;
    }
    let path = unsafe { std::ffi::CStr::from_ptr(path) }
        .to_string_lossy()
        .into_owned();
    if !path.is_empty() {
        push_command(AppCommand::OpenTitlePath(path.into()));
    }
}

#[cfg(target_os = "ios")]
pub(crate) fn queue_document_open(path: std::path::PathBuf, from_title: bool) {
    push_command(if from_title {
        AppCommand::OpenTitlePath(path)
    } else {
        AppCommand::OpenPath(path)
    });
}

#[cfg(target_os = "ios")]
pub(crate) fn queue_document_saved(path: std::path::PathBuf, previous_path: std::path::PathBuf) {
    push_command(AppCommand::DocumentSaved {
        path,
        previous_path,
    });
}

use coverage::*;
use cursors::{GameCursor, GameCursors};
use demolition::{
    pe_clear_tunnel_portal_surface, pe_clear_xtxt_sign_label, pe_destroy_building_footprint,
    pe_destroy_building_mode0_optional_crt, pe_destroy_building_mode1_optional_crt,
    pe_destroy_building_size, pe_destroy_structure_fire_paint,
    pe_flood_destroy_tile_mode_maybe_crt, pe_flood_destroy_tile_with_crt,
};
#[cfg(test)]
use demolition::{
    pe_destroy_building_mode1_scatter_draw_count_for_hit,
    pe_destroy_building_selector1_support_walk_458e0f,
    pe_destroy_building_selector2_high_class_walk_plan,
    pe_destroy_building_selector2_low_class_cleanup_order,
    pe_destroy_building_size1_nonflat_fun_00447020_45a1ee,
    pe_destroy_building_size2_snap_hit_45b1c0, pe_flood_destroy_tile,
    pe_maptool_funds_debit_44338a, pe_maptool_refuse_xzon_type7_4432d2,
    pe_maptool_surface_clear_subway_xund_44344b, pe_maptool_surface_destroy_building_443461,
    pe_maptool_surface_placetile_zero_4434d2, pe_maptool_surface_preclear_subway_443461,
    pe_maptool_surface_refuse_xbld5_4432fd, pe_maptool_surface_vacant_refuse_44331c,
    pe_maptool_surface_xter_gate_4434de, pe_maptool_underground_clear_subway_443427,
    pe_maptool_underground_empty_refuse_443350,
};
use density::*;
use dialogs::{
    ARCOLOGY_CHOICES, ArcologyDialogState, CityInfoDialog, MilitaryBaseDialogState,
    NewspaperEdition, PowerPlantDialogState, SignDialogState, arcology_choice_for_tool,
    arcology_dialog_hit_test, city_info_dialog_rect_for, handle_modal_pointer_down,
    handle_modal_pointer_up, military_base_dialog_hit_test, power_plant_choice_for_tool,
    power_plant_dialog_hit_test, sync_sign_text_input,
};
#[cfg(test)]
use dialogs::{
    ArcologyDialogAction, CityInfoDialogAction, POWER_PLANT_CHOICES, PowerPlantDialogAction,
    arcology_cancel_rect, arcology_card_rect, arcology_dialog_rect, arcology_info_rect,
    budget_auto_budget_rect, budget_bond_button_rect, budget_service_funding_adjust_rect,
    budget_tax_adjust_rect, city_info_close_rect, city_info_dialog_hit_test, city_info_dialog_rect,
    city_map_cell_color, city_map_cell_rect_for_tile, city_map_panel_rect_and_cell,
    city_map_tile_for_point, graph_legend_rect, graph_panel_xgrp_series, graph_scale_rect,
    industry_tax_adjust_rect, ordinance_row_rect, point_in_arcology_dialog,
    point_in_power_plant_dialog, population_panel_demographic_rows,
    population_panel_demographic_rows_with_power,
    population_panel_demographic_rows_with_power_and_fidelity, population_panel_transit_rows,
    power_plant_cancel_rect, power_plant_card_rect, power_plant_dialog_rect, power_plant_info_rect,
    query_dialog_row_groups, road_bridge_choice_rect,
};
use disaster_actors::*;
use disaster_model::*;
use disaster_progression::persist_pe_disaster_misc;
#[cfg(test)]
use disaster_progression::{
    clear_pe_maxis_man_actors, pe_disaster_overlay_seen, pe_disaster_scan_sounds_with_rng,
};
use disaster_start::*;
use disasters::*;
use effect_sound::{PlacementSound, disaster_start_sound, play_placement_sound};
#[cfg(test)]
use graph_history::XGRP_SAMPLES_PER_SERIES;
use graph_history::{
    GraphScale, XGRP_CHUNK_NAME, XGRP_FIVE_YEAR_OFFSET, XGRP_FIVE_YEAR_SAMPLES,
    XGRP_HALF_YEAR_OFFSET, XGRP_HALF_YEAR_SAMPLES, XGRP_MONTH_OFFSET, XGRP_MONTH_SAMPLES,
    XGRP_VALUES, pe_xgrp_rolls_five_year, pe_xgrp_rolls_half_year, roll_xgrp_history_section,
    xgrp_history_latest_values, xgrp_samples_oldest_first,
};
#[cfg(test)]
use growth_actors::{ambient_airplane_takeoff_direction, pe_budget_transport_xbit_powered};
use growth_bookkeeping::TransitPassengerStats;
use industry::*;
use inspection::{
    microsim_query_extra, query_altitude_feet, query_metric_band, query_service_yes_no,
    query_tile_label, surface_building_query_label, transit_query_extra,
};
#[cfg(test)]
use inspection::{
    microsim_query_letter_grade, query_tile_status_line, query_tile_status_line_with_passengers,
};
use map_interaction::apply_map_touch_action;
use map_rendering::{
    DEPTH_BUILDING, DEPTH_HIGHWAY, DEPTH_ROAD, DEPTH_ZONE, SpriteCache, TileSpriteCtx,
    building_sprite_id, push_sprite, push_sprite_to_atlas_with_anchor,
    push_underground_tile_sprite, sprite_depth, xbld_sprite_info,
};
#[cfg(test)]
use map_rendering::{
    DEPTH_POWER, DEPTH_RAIL, DEPTH_TERRAIN, DISASTER_FLOOD_SPRITE_ID, DISASTER_RIOTERS_1_SPRITE_ID,
    DISASTER_RIOTERS_2_SPRITE_ID, DISASTER_TOXIC_CLOUD_SPRITE_ID, TerrainSprites, XbldAnchor,
    disaster_sprite_id_for_text_overlay, terrain_sprites, tile_sprite_index,
    underground_surface_sprite_info, underground_terrain_sprite_id, underground_tile_sprite_id,
    underground_tile_tint, xbld_base_layer_sprite_id,
};
#[cfg(test)]
use military_base::{
    MILITARY_BASE_FALLBACK_PATCH_COUNT, MILITARY_BASE_MIN_VALID_TILES, MILITARY_BASE_PLOT_SIZE,
    MILITARY_MAX_SILOS, MILITARY_NAVAL_ZONE_LENGTH, NavalCoastStrip, evaluate_military_base_plot,
    pe_clear_small_xbld_for_military_zone, pe_military_base_fallback_plot_is_candidate,
    pe_military_base_plot_type_from_counts, sample_military_base_plot,
    select_pe_military_base_fallback_plots, stamp_pe_military_base_fallback_plot,
};
use military_base::{MILITARY_SILO_PLOT_SIZE, pe_stamp_military_zone_tile_4144d4};
#[cfg(test)]
use nature_tools::is_edit_map_tree_terrain;
use nature_tools::{
    NatureDrag, is_tree_building, pe_fun_00447500, pe_fun_00447500_new_city_tree_count,
    pe_place_water_helper_446650, pe_place_water_reslope_neighbors_446a66,
    pe_terrain_generation_water_walk_446440,
};
#[cfg(test)]
use nature_tools::{pe_forest_shift_xbld, pe_fun_00447500_tree_place_tile, pe_tree_place_xbld};
#[cfg(test)]
use network::network_placement_sound;
#[cfg(test)]
use network::persist_pe_neighbor_connection_overlays;
use network::{
    HIGHWAY_COST_PER_SEGMENT, HIGHWAY_TILES_PER_SEGMENT, HighwayDrag, HighwayNeighborLinkCandidate,
    HighwayWaterBridgeCandidate, NETWORK_BOTTOM, NETWORK_LEFT, NETWORK_RIGHT, NETWORK_TOP,
    NetworkAxis, NetworkDrag, NetworkKind, PE_NETWORK_SURFACE_REMAP_GATE_4E7B68,
    RailNeighborLinkCandidate, RoadNeighborLinkCandidate, building_network_access_kinds,
    directional_neighbor_tiles, highway_neighbor_connection_tiles_for_city, highway_segment_tiles,
    neighbor_connection_tiles_for_city, network_axis_mask_for_side_mask,
    network_building_connects_to, network_building_for_mask, network_drag_path,
    network_kind_for_tool, network_kinds_for_building, network_path_axis_mask, network_side_axis,
    network_single_axis, opposite_network_side, pe_highway_classify_463c40,
    pe_highway_water_selector_cost_45c1e0, pe_highway_water_selector_name_45c1e0,
    pe_highway_water_selectors_45c1e0, pe_network_gate_connects_side,
    pe_network_surface_edit_448e20, pe_network_surface_stroke_cost_448a90,
    pe_road_bridge_selector_cost, pe_road_bridge_selector_name, pe_road_bridge_selectors,
    persist_pe_neighbor_connection_overlay, power_line_drag_path,
    rail_neighbor_connection_tiles_for_city, road_neighbor_connection_tiles_for_city,
};
#[cfg(test)]
use network::{
    PE_NETWORK_SURFACE_CLEAR_COST, PE_POWER_BRIDGE_SELECTOR_MASK, PeHighwayRouteStep461ae0,
    apply_highway_neighbor_link_confirmation, apply_rail_neighbor_link_confirmation,
    apply_road_neighbor_link_confirmation, highway_axis_for_drag_plan_tile,
    highway_axis_for_path_tile, highway_building_for_axis, highway_building_for_mask,
    highway_drag_path, highway_neighbor_link_is_available, mac_onramp_compatible_road_mask,
    network_building_for_tile, network_commit_path_for_kind, network_drag_path_for_kind,
    network_preview_path_for_kind, onramp_building_for_sides,
    pe_city_tool_place_highway_stamp_body_4626d0, pe_get_highway_tile_462910,
    pe_get_highway_tile_classify_refuse_462956, pe_highway_gate_early_8000_4640a1,
    pe_highway_gate_returns_4000_4642e7, pe_highway_gate_si0_fallthrough_464310,
    pe_highway_neighbor_low_bit_462990, pe_highway_route_gate_low_word_464040,
    pe_highway_route_path_461ae0, pe_highway_route_step_461ae0,
    pe_highway_water_bridge_candidate_45c1e0, pe_highway_water_footprint_class_45c1e0,
    pe_network_surface_terrain_result_448e20, pe_network_xtxt_fa_edge_mask_bits_44b0ac,
    pe_onramp_compatible_road_mask, pe_place_highway_atcoordinates_funds_debit_44c267,
    pe_place_highway_tile_462ed0_class_4_to_7, pe_place_highway_tile_462ed0_class_8_to_c,
    pe_place_highway_tile_462ed0_class_ge_0d, pe_place_highway_tile_462ed0_class_lt4,
    pe_place_highway_tile_463b30, pe_place_network_crossover_from_existing,
    pe_place_power_crossover_from_existing_44a9e8, pe_place_power_mask_tile_44ac2c,
    pe_place_power_reshape_neighbor_connects_44ab30, pe_place_power_xtxt_reshape_44aafb,
    pe_place_rail_mask_tile_44b6a0, pe_place_rail_reshape_neighbor_connects_44b485,
    pe_place_rail_slope_from_cardinal_neighbor_44b467, pe_place_rail_xtxt_reshape_44b41d,
    pe_place_rails_crossover_from_existing_44b2f4, pe_place_road_mask_tile_44b0e5,
    pe_place_road_reshape_neighbor_connects_44af6e, pe_place_road_xtxt_reshape_44af50,
    pe_place_roads_crossover_from_existing_44ae24, pe_power_bridge_selector_cost_45b930,
    pe_rail_bridge_interior_tile, pe_rail_bridge_selector_cost, pe_rail_bridge_span_plan,
    pe_rail_next_stroke_tile_449f50, pe_road_bridge_direction_mask, pe_road_bridge_interior_tile,
    pe_road_bridge_selector_mask, pe_road_bridge_span_plan,
    pe_road_candidate_xbld_allows_step_448f50, pe_road_next_stroke_tile_448f50,
    pe_road_stroke_plan_449bb0, pe_road_stroke_walk_init_448bc0, power_bridge_span_plan,
    rail_neighbor_link_is_available, road_neighbor_link_is_available,
};
#[cfg(test)]
use newspaper_events::citygame_fa_non_network_neighbor_marker_count;
use newspaper_events::{
    pe_rci_demand_updates_attitude_newspaper_46881f, pe_rci_demand_updates_eq_newspaper_468905,
    pe_rci_demand_updates_le_newspaper_468960,
    pe_rci_demand_updates_neighbor_fame_newspaper_4689c8,
    pe_rci_demand_updates_sports_newspaper_4687f3,
    pe_rci_demand_updates_unemployment_newspaper_4688a8,
};
#[cfg(test)]
use newspaper_gifts::*;
use newspaper_state::*;
use newspaper_stories::*;
use ordinances::*;
use power_water::*;
use rci::*;
use scenario::*;
use scenario_lifecycle::{
    reward_available_status_line,
};
#[cfg(test)]
use session::{
    AppCommandStateEffect, apply_new_city_title_selections, game_speed_from_sc2k_misc,
    initialize_mac_fidelity_new_city_industry_rows, initialize_original_new_city_national_misc,
    traffic_system_for_city,
};
use session::{
    open_city_dialog, open_scenario_dialog, open_title_city_dialog, open_title_scenario_dialog,
    pe_city_mode_misc_value, persist_pe_city_mode_misc, persist_speed_misc_value_to_city,
    save_city_dialog, sc2k_speed_misc_value, set_window_title, speed_misc_value_for_city,
};
use special_building_growth::*;
use special_zone_growth::*;
#[cfg(any(target_os = "android", target_os = "ios"))]
use startup_assets::seed_bundled_assets_if_needed;
use startup_assets::{
    AssetFolderPurpose, ensure_first_run_assets, load_profile_sound, open_asset_folder_dialog,
    open_tile_set_dialog, reload_asset_profile, show_asset_error,
};
#[cfg(test)]
use status_display::{CITY_CLEAR_COLOR, UNDERGROUND_CLEAR_COLOR};
use terrain_editing::{
    EditMapLevelDrag, EditMapStretchDrag, PE_447490_UI_WAIT_4012C1_PER_CALL,
    PE_SPREAD_TOOL4_UI_TIMER_INIT, TerrainDrag, pe_fun_00447020_mode0_xund_slope_sea,
    pe_fun_00447020_mode1_destroy_and_city_clear_447085, pe_lower_terrain_from_packed_443900,
    pe_selected_water_height_for_fun_00447020, pe_spread_terrain_from_packed_442a30_with_crt,
    pe_terrain_nesw_land_copy_442a30_with_crt, pe_volcano_apply_flatten_with_crt,
    pe_volcano_flatten_check,
};
#[cfg(test)]
use terrain_editing::{
    PE_POST_FLATTEN_NEIGHBOR_XBLD_GATE, PE_RAISE_SUBMODE_DEFAULT, PE_TERRAIN_TOOL_LEVEL,
    PE_TERRAIN_TOOL_LOWER, PE_TERRAIN_TOOL_NESW_COPY, PE_TERRAIN_TOOL_STRETCH,
    PE_VOLCANO_FLATTEN_COST, PE_VOLCANO_FLATTEN_MAX_HEIGHT, PeTerrainCoordinateQueue,
    pe_can_raise_stretch_hill_444ff0, pe_fun_00447020_slope_xter, pe_lower_444440_mode_cost,
    pe_lower_terrain_444440_mode_skips_funds, pe_lower_terrain_crt_step_443900,
    pe_lower_terrain_land_dec_444440, pe_lower_terrain_land_dec_444440_mode_with_submode,
    pe_lower_terrain_land_dec_444440_mode_with_submode_and_crt,
    pe_lower_terrain_land_dec_444440_mode_with_submode_and_crt_effects,
    pe_lower_unpack_tile_444bf0, pe_post_flatten_neighbor_altm_445b50,
    pe_raise_adjust_land_toward_443200, pe_raise_adjust_land_toward_443200_with_crt,
    pe_raise_drag_step_dir_445da0, pe_raise_epilogue_surface_reshape_kind_44c7a0,
    pe_raise_epilogue_underground_reshape_kind_44c9b0, pe_raise_land_inc_443c60,
    pe_raise_land_inc_443c60_with_crt, pe_raise_unpack_tile_444bf0, pe_set_flatten_mark,
    pe_spread_terrain_from_packed_442a30, pe_stretch_hill_crt_jitter,
    pe_stretch_hold_after_successful_hill_443b88, pe_stretch_hold_ui_refresh_due_443ba6,
    pe_stretch_terrain_from_packed_443a50, pe_stretch_terrain_hill_step_443a50,
    pe_stretch_terrain_hill_step_443a50_with_crt, pe_stretch_terrain_mouse_land_444951,
    pe_terrain_neighbor_fun_00447020_447490, pe_terrain_nesw_land_copy_442a30,
    pe_terrain_nesw_land_copy_442a30_if_moved, pe_volcano_apply_flatten,
};
#[cfg(test)]
use tile_classification::is_rubble_tile;
use tile_classification::{
    is_bridge_tile, is_decorative_water_terrain, is_highway_tile, is_power_tile, is_rail_tile,
    is_road_tile, is_surface_infrastructure_tile, is_tunnel_tile, is_water_terrain,
};
use title_flow::run_title_screen;
#[cfg(test)]
use tool_controller::one_click_placement_sound;
#[cfg(test)]
use tool_controller::{
    DragReleaseOutcome, dispatch_kind_for_tool, format_tool_cost_status, is_chooser_menu_tool,
    is_dispatch_tool, tool_allowed_in_underground_view,
};
use tool_controller::{
    PalettePress, ToolPlacement, ToolPreview, tool_allows_touch_paint_stroke, tool_build_cost,
};
#[cfg(test)]
use transport_maintenance::transport_surface_budget_index;
use ui_format::{format_number, format_signed_number};
#[cfg(test)]
use underground::underground_placement_sound;
use underground::{
    PE_PIPE_WALK_DIRECTIONS_4E7B18, PE_PIPE_XTER_SLOPE_ADJUST_4E7BA8, PE_PLACE_PIPES_MASK_TILE,
    PE_PLACE_PIPES_XTXT_GATE, PE_PLACE_PIPES_XTXT_TILE, PE_XUND_EW_GATE_4E7C70,
    PE_XUND_NS_GATE_4E7CB8, UNDER_TILE_BUILDING_PIPE_TEE, UNDER_TILE_BUILDING_PIPES,
    UNDER_TILE_GREY_BOX_1, UNDER_TILE_GREY_BOX_2, UNDER_TILE_SUBWAY_ENTRANCE, UndergroundDrag,
    UndergroundKind, pe_pipe_alternate_direction_44c6f0, pe_pipe_direction_for_path_tile,
    pe_pipe_direction_to_target_44c680, pe_pipe_inside_stroke_bounds_44f50,
    pe_pipe_step_coordinate_44f50, pe_pipe_tile_slope_blocks_44f50,
    pe_pipe_xter_step_blocked_44f50, pe_place_xund_442370, underground_crossover_axis,
    underground_kind_for_tool, underground_tile_connects_to, underground_tile_for_mask,
};
#[cfg(test)]
use underground::{
    UNDER_TILE_CROSSOVER_PIPES_LR_SUBWAY_TB, UNDER_TILE_CROSSOVER_PIPES_TB_SUBWAY_LR,
    UNDER_TILE_PIPES_LR, UNDER_TILE_PIPES_TB, pe_pipe_next_stroke_tile_448f50,
    pe_pipe_stroke_path_448bc0_448f50, pe_pipe_stroke_walk_init_448bc0,
    pe_place_pipes_crossover_from_existing_44b7f5, pe_place_pipes_mask_tile_44ba9a,
    pe_place_pipes_reshape_neighbor_connects_44b98a, pe_place_pipes_xtxt_reshape_44b961,
    pe_place_subway_crossover_from_existing_44bb8e, pe_subway_mask_tile_44be07,
    pe_subway_reshape_neighbor_connects_44bcf6, pe_subway_xtxt_reshape_44bcc5,
    underground_commit_path_for_kind, underground_crossing_tile_for,
    underground_preview_path_for_kind,
};
#[cfg(test)]
use underground_model::underground_tile_source;
use underground_model::{
    UndergroundRenderTile, UndergroundTileSource, rotated_underground_tile,
    underground_building_pipe_overlay_tile, underground_building_pipe_underlay_tile,
    underground_render_tile, underground_tile_counts_as_subway, underground_tile_has_kind,
};
use view_navigation::{
    MAX_ZOOM, MIN_ZOOM, projected_tile_units, render_zoom, rotate_tile_coords, viewport_center,
    wheel_zoom_factor,
};
#[cfg(test)]
use view_navigation::{
    camera_for_map_units_at_screen, layer_height, projected_tile_units_f32, screen_to_map_units,
    viewport_origin,
};
use weather::*;
#[cfg(test)]
use window_support::scale_ratio;
use window_support::{
    bootstrap_window, configure_linux_video_driver, fit_window_to_usable_display, hide_window,
    input_pixel_scale, pixel_scale, raise_window, show_window, sync_renderer_to_window,
    sync_window, wait_for_window_pixels,
};

const WINDOW_WIDTH: u32 = 1024;
const WINDOW_HEIGHT: u32 = 768;
const SIGN_LABEL_MAX_BYTES: usize = 23;
const RCI_DEMAND_MISC_OFFSET: usize = 0x0718;
const RCI_DEMAND_MIN: i32 = -2_000;
const RCI_DEMAND_MAX: i32 = 2_000;
/// Win95 `SimulationRCIDemandUpdates` scales the zone-balance float by this before `_ftol`.
const PE_RCI_DEMAND_BALANCE_SCALE: f64 = 600.0;
const RCI_DEVELOPMENT_DEMAND_STEP: i32 = 250;
/// Legacy demand thresholds kept for tests; vacant growth no longer uses them.
const RCI_DENSE_2X2_DEMAND_THRESHOLD: i32 = RCI_DEVELOPMENT_DEMAND_STEP / 2;
const RCI_DENSE_3X3_DEMAND_THRESHOLD: i32 = RCI_DEVELOPMENT_DEMAND_STEP;
/// Win95 `XVALPOPLEVEL_*` gates before `StartBuilding` size-up.
const XVAL_POPLEVEL_LOW: u8 = 32;
const XVAL_POPLEVEL_MEDIUM: u8 = 96; // LOW * 3
const XVAL_POPLEVEL_HIGH: u8 = 192; // MEDIUM * 2
const RCI_DEVELOPMENT_SIZES_LIGHT: &[usize] = &[1];
/// Vacant tiles always `StartBuilding(pop=0)` → 1×1 construction only.
const RCI_DEVELOPMENT_SIZES_VACANT: &[usize] = &[1];
const SMALL_PARK_BUILDING_ID: u8 = 13;
const BIG_PARK_BUILDING_ID: u8 = 213;
const HOSPITAL_BUILDING_ID: u8 = 209;
const FIRE_STATION_BUILDING_ID: u8 = 211;
const FIRE_STATION_BUILDING_SIZE: usize = 3;
const POLICE_STATION_BUILDING_ID: u8 = 210;
const POLICE_STATION_BUILDING_SIZE: usize = 3;
const COAL_POWER_BUILDING_ID: u8 = 207;
const COAL_POWER_BUILDING_SIZE: usize = 4;
const SCHOOL_BUILDING_ID: u8 = 214;
const STADIUM_BUILDING_ID: u8 = 215;
const STADIUM_BUILDING_SIZE: usize = 4;
const ZOO_BUILDING_ID: u8 = 218;
const MARINA_BUILDING_ID: u8 = 248;
const CHURCH_BUILDING_ID: u8 = 247;
const CHURCH_BUILDING_SIZE: usize = 2;
const SUBWAY_STATION_BUILDING_ID: u8 = 233;
const BUS_DEPOT_BUILDING_ID: u8 = 236;
const RAIL_STATION_BUILDING_ID: u8 = 237;
const CHURCH_POPULATION_STEP: u32 = 2_500;
const FIRE_STATION_DEMAND_POPULATION_STEP: u32 = 20_000;
const SCHOOL_DEMAND_POPULATION_STEP: u32 = 20_000;
const HOSPITAL_DEMAND_POPULATION_STEP: u32 = 25_000;
/// Stock SC2K `wCityDifficulty`: 0 = none/map, 1 = easy, 2 = medium, 3 = hard.
const MISC_DIFFICULTY_OFFSET: usize = 0x001C;
const CITY_DIFFICULTY_NONE: u32 = 0;
const CITY_DIFFICULTY_EASY: u32 = 1;
const CITY_DIFFICULTY_MEDIUM: u32 = 2;
const CITY_DIFFICULTY_HARD: u32 = 3;
const MISC_CITY_VALUE_OFFSET: usize = 0x0024;
const MISC_LAND_VALUE_OFFSET: usize = 0x0028;
const MISC_CITY_FUNDS_OFFSET: usize = 0x0014;
const MISC_CITY_AGE_OFFSET: usize = 0x0010;
const MISC_YEAR_FOUNDED_OFFSET: usize = 0x000c;
const MISC_CITY_ROTATION_OFFSET: usize = 0x0008;
const MISC_RESIDENTIAL_POPULATION_OFFSET: usize = 0x0074;
const MISC_REWARDS_AVAILABLE_OFFSET: usize = 0x0078;
const MISC_WORK_FORCE_PERCENT_OFFSET: usize = 0x0044;
const MISC_CITY_LIFE_EXPECTANCY_OFFSET: usize = 0x0048;
const MISC_CITY_EDUCATION_QUOTIENT_OFFSET: usize = 0x004c;
const MISC_NATIONAL_POPULATION_OFFSET: usize = 0x0050;
const MISC_NATIONAL_VALUE_OFFSET: usize = 0x0054;
const MISC_NATIONAL_TAX_OFFSET: usize = 0x0058;
const MISC_NATIONAL_TREND_OFFSET: usize = 0x005c;
const MISC_HEAT_OFFSET: usize = 0x0060; // PE `0x004CADE0`
const MISC_WIND_OFFSET: usize = 0x0064; // PE `0x004CAE0C` (weather wind + `0xC8` rand modulus)
const MISC_HUMID_OFFSET: usize = 0x0068; // PE `0x004CB3D4` (weather humidity + `0xCC` solar rand)
const MISC_WEATHER_TREND_OFFSET: usize = 0x006c; // PE `0x004CB40C`
/// Stock SC2K `wSetTriggerDisasterType` (`0x4CA420`).
const MISC_TRIGGER_DISASTER_OFFSET: usize = 0x0070;
const MISC_PE_GROWTH_AGGREGATE_OFFSET: usize = 0x05f0;
const MISC_ARCOLOGY_POPULATION_OFFSET: usize = 0x1020;
const MISC_NORMAL_POPULATION_OFFSET: usize = 0x102c;
const MISC_BUILDING_TILE_COUNTS_OFFSET: usize = 0x01f0;
const BUILDING_TILE_COUNT_VALUES: usize = 256;
/// Stock SC2K `dwCityUnemployment` (`0x4CA5F8`).
const MISC_UNEMPLOYMENT_OFFSET: usize = 0x0FA4;
/// Mac CODE 4 persists the riot gate input `fffffdf2` at state-vector slot 1001.
const MAC_RIOT_UNEMPLOYMENT_STATE_OFFSET: usize = 0x0FA4;
/// Mac CODE 8 weather byte `fffff084` is the low byte of state-vector slot 24.
const MAC_RIOT_WEATHER_HEAT_STATE_OFFSET: usize = 0x0060;
/// Mac CODE 8 `fffff01a` demand scalar is persisted at state-vector slot 1035.
const MAC_MASS_RIOT_GROWTH_STATE_OFFSET: usize = 0x102C;
const MISC_MILITARY_TILE_COUNTS_OFFSET: usize = 0x0fa8;
const MILITARY_TILE_COUNT_VALUES: usize = 16;
/// PE `0x4ca934` word index → special XBLD (word 0 is the non-special catch-all).
#[allow(dead_code)]
const MILITARY_TILE_COUNT_BUILDINGS: [Option<u8>; MILITARY_TILE_COUNT_VALUES] = [
    None,      // Total military tiles.
    Some(221), // Straight runway.
    Some(222), // Runway intersection.
    Some(239), // Military parking lot.
    Some(242), // Cargo yard.
    Some(234), // Radar.
    Some(227), // Warehouse.
    Some(228), // Airport building 1.
    Some(229), // Airport building 2.
    Some(241), // Top Secret.
    Some(224), // Crane.
    Some(226), // Military control tower.
    Some(231), // F-15B.
    Some(232), // Military hangar / Hangar 1.
    Some(246), // Big hangar / Hangar 2.
    Some(249), // Missile silo.
];
/// Stock SC2K `wCityMode`: 0 = map editor, 1 = gameplay, 2 = disaster.
const MISC_CITY_MODE_OFFSET: usize = 0x0004;
const CITY_MODE_EDIT_MAP: u32 = 0;
const CITY_MODE_GAMEPLAY: u32 = 1;
const CITY_MODE_DISASTER: u32 = 2;
// PE / stock MISC subway field: BSS `0x4ca41c` PlaceXUND occupancy (`0x431a40`/`0x4340b8`).
const MISC_SUBWAY_COUNT_OFFSET: usize = 0x0fe8;
const MISC_SPEED_OFFSET: usize = 0x0fec;
const MISC_SPORTS_TEAMS_OFFSET: usize = 0x1028;
/// Stock SC2K `wIndustrialMixBonus` / `wIndustrialMixPollutionBonus` / `wOldArrests` /
/// `wPrisonBonus` / `wSewerBonus`. Docs label `0x103C` as "Police Bonus"; sc2kfix is `wPrisonBonus`.
const MISC_INDUSTRIAL_MIX_BONUS_OFFSET: usize = 0x1030;
const MISC_INDUSTRIAL_MIX_POLLUTION_BONUS_OFFSET: usize = 0x1034;
const MISC_OLD_ARRESTS_OFFSET: usize = 0x1038;
const MISC_PRISON_BONUS_OFFSET: usize = 0x103C;
/// Stock SC2K `wMonsterXTHGIndex` / `wCurrentDisasterType` / `dwDisasterActive`
/// (`0x4CA81C` / `0x4CA93C` / `0x4C9EE8`).
const MISC_MONSTER_XTHG_INDEX_OFFSET: usize = 0x1040;
const MISC_CURRENT_DISASTER_TYPE_OFFSET: usize = 0x1044;
const MISC_DISASTER_ACTIVE_OFFSET: usize = 0x1048;
const MISC_SEWER_BONUS_OFFSET: usize = 0x1050;
/// Viewer leftover at `0x1400`. PE graphs live in XGRP; we no longer roll this ring.
const GRAPH_HISTORY_MISC_OFFSET: usize = 0x1400;
/// Viewer leftover at `0x1300`. Unused extra graph ring; kept as a MISC-span probe
/// so short blank MISC is not extended, and to prove it does not occupy EQ_LE
/// `0x7c` or stock mix/sewer bonus MISC `0x1030..0x1054`.
const GRAPH_EXTRA_HISTORY_MISC_OFFSET: usize = 0x1300;
/// Viewer extension: last-month Win95 passenger totals (not a stock MISC field).
const MISC_BUS_PASSENGERS_OFFSET: usize = 0x1120;
const MISC_RAIL_PASSENGERS_OFFSET: usize = 0x1124;
const MISC_SUBWAY_PASSENGERS_OFFSET: usize = 0x1128;
/// Viewer extension: Win95 `dwPowerUsedPercentage` / `dwWaterUsedPercentage` snapshots.
const MISC_POWER_USED_PERCENT_OFFSET: usize = 0x112c;
const MISC_WATER_USED_PERCENT_OFFSET: usize = 0x1130;
/// Mac CODE 14 `export_218` power summary `fffff00a`, MISC dword 1048.
const MAC_POWER_SERVICE_PERCENT_STATE_OFFSET: usize = 1048 * 4;
const GRAPH_HISTORY_SAMPLES: usize = 20;
const GRAPH_HISTORY_SERIES: usize = 3;
const GRAPH_EXTRA_HISTORY_SERIES: usize = 3;
const GRAPH_HISTORY_TOTAL_SERIES: usize = GRAPH_HISTORY_SERIES + GRAPH_EXTRA_HISTORY_SERIES;
const GRAPH_HISTORY_VALUES: usize = GRAPH_HISTORY_SAMPLES * GRAPH_HISTORY_SERIES;
const GRAPH_EXTRA_HISTORY_VALUES: usize = GRAPH_HISTORY_SAMPLES * GRAPH_EXTRA_HISTORY_SERIES;

const GRAPH_HISTORY_LEGEND: [(&str, [f32; 4]); GRAPH_HISTORY_TOTAL_SERIES] = [
    ("Population", [0.16, 0.62, 0.20, 1.0]),
    ("Health", [0.82, 0.18, 0.18, 1.0]),
    ("Education", [0.16, 0.28, 0.84, 1.0]),
    ("Crime", [0.62, 0.18, 0.76, 1.0]),
    ("Traffic", [0.90, 0.54, 0.12, 1.0]),
    ("Pollution", [0.32, 0.32, 0.32, 1.0]),
];
/// Win95 `GRP_CITYSIZE` / `GRP_HEALTH` / `GRP_EDUCATION` / `GRP_CITYCRIME` /
/// `GRP_TRAFFIC` / `GRP_POLLUTION` for the six legend lines.
const XGRP_GRAPH_LEGEND_SERIES: [usize; GRAPH_HISTORY_TOTAL_SERIES] = [0, 10, 11, 7, 4, 5];

const MISC_CITY_CRIME_OFFSET: usize = 0x002c;
const MISC_CITY_TRAFFIC_OFFSET: usize = 0x0030;
const MISC_CITY_POLLUTION_OFFSET: usize = 0x0034;
const MISC_CITY_FAME_OFFSET: usize = 0x0038;
/// Stock SC2K `dwCityAdvertising` (`0x4CB018`). Unused by vanilla gameplay; PE still load/saves it.
const MISC_CITY_ADVERTISING_OFFSET: usize = 0x003c;
const MISC_GARBAGE_OFFSET: usize = 0x0040;
/// These PE MISC mirrors collide with Mac CODE 4 state-vector slots. In
/// particular, +0x2c/+0x30/+0x34 are native XCRM/XTRF/XPLT aggregates, not
/// the PE crime/traffic/pollution snapshots. The +0x38/+0x3c
/// Mac roles are still opaque; +0x44/+0x74 are known saved state.
const MAC_NATIVE_MISC_STATE_COLLISIONS: &[usize] = &[
    MISC_CITY_MODE_OFFSET,
    MISC_CITY_ROTATION_OFFSET,
    MISC_CITY_AGE_OFFSET,
    MISC_DIFFICULTY_OFFSET,
    MISC_CITY_PROGRESSION_OFFSET,
    MISC_CITY_CRIME_OFFSET,
    MISC_CITY_TRAFFIC_OFFSET,
    MISC_CITY_POLLUTION_OFFSET,
    MISC_CITY_FAME_OFFSET,
    MISC_CITY_ADVERTISING_OFFSET,
    MISC_GARBAGE_OFFSET,
    MISC_WORK_FORCE_PERCENT_OFFSET,
    MISC_CITY_LIFE_EXPECTANCY_OFFSET,
    MISC_CITY_EDUCATION_QUOTIENT_OFFSET,
    MISC_HEAT_OFFSET,
    MISC_WIND_OFFSET,
    MISC_HUMID_OFFSET,
    MISC_WEATHER_TREND_OFFSET,
    MISC_TRIGGER_DISASTER_OFFSET,
    MISC_RESIDENTIAL_POPULATION_OFFSET,
    MISC_REWARDS_AVAILABLE_OFFSET,
    MISC_NORMAL_POPULATION_OFFSET,
    MISC_ARCOLOGY_POPULATION_OFFSET,
    MISC_UNEMPLOYMENT_OFFSET,
    0x0f98, // Mac-owned slot 998.
    0x0fa0, // Mac-owned state slot 1000.
    0x0fa8, // Mac slot 1002; first word of the zone-7 census.
    MISC_SUBWAY_COUNT_OFFSET,
    MISC_SPEED_OFFSET,
    MISC_SPORTS_TEAMS_OFFSET,
    NEIGHBOR_CONNECTION_MISC_OFFSET,
    MISC_INDUSTRIAL_MIX_BONUS_OFFSET,
    MISC_INDUSTRIAL_MIX_POLLUTION_BONUS_OFFSET,
    MISC_OLD_ARRESTS_OFFSET,
    MISC_PRISON_BONUS_OFFSET,
    MISC_SEWER_BONUS_OFFSET,
    0x0e40, // Shared selected-height slot 912; native generators/controls publish it.
    0x0e44, // Mac slot 913, `fffffd50`; PE city-has-ocean mirror.
    0x0e48, // Mac slot 914, `fffffd51`; PE city-has-river mirror.
    MISC_MILITARY_BASE_TYPE_OFFSET,
    // Mac CODE 4 slot 911 serializes `ffffe4ae`; PE year-end flag is not shared.
    MISC_YEAR_END_OFFSET,
];

/// PE-owned MISC mirrors are safe to publish only when both the selected
/// engine and the serialized city layout use the Win95 field map.
fn pe_misc_mirror_storage_active(city: &City, fidelity: EngineFidelity) -> bool {
    fidelity.uses_win95() && city.kind != CityFileKind::MacCity
}

/// Stock SC2K `wCityGarbage` (`0x4CA5F0`). Unused by gameplay; PE still accumulates it on day 22.
const NEIGHBOR_CONNECTION_MISC_OFFSET: usize = 0x1024;

/// Win95 paper names (GameFAQs order); PE handlers write choice `0..=6`, menu clamps to `0..=5`.
const NEWSPAPER_PAPER_NAMES: [&str; 6] = [
    "Times",
    "Chronicle",
    "Courier",
    "Picayune",
    "Herald",
    "Journal",
];
const NEWSPAPER_CHOICE_MAX: u32 = (NEWSPAPER_PAPER_NAMES.len() - 1) as u32;
/// GameFAQs/PE paper index for Courier; Win95 menu shows it once `wCityProgression >= 2`.
const NEWSPAPER_COURIER_CHOICE: u32 = 2;
/// Win95 newspaper paper records @ `0x4CAD24`, saved as MISC `0x0E50` (6×5 bytes → 30 dwords).
const MISC_NEWSPAPER_LIST_OFFSET: usize = 0x0E50;
const NEWSPAPER_LIST_RECORD_COUNT: usize = 6;
const NEWSPAPER_LIST_RECORD_BYTES: usize = 5;
const NEWSPAPER_LIST_MISC_DWORDS: usize = NEWSPAPER_LIST_RECORD_COUNT * NEWSPAPER_LIST_RECORD_BYTES;
/// Win95 `0x4797c0`: `_Random` pairwise byte-swaps repeat `si=0x0C` times.
const PE_NEWSPAPER_SHUFFLE_ROUNDS: usize = 0x0C;
/// Win95 newspaper story records @ `0x4CA1C0`, saved as MISC `0x0EC8` (9×(2 words + 4 bytes)).
const MISC_NEWSPAPER_STORY_LIST_OFFSET: usize = 0x0EC8;
const NEWSPAPER_STORY_RECORD_COUNT: usize = 9;
const NEWSPAPER_STORY_MISC_FIELDS: usize = 6;
const NEWSPAPER_STORY_LIST_MISC_DWORDS: usize =
    NEWSPAPER_STORY_RECORD_COUNT * NEWSPAPER_STORY_MISC_FIELDS;
/// Stock SC2K `bYearEndFlag` (`0x4CAD2C`): 1 while `wCityCurrentMonth == 0x0B` (December).
const MISC_YEAR_END_OFFSET: usize = 0x0E3C;
const EDIT_MAP_COAST_MISC_OFFSET: usize = 0x0e44;
const EDIT_MAP_RIVER_MISC_OFFSET: usize = 0x0e48;

const NUCLEAR_POWER_BUILDING_ID: u8 = 203;
const MICROWAVE_POWER_BUILDING_ID: u8 = 205;

fn pe_place_tile_441f00(city: &mut City, x: usize, y: usize, building: u8) {
    // PE PlaceTile @ `0x441f08`/`0x441f12`: refuse x/y outside `0..0x80`.
    if x >= 0x80 || y >= 0x80 {
        return;
    }
    // PE @ `0x441f4c`/`0x441f52`–`0x441f73`: non-military adjusts `0x4ca4c8` WORD[256]
    // (dec old XBLD, inc new). Military XZON type 7 adjusts `0x4ca934` WORD[16]
    // (@ `0x441f86`/`0x442066`) then writes.
    let old = city.building_at(x, y);
    let military = city.zone_at(x, y).is_some_and(|z| z.zone_id() == 7);
    if military {
        city.pe_adjust_military_tile_count_441f86(old, building);
    } else {
        city.pe_dw_tile_count[old as usize] = city.pe_dw_tile_count[old as usize].wrapping_sub(1);
        city.pe_dw_tile_count[building as usize] =
            city.pe_dw_tile_count[building as usize].wrapping_add(1);
    }
    city.set_building(x, y, building);
}

/// Map-space directions encoded in road tunnel portals in both originals.
fn tunnel_portal_direction(building: u8) -> Option<(isize, isize)> {
    match building {
        0x3f => Some((-1, 0)),
        0x40 => Some((0, -1)),
        0x41 => Some((1, 0)),
        0x42 => Some((0, 1)),
        _ => None,
    }
}

fn offset_city_tile(x: usize, y: usize, dx: isize, dy: isize) -> Option<(usize, usize)> {
    let nx = x as isize + dx;
    let ny = y as isize + dy;
    if nx < 0 || ny < 0 {
        return None;
    }

    let nx = nx as usize;
    let ny = ny as usize;
    (nx < CITY_WIDTH && ny < CITY_HEIGHT).then_some((nx, ny))
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ZoneDevelopmentPlan {
    origin: (usize, usize),
    zone_id: u8,
    building: u8,
    footprint: Vec<(usize, usize)>,
    /// Win95 `PerhapsGeneralZoneStartBuilding` path: stamp construction first.
    /// Abandoned redevelop uses `ChooseAndPlaceBuilding` and finishes immediately.
    place_as_construction: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct RciDevelopmentCandidate {
    plan: ZoneDevelopmentPlan,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct RciDevelopmentServices {
    police: Vec<CoverageSource>,
    fire: Vec<CoverageSource>,
    health: Vec<CoverageSource>,
    education: Vec<CoverageSource>,
    recreation: Vec<CoverageSource>,
    transit: Vec<CoverageSource>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct LiveDensityMaps {
    pe_city_center: Option<(i16, i16)>,
    land_value: Vec<u8>,
    crime: Vec<u8>,
    pollution: Vec<u8>,
    police: Vec<u8>,
    fire: Vec<u8>,
    population_density: Vec<u8>,
    rate_of_growth: Vec<u8>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RciKind {
    Residential,
    Commercial,
    Industrial,
}

type BuildablePrefixPath = (Vec<(usize, usize)>, Option<(usize, usize)>);

struct ViewerState {
    city: City,
    camera: Vec2,
    zoom: f32,
    /// Backing scale of the current viewport, retained across sprite-cache invalidations.
    viewport_pixel_scale: f32,
    /// Grid rotation used for tile screen positions. Keep this coupled to the effective compass:
    /// infrastructure sprite ids and tile positions must rotate together or roads disconnect.
    camera_rotation: u8,
    show_grid: bool,
    show_zone_overlay: bool,
    show_traffic: bool,
    data_map_mode: CityDataMapMode,
    show_buildings: bool,
    show_signs: bool,
    show_infrastructure: bool,
    show_zones: bool,
    show_underground: bool,
    modifiers: Mod,
    auto_goto_enabled: bool,
    pending_disaster_auto_goto: Option<(usize, usize)>,
    pending_disaster_start_sound: Option<PlacementSound>,
    /// Win95 consume common-path `0x208` / OpenSC2K `SOUND_SIREN` after the type sound.
    pending_disaster_start_siren: bool,
    pending_disaster_extra_sounds: Vec<PlacementSound>,
    /// Win95 type-6 `ebx < 0x18` dest-blit shake remaining at `0x45eb20`.
    earthquake_shake_remaining: u8,
    /// Win95 flood/hurricane duration at `0x4c93a8` (not a MISC field).
    pe_disaster_duration: u16,
    /// Win95 hurricane wind remainder at `0x4e86b0` (not a MISC field).
    pe_hurricane_wind_remaining: u16,
    sound_effects_enabled: bool,
    music_enabled: bool,
    no_disasters_enabled: bool,
    balloon_help_enabled: bool,
    balloon_help_line: Option<&'static str>,
    anim_time: f32,
    traffic_redraw_epoch: u32,
    traffic_redraw_accumulator: f32,
    pending_traffic_redraw: bool,
    pe_idle_scheduler: pe_idle_scheduler::PeIdleScheduler,
    mac_loop_scheduler: mac_loop_scheduler::MacLoopScheduler,
    traffic: TrafficSystem,
    win95_overlay_sim: Win95OverlaySim,
    /// Win95 shared CRT `rand()` stream consumed by weather, day-1 power recalc, growth, etc.
    pe_process_crt: Win95CrtRand,
    /// QuickDraw process `_Random` stream. Starts at InitGraf's seed; each
    /// recovered Mac caller must share this state instead of reseeding locally.
    mac_process_rng: MacToolboxRng,
    /// Mac CODE 6 `ffffa0a2`: process-global kind-2 sound deadline.
    /// It is reset by the native New City worker, not by opening a saved city.
    mac_kind2_sound_deadline: u32,
    /// Mac CODE 13 app-owned game-LCG shared by map-generation and XMIC zoo calls.
    mac_game_rng: MacGameLcgRng,
    /// Win95 game-specific LCG (`0x4cdb80`), separate from CRT and Game_Random.
    pe_process_game_rand: Win95GameRand,
    /// Whether a held editor height tool has made any accepted edit.
    edit_map_height_hold: Option<bool>,
    /// PE Raise tool-0 UI byte at `[0x4ca434]+5` from XTER≠0 (`0x442e8b`).
    pe_raise_xter_ui_byte: u8,
    /// PE Raise tool-0 UI tool-id at `[0x4ca434]+0` (`0x442e15` = 4).
    pe_raise_ui_tool_id: u8,
    /// PE Raise tool-0 UI tile X/Y at `[0x4ca434]+3`/`+4` (`0x443003`/`0x44300c`).
    pe_raise_ui_tile_x: u8,
    pe_raise_ui_tile_y: u8,
    /// PE Raise tool-0 UI byte at `[0x4ca434]+1` (`0x442ff4`).
    pe_raise_ui_press_y_byte: u8,
    /// PE Raise tool-0 entry zeros UI bytes `+6`/`+7`/`+8`/`+9` (`0x442e3c`–`0x442e4e`).
    pe_raise_ui_byte6: u8,
    pe_raise_ui_byte7: u8,
    pe_raise_ui_byte8: u8,
    pe_raise_ui_byte9: u8,
    /// PE terrain tool-4 `[0x4cad4c]` init (`0x442a88` = `0xfffffc18`).
    pe_spread_tool4_ui_timer: i32,
    /// PE Spread hold prev-tile X/Y at `[esp+0x14]`/`[esp+0x16]` (`0x442c7e`/`0x442c83`).
    pe_spread_prev_tile_x: i16,
    pe_spread_prev_tile_y: i16,
    /// Win95 Stretch refreshes after each accepted height request at 0x444a32.
    pe_stretch_hold_ui_refresh_count: u32,
    /// Win95 Stretch hold cursor at 0x44494c.
    pe_stretch_hold_cursor_id: u16,
    pe_stretch_hold_cursor_arg: i16,
    /// Win95 terrain refresh waits tracked by the Stretch adapters.
    pe_stretch_hold_ui_wait_4012c1_count: u32,
    /// PE Lower redraw after each in-bounds dig call (0x4439c9).
    pe_lower_hold_ui_refresh_count: u32,
    /// City-view terrain leveling refresh waits.
    pe_level_hold_ui_wait_4012c1_count: u32,
    /// PE Spread hold UI refresh count (`0x442c8c` / exit `0x442cf0`→`0x40226b`).
    pe_spread_hold_ui_refresh_count: u32,
    /// PE Spread hold cursor from `0x401e7e(0x1fc, -1)` (`0x442a5f`/`0x442a7d`).
    pe_spread_hold_cursor_id: u16,
    pe_spread_hold_cursor_arg: i16,
    /// PE Spread `0x401758`→`0x4519e0` UI-cursor helper call count (arg 0 @ `0x442bd3`/`0x442c63`/`0x442ce4`).
    pe_spread_ui_cursor_4519e0_count: u32,
    pe_spread_ui_cursor_4519e0_last_arg: i32,
    /// PE Spread `0x4012c1` UI wait count (`0x442c1c`/`0x442c71`/`0x442cda`).
    pe_spread_ui_wait_4012c1_count: u32,
    /// PE Lower hold blit via `[0x4efe74]` after UI (`0x4439d2`).
    pe_lower_hold_blit_count: u32,
    /// Win95 shared Game_Random LFSR (`0x4cdb7c`) for disaster rubble/recede/wind arms.
    pe_process_lfsr: Win95Lfsr,
    /// Explicit executable-era behavior. City file kind identifies the save format only.
    engine_fidelity: EngineFidelity,
    game_clock: GameClock,
    /// Placeholder growth is an internal synthetic development path, not recovered SC2K sim.
    /// Keep it disabled for loaded cities so monthly ticks do not rewrite real saves.
    zone_growth_enabled: bool,
    /// Loaded/new cities can opt into synthetic growth only for tiles the player manually zones.
    /// Test fixtures leave this false so recovered growth helpers can exercise whole-map cases.
    zone_growth_uses_allowlist: bool,
    zone_growth_allowed_tiles: Vec<bool>,
    /// Origins that completed a successful trip this month (Win95 growth-tick demand gate).
    trip_served_tiles: Vec<bool>,
    /// Isolated RCI fixtures skip the trip-generator demand gate. Live play does not.
    bypass_growth_trip_gate: bool,
    /// Win95 transient `0x4CA428` growth aggregate for the current simulation month.
    pe_growth_aggregate: PeGrowthAggregate,
    /// Win95 `dwBusPassengers` / `dwRailPassengers` / `dwSubwayPassengers` for the latest month.
    trip_bus_passengers: u32,
    trip_rail_passengers: u32,
    trip_subway_passengers: u32,
    scenario_start_clock: Option<GameClock>,
    announced_scenario_outcome: Option<ScenarioOutcome>,
    /// PE day-22 `dwCityFunds < -100000` → `GAMEOVER_BANKRUPT` (once per load).
    announced_bankruptcy: bool,
    edit_map_generation_seed: u32,
    edit_map_sliders: ui::EditMapSliderValues,
    edit_map_slider_drag: Option<ui::EditMapSlider>,
    mode: AppMode,
    tool: EditTool,
    open_tool_menu: Option<ui::ToolMenu>,
    hover_tile: Option<(usize, usize)>,
    zone_drag: Option<ZoneDrag>,
    network_drag: Option<NetworkDrag>,
    underground_drag: Option<UndergroundDrag>,
    highway_drag: Option<HighwayDrag>,
    terrain_drag: Option<TerrainDrag>,
    edit_map_stretch_drag: Option<EditMapStretchDrag>,
    edit_map_level_drag: Option<EditMapLevelDrag>,
    nature_drag: Option<NatureDrag>,
    power_plant_dialog: Option<PowerPlantDialogState>,
    arcology_dialog: Option<ArcologyDialogState>,
    sign_dialog: Option<SignDialogState>,
    military_base_dialog: Option<MilitaryBaseDialogState>,
    city_info_dialog: Option<CityInfoDialog>,
    status_panel_origin: Option<Vec2>,
    status_panel_grab: Option<Vec2>,
    info_panel_origin: Option<Vec2>,
    info_panel_scale: f32,
    info_panel_drag: Option<(bool, Vec2, Vec2, f32)>,
    founding_newspaper_pending: bool,
    graph_series_visible: [bool; GRAPH_HISTORY_TOTAL_SERIES],
    graph_scale: GraphScale,
    query_line: String,
    query_is_inspect: bool,
    /// Set by SimCalendar announcements so later days do not rewrite the idle clock.
    query_status_pinned: bool,
    sprite_cache: SpriteCache,
    palette_press: Option<PalettePress>,
    /// In-game menubar checkmarks / enable flags (Linux/Windows; macOS uses AppKit).
    menu_checks: MenuCheckState,
    dark_appearance: bool,
    menubar: MenubarUi,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct FloatRect {
    x: f32,
    y: f32,
    w: f32,
    h: f32,
}

impl ViewerState {
    fn advance_ambient_animation(&mut self, delta_seconds: f32) {
        self.anim_time += delta_seconds;
    }

    fn tick_win95_overlay_sim_frame(&mut self, delta_seconds: f32) -> bool {
        if !self.engine_fidelity.uses_win95() {
            return false;
        }
        let speed = self.traffic.speed();
        if uses_win95_overlay_draw(&self.city) {
            self.win95_overlay_sim
                .tick_with_game_rand_defer_xthg_slot_actors(
                    &mut self.city,
                    speed,
                    delta_seconds,
                    &mut self.pe_process_crt,
                    &mut self.pe_process_lfsr,
                    &mut self.pe_process_game_rand,
                )
        } else if speed != GameSpeed::Paused {
            // PE ambient boats/trains still move when the road-overlay sim is off.
            self.win95_overlay_sim
                .tick_ambient_with_game_rand_defer_xthg_slot_actors(
                    &mut self.city,
                    speed,
                    delta_seconds,
                    &mut self.pe_process_crt,
                    &mut self.pe_process_lfsr,
                    &mut self.pe_process_game_rand,
                )
        } else {
            false
        }
    }

    fn animation_frame(&self) -> u8 {
        let frames_per_second = match self.engine_fidelity {
            EngineFidelity::Win95 => ANIMATION_FPS,
            EngineFidelity::Mac68k => MAC_PALETTE_ANIMATION_FPS,
        };
        ((self.anim_time * frames_per_second).floor() as u64 % 8) as u8
    }

    fn underground_surface_building_at(&self, x: usize, y: usize) -> u8 {
        if self.city.dispatch_actor_at(x, y).is_some() {
            return 0;
        }
        let building = self.city.building_at(x, y);
        if NetworkKind::Road.contains(building) {
            let mask = self.network_neighbor_mask(x, y, NetworkKind::Road);
            if mask != 0 {
                return network_building_for_mask(NetworkKind::Road, mask);
            }
        }
        building
    }

    fn sim_calendar_day19_traffic(&mut self) {
        // Win95 `SimulationUpdateMonthlyTrafficData` @ `0x4586F0`: after days 3–18
        // trips, decay each XTRF cell (`value - (value >> 2)`) and store the sum of
        // microsim cells as city traffic. Do not re-run the trip generator here.
        // Mac `FUN_0002997c` (calendar slot `0x13` / day-19) likewise decays/sums the
        // Mac XTRF plane into the saved traffic total. Keep that MISC write separate
        // from the Win95-only `dwCityTrafficCount` runtime cache at `0x4CC6F4`.
        trip_generator::decay_traffic_density(&mut self.city);
        let traffic = trip_generator::traffic_density_microsim_sum(&self.city);
        if self.engine_fidelity.uses_win95() {
            self.city.set_misc_u32_at(MISC_CITY_TRAFFIC_OFFSET, traffic);
            self.city.set_pe_traffic_count(traffic);
        } else {
            self.city
                .set_misc_city_traffic_total_preserving_pe_cache(traffic);
        }
        self.pending_traffic_redraw = true;
        self.city.mark_all_dirty();
    }

    pub(super) fn pe_process_crt_seed(&self) -> u32 {
        self.pe_process_crt.seed()
    }

    pub(super) fn pe_process_lfsr_state(&self) -> u16 {
        self.pe_process_lfsr.state()
    }

    /// Mac CODE 13 export 198: inspect west/east/north/south XBLD neighbors
    /// and dispatch their surface-network band to its retile worker.
    fn mac_export198_retile_surface_neighbors(&mut self, x: usize, y: usize) {
        for (nx, ny) in neighbor_tiles(x, y) {
            let kind = match self.city.building_at(nx, ny) {
                0x0e..=0x1c => NetworkKind::Power,
                0x1d..=0x2b => NetworkKind::Road,
                0x2c..=0x3a => NetworkKind::Rail,
                _ => continue,
            };
            self.refresh_network_tile(nx, ny, kind);
        }
    }
}

fn neighbor_tiles(x: usize, y: usize) -> impl Iterator<Item = (usize, usize)> {
    [
        x.checked_sub(1).map(|nx| (nx, y)),
        (x + 1 < CITY_WIDTH).then_some((x + 1, y)),
        y.checked_sub(1).map(|ny| (x, ny)),
        (y + 1 < CITY_HEIGHT).then_some((x, y + 1)),
    ]
    .into_iter()
    .flatten()
}

fn subway_tile_count_for_city(city: &City) -> u32 {
    let mut count = 0_u32;
    for y in 0..CITY_HEIGHT {
        for x in 0..CITY_WIDTH {
            if underground_tile_counts_as_subway(city.underground_at(x, y)) {
                count = count.saturating_add(1);
            }
        }
    }
    count
}

trait AmbientDisasterRandom: Sized {
    fn from_seed(seed: u32) -> Self;
    fn random_u16(&mut self) -> u16;
    fn random_range(&mut self, max: usize) -> usize;
    /// When `Some`, mass-riot / blizzard start gates should peek this CRT copy
    /// (live Win95 process stream) instead of the offline city-name helper seed.
    fn as_win95_crt_peek(&self) -> Option<Win95CrtRand> {
        None
    }
}

impl AmbientDisasterRandom for MacToolboxRng {
    fn from_seed(seed: u32) -> Self {
        Self::new(seed)
    }

    fn random_u16(&mut self) -> u16 {
        MacToolboxRng::random_u16(self)
    }

    fn random_range(&mut self, max: usize) -> usize {
        if max == 0 {
            return 0;
        }
        let span = i32::try_from(max).unwrap_or(i32::MAX);
        i32::from(self.random_i16()).rem_euclid(span) as usize
    }
}

impl AmbientDisasterRandom for Win95CrtRand {
    fn from_seed(seed: u32) -> Self {
        Self::new(seed)
    }

    fn random_u16(&mut self) -> u16 {
        Win95CrtRand::random_u16(self)
    }

    fn random_range(&mut self, max: usize) -> usize {
        if max == 0 {
            return 0;
        }
        let modulus = u16::try_from(max).unwrap_or(u16::MAX);
        usize::from(self.random_mod(modulus))
    }

    fn as_win95_crt_peek(&self) -> Option<Win95CrtRand> {
        Some(*self)
    }
}

fn highway_segment_count_for_city(city: &City) -> u32 {
    let mut claimed = vec![false; CITY_WIDTH * CITY_HEIGHT];
    let mut count = 0_u32;
    for origin_y in (0..CITY_HEIGHT.saturating_sub(1)).step_by(2) {
        for origin_x in (0..CITY_WIDTH.saturating_sub(1)).step_by(2) {
            let tiles = highway_segment_tiles((origin_x, origin_y), NetworkAxis::GridX);
            let complete = tiles.len() == HIGHWAY_TILES_PER_SEGMENT
                && tiles.iter().all(|&(x, y)| {
                    let index = y * CITY_WIDTH + x;
                    !claimed[index] && is_highway_tile(city.building_at(x, y))
                });
            if !complete {
                continue;
            }
            count = count.saturating_add(1);
            for (x, y) in tiles {
                claimed[y * CITY_WIDTH + x] = true;
            }
        }
    }
    count
}

fn point_in_float_rect(point: Vec2, rect: FloatRect) -> bool {
    point.x >= rect.x && point.x < rect.x + rect.w && point.y >= rect.y && point.y < rect.y + rect.h
}

fn push_ui_rect(solids: &mut Vec<UiSolid>, rect: FloatRect, color: [f32; 4], depth: f32) {
    solids.push(UiSolid {
        x: rect.x,
        y: rect.y,
        w: rect.w,
        h: rect.h,
        color,
        depth,
    });
}

fn push_ui_bevel(solids: &mut Vec<UiSolid>, rect: FloatRect, line: f32, pressed: bool, depth: f32) {
    let (top_left, bottom_right) = if pressed {
        ([0.10, 0.10, 0.10, 1.0], [0.82, 0.82, 0.82, 1.0])
    } else {
        ([0.86, 0.86, 0.86, 1.0], [0.22, 0.22, 0.22, 1.0])
    };
    push_ui_rect(
        solids,
        FloatRect {
            x: rect.x,
            y: rect.y,
            w: rect.w,
            h: line,
        },
        top_left,
        depth,
    );
    push_ui_rect(
        solids,
        FloatRect {
            x: rect.x,
            y: rect.y,
            w: line,
            h: rect.h,
        },
        top_left,
        depth,
    );
    push_ui_rect(
        solids,
        FloatRect {
            x: rect.x,
            y: rect.y + rect.h - line,
            w: rect.w,
            h: line,
        },
        bottom_right,
        depth,
    );
    push_ui_rect(
        solids,
        FloatRect {
            x: rect.x + rect.w - line,
            y: rect.y,
            w: line,
            h: rect.h,
        },
        bottom_right,
        depth,
    );
}

fn dirty_tiles_repaint_traffic(
    city: &City,
    fidelity: EngineFidelity,
    dirty: &HashSet<(usize, usize)>,
) -> bool {
    if dirty.is_empty() {
        return false;
    }
    let rotation = city.rotation();
    let mac_engine = matches!(fidelity, EngineFidelity::Mac68k);
    dirty.iter().any(|&(x, y)| {
        if x >= CITY_WIDTH || y >= CITY_HEIGHT {
            return false;
        }
        let building = rotated_building_id(city.building_at(x, y), rotation);
        (has_decorative_traffic(building) || (mac_engine && mac_export80_stamp_building(building)))
            && mac_static_traffic_has_road_surface(building)
    })
}

fn mac_static_traffic_has_road_surface(rotated_building: u8) -> bool {
    mac_export80_ordinary_road_building(rotated_building)
        || mac_export80_special_stamp_building(rotated_building)
        || xbld_sprite_info(rotated_building, 0)
            .is_some_and(|info| matches!(info.layer_depth, DEPTH_ROAD | DEPTH_HIGHWAY))
}

fn xbld_tile_size(building: u8) -> u8 {
    match building {
        97..=107 => 2,
        140..=173 => 2,
        174..=197 => 3,
        198 => 1,
        200 => 1,
        201..=207 => 4,
        208..=214 => 3,
        215..=218 => 4,
        235..=247 => 2,
        248..=250 => 3,
        251..=255 => 4,
        _ => 1,
    }
}

fn traffic_surface_layer_depth(building: u8) -> i32 {
    // Traffic belongs above its own road/deck, while keeping overhead power lines and
    // buildings in front. Elevated highway decks are drawn above the ordinary road layer.
    if is_highway_tile(building) {
        DEPTH_HIGHWAY
    } else {
        DEPTH_ROAD
    }
}

fn city_display_name(city: &City) -> String {
    match city.kind {
        CityFileKind::WinScenario => format!("{} Scenario", city.name),
        CityFileKind::MacCity | CityFileKind::WinSc2 => city.name.clone(),
    }
}
