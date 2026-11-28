//! Pointer handling and action routing for modal dialogs.

use super::dialog_content::*;
use super::dialog_layout::*;
use super::network::{show_rail_neighbor_link_confirmation, show_road_neighbor_link_confirmation};
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
    if state.win95_new_city_setup.is_some() {
        if state.win95_setup_pointer(screen, window_size) {
            push_command(AppCommand::FinishWin95Charter);
        }
        return true;
    }
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
        if matches!(dialog, CityInfoDialog::Newspaper(edition) if !edition.is_mac())
            && state.engine_fidelity.uses_win95()
            && handle_pe_paper_pointer(
                state,
                screen,
                city_info_dialog_rect_for_engine(dialog, window_size, state.engine_fidelity),
            )
        {
            return true;
        }
        if matches!(dialog, CityInfoDialog::Newspaper(edition) if edition.is_mac())
            && handle_mac_paper_pointer(
                state,
                screen,
                city_info_dialog_rect_for_engine(dialog, window_size, state.engine_fidelity),
            )
        {
            return true;
        }
        if let Some(action) =
            city_info_dialog_hit_test_for_engine(screen, window_size, dialog, state.engine_fidelity)
        {
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
        if state.mac_loop_scheduler.january_budget_review_pending
            || (!state.engine_fidelity.uses_win95() && dialog.holds_mac_simulation())
            || point_in_float_rect(
                screen,
                city_info_dialog_rect_for_engine(dialog, window_size, state.engine_fidelity),
            )
        {
            return true;
        }
    }
    false
}

/// Dispatch Win95 clicks in the same viewport used by the recovered layout.
pub(super) fn handle_pe_paper_pointer(
    state: &mut ViewerState,
    screen: Vec2,
    page: FloatRect,
) -> bool {
    if let Some(article) = state.pe_newspaper_article.as_ref() {
        if point_in_float_rect(screen, newspaper::pe_detail_close_rect(page, &article.body)) {
            state.pe_newspaper_article = None;
        } else if newspaper::pe_detail_scroll_max(&article.body) > 0
            && point_in_float_rect(
                screen,
                newspaper::pe_detail_scroll_rect(page, &article.body, false),
            )
        {
            state.scroll_pe_newspaper_article(-3);
        } else if newspaper::pe_detail_scroll_max(&article.body) > 0
            && point_in_float_rect(
                screen,
                newspaper::pe_detail_scroll_rect(page, &article.body, true),
            )
        {
            state.scroll_pe_newspaper_article(3);
        }
        return true;
    }
    if let Some(paint) = state.pe_newspaper_paint.as_ref() {
        if point_in_float_rect(screen, newspaper::pe_close_rect(page)) {
            state.close_city_info_dialog();
            return true;
        }
        let hits = newspaper::pe_hit_positions(page, paint.style, screen);
        for position in hits {
            // Native visits overlapping regions in order. Its article guard
            // prevents opening another dialog after the first successful hit.
            if state.open_pe_newspaper_article(position) {
                break;
            }
        }
        return point_in_float_rect(screen, page);
    }
    if point_in_float_rect(screen, newspaper::lead_hit_rect(page)) {
        let _ = state.open_pe_newspaper_article(4);
        return true;
    }
    false
}

/// Handle the lead article and its modal detail before ordinary paper clicks.
pub(super) fn handle_mac_paper_pointer(
    state: &mut ViewerState,
    screen: Vec2,
    page: FloatRect,
) -> bool {
    if state.mac_newspaper.detail_open {
        if point_in_float_rect(screen, newspaper::detail_scroll_rect(page, false)) {
            reset_mac_disaster_paper_timeout(state);
            state.scroll_mac_paper_detail(-3);
        } else if point_in_float_rect(screen, newspaper::detail_scroll_rect(page, true)) {
            reset_mac_disaster_paper_timeout(state);
            state.scroll_mac_paper_detail(3);
        } else if point_in_float_rect(screen, newspaper::detail_close_rect(page))
            || !point_in_float_rect(screen, newspaper::detail_rect(page))
        {
            reset_mac_disaster_paper_timeout(state);
            state.close_mac_paper_detail();
        }
        return true;
    }
    if state.mac_newspaper.paint.is_some() {
        if point_in_float_rect(screen, newspaper::mac_close_rect(page)) {
            state.close_city_info_dialog();
            return true;
        }
        let family = state.mac_newspaper.paint.as_ref().unwrap().family;
        for position in 4..=10 {
            if newspaper::mac_story_hit_rect(page, family, position)
                .is_some_and(|rect| point_in_float_rect(screen, rect))
            {
                reset_mac_disaster_paper_timeout(state);
                // The original leaves the weather and survey boxes inert in layout 2.
                if family != 2 || (position != 5 && position != 6) {
                    state.open_mac_paper_detail_at(position);
                }
                return true;
            }
        }
        return false;
    }
    if point_in_float_rect(screen, newspaper::lead_hit_rect(page)) {
        reset_mac_disaster_paper_timeout(state);
        state.open_mac_paper_detail();
        return true;
    }
    false
}

fn reset_mac_disaster_paper_timeout(state: &mut ViewerState) {
    if state.city_info_dialog == Some(CityInfoDialog::Newspaper(NewspaperEdition::MacDisaster)) {
        state.mac_loop_scheduler.mac_disaster_paper_started_at =
            Some(state.mac_loop_scheduler.current_tick());
    }
}

pub(super) fn close_city_info_dialog_from_input(
    state: &mut ViewerState,
    window: &sdl3::video::Window,
) {
    let dialog = state.city_info_dialog;
    if matches!(dialog, Some(CityInfoDialog::StadiumTeam { .. })) {
        return;
    }
    state.close_city_info_dialog();
    match dialog {
        Some(CityInfoDialog::RoadBridge {
            mac_neighbor_link, ..
        }) => {
            show_road_neighbor_link_confirmation(state, window, mac_neighbor_link);
        }
        Some(CityInfoDialog::RailBridge { neighbor_link, .. }) => {
            show_rail_neighbor_link_confirmation(state, window, neighbor_link);
        }
        _ => {}
    }
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
        CityInfoDialogAction::Close => close_city_info_dialog_from_input(state, window),
        CityInfoDialogAction::OpenHelp => {
            state.mac_budget_help_selector = None;
            state.open_city_info_dialog(CityInfoDialog::Help);
            state.play_click_sound(sound);
        }
        CityInfoDialogAction::OpenMacBudgetHelp(selector) => {
            if state.open_mac_budget_context_help(selector) {
                state.play_click_sound(sound);
            }
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
            let changed = if state.engine_fidelity.uses_win95() {
                state.adjust_budget_tax_rate(tax_index, delta)
            } else {
                state.adjust_mac_budget_property_tax(delta)
            };
            if changed {
                state.play_click_sound(sound);
                state.update_window_title(video, window.id());
            }
        }
        CityInfoDialogAction::AdjustBudgetFunding {
            service_index,
            delta,
        } => {
            if state.adjust_budget_service_funding_from_dialog(service_index, delta) {
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
                ..
            }) = state.city_info_dialog
                && selector_mask & (1 << selector) != 0
            {
                let placed =
                    state.finish_road_bridge((seed_x, seed_y), direction, span_count, selector);
                if placed > 0 || !state.engine_fidelity.uses_win95() {
                    close_city_info_dialog_from_input(state, window);
                }
                if placed > 0 {
                    state.play_click_sound(sound);
                    state.update_window_title(video, window.id());
                }
            }
        }
        CityInfoDialogAction::SelectHighwayWaterBridge(selector) => {
            if let Some(CityInfoDialog::HighwayWaterBridge { candidate }) = state.city_info_dialog
                && candidate.selector_mask & (1 << selector) != 0
            {
                let placed = state.finish_highway_water_bridge(candidate, selector);
                if placed > 0 || !state.engine_fidelity.uses_win95() {
                    close_city_info_dialog_from_input(state, window);
                }
                if placed > 0 {
                    state.play_click_sound(sound);
                    state.update_window_title(video, window.id());
                }
            }
        }
        CityInfoDialogAction::SelectStadiumTeam(team) => {
            if state.finish_mac_stadium_team(team) {
                state.play_click_sound(sound);
                state.update_window_title(video, window.id());
            }
        }
        CityInfoDialogAction::SelectRailBridge => {
            if let Some(CityInfoDialog::RailBridge {
                seed_x,
                seed_y,
                direction,
                span_count,
                ..
            }) = state.city_info_dialog
            {
                let placed = state.finish_rail_bridge((seed_x, seed_y), direction, span_count);
                close_city_info_dialog_from_input(state, window);
                if placed > 0 {
                    state.play_click_sound(sound);
                    state.update_window_title(video, window.id());
                }
            }
        }
        CityInfoDialogAction::ConfirmTunnel => {
            if state.accept_mac_tunnel_quote() > 0 {
                state.play_click_sound(sound);
                state.update_window_title(video, window.id());
            }
        }
        CityInfoDialogAction::SelectPowerBridge => {
            if let Some(CityInfoDialog::PowerBridge {
                seed_x,
                seed_y,
                direction,
                span_count,
                ..
            }) = state.city_info_dialog
            {
                let placed = state.finish_power_bridge((seed_x, seed_y), direction, span_count);
                state.close_city_info_dialog();
                if placed > 0 {
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
    city_info_dialog_hit_test_for_engine(screen, window_size, active_dialog, EngineFidelity::Win95)
}

pub(super) fn city_info_dialog_hit_test_for_engine(
    screen: Vec2,
    window_size: (u32, u32),
    active_dialog: CityInfoDialog,
    fidelity: EngineFidelity,
) -> Option<CityInfoDialogAction> {
    let dialog = city_info_dialog_rect_for_engine(active_dialog, window_size, fidelity);
    if let CityInfoDialog::StadiumTeam { available, .. } = active_dialog {
        return (0..5)
            .find(|&team| {
                available & (1 << team) != 0
                    && point_in_float_rect(screen, stadium_team_choice_rect(dialog, team))
            })
            .map(CityInfoDialogAction::SelectStadiumTeam);
    }
    let close = if matches!(active_dialog, CityInfoDialog::Newspaper(_)) {
        newspaper::close_rect(dialog)
    } else if active_dialog == CityInfoDialog::Budget {
        if fidelity.uses_win95() {
            budget_window::close_rect(dialog)
        } else {
            budget_window::mac_close_rect(dialog)
        }
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
    if matches!(active_dialog, CityInfoDialog::TunnelQuote { .. }) {
        return point_in_float_rect(screen, road_bridge_choice_rect(dialog, 0))
            .then_some(CityInfoDialogAction::ConfirmTunnel);
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
        if !fidelity.uses_win95() {
            if point_in_float_rect(screen, budget_window::mac_help_rect(dialog)) {
                return Some(CityInfoDialogAction::OpenHelp);
            }
            for row in 0..8 {
                if budget_window::mac_row_help_rect(dialog, row)
                    .is_some_and(|rect| point_in_float_rect(screen, rect))
                {
                    return Some(CityInfoDialogAction::OpenMacBudgetHelp(0x51d + row as u16));
                }
            }
            for increase in [false, true] {
                if point_in_float_rect(screen, budget_window::mac_tax_adjust_rect(dialog, increase))
                {
                    return Some(CityInfoDialogAction::AdjustBudgetTax {
                        tax_index: 0,
                        delta: if increase {
                            BUDGET_TAX_STEP
                        } else {
                            -BUDGET_TAX_STEP
                        },
                    });
                }
            }
            for service_index in [0, 1, 2, 3, 5] {
                for increase in [false, true] {
                    if point_in_float_rect(
                        screen,
                        budget_window::mac_service_adjust_rect(dialog, service_index, increase)?,
                    ) {
                        return Some(CityInfoDialogAction::AdjustBudgetFunding {
                            service_index,
                            delta: if increase {
                                BUDGET_FUNDING_STEP
                            } else {
                                -BUDGET_FUNDING_STEP
                            },
                        });
                    }
                }
            }
            return None;
        }
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

/// The native Mac annual budget modal tracks the control under the initial
/// press, then commits only if release remains inside that same control.
pub(super) fn mac_budget_review_action_for_release(
    down: Vec2,
    release: Vec2,
    window_size: (u32, u32),
) -> Option<CityInfoDialogAction> {
    let pressed = city_info_dialog_hit_test_for_engine(
        down,
        window_size,
        CityInfoDialog::Budget,
        EngineFidelity::Mac68k,
    )?;
    (city_info_dialog_hit_test_for_engine(
        release,
        window_size,
        CityInfoDialog::Budget,
        EngineFidelity::Mac68k,
    ) == Some(pressed))
    .then_some(pressed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mac_annual_budget_action_commits_only_when_released_inside_pressed_control() {
        let done = Vec2::new(464.0, 126.0);
        assert_eq!(
            mac_budget_review_action_for_release(done, done, (640, 480)),
            Some(CityInfoDialogAction::Close),
        );
        assert_eq!(
            mac_budget_review_action_for_release(done, Vec2::new(464.0, 145.0), (640, 480)),
            None,
        );
    }

    #[test]
    fn mac_annual_budget_action_does_not_switch_controls_while_held() {
        let close = Vec2::new(464.0, 126.0);
        let police_decrease = Vec2::new(329.0, 228.0);
        assert_eq!(
            city_info_dialog_hit_test_for_engine(
                police_decrease,
                (640, 480),
                CityInfoDialog::Budget,
                EngineFidelity::Mac68k,
            ),
            Some(CityInfoDialogAction::AdjustBudgetFunding {
                service_index: 0,
                delta: -5,
            }),
        );
        assert_eq!(
            mac_budget_review_action_for_release(close, police_decrease, (640, 480)),
            None,
        );
        assert_eq!(
            mac_budget_review_action_for_release(police_decrease, police_decrease, (640, 480)),
            Some(CityInfoDialogAction::AdjustBudgetFunding {
                service_index: 0,
                delta: -5,
            }),
        );
    }

    #[test]
    fn mac_budget_done_point_maps_to_close_action() {
        let action = city_info_dialog_hit_test_for_engine(
            Vec2::new(464.0, 126.0),
            (640, 480),
            CityInfoDialog::Budget,
            EngineFidelity::Mac68k,
        );

        assert_eq!(action, Some(CityInfoDialogAction::Close));
    }

    #[test]
    fn mac_annual_budget_help_button_maps_to_open_help_action() {
        let dialog = city_info_dialog_rect_for_engine(
            CityInfoDialog::Budget,
            (640, 480),
            EngineFidelity::Mac68k,
        );
        let help_rect = budget_window::mac_help_rect(dialog);
        let help = Vec2::new(
            help_rect.x + help_rect.w * 0.5,
            help_rect.y + help_rect.h * 0.5,
        );

        assert_eq!(
            city_info_dialog_hit_test_for_engine(
                help,
                (640, 480),
                CityInfoDialog::Budget,
                EngineFidelity::Mac68k,
            ),
            Some(CityInfoDialogAction::OpenHelp),
        );
        assert_eq!(
            mac_budget_review_action_for_release(help, help, (640, 480)),
            Some(CityInfoDialogAction::OpenHelp),
        );
    }

    #[test]
    fn mac_budget_row_help_targets_map_to_native_text_selectors() {
        let dialog = city_info_dialog_rect_for_engine(
            CityInfoDialog::Budget,
            (640, 480),
            EngineFidelity::Mac68k,
        );
        let native_body_lengths = [534, 485, 362, 268, 281, 294, 347, 433];
        for row in 0..8 {
            let rect = budget_window::mac_row_help_rect(dialog, row).unwrap();
            let point = Vec2::new(rect.x + rect.w * 0.5, rect.y + rect.h * 0.5);
            let selector = 0x51d + row as u16;
            let (_, description) = budget_window::mac_budget_help_topic(selector).unwrap();
            assert_eq!(
                description.len(),
                native_body_lengths[row],
                "row {row} should retain the full recovered TEXT resource after control-byte cleanup"
            );
            let max_chars = ((544.0 - 72.0) / (6.0 * 1.25)) as usize;
            let lines = super::wrap_dialog_text(description, max_chars);
            assert!(
                lines.len() <= 12,
                "row {row} help should fit the contextual panel at 640x480"
            );
            assert!(
                lines
                    .iter()
                    .all(|line| crate::ui::pixel_text_width(line, 1.25) <= 544.0 - 72.0),
                "row {row} help lines should stay inside the panel's horizontal margins"
            );
            assert_eq!(
                city_info_dialog_hit_test_for_engine(
                    point,
                    (640, 480),
                    CityInfoDialog::Budget,
                    EngineFidelity::Mac68k,
                ),
                Some(CityInfoDialogAction::OpenMacBudgetHelp(selector)),
                "Mac budget row {row} should dispatch native TEXT selector {selector:#x}"
            );
        }

        let rect = budget_window::mac_row_help_rect(dialog, 0).unwrap();
        let point = Vec2::new(rect.x + rect.w * 0.5, rect.y + rect.h * 0.5);
        assert_eq!(
            mac_budget_review_action_for_release(point, point, (640, 480)),
            Some(CityInfoDialogAction::OpenMacBudgetHelp(0x51d))
        );
        assert_ne!(
            city_info_dialog_hit_test_for_engine(
                point,
                (640, 480),
                CityInfoDialog::Budget,
                EngineFidelity::Win95,
            ),
            Some(CityInfoDialogAction::OpenMacBudgetHelp(0x51d))
        );
    }
}
