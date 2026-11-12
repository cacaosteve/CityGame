//! Tool selection, palette state, placement dispatch, drag completion and previews.
//!
//! Existing tool-call order, refusal/debit gates, held-input and release behavior,
//! and preview geometry are preserved. Native workers, traffic rendering, and
//! simulation scheduling remain in their existing modules.

use super::terrain_editing::{PE_RAISE_TOOL0_UI_TOOL_ID, pe_raise_tool0_ui_tile_xy_443003};
use super::{
    AppCommand, CityInfoDialog, DEPTH_BUILDING, DEPTH_ZONE, EDIT_MAP_COAST_MISC_OFFSET,
    EDIT_MAP_RIVER_MISC_OFFSET, GameCursor, HIGHWAY_COST_PER_SEGMENT, HighwayNeighborLinkCandidate,
    MAX_ZOOM, MIN_ZOOM, NetworkKind, PE_447490_UI_WAIT_4012C1_PER_CALL,
    PE_SPREAD_TOOL4_UI_TIMER_INIT, PlacementSound, RailNeighborLinkCandidate,
    RoadNeighborLinkCandidate, UndergroundKind, ViewerState, arcology_choice_for_tool,
    format_number, mac_terrain_editing, network_drag_path, network_kind_for_tool,
    pe_lower_terrain_from_packed_443900, pe_place_tile_441f00,
    pe_spread_terrain_from_packed_442a30_with_crt, pe_terrain_nesw_land_copy_442a30_with_crt,
    placeable_building_for_tool, power_plant_choice_for_tool, push_command, push_sprite,
    reward_mask_for_tool, sprite_depth, underground_kind_for_tool, zone_id_for_tool,
};
use crate::city::{AppMode, Technology};
use crate::engine_fidelity::EngineFidelity;
use crate::renderer::{RenderSprite, Renderer, UiSolid};
use crate::ui::{EditTool, UiAction};
use crate::win95_overlay::DispatchKind;
use crate::{mac_menu, ui};
use glam::Vec2;
use std::collections::HashSet;
use std::time::{Duration, Instant};

pub(super) const CITY_TOOL_MENUS: &[ui::ToolMenu] = &[
    ui::ToolMenu::Residential,
    ui::ToolMenu::Commercial,
    ui::ToolMenu::Industrial,
    ui::ToolMenu::Parks,
    ui::ToolMenu::Power,
    ui::ToolMenu::Civic,
    ui::ToolMenu::Education,
    ui::ToolMenu::Water,
    ui::ToolMenu::Terrain,
    ui::ToolMenu::TerrainEdit,
    ui::ToolMenu::Road,
    ui::ToolMenu::Rail,
    ui::ToolMenu::Transport,
    ui::ToolMenu::Emergency,
    ui::ToolMenu::Rewards,
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct ToolPlacement {
    pub(super) valid_tiles: Vec<(usize, usize)>,
    pub(super) invalid_tiles: Vec<(usize, usize)>,
    pub(super) preview: ToolPreview,
}

impl ToolPlacement {
    pub(super) fn valid(preview: ToolPreview, tiles: Vec<(usize, usize)>) -> Self {
        Self {
            valid_tiles: tiles,
            invalid_tiles: Vec::new(),
            preview,
        }
    }

    pub(super) fn is_empty(&self) -> bool {
        self.valid_tiles.is_empty() && self.invalid_tiles.is_empty()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ToolPreview {
    Bulldoze,
    Building(u8),
    Highway,
    Network(NetworkKind),
    Underground(UndergroundKind),
    Zone(u8),
    Terrain,
}

impl ToolPreview {
    pub(super) fn zone_sprite_id(self) -> Option<u32> {
        match self {
            Self::Zone(zone_id @ 1..=9) => Some(1290 + zone_id as u32),
            _ => None,
        }
    }

    pub(super) fn draws_valid_diamond(self) -> bool {
        self.zone_sprite_id().is_none()
            && !matches!(
                self,
                Self::Highway | Self::Building(_) | Self::Underground(_)
            )
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct DragReleaseOutcome {
    pub(super) placed_zones: usize,
    pub(super) network: Option<(NetworkKind, usize)>,
    pub(super) underground: Option<(UndergroundKind, usize)>,
    pub(super) highway_segments: usize,
    pub(super) highway_neighbor_link: Option<HighwayNeighborLinkCandidate>,
    pub(super) rail_neighbor_link: Option<RailNeighborLinkCandidate>,
    pub(super) road_neighbor_link: Option<RoadNeighborLinkCandidate>,
    pub(super) terrain_tiles: usize,
    pub(super) nature: Option<(EditTool, usize)>,
}

/// Tracks the palette button currently held (or just clicked) so the UI can draw it depressed.
#[derive(Debug, Clone, Copy)]
pub(super) struct PalettePress {
    pub(super) action: UiAction,
    pub(super) pressed_at: Instant,
    pub(super) released: bool,
}

/// Minimum time a clicked palette button stays visibly depressed, so fast clicks still flash.
pub(super) const PALETTE_PRESS_MIN: Duration = Duration::from_millis(110);

/// Tools that remain usable while the underground view is active (OpenSC2K city tool gates).
pub(super) fn tool_allowed_in_underground_view(tool: EditTool) -> bool {
    matches!(tool, EditTool::Query | EditTool::Bulldoze)
        || underground_kind_for_tool(tool).is_some()
}

pub(super) fn technology_for_tool(tool: EditTool) -> Option<Technology> {
    match tool {
        EditTool::Highway | EditTool::Onramp => Some(Technology::Highways),
        EditTool::BusDepot => Some(Technology::Buses),
        EditTool::Subway | EditTool::SubStation | EditTool::SubToRail => Some(Technology::Subways),
        EditTool::PowerPlantGas => Some(Technology::GasPower),
        EditTool::PowerPlantNuclear => Some(Technology::NuclearPower),
        EditTool::PowerPlantWind => Some(Technology::WindPower),
        EditTool::PowerPlantSolar => Some(Technology::SolarPower),
        EditTool::PowerPlantMicrowave => Some(Technology::MicrowavePower),
        EditTool::PowerPlantFusion => Some(Technology::FusionPower),
        EditTool::WaterTreatment => Some(Technology::WaterTreatment),
        EditTool::Desalinization => Some(Technology::Desalinization),
        EditTool::Airport => Some(Technology::Airport),
        EditTool::ArcologyPlymouth => Some(Technology::PlymouthArcology),
        EditTool::ArcologyForest => Some(Technology::ForestArcology),
        EditTool::ArcologyDarco => Some(Technology::DarcoArcology),
        EditTool::ArcologyLaunch => Some(Technology::LaunchArcology),
        _ => None,
    }
}

pub(super) fn one_click_placement_sound(tool: EditTool) -> Option<PlacementSound> {
    match tool {
        EditTool::Tunnel | EditTool::Onramp | EditTool::SubToRail => Some(PlacementSound::Build),
        EditTool::DispatchPolice => Some(PlacementSound::Police),
        EditTool::DispatchFirefighters => Some(PlacementSound::FireTruck),
        EditTool::DispatchMilitary => Some(PlacementSound::Military),
        EditTool::BusDepot => Some(PlacementSound::Horns),
        EditTool::RailDepot => Some(PlacementSound::Train),
        EditTool::SubStation => Some(PlacementSound::Build),
        EditTool::SmallPark | EditTool::BigPark | EditTool::Stadium | EditTool::Marina => {
            Some(PlacementSound::Cheers)
        }
        EditTool::Zoo => Some(PlacementSound::Monster),
        EditTool::PowerPlantCoal
        | EditTool::PowerPlantHydro
        | EditTool::PowerPlantOil
        | EditTool::PowerPlantGas
        | EditTool::PowerPlantNuclear
        | EditTool::PowerPlantWind
        | EditTool::PowerPlantSolar
        | EditTool::PowerPlantMicrowave
        | EditTool::PowerPlantFusion
        | EditTool::WaterPump
        | EditTool::WaterTower
        | EditTool::WaterTreatment
        | EditTool::Desalinization => Some(PlacementSound::Build),
        EditTool::Police | EditTool::Hospital => Some(PlacementSound::Police),
        EditTool::FireStation => Some(PlacementSound::FireTruck),
        EditTool::Prison => Some(PlacementSound::Prison),
        EditTool::School | EditTool::College | EditTool::Library | EditTool::Museum => {
            Some(PlacementSound::School)
        }
        EditTool::MayorHouse
        | EditTool::CityHall
        | EditTool::Statue
        | EditTool::BraunLlamaDome
        | EditTool::ArcologyPlymouth
        | EditTool::ArcologyForest
        | EditTool::ArcologyDarco
        | EditTool::ArcologyLaunch => Some(PlacementSound::Cheers),
        _ => None,
    }
}

pub(super) fn is_chooser_menu_tool(tool: EditTool) -> bool {
    matches!(tool, EditTool::PowerPlant | EditTool::Arcologies)
}

pub(super) fn is_dispatch_tool(tool: EditTool) -> bool {
    matches!(
        tool,
        EditTool::DispatchPolice | EditTool::DispatchFirefighters | EditTool::DispatchMilitary
    )
}

pub(super) fn dispatch_kind_for_tool(tool: EditTool) -> Option<DispatchKind> {
    match tool {
        EditTool::DispatchPolice => Some(DispatchKind::Police),
        EditTool::DispatchFirefighters => Some(DispatchKind::Firefighters),
        EditTool::DispatchMilitary => Some(DispatchKind::Military),
        _ => None,
    }
}

pub(super) fn tool_build_cost(tool: EditTool) -> Option<u32> {
    if let Some(choice) = power_plant_choice_for_tool(tool) {
        return Some(choice.cost);
    }
    if let Some(choice) = arcology_choice_for_tool(tool) {
        return Some(choice.cost);
    }

    match tool {
        EditTool::Bulldoze => Some(1),
        EditTool::RaiseTerrain
        | EditTool::LowerTerrain
        | EditTool::StretchTerrain
        | EditTool::LevelTerrain
        | EditTool::SpreadTerrain => Some(25),
        EditTool::Trees | EditTool::Forest => Some(3),
        EditTool::Water | EditTool::Stream => Some(100),
        EditTool::ZoneResidential
        | EditTool::ZoneCommercial
        | EditTool::ZoneIndustrial
        | EditTool::DeZone => Some(5),
        EditTool::ZoneResidentialDense
        | EditTool::ZoneCommercialDense
        | EditTool::ZoneIndustrialDense => Some(10),
        EditTool::Tunnel => Some(150),
        EditTool::Onramp => Some(25),
        EditTool::BusDepot => Some(250),
        EditTool::RailDepot => Some(500),
        EditTool::SubStation => Some(250),
        EditTool::SubToRail => Some(250),
        EditTool::SmallPark => Some(20),
        EditTool::BigPark => Some(150),
        EditTool::Zoo => Some(3_000),
        EditTool::Stadium => Some(5_000),
        EditTool::Marina => Some(1_000),
        EditTool::Police | EditTool::FireStation | EditTool::Hospital => Some(500),
        EditTool::Prison => Some(3_000),
        EditTool::School => Some(250),
        EditTool::College => Some(1_000),
        EditTool::Library => Some(500),
        EditTool::Museum => Some(1_000),
        EditTool::WaterPump => Some(100),
        EditTool::WaterTower => Some(250),
        EditTool::WaterTreatment => Some(500),
        EditTool::Desalinization => Some(1_000),
        EditTool::Seaport => Some(150),
        EditTool::Airport => Some(250),
        EditTool::MayorHouse | EditTool::CityHall | EditTool::Statue => Some(0),
        EditTool::BraunLlamaDome => Some(0),
        _ => None,
    }
}

pub(super) fn format_tool_cost_status(label: &str, cost: u32) -> String {
    format!("{label} ${}", format_number(cost))
}

pub(super) fn tool_allows_touch_paint_stroke(mode: AppMode, tool: EditTool) -> bool {
    if mode == AppMode::EditNewMap {
        return true;
    }
    zone_id_for_tool(tool).is_some()
        || network_kind_for_tool(tool).is_some()
        || underground_kind_for_tool(tool).is_some()
        || matches!(
            tool,
            EditTool::Bulldoze
                | EditTool::Highway
                | EditTool::RaiseTerrain
                | EditTool::LowerTerrain
                | EditTool::LevelTerrain
                | EditTool::StretchTerrain
                | EditTool::SpreadTerrain
                | EditTool::Trees
                | EditTool::Water
                | EditTool::Forest
                | EditTool::DeZone
        )
}

impl ViewerState {
    pub(super) fn open_tool_menu(&mut self, menu: ui::ToolMenu) {
        if !self.ui_action_enabled(UiAction::OpenMenu(menu)) {
            return;
        }
        self.close_modal_dialogs();
        self.cancel_zone_drag();
        self.cancel_network_drag();
        self.cancel_underground_drag();
        self.cancel_highway_drag();
        self.cancel_terrain_drag();
        self.cancel_edit_map_stretch_drag();
        self.cancel_edit_map_level_drag();
        self.cancel_edit_map_height_hold();
        self.cancel_edit_map_spread_hold();
        self.cancel_nature_drag();
        if menu == ui::ToolMenu::Emergency {
            self.city
                .reset_emergency_dispatch_for_engine(self.engine_fidelity);
        }
        if let Some(item) = ui::tool_menu_items(menu).iter().find(|item| {
            !is_chooser_menu_tool(item.tool)
                && self.ui_action_enabled(UiAction::SelectTool(item.tool))
        }) {
            self.apply_selected_tool(item.tool);
            self.query_is_inspect = false;
            self.refresh_status_line();
            log::info!("tool: {}", item.tool.label());
        }
        self.open_tool_menu = Some(menu);
    }

    pub(super) fn close_tool_menu(&mut self) {
        self.open_tool_menu = None;
    }

    pub(super) fn press_palette_button(&mut self, action: UiAction) {
        self.palette_press = Some(PalettePress {
            action,
            pressed_at: Instant::now(),
            released: false,
        });
    }

    pub(super) fn release_palette_button(&mut self) {
        if let Some(press) = self.palette_press.as_mut() {
            press.released = true;
        }
    }

    /// The palette button to draw depressed this frame: while held, or briefly after a fast click.
    pub(super) fn pressed_palette_action(&mut self) -> Option<UiAction> {
        let press = self.palette_press?;
        if press.released && press.pressed_at.elapsed() >= PALETTE_PRESS_MIN {
            self.palette_press = None;
            return None;
        }
        Some(press.action)
    }

    pub(super) fn disabled_palette_actions(&self) -> Vec<UiAction> {
        let mut disabled = Vec::new();
        if !self.city.has_active_disaster() {
            disabled.push(UiAction::OpenMenu(ui::ToolMenu::Emergency));
        }
        if self.active_reward_mask() == 0 {
            disabled.push(UiAction::OpenMenu(ui::ToolMenu::Rewards));
        }
        for menu in CITY_TOOL_MENUS {
            for item in ui::tool_menu_items(*menu) {
                let action = UiAction::SelectTool(item.tool);
                if !self.ui_action_enabled(action) {
                    disabled.push(action);
                }
            }
        }
        if self.zoom <= MIN_ZOOM + f32::EPSILON {
            disabled.push(UiAction::ZoomOut);
        }
        if self.zoom >= MAX_ZOOM - f32::EPSILON {
            disabled.push(UiAction::ZoomIn);
        }
        disabled
    }

    pub(super) fn checked_palette_actions(&self) -> Vec<UiAction> {
        let mut checked = Vec::with_capacity(8);
        if self.mode == AppMode::EditNewMap {
            if self.edit_map_option_enabled(EDIT_MAP_COAST_MISC_OFFSET) {
                checked.push(UiAction::ToggleEditCoast);
            }
            if self.edit_map_option_enabled(EDIT_MAP_RIVER_MISC_OFFSET) {
                checked.push(UiAction::ToggleEditRiver);
            }
        }
        if self.show_buildings {
            checked.push(UiAction::ToggleBuildings);
        }
        if self.show_signs {
            checked.push(UiAction::ToggleSigns);
        }
        if self.show_infrastructure {
            checked.push(UiAction::ToggleInfrastructure);
        }
        if self.show_zones {
            checked.push(UiAction::ToggleZones);
        }
        if self.show_underground {
            checked.push(UiAction::ToggleUnderground);
        }
        if let Some(action) = self
            .city_info_dialog
            .and_then(CityInfoDialog::toolbar_action)
        {
            checked.push(action);
        }
        checked
    }

    pub(super) fn active_reward_mask(&self) -> u32 {
        if self.engine_fidelity.uses_win95() {
            self.city.rewards_available_mask()
        } else {
            self.city.mac_civic_latch() & 0x1f
        }
    }

    pub(super) fn ui_action_enabled(&self, action: UiAction) -> bool {
        match action {
            UiAction::ToggleEditCoast | UiAction::ToggleEditRiver | UiAction::GenerateEditMap => {
                self.mode == AppMode::EditNewMap
            }
            UiAction::RaiseEditSeaLevel | UiAction::LowerEditSeaLevel => {
                self.mode == AppMode::EditNewMap
            }
            UiAction::ZoomOut => self.zoom > MIN_ZOOM + f32::EPSILON,
            UiAction::ZoomIn => self.zoom < MAX_ZOOM - f32::EPSILON,
            UiAction::OpenMenu(ui::ToolMenu::Emergency) => self.city.has_active_disaster(),
            UiAction::OpenMenu(ui::ToolMenu::Rewards) => self.active_reward_mask() != 0,
            UiAction::SelectTool(tool) if is_dispatch_tool(tool) => {
                self.city.has_active_disaster()
                    && dispatch_kind_for_tool(tool).is_some_and(|kind| {
                        let mac_military_option_unlocked = kind != DispatchKind::Military
                            || self.engine_fidelity.uses_win95()
                            // Mac CODE 8 export_240 enables command 2's
                            // military option only for MISC slot 915 values
                            // 2–4; the silo fallback (5) is excluded.
                            || self
                                .city
                                .mac_state_misc_u32_at(0x0e4c)
                                .is_some_and(|selector| (2..=4).contains(&(selector as u8)));
                        mac_military_option_unlocked
                            && self
                                .city
                                .dispatch_capacity_for_engine(kind, self.engine_fidelity)
                                > 0
                    })
            }
            UiAction::SelectTool(tool) => {
                reward_mask_for_tool(tool).is_none_or(|mask| self.active_reward_mask() & mask != 0)
                    && self.tool_technology_available(tool)
            }
            _ => true,
        }
    }

    pub(super) fn tool_technology_available(&self, tool: EditTool) -> bool {
        let Some(technology) = technology_for_tool(tool) else {
            return true;
        };
        let Some(discovery_year) = self.city.technology_discovery_year(technology) else {
            return true;
        };
        if discovery_year == 0 {
            return true;
        }

        if self.engine_fidelity == EngineFidelity::Mac68k {
            // Mac CODE 8 export_237 only clears the first due technology
            // timer on its monthly 1-in-8 gate. Reaching the saved year does
            // not itself unlock the command; export_240 rebuilds the native
            // command masks after that timer is cleared.
            return false;
        }

        u32::from(self.game_clock.year()) >= discovery_year
    }

    pub(super) fn chooser_tool_enabled(&self, tool: EditTool) -> bool {
        self.ui_action_enabled(UiAction::SelectTool(tool))
            && self.can_afford_tool_build_cost(tool, 1)
    }

    fn active_tool_cost_per_tile(&self) -> Option<u32> {
        if self.edit_map_sampled_height_tool()
            || (self.mode == AppMode::EditNewMap
                && matches!(self.tool, EditTool::StretchTerrain | EditTool::LevelTerrain))
        {
            return None;
        }
        if let Some(zone_id) = super::zone_id_for_tool(self.tool) {
            Some(self.zone_unit_cost(zone_id))
        } else {
            tool_build_cost(self.tool)
        }
    }

    pub(super) fn selected_tool_status_line(&self) -> Option<String> {
        if let Some(kind) = network_kind_for_tool(self.tool) {
            let cost = kind.cost_per_tile()?;
            return Some(format_tool_cost_status(kind.label(), cost));
        }
        if let Some(kind) = underground_kind_for_tool(self.tool) {
            return Some(format_tool_cost_status(kind.label(), kind.cost_per_tile()));
        }
        if self.tool == EditTool::Highway {
            return Some(format_tool_cost_status("Highway", HIGHWAY_COST_PER_SEGMENT));
        }
        let cost = self.active_tool_cost_per_tile()?;
        Some(format_tool_cost_status(self.tool.label(), cost))
    }

    pub(super) fn cursor_kind(&self, panning: bool) -> GameCursor {
        if panning {
            GameCursor::Pan
        } else {
            GameCursor::for_tool(self.tool)
        }
    }

    pub(super) fn select_tool_state(&mut self, tool: EditTool) {
        self.cancel_zone_drag();
        self.cancel_network_drag();
        self.cancel_underground_drag();
        self.cancel_highway_drag();
        self.cancel_terrain_drag();
        self.cancel_edit_map_stretch_drag();
        self.cancel_edit_map_level_drag();
        self.cancel_edit_map_height_hold();
        self.cancel_edit_map_spread_hold();
        self.cancel_nature_drag();
        self.close_tool_menu();
        self.close_modal_dialogs();
        self.apply_selected_tool(tool);
        if self.mode == AppMode::CityView {
            self.query_is_inspect = false;
            self.refresh_status_line();
        }
    }

    pub(super) fn set_tool_and_log(&mut self, tool: EditTool) {
        self.select_tool_state(tool);
        log::info!("tool: {}", tool.label());
    }

    pub(super) fn set_tool(
        &mut self,
        video: &sdl3::VideoSubsystem,
        window_id: u32,
        tool: EditTool,
    ) {
        self.set_tool_and_log(tool);
        self.update_window_title(video, window_id);
    }

    pub(super) fn apply_selected_tool(&mut self, tool: EditTool) {
        self.tool = tool;
        if self.mode != AppMode::CityView {
            return;
        }
        // OpenSC2K: underground view stays for pipes/subway/query/demolish; other tools
        // leave the view. Pipe/subway selection always enters underground.
        let want_underground = if underground_kind_for_tool(tool).is_some() {
            true
        } else if tool_allowed_in_underground_view(tool) {
            self.show_underground
        } else {
            false
        };
        self.set_underground_view(want_underground);
    }

    pub(super) fn toggle_underground_view(&mut self) {
        if self.mode != AppMode::CityView {
            return;
        }
        self.set_underground_view(!self.show_underground);
        log::info!(
            "underground view {}",
            if self.show_underground { "on" } else { "off" }
        );
    }

    pub(super) fn set_underground_view(&mut self, enabled: bool) {
        if self.show_underground == enabled {
            self.menu_checks.show_underground = enabled;
            mac_menu::set_underground_view(enabled);
            return;
        }
        self.show_underground = enabled;
        self.menu_checks.show_underground = enabled;
        mac_menu::set_underground_view(enabled);
        if self.show_underground {
            self.show_traffic = false;
        }
        self.invalidate_sprite_cache();
        self.refresh_idle_status_line();
    }

    pub(super) fn handle_ui_action(
        &mut self,
        action: UiAction,
        video: &sdl3::VideoSubsystem,
        window_id: u32,
    ) {
        if self.handle_ui_action_state(action) {
            self.update_window_title(video, window_id);
        }
    }

    pub(super) fn handle_ui_action_state(&mut self, action: UiAction) -> bool {
        if !self.ui_action_enabled(action) {
            return false;
        }
        if let Some(dialog) = CityInfoDialog::for_action(action) {
            self.open_city_info_dialog(dialog);
            return true;
        }
        match action {
            UiAction::SelectTool(EditTool::PowerPlant) => {
                self.open_power_plant_dialog();
                true
            }
            UiAction::SelectTool(EditTool::Arcologies) => {
                self.open_arcology_dialog();
                true
            }
            UiAction::SelectTool(tool) => {
                self.set_tool_and_log(tool);
                true
            }
            UiAction::ToggleEditCoast => {
                self.toggle_edit_map_option(EDIT_MAP_COAST_MISC_OFFSET);
                true
            }
            UiAction::ToggleEditRiver => {
                self.toggle_edit_map_option(EDIT_MAP_RIVER_MISC_OFFSET);
                true
            }
            UiAction::GenerateEditMap => {
                self.generate_edit_map();
                true
            }
            UiAction::RaiseEditSeaLevel => {
                self.adjust_edit_map_sea_level(1);
                true
            }
            UiAction::LowerEditSeaLevel => {
                self.adjust_edit_map_sea_level(-1);
                true
            }
            UiAction::DoneEditMap => {
                self.finish_edit_map_state();
                true
            }
            UiAction::RotateViewCcw => {
                self.rotate_view(-1);
                false
            }
            UiAction::RotateViewCw => {
                self.rotate_view(1);
                false
            }
            UiAction::ZoomOut => {
                push_command(AppCommand::ZoomOut);
                false
            }
            UiAction::ZoomIn => {
                push_command(AppCommand::ZoomIn);
                false
            }
            UiAction::CenterMap => {
                push_command(AppCommand::CenterMap);
                false
            }
            UiAction::ToggleBuildings
            | UiAction::ToggleSigns
            | UiAction::ToggleInfrastructure
            | UiAction::ToggleZones => {
                self.toggle_display_layer(action);
                false
            }
            UiAction::ToggleUnderground => {
                self.toggle_underground_view();
                false
            }
            UiAction::OpenMenu(menu) => {
                self.open_tool_menu(menu);
                true
            }
            UiAction::OpenCityMap
            | UiAction::OpenCityPopulation
            | UiAction::OpenCityNeighbors
            | UiAction::OpenCityGraphs
            | UiAction::OpenCityIndustry
            | UiAction::OpenBudget
            | UiAction::OpenHelp => unreachable!("handled above"),
        }
    }

    pub(super) fn cancel_active_drag(&mut self) -> bool {
        let had_active_drag = self.zone_drag_active()
            || self.network_drag_active()
            || self.underground_drag_active()
            || self.highway_drag_active()
            || self.terrain_drag_active()
            || self.edit_map_stretch_drag_active()
            || self.edit_map_level_drag_active()
            || self.edit_map_height_hold.is_some()
            || self.nature_drag_active();
        self.cancel_zone_drag();
        self.cancel_network_drag();
        self.cancel_underground_drag();
        self.cancel_highway_drag();
        self.cancel_terrain_drag();
        self.cancel_edit_map_stretch_drag();
        self.cancel_edit_map_level_drag();
        self.cancel_edit_map_height_hold();
        self.cancel_edit_map_spread_hold();
        self.cancel_nature_drag();
        had_active_drag
    }

    pub(super) fn release_left_mouse_drag(&mut self, cancel_drag: bool) -> DragReleaseOutcome {
        let mut outcome = DragReleaseOutcome::default();
        let height_changed = self.edit_map_height_hold.take() == Some(true);

        if cancel_drag {
            self.cancel_zone_drag();
            self.cancel_network_drag();
            self.cancel_underground_drag();
            self.cancel_highway_drag();
            self.cancel_terrain_drag();
            self.cancel_edit_map_stretch_drag();
            self.cancel_edit_map_level_drag();
            self.cancel_edit_map_spread_hold();
            outcome.nature = self.finish_nature_drag();
            return outcome;
        }

        if height_changed
            && self.tool == EditTool::LowerTerrain
            && self.engine_fidelity.uses_win95()
        {
            outcome.terrain_tiles = 1;
        }
        if self.zone_drag_active() {
            outcome.placed_zones = self.finish_zone_drag();
        } else if let Some(network) = self.finish_network_drag() {
            outcome.network = Some((network.0, network.1));
            outcome.rail_neighbor_link = network.2;
            outcome.road_neighbor_link = network.3;
        } else if let Some(underground) = self.finish_underground_drag() {
            outcome.underground = Some(underground);
        } else if self.highway_drag_active() {
            let (segments, neighbor_link) = self.finish_highway_drag();
            outcome.highway_segments = segments;
            outcome.highway_neighbor_link = neighbor_link;
        } else if self.terrain_drag_active() {
            outcome.terrain_tiles = self.finish_terrain_drag();
        } else if self.edit_map_stretch_drag_active() {
            outcome.terrain_tiles = self.finish_edit_map_stretch_drag();
        } else if self.edit_map_level_drag_active() {
            outcome.terrain_tiles = self.finish_edit_map_level_drag();
        } else if self.nature_drag_active() {
            outcome.nature = self.finish_nature_drag();
        }

        outcome
    }

    pub(super) fn can_afford_build_cost(&self, cost: u32) -> bool {
        let cost = i32::try_from(cost).unwrap_or(i32::MAX);
        cost == 0 || self.city.city_funds() >= cost
    }

    pub(super) fn affordable_build_units(&self, per_unit_cost: u32) -> usize {
        if per_unit_cost == 0 {
            return usize::MAX;
        }
        let Ok(funds) = u32::try_from(self.city.city_funds()) else {
            return 0;
        };
        usize::try_from(funds / per_unit_cost).unwrap_or(usize::MAX)
    }

    pub(super) fn tool_build_total_cost(tool: EditTool, units: u32) -> Option<u32> {
        tool_build_cost(tool).map(|per_unit| per_unit.saturating_mul(units))
    }

    pub(super) fn can_afford_tool_build_cost(&self, tool: EditTool, units: u32) -> bool {
        Self::tool_build_total_cost(tool, units).is_none_or(|cost| self.can_afford_build_cost(cost))
    }

    pub(super) fn debit_build_cost(&mut self, cost: u32) {
        if cost > 0 {
            self.city.debit_city_funds(cost);
        }
    }

    pub(super) fn debit_tool_build_cost(&mut self, tool: EditTool, units: u32) {
        if let Some(cost) = Self::tool_build_total_cost(tool, units) {
            self.debit_build_cost(cost);
        }
    }

    pub(super) fn apply_tool_at(
        &mut self,
        screen: Vec2,
        window_size: (u32, u32),
        pixel_scale: f32,
    ) -> bool {
        self.apply_tool_at_with_shift(screen, window_size, pixel_scale, false)
    }

    pub(super) fn apply_tool_at_with_shift(
        &mut self,
        screen: Vec2,
        window_size: (u32, u32),
        pixel_scale: f32,
        shift_down: bool,
    ) -> bool {
        let Some((x, y)) = self.tile_at_screen(screen, window_size, pixel_scale) else {
            return false;
        };
        self.apply_tool_at_tile_with_shift(x, y, shift_down)
    }

    pub(super) fn paint_city_tool_drag_to(
        &mut self,
        previous_tile: Option<(usize, usize)>,
        tile: (usize, usize),
    ) -> (usize, Option<(usize, usize)>) {
        if self.mode != AppMode::CityView {
            return (0, previous_tile);
        }
        let fill_path = self.tool == EditTool::Bulldoze;
        let path = if fill_path {
            previous_tile.map_or_else(|| vec![tile], |previous| network_drag_path(previous, tile))
        } else {
            vec![tile]
        };

        let mut applied = 0;
        for (index, (x, y)) in path.into_iter().enumerate() {
            if fill_path && previous_tile.is_some() && index == 0 {
                continue;
            }
            if self.apply_tool_at_tile(x, y) {
                applied += 1;
            }
        }
        (applied, Some(tile))
    }

    pub(super) fn paint_edit_map_tool_drag_to(
        &mut self,
        previous_tile: Option<(usize, usize)>,
        tile: (usize, usize),
        shift_down: bool,
    ) -> (usize, Option<(usize, usize)>) {
        if self.mode != AppMode::EditNewMap
            || matches!(self.tool, EditTool::StretchTerrain | EditTool::LevelTerrain)
        {
            return (0, previous_tile);
        }
        if self.edit_map_sampled_height_tool() {
            // Motion updates the target. The press and held frame loop sample it.
            return (0, Some(tile));
        }
        // PE Spread tool-4 hold @ `0x442b07`/`0x442b31`: unpack `0x444bf0` then same-tile gate.
        if self.tool == EditTool::SpreadTerrain {
            if previous_tile == Some(tile) {
                return (0, previous_tile);
            }
            let (px, py) = previous_tile.unwrap_or(tile);
            self.pe_spread_tool4_ui_timer = PE_SPREAD_TOOL4_UI_TIMER_INIT;
            let packed = ((tile.1 as u16) << 8 | (tile.0 as u16 & 0xff)) as i16;
            let (neighbor_447490, waits_445b50) = pe_spread_terrain_from_packed_442a30_with_crt(
                &mut self.city,
                Some(&mut self.pe_process_crt),
                packed,
                px as i16,
                py as i16,
            );
            let applied = usize::from(neighbor_447490 != 0);
            if applied != 0 {
                // PE @ `0x442c7e`/`0x442c83`: store current tile as next prev words.
                self.pe_spread_prev_tile_x = tile.0 as i16;
                self.pe_spread_prev_tile_y = tile.1 as i16;
                // PE tool-4 hold @ `0x442c44`/`0x442c4c`/`0x442c57`/`0x442c5f`: refresh UI.
                let (ux, uy) = pe_raise_tool0_ui_tile_xy_443003(tile.0, tile.1);
                self.pe_raise_ui_tile_x = ux;
                self.pe_raise_ui_tile_y = uy;
                self.pe_raise_ui_press_y_byte = uy;
                self.pe_raise_xter_ui_byte = 0;
                // PE tool-4 hold @ `0x442bd3`/`0x442c63`: `0x401758(0)`.
                self.pe_spread_call_ui_cursor_4519e0(0);
                self.pe_spread_call_ui_cursor_4519e0(0);
                // PE tool-4 hold @ `0x442c1c`/`0x442c71`: `0x4012c1`.
                self.pe_spread_ui_wait_4012c1_count =
                    self.pe_spread_ui_wait_4012c1_count.wrapping_add(2);
                // PE NESW loop @ `0x442bb3`→`0x445b50`: adjust-path `0x4012c1` waits.
                self.pe_spread_ui_wait_4012c1_count = self
                    .pe_spread_ui_wait_4012c1_count
                    .wrapping_add(waits_445b50);
                // PE NESW loop @ `0x442bbd`→`0x447490`: ×2 waits per in-bounds neighbor.
                self.pe_spread_ui_wait_4012c1_count = self
                    .pe_spread_ui_wait_4012c1_count
                    .wrapping_add(neighbor_447490.wrapping_mul(PE_447490_UI_WAIT_4012C1_PER_CALL));
                // PE tool-4 hold @ `0x442c8c`: UI `0x40226b` after stamp.
                self.pe_spread_hold_ui_refresh_count =
                    self.pe_spread_hold_ui_refresh_count.wrapping_add(1);
            }
            return (applied, Some(tile));
        }
        let path =
            previous_tile.map_or_else(|| vec![tile], |previous| network_drag_path(previous, tile));
        let mut applied = 0;
        for (index, (x, y)) in path.into_iter().enumerate() {
            if previous_tile.is_some() && index == 0 {
                continue;
            }
            if self.apply_tool_at_tile_with_shift(x, y, shift_down) {
                applied += 1;
            }
        }
        (applied, Some(tile))
    }

    pub(super) fn apply_tool_at_tile(&mut self, x: usize, y: usize) -> bool {
        self.apply_tool_at_tile_with_shift(x, y, false)
    }

    pub(super) fn apply_tool_at_tile_with_shift(
        &mut self,
        x: usize,
        y: usize,
        shift_down: bool,
    ) -> bool {
        if self.mode == AppMode::CityView
            && !self.ui_action_enabled(UiAction::SelectTool(self.tool))
        {
            return false;
        }
        match (self.mode, self.tool) {
            (AppMode::EditNewMap, EditTool::Bulldoze) => {
                self.city.set_terrain(x, y, 0);
                // PE PlaceTile (`0x441f00`): clear XBLD; leave watered/piped/XTRF.
                pe_place_tile_441f00(&mut self.city, x, y, 0);
                self.city.set_zone(x, y, 0);
                true
            }
            (AppMode::EditNewMap, EditTool::RaiseTerrain) => {
                if self.engine_fidelity == EngineFidelity::Mac68k {
                    return mac_terrain_editing::sample_height_step(
                        &mut self.city,
                        x,
                        y,
                        true,
                        &mut self.mac_process_rng,
                    );
                }
                // Editor selector 1 -> 0x443a50. Its held loop samples near
                // the pointer using the live CRT and leaves city funds alone.
                super::terrain_editing::pe_stretch_terrain_hill_step_443a50_with_crt(
                    &mut self.city,
                    Some(&mut self.pe_process_crt),
                    x,
                    y,
                )
                .is_some()
            }
            (AppMode::EditNewMap, EditTool::LowerTerrain) => {
                if self.engine_fidelity == EngineFidelity::Mac68k {
                    return mac_terrain_editing::sample_height_step(
                        &mut self.city,
                        x,
                        y,
                        false,
                        &mut self.mac_process_rng,
                    );
                }
                // PE LowerTerrain `0x401ea1`→`0x443900`: unpack `0x444bf0` then CRT jitter dig.
                let packed = ((y as u16) << 8 | (x as u16 & 0xff)) as i16;
                let Some(changed) = pe_lower_terrain_from_packed_443900(
                    &mut self.city,
                    &mut self.pe_process_crt,
                    packed,
                ) else {
                    return false;
                };
                // The native loop redraws after every in-bounds worker call.
                self.pe_lower_hold_ui_refresh_count =
                    self.pe_lower_hold_ui_refresh_count.wrapping_add(1);
                self.pe_lower_hold_blit_count = self.pe_lower_hold_blit_count.wrapping_add(1);
                changed
            }
            // Stretch changes height only after vertical pointer motion.
            (AppMode::EditNewMap, EditTool::StretchTerrain) => false,
            (AppMode::EditNewMap, EditTool::LevelTerrain) => {
                self.begin_edit_map_level_drag_at(x, y) && self.finish_edit_map_level_drag() > 0
            }
            (AppMode::EditNewMap, EditTool::SpreadTerrain) => {
                // PE terrain tool-4 @ `0x442a88`: `[0x4cad4c] = 0xfffffc18`.
                self.pe_spread_tool4_ui_timer = PE_SPREAD_TOOL4_UI_TIMER_INIT;
                // PE terrain tool-4 @ `0x442a5f`/`0x442a7d`: `0x401e7e(0x1fc, -1)`.
                self.pe_spread_hold_cursor_id = 0x1fc;
                self.pe_spread_hold_cursor_arg = -1;
                // PE terrain tool-4 @ `0x442a92`: MapTool UI `[0x4ca434]+0` = 4.
                self.pe_raise_ui_tool_id = PE_RAISE_TOOL0_UI_TOOL_ID;
                // PE terrain tool-4 @ `0x442a9b`–`0x442ad1`: zero UI +1/+5/+6/+7/+8/+9.
                self.pe_raise_ui_press_y_byte = 0;
                self.pe_raise_xter_ui_byte = 0;
                self.pe_raise_ui_byte6 = 0;
                self.pe_raise_ui_byte7 = 0;
                self.pe_raise_ui_byte8 = 0;
                self.pe_raise_ui_byte9 = 0;
                // PE terrain tool-4 `0x402b94`→`0x442a30`.
                let (neighbor_447490, waits_445b50) = pe_terrain_nesw_land_copy_442a30_with_crt(
                    &mut self.city,
                    Some(&mut self.pe_process_crt),
                    x,
                    y,
                );
                // PE tool-4 @ `0x442c33`: stamp XTXT `0xc9` on the current tile.
                self.city.set_text_overlay(x, y, 0xc9);
                // PE tool-4 @ `0x442c4c`/`0x442c57`: UI tile X/Y at `+3`/`+4`.
                let (ux, uy) = pe_raise_tool0_ui_tile_xy_443003(x, y);
                self.pe_raise_ui_tile_x = ux;
                self.pe_raise_ui_tile_y = uy;
                // PE tool-4 @ `0x442c44`: UI `+1` = current Y.
                self.pe_raise_ui_press_y_byte = uy;
                // PE tool-4 @ `0x442c5f`: UI `+5` = 0 after stamp.
                self.pe_raise_xter_ui_byte = 0;
                // PE tool-4 @ `0x442c7e`/`0x442c83`: store current tile X/Y as prev words.
                self.pe_spread_prev_tile_x = x as i16;
                self.pe_spread_prev_tile_y = y as i16;
                // PE tool-4 @ `0x442bd3`/`0x442c63`: `0x401758(0)` UI-cursor helper.
                self.pe_spread_call_ui_cursor_4519e0(0);
                self.pe_spread_call_ui_cursor_4519e0(0);
                // PE tool-4 @ `0x442c1c`/`0x442c71`: `0x4012c1` UI wait.
                self.pe_spread_ui_wait_4012c1_count =
                    self.pe_spread_ui_wait_4012c1_count.wrapping_add(2);
                // PE NESW loop @ `0x442bb3`→`0x445b50`: adjust-path `0x4012c1` waits.
                self.pe_spread_ui_wait_4012c1_count = self
                    .pe_spread_ui_wait_4012c1_count
                    .wrapping_add(waits_445b50);
                // PE NESW loop @ `0x442bbd`→`0x447490`: ×2 waits per in-bounds neighbor.
                self.pe_spread_ui_wait_4012c1_count = self
                    .pe_spread_ui_wait_4012c1_count
                    .wrapping_add(neighbor_447490.wrapping_mul(PE_447490_UI_WAIT_4012C1_PER_CALL));
                // PE tool-4 @ `0x442c8c`: UI `0x40226b` after stamp.
                self.pe_spread_hold_ui_refresh_count =
                    self.pe_spread_hold_ui_refresh_count.wrapping_add(1);
                true
            }
            (AppMode::EditNewMap, EditTool::Water) => {
                self.place_edit_map_water_at(x, y);
                true
            }
            (AppMode::EditNewMap, EditTool::Stream) => {
                self.place_edit_map_stream_at(x, y);
                true
            }
            (AppMode::EditNewMap, EditTool::Trees) => {
                if shift_down {
                    self.remove_edit_map_tree_at(x, y);
                } else {
                    self.place_edit_map_tree_at(x, y, x + y);
                }
                true
            }
            (AppMode::EditNewMap, EditTool::Forest) => {
                if shift_down {
                    self.remove_edit_map_forest_at(x, y);
                } else {
                    self.place_edit_map_forest_at(x, y);
                }
                true
            }
            (AppMode::CityView, EditTool::Bulldoze) => {
                let clearable_tiles = self.bulldoze_clearable_tiles(x, y);
                let billable_tiles = self.bulldoze_billable_tiles(x, y);
                let billable_units = self.bulldoze_cost_units_for_click(x, y, billable_tiles.len());
                if clearable_tiles.is_empty()
                    || (self.engine_fidelity.uses_win95() && billable_units == 0)
                    || !self.can_afford_tool_build_cost(EditTool::Bulldoze, billable_units)
                {
                    return false;
                }
                if self.show_underground {
                    self.bulldoze_underground_at(x, y);
                } else {
                    self.bulldoze_at(x, y);
                }
                true
            }
            (
                AppMode::CityView,
                EditTool::RaiseTerrain
                | EditTool::LowerTerrain
                | EditTool::LevelTerrain
                | EditTool::StretchTerrain
                | EditTool::SpreadTerrain,
            ) => self.place_city_terrain_edit_at(x, y),
            (AppMode::CityView, EditTool::Sign) => self.place_sign_at(x, y),
            (AppMode::CityView, EditTool::Road | EditTool::Rail | EditTool::Power) => {
                if let Some(kind) = network_kind_for_tool(self.tool) {
                    self.place_network_at(x, y, kind) > 0
                } else {
                    false
                }
            }
            (AppMode::CityView, tool) if zone_id_for_tool(tool).is_some() => {
                if let Some(zone_id) = zone_id_for_tool(tool) {
                    self.place_zone_at(x, y, zone_id) > 0
                } else {
                    false
                }
            }
            (AppMode::CityView, EditTool::Water) => self.place_surface_water_at(x, y) > 0,
            (AppMode::CityView, EditTool::Trees) => {
                self.place_tree_at_with_shift(x, y, shift_down) > 0
            }
            (AppMode::CityView, EditTool::Forest) => {
                self.place_forest_at_with_shift(x, y, shift_down) > 0
            }
            (AppMode::CityView, EditTool::Pipes | EditTool::Subway) => {
                if let Some(kind) = underground_kind_for_tool(self.tool) {
                    self.place_underground_at(x, y, kind) > 0
                } else {
                    false
                }
            }
            (AppMode::CityView, tool) if dispatch_kind_for_tool(tool).is_some() => {
                let kind = dispatch_kind_for_tool(tool).expect("dispatch tool");
                let placed =
                    self.city
                        .place_dispatch_actor_for_engine(x, y, kind, self.engine_fidelity);
                if placed {
                    self.query_is_inspect = false;
                    self.refresh_status_line();
                }
                placed
            }
            (AppMode::CityView, EditTool::Onramp) => self.place_onramp_at(x, y),
            (AppMode::CityView, EditTool::SubToRail) => self.place_sub_to_rail_at(x, y),
            (AppMode::CityView, EditTool::Tunnel) => self.place_tunnel_at(x, y) > 0,
            (AppMode::CityView, EditTool::PowerPlantHydro) => self.place_hydro_power_at(x, y) > 0,
            (AppMode::CityView, tool) if placeable_building_for_tool(tool).is_some() => {
                if let Some(building) = placeable_building_for_tool(tool) {
                    let Some((origin_x, origin_y)) = self.player_building_origin(x, y, building)
                    else {
                        return false;
                    };
                    let placed = self.place_building_at(origin_x, origin_y, building) > 0;
                    if placed {
                        self.finish_reward_placement(tool);
                    }
                    placed
                } else {
                    false
                }
            }
            _ => false,
        }
    }

    pub(super) fn push_tool_preview(
        &self,
        renderer: &Renderer<'_>,
        window_size: (u32, u32),
        pixel_scale: f32,
        sprites: &mut Vec<RenderSprite>,
    ) {
        let plan = if let Some(drag) = self.zone_drag {
            self.zone_placement_for_rect(drag.start, drag.current, drag.zone_id)
        } else if let Some(drag) = self.network_drag {
            let path = self.network_drag_preview_path(drag);
            self.network_drag_placement_for_path(&path, drag.kind)
        } else if let Some(drag) = self.underground_drag {
            let path =
                self.underground_drag_path_for_kind(drag.start, drag.current, drag.kind, true);
            self.underground_drag_placement_for_path(&path, drag.kind)
        } else if let Some(drag) = self.highway_drag {
            let plan = self.highway_drag_plan_for_city(drag.start, drag.current);
            self.highway_drag_placement_for_plan(&plan)
        } else {
            let Some(tile) = self.hover_tile else {
                return;
            };
            let Some(plan) = self.tool_placement_at(tile.0, tile.1) else {
                return;
            };
            plan
        };
        if plan.is_empty() {
            return;
        }

        if plan.preview.draws_valid_diamond() {
            for &(x, y) in &plan.valid_tiles {
                self.push_preview_diamond(
                    renderer,
                    window_size,
                    pixel_scale,
                    sprites,
                    x,
                    y,
                    [0.35, 0.95, 1.0, 0.45],
                );
            }
        }
        for &(x, y) in &plan.invalid_tiles {
            self.push_preview_diamond(
                renderer,
                window_size,
                pixel_scale,
                sprites,
                x,
                y,
                [1.0, 0.15, 0.08, 0.55],
            );
        }

        match plan.preview {
            ToolPreview::Building(_) => {
                for &(x, y) in &plan.valid_tiles {
                    self.push_preview_diamond(
                        renderer,
                        window_size,
                        pixel_scale,
                        sprites,
                        x,
                        y,
                        [0.05, 0.05, 0.05, 0.58],
                    );
                }
            }
            ToolPreview::Highway => {
                for &(x, y) in &plan.valid_tiles {
                    self.push_preview_diamond(
                        renderer,
                        window_size,
                        pixel_scale,
                        sprites,
                        x,
                        y,
                        [0.05, 0.05, 0.05, 0.58],
                    );
                }
            }
            ToolPreview::Network(kind) => {
                let virtual_tiles: HashSet<(usize, usize)> =
                    plan.valid_tiles.iter().copied().collect();
                for &(x, y) in &plan.valid_tiles {
                    self.push_network_preview(
                        renderer,
                        window_size,
                        pixel_scale,
                        sprites,
                        x,
                        y,
                        kind,
                        &virtual_tiles,
                    );
                }
            }
            ToolPreview::Underground(kind) => {
                let virtual_tiles: HashSet<(usize, usize)> =
                    plan.valid_tiles.iter().copied().collect();
                for &(x, y) in &plan.valid_tiles {
                    self.push_underground_preview(
                        renderer,
                        window_size,
                        pixel_scale,
                        sprites,
                        x,
                        y,
                        kind,
                        &virtual_tiles,
                    );
                }
            }
            ToolPreview::Zone(_) => {
                if let Some(sprite_id) = plan.preview.zone_sprite_id() {
                    for &(x, y) in &plan.valid_tiles {
                        let (anchor, zoom, sx, sy) =
                            self.structure_anchor(window_size, pixel_scale, x, y);
                        push_sprite(
                            renderer,
                            sprites,
                            sprite_id,
                            anchor,
                            zoom,
                            sprite_depth(sx, sy, DEPTH_ZONE, 48),
                            [1.0, 1.0, 1.0, 0.72],
                            self.animation_frame(),
                        );
                    }
                }
            }
            ToolPreview::Bulldoze | ToolPreview::Terrain => {}
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn push_preview_diamond(
        &self,
        renderer: &Renderer<'_>,
        window_size: (u32, u32),
        pixel_scale: f32,
        sprites: &mut Vec<RenderSprite>,
        x: usize,
        y: usize,
        tint: [f32; 4],
    ) {
        let (anchor, zoom, sx, sy) = self.structure_anchor(window_size, pixel_scale, x, y);
        push_sprite(
            renderer,
            sprites,
            1256,
            anchor,
            zoom,
            sprite_depth(sx, sy, DEPTH_BUILDING, 96),
            tint,
            self.animation_frame(),
        );
    }

    pub(super) fn push_drag_cost_overlay(
        &self,
        window_size: (u32, u32),
        pixel_scale: f32,
        solids: &mut Vec<UiSolid>,
    ) {
        let overlay = if let Some(drag) = self.network_drag {
            self.network_drag_cost()
                .map(|(_, cost)| (cost, drag.current))
        } else if let Some(drag) = self.underground_drag {
            self.underground_drag_cost()
                .map(|(_, cost)| (cost, drag.current))
        } else if let Some(drag) = self.highway_drag {
            self.highway_drag_cost().map(|cost| (cost, drag.current))
        } else if let Some(drag) = self.zone_drag {
            self.zone_drag_cost().map(|cost| (cost, drag.current))
        } else {
            self.hover_cost_overlay()
        };
        let Some((cost, tile)) = overlay else {
            return;
        };

        let text = format!("${}", format_number(cost));
        let scale = (3.0 * pixel_scale).round().max(2.0);
        let width = ui::pixel_text_width(&text, scale);
        let height = 5.0 * scale;
        let (anchor, _, _, _) = self.structure_anchor(window_size, pixel_scale, tile.0, tile.1);
        let screen_w = window_size.0 as f32;
        let screen_h = window_size.1 as f32;
        let x = (anchor.x + 18.0 * pixel_scale).clamp(0.0, (screen_w - width).max(0.0));
        let y = (anchor.y + 8.0 * pixel_scale).clamp(0.0, (screen_h - height).max(0.0));
        let depth = ui::UI_DEPTH + 100.0;

        ui::push_pixel_text(
            solids,
            &text,
            x + scale,
            y + scale,
            scale,
            [0.0, 0.0, 0.0, 0.85],
            depth,
        );
        ui::push_pixel_text(
            solids,
            &text,
            x,
            y,
            scale,
            [1.0, 1.0, 1.0, 1.0],
            depth + 0.1,
        );
    }

    pub(super) fn hover_cost_overlay(&self) -> Option<(u32, (usize, usize))> {
        let tile = self.hover_tile?;
        let placement = self.tool_placement_at(tile.0, tile.1)?;
        let per_tile = self.active_tool_cost_per_tile()?;
        let cost = if self.tool == EditTool::Bulldoze {
            self.bulldoze_cost_units_for_click(tile.0, tile.1, placement.valid_tiles.len())
                .saturating_mul(per_tile)
        } else if self.tool == EditTool::Tunnel {
            placement.valid_tiles.len() as u32 * per_tile
        } else {
            per_tile
        };
        (!placement.valid_tiles.is_empty() && placement.invalid_tiles.is_empty())
            .then_some((cost, tile))
    }

    pub(super) fn tool_placement_at(&self, x: usize, y: usize) -> Option<ToolPlacement> {
        if self.mode == AppMode::CityView
            && !self.ui_action_enabled(UiAction::SelectTool(self.tool))
        {
            return None;
        }
        match (self.mode, self.tool) {
            (AppMode::EditNewMap, EditTool::Query) | (AppMode::CityView, EditTool::Query) => None,
            (AppMode::EditNewMap, _) => {
                Some(ToolPlacement::valid(ToolPreview::Terrain, vec![(x, y)]))
            }
            (AppMode::CityView, EditTool::Bulldoze) => Some(self.bulldoze_placement_at(x, y)),
            (AppMode::CityView, EditTool::Sign) => Some(self.sign_placement_at(x, y)),
            (
                AppMode::CityView,
                EditTool::RaiseTerrain
                | EditTool::LowerTerrain
                | EditTool::LevelTerrain
                | EditTool::StretchTerrain
                | EditTool::SpreadTerrain,
            ) => Some(self.city_terrain_edit_placement_at(x, y)),
            (AppMode::CityView, EditTool::Road | EditTool::Rail | EditTool::Power) => {
                let kind = network_kind_for_tool(self.tool)?;
                Some(self.network_placement_for_path(&[(x, y)], kind))
            }
            (AppMode::CityView, tool) if zone_id_for_tool(tool).is_some() => {
                let zone_id = zone_id_for_tool(tool)?;
                Some(self.zone_placement_for_rect((x, y), (x, y), zone_id))
            }
            (AppMode::CityView, EditTool::Water) => Some(self.surface_water_placement_at(x, y)),
            (AppMode::CityView, EditTool::Trees | EditTool::Forest) => {
                Some(self.tree_placement_at(x, y))
            }
            (AppMode::CityView, EditTool::Pipes | EditTool::Subway) => {
                let kind = underground_kind_for_tool(self.tool)?;
                Some(self.underground_placement_at(x, y, kind))
            }
            (AppMode::CityView, EditTool::Highway) => {
                Some(self.highway_placement_for_path(&[(x, y)]))
            }
            (AppMode::CityView, EditTool::Onramp) => Some(self.onramp_placement_at(x, y)),
            (AppMode::CityView, EditTool::SubToRail) => Some(self.sub_to_rail_placement_at(x, y)),
            (AppMode::CityView, EditTool::Tunnel) => Some(self.tunnel_placement_at(x, y)),
            (AppMode::CityView, tool) if dispatch_kind_for_tool(tool).is_some() => {
                let kind = dispatch_kind_for_tool(tool).expect("dispatch tool");
                Some(
                    if self.city.can_place_dispatch_actor_for_engine(
                        x,
                        y,
                        kind,
                        self.engine_fidelity,
                    ) {
                        ToolPlacement::valid(ToolPreview::Terrain, vec![(x, y)])
                    } else {
                        ToolPlacement {
                            valid_tiles: Vec::new(),
                            invalid_tiles: vec![(x, y)],
                            preview: ToolPreview::Terrain,
                        }
                    },
                )
            }
            (AppMode::CityView, EditTool::PowerPlantHydro) => {
                Some(self.hydro_power_placement_at(x, y))
            }
            (AppMode::CityView, tool) if placeable_building_for_tool(tool).is_some() => {
                let building = placeable_building_for_tool(tool)?;
                let (origin_x, origin_y) = self.player_building_origin(x, y, building)?;
                Some(self.building_placement_at(origin_x, origin_y, building))
            }
            _ => None,
        }
    }
}
