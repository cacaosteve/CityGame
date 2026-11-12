//! Highway neighbor-link edge checks, prompt, and accepted-link transaction.
//!
//! The confirmation path preserves the native affordability gate and the
//! existing debit, overlay write, counter update, and UI refresh order.

use super::super::HighwayNeighborLinkCandidate;
use super::{ViewerState, is_highway_tile};
use crate::city::{CITY_HEIGHT, CITY_WIDTH, City};
use sdl3::messagebox::{
    ButtonData, ClickedButton, MessageBoxButtonFlag, MessageBoxFlag, show_message_box,
};

pub(in crate::app) fn pe_highway_neighbor_link_origin_at(city: &City, x: usize, y: usize) -> bool {
    x & 1 == 0
        && y & 1 == 0
        && (x == 0 || y == 0 || x + 2 == CITY_WIDTH || y + 2 == CITY_HEIGHT)
        && is_highway_tile(city.building_at(x, y))
}

pub(in crate::app) fn highway_neighbor_link_is_available(
    state: &ViewerState,
    candidate: HighwayNeighborLinkCandidate,
) -> bool {
    let (x, y) = candidate.start;
    (state.city.text_overlay_at(x, y) != Some(0xfa)
        || (!state.engine_fidelity.uses_win95() && state.city.is_synthetic_trip_marker(x, y)))
        && state.city.city_funds() >= 1_500
}

pub(in crate::app) fn apply_highway_neighbor_link_confirmation(
    state: &mut ViewerState,
    candidate: HighwayNeighborLinkCandidate,
    accepted: bool,
) -> bool {
    if !accepted || !highway_neighbor_link_is_available(state, candidate) {
        return false;
    }
    state.city.debit_city_funds(1_500);
    state
        .city
        .set_text_overlay(candidate.endpoint.0, candidate.endpoint.1, 0xfa);
    if state.engine_fidelity.uses_win95() {
        state.city.pe_nonnetwork_neighbor_links =
            state.city.pe_nonnetwork_neighbor_links.wrapping_add(1);
        state.refresh_live_misc_current_values();
        state.refresh_budget_service_rows_from_live_counts();
    } else {
        // CODE 13 @ 0x20d28 calls the read-only classifier, then increments
        // the same saved non-road count as Mac rail.
        state.city.mac_increment_rail_neighbor_link_count();
    }
    state.refresh_status_line();
    true
}

pub(in crate::app) fn show_highway_neighbor_link_confirmation(
    state: &mut ViewerState,
    window: &sdl3::video::Window,
    candidate: Option<HighwayNeighborLinkCandidate>,
) {
    let Some(candidate) = candidate else {
        return;
    };
    if !highway_neighbor_link_is_available(state, candidate) {
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
            log::warn!("could not show highway neighbor-link prompt: {error}");
            false
        }
    };
    apply_highway_neighbor_link_confirmation(state, candidate, accepted);
}
