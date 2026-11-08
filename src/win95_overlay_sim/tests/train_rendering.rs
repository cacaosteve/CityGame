use super::*;

#[test]
fn ambient_train_draw_codes_cover_curves_fallbacks_and_surface_sprites() {
    assert_eq!(
        PE_TRAIN_DRAW_CODES,
        [[0, 1, 2, 7], [1, 2, 3, 4], [2, 3, 4, 5], [7, 4, 5, 6]]
    );
    assert_eq!(pe_train_draw_code(1, 2), 3, "east-to-south curve");
    assert_eq!(pe_train_draw_code(1, 3), 4, "east fallback to west");

    let curve = Win95OverlayRecord {
        kind: XTHG_TRAIN_ENGINE,
        lane: 0,
        target_x: pe_train_draw_code(1, 0),
        ..Default::default()
    };
    assert_eq!(
        curve.ambient_port_sprite_id(),
        Some(crate::win95_overlay::XTHG_TRAIN_SPRITE_BASE + 1),
        "northbound engine retains the east-to-north curve draw code"
    );
}

#[test]
fn ambient_train_draw_train_tables_use_tile_shape_and_idx() {
    let east = Win95OverlayRecord {
        kind: XTHG_TRAIN_ENGINE,
        lane: 2,
        target_x: 2,
        ..Default::default()
    };
    assert_eq!(
        east.ambient_port_sprite(54, None),
        Some((crate::win95_overlay::XTHG_TRAIN_SPRITE_BASE, true)),
        "DrawTrain shape 0x32 remaps east iDX through byte[0x4E8180]"
    );

    let curve = Win95OverlayRecord {
        target_x: 3,
        ..east
    };
    assert_eq!(
        curve.ambient_port_sprite(54, None),
        Some((crate::win95_overlay::XTHG_TRAIN_SPRITE_BASE + 2, false)),
        "southeast curve uses DrawTrain variant 16"
    );
    assert_eq!(
        curve.ambient_port_sprite(45, None),
        Some((crate::win95_overlay::XTHG_TRAIN_SPRITE_BASE, true)),
        "ordinary rail tiles use byte[0x4E8160] directly"
    );
}

#[test]
fn ambient_train_rail_bridge_draw_uses_pe_xbit_variant() {
    let train = Win95OverlayRecord {
        kind: XTHG_TRAIN_ENGINE,
        target_x: 3,
        ..Default::default()
    };
    assert_eq!(
        train.ambient_port_sprite(90, Some(0)),
        Some((crate::win95_overlay::XTHG_TRAIN_SPRITE_BASE, false))
    );
    assert_eq!(
        train.ambient_port_sprite(91, Some(0x02)),
        Some((crate::win95_overlay::XTHG_TRAIN_SPRITE_BASE, true)),
        "DrawTrain portal branch ignores iDX and selects variant 1 from XBIT bit 1"
    );
}

#[test]
fn ambient_train_rail_bridge_draw_uses_pe_water_elevation() {
    let train = Win95OverlayRecord {
        kind: XTHG_TRAIN_ENGINE,
        ..Default::default()
    };
    assert!(train.ambient_train_uses_water_elevation(90));
    assert!(train.ambient_train_uses_water_elevation(91));
    assert!(!train.ambient_train_uses_water_elevation(45));

    let subway = Win95OverlayRecord {
        kind: XTHG_SUBWAY_TRAIN_ENGINE,
        ..Default::default()
    };
    assert!(!subway.ambient_train_uses_water_elevation(90));
}

#[test]
fn ambient_train_draw_train_placement_offsets_match_pe_tables() {
    let straight = Win95OverlayRecord {
        kind: XTHG_TRAIN_ENGINE,
        target_x: 2,
        ..Default::default()
    };
    assert_eq!(straight.ambient_train_draw_offset(46, None), Some((16, 0)));

    let curve = Win95OverlayRecord {
        target_x: 1,
        ..straight
    };
    assert_eq!(
        curve.ambient_train_draw_offset(54, None),
        Some((0, 3)),
        "shape 0x32 selects PE DrawTrain iDX variant 17"
    );
}

#[test]
fn ambient_train_draw_train_skips_pe_bridge_only_tiles() {
    let train = Win95OverlayRecord {
        kind: XTHG_TRAIN_ENGINE,
        target_x: 2,
        ..Default::default()
    };
    assert_eq!(train.ambient_port_sprite(77, None), None);
    assert_eq!(train.ambient_port_sprite(78, None), None);
}
