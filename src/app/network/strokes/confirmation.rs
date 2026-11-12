use super::*;

pub(in crate::app) fn road_neighbor_link_is_available(
    state: &ViewerState,
    candidate: RoadNeighborLinkCandidate,
) -> bool {
    let (x, y) = candidate.endpoint;
    if x >= CITY_WIDTH || y >= CITY_HEIGHT || !state.can_afford_build_cost(1_000) {
        return false;
    }
    state.city.text_overlay_at(x, y) != Some(0xfa) || state.city.is_synthetic_trip_marker(x, y)
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
    if state.engine_fidelity.uses_win95() {
        // PE thunk 0x401c76 -> FUN_0044aed0 reshapes this endpoint only.
        state.refresh_pe_road_tile_44aed0(x, y);
        // 0x449e5c credits the Road counter for every accepted Road link.
        if !crate::app::pe_xbld_counts_fa_network_overlay_4cc4d8(state.city.building_at(x, y)) {
            state.city.pe_road_link_census_delta =
                state.city.pe_road_link_census_delta.wrapping_add(1);
        }
        state.refresh_live_misc_current_values();
        state.refresh_budget_service_rows_from_live_counts();
        state.refresh_reward_availability();
    } else {
        // CODE 13 @ 0x234c2..0x234da writes the paid marker, reshapes the
        // endpoint and increments the saved Mac network tally.
        state.refresh_mac_road_tile(x, y);
        state.city.mac_increment_road_neighbor_link_count();
    }
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
    let (x, y) = candidate.endpoint;
    x < CITY_WIDTH
        && y < CITY_HEIGHT
        && (state.city.text_overlay_at(x, y) != Some(0xfa)
            || state.city.is_synthetic_trip_marker(x, y))
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
    if state.engine_fidelity.uses_win95() {
        // PE thunk 0x401843 -> FUN_0044B3A0 reshapes this endpoint only.
        state.refresh_pe_rail_tile_44b3a0(x, y);
        if crate::app::pe_xbld_counts_fa_network_overlay_4cc4d8(state.city.building_at(x, y)) {
            state.city.pe_road_link_census_delta =
                state.city.pe_road_link_census_delta.wrapping_sub(1);
        }
        state.city.pe_nonnetwork_neighbor_links =
            state.city.pe_nonnetwork_neighbor_links.wrapping_add(1);
        state.refresh_live_misc_current_values();
        state.refresh_budget_service_rows_from_live_counts();
    } else {
        // CODE 13 @ 0x23716..0x2372a increments the non-road tally.
        state.refresh_mac_rail_tile(x, y);
        state.city.mac_increment_rail_neighbor_link_count();
    }
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
