//! Modal dialog state, controls, and layout.
//!
//! Placement and simulation workers remain in their existing modules. These
//! views preserve their existing cancellation, modal pause, and click behavior.

pub(super) use super::dialog_layout::*;

use super::{
    GraphScale, MILITARY_BASE_AIR_FORCE, MILITARY_BASE_ARMY, MILITARY_BASE_DECLINED,
    MILITARY_BASE_MISSILE_SILOS, MILITARY_BASE_NAVY, MISC_MILITARY_BASE_TYPE_OFFSET,
    PlacementSound, SIGN_LABEL_MAX_BYTES, ViewerState, newspaper_open_choice_for_edition,
    ordinance_enabled_for_city_with_fidelity, ordinance_index_is_valid,
    pe_powerline_post_place_power_recalc_43f396, raw_tax_rate_for_city, set_city_newspaper_choice,
    set_city_ordinance_enabled_with_fidelity,
};
use crate::city::CityFileKind;
use crate::ui::EditTool;

pub(super) use super::dialog_content::*;
pub(super) use super::dialog_input::*;

impl ViewerState {
    pub(super) fn close_other_modal_dialogs(&mut self) {
        self.power_plant_dialog = None;
        self.arcology_dialog = None;
        self.cancel_sign_dialog();
        self.military_base_dialog = None;
    }

    pub(super) fn close_modal_dialogs(&mut self) {
        self.close_other_modal_dialogs();
        self.city_info_dialog = None;
        self.sync_pe_city_info_modal_state();
    }

    pub(super) fn open_power_plant_dialog(&mut self) {
        self.close_tool_menu();
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
        self.power_plant_dialog = Some(PowerPlantDialogState {
            info_tool: None,
            pressed_tool: None,
        });
        self.query_is_inspect = false;
        self.refresh_status_line();
    }

    pub(super) fn close_power_plant_dialog(&mut self) {
        self.power_plant_dialog = None;
    }

    pub(super) fn open_arcology_dialog(&mut self) {
        self.close_tool_menu();
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
        self.arcology_dialog = Some(ArcologyDialogState {
            info_tool: None,
            pressed_tool: None,
        });
        self.query_is_inspect = false;
        self.refresh_status_line();
    }

    pub(super) fn close_arcology_dialog(&mut self) {
        self.arcology_dialog = None;
    }

    pub(super) fn open_military_base_dialog(&mut self) {
        self.close_tool_menu();
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
        self.military_base_dialog = Some(MilitaryBaseDialogState::default());
        self.query_is_inspect = false;
        self.set_sim_status_announcement("Military Base Offered");
    }

    pub(super) fn close_military_base_dialog(&mut self) {
        self.military_base_dialog = None;
    }

    pub(super) fn accept_military_base_offer(&mut self) -> bool {
        let placed = if self.engine_fidelity.uses_win95() {
            self.try_place_military_base()
        } else {
            super::military_base::place_mac_military_base(&mut self.city, || {
                self.mac_process_rng.random_u16()
            })
        };
        match placed {
            Some((base_type, center)) => {
                self.finish_military_base_offer(base_type);
                self.city.mark_all_dirty();
                self.query_is_inspect = false;
                // PE Game_CenterOnTileCoords after army/AF/naval/silo placement.
                if self.auto_goto_enabled || !self.engine_fidelity.uses_win95() {
                    self.pending_disaster_auto_goto = Some(center);
                }
                self.set_sim_status_announcement(match base_type {
                    MILITARY_BASE_AIR_FORCE => "Air Force Base Established",
                    MILITARY_BASE_ARMY => "Army Base Established",
                    MILITARY_BASE_NAVY => "Naval Yard Established",
                    MILITARY_BASE_MISSILE_SILOS => "Missile Silos Established",
                    _ => "Military Base Established",
                });
                self.finish_mac_military_scenario_tail();
                true
            }
            None => {
                self.decline_military_base_offer_with_message("Military Base Declined");
                false
            }
        }
    }

    pub(super) fn decline_military_base_offer(&mut self) {
        self.decline_military_base_offer_with_message("Military Base Declined");
    }

    pub(super) fn decline_military_base_offer_with_message(&mut self, message: &str) {
        self.finish_military_base_offer(MILITARY_BASE_DECLINED);
        self.query_is_inspect = false;
        self.set_sim_status_announcement(message);
        self.finish_mac_military_scenario_tail();
    }

    fn finish_mac_military_scenario_tail(&mut self) {
        if !self.engine_fidelity.uses_win95() {
            self.tick_mac_scenario_tail();
        }
    }

    fn finish_military_base_offer(&mut self, base_type: u32) {
        self.close_military_base_dialog();
        if self.engine_fidelity.uses_win95() {
            self.city.set_pe_military_base_type(base_type);
        } else {
            self.city.set_mac_city_progression(4);
            self.city
                .set_mac_state_misc_u32_at(MISC_MILITARY_BASE_TYPE_OFFSET, base_type);
            self.city.refresh_mac_arcology_latch();
        }
    }

    pub(super) fn open_sign_dialog(&mut self, sign_id: u8) {
        self.close_tool_menu();
        self.close_modal_dialogs();
        let text = self.city.label_at(sign_id).unwrap_or("Sign").to_owned();
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
        self.sign_dialog = Some(SignDialogState {
            sign_id,
            original_text: text.clone(),
            text,
        });
        self.query_is_inspect = false;
        self.refresh_status_line();
    }

    pub(super) fn cancel_sign_dialog(&mut self) {
        if let Some(dialog) = self.sign_dialog.take() {
            self.city.set_label(dialog.sign_id, dialog.original_text);
        }
    }

    pub(super) fn open_city_info_dialog(&mut self, dialog: CityInfoDialog) {
        if dialog == CityInfoDialog::Scenario && self.city.kind != CityFileKind::WinScenario {
            return;
        }
        // The custom newspaper UI uses Win95 MISC fields. On Mac those same
        // offsets are CODE 4 state-vector slots, so keep this PE-only dialog
        // out of Mac-layout saves even when the selected simulation is Win95.
        // The authored founding edition bypasses those saved PE fields entirely.
        if let CityInfoDialog::Newspaper(edition) = dialog
            && (!self.menu_checks.citygame_newspaper
                || (edition != NewspaperEdition::Founding
                    && (!self.engine_fidelity.uses_win95()
                        || self.city.kind == CityFileKind::MacCity)))
        {
            return;
        }
        if let CityInfoDialog::Newspaper(edition) = dialog
            && edition != NewspaperEdition::Founding
        {
            let choice = newspaper_open_choice_for_edition(&self.city, edition);
            set_city_newspaper_choice(&mut self.city, choice);
            let tax_rate = raw_tax_rate_for_city(&self.city, 0);
            if self.city_info_dialog != Some(dialog)
                && self
                    .city
                    .run_pe_opinion_poll_with_rng(tax_rate, &mut self.pe_process_crt)
            {
                self.queue_disaster_extra_sounds(PlacementSound::Cheers, 1);
            }
        }
        let next_dialog = (self.city_info_dialog != Some(dialog)).then_some(dialog);
        if next_dialog == Some(CityInfoDialog::Budget) {
            self.info_panel_origin = None;
            self.info_panel_scale = 1.0;
        }
        self.close_tool_menu();
        // Replacing one native modal with another retains its outer speed
        // snapshot; there is no timer/idle opportunity between these views.
        self.close_other_modal_dialogs();
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
        self.city_info_dialog = next_dialog;
        self.sync_pe_city_info_modal_state();
    }

    pub(super) fn close_city_info_dialog(&mut self) {
        let closed = self.city_info_dialog.take();
        self.sync_pe_city_info_modal_state();
        // The original dry tool succeeds even when its optional bridge is
        // canceled or refused. Its dispatcher refreshes power on that return.
        if matches!(
            closed,
            Some(CityInfoDialog::PowerBridge {
                dry_stroke: true,
                ..
            })
        ) {
            if self.engine_fidelity.uses_win95() {
                pe_powerline_post_place_power_recalc_43f396(
                    &mut self.city,
                    &mut self.pe_process_crt,
                );
            } else {
                self.mac_power_tool_post_place_recalc();
            }
        }
    }

    pub(super) fn toggle_graph_series(&mut self, index: usize) -> bool {
        let Some(visible) = self.graph_series_visible.get_mut(index) else {
            return false;
        };
        *visible = !*visible;
        true
    }

    pub(super) fn set_graph_scale(&mut self, scale: GraphScale) -> bool {
        if self.graph_scale == scale {
            return false;
        }
        self.graph_scale = scale;
        true
    }

    pub(super) fn toggle_ordinance(&mut self, index: usize) {
        if !ordinance_index_is_valid(index) {
            return;
        }
        let enabled =
            !ordinance_enabled_for_city_with_fidelity(&self.city, index, self.engine_fidelity);
        if set_city_ordinance_enabled_with_fidelity(
            &mut self.city,
            index,
            enabled,
            self.engine_fidelity,
        ) {
            self.refresh_status_line();
        }
    }

    pub(super) fn commit_sign_dialog(&mut self) {
        if let Some(dialog) = self.sign_dialog.take() {
            let text = if dialog.text.is_empty() {
                "Sign".to_owned()
            } else {
                dialog.text
            };
            self.city.set_label(dialog.sign_id, text);
        }
    }

    pub(super) fn append_sign_dialog_text(&mut self, text: &str) {
        let Some(dialog) = self.sign_dialog.as_mut() else {
            return;
        };
        for ch in text.chars().filter(|ch| ch.is_ascii() && !ch.is_control()) {
            if dialog.text.len() + ch.len_utf8() > SIGN_LABEL_MAX_BYTES {
                break;
            }
            dialog.text.push(ch);
        }
    }

    pub(super) fn backspace_sign_dialog_text(&mut self) {
        if let Some(dialog) = self.sign_dialog.as_mut() {
            dialog.text.pop();
        }
    }

    pub(super) fn handle_power_plant_dialog_mouse_down(
        &mut self,
        action: PowerPlantDialogAction,
        video: &sdl3::VideoSubsystem,
        window_id: u32,
    ) {
        self.apply_power_plant_dialog_mouse_down(action);
        self.update_window_title(video, window_id);
    }

    pub(super) fn handle_military_base_dialog_mouse_down(
        &mut self,
        action: MilitaryBaseDialogAction,
    ) {
        if let Some(dialog) = self.military_base_dialog.as_mut() {
            dialog.pressed = Some(action);
        }
    }

    pub(super) fn handle_military_base_dialog_mouse_up(
        &mut self,
        action: Option<MilitaryBaseDialogAction>,
    ) {
        let pressed = self
            .military_base_dialog
            .as_ref()
            .and_then(|dialog| dialog.pressed);
        if let Some(dialog) = self.military_base_dialog.as_mut() {
            dialog.pressed = None;
        }
        if pressed.is_some() && pressed == action {
            match action {
                Some(MilitaryBaseDialogAction::Accept) => {
                    let _ = self.accept_military_base_offer();
                }
                Some(MilitaryBaseDialogAction::Decline) => self.decline_military_base_offer(),
                None => {}
            }
        }
    }

    pub(super) fn apply_power_plant_dialog_mouse_down(&mut self, action: PowerPlantDialogAction) {
        match action {
            PowerPlantDialogAction::Cancel => self.close_power_plant_dialog(),
            PowerPlantDialogAction::Info(tool) => {
                if let Some(dialog) = self.power_plant_dialog.as_mut() {
                    dialog.info_tool = Some(tool);
                    dialog.pressed_tool = None;
                }
            }
            PowerPlantDialogAction::Select(tool) => {
                let enabled = self.chooser_tool_enabled(tool);
                if let Some(dialog) = self.power_plant_dialog.as_mut() {
                    dialog.pressed_tool = enabled.then_some(tool);
                }
            }
        }
    }

    pub(super) fn handle_power_plant_dialog_mouse_up(
        &mut self,
        action: Option<PowerPlantDialogAction>,
        video: &sdl3::VideoSubsystem,
        window_id: u32,
    ) {
        self.finish_power_plant_dialog_mouse_up(action);
        self.update_window_title(video, window_id);
    }

    pub(super) fn finish_power_plant_dialog_mouse_up(
        &mut self,
        action: Option<PowerPlantDialogAction>,
    ) -> Option<EditTool> {
        let pressed_tool = self
            .power_plant_dialog
            .and_then(|dialog| dialog.pressed_tool);
        if let Some(dialog) = self.power_plant_dialog.as_mut() {
            dialog.pressed_tool = None;
        }
        if let (Some(tool), Some(PowerPlantDialogAction::Select(released_tool))) =
            (pressed_tool, action)
            && tool == released_tool
            && self.chooser_tool_enabled(tool)
        {
            self.select_tool_state(tool);
            log::info!("tool: {}", tool.label());
            return Some(tool);
        }
        None
    }

    pub(super) fn handle_arcology_dialog_mouse_down(
        &mut self,
        action: ArcologyDialogAction,
        video: &sdl3::VideoSubsystem,
        window_id: u32,
    ) {
        self.apply_arcology_dialog_mouse_down(action);
        self.update_window_title(video, window_id);
    }

    pub(super) fn apply_arcology_dialog_mouse_down(&mut self, action: ArcologyDialogAction) {
        match action {
            ArcologyDialogAction::Cancel => self.close_arcology_dialog(),
            ArcologyDialogAction::Info(tool) => {
                if let Some(dialog) = self.arcology_dialog.as_mut() {
                    dialog.info_tool = Some(tool);
                    dialog.pressed_tool = None;
                }
            }
            ArcologyDialogAction::Select(tool) => {
                let enabled = self.chooser_tool_enabled(tool);
                if let Some(dialog) = self.arcology_dialog.as_mut() {
                    dialog.pressed_tool = enabled.then_some(tool);
                }
            }
        }
    }

    pub(super) fn handle_arcology_dialog_mouse_up(
        &mut self,
        action: Option<ArcologyDialogAction>,
        video: &sdl3::VideoSubsystem,
        window_id: u32,
    ) {
        self.finish_arcology_dialog_mouse_up(action);
        self.update_window_title(video, window_id);
    }

    pub(super) fn finish_arcology_dialog_mouse_up(
        &mut self,
        action: Option<ArcologyDialogAction>,
    ) -> Option<EditTool> {
        let pressed_tool = self.arcology_dialog.and_then(|dialog| dialog.pressed_tool);
        if let Some(dialog) = self.arcology_dialog.as_mut() {
            dialog.pressed_tool = None;
        }
        if let (Some(tool), Some(ArcologyDialogAction::Select(released_tool))) =
            (pressed_tool, action)
            && tool == released_tool
            && self.chooser_tool_enabled(tool)
        {
            self.select_tool_state(tool);
            log::info!("tool: {}", tool.label());
            return Some(tool);
        }
        None
    }
}
