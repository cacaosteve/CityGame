pub(super) fn is_water_terrain(terrain: u8) -> bool {
    matches!(terrain, 0x10..=0x1d | 0x30..=0x3e | 0x40..=0x45)
}

pub(super) fn is_decorative_water_terrain(terrain: u8) -> bool {
    matches!(terrain, 0x30..=0x3d | 0x40..=0x45)
}

pub(super) fn is_power_tile(building: u8) -> bool {
    matches!(building, 14..=28 | 67 | 68 | 92)
}

pub(super) fn is_road_tile(building: u8) -> bool {
    matches!(building, 29..=43 | 63..=66 | 69 | 70 | 81..=89)
}

pub(super) fn is_rubble_tile(building: u8) -> bool {
    matches!(building, 1..=4)
}

pub(super) fn is_bridge_tile(building: u8) -> bool {
    // PE DoBudgetBridgeCheck @ `0x435a34`–`0x435a3e`: XBLD `0x51..=0x5c` (81..=92)
    // plus overlaps `0x6a`/`0x6b` @ `0x435a44`/`0x435a4e`. Do not stop at `0x5b`.
    matches!(building, 81..=92 | 106 | 107)
}

pub(super) fn is_tunnel_tile(building: u8) -> bool {
    matches!(building, 63..=66)
}

pub(super) fn is_rail_tile(building: u8) -> bool {
    matches!(building, 44..=62 | 71 | 72 | 90 | 91 | 108..=111)
}

pub(super) fn is_highway_tile(building: u8) -> bool {
    matches!(building, 73..=80 | 93..=107)
}

pub(super) fn is_surface_infrastructure_tile(building: u8) -> bool {
    is_power_tile(building)
        || is_road_tile(building)
        || is_rail_tile(building)
        || is_highway_tile(building)
}
