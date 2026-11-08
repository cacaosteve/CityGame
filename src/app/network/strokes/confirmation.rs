use super::*;

pub(in crate::app) fn road_neighbor_link_is_available(
    state: &ViewerState,
    candidate: RoadNeighborLinkCandidate,
) -> bool {
    candidate.endpoint.0 < CITY_WIDTH
        && candidate.endpoint.1 < CITY_HEIGHT
        && state.engine_fidelity == EngineFidelity::Win95
        && candidate.initial_xtxt != 0xfa
        && (0x1d..=0x2b).contains(
            &state
                .city
                .building_at(candidate.endpoint.0, candidate.endpoint.1),
        )
        && state.city.city_funds() >= 1_000
}

pub(in crate::app) fn apply_road_neighbor_link_confirmation(
    state: &mut ViewerState,
    candidate: RoadNeighborLinkCandidate,
    accepted: bool,
) -> bool {
    if !accepted || !road_neighbor_link_is_available(state, candidate) {
        return false;
    }
    let (x, y) = candidate.endpoint;
    state.city.debit_city_funds(1_000);
    state.city.set_text_overlay(x, y, 0xfa);
    // PE thunk 0x401c76 -> FUN_0044aed0 reshapes this endpoint only.
    state.refresh_network_tile_with_pe_neighbor_gates(x, y, NetworkKind::Road, true, false);
    state.refresh_live_misc_current_values();
    state.refresh_budget_service_rows_from_live_counts();
    state.refresh_reward_availability();
    state.refresh_status_line();
    true
}

pub(in crate::app) fn show_road_neighbor_link_confirmation(
    state: &mut ViewerState,
    window: &sdl3::video::Window,
    candidate: Option<RoadNeighborLinkCandidate>,
) {
    let Some(candidate) = candidate else {
        return;
    };
    if !road_neighbor_link_is_available(state, candidate) {
        return;
    }
    let buttons = [
        ButtonData {
            flags: MessageBoxButtonFlag::RETURNKEY_DEFAULT,
            button_id: 6,
            text: "Yes",
        },
        ButtonData {
            flags: MessageBoxButtonFlag::ESCAPEKEY_DEFAULT,
            button_id: 7,
            text: "No",
        },
    ];
    let accepted = match show_message_box(
        MessageBoxFlag::empty(),
        &buttons,
        "Neighbor Connection",
        "Do you want to build a connection to your neighbor for $1000?",
        window,
        None,
    ) {
        Ok(ClickedButton::CustomButton(button)) => button.button_id == 6,
        Ok(ClickedButton::CloseButton) => false,
        Err(error) => {
            log::warn!("could not show road neighbor-link prompt: {error}");
            false
        }
    };
    apply_road_neighbor_link_confirmation(state, candidate, accepted);
}

pub(in crate::app) fn rail_neighbor_link_is_available(
    state: &ViewerState,
    candidate: RailNeighborLinkCandidate,
) -> bool {
    candidate.endpoint.0 < CITY_WIDTH
        && candidate.endpoint.1 < CITY_HEIGHT
        && state.engine_fidelity == EngineFidelity::Win95
        && state
            .city
            .text_overlay_at(candidate.endpoint.0, candidate.endpoint.1)
            != Some(0xfa)
        && state.city.city_funds() >= 1_500
}

pub(in crate::app) fn apply_rail_neighbor_link_confirmation(
    state: &mut ViewerState,
    candidate: RailNeighborLinkCandidate,
    accepted: bool,
) -> bool {
    if !accepted || !rail_neighbor_link_is_available(state, candidate) {
        return false;
    }
    let (x, y) = candidate.endpoint;
    state.city.debit_city_funds(1_500);
    state.city.set_text_overlay(x, y, 0xfa);
    // PE thunk 0x401843 -> FUN_0044B3A0 reshapes this endpoint only.
    state.refresh_network_tile_with_pe_neighbor_gates(x, y, NetworkKind::Rail, false, true);
    state.city.pe_nonnetwork_neighbor_links =
        state.city.pe_nonnetwork_neighbor_links.wrapping_add(1);
    state.refresh_live_misc_current_values();
    state.refresh_budget_service_rows_from_live_counts();
    state.refresh_status_line();
    true
}

pub(in crate::app) fn show_rail_neighbor_link_confirmation(
    state: &mut ViewerState,
    window: &sdl3::video::Window,
    candidate: Option<RailNeighborLinkCandidate>,
) {
    let Some(candidate) = candidate else {
        return;
    };
    if !rail_neighbor_link_is_available(state, candidate) {
        return;
    }
    let buttons = [
        ButtonData {
            flags: MessageBoxButtonFlag::RETURNKEY_DEFAULT,
            button_id: 6,
            text: "Yes",
        },
        ButtonData {
            flags: MessageBoxButtonFlag::ESCAPEKEY_DEFAULT,
            button_id: 7,
            text: "No",
        },
    ];
    let accepted = match show_message_box(
        MessageBoxFlag::empty(),
        &buttons,
        "Neighbor Connection",
        "Do you want to build a connection to your neighbor for $1500?",
        window,
        None,
    ) {
        Ok(ClickedButton::CustomButton(button)) => button.button_id == 6,
        Ok(ClickedButton::CloseButton) => false,
        Err(error) => {
            log::warn!("could not show Rail neighbor-link prompt: {error}");
            false
        }
    };
    apply_rail_neighbor_link_confirmation(state, candidate, accepted);
}
