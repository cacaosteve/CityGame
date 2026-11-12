//! Pointer handling and action routing for modal dialogs.

use super::dialog_content::*;
use super::dialog_layout::*;
use super::*;

pub(super) fn handle_modal_pointer_down(
    state: &mut ViewerState,
    screen: Vec2,
    window_size: (u32, u32),
    pixel_scale: f32,
    video: &sdl3::VideoSubsystem,
    window: &sdl3::video::Window,
    sound: Option<&crate::audio::SoundPlayer>,
) -> bool {
    if state.sign_dialog.is_some() {
        match sign_dialog_hit_test(screen, window_size) {
            Some(SignDialogAction::Commit) => state.commit_sign_dialog(),
            Some(SignDialogAction::Cancel) => state.cancel_sign_dialog(),
            None => {}
        }
        if state.sign_dialog.is_none() {
            video.text_input().stop(window);
        }
        return true;
    }
    if state.military_base_dialog.is_some() {
        if let Some(action) = military_base_dialog_hit_test(screen, window_size) {
            state.handle_military_base_dialog_mouse_down(action);
            state.play_click_sound(sound);
        } else if !point_in_military_base_dialog(screen, window_size) {
            state.decline_military_base_offer();
        }
        return true;
    }
    if state.power_plant_dialog.is_some() {
        if let Some(action) = power_plant_dialog_hit_test(screen, window_size) {
            state.handle_power_plant_dialog_mouse_down(action, video, window.id());
            state.play_click_sound(sound);
        } else if !point_in_power_plant_dialog(screen, window_size) {
            state.close_power_plant_dialog();
        }
        return true;
    }
    if state.arcology_dialog.is_some() {
        if let Some(action) = arcology_dialog_hit_test(screen, window_size) {
            state.handle_arcology_dialog_mouse_down(action, video, window.id());
            state.play_click_sound(sound);
        } else if !point_in_arcology_dialog(screen, window_size) {
            state.close_arcology_dialog();
        }
        return true;
    }
    if let Some(dialog) = state.city_info_dialog {
        let screen = state.info_panel_input_point(screen, window_size);
        if let Some(action) = city_info_dialog_hit_test(screen, window_size, dialog) {
            if !state.city_info_dialog_action_enabled(action) {
                return true;
            }
            apply_city_info_dialog_action(
                state,
                action,
                window_size,
                pixel_scale,
                video,
                window,
                sound,
            );
            return true;
        }
        if point_in_float_rect(screen, city_info_dialog_rect_for(dialog, window_size)) {
            return true;
        }
    }
    false
}

pub(super) fn apply_city_info_dialog_action(
    state: &mut ViewerState,
    action: CityInfoDialogAction,
    window_size: (u32, u32),
    pixel_scale: f32,
    video: &sdl3::VideoSubsystem,
    window: &sdl3::video::Window,
    sound: Option<&crate::audio::SoundPlayer>,
) {
    match action {
        CityInfoDialogAction::Close => {
            state.close_city_info_dialog();
        }
        CityInfoDialogAction::CenterMapOn { x, y } => {
            state.center_tile_at_viewport(x, y, window_size, pixel_scale);
            state.play_click_sound(sound);
        }
        CityInfoDialogAction::ToggleAutoBudget => {
            state.toggle_auto_budget();
            state.play_click_sound(sound);
            state.update_window_title(video, window.id());
        }
        CityInfoDialogAction::ToggleGraphSeries(index) => {
            if state.toggle_graph_series(index) {
                state.play_click_sound(sound);
            }
        }
        CityInfoDialogAction::SetGraphScale(scale) => {
            if state.set_graph_scale(scale) {
                state.play_click_sound(sound);
            }
        }
        CityInfoDialogAction::ToggleOrdinance(index) => {
            state.toggle_ordinance(index);
            state.play_click_sound(sound);
            state.update_window_title(video, window.id());
        }
        CityInfoDialogAction::AdjustIndustryTax {
            industry_index,
            delta,
        } => {
            if state.adjust_industry_tax_rate(industry_index, delta) {
                state.play_click_sound(sound);
                state.update_window_title(video, window.id());
            }
        }
        CityInfoDialogAction::AdjustBudgetTax { tax_index, delta } => {
            if state.adjust_budget_tax_rate(tax_index, delta) {
                state.play_click_sound(sound);
                state.update_window_title(video, window.id());
            }
        }
        CityInfoDialogAction::AdjustBudgetFunding {
            service_index,
            delta,
        } => {
            if state.adjust_budget_service_funding(service_index, delta) {
                state.play_click_sound(sound);
                state.update_window_title(video, window.id());
            }
        }
        CityInfoDialogAction::IssueBond => {
            if state.issue_budget_bond() {
                state.play_click_sound(sound);
                state.update_window_title(video, window.id());
            }
        }
        CityInfoDialogAction::RepayBond => {
            if state.repay_budget_bond() {
                state.play_click_sound(sound);
                state.update_window_title(video, window.id());
            }
        }
        CityInfoDialogAction::SelectRoadBridge(selector) => {
            if let Some(CityInfoDialog::RoadBridge {
                seed_x,
                seed_y,
                direction,
                span_count,
                selector_mask,
            }) = state.city_info_dialog
                && selector_mask & (1 << selector) != 0
            {
                let placed = state.finish_win95_road_bridge(
                    (seed_x, seed_y),
                    direction,
                    span_count,
                    selector,
                );
                if placed > 0 {
                    state.close_city_info_dialog();
                    state.play_click_sound(sound);
                    state.update_window_title(video, window.id());
                }
            }
        }
        CityInfoDialogAction::SelectHighwayWaterBridge(selector) => {
            if let Some(CityInfoDialog::HighwayWaterBridge { candidate }) = state.city_info_dialog
                && candidate.selector_mask & (1 << selector) != 0
            {
                let placed = state.finish_win95_highway_water_bridge(candidate, selector);
                if placed > 0 {
                    state.close_city_info_dialog();
                    state.play_click_sound(sound);
                    state.update_window_title(video, window.id());
                }
            }
        }
        CityInfoDialogAction::SelectRailBridge => {
            if let Some(CityInfoDialog::RailBridge {
                seed_x,
                seed_y,
                direction,
                span_count,
            }) = state.city_info_dialog
            {
                let placed =
                    state.finish_win95_rail_bridge((seed_x, seed_y), direction, span_count);
                if placed > 0 {
                    state.close_city_info_dialog();
                    state.play_click_sound(sound);
                    state.update_window_title(video, window.id());
                }
            }
        }
        CityInfoDialogAction::SelectPowerBridge => {
            if let Some(CityInfoDialog::PowerBridge {
                seed_x,
                seed_y,
                direction,
                span_count,
            }) = state.city_info_dialog
            {
                let placed =
                    state.finish_win95_power_bridge((seed_x, seed_y), direction, span_count);
                if placed > 0 {
                    state.close_city_info_dialog();
                    state.play_click_sound(sound);
                    state.update_window_title(video, window.id());
                }
            }
        }
    }
}

pub(super) fn handle_modal_pointer_up(
    state: &mut ViewerState,
    screen: Vec2,
    window_size: (u32, u32),
    video: &sdl3::VideoSubsystem,
    window: &sdl3::video::Window,
) -> bool {
    if state.military_base_dialog.is_some() {
        state.handle_military_base_dialog_mouse_up(military_base_dialog_hit_test(
            screen,
            window_size,
        ));
        state.update_window_title(video, window.id());
        return true;
    }
    if state.power_plant_dialog.is_some() {
        state.handle_power_plant_dialog_mouse_up(
            power_plant_dialog_hit_test(screen, window_size),
            video,
            window.id(),
        );
        state.update_window_title(video, window.id());
        return true;
    }
    if state.arcology_dialog.is_some() {
        state.handle_arcology_dialog_mouse_up(
            arcology_dialog_hit_test(screen, window_size),
            video,
            window.id(),
        );
        state.update_window_title(video, window.id());
        return true;
    }
    false
}

pub(super) fn city_info_dialog_hit_test(
    screen: Vec2,
    window_size: (u32, u32),
    active_dialog: CityInfoDialog,
) -> Option<CityInfoDialogAction> {
    let dialog = city_info_dialog_rect_for(active_dialog, window_size);
    let close = if matches!(active_dialog, CityInfoDialog::Newspaper(_)) {
        newspaper::close_rect(dialog)
    } else if active_dialog == CityInfoDialog::Budget {
        budget_window::close_rect(dialog)
    } else {
        city_info_close_rect(dialog)
    };
    if point_in_float_rect(screen, close) {
        return Some(CityInfoDialogAction::Close);
    }
    if let CityInfoDialog::RoadBridge { selector_mask, .. } = active_dialog {
        for (ordinal, selector) in pe_road_bridge_selectors(selector_mask)
            .into_iter()
            .enumerate()
        {
            if point_in_float_rect(screen, road_bridge_choice_rect(dialog, ordinal)) {
                return Some(CityInfoDialogAction::SelectRoadBridge(selector));
            }
        }
        return None;
    }
    if matches!(active_dialog, CityInfoDialog::RailBridge { .. }) {
        return point_in_float_rect(screen, road_bridge_choice_rect(dialog, 0))
            .then_some(CityInfoDialogAction::SelectRailBridge);
    }
    if matches!(active_dialog, CityInfoDialog::PowerBridge { .. }) {
        return point_in_float_rect(screen, road_bridge_choice_rect(dialog, 0))
            .then_some(CityInfoDialogAction::SelectPowerBridge);
    }
    if let CityInfoDialog::HighwayWaterBridge { candidate } = active_dialog {
        for (ordinal, selector) in pe_highway_water_selectors_45c1e0(candidate.selector_mask)
            .into_iter()
            .enumerate()
        {
            if point_in_float_rect(screen, road_bridge_choice_rect(dialog, ordinal)) {
                return Some(CityInfoDialogAction::SelectHighwayWaterBridge(selector));
            }
        }
        return None;
    }
    if active_dialog == CityInfoDialog::Map
        && let Some((x, y)) = city_map_tile_for_point(screen, dialog)
    {
        return Some(CityInfoDialogAction::CenterMapOn { x, y });
    }
    if active_dialog == CityInfoDialog::Graphs {
        for (index, scale) in GraphScale::ALL.into_iter().enumerate() {
            if point_in_float_rect(screen, graph_scale_rect(dialog, index)) {
                return Some(CityInfoDialogAction::SetGraphScale(scale));
            }
        }
        for index in 0..GRAPH_HISTORY_TOTAL_SERIES {
            if point_in_float_rect(screen, graph_legend_rect(dialog, index)) {
                return Some(CityInfoDialogAction::ToggleGraphSeries(index));
            }
        }
    }
    if active_dialog == CityInfoDialog::Budget {
        if point_in_float_rect(screen, budget_auto_budget_rect(dialog)) {
            return Some(CityInfoDialogAction::ToggleAutoBudget);
        }
        if point_in_float_rect(screen, budget_bond_button_rect(dialog, true)) {
            return Some(CityInfoDialogAction::IssueBond);
        }
        if point_in_float_rect(screen, budget_bond_button_rect(dialog, false)) {
            return Some(CityInfoDialogAction::RepayBond);
        }
        for tax_index in 0..BUDGET_TAX_COUNT {
            if point_in_float_rect(screen, budget_tax_adjust_rect(dialog, tax_index, false)) {
                return Some(CityInfoDialogAction::AdjustBudgetTax {
                    tax_index,
                    delta: -BUDGET_TAX_STEP,
                });
            }
            if point_in_float_rect(screen, budget_tax_adjust_rect(dialog, tax_index, true)) {
                return Some(CityInfoDialogAction::AdjustBudgetTax {
                    tax_index,
                    delta: BUDGET_TAX_STEP,
                });
            }
        }
        for service_index in 0..BUDGET_SERVICE_COUNT {
            if point_in_float_rect(
                screen,
                budget_service_funding_adjust_rect(dialog, service_index, false),
            ) {
                return Some(CityInfoDialogAction::AdjustBudgetFunding {
                    service_index,
                    delta: -BUDGET_FUNDING_STEP,
                });
            }
            if point_in_float_rect(
                screen,
                budget_service_funding_adjust_rect(dialog, service_index, true),
            ) {
                return Some(CityInfoDialogAction::AdjustBudgetFunding {
                    service_index,
                    delta: BUDGET_FUNDING_STEP,
                });
            }
        }
    }
    if active_dialog == CityInfoDialog::Industry {
        for industry_index in 0..INDUSTRY_COUNT {
            if point_in_float_rect(
                screen,
                industry_tax_adjust_rect(dialog, industry_index, false),
            ) {
                return Some(CityInfoDialogAction::AdjustIndustryTax {
                    industry_index,
                    delta: -BUDGET_TAX_STEP,
                });
            }
            if point_in_float_rect(
                screen,
                industry_tax_adjust_rect(dialog, industry_index, true),
            ) {
                return Some(CityInfoDialogAction::AdjustIndustryTax {
                    industry_index,
                    delta: BUDGET_TAX_STEP,
                });
            }
        }
    }
    if active_dialog == CityInfoDialog::Ordinances {
        for index in 0..ORDINANCE_CHOICES.len() {
            if point_in_float_rect(screen, ordinance_row_rect(dialog, index)) {
                return Some(CityInfoDialogAction::ToggleOrdinance(index));
            }
        }
    }
    None
}
