use super::*;

#[test]
fn mac_export80_special_branches_keep_their_68k_elevation_sources() {
    assert_eq!(mac_export80_special_stamp_water_from_xbit(221), Some(false));
    assert_eq!(mac_export80_special_stamp_water_from_xbit(222), Some(false));
    assert_eq!(mac_export80_special_stamp_water_from_xbit(223), Some(true));
    assert_eq!(mac_export80_special_stamp_water_from_xbit(224), Some(true));
    assert_eq!(mac_export80_special_stamp_water_from_xbit(29), None);
}

#[test]
fn mac_export80_special_branches_are_not_ordinary_roads() {
    for building in 221..=224 {
        assert!(mac_export80_special_stamp_building(building));
        assert!(mac_export80_stamp_building(building));
        assert!(!mac_road_carries_traffic(building));
    }
}

#[test]
fn mac_export80_ordinary_road_branch_covers_full_recovered_ranges() {
    for building in 81..=92 {
        assert!(mac_export80_ordinary_road_building(building));
        assert!(mac_export80_stamp_building(building));
    }
    assert!(mac_export80_ordinary_road_building(106));
    assert!(mac_export80_ordinary_road_building(107));
    assert!(!mac_export80_ordinary_road_building(80));
    assert!(!mac_export80_ordinary_road_building(93));
}
