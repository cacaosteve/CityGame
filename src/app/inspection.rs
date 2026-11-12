//! Tile inspection, query titles, status text, and microsim formatting.
//!
//! Inspection retains its existing mayor-stat refresh, service probes, and dialog
//! routing. Transit simulation and shared passenger counters stay in the parent.

use super::{
    BUS_DEPOT_BUILDING_ID, CityInfoDialog, PlacementSound, RAIL_STATION_BUILDING_ID, RciKind,
    SUBWAY_STATION_BUILDING_ID, TransitPassengerStats, UNDER_TILE_GREY_BOX_1,
    UNDER_TILE_GREY_BOX_2, UndergroundKind, ViewerState, ZONE_AIRPORT, ZONE_COMMERCIAL_DENSE,
    ZONE_COMMERCIAL_LIGHT, ZONE_INDUSTRIAL_DENSE, ZONE_INDUSTRIAL_LIGHT, ZONE_RESIDENTIAL_DENSE,
    ZONE_RESIDENTIAL_LIGHT, ZONE_SEAPORT, base_building_pipe_present,
    building_internal_pipe_present, complete_building_count_for_city, format_number,
    is_abandoned_building, is_bridge_tile, is_highway_tile, is_power_tile, is_rail_tile,
    is_road_tile, is_tunnel_tile, is_user_sign, is_water_terrain, pe_power_recalc_seed,
    raw_tax_rate_for_city, rci_building_kind, structural_internal_pipe_eligible,
    tile_power_status_text_with_entry_seed, tile_water_status_text_with_entry_seed,
    underground_building_pipe_overlay_tile, underground_building_pipe_tile,
    underground_render_tile, underground_tile_has_kind, water_pipe_network_tile, xbld_tile_size,
};
use crate::city::{
    BLANK_MAP_BASE_HEIGHT, CITY_HEIGHT, CITY_WIDTH, City, MAX_SIM_TEXT_ENTRIES,
    MIN_SIM_TEXT_ENTRIES, MicrosimRecord, QUERY_SPECIFIC_MAYOR_STATS_OVERLAY,
};
use glam::Vec2;

pub(super) fn query_tile_status_line(
    city: &City,
    x: usize,
    y: usize,
    prefer_underground: bool,
) -> String {
    query_tile_status_line_with_passengers(
        city,
        x,
        y,
        prefer_underground,
        TransitPassengerStats::default(),
        pe_power_recalc_seed(city),
    )
}

pub(super) fn query_tile_status_line_with_passengers(
    city: &City,
    x: usize,
    y: usize,
    prefer_underground: bool,
    passengers: TransitPassengerStats,
    entry_seed: u32,
) -> String {
    let label = query_tile_label(city, x, y, prefer_underground);
    let mut line = format!(
        "{label} ({x},{y}){}{}",
        tile_water_status_text_with_entry_seed(city, x, y, entry_seed),
        tile_power_status_text_with_entry_seed(city, x, y, entry_seed)
    );
    if let Some(extra) = microsim_query_extra(city, x, y) {
        line.push_str("  ");
        line.push_str(&extra);
    } else if let Some(extra) = transit_query_extra(city, city.building_at(x, y), passengers) {
        line.push_str("  ");
        line.push_str(&extra);
    }
    line
}

/// ClubOpolis QueryGeneral bands (sc2kfix `GetLowHighScale`).
pub(super) fn query_metric_band(value: u8) -> &'static str {
    match value {
        0 => "None",
        1..=59 => "Low",
        60..=119 => "Medium",
        120..=179 => "High",
        _ => "Very High",
    }
}

/// Win95 QueryGeneral altitude in feet (sc2kfix advanced-query formula for land).
pub(super) fn query_altitude_feet(city: &City, x: usize, y: usize) -> i32 {
    let altitude = i32::from(city.altitude_at(x, y));
    let water_level = i32::from(
        city.misc_water_table_level()
            .unwrap_or(u32::from(BLANK_MAP_BASE_HEIGHT)) as u8,
    );
    let under_water = city
        .tile_bits_at(x, y)
        .is_some_and(|bits| bits.water_tile())
        && altitude < water_level;
    if under_water {
        100 * (water_level - altitude) - 50
    } else {
        100 * (altitude - water_level) + 50
    }
}

pub(super) fn query_service_yes_no(status_text: &str) -> &'static str {
    if status_text.contains("=on") {
        "Yes"
    } else if status_text.contains("=off") {
        "No"
    } else {
        "—"
    }
}

pub(super) fn transit_query_extra(
    city: &City,
    building: u8,
    passengers: TransitPassengerStats,
) -> Option<String> {
    let (passengers_per_day, station_label, station_building) = match building {
        BUS_DEPOT_BUILDING_ID => (passengers.bus, "Bus Stations", BUS_DEPOT_BUILDING_ID),
        RAIL_STATION_BUILDING_ID => (passengers.rail, "Rail Stations", RAIL_STATION_BUILDING_ID),
        SUBWAY_STATION_BUILDING_ID => (
            passengers.subway,
            "Subway Stations",
            SUBWAY_STATION_BUILDING_ID,
        ),
        _ => return None,
    };
    let stations = transit_station_count(city, station_building);
    Some(format!(
        "Passengers/day: {}  {station_label}: {}",
        format_number(passengers_per_day),
        format_number(stations)
    ))
}

fn transit_station_count(city: &City, building: u8) -> u32 {
    let size = usize::from(xbld_tile_size(building).max(1));
    let complete = complete_building_count_for_city(city, building, size);
    if complete > 0 {
        return complete;
    }
    let mut tiles = 0_u32;
    for y in 0..CITY_HEIGHT {
        for x in 0..CITY_WIDTH {
            if city.building_at(x, y) == building {
                tiles = tiles.saturating_add(1);
            }
        }
    }
    tiles
}

/// ClubOpolis/manual-style microsim query fields for the status line.
pub(super) fn microsim_query_extra(city: &City, x: usize, y: usize) -> Option<String> {
    let text = city.text_overlay_at(x, y)?;
    if !(MIN_SIM_TEXT_ENTRIES..=MAX_SIM_TEXT_ENTRIES).contains(&text) {
        return None;
    }
    let index = usize::from(text.saturating_sub(MIN_SIM_TEXT_ENTRIES));
    let record = city.microsim_at(index)?;
    if record.tile_id == 0 {
        return None;
    }
    format_microsim_query_stats(city, record)
}

/// ClubOpolis letter grade from microsim `stat0`.
/// Education uses even steps 12..0 → A..F; hospital uses 12/9/6/3/0 → A/B/C/D/F.
pub(super) fn microsim_query_letter_grade(tile_id: u8, stat0: u8) -> char {
    match tile_id {
        209 => match stat0 {
            12 => 'A',
            9 => 'B',
            6 => 'C',
            3 => 'D',
            _ => 'F',
        },
        245 => match stat0 {
            12 => 'A',
            n if n >= 10 => 'B',
            n if n >= 8 => 'C',
            n if n >= 6 => 'D',
            n if n >= 4 => 'E',
            _ => 'F',
        },
        214 | 217 => match stat0 {
            12 => 'A',
            10 => 'B',
            8 => 'C',
            6 => 'D',
            4 => 'E',
            _ => 'F',
        },
        _ => '?',
    }
}

pub(super) fn format_microsim_query_stats(city: &City, record: MicrosimRecord) -> Option<String> {
    Some(match record.tile_id {
        201..=207 => format!(
            "Max {} MW  Running {}  Age {}",
            format_number(u32::from(record.stat1)),
            format_number(u32::from(record.stat2)),
            format_number(u32::from(record.stat0))
        ),
        198 | 200 => format!("Units: {}", format_number(u32::from(record.stat1))),
        236 => format!(
            // PE query: Bus Stations : #1; Buses : #2; Passengers/day : #3.
            "Bus Stations {}  Buses {}  Passengers/day {}",
            format_number(u32::from(record.stat1)),
            format_number(u32::from(record.stat2)),
            format_number(u32::from(record.stat3))
        ),
        237 => format!(
            // PE query: Rail Stations : #1; Passengers/day : #3 (`0x44ee5f`).
            "Rail Stations {}  Passengers/day {}",
            format_number(u32::from(record.stat1)),
            format_number(u32::from(record.stat3))
        ),
        233 => format!(
            // PE query: Subway Stations : #1; Passengers/day : #3 (`0x44edf3`).
            "Subway Stations {}  Passengers/day {}",
            format_number(u32::from(record.stat1)),
            format_number(u32::from(record.stat3))
        ),
        209 => format!(
            "Beds 1,000  Patients {}  Doctors {}  Grade {}",
            format_number(u32::from(record.stat1)),
            format_number(u32::from(record.stat2)),
            microsim_query_letter_grade(record.tile_id, record.stat0)
        ),
        210 => format!(
            // Win95 XMIC: stat1 capacity, stat2 crimes/station, stat3 arrests.
            "Officers {}  Crimes {}  Arrests {}",
            format_number(u32::from(record.stat1)),
            format_number(u32::from(record.stat2)),
            format_number(u32::from(record.stat3))
        ),
        211 => format!(
            "Firefighters {}  Trucks {}  Capacity {}",
            format_number(u32::from(record.stat3)),
            format_number(u32::from(record.stat2)),
            format_number(u32::from(record.stat1))
        ),
        214 => format!(
            "Students {}  Teachers {}  Grade {}",
            format_number(u32::from(record.stat1)),
            format_number(u32::from(record.stat2)),
            microsim_query_letter_grade(record.tile_id, record.stat0)
        ),
        212 => format!(
            // PE query: Attendance : #1; Employees : #2 (`0x44e887` college funding).
            "Attendance {}  Employees {}",
            format_number(u32::from(record.stat1)),
            format_number(u32::from(record.stat2))
        ),
        13 | 213 => format!(
            // PE query: Attendance : #1; Acres : #2 (citywide) (`0x44e8e6`).
            "Attendance {}  Acres {}",
            format_number(u32::from(record.stat1)),
            format_number(u32::from(record.stat2))
        ),
        248 => format!(
            // PE query: Boats : #1 (`0x44efd2`).
            "Boats {}",
            format_number(u32::from(record.stat1))
        ),
        245 => format!(
            // PE query: Visitors : #1; Books : #2; Grade : #G (`0x44ef17`).
            "Visitors {}  Books {}  Grade {}",
            format_number(u32::from(record.stat1)),
            format_number(u32::from(record.stat2)),
            microsim_query_letter_grade(record.tile_id, record.stat0)
        ),
        215 => {
            let team = city
                .label_at(record.stat3.min(255) as u8)
                .filter(|name| !name.is_empty())
                .unwrap_or("Football");
            format!(
                "Attendance {}  Team {}",
                format_number(u32::from(record.stat1)),
                team
            )
        }
        216 => format!(
            "Inmates {}  Capacity {}  Guards {}",
            format_number(u32::from(record.stat1)),
            format_number(u32::from(record.stat2)),
            format_number(u32::from(record.stat3))
        ),
        217 => format!(
            "Students {}  Professors {}  Grade {}",
            format_number(u32::from(record.stat1)),
            format_number(u32::from(record.stat2)),
            microsim_query_letter_grade(record.tile_id, record.stat0)
        ),
        218 => format!(
            "Exhibits {}  Visitors {}  Animals {}  Rating {}",
            format_number(u32::from(record.stat0)),
            format_number(u32::from(record.stat1)),
            format_number(u32::from(record.stat2)),
            format_number(u32::from(record.stat3))
        ),
        208 => format!(
            "Size {}  Built {}",
            format_number(u32::from(record.stat1)),
            format_number(u32::from(record.stat2))
        ),
        219 => format!(
            "Built {}  Fame {}",
            format_number(u32::from(record.stat1)),
            format_number(u32::from(record.stat2))
        ),
        243 => format!(
            "Built {}  Rating {}  Approval {}",
            format_number(u32::from(record.stat1)),
            format_number(u32::from(record.stat0)),
            format_number(u32::from(record.stat2))
        ),
        244 | 250 => format!(
            "Output {}  Capacity {}",
            format_number(u32::from(record.stat1)),
            format_number(u32::from(record.stat2))
        ),
        251..=254 => format!(
            "Design Capacity {}  Residents {}  Built {}  Conditions {}",
            format_number(u32::from(record.stat1).saturating_mul(1_000)),
            format_number(u32::from(record.stat2)),
            format_number(u32::from(record.stat3)),
            format_number(u32::from(record.stat0))
        ),
        255 => format!(
            "Weddings {}  Visitors {}  Sightings {}  Bungee {}",
            format_number(u32::from(record.stat0)),
            format_number(u32::from(record.stat1)),
            format_number(u32::from(record.stat2)),
            format_number(u32::from(record.stat3))
        ),
        _ => return None,
    })
}

/// Win95 `QuerySpecificItem` (`0x44d209`): `XMIC[XTXT-51].tile_id` must be
/// non-zero, then the title is the C-string at `XLAB[XTXT]` (`0x4ca198+25*XTXT`).
/// Overlay `0x6f` refreshes mayor stats in `begin_tile_inspect`, not here.
pub(super) fn microsim_query_title(city: &City, x: usize, y: usize) -> Option<String> {
    let text = city.text_overlay_at(x, y)?;
    if !(MIN_SIM_TEXT_ENTRIES..=MAX_SIM_TEXT_ENTRIES).contains(&text) {
        return None;
    }
    let index = usize::from(text.saturating_sub(MIN_SIM_TEXT_ENTRIES));
    let record = city.microsim_at(index)?;
    if record.tile_id == 0 {
        return None;
    }
    let name = city.label_at(text)?.trim();
    if name.is_empty() {
        return None;
    }
    Some(name.to_owned())
}

pub(super) fn user_sign_query_title(city: &City, x: usize, y: usize) -> Option<String> {
    let text = city.text_overlay_at(x, y)?;
    if !is_user_sign(text) {
        return None;
    }
    let name = city.label_at(text)?.trim();
    if name.is_empty() {
        return None;
    }
    Some(name.to_owned())
}

pub(super) fn query_tile_label(
    city: &City,
    x: usize,
    y: usize,
    prefer_underground: bool,
) -> String {
    if prefer_underground && let Some(label) = underground_query_label(city, x, y) {
        return label.to_owned();
    }

    if let Some(label) = city
        .text_overlay_at(x, y)
        .and_then(disaster_text_overlay_query_label)
    {
        return label.to_owned();
    }

    // Win95 `QuerySpecificItem` (`0x44d24e`): title is XLAB[XTXT] when the
    // overlay maps to a live XMIC row (tile_id != 0). Empty labels fall through
    // to the palette name like QueryGeneral.
    if let Some(label) = microsim_query_title(city, x, y) {
        return label;
    }

    // User signs are XTXT 1..=50. The Query tool routes those to QueryGeneral,
    // which never indexes XMIC (`XTXT-51` would be negative). Title is still
    // the C-string at XLAB[XTXT]; empty labels fall through to the palette.
    if let Some(label) = user_sign_query_title(city, x, y) {
        return label;
    }

    let building = city.building_at(x, y);
    if let Some(label) = surface_building_query_label(building) {
        return label.to_owned();
    }
    if building != 0 {
        return format!("Building {building}");
    }

    if let Some(zone) = city.zone_at(x, y)
        && let Some(label) = zone_query_label(zone.zone_id())
    {
        return label.to_owned();
    }

    if let Some(label) = underground_query_label(city, x, y) {
        return label.to_owned();
    }

    if is_water_terrain(city.terrain_at(x, y)) {
        "Water".to_owned()
    } else {
        "Terrain".to_owned()
    }
}

pub(super) fn disaster_text_overlay_query_label(text_overlay: u8) -> Option<&'static str> {
    Some(match text_overlay {
        0xfb => "Toxic Cloud",
        0xfc => "Flood",
        0xfd | 0xfe => "Rioters",
        0xff => "Fire",
        _ => return None,
    })
}

pub(super) fn surface_building_query_label(building: u8) -> Option<&'static str> {
    if building == 0 {
        return None;
    }
    if let Some(kind) = rci_building_kind(building) {
        return Some(match kind {
            RciKind::Residential => "Residential Building",
            RciKind::Commercial => "Commercial Building",
            RciKind::Industrial => "Industrial Building",
        });
    }
    if is_abandoned_building(building) {
        return Some("Abandoned Building");
    }
    if is_power_tile(building) {
        return Some("Power Lines");
    }
    if is_bridge_tile(building) {
        return Some("Bridge");
    }
    if is_tunnel_tile(building) {
        return Some("Tunnel");
    }
    if is_road_tile(building) {
        return Some("Road");
    }
    if is_rail_tile(building) {
        return Some("Rail");
    }
    if is_highway_tile(building) {
        return Some("Highway");
    }

    Some(match building {
        6 => "Trees",
        13 => "Small Park",
        198 => "Hydro Electric",
        200 => "Wind Power",
        201 => "Gas Power",
        202 => "Oil Power",
        203 => "Nuclear Power",
        204 => "Solar Power",
        205 => "Microwave Power",
        206 => "Fusion Power",
        207 => "Coal Power",
        208 => "City Hall",
        209 => "Hospital",
        210 => "Police",
        211 => "Fire Station",
        212 => "Museum",
        213 => "Big Park",
        214 => "School",
        215 => "Stadium",
        216 => "Prison",
        217 => "College",
        218 => "Zoo",
        219 => "Statue",
        220 => "Water Pump",
        221 => "Runway",
        222 => "Runway Intersection",
        223 => "Seaport Pier",
        224 => "Crane",
        225 => "Control Tower",
        226 => "Military Control Tower",
        227 => "Warehouse",
        228 | 229 => "Airport Building",
        230 => "Tarmac",
        231 => "F-15b",
        232 => "Military Hangar",
        233 => "Subway Station",
        234 => "Radar",
        235 => "Water Tower",
        236 => "Bus Depot",
        237 => "Rail Depot",
        238 => "Parking Lot",
        239 => "Military Parking Lot",
        240 => "Loading Bay",
        241 => "Top Secret",
        242 => "Cargo Yard",
        243 => "Mayor's House",
        244 => "Water Treatment",
        245 => "Library",
        246 => "Big Hangar",
        247 => "Church",
        248 => "Marina",
        249 => "Missile Silo",
        250 => "Desalinization",
        251 => "Plymouth Arco",
        252 => "Forest Arco",
        253 => "Darco",
        254 => "Launch Arco",
        255 => "Braun Llama Dome",
        _ => return None,
    })
}

pub(super) fn zone_query_label(zone_id: u8) -> Option<&'static str> {
    Some(match zone_id {
        ZONE_RESIDENTIAL_LIGHT => "Light Residential",
        ZONE_RESIDENTIAL_DENSE => "Dense Residential",
        ZONE_COMMERCIAL_LIGHT => "Light Commercial",
        ZONE_COMMERCIAL_DENSE => "Dense Commercial",
        ZONE_INDUSTRIAL_LIGHT => "Light Industrial",
        ZONE_INDUSTRIAL_DENSE => "Dense Industrial",
        7 => "Military Zone",
        ZONE_AIRPORT => "Airport Zone",
        ZONE_SEAPORT => "Seaport Zone",
        _ => return None,
    })
}

pub(super) fn underground_query_label(city: &City, x: usize, y: usize) -> Option<&'static str> {
    let underground = city.underground_at(x, y);
    let pipe = underground_tile_has_kind(underground, UndergroundKind::Pipes);
    let subway = underground_tile_has_kind(underground, UndergroundKind::Subway);
    let building_pipe =
        underground_building_pipe_tile(underground) || base_building_pipe_present(city, x, y);

    match (pipe, subway, building_pipe) {
        (true, true, true) => Some("Pipes/Subway + Building Pipes"),
        (true, false, true) => Some("Pipes + Building Pipes"),
        (false, true, true) => Some("Subway + Building Pipes"),
        (true, true, false) => Some("Pipes/Subway Crossing"),
        (true, false, false) => Some("Pipes"),
        (false, true, false) => Some("Subway"),
        (false, false, true) => Some("Building Pipes"),
        (false, false, false) => match underground {
            UNDER_TILE_GREY_BOX_1 | UNDER_TILE_GREY_BOX_2 => Some("Subway Station"),
            _ => None,
        },
    }
}

impl ViewerState {
    pub(super) fn inspect_tile_at(
        &mut self,
        screen: Vec2,
        window_size: (u32, u32),
        pixel_scale: f32,
    ) {
        let Some((x, y)) = self.tile_at_screen(screen, window_size, pixel_scale) else {
            self.query_line = "No tile".to_string();
            self.query_is_inspect = true;
            self.close_city_info_dialog();
            log::info!("inspect: no tile under cursor");
            return;
        };
        self.begin_tile_inspect(x, y);
    }

    pub(super) fn begin_tile_inspect(&mut self, x: usize, y: usize) {
        // Win95 `QuerySpecificItem` (`0x44d1fe`): overlay 0x6f re-runs the mayor
        // poll/countdown walker before the dialog is filled. First mayor is
        // typically overlay 61; 111 is XMIC index 60.
        if self.engine_fidelity.uses_win95()
            && self.city.text_overlay_at(x, y) == Some(QUERY_SPECIFIC_MAYOR_STATS_OVERLAY)
        {
            let tax_rate = raw_tax_rate_for_city(&self.city, 0);
            if self
                .city
                .recalculate_mayors_house_stats(tax_rate, &mut self.pe_process_crt)
            {
                self.queue_disaster_extra_sounds(PlacementSound::Cheers, 1);
            }
        }
        let building = self.city.building_at(x, y);
        let entry_seed = self.pe_process_crt.seed();
        let water_status = tile_water_status_text_with_entry_seed(&self.city, x, y, entry_seed);
        let power_status = tile_power_status_text_with_entry_seed(&self.city, x, y, entry_seed);
        self.query_is_inspect = true;
        self.query_line = query_tile_status_line_with_passengers(
            &self.city,
            x,
            y,
            self.show_underground,
            self.transit_passenger_stats(),
            entry_seed,
        );
        let dialog = CityInfoDialog::Query { x, y };
        self.open_city_info_dialog(dialog);
        // Always keep Query open after inspect (toolbar dialogs toggle; Query refreshes).
        self.city_info_dialog = Some(dialog);
        log::info!(
            "inspect tile ({x}, {y}) xter=0x{:02x} xbld=0x{:02x} xund=0x{:02x} altm land={} water={} struct={} xzon=0x{:02x} xbit=0x{:02x}{water_status}{power_status} building_pipe_present={} structural_pipe_eligible={} base_pipe_present={} water_network={} render={:?} overlay={:?} compass={} camera_rotation={}",
            self.city.terrain_at(x, y),
            building,
            self.city.underground_at(x, y),
            self.city.altitude_at(x, y),
            self.city.water_height_at(x, y),
            self.city.structure_elevation_at(x, y),
            self.city
                .zone_at(x, y)
                .map(|zone| zone.raw_bits())
                .unwrap_or(0),
            self.city
                .tile_bits_at(x, y)
                .map(|bits| bits.raw_bits())
                .unwrap_or(0),
            building_internal_pipe_present(&self.city, x, y),
            structural_internal_pipe_eligible(&self.city, x, y),
            base_building_pipe_present(&self.city, x, y),
            water_pipe_network_tile(&self.city, x, y),
            underground_render_tile(&self.city, x, y, self.city.rotation()),
            underground_building_pipe_overlay_tile(&self.city, x, y, self.city.rotation()),
            self.city.rotation(),
            self.camera_rotation
        );
    }
}
