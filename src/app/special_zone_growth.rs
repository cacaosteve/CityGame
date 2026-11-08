//! Special-zone demand classification, developed-building rules and growth selectors.
//!
//! Keep civilian saved tile counts and the military PlaceTile census as separate
//! inputs. Preserve threshold and selector priority, arithmetic and the no-RNG boundary.

use super::{
    highway_neighbor_connection_tiles_for_city, population_stats_for_city,
    rail_neighbor_connection_tiles_for_city, road_neighbor_connection_tiles_for_city,
    special_zone_building_tile_count_for_city,
};
use crate::city::City;

pub(super) const SEAPORT_DEMAND_MIN_INDUSTRIAL_POPULATION: u32 = 10_000;
pub(super) const AIRPORT_DEMAND_MIN_COMMERCIAL_POPULATION: u32 = 10_000;
pub(super) const AIRPORT_ROAD_CONNECTION_POPULATION_CAPACITY: u32 = 2_000;
pub(super) const SEAPORT_TRANSPORT_CONNECTION_POPULATION_CAPACITY: u32 = 10_000;
pub(super) const AIRPORT_RUNWAY_CROSS_BUILDING_ID: u8 = 222;
pub(super) const AIRPORT_RUNWAY_CROSS_POPULATION_CAPACITY: u32 = 10_000;
pub(super) const SEAPORT_CRANE_BUILDING_ID: u8 = 224;
pub(super) const SEAPORT_CRANE_POPULATION_CAPACITY: u32 = 10_000;

pub(super) const ZONE_MILITARY: u8 = 7;
pub(super) const ZONE_AIRPORT: u8 = 8;
pub(super) const ZONE_SEAPORT: u8 = 9;
pub(super) const MISC_MILITARY_BASE_TYPE_OFFSET: usize = 0x0e4c;
pub(super) const MILITARY_BASE_NONE: u32 = 0;
pub(super) const MILITARY_BASE_DECLINED: u32 = 1;
pub(super) const MILITARY_BASE_ARMY: u32 = 2;
pub(super) const MILITARY_BASE_AIR_FORCE: u32 = 3;
pub(super) const MILITARY_BASE_NAVY: u32 = 4;
pub(super) const MILITARY_BASE_MISSILE_SILOS: u32 = 5;

pub(super) const AIRPORT_DEVELOPMENT_BUILDINGS: [u8; 9] =
    [221, 222, 225, 226, 228, 229, 230, 232, 234];
pub(super) const AIRPORT_LARGE_DEVELOPMENT_BUILDINGS: [u8; 3] = [238, 239, 246];
pub(super) const SEAPORT_DEVELOPMENT_BUILDINGS: [u8; 3] = [223, 224, 227];
pub(super) const SEAPORT_LARGE_DEVELOPMENT_BUILDINGS: [u8; 2] = [240, 242];
/// Win95 `DoArmyBaseGrowth` / `DoAirPortGrowth` / `DoSeaPortGrowth` building IDs.
pub(super) const MILITARY_PARKING_LOT_BUILDING_ID: u8 = 239;
pub(super) const CIV_PARKING_LOT_BUILDING_ID: u8 = 238;
pub(super) const MILITARY_TOP_SECRET_BUILDING_ID: u8 = 241;
pub(super) const CIV_LOADING_BAY_BUILDING_ID: u8 = 240;
pub(super) const MILITARY_HANGAR1_BUILDING_ID: u8 = 232;
pub(super) const CIV_CONTROL_TOWER_BUILDING_ID: u8 = 225;
pub(super) const MILITARY_CONTROL_TOWER_BUILDING_ID: u8 = 226;
pub(super) const MILITARY_RADAR_BUILDING_ID: u8 = 234;
pub(super) const CIV_TARMAC_BUILDING_ID: u8 = 230;
pub(super) const MILITARY_F15B_BUILDING_ID: u8 = 231;
pub(super) const MILITARY_HANGAR2_BUILDING_ID: u8 = 246;
pub(super) const MILITARY_WAREHOUSE_BUILDING_ID: u8 = 227;
pub(super) const MILITARY_CARGO_YARD_BUILDING_ID: u8 = 242;
pub(super) const MILITARY_CRANE_BUILDING_ID: u8 = 224;
pub(super) const MILITARY_AIRPORT_BUILDING1_ID: u8 = 228;
pub(super) const MILITARY_AIRPORT_BUILDING2_ID: u8 = 229;
pub(super) const RUNWAY_BUILDING_ID: u8 = 221;
pub(super) const MILITARY_SILO_BUILDING_ID: u8 = 249;
pub(super) const TILE_AREA_2X2: u32 = 4;

pub(super) fn special_zone_kind_index(zone_id: u8) -> Option<usize> {
    match zone_id {
        ZONE_AIRPORT => Some(0),
        ZONE_SEAPORT => Some(1),
        ZONE_MILITARY => Some(2),
        _ => None,
    }
}

pub(super) fn military_base_type_for_city(city: &City) -> u32 {
    city.pe_w_military_base_type
}

fn airport_demand_threshold_for_city(city: &City) -> u32 {
    AIRPORT_DEMAND_MIN_COMMERCIAL_POPULATION
        .saturating_add(
            road_neighbor_connection_tiles_for_city(city)
                .saturating_mul(AIRPORT_ROAD_CONNECTION_POPULATION_CAPACITY),
        )
        .saturating_add(
            special_zone_building_tile_count_for_city(
                city,
                ZONE_AIRPORT,
                AIRPORT_RUNWAY_CROSS_BUILDING_ID,
            )
            .saturating_mul(AIRPORT_RUNWAY_CROSS_POPULATION_CAPACITY),
        )
}

fn seaport_demand_threshold_for_city(city: &City) -> u32 {
    let industrial_connections = rail_neighbor_connection_tiles_for_city(city)
        .saturating_add(highway_neighbor_connection_tiles_for_city(city));
    SEAPORT_DEMAND_MIN_INDUSTRIAL_POPULATION
        .saturating_add(
            industrial_connections.saturating_mul(SEAPORT_TRANSPORT_CONNECTION_POPULATION_CAPACITY),
        )
        .saturating_add(
            special_zone_building_tile_count_for_city(
                city,
                ZONE_SEAPORT,
                SEAPORT_CRANE_BUILDING_ID,
            )
            .saturating_mul(SEAPORT_CRANE_POPULATION_CAPACITY),
        )
}

pub(super) fn special_zone_has_growth_demand(city: &City, zone_id: u8) -> bool {
    let population = population_stats_for_city(city);
    match zone_id {
        ZONE_AIRPORT => population.commercial_population >= airport_demand_threshold_for_city(city),
        ZONE_SEAPORT => population.industrial_population >= seaport_demand_threshold_for_city(city),
        ZONE_MILITARY => matches!(
            military_base_type_for_city(city),
            MILITARY_BASE_ARMY
                | MILITARY_BASE_AIR_FORCE
                | MILITARY_BASE_NAVY
                | MILITARY_BASE_MISSILE_SILOS
        ),
        _ => false,
    }
}

pub(super) fn special_zone_developed_building(zone_id: u8, building: u8) -> bool {
    match zone_id {
        ZONE_AIRPORT => {
            AIRPORT_DEVELOPMENT_BUILDINGS.contains(&building)
                || AIRPORT_LARGE_DEVELOPMENT_BUILDINGS.contains(&building)
        }
        ZONE_SEAPORT => {
            SEAPORT_DEVELOPMENT_BUILDINGS.contains(&building)
                || SEAPORT_LARGE_DEVELOPMENT_BUILDINGS.contains(&building)
        }
        ZONE_MILITARY => matches!(
            building,
            MILITARY_SILO_BUILDING_ID
                | MILITARY_PARKING_LOT_BUILDING_ID
                | MILITARY_TOP_SECRET_BUILDING_ID
                | MILITARY_HANGAR1_BUILDING_ID
                | MILITARY_HANGAR2_BUILDING_ID
                | MILITARY_CONTROL_TOWER_BUILDING_ID
                | MILITARY_RADAR_BUILDING_ID
                | MILITARY_F15B_BUILDING_ID
                | MILITARY_WAREHOUSE_BUILDING_ID
                | MILITARY_CARGO_YARD_BUILDING_ID
                | MILITARY_CRANE_BUILDING_ID
                | MILITARY_AIRPORT_BUILDING1_ID
                | MILITARY_AIRPORT_BUILDING2_ID
                | RUNWAY_BUILDING_ID
                | AIRPORT_RUNWAY_CROSS_BUILDING_ID
        ),
        _ => false,
    }
}

fn military_flagged_tile_count(city: &City, building: u8) -> u32 {
    // PE DoArmyBaseGrowth / military DoAirPortGrowth @ `0x436310`+ read `0x4ca934`
    // WORD[16] (PlaceTile military census), not a live XBLD×XZON walk.
    u32::from(city.pe_military_tile_count[City::pe_military_tile_count_index(building)])
}

/// PE `cdq; and edx,3; add; sar 2` — signed toward-zero `/4` (non-negative → `n/4`).
/// Not `(n+3)/4`; that misread was corrected against army/AF/navy/train census arms.
pub(super) fn pe_military_count_sar_div4(count: u32) -> u32 {
    count / 4
}

fn military_tile_count_divided(city: &City, building: u8, divisor: u32) -> u32 {
    let divisor = divisor.max(1);
    military_flagged_tile_count(city, building) / divisor
}

fn military_tile_count_multiplied(city: &City, building: u8, mult: u32) -> u32 {
    military_flagged_tile_count(city, building).saturating_mul(mult.max(1))
}

/// Win95 civilian `DoAirPortGrowth` picker (bMilitary = FALSE).
pub(super) fn select_airport_growth_building(city: &City) -> u8 {
    // PE civilian airport arms read `dwTileCount` (`0x4ca4c8`); military tiles live in
    // `0x4ca934` instead, so the WORD[256] runway total is airport-only.
    let runway_units =
        (u32::from(city.pe_dw_tile_count[RUNWAY_BUILDING_ID as usize]).saturating_add(u32::from(
            city.pe_dw_tile_count[AIRPORT_RUNWAY_CROSS_BUILDING_ID as usize],
        ))) / 5;
    let mut selected = RUNWAY_BUILDING_ID;
    if u32::from(city.pe_dw_tile_count[CIV_PARKING_LOT_BUILDING_ID as usize]) / TILE_AREA_2X2
        < runway_units
    {
        selected = CIV_CONTROL_TOWER_BUILDING_ID;
        if u32::from(city.pe_dw_tile_count[CIV_CONTROL_TOWER_BUILDING_ID as usize])
            .saturating_mul(2)
            >= runway_units
        {
            selected = MILITARY_RADAR_BUILDING_ID;
            if u32::from(city.pe_dw_tile_count[MILITARY_RADAR_BUILDING_ID as usize])
                .saturating_mul(2)
                >= runway_units
            {
                selected = CIV_TARMAC_BUILDING_ID;
                if u32::from(city.pe_dw_tile_count[CIV_TARMAC_BUILDING_ID as usize]) >= runway_units
                {
                    selected = MILITARY_AIRPORT_BUILDING1_ID;
                    if u32::from(city.pe_dw_tile_count[MILITARY_AIRPORT_BUILDING1_ID as usize]) / 2
                        >= runway_units
                    {
                        selected = MILITARY_AIRPORT_BUILDING2_ID;
                        if u32::from(city.pe_dw_tile_count[MILITARY_AIRPORT_BUILDING2_ID as usize])
                            / 2
                            >= runway_units
                        {
                            selected = MILITARY_HANGAR2_BUILDING_ID;
                            if u32::from(
                                city.pe_dw_tile_count[MILITARY_HANGAR2_BUILDING_ID as usize],
                            ) / TILE_AREA_2X2
                                >= runway_units
                            {
                                selected = CIV_PARKING_LOT_BUILDING_ID;
                            }
                        }
                    }
                }
            }
        }
    }
    selected
}

/// Win95 civilian `DoSeaPortGrowth` picker (bMilitary = FALSE).
pub(super) fn select_seaport_growth_building(city: &City) -> u8 {
    let crane_count = u32::from(city.pe_dw_tile_count[SEAPORT_CRANE_BUILDING_ID as usize]);
    let mut selected = SEAPORT_CRANE_BUILDING_ID;
    if u32::from(city.pe_dw_tile_count[MILITARY_CARGO_YARD_BUILDING_ID as usize]) / TILE_AREA_2X2
        < crane_count
    {
        selected = CIV_LOADING_BAY_BUILDING_ID;
        if u32::from(city.pe_dw_tile_count[CIV_LOADING_BAY_BUILDING_ID as usize]) / TILE_AREA_2X2
            >= crane_count
        {
            selected = MILITARY_WAREHOUSE_BUILDING_ID;
            if u32::from(city.pe_dw_tile_count[MILITARY_WAREHOUSE_BUILDING_ID as usize]) / 3
                >= crane_count
            {
                selected = MILITARY_CARGO_YARD_BUILDING_ID;
            }
        }
    }
    selected
}

/// Win95 `DoArmyBaseGrowth` / military `DoAirPortGrowth` / `DoSeaPortGrowth` pickers.
pub(super) fn select_military_growth_building(city: &City, base_type: u32) -> Option<u8> {
    match base_type {
        MILITARY_BASE_ARMY => Some(select_army_growth_building(city)),
        MILITARY_BASE_AIR_FORCE => Some(select_military_air_force_growth_building(city)),
        MILITARY_BASE_NAVY => Some(select_military_navy_growth_building(city)),
        _ => None,
    }
}

pub(super) fn military_growth_fallback_building(base_type: u32) -> Option<u8> {
    match base_type {
        MILITARY_BASE_ARMY => Some(MILITARY_HANGAR1_BUILDING_ID),
        MILITARY_BASE_NAVY => Some(MILITARY_WAREHOUSE_BUILDING_ID),
        _ => None,
    }
}

pub(super) fn select_army_growth_building(city: &City) -> u8 {
    // PE DoArmyBaseGrowth army arm @ `0x43630c`–`0x43633d`:
    // `parking = word[3] sar/4` (`cdq; and 3; sar 2`), `hangar1 = word[13]/12`;
    // pick `0xef` if parking<=hangar else `0xe8`.
    let parking_units = pe_military_count_sar_div4(military_flagged_tile_count(
        city,
        MILITARY_PARKING_LOT_BUILDING_ID,
    ));
    let hangar_units = military_tile_count_divided(city, MILITARY_HANGAR1_BUILDING_ID, 12);
    if parking_units <= hangar_units {
        MILITARY_PARKING_LOT_BUILDING_ID
    } else {
        MILITARY_HANGAR1_BUILDING_ID
    }
}

pub(super) fn select_military_air_force_growth_building(city: &City) -> u8 {
    let runway_units = (military_flagged_tile_count(city, RUNWAY_BUILDING_ID).saturating_add(
        military_flagged_tile_count(city, AIRPORT_RUNWAY_CROSS_BUILDING_ID),
    )) / 5;
    let mut selected = RUNWAY_BUILDING_ID;
    if pe_military_count_sar_div4(military_flagged_tile_count(
        city,
        MILITARY_PARKING_LOT_BUILDING_ID,
    )) < runway_units
    {
        selected = MILITARY_CONTROL_TOWER_BUILDING_ID;
        if military_tile_count_multiplied(city, MILITARY_CONTROL_TOWER_BUILDING_ID, 2)
            >= runway_units
        {
            selected = MILITARY_RADAR_BUILDING_ID;
            if military_tile_count_multiplied(city, MILITARY_RADAR_BUILDING_ID, 2) >= runway_units {
                selected = MILITARY_F15B_BUILDING_ID;
                if military_flagged_tile_count(city, MILITARY_F15B_BUILDING_ID) >= runway_units {
                    selected = MILITARY_AIRPORT_BUILDING1_ID;
                    if military_tile_count_divided(city, MILITARY_AIRPORT_BUILDING1_ID, 2)
                        >= runway_units
                    {
                        selected = MILITARY_AIRPORT_BUILDING2_ID;
                        if military_tile_count_divided(city, MILITARY_AIRPORT_BUILDING2_ID, 2)
                            >= runway_units
                        {
                            selected = MILITARY_HANGAR2_BUILDING_ID;
                            if pe_military_count_sar_div4(military_flagged_tile_count(
                                city,
                                MILITARY_HANGAR2_BUILDING_ID,
                            )) >= runway_units
                            {
                                selected = MILITARY_PARKING_LOT_BUILDING_ID;
                            }
                        }
                    }
                }
            }
        }
    }
    selected
}

pub(super) fn select_military_navy_growth_building(city: &City) -> u8 {
    let crane_count = military_flagged_tile_count(city, MILITARY_CRANE_BUILDING_ID);
    let mut selected = MILITARY_CRANE_BUILDING_ID;
    if pe_military_count_sar_div4(military_flagged_tile_count(
        city,
        MILITARY_CARGO_YARD_BUILDING_ID,
    )) < crane_count
    {
        selected = MILITARY_TOP_SECRET_BUILDING_ID;
        if pe_military_count_sar_div4(military_flagged_tile_count(
            city,
            MILITARY_TOP_SECRET_BUILDING_ID,
        )) >= crane_count
        {
            selected = MILITARY_WAREHOUSE_BUILDING_ID;
            if military_tile_count_divided(city, MILITARY_WAREHOUSE_BUILDING_ID, 3) >= crane_count {
                selected = MILITARY_CARGO_YARD_BUILDING_ID;
            }
        }
    }
    selected
}
