/// Choose one of the eight Mac isometric directions from a tile to a target.
///
/// This small geometry helper is shared by Mac budget placement and the
/// test-only actor lifecycle model; it does not implement or render traffic.
pub(crate) fn mac_direction_to_tile(x: usize, y: usize, target_x: usize, target_y: usize) -> u8 {
    let dx = target_x as i16 - x as i16;
    let dy = target_y as i16 - y as i16;
    let abs_x = dx.abs();
    let abs_y = dy.abs();

    if abs_x < ((abs_y + 1) / 2) {
        if dy < 0 { 0 } else { 4 }
    } else if dx < 0 {
        if abs_y < ((abs_x + 1) / 2) {
            6
        } else if dy < 0 {
            7
        } else {
            5
        }
    } else if abs_y < ((abs_x + 1) / 2) {
        2
    } else if dy < 0 {
        1
    } else {
        3
    }
}
