//! City information, query, and reference panels.

use super::dialogs::*;
use super::*;

impl ViewerState {
    pub(super) fn push_city_info_dialog(
        &self,
        renderer: Option<&Renderer<'_>>,
        window_size: (u32, u32),
        pixel_scale: f32,
        solids: &mut Vec<UiSolid>,
        images: &mut Vec<UiImage>,
    ) {
        let Some(dialog) = self.city_info_dialog else {
            return;
        };
        let rect = city_info_dialog_rect_for(dialog, window_size);
        let depth = ui::UI_DEPTH + 300.0;
        if let CityInfoDialog::Newspaper(edition) = dialog {
            self.push_newspaper_panel(renderer, rect, solids, images, depth, edition);
            return;
        }
        if dialog == CityInfoDialog::Budget {
            self.push_budget_panel(renderer, rect, solids, images, depth);
            return;
        }
        if let CityInfoDialog::RoadBridge {
            span_count,
            selector_mask,
            ..
        } = dialog
        {
            self.push_road_bridge_choice_panel(rect, solids, depth, span_count, selector_mask);
            return;
        }
        if let CityInfoDialog::RailBridge { span_count, .. } = dialog {
            self.push_rail_bridge_choice_panel(rect, solids, depth, span_count);
            return;
        }
        if let CityInfoDialog::PowerBridge { span_count, .. } = dialog {
            self.push_power_bridge_choice_panel(rect, solids, depth, span_count);
            return;
        }
        if let CityInfoDialog::HighwayWaterBridge { candidate } = dialog {
            self.push_highway_water_bridge_choice_panel(rect, solids, depth, candidate);
            return;
        }
        push_ui_rect(solids, rect, [0.64, 0.64, 0.64, 1.0], depth);
        push_ui_bevel(solids, rect, 3.0, false, depth + 0.1);
        ui::push_pixel_text(
            solids,
            dialog.title(),
            rect.x + (rect.w - ui::pixel_text_width(dialog.title(), CITY_INFO_UI_SCALE)) * 0.5,
            rect.y + 16.0,
            CITY_INFO_UI_SCALE,
            [0.0, 0.0, 0.0, 1.0],
            depth + 0.4,
        );
        if !matches!(dialog, CityInfoDialog::StadiumTeam { .. }) {
            push_dialog_button(
                solids,
                city_info_close_rect(rect),
                if matches!(dialog, CityInfoDialog::TunnelQuote { .. }) {
                    "Cancel"
                } else {
                    "Close"
                },
                false,
                depth + 0.2,
            );
        }

        match dialog {
            CityInfoDialog::Map => {
                self.push_city_map_panel(rect, window_size, pixel_scale, solids, depth + 1.0)
            }
            CityInfoDialog::Population => self.push_population_panel(rect, solids, depth + 1.0),
            CityInfoDialog::Neighbors => self.push_neighbors_panel(rect, solids, depth + 1.0),
            CityInfoDialog::Graphs => self.push_graphs_panel(rect, solids, depth + 1.0),
            CityInfoDialog::Industry => self.push_industry_panel(rect, solids, depth + 1.0),
            CityInfoDialog::Budget => unreachable!("budget drawn above"),
            CityInfoDialog::FiscalCrisis => {
                let mut y = rect.y + 70.0;
                let body_scale = 2.0;
                for line in wrap_dialog_text(
                    "The city council urges cuts in spending. Auto-Budget has been turned off. Review taxes and service funding before the next annual budget.",
                    ((rect.w - 48.0) / (6.0 * body_scale)).max(1.0) as usize,
                ) {
                    ui::push_pixel_text(
                        solids,
                        &line,
                        rect.x + 24.0,
                        y,
                        body_scale,
                        [0.0, 0.0, 0.0, 1.0],
                        depth + 1.0,
                    );
                    y += 18.0 * body_scale;
                }
            }
            CityInfoDialog::Ordinances => self.push_ordinances_panel(rect, solids, depth + 1.0),
            CityInfoDialog::Newspaper(edition) => {
                unreachable!("newspaper edition {edition:?} drawn above")
            }
            CityInfoDialog::Scenario => self.push_scenario_panel(rect, solids, depth + 1.0),
            CityInfoDialog::Help => self.push_help_panel(rect, solids, depth + 1.0),
            CityInfoDialog::Query { x, y } => {
                self.push_query_panel(rect, solids, depth + 1.0, x, y)
            }
            CityInfoDialog::TunnelQuote { cost, .. } => {
                push_dialog_button(
                    solids,
                    road_bridge_choice_rect(rect, 0),
                    &format!("Build tunnel for ${cost}"),
                    false,
                    depth + 0.2,
                );
            }
            CityInfoDialog::StadiumTeam { available, .. } => {
                for team in 0..5 {
                    if available & (1 << team) != 0 {
                        let fallback = format!("Team {}", team + 1);
                        let label = self
                            .city
                            .label_at(0xfb + team)
                            .filter(|name| !name.is_empty())
                            .unwrap_or(&fallback);
                        push_dialog_button(
                            solids,
                            stadium_team_choice_rect(rect, team),
                            label,
                            false,
                            depth + 0.2,
                        );
                    }
                }
            }
            CityInfoDialog::RoadBridge { .. } => unreachable!("road bridge chooser drawn above"),
            CityInfoDialog::RailBridge { .. } => unreachable!("rail bridge chooser drawn above"),
            CityInfoDialog::PowerBridge { .. } => unreachable!("power bridge chooser drawn above"),
            CityInfoDialog::HighwayWaterBridge { .. } => {
                unreachable!("highway water chooser drawn above")
            }
        }
    }

    pub(super) fn push_highway_water_bridge_choice_panel(
        &self,
        rect: FloatRect,
        solids: &mut Vec<UiSolid>,
        depth: f32,
        candidate: HighwayWaterBridgeCandidate,
    ) {
        push_ui_rect(solids, rect, [0.64, 0.64, 0.64, 1.0], depth);
        push_ui_bevel(solids, rect, 3.0, false, depth + 0.1);
        ui::push_pixel_text(
            solids,
            "Select Highway Bridge",
            rect.x
                + (rect.w - ui::pixel_text_width("Select Highway Bridge", CITY_INFO_UI_SCALE))
                    * 0.5,
            rect.y + 16.0,
            CITY_INFO_UI_SCALE,
            [0.0, 0.0, 0.0, 1.0],
            depth + 0.4,
        );
        push_dialog_button(
            solids,
            city_info_close_rect(rect),
            "Cancel",
            false,
            depth + 0.2,
        );
        ui::push_pixel_text(
            solids,
            &format!(
                "Span: {} segments; prices are per segment.",
                candidate.span_count
            ),
            rect.x + 34.0,
            rect.y + 58.0,
            CITY_INFO_UI_SCALE,
            [0.0, 0.0, 0.0, 1.0],
            depth + 0.3,
        );
        for (ordinal, selector) in pe_highway_water_selectors_45c1e0(candidate.selector_mask)
            .into_iter()
            .enumerate()
        {
            let label = format!(
                "{}   ${} per segment",
                pe_highway_water_selector_name_45c1e0(selector),
                pe_highway_water_selector_cost_45c1e0(selector).unwrap_or(0)
            );
            push_dialog_button(
                solids,
                road_bridge_choice_rect(rect, ordinal),
                &label,
                false,
                depth + 0.2,
            );
        }
    }

    pub(super) fn push_road_bridge_choice_panel(
        &self,
        rect: FloatRect,
        solids: &mut Vec<UiSolid>,
        depth: f32,
        span_count: usize,
        selector_mask: u8,
    ) {
        push_ui_rect(solids, rect, [0.64, 0.64, 0.64, 1.0], depth);
        push_ui_bevel(solids, rect, 3.0, false, depth + 0.1);
        ui::push_pixel_text(
            solids,
            "Select Road Bridge",
            rect.x
                + (rect.w - ui::pixel_text_width("Select Road Bridge", CITY_INFO_UI_SCALE)) * 0.5,
            rect.y + 16.0,
            CITY_INFO_UI_SCALE,
            [0.0, 0.0, 0.0, 1.0],
            depth + 0.4,
        );
        push_dialog_button(
            solids,
            city_info_close_rect(rect),
            "Cancel",
            false,
            depth + 0.2,
        );
        ui::push_pixel_text(
            solids,
            &format!("Span: {span_count} tiles; prices are per tile."),
            rect.x + 34.0,
            rect.y + 58.0,
            CITY_INFO_UI_SCALE,
            [0.0, 0.0, 0.0, 1.0],
            depth + 0.3,
        );
        for (ordinal, selector) in pe_road_bridge_selectors(selector_mask)
            .into_iter()
            .enumerate()
        {
            let label = format!(
                "{}   ${} per tile",
                pe_road_bridge_selector_name(selector),
                pe_road_bridge_selector_cost(selector)
            );
            push_dialog_button(
                solids,
                road_bridge_choice_rect(rect, ordinal),
                &label,
                false,
                depth + 0.2,
            );
        }
    }

    pub(super) fn push_rail_bridge_choice_panel(
        &self,
        rect: FloatRect,
        solids: &mut Vec<UiSolid>,
        depth: f32,
        span_count: usize,
    ) {
        push_ui_rect(solids, rect, [0.64, 0.64, 0.64, 1.0], depth);
        push_ui_bevel(solids, rect, 3.0, false, depth + 0.1);
        ui::push_pixel_text(
            solids,
            "Select Rail Bridge",
            rect.x
                + (rect.w - ui::pixel_text_width("Select Rail Bridge", CITY_INFO_UI_SCALE)) * 0.5,
            rect.y + 16.0,
            CITY_INFO_UI_SCALE,
            [0.0, 0.0, 0.0, 1.0],
            depth + 0.4,
        );
        push_dialog_button(
            solids,
            city_info_close_rect(rect),
            "Cancel",
            false,
            depth + 0.2,
        );
        ui::push_pixel_text(
            solids,
            &format!("Span: {span_count} tiles; $75 per tile."),
            rect.x + 34.0,
            rect.y + 58.0,
            CITY_INFO_UI_SCALE,
            [0.0, 0.0, 0.0, 1.0],
            depth + 0.3,
        );
        push_dialog_button(
            solids,
            road_bridge_choice_rect(rect, 0),
            "Rail Bridge   $75 per tile",
            false,
            depth + 0.2,
        );
    }

    pub(super) fn push_power_bridge_choice_panel(
        &self,
        rect: FloatRect,
        solids: &mut Vec<UiSolid>,
        depth: f32,
        span_count: usize,
    ) {
        push_ui_rect(solids, rect, [0.64, 0.64, 0.64, 1.0], depth);
        push_ui_bevel(solids, rect, 3.0, false, depth + 0.1);
        ui::push_pixel_text(
            solids,
            "Select Power Bridge",
            rect.x
                + (rect.w - ui::pixel_text_width("Select Power Bridge", CITY_INFO_UI_SCALE)) * 0.5,
            rect.y + 16.0,
            CITY_INFO_UI_SCALE,
            [0.0, 0.0, 0.0, 1.0],
            depth + 0.4,
        );
        push_dialog_button(
            solids,
            city_info_close_rect(rect),
            "Cancel",
            false,
            depth + 0.2,
        );
        ui::push_pixel_text(
            solids,
            &format!("Span: {span_count} tiles; $10 per tile."),
            rect.x + 34.0,
            rect.y + 58.0,
            CITY_INFO_UI_SCALE,
            [0.0, 0.0, 0.0, 1.0],
            depth + 0.3,
        );
        push_dialog_button(
            solids,
            road_bridge_choice_rect(rect, 0),
            "Power Bridge   $10 per tile",
            false,
            depth + 0.2,
        );
    }

    pub(super) fn push_query_panel(
        &self,
        rect: FloatRect,
        solids: &mut Vec<UiSolid>,
        depth: f32,
        x: usize,
        y: usize,
    ) {
        let label = query_tile_label(&self.city, x, y, self.show_underground);
        ui::push_pixel_text(
            solids,
            &label,
            rect.x + 48.0,
            rect.y + 58.0,
            2.5,
            [0.0, 0.0, 0.0, 1.0],
            depth,
        );

        let (general, specific) = query_dialog_row_groups(
            &self.city,
            x,
            y,
            self.show_underground,
            self.transit_passenger_stats(),
            self.pe_process_crt.seed(),
        );
        let left = rect.x + 48.0;
        let right = rect.x + rect.w * 0.52;
        let mut y_pos = rect.y + 96.0;
        for (row_label, value) in &general {
            push_compact_info_text(solids, row_label, value, left, y_pos, depth);
            y_pos += 28.0;
        }
        y_pos = rect.y + 96.0;
        for (row_label, value) in &specific {
            push_compact_info_text(solids, row_label, value, right, y_pos, depth);
            y_pos += 28.0;
        }
    }

    pub(super) fn push_city_map_panel(
        &self,
        rect: FloatRect,
        window_size: (u32, u32),
        pixel_scale: f32,
        solids: &mut Vec<UiSolid>,
        depth: f32,
    ) {
        let (map, cell) = city_map_panel_rect_and_cell(rect);
        push_ui_rect(solids, map, [0.16, 0.16, 0.16, 1.0], depth);
        for map_y in 0..CITY_MAP_GRID {
            for map_x in 0..CITY_MAP_GRID {
                push_ui_rect(
                    solids,
                    FloatRect {
                        x: map.x + map_x as f32 * cell,
                        y: map.y + map_y as f32 * cell,
                        w: cell.max(1.0),
                        h: cell.max(1.0),
                    },
                    city_map_cell_color(&self.city, map_x, map_y),
                    depth + 0.1,
                );
            }
        }
        push_ui_bevel(solids, map, 2.0, true, depth + 0.2);
        if let Some((x, y)) =
            self.tile_at_screen(viewport_center(window_size), window_size, pixel_scale)
        {
            push_ui_bevel(
                solids,
                city_map_cell_rect_for_tile(x, y, map, cell),
                1.0,
                false,
                depth + 0.35,
            );
        }

        let text_x = map.x + map.w + 28.0;
        let mut y = map.y + 4.0;
        push_info_text(solids, "City", &self.city.name, text_x, y, depth + 0.3);
        y += 34.0;
        let population = self.live_population_stats_for_fidelity();
        push_info_text(
            solids,
            "Population",
            &format_number(population.total_population),
            text_x,
            y,
            depth + 0.3,
        );
        y += 42.0;
        ui::push_pixel_text(
            solids,
            "Demand",
            text_x,
            y,
            CITY_INFO_UI_SCALE,
            [0.0, 0.0, 0.0, 1.0],
            depth + 0.3,
        );
        y += 28.0;
        let demand = rci_demand_values_for_city(
            &self.city,
            population,
            self.live_city_service_outcomes_with_process_crt(),
        );
        for (label, color, value) in [
            ("Residential", [0.15, 0.68, 0.18, 1.0], demand[0]),
            ("Commercial", [0.16, 0.32, 0.86, 1.0], demand[1]),
            ("Industrial", [0.88, 0.72, 0.08, 1.0], demand[2]),
        ] {
            push_demand_row(
                solids,
                label,
                value,
                FloatRect {
                    x: text_x,
                    y: y + 18.0,
                    w: 252.0,
                    h: 12.0,
                },
                color,
                depth + 0.3,
            );
            y += 48.0;
        }

        let service_status =
            ServiceStatus::for_city_with_entry_seed(&self.city, self.pe_process_crt.seed());
        let facilities = facility_census_for_city(&self.city);
        let coverage = self.live_city_service_coverage_with_process_crt();
        y += 4.0;
        ui::push_pixel_text(
            solids,
            "Services",
            text_x,
            y,
            CITY_INFO_UI_SCALE,
            [0.0, 0.0, 0.0, 1.0],
            depth + 0.3,
        );
        y += 28.0;
        let facility_rows = [
            (
                "Power",
                format!(
                    "{}/{}",
                    format_number(service_status.power.served),
                    format_number(service_status.power.demand)
                ),
            ),
            (
                "Water",
                format!(
                    "{}/{}",
                    format_number(service_status.water.served),
                    format_number(service_status.water.demand)
                ),
            ),
            ("Police", coverage.police.label()),
            ("Fire", coverage.fire.label()),
            ("Health", coverage.health.label()),
            ("Education", coverage.education.label()),
            ("Parks", coverage.recreation.label()),
            ("Transit", coverage.transit.label()),
        ];
        for (index, (label, value)) in facility_rows.iter().enumerate() {
            let col = index % 2;
            let row = index / 2;
            push_compact_info_text(
                solids,
                label,
                value,
                text_x + col as f32 * 126.0,
                y + row as f32 * 31.0,
                depth + 0.3,
            );
        }
        let special_y = y + 4.0 * 31.0 + 4.0;
        let built_summary = format!(
            "Built Pwr {} Wat {} Civic {} Edu {}",
            format_number(facilities.power_plants),
            format_number(facilities.water_utilities),
            format_number(facilities.civic_services),
            format_number(facilities.education),
        );
        ui::push_pixel_text(
            solids,
            &built_summary,
            text_x,
            special_y,
            2.0,
            [0.12, 0.12, 0.12, 1.0],
            depth + 0.3,
        );
        let special_summary = format!(
            "Parks {} Trn {} Air {}/{} Sea {}/{} Rwd {}",
            format_number(facilities.parks),
            format_number(facilities.transit),
            format_number(facilities.developed_airport_tiles),
            format_number(facilities.airport_tiles),
            format_number(facilities.developed_seaport_tiles),
            format_number(facilities.seaport_tiles),
            format_number(facilities.rewards.saturating_add(facilities.arcologies))
        );
        ui::push_pixel_text(
            solids,
            &special_summary,
            text_x,
            special_y + 20.0,
            2.0,
            [0.12, 0.12, 0.12, 1.0],
            depth + 0.3,
        );

        let legend_y = rect.y + rect.h - 34.0;
        let mut legend_x = map.x;
        for (label, color) in [
            ("Water", [0.12, 0.28, 0.82, 1.0]),
            ("Residential", [0.15, 0.68, 0.18, 1.0]),
            ("Commercial", [0.16, 0.32, 0.86, 1.0]),
            ("Industrial", [0.88, 0.72, 0.08, 1.0]),
            ("Transport", [0.22, 0.22, 0.22, 1.0]),
        ] {
            push_ui_rect(
                solids,
                FloatRect {
                    x: legend_x,
                    y: legend_y,
                    w: 12.0,
                    h: 12.0,
                },
                color,
                depth + 0.3,
            );
            ui::push_pixel_text(
                solids,
                label,
                legend_x + 17.0,
                legend_y,
                2.0,
                [0.0, 0.0, 0.0, 1.0],
                depth + 0.4,
            );
            legend_x += ui::pixel_text_width(label, 2.0) + 34.0;
        }
    }

    pub(super) fn push_population_panel(
        &self,
        rect: FloatRect,
        solids: &mut Vec<UiSolid>,
        depth: f32,
    ) {
        let population = self.live_population_stats_for_fidelity();
        let values = [
            ("Total Population", population.total_population),
            ("Normal Population", population.normal_population),
            ("Arcology Population", population.arcology_population),
            ("Populated Tiles", population.populated_tiles),
            ("Residential Tiles", population.residential_tiles),
            ("Commercial Tiles", population.commercial_tiles),
            ("Industrial Tiles", population.industrial_tiles),
        ];
        let left = rect.x + 64.0;
        let mut y = rect.y + 84.0;
        for (label, value) in values {
            push_info_text(solids, label, &format_number(value), left, y, depth);
            y += 42.0;
        }

        let right = rect.x + rect.w * 0.58;
        let power_maps = power_service_maps_with_entry_seed(&self.city, self.pe_process_crt.seed());
        let outcomes = live_city_service_outcomes_for_city_with_power_and_fidelity(
            &self.city,
            &power_maps,
            self.engine_fidelity,
        );
        y = rect.y + 84.0;
        let demographic_rows = population_panel_demographic_rows_with_power_and_fidelity(
            &self.city,
            population,
            outcomes,
            &power_maps,
            self.engine_fidelity,
        );
        for (label, value) in demographic_rows {
            push_compact_info_text(solids, label, &format_number(value), right, y, depth);
            y += 27.0;
        }

        let mut transit_x = left;
        let transit_y = rect.y + rect.h - 40.0;
        for (label, value) in population_panel_transit_rows(self.transit_passenger_stats()) {
            push_compact_info_text(
                solids,
                label,
                &format_number(value),
                transit_x,
                transit_y,
                depth,
            );
            transit_x += 200.0;
        }
    }

    pub(super) fn push_neighbors_panel(
        &self,
        rect: FloatRect,
        solids: &mut Vec<UiSolid>,
        depth: f32,
    ) {
        let values = self.city.misc_u32_values(0x06d8, 16);
        for index in 0..4 {
            let col = index % 2;
            let row = index / 2;
            let card = FloatRect {
                x: rect.x + 34.0 + col as f32 * (rect.w * 0.5 - 8.0),
                y: rect.y + 76.0 + row as f32 * 174.0,
                w: rect.w * 0.5 - 50.0,
                h: 148.0,
            };
            push_ui_rect(solids, card, [0.70, 0.70, 0.70, 1.0], depth);
            push_ui_bevel(solids, card, 2.0, false, depth + 0.1);
            let base = index * 4;
            let neighbor_id = values.get(base).copied().unwrap_or(index as u32);
            let title = format!("Neighbor {}", neighbor_id.saturating_add(1));
            ui::push_pixel_text(
                solids,
                &title,
                card.x + 14.0,
                card.y + 14.0,
                CITY_INFO_UI_SCALE,
                [0.0, 0.0, 0.0, 1.0],
                depth + 0.2,
            );
            push_info_text(
                solids,
                "Population",
                &format_number(values.get(base + 1).copied().unwrap_or(0)),
                card.x + 14.0,
                card.y + 52.0,
                depth + 0.2,
            );
            push_info_text(
                solids,
                "Value",
                &format_number(values.get(base + 2).copied().unwrap_or(0)),
                card.x + 14.0,
                card.y + 84.0,
                depth + 0.2,
            );
            push_info_text(
                solids,
                "Fame",
                &format_number(values.get(base + 3).copied().unwrap_or(0)),
                card.x + 14.0,
                card.y + 116.0,
                depth + 0.2,
            );
        }
        let connections = neighbor_connection_tiles_for_city(&self.city);
        push_info_text(
            solids,
            "Connection Tiles",
            &format_number(connections),
            rect.x + 42.0,
            rect.y + rect.h - 46.0,
            depth + 0.2,
        );
    }

    pub(super) fn push_graphs_panel(&self, rect: FloatRect, solids: &mut Vec<UiSolid>, depth: f32) {
        let series = graph_panel_xgrp_series(&self.city, self.graph_scale);
        let chart = FloatRect {
            x: rect.x + 54.0,
            y: rect.y + 104.0,
            w: rect.w - 108.0,
            h: rect.h - 210.0,
        };
        push_ui_rect(solids, chart, [0.86, 0.86, 0.82, 1.0], depth);
        push_ui_bevel(solids, chart, 2.0, true, depth + 0.1);
        for fraction in [0.25_f32, 0.5, 0.75] {
            push_ui_rect(
                solids,
                FloatRect {
                    x: chart.x + 2.0,
                    y: chart.y + chart.h * fraction,
                    w: chart.w - 4.0,
                    h: 1.0,
                },
                [0.62, 0.62, 0.58, 1.0],
                depth + 0.15,
            );
        }
        for (index, (values, (_, color))) in series.iter().zip(GRAPH_HISTORY_LEGEND).enumerate() {
            if !self.graph_series_visible[index] {
                continue;
            }
            let max_value = values.iter().copied().max().unwrap_or(1).max(1);
            push_graph_series(solids, values, chart, max_value, color, depth + 0.3);
        }
        for (index, (label, color)) in GRAPH_HISTORY_LEGEND.into_iter().enumerate() {
            let button = graph_legend_rect(rect, index);
            let visible = self.graph_series_visible[index];
            push_ui_rect(
                solids,
                button,
                if visible {
                    [0.52, 0.52, 0.52, 1.0]
                } else {
                    [0.66, 0.66, 0.66, 1.0]
                },
                depth + 0.2,
            );
            push_ui_bevel(solids, button, 2.0, visible, depth + 0.25);
            push_ui_rect(
                solids,
                FloatRect {
                    x: button.x + 8.0,
                    y: button.y + 5.0,
                    w: 16.0,
                    h: 10.0,
                },
                if visible {
                    color
                } else {
                    [0.38, 0.38, 0.38, 1.0]
                },
                depth + 0.3,
            );
            ui::push_pixel_text(
                solids,
                label,
                button.x + 30.0,
                button.y + 4.0,
                2.5,
                [0.0, 0.0, 0.0, 1.0],
                depth + 0.4,
            );
        }
        ui::push_pixel_text(
            solids,
            "Oldest",
            chart.x,
            chart.y + chart.h + 14.0,
            2.5,
            [0.0, 0.0, 0.0, 1.0],
            depth + 0.3,
        );
        let newest = "Newest";
        ui::push_pixel_text(
            solids,
            newest,
            chart.x + chart.w - ui::pixel_text_width(newest, 2.5),
            chart.y + chart.h + 14.0,
            2.5,
            [0.0, 0.0, 0.0, 1.0],
            depth + 0.3,
        );
        for (index, scale) in GraphScale::ALL.into_iter().enumerate() {
            let button = graph_scale_rect(rect, index);
            let selected = self.graph_scale == scale;
            push_ui_rect(
                solids,
                button,
                if selected {
                    [0.52, 0.52, 0.52, 1.0]
                } else {
                    [0.66, 0.66, 0.66, 1.0]
                },
                depth + 0.2,
            );
            push_ui_bevel(solids, button, 2.0, selected, depth + 0.25);
            ui::push_pixel_text(
                solids,
                scale.label(),
                button.x + 12.0,
                button.y + 8.0,
                2.5,
                [0.0, 0.0, 0.0, 1.0],
                depth + 0.4,
            );
        }
    }

    pub(super) fn push_industry_panel(
        &self,
        rect: FloatRect,
        solids: &mut Vec<UiSolid>,
        depth: f32,
    ) {
        let values = if self.engine_fidelity.uses_win95() {
            industry_stats_for_city(&self.city)
        } else {
            mac_industry_stats_for_city(&self.city)
        };
        let label_x = rect.x + 42.0;
        let ratio_x = rect.x + 330.0;
        let tax_x = rect.x + 450.0;
        let demand_x = rect.x + 560.0;
        for (label, x) in [
            ("Industry", label_x),
            ("Ratio", ratio_x),
            ("Tax", tax_x),
            ("Demand", demand_x),
        ] {
            ui::push_pixel_text(
                solids,
                label,
                x,
                rect.y + 70.0,
                2.5,
                [0.0, 0.0, 0.0, 1.0],
                depth,
            );
        }
        for (index, name) in INDUSTRY_LABELS.iter().enumerate() {
            let y = industry_row_y(rect, index);
            if index % 2 == 0 {
                push_ui_rect(
                    solids,
                    FloatRect {
                        x: rect.x + 28.0,
                        y: y - 7.0,
                        w: rect.w - 56.0,
                        h: 26.0,
                    },
                    [0.69, 0.69, 0.69, 1.0],
                    depth,
                );
            }
            ui::push_pixel_text(
                solids,
                name,
                label_x,
                y,
                2.5,
                [0.0, 0.0, 0.0, 1.0],
                depth + 0.1,
            );
            let industry = values[index];
            ui::push_pixel_text(
                solids,
                &format_signed_number(industry.ratio),
                ratio_x,
                y,
                2.5,
                [0.0, 0.0, 0.0, 1.0],
                depth + 0.1,
            );
            push_dialog_button(
                solids,
                industry_tax_adjust_rect(rect, index, false),
                "-",
                false,
                depth + 0.1,
            );
            ui::push_pixel_text(
                solids,
                &format_signed_number(industry.tax),
                tax_x,
                y,
                2.5,
                [0.0, 0.0, 0.0, 1.0],
                depth + 0.2,
            );
            push_dialog_button(
                solids,
                industry_tax_adjust_rect(rect, index, true),
                "+",
                false,
                depth + 0.1,
            );
            ui::push_pixel_text(
                solids,
                &format_signed_number(industry.demand),
                demand_x,
                y,
                2.5,
                [0.0, 0.0, 0.0, 1.0],
                depth + 0.1,
            );
        }
    }

    pub(super) fn push_budget_panel(
        &self,
        renderer: Option<&Renderer<'_>>,
        rect: FloatRect,
        solids: &mut Vec<UiSolid>,
        images: &mut Vec<UiImage>,
        depth: f32,
    ) {
        let bonds = outstanding_bond_count_for_city(&self.city);
        let bond_score = if self.engine_fidelity.uses_win95() {
            pe_bond_city_value_ratio_score_would_41e580(&self.city)
        } else {
            mac_bond_city_value_ratio_3a714(mac_city_value_total_3a714(&self.city), bonds)
                .unwrap_or(u32::MAX)
                .min(6) as u16
        };
        let entries = budget_window::entries_for_city(&self.city, self.engine_fidelity);
        let date = self.game_clock.status_line();
        budget_window::draw(
            rect,
            &budget_window::Content {
                city: &self.city.name,
                date: &date,
                funds: i64::from(self.city.city_funds()),
                bonds,
                rating: pe_bond_credit_rating_caption_4e6a78(bond_score),
                auto_budget: auto_budget_enabled_for_engine(&self.city, self.engine_fidelity),
                can_issue: self.budget_bond_action_enabled(true),
                can_repay: self.budget_bond_action_enabled(false),
                entries: &entries,
                font_atlas: renderer
                    .and_then(|r| r.atlas_for_sprite(crate::newspaper_font::ATLAS_ID)),
                dark_appearance: renderer.is_some_and(|r| r.dark_appearance()),
            },
            solids,
            images,
            depth,
        );
    }

    pub(super) fn push_ordinances_panel(
        &self,
        rect: FloatRect,
        solids: &mut Vec<UiSolid>,
        depth: f32,
    ) {
        let flags = ordinance_flags_for_city_with_fidelity(&self.city, self.engine_fidelity);
        let panel = FloatRect {
            x: rect.x + 24.0,
            y: rect.y + 58.0,
            w: rect.w - 48.0,
            h: rect.h - 116.0,
        };
        push_ui_rect(solids, panel, [0.70, 0.70, 0.70, 1.0], depth);
        push_ui_bevel(solids, panel, 2.0, true, depth + 0.1);
        for col in 0..2 {
            let heading = if col == 0 {
                "Finance / City Services"
            } else {
                "Promotion / Environment"
            };
            ui::push_pixel_text(
                solids,
                heading,
                rect.x + 42.0 + col as f32 * 338.0,
                rect.y + 76.0,
                2.0,
                [0.0, 0.0, 0.0, 1.0],
                depth + 0.2,
            );
        }
        for (index, ordinance) in ORDINANCE_CHOICES.iter().enumerate() {
            let row_rect = ordinance_row_rect(rect, index);
            if index % 2 == 0 {
                push_ui_rect(solids, row_rect, [0.66, 0.66, 0.66, 1.0], depth + 0.1);
            }
            let enabled = (flags & (1 << ordinance.bit)) != 0;
            let checkbox = ordinance_checkbox_rect(row_rect);
            push_ui_rect(solids, checkbox, [0.78, 0.78, 0.78, 1.0], depth + 0.2);
            push_ui_bevel(solids, checkbox, 2.0, enabled, depth + 0.3);
            if enabled {
                push_ui_rect(
                    solids,
                    FloatRect {
                        x: checkbox.x + 5.0,
                        y: checkbox.y + 5.0,
                        w: checkbox.w - 10.0,
                        h: checkbox.h - 10.0,
                    },
                    [0.0, 0.0, 0.0, 1.0],
                    depth + 0.45,
                );
            }
            ui::push_pixel_text(
                solids,
                ordinance.label,
                row_rect.x + 34.0,
                row_rect.y + 6.0,
                2.0,
                [0.0, 0.0, 0.0, 1.0],
                depth + 0.4,
            );
            ui::push_pixel_text(
                solids,
                if enabled { "On" } else { "Off" },
                row_rect.x + row_rect.w - 42.0,
                row_rect.y + 6.0,
                2.0,
                [0.12, 0.12, 0.12, 1.0],
                depth + 0.4,
            );
        }
        let enabled_count = flags.count_ones();
        let summary = format!(
            "Enacted {} / {}   Flags 0x{:05x}",
            enabled_count, ORDINANCE_COUNT, flags
        );
        ui::push_pixel_text(
            solids,
            &summary,
            rect.x + 42.0,
            rect.y + rect.h - 52.0,
            2.0,
            [0.0, 0.0, 0.0, 1.0],
            depth + 0.4,
        );
    }

    pub(super) fn push_help_panel(&self, rect: FloatRect, solids: &mut Vec<UiSolid>, depth: f32) {
        let panel = FloatRect {
            x: rect.x + 72.0,
            y: rect.y + 96.0,
            w: rect.w - 144.0,
            h: rect.h - 180.0,
        };
        push_ui_rect(solids, panel, [0.72, 0.72, 0.72, 1.0], depth);
        push_ui_bevel(solids, panel, 3.0, false, depth + 0.1);
        let mode = match self.mode {
            AppMode::CityView => "City View",
            AppMode::EditNewMap => "Edit Map",
        };
        let rows = [
            ("City", self.city.name.as_str()),
            ("Mode", mode),
            ("Selected Tool", self.tool.label()),
            ("Simulation Speed", self.traffic.speed().label()),
        ];
        let mut y = panel.y + 48.0;
        for (label, value) in rows {
            push_info_text(solids, label, value, panel.x + 36.0, y, depth + 0.2);
            y += 54.0;
        }
    }

    pub(super) fn push_scenario_panel(
        &self,
        rect: FloatRect,
        solids: &mut Vec<UiSolid>,
        depth: f32,
    ) {
        let panel = FloatRect {
            x: rect.x + 36.0,
            y: rect.y + 66.0,
            w: rect.w - 72.0,
            h: rect.h - 126.0,
        };
        push_ui_rect(solids, panel, [0.74, 0.74, 0.72, 1.0], depth);
        push_ui_bevel(solids, panel, 3.0, true, depth + 0.1);

        let title = city_display_name(&self.city);
        ui::push_pixel_text(
            solids,
            &title,
            panel.x + (panel.w - ui::pixel_text_width(&title, 2.5)) * 0.5,
            panel.y + 18.0,
            2.5,
            [0.0, 0.0, 0.0, 1.0],
            depth + 0.2,
        );

        let briefing_rect = FloatRect {
            x: panel.x + 22.0,
            y: panel.y + 54.0,
            w: panel.w - 44.0,
            h: 130.0,
        };
        push_ui_rect(solids, briefing_rect, [0.68, 0.68, 0.66, 1.0], depth + 0.1);
        push_ui_bevel(solids, briefing_rect, 2.0, false, depth + 0.2);
        let scenario_text = scenario_text_for_city(&self.city);
        let briefing = scenario_text
            .extended
            .as_deref()
            .or(scenario_text.selection.as_deref())
            .unwrap_or("No scenario briefing text found.");
        let mut y = briefing_rect.y + 16.0;
        for line in wrap_dialog_text(briefing, 76).into_iter().take(5) {
            ui::push_pixel_text(
                solids,
                &line,
                briefing_rect.x + 16.0,
                y,
                2.0,
                [0.0, 0.0, 0.0, 1.0],
                depth + 0.3,
            );
            y += 20.0;
        }

        let goals_rect = FloatRect {
            x: panel.x + 22.0,
            y: briefing_rect.y + briefing_rect.h + 18.0,
            w: panel.w - 44.0,
            h: panel.y + panel.h - briefing_rect.y - briefing_rect.h - 40.0,
        };
        push_ui_rect(solids, goals_rect, [0.68, 0.68, 0.66, 1.0], depth + 0.1);
        push_ui_bevel(solids, goals_rect, 2.0, false, depth + 0.2);
        ui::push_pixel_text(
            solids,
            "Goals",
            goals_rect.x + 16.0,
            goals_rect.y + 14.0,
            2.5,
            [0.0, 0.0, 0.0, 1.0],
            depth + 0.3,
        );
        let goal_lines = scenario_goals_for_city(&self.city)
            .map(|goals| {
                scenario_goal_lines_with_entry_seed(
                    &self.city,
                    goals,
                    self.scenario_deadline_base_clock(),
                    self.game_clock,
                    self.pe_process_crt.seed(),
                )
            })
            .unwrap_or_else(|| vec!["No scenario goal data found.".to_owned()]);
        let mut y = goals_rect.y + 46.0;
        for line in goal_lines.into_iter().take(10) {
            ui::push_pixel_text(
                solids,
                &line,
                goals_rect.x + 18.0,
                y,
                2.0,
                [0.0, 0.0, 0.0, 1.0],
                depth + 0.3,
            );
            y += 22.0;
        }
    }

    pub(super) fn push_newspaper_panel(
        &self,
        renderer: Option<&Renderer<'_>>,
        rect: FloatRect,
        solids: &mut Vec<UiSolid>,
        images: &mut Vec<UiImage>,
        depth: f32,
        edition: NewspaperEdition,
    ) {
        let population = self.live_population_stats_for_fidelity();
        let services =
            ServiceStatus::for_city_with_entry_seed(&self.city, self.pe_process_crt.seed());
        let active_disaster_tiles = active_disaster_tiles_for_city(&self.city).len();
        let available_rewards = self.active_reward_mask();
        let pe_story = if edition == NewspaperEdition::Founding {
            None
        } else {
            pe_top_newspaper_story_for_city(&self.city)
        };
        let headline = match pe_story {
            Some((PE_NEWSPAPER_STORY_POWER_PLANT_AGING, param)) => {
                pe_power_plant_aging_headline(edition, param)
            }
            Some((PE_NEWSPAPER_STORY_FUND_EDUCATION, _)) => pe_fund_education_headline(edition),
            Some((PE_NEWSPAPER_STORY_PRISON_OVERCROWDED, _)) => {
                pe_prison_overcrowded_headline(edition)
            }
            Some((PE_NEWSPAPER_STORY_BRIDGE_COLLAPSE, _)) => pe_bridge_collapse_headline(edition),
            Some((story_id, _)) if pe_disaster_end_summary_label(story_id).is_some() => {
                pe_disaster_end_summary_headline(edition, story_id)
            }
            Some((story_id, _)) if pe_health_amenities_newspaper_label(story_id).is_some() => {
                pe_health_amenities_newspaper_headline(edition, story_id)
            }
            _ => edition.headline(&services, active_disaster_tiles, available_rewards),
        };
        let story = match pe_story {
            Some((PE_NEWSPAPER_STORY_POWER_PLANT_AGING, param)) => {
                pe_power_plant_aging_story(&self.city.name, param)
            }
            Some((PE_NEWSPAPER_STORY_FUND_EDUCATION, _)) => {
                pe_fund_education_story(&self.city.name)
            }
            Some((PE_NEWSPAPER_STORY_PRISON_OVERCROWDED, _)) => {
                pe_prison_overcrowded_story(&self.city.name)
            }
            Some((PE_NEWSPAPER_STORY_BRIDGE_COLLAPSE, _)) => {
                pe_bridge_collapse_story(&self.city.name)
            }
            Some((story_id, _)) if pe_disaster_end_summary_label(story_id).is_some() => {
                pe_disaster_end_summary_story(&self.city.name, story_id)
            }
            Some((story_id, _)) if pe_health_amenities_newspaper_label(story_id).is_some() => {
                pe_health_amenities_newspaper_story(&self.city.name, story_id)
            }
            _ => edition.story(
                &self.city.name,
                &population,
                active_disaster_tiles,
                available_rewards,
            ),
        };

        let masthead = if edition == NewspaperEdition::Founding {
            edition.masthead(&self.city.name, 0)
        } else {
            NewspaperEdition::Courier
                .masthead(&self.city.name, newspaper_choice_for_city(&self.city))
        };
        let connection = |served: u32, demand: u32| {
            if demand == 0 {
                "No current demand".to_owned()
            } else {
                format!(
                    "{}% connected",
                    (u64::from(served) * 100 / u64::from(demand)).min(100)
                )
            }
        };
        let statistics = [
            ("Population", format_number(population.total_population)),
            (
                "City treasury",
                format!("${}", format_signed_number(self.city.city_funds())),
            ),
            (
                "Water service",
                connection(services.water.served, services.water.demand),
            ),
            (
                "Power service",
                connection(services.power.served, services.power.demand),
            ),
        ];
        let weather = super::weather::weather_status_label(&self.city);
        let picture_id =
            crate::assets::NEWSPAPER_PICTURE_BASE + newspaper::picture_for_headline(&headline).0;
        let picture = renderer.and_then(|renderer| {
            renderer.atlas_for_sprite(picture_id).and_then(|atlas| {
                renderer
                    .sprite_frame_rect(atlas, picture_id)
                    .map(|frame| (atlas, frame))
            })
        });
        let date = self.game_clock.status_line();
        let edition_label = match edition {
            NewspaperEdition::Founding => "FOUNDING EDITION",
            NewspaperEdition::Extra => "SPECIAL EXTRA EDITION",
            NewspaperEdition::Subscription => "DELIVERED TO YOUR CITY",
            NewspaperEdition::Courier => "THE DAILY EDITION",
        };
        newspaper::draw(
            rect,
            &newspaper::Content {
                masthead: &masthead,
                headline: &headline,
                story: &story,
                date: &date,
                edition: edition_label,
                weather,
                statistics: &statistics,
                picture,
                font_atlas: renderer.and_then(|renderer| {
                    renderer.atlas_for_sprite(crate::newspaper_font::ATLAS_ID)
                }),
            },
            solids,
            images,
            depth,
        );
    }

    pub(super) fn city_info_dialog_action_enabled(&self, action: CityInfoDialogAction) -> bool {
        match action {
            CityInfoDialogAction::IssueBond => self.budget_bond_action_enabled(true),
            CityInfoDialogAction::RepayBond => self.budget_bond_action_enabled(false),
            CityInfoDialogAction::SelectRoadBridge(selector) => {
                self.city_info_dialog.is_some_and(|dialog| match dialog {
                    CityInfoDialog::RoadBridge { selector_mask, .. } => {
                        selector_mask & (1 << selector) != 0
                    }
                    _ => false,
                })
            }
            CityInfoDialogAction::SelectRailBridge => self
                .city_info_dialog
                .is_some_and(|dialog| matches!(dialog, CityInfoDialog::RailBridge { .. })),
            CityInfoDialogAction::SelectPowerBridge => self
                .city_info_dialog
                .is_some_and(|dialog| matches!(dialog, CityInfoDialog::PowerBridge { .. })),
            CityInfoDialogAction::SelectHighwayWaterBridge(selector) => {
                self.city_info_dialog.is_some_and(|dialog| match dialog {
                    CityInfoDialog::HighwayWaterBridge { candidate } => {
                        candidate.selector_mask & (1 << selector) != 0
                            && pe_highway_water_selector_cost_45c1e0(selector).is_some()
                    }
                    _ => false,
                })
            }
            _ => true,
        }
    }
}
