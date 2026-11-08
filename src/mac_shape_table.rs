//! Mac `ffffb302` shape offset table (8 bytes per sprite).
//!
//! Records are derived from SPRT manifest bounds; see
//! `outputs/disasm_probe/mac_shape_table.json` and `mac_traffic_trace_notes.md`.
//!
//! `export_80` traffic blit @ `0x6cd6` subtracts `record.y_sub` from screen Y.
//! `export_156` query icons halve `y_sub` / `x_sub` before blit.

#[path = "mac_shape_table_data.rs"]
mod mac_shape_table_data;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MacShapeRecord {
    pub width: u32,
    pub height: u32,
    /// Table offset +4 — subtracted from screen Y before `jsr 0x55a`.
    pub y_sub: f32,
    /// Table offset +6 — subtracted (halved in query UI) for X placement.
    pub x_sub: f32,
}

/// Lookup shape-table entry for traffic / small overlay sprites.
pub fn lookup(sprite_id: u32) -> Option<MacShapeRecord> {
    mac_shape_table_data::lookup_generated(sprite_id)
}

/// Mac kind-1's CODE 6 redirect reads the base-zoom SPRT height at
/// `ffffb302 + 8*XBLD + 4` and compares record `+5` with `height / 3`.
/// Keep this simulation input embedded: the original runtime builds the table
/// from imported SPRT resources, but actor behavior must not require graphics
/// imports to be present.
pub(crate) fn kind1_redirect_height_threshold(xbld: u8) -> Option<u8> {
    // Extracted from the native Mac 1.2 SPRT_00128 descriptors. The IDs are
    // dense from 0x70 through 0xfa; the handler's comparison gate itself only
    // reaches 0x71..=0xfa. See the adjacent 2026-10-31 CSV evidence.
    const HEIGHTS: [u8; 0x8b] = [
        5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 6, 8, 7, 6, 6, 8, 12, 7, 9, 5, 8, 6, 7, 5, 5, 5, 5, 10,
        11, 11, 11, 16, 14, 20, 20, 9, 11, 11, 12, 12, 14, 19, 17, 19, 22, 11, 11, 11, 10, 10, 14,
        15, 16, 10, 12, 12, 17, 14, 15, 18, 19, 18, 23, 15, 24, 16, 22, 17, 24, 16, 30, 35, 20, 28,
        38, 13, 24, 15, 15, 15, 13, 21, 17, 18, 24, 9, 9, 11, 23, 29, 21, 20, 24, 18, 28, 18, 21,
        19, 17, 15, 14, 15, 22, 19, 23, 17, 13, 5, 5, 5, 6, 11, 16, 18, 5, 6, 6, 5, 5, 6, 6, 5, 17,
        10, 11, 10, 10, 10, 14, 9, 10, 10, 14, 10, 13, 14, 13, 16,
    ];
    let index = usize::from(xbld.checked_sub(0x70)?);
    HEIGHTS.get(index).map(|height| height / 3)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn traffic_dash_records_match_manifest() {
        let r400 = lookup(400).expect("400");
        assert_eq!(r400.width, 8);
        assert_eq!(r400.height, 4);
        assert_eq!(r400.y_sub, 4.0);
        assert_eq!(r400.x_sub, 4.0);
    }

    #[test]
    fn curve_traffic_differs_by_height() {
        let r402 = lookup(402).expect("402");
        assert_eq!(r402.y_sub, 7.0);
    }

    #[test]
    fn kind1_sprite_height_threshold_covers_native_xbld_range() {
        for xbld in 0x70..=0xfa {
            assert!(kind1_redirect_height_threshold(xbld).is_some());
        }
        assert_eq!(kind1_redirect_height_threshold(0x6f), None);
        assert_eq!(kind1_redirect_height_threshold(0x70), Some(1));
        assert_eq!(kind1_redirect_height_threshold(0x81), Some(4));
        assert_eq!(kind1_redirect_height_threshold(0xfa), Some(5));
        assert_eq!(kind1_redirect_height_threshold(0xfb), None);
    }
}
