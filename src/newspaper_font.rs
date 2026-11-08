//! Bundled serif glyphs for newspaper and budget documents. No runtime system font is needed.
use crate::assets::{AtlasFrame, AtlasSource};
use crate::renderer::UiImage;
use serde::Deserialize;
use std::sync::OnceLock;

pub(crate) const ATLAS_ID: u32 = 80_000;

#[derive(Deserialize)]
struct Glyph {
    code: u32,
    x: u32,
    y: u32,
    w: u32,
    h: u32,
    left: f32,
    top: f32,
    advance: f32,
}

fn glyphs() -> &'static [Glyph] {
    static GLYPHS: OnceLock<Vec<Glyph>> = OnceLock::new();
    GLYPHS.get_or_init(|| {
        serde_json::from_str(include_str!("../assets/ui/newspaper/serif.json"))
            .expect("bundled newspaper type metrics")
    })
}

fn glyph(ch: char) -> &'static Glyph {
    let list = glyphs();
    let index = list
        .binary_search_by_key(&(ch as u32), |g| g.code)
        .unwrap_or_else(|_| {
            list.binary_search_by_key(&u32::from(b'?'), |g| g.code)
                .unwrap()
        });
    &list[index]
}

// The page's existing type scale corresponds to about seven points per design unit.
fn em(size: f32) -> f32 {
    size * 7.2
}

pub(crate) fn width(value: &str, size: f32) -> f32 {
    value.chars().map(|ch| glyph(ch).advance).sum::<f32>() * em(size) / 96.0
}

pub(crate) fn atlas() -> anyhow::Result<AtlasSource> {
    let rgba =
        image::load_from_memory(include_bytes!("../assets/ui/newspaper/serif.png"))?.to_rgba8();
    let (w, h) = rgba.dimensions();
    Ok(AtlasSource::from_rgba(
        "newspaper_type",
        rgba,
        vec![AtlasFrame {
            file: "newspaper/serif.png".to_owned(),
            index: 0,
            id: ATLAS_ID,
            source_offset: 0,
            x: 0,
            y: 0,
            w,
            h,
            logical_size: None,
            anchor: None,
        }],
    ))
}

pub(crate) fn draw(
    images: &mut Vec<UiImage>,
    atlas: usize,
    value: &str,
    x: f32,
    y: f32,
    size: f32,
    color: [f32; 4],
    depth: f32,
) {
    let scale = em(size) / 96.0;
    let baseline = y + em(size) * 0.72;
    let mut cursor = x;
    for ch in value.chars() {
        let g = glyph(ch);
        if g.w > 0 && g.h > 0 {
            images.push(UiImage {
                atlas,
                src_x: g.x,
                src_y: g.y,
                src_w: g.w,
                src_h: g.h,
                x: cursor + g.left * scale,
                y: baseline + g.top * scale,
                w: g.w as f32 * scale,
                h: g.h as f32 * scale,
                depth,
                tint: color,
            });
        }
        cursor += g.advance * scale;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn newspaper_type_metrics_fit_atlas_and_cover_currency_and_percent() {
        let source = atlas().unwrap();
        let image = source.load_rgba().unwrap();
        for g in glyphs() {
            assert!(g.x + g.w <= image.width() && g.y + g.h <= image.height());
        }
        for ch in ['$', '%', 'é', '–', '’'] {
            assert_eq!(glyph(ch).code, ch as u32);
        }
        assert_ne!(width("iii", 2.0), width("WWW", 2.0));
    }
}
