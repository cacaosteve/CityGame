use crate::renderer::UiSolid;

/// Recolor paper-like dialog surfaces and their neutral ink for dark appearance.
/// Saturated CityGame accents, charts, map colors, and toolbar artwork are untouched.
pub fn apply_dark_window_colors(solids: &mut [UiSolid]) {
    for solid in solids {
        let [r, g, b, a] = solid.color;
        if a < 0.85 {
            continue;
        }
        let high = r.max(g).max(b);
        let low = r.min(g).min(b);
        let spread = high - low;
        let brightness = (r + g + b) / 3.0;
        if brightness > 0.67 && spread < 0.20 {
            solid.color = [0.105, 0.145, 0.17, a];
        } else if brightness < 0.22 && spread < 0.16 {
            solid.color = [0.83, 0.88, 0.90, a];
        } else if brightness > 0.44 && spread < 0.12 {
            solid.color = [0.26, 0.32, 0.35, a];
        }
    }
}
