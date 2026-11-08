use super::*;

#[test]
fn pe_terrain_mask_follows_position_angle_table() {
    assert_eq!(win95_pe_terrain_mask_for_rotation(0), 0x80);
    assert_eq!(win95_pe_terrain_mask_for_rotation(1), 0x10);
    assert_eq!(win95_pe_terrain_mask_for_rotation(2), 0x20);
    assert_eq!(win95_pe_terrain_mask_for_rotation(3), 0x40);
    assert_eq!(win95_pe_terrain_mask_for_rotation(4), 0x80);
}

#[test]
fn pe_terrain_sprite_requires_matching_xter_high_nibble() {
    assert_eq!(
        win95_pe_terrain_sprite_for_mask(0x80, 0x83),
        Some(0x514 + 1)
    );
    assert_eq!(win95_pe_terrain_sprite_for_mask(0x80, 0x13), None);
}
