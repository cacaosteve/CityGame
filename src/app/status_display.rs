//! Status-line presentation and display-layer controls.
//!
//! Message guards, drag-cost priority, service reads and display side effects
//! retain their existing order. Rendering, command routing and simulation remain
//! with their existing owners.

use super::ViewerState;
use super::power_water::ServiceStatus;
use crate::data_map::CityDataMapMode;
use crate::mac_menu;
use crate::traffic::GameSpeed;
use crate::ui::UiAction;

mod panel;

pub(super) const CITY_CLEAR_COLOR: [f64; 4] = [0.048, 0.015, 0.004, 1.0];
pub(super) const UNDERGROUND_CLEAR_COLOR: [f64; 4] = [0.82, 0.82, 0.82, 1.0];

impl ViewerState {
    pub(super) fn clear_color(&self) -> [f64; 4] {
        if self.show_underground {
            UNDERGROUND_CLEAR_COLOR
        } else {
            CITY_CLEAR_COLOR
        }
    }

    pub(super) fn set_sim_status_announcement(&mut self, line: impl Into<String>) {
        if self.query_is_inspect {
            return;
        }
        self.query_line = line.into();
        self.query_status_pinned = true;
    }

    pub(super) fn refresh_status_line(&mut self) {
        if self.query_is_inspect {
            return;
        }
        // Later SimCalendar days (power, water, demand) refresh the clock line.
        if self.query_status_pinned {
            return;
        }
        self.write_idle_status_line();
    }

    pub(super) fn refresh_idle_status_line(&mut self) {
        if self.query_is_inspect {
            return;
        }
        self.write_idle_status_line();
    }

    fn write_idle_status_line(&mut self) {
        if let Some((kind, cost)) = self.network_drag_cost() {
            self.query_line = format!("{} ${cost}", kind.label());
            return;
        }
        if let Some((kind, cost)) = self.underground_drag_cost() {
            self.query_line = format!("{} ${cost}", kind.label());
            return;
        }
        if let Some(cost) = self.highway_drag_cost() {
            self.query_line = format!("Highway ${cost}");
            return;
        }
        if let Some(cost) = self.zone_drag_cost() {
            self.query_line = format!("{} ${cost}", self.tool.label());
            return;
        }
        let tool_prefix = self
            .selected_tool_status_line()
            .map(|status| format!("{status}  "))
            .unwrap_or_default();
        let map_prefix = if self.show_underground {
            "Underground  ".to_string()
        } else if self.data_map_mode.is_overlay() {
            format!("Maps: {}  ", self.data_map_mode.label())
        } else {
            String::new()
        };
        let service_status = self.service_status_line();
        let traffic_debug_status = self.traffic_debug_status_line();
        if self.traffic.speed() == GameSpeed::Paused {
            self.query_line = format!(
                "{}{}{}{}  {}  *PAUSED*  anim {}",
                tool_prefix,
                map_prefix,
                self.game_clock.status_line(),
                service_status,
                traffic_debug_status,
                self.animation_frame()
            );
        } else {
            self.query_line = format!(
                "{}{}{}{}  {}  anim {}",
                tool_prefix,
                map_prefix,
                self.game_clock.status_line(),
                service_status,
                traffic_debug_status,
                self.animation_frame()
            );
        }
    }

    pub(super) fn service_status_line(&self) -> String {
        ServiceStatus::for_city_with_entry_seed(&self.city, self.pe_process_crt.seed())
            .status_line_suffix()
    }

    fn traffic_debug_status_line(&self) -> String {
        let fidelity = self.engine_fidelity.label();
        format!("engine={fidelity} traffic=win95-xtrf-animated-overlay")
    }

    pub(super) fn set_data_map_mode(&mut self, mode: CityDataMapMode) {
        if self.data_map_mode == mode {
            return;
        }
        self.data_map_mode = mode;
        self.menu_checks.data_map_mode = mode;
        mac_menu::set_data_map_mode(mode);
        self.invalidate_sprite_cache();
        self.city.mark_all_dirty();
        self.refresh_idle_status_line();
        log::info!("data map mode {}", mode.label());
    }

    pub(super) fn toggle_display_layer(&mut self, action: UiAction) {
        let (label, enabled, invalidate_tiles) = match action {
            UiAction::ToggleBuildings => {
                self.show_buildings = !self.show_buildings;
                ("buildings", self.show_buildings, true)
            }
            UiAction::ToggleSigns => {
                self.show_signs = !self.show_signs;
                ("signs", self.show_signs, false)
            }
            UiAction::ToggleInfrastructure => {
                self.show_infrastructure = !self.show_infrastructure;
                ("infrastructure", self.show_infrastructure, true)
            }
            UiAction::ToggleZones => {
                self.show_zones = !self.show_zones;
                ("zones", self.show_zones, true)
            }
            _ => return,
        };
        if invalidate_tiles {
            self.invalidate_sprite_cache();
        }
        log::info!("display {label} {}", if enabled { "on" } else { "off" });
    }
}
