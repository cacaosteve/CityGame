//! CityGame's own newspaper composition. Imported pictures are editorial illustrations;
//! the column ornaments are deterministic typesetting, not recovered original prose.
use super::*;
use crate::newspaper_font as font;

const PAPER: [f32; 4] = [0.91, 0.87, 0.80, 1.0];
const INK: [f32; 4] = [0.008, 0.010, 0.009, 1.0];
const MUTED: [f32; 4] = [0.12, 0.12, 0.105, 1.0];

pub(super) struct Content<'a> {
    pub masthead: &'a str,
    pub headline: &'a str,
    pub story: &'a str,
    pub date: &'a str,
    pub edition: &'a str,
    pub weather: &'a str,
    pub statistics: &'a [(&'a str, String)],
    pub picture: Option<(usize, [u32; 4])>,
    pub font_atlas: Option<usize>,
}

pub(super) struct PeTailText {
    pub weather_label: &'static str,
    pub opinion_label: &'static str,
    pub weather_line: String,
    pub opinion_lines: Vec<String>,
}

pub(super) fn page_rect(window: (u32, u32)) -> FloatRect {
    let scale = ((window.0 as f32 - 32.0).max(1.0) / 950.0)
        .min((window.1 as f32 - 32.0).max(1.0) / 640.0)
        .min(1.7);
    let (w, h) = (950.0 * scale, 640.0 * scale);
    FloatRect {
        x: (window.0 as f32 - w) * 0.5,
        y: (window.1 as f32 - h) * 0.5,
        w,
        h,
    }
}

pub(super) fn close_rect(page: FloatRect) -> FloatRect {
    let s = (page.w / 950.0).min(1.0);
    FloatRect {
        x: page.x + page.w - 104.0 * s,
        y: page.y + 10.0 * s,
        w: 82.0 * s,
        h: 30.0 * s,
    }
}

/// Fit the original Win95 paper's 640×400 client area inside the available
/// CityGame panel. Both drawing and hit testing use this same viewport.
pub(super) fn pe_paper_rect(page: FloatRect) -> FloatRect {
    let scale = (page.w / 640.0).min(page.h / 400.0);
    let (w, h) = (640.0 * scale, 400.0 * scale);
    FloatRect {
        x: page.x + (page.w - w) * 0.5,
        y: page.y + (page.h - h) * 0.5,
        w,
        h,
    }
}

pub(super) fn pe_layout_rect(
    page: FloatRect,
    rect: crate::win95_newspaper::NewspaperRect,
) -> FloatRect {
    let paper = pe_paper_rect(page);
    FloatRect {
        x: paper.x + rect.left as f32 * paper.w / 640.0,
        y: paper.y + rect.top as f32 * paper.h / 400.0,
        w: (rect.right - rect.left) as f32 * paper.w / 640.0,
        h: (rect.bottom - rect.top) as f32 * paper.h / 400.0,
    }
}

pub(super) fn pe_story_rect(page: FloatRect, style: u8, position: u16) -> Option<FloatRect> {
    crate::win95_newspaper::pe_newspaper_layout_rect(style, position)
        .map(|rect| pe_layout_rect(page, rect))
}

pub(super) fn pe_close_rect(page: FloatRect) -> FloatRect {
    let paper = pe_paper_rect(page);
    let scale = paper.w / 640.0;
    FloatRect {
        x: paper.x + 4.0 * scale,
        y: paper.y + 3.0 * scale,
        w: 16.0 * scale,
        h: 16.0 * scale,
    }
}

pub(super) fn pe_hit_positions(page: FloatRect, style: u8, point: Vec2) -> Vec<u16> {
    let paper = pe_paper_rect(page);
    if point.x < paper.x
        || point.y < paper.y
        || point.x >= paper.x + paper.w
        || point.y >= paper.y + paper.h
    {
        return Vec::new();
    }
    let x = ((point.x - paper.x) * 640.0 / paper.w).floor() as i32;
    let y = ((point.y - paper.y) * 400.0 / paper.h).floor() as i32;
    crate::win95_newspaper::pe_newspaper_hit_positions(style, x, y)
}

/// Draw the recovered PE layout and filler; GDI fonts and window chrome are
/// approximated until their native measurements are available.
pub(super) fn draw_pe_paper(
    page: FloatRect,
    paint: &crate::win95_newspaper::PeNewspaperPaint,
    masthead: &str,
    date: &str,
    price: Option<&str>,
    tail_text: Option<&PeTailText>,
    picture: Option<(usize, [u32; 4])>,
    font_atlas: Option<usize>,
    solids: &mut Vec<UiSolid>,
    images: &mut Vec<UiImage>,
    depth: f32,
) {
    let paper = pe_paper_rect(page);
    let scale = paper.w / 640.0;
    let paper_gray = 192.0 / 255.0;
    push_ui_rect(
        solids,
        paper,
        [paper_gray, paper_gray, paper_gray, 1.0],
        depth + 0.1,
    );
    let ink_depth = depth + 0.3;
    let draw_line = |bytes: &[u8],
                     native: crate::win95_newspaper::NewspaperRect,
                     native_size: f32,
                     align: u8,
                     solids: &mut Vec<UiSolid>,
                     images: &mut Vec<UiImage>| {
        let rect = pe_layout_rect(page, native);
        let text = String::from_utf8_lossy(bytes);
        let size = (native_size * scale / 7.2)
            .min((rect.w - 4.0 * scale).max(0.0) / font::width(&text, 1.0).max(1.0));
        let width = font::width(&text, size);
        let x = match align {
            1 => rect.x + (rect.w - width) * 0.5,
            2 => rect.x + rect.w - width - 2.0 * scale,
            _ => rect.x + 2.0 * scale,
        };
        if let Some(atlas) = font_atlas {
            font::draw(images, atlas, &text, x, rect.y, size, INK, ink_depth);
        } else {
            ui::push_pixel_text_preserving_case(
                solids,
                &text,
                x,
                rect.y,
                (size * 1.1).min(1.5 * scale),
                INK,
                ink_depth,
            );
        }
    };
    let rect = |position| crate::win95_newspaper::pe_newspaper_layout_rect(paint.style, position);
    if let Some(r) = rect(0) {
        draw_line(masthead.as_bytes(), r, 22.0, 1, solids, images);
    }
    if let Some(r) = rect(1) {
        draw_line(
            date.as_bytes(),
            r,
            9.0,
            if paint.style == 1 { 2 } else { 0 },
            solids,
            images,
        );
    }
    if let (Some(price), Some(r)) = (price, rect(2)) {
        draw_line(
            price.as_bytes(),
            r,
            9.0,
            if paint.style == 1 { 0 } else { 2 },
            solids,
            images,
        );
    }
    if let Some(r) = rect(4) {
        let headline = crate::win95_oem::decode_oem(&paint.lead.headline);
        draw_line(
            headline.as_bytes(),
            r,
            if paint.style == 2 { 10.0 } else { 24.0 },
            u8::from(paint.style != 2),
            solids,
            images,
        );
    }
    for story in &paint.secondary {
        if let Some(r) = rect((story.story_row + 6) as u16) {
            let headline = crate::win95_oem::decode_oem(&story.headline);
            draw_line(headline.as_bytes(), r, 10.0, 1, solids, images);
        }
    }
    for section in &paint.tail {
        let position = if section.story_row == 8 { 5 } else { 6 };
        if let (Some(headline), Some(mut r)) = (&section.headline, rect(position)) {
            let label = if section.story_row == 7 {
                tail_text.map_or("Weather", |text| text.weather_label)
            } else {
                tail_text.map_or("Survey", |text| text.opinion_label)
            };
            draw_line(label.as_bytes(), r, 10.0, 0, solids, images);
            r.top += 15;
            let headline = crate::win95_oem::decode_oem(headline);
            draw_line(headline.as_bytes(), r, 10.0, 0, solids, images);
            if section.story_row == 7 {
                if let Some(text) = tail_text {
                    r.top += 15;
                    draw_line(text.weather_line.as_bytes(), r, 10.0, 0, solids, images);
                }
            } else if let Some(text) = tail_text {
                for line in &text.opinion_lines {
                    r.top += 15;
                    draw_line(line.as_bytes(), r, 10.0, 0, solids, images);
                }
            }
        }
    }
    for bar in paint
        .lead
        .bars
        .iter()
        .chain(paint.secondary.iter().flat_map(|story| story.bars.iter()))
        .chain(paint.tail.iter().flat_map(|section| section.bars.iter()))
    {
        let rect = pe_layout_rect(page, *bar);
        push_ui_rect(solids, rect, INK, ink_depth);
        // GDI Rectangle uses a one-pixel black pen and white brush here.
        // Keep the pen one native pixel wide as the paper viewport scales.
        if rect.w > 2.0 * scale && rect.h > 2.0 * scale {
            push_ui_rect(
                solids,
                FloatRect {
                    x: rect.x + scale,
                    y: rect.y + scale,
                    w: rect.w - 2.0 * scale,
                    h: rect.h - 2.0 * scale,
                },
                [1.0; 4],
                ink_depth + 0.01,
            );
        }
    }
    if paint.style != 2 {
        if let (Some((atlas, frame)), Some(r)) = (
            picture.filter(|(_, frame)| frame[2] != 0 && frame[3] != 0),
            rect(3),
        ) {
            let bounds = pe_layout_rect(page, r);
            let factor = (bounds.w / frame[2] as f32).min(bounds.h / frame[3] as f32);
            let (w, h) = (frame[2] as f32 * factor, frame[3] as f32 * factor);
            images.push(UiImage {
                atlas,
                src_x: frame[0],
                src_y: frame[1],
                src_w: frame[2],
                src_h: frame[3],
                x: bounds.x + (bounds.w - w) * 0.5,
                y: bounds.y + (bounds.h - h) * 0.5,
                w,
                h,
                depth: ink_depth,
                tint: [1.0; 4],
            });
        }
    }
    let close = pe_close_rect(page);
    push_ui_rect(solids, close, [0.84, 0.84, 0.84, 1.0], ink_depth + 0.1);
    push_ui_bevel(solids, close, scale.max(0.5), false, ink_depth + 0.2);
    ui::push_pixel_text(
        solids,
        "X",
        close.x + 4.0 * scale,
        close.y + 3.0 * scale,
        scale,
        INK,
        ink_depth + 0.3,
    );
}

/// The native CODE 7 article window is anchored in its 512×320 paper.
pub(super) fn detail_rect(page: FloatRect) -> FloatRect {
    area(page, 289.5, 42.0, 371.0, 556.0)
}

pub(super) fn lead_hit_rect(page: FloatRect) -> FloatRect {
    area(page, 28.0, 120.0, 894.0, 82.0)
}

fn mac_area(page: FloatRect, rect: crate::mac_newspaper::PaperRect) -> FloatRect {
    let crate::mac_newspaper::PaperRect(top, left, bottom, right) = rect;
    FloatRect {
        x: page.x + f32::from(left) * page.w / 512.0,
        y: page.y + f32::from(top) * page.h / 320.0,
        w: f32::from(right - left) * page.w / 512.0,
        h: f32::from(bottom - top) * page.h / 320.0,
    }
}

pub(super) fn mac_lead_hit_rect(page: FloatRect, family: usize) -> FloatRect {
    crate::mac_newspaper::lead_story_rect(family)
        .map_or_else(|| lead_hit_rect(page), |r| mac_area(page, r))
}

pub(super) fn mac_story_hit_rect(
    page: FloatRect,
    family: usize,
    position: usize,
) -> Option<FloatRect> {
    crate::mac_newspaper::story_rect(family, position).map(|r| mac_area(page, r))
}

pub(super) fn mac_close_rect(page: FloatRect) -> FloatRect {
    mac_area(page, crate::mac_newspaper::PaperRect(3, 3, 15, 15))
}

/// Draw the composed Mac paper using its recovered 512×320 geometry. The
/// bundled serif remains a substitute for the original QuickDraw glyphs.
pub(super) fn draw_mac_paper(
    page: FloatRect,
    paint: &super::newspaper_state::MacPaperPaint,
    masthead: &str,
    date: &str,
    picture: Option<(usize, [u32; 4])>,
    font_atlas: Option<usize>,
    solids: &mut Vec<UiSolid>,
    images: &mut Vec<UiImage>,
    depth: f32,
) {
    use crate::mac_newspaper::{PaperHeadlineLine, PaperRect};
    push_ui_rect(solids, page, [1.0, 1.0, 1.0, 1.0], depth + 0.1);
    let sx = page.w / 512.0;
    let sy = page.h / 320.0;
    let ink_depth = depth + 0.3;
    let line = |item: &PaperHeadlineLine,
                centered: bool,
                solids: &mut Vec<UiSolid>,
                images: &mut Vec<UiImage>| {
        let r = mac_area(page, item.rect);
        let text = crate::city::mac_roman::decode(&item.text);
        let mut size = f32::from(item.size) * sy / 7.2;
        size = size.min((r.w - 4.0 * sx).max(0.0) / font::width(&text, 1.0).max(1.0));
        let x = if centered {
            r.x + (r.w - font::width(&text, size)) * 0.5
        } else {
            r.x + 2.0 * sx
        };
        let y = r.y + f32::from(item.size) * sy * 0.28;
        if let Some(atlas) = font_atlas {
            font::draw(images, atlas, &text, x, y, size, INK, ink_depth);
        } else {
            ui::push_pixel_text_preserving_case(
                solids,
                &text,
                x,
                y,
                (size * 1.1).min(1.5 * sy),
                INK,
                ink_depth,
            );
        }
    };
    let (masthead_box, masthead_size) = match paint.family {
        0 => (PaperRect(0, 0, 36, 512), 24),
        1 => (PaperRect(30, 0, 57, 512), 18),
        _ => (PaperRect(0, 0, 30, 307), 24),
    };
    for y in [masthead_box.0 + 2, masthead_box.2] {
        let rule = PaperRect(y, masthead_box.1 + 3, y + 1, masthead_box.3 - 3);
        push_ui_rect(solids, mac_area(page, rule), INK, ink_depth);
    }
    let title = PaperHeadlineLine {
        text: crate::city::mac_roman::encode(masthead, 255),
        size: masthead_size,
        rect: masthead_box,
    };
    line(&title, true, solids, images);
    // The date is positioned inside the masthead band; formatting remains
    // the game's existing calendar presentation.
    let date_rect = match paint.family {
        1 => PaperRect(46, 341, 57, 512),
        _ => PaperRect(24, 0, 36, 170),
    };
    line(
        &PaperHeadlineLine {
            text: crate::city::mac_roman::encode(date, 255),
            size: 9,
            rect: date_rect,
        },
        false,
        solids,
        images,
    );
    let centered = paint.family != 2;
    for item in &paint.lead.headline {
        line(item, centered, solids, images);
    }
    for story in &paint.stories {
        for item in &story.layout.headline {
            line(item, centered, solids, images);
        }
    }
    for item in &paint.weather.lines {
        line(item, paint.family == 1, solids, images);
    }
    for item in &paint.survey.lines {
        line(item, paint.family == 1, solids, images);
    }
    for rect in paint
        .lead
        .story_filler
        .iter()
        .chain(&paint.lead.picture_filler)
        .chain(
            paint
                .stories
                .iter()
                .flat_map(|story| story.layout.filler.iter()),
        )
        .chain(&paint.weather.filler)
        .chain(&paint.survey.filler)
    {
        // QuickDraw fills these three-pixel lines with a halftone pattern.
        // A gray fill preserves its apparent weight until patterned UI
        // primitives are available.
        push_ui_rect(
            solids,
            mac_area(page, *rect),
            [0.28, 0.28, 0.28, 1.0],
            ink_depth,
        );
    }
    if paint.lead.picture_filler.is_empty() {
        if let Some((atlas, frame)) = picture {
            let box_rect = mac_area(page, paint.lead.picture_rect);
            let w = (frame[2] as f32 * sx).min(box_rect.w);
            let h = (frame[3] as f32 * sy).min(box_rect.h);
            images.push(UiImage {
                atlas,
                src_x: frame[0],
                src_y: frame[1],
                src_w: frame[2],
                src_h: frame[3],
                x: box_rect.x + (box_rect.w - w) * 0.5,
                y: box_rect.y + (box_rect.h - h) * 0.5,
                w,
                h,
                depth: ink_depth,
                tint: [1.0; 4],
            });
        }
    }
    let close = mac_close_rect(page);
    push_ui_rect(solids, close, INK, ink_depth + 0.1);
    push_ui_rect(
        solids,
        FloatRect {
            x: close.x + sx,
            y: close.y + sy,
            w: (close.w - 2.0 * sx).max(0.0),
            h: (close.h - 2.0 * sy).max(0.0),
        },
        [1.0; 4],
        ink_depth + 0.2,
    );
}

pub(super) fn detail_close_rect(page: FloatRect) -> FloatRect {
    area(page, 298.0, 51.0, 25.0, 25.0)
}

pub(super) fn detail_scroll_rect(page: FloatRect, down: bool) -> FloatRect {
    area(page, 625.0, if down { 559.0 } else { 529.0 }, 25.0, 25.0)
}

const PE_DETAIL_VISIBLE_LINES: usize = 22;
fn pe_detail_lines(article: &str) -> Vec<String> {
    let mut result = Vec::new();
    for paragraph in article.split('\r') {
        let wrapped = lines(paragraph, 400.0, 2.1);
        if wrapped.is_empty() {
            result.push(String::new());
        } else {
            result.extend(wrapped);
        }
    }
    result
}

pub(super) fn pe_detail_scroll_max(article: &str) -> usize {
    pe_detail_lines(article)
        .len()
        .saturating_sub(PE_DETAIL_VISIBLE_LINES)
}

/// The Win95 dialog is centered over the paper and grows with wrapped text.
/// Native GDI font metrics and exact dialog sizing remain to be recovered.
pub(super) fn pe_detail_rect(page: FloatRect, article: &str) -> FloatRect {
    let paper = pe_paper_rect(page);
    let scale = paper.w / 640.0;
    let rows = pe_detail_lines(article).len().min(PE_DETAIL_VISIBLE_LINES);
    let h = ((rows as f32 + 2.0) * 16.0 + 78.0).clamp(140.0, 430.0) * scale;
    let w = 444.0 * scale;
    FloatRect {
        x: paper.x + (paper.w - w) * 0.5,
        y: paper.y + (paper.h - h) * 0.5,
        w,
        h,
    }
}

pub(super) fn pe_detail_close_rect(page: FloatRect, article: &str) -> FloatRect {
    let rect = pe_detail_rect(page, article);
    let scale = pe_paper_rect(page).w / 640.0;
    FloatRect {
        x: rect.x + 6.0 * scale,
        y: rect.y + 6.0 * scale,
        w: 27.0 * scale,
        h: 21.0 * scale,
    }
}

pub(super) fn pe_detail_scroll_rect(page: FloatRect, article: &str, down: bool) -> FloatRect {
    let rect = pe_detail_rect(page, article);
    let scale = pe_paper_rect(page).w / 640.0;
    FloatRect {
        x: rect.x + rect.w - 26.0 * scale,
        y: rect.y + rect.h - (if down { 24.0 } else { 48.0 }) * scale,
        w: 20.0 * scale,
        h: 20.0 * scale,
    }
}

pub(super) fn draw_pe_detail(
    page: FloatRect,
    headline: &str,
    article: &str,
    scroll: usize,
    font_atlas: Option<usize>,
    solids: &mut Vec<UiSolid>,
    images: &mut Vec<UiImage>,
    depth: f32,
) {
    let rect = pe_detail_rect(page, article);
    let scale = pe_paper_rect(page).w / 640.0;
    let gray = 192.0 / 255.0;
    push_ui_rect(solids, rect, [gray, gray, gray, 1.0], depth);
    push_ui_bevel(solids, rect, scale.max(0.5), false, depth + 0.1);
    let close = pe_detail_close_rect(page, article);
    push_ui_rect(solids, close, [0.85, 0.85, 0.85, 1.0], depth + 0.2);
    push_ui_bevel(solids, close, scale.max(0.5), false, depth + 0.3);
    ui::push_pixel_text(
        solids,
        "OK",
        close.x + 4.0 * scale,
        close.y + 4.0 * scale,
        scale,
        INK,
        depth + 0.4,
    );
    let body = pe_detail_lines(article);
    let scroll = scroll.min(body.len().saturating_sub(PE_DETAIL_VISIBLE_LINES));
    let draw =
        |value: &str, x: f32, y: f32, solids: &mut Vec<UiSolid>, images: &mut Vec<UiImage>| {
            if let Some(atlas) = font_atlas {
                font::draw(images, atlas, value, x, y, 2.1 * scale, INK, depth + 0.3);
            } else {
                ui::push_pixel_text_preserving_case(solids, value, x, y, scale, INK, depth + 0.3);
            }
        };
    draw(
        &format!("{headline} -"),
        rect.x + 22.0 * scale,
        rect.y + 50.0 * scale,
        solids,
        images,
    );
    for (row, line) in body
        .iter()
        .skip(scroll)
        .take(PE_DETAIL_VISIBLE_LINES)
        .enumerate()
    {
        draw(
            line,
            rect.x + 22.0 * scale,
            rect.y + (82.0 + row as f32 * 16.0) * scale,
            solids,
            images,
        );
    }
    if pe_detail_scroll_max(article) > 0 {
        for down in [false, true] {
            let button = pe_detail_scroll_rect(page, article, down);
            push_ui_rect(solids, button, [0.85, 0.85, 0.85, 1.0], depth + 0.2);
            ui::push_pixel_text(
                solids,
                if down { "V" } else { "^" },
                button.x + 5.0 * scale,
                button.y + 4.0 * scale,
                scale,
                INK,
                depth + 0.3,
            );
        }
    }
}

const DETAIL_VISIBLE_LINES: usize = 28;
fn detail_lines(article: &str) -> Vec<String> {
    let mut result = Vec::new();
    for paragraph in article.split('\r') {
        let wrapped = lines(paragraph, 310.0, 1.6);
        if wrapped.is_empty() {
            result.push(String::new());
        } else {
            result.extend(wrapped);
        }
    }
    result
}

pub(super) fn detail_scroll_max(article: &str) -> usize {
    detail_lines(article)
        .len()
        .saturating_sub(DETAIL_VISIBLE_LINES)
}

/// A readable detail overlay using the original window's position. Native
/// QuickDraw typography and TextEdit behavior are still separate work.
pub(super) fn draw_mac_detail(
    page: FloatRect,
    headline: &str,
    article: &str,
    scroll: usize,
    font_atlas: Option<usize>,
    solids: &mut Vec<UiSolid>,
    images: &mut Vec<UiImage>,
    depth: f32,
) {
    let rect = detail_rect(page);
    push_ui_rect(solids, rect, [1.0, 1.0, 1.0, 1.0], depth);
    let scale = page.w / 950.0;
    let edge = (1.2 * scale).max(0.5);
    for line in [
        FloatRect { h: edge, ..rect },
        FloatRect {
            y: rect.y + rect.h - edge,
            h: edge,
            ..rect
        },
        FloatRect { w: edge, ..rect },
        FloatRect {
            x: rect.x + rect.w - edge,
            w: edge,
            ..rect
        },
    ] {
        push_ui_rect(solids, line, INK, depth + 0.1);
    }
    let close = detail_close_rect(page);
    push_ui_rect(solids, close, [0.85, 0.85, 0.85, 1.0], depth + 0.1);
    ui::push_pixel_text(
        solids,
        "X",
        close.x + 7.0 * scale,
        close.y + 7.0 * scale,
        1.5 * scale,
        INK,
        depth + 0.2,
    );
    let mut runs = Vec::new();
    fitted(
        &mut runs,
        page,
        headline,
        332.0,
        52.0,
        278.0,
        2.1,
        true,
        depth + 0.2,
    );
    let body = detail_lines(article);
    let start = scroll.min(body.len().saturating_sub(DETAIL_VISIBLE_LINES));
    for (row, line) in body
        .iter()
        .skip(start)
        .take(DETAIL_VISIBLE_LINES)
        .enumerate()
    {
        text(
            &mut runs,
            page,
            line,
            306.0,
            96.0 + row as f32 * 16.2,
            1.6,
            INK,
            depth + 0.2,
        );
    }
    for (down, symbol) in [(false, "^"), (true, "V")] {
        let button = detail_scroll_rect(page, down);
        push_ui_rect(solids, button, [0.85, 0.85, 0.85, 1.0], depth + 0.1);
        ui::push_pixel_text(
            solids,
            symbol,
            button.x + 7.0 * scale,
            button.y + 7.0 * scale,
            1.5 * scale,
            INK,
            depth + 0.2,
        );
    }
    for run in runs {
        if let Some(atlas) = font_atlas {
            font::draw(
                images, atlas, &run.value, run.x, run.y, run.size, run.color, run.depth,
            );
        } else {
            ui::push_pixel_text_preserving_case(
                solids,
                &run.value,
                run.x,
                run.y,
                run.size.min(1.0 * scale),
                run.color,
                run.depth,
            );
        }
    }
}

/// A CityGame editorial choice, not a recovered original story-to-picture table.
pub(super) fn picture_for_headline(headline: &str) -> (u32, &'static str) {
    let h = headline.to_ascii_uppercase();
    let has = |words: &[&str]| words.iter().any(|word| h.contains(word));
    if has(&["FIRE", "BLAZE"]) {
        (408, "CITY DESK / EMERGENCY RESPONSE")
    } else if has(&["TORNADO", "HURRICANE", "STORM", "WIND"]) {
        (409, "CITY DESK / WEATHER AND RECOVERY")
    } else if has(&["POWER", "ENERGY", "PLANT", "NUCLEAR"]) {
        (411, "CITY DESK / ENERGY AND INDUSTRY")
    } else if has(&["TRAFFIC", "ROAD", "BRIDGE", "TRANSIT"]) {
        (401, "CITY DESK / GETTING AROUND")
    } else if has(&["STADIUM", "SPORT", "TEAM", "PLAYOFF"]) {
        (410, "CITY DESK / COMMUNITY SPORTS")
    } else if has(&["INDUSTR", "EMPLOY", "JOBS", "FACTORY"]) {
        (405, "CITY DESK / THE LOCAL ECONOMY")
    } else if has(&["PROTEST", "TAX", "APPROVAL", "MAYOR"]) {
        (403, "CITY DESK / THE PUBLIC VOICE")
    } else if has(&["WATER", "FLOOD", "TREATMENT"]) {
        (402, "CITY DESK / ESSENTIAL SERVICES")
    } else {
        (404, "CITY DESK / CIVIC AFFAIRS")
    }
}

fn area(page: FloatRect, x: f32, y: f32, w: f32, h: f32) -> FloatRect {
    let s = page.w / 950.0;
    FloatRect {
        x: page.x + x * s,
        y: page.y + y * s,
        w: w * s,
        h: h * s,
    }
}

struct TypeRun {
    value: String,
    x: f32,
    y: f32,
    size: f32,
    color: [f32; 4],
    depth: f32,
}

fn text(
    runs: &mut Vec<TypeRun>,
    page: FloatRect,
    value: &str,
    x: f32,
    y: f32,
    size: f32,
    color: [f32; 4],
    depth: f32,
) {
    let p = area(page, x, y, 0.0, 0.0);
    runs.push(TypeRun {
        value: value.to_owned(),
        x: p.x,
        y: p.y,
        size: size * page.w / 950.0,
        color,
        depth,
    });
}

fn fitted(
    runs: &mut Vec<TypeRun>,
    page: FloatRect,
    value: &str,
    x: f32,
    y: f32,
    width: f32,
    size: f32,
    centered: bool,
    depth: f32,
) {
    let size = size.min(width / font::width(value, 1.0).max(1.0));
    let x = x + if centered {
        (width - font::width(value, size)) * 0.5
    } else {
        0.0
    };
    text(runs, page, value, x, y, size, INK, depth);
}

fn lines(value: &str, width: f32, size: f32) -> Vec<String> {
    let mut lines = Vec::new();
    let mut line = String::new();
    for word in value.split_whitespace() {
        let candidate = if line.is_empty() {
            word.to_owned()
        } else {
            format!("{line} {word}")
        };
        if font::width(&candidate, size) <= width {
            line = candidate;
        } else {
            if !line.is_empty() {
                lines.push(std::mem::take(&mut line));
            }
            for ch in word.chars() {
                if font::width(&format!("{line}{ch}"), size) > width && !line.is_empty() {
                    lines.push(std::mem::take(&mut line));
                }
                line.push(ch);
            }
        }
    }
    if !line.is_empty() {
        lines.push(line);
    }
    lines
}

fn rule(solids: &mut Vec<UiSolid>, page: FloatRect, x: f32, y: f32, w: f32, h: f32, depth: f32) {
    push_ui_rect(solids, area(page, x, y, w, h), INK, depth);
}

fn column(solids: &mut Vec<UiSolid>, page: FloatRect, rect: FloatRect, mut seed: u32, depth: f32) {
    let mut y = rect.y;
    let mut row = 0;
    while y + 4.0 < rect.y + rect.h {
        seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
        let indent = if row % 7 == 0 { 8.0 } else { 0.0 };
        let width = if row % 7 == 6 {
            rect.w * (0.52 + (seed % 27) as f32 / 100.0)
        } else {
            rect.w
        };
        let mut x = rect.x + indent;
        while x + 7.0 < rect.x + width {
            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            let word = (7.0 + (seed % 23) as f32).min(rect.x + width - x);
            for tick in 0..3 {
                push_ui_rect(
                    solids,
                    area(page, x, y + tick as f32 * 1.25, word, 0.65),
                    MUTED,
                    depth,
                );
            }
            x += word + 3.0;
        }
        row += 1;
        y += if row % 7 == 0 { 12.0 } else { 8.0 };
    }
}

pub(super) fn draw(
    page: FloatRect,
    content: &Content<'_>,
    solids: &mut Vec<UiSolid>,
    images: &mut Vec<UiImage>,
    depth: f32,
) {
    let mut type_runs = Vec::new();
    push_ui_rect(
        solids,
        FloatRect {
            x: page.x + 7.0,
            y: page.y + 9.0,
            ..page
        },
        [0.0, 0.0, 0.0, 0.32],
        depth,
    );
    push_ui_rect(solids, page, PAPER, depth + 0.1);
    let ink_depth = depth + 0.3;
    text(
        &mut type_runs,
        page,
        "CITYGAME / LOCAL EDITION",
        28.0,
        15.0,
        1.35,
        MUTED,
        ink_depth,
    );
    fitted(
        &mut type_runs,
        page,
        content.masthead,
        105.0,
        39.0,
        740.0,
        7.0,
        true,
        ink_depth,
    );
    text(
        &mut type_runs,
        page,
        content.date,
        28.0,
        91.0,
        1.8,
        INK,
        ink_depth,
    );
    fitted(
        &mut type_runs,
        page,
        content.edition,
        600.0,
        91.0,
        322.0,
        1.8,
        false,
        ink_depth,
    );
    rule(solids, page, 28.0, 112.0, 894.0, 2.5, ink_depth);
    rule(solids, page, 28.0, 118.0, 894.0, 0.8, ink_depth);

    let mut headline_size = 6.2;
    if lines(content.headline, 880.0, headline_size).len() > 1 {
        headline_size = 4.8;
    }
    while lines(content.headline, 880.0, headline_size).len() > 2 && headline_size > 2.0 {
        headline_size -= 0.2;
    }
    let headline_lines = lines(content.headline, 880.0, headline_size);
    for (row, line) in headline_lines.iter().take(2).enumerate() {
        fitted(
            &mut type_runs,
            page,
            line,
            35.0,
            if headline_lines.len() > 1 {
                130.0 + row as f32 * 34.0
            } else {
                137.0
            },
            880.0,
            headline_size,
            true,
            ink_depth,
        );
    }
    rule(solids, page, 28.0, 202.0, 894.0, 0.8, ink_depth);
    rule(solids, page, 229.0, 216.0, 0.65, 382.0, ink_depth);
    rule(solids, page, 720.0, 216.0, 0.65, 382.0, ink_depth);

    fitted(
        &mut type_runs,
        page,
        "CITY AT A GLANCE",
        28.0,
        218.0,
        184.0,
        2.4,
        false,
        ink_depth,
    );
    rule(solids, page, 28.0, 239.0, 184.0, 1.0, ink_depth);
    for (row, (label, value)) in content.statistics.iter().take(4).enumerate() {
        let y = 255.0 + row as f32 * 43.0;
        text(&mut type_runs, page, label, 28.0, y, 1.5, MUTED, ink_depth);
        fitted(
            &mut type_runs,
            page,
            value,
            28.0,
            y + 13.0,
            184.0,
            2.9,
            false,
            ink_depth,
        );
    }
    fitted(
        &mut type_runs,
        page,
        "THE CIVIC DESK",
        28.0,
        446.0,
        184.0,
        2.3,
        false,
        ink_depth,
    );
    column(
        solids,
        page,
        FloatRect {
            x: 28.0,
            y: 470.0,
            w: 184.0,
            h: 126.0,
        },
        147,
        ink_depth,
    );

    let picture_area = area(page, 242.0, 216.0, 466.0, 220.0);
    if let Some((atlas, source)) = content.picture {
        let ratio = (picture_area.w / source[2] as f32).min(picture_area.h / source[3] as f32);
        let (w, h) = (source[2] as f32 * ratio, source[3] as f32 * ratio);
        images.push(UiImage {
            atlas,
            src_x: source[0],
            src_y: source[1],
            src_w: source[2],
            src_h: source[3],
            x: picture_area.x + (picture_area.w - w) * 0.5,
            y: picture_area.y + (picture_area.h - h) * 0.5,
            w,
            h,
            depth: ink_depth,
            tint: [0.96, 0.95, 0.91, 1.0],
        });
    } else {
        // A quiet original illustration when a pack contains no editorial photographs.
        for (i, height) in [35.0, 70.0, 118.0, 85.0, 154.0, 98.0, 58.0]
            .iter()
            .enumerate()
        {
            rule(
                solids,
                page,
                290.0 + i as f32 * 52.0,
                421.0 - height,
                42.0,
                *height,
                ink_depth,
            );
        }
    }
    fitted(
        &mut type_runs,
        page,
        picture_for_headline(content.headline).1,
        242.0,
        448.0,
        466.0,
        1.4,
        true,
        ink_depth,
    );
    fitted(
        &mut type_runs,
        page,
        "THE STORY IN OUR CITY",
        242.0,
        476.0,
        466.0,
        2.6,
        false,
        ink_depth,
    );
    let body = lines(content.story, 466.0, 1.8);
    for (row, line) in body.iter().take(5).enumerate() {
        text(
            &mut type_runs,
            page,
            line,
            242.0,
            500.0 + row as f32 * 13.0,
            1.8,
            INK,
            ink_depth,
        );
    }
    column(
        solids,
        page,
        FloatRect {
            x: 242.0,
            y: 570.0,
            w: 218.0,
            h: 28.0,
        },
        815,
        ink_depth,
    );
    column(
        solids,
        page,
        FloatRect {
            x: 487.0,
            y: 570.0,
            w: 221.0,
            h: 28.0,
        },
        2017,
        ink_depth,
    );

    fitted(
        &mut type_runs,
        page,
        "WEATHER WATCH",
        738.0,
        218.0,
        184.0,
        2.4,
        false,
        ink_depth,
    );
    rule(solids, page, 738.0, 239.0, 184.0, 1.0, ink_depth);
    fitted(
        &mut type_runs,
        page,
        content.weather,
        738.0,
        256.0,
        184.0,
        2.7,
        false,
        ink_depth,
    );
    fitted(
        &mut type_runs,
        page,
        "THE LOCAL OUTLOOK",
        738.0,
        284.0,
        184.0,
        1.5,
        false,
        ink_depth,
    );
    fitted(
        &mut type_runs,
        page,
        "NEIGHBORHOOD NOTES",
        738.0,
        323.0,
        184.0,
        2.3,
        false,
        ink_depth,
    );
    column(
        solids,
        page,
        FloatRect {
            x: 738.0,
            y: 349.0,
            w: 184.0,
            h: 247.0,
        },
        409,
        ink_depth,
    );
    rule(solids, page, 28.0, 612.0, 894.0, 1.0, ink_depth);
    text(
        &mut type_runs,
        page,
        "LOCAL REPORTS / THE LIFE OF A CITY",
        28.0,
        623.0,
        1.4,
        MUTED,
        ink_depth,
    );
    text(
        &mut type_runs,
        page,
        "01",
        900.0,
        623.0,
        1.4,
        MUTED,
        ink_depth,
    );

    let close = close_rect(page);
    let close_scale = (page.w / 950.0).min(1.0);
    push_ui_rect(solids, close, PAPER, ink_depth);
    push_ui_rect(solids, FloatRect { h: 1.0, ..close }, INK, ink_depth + 0.01);
    push_ui_rect(
        solids,
        FloatRect {
            y: close.y + close.h - 1.0,
            h: 1.0,
            ..close
        },
        INK,
        ink_depth + 0.01,
    );
    push_ui_rect(solids, FloatRect { w: 1.0, ..close }, INK, ink_depth + 0.01);
    push_ui_rect(
        solids,
        FloatRect {
            x: close.x + close.w - 1.0,
            w: 1.0,
            ..close
        },
        INK,
        ink_depth + 0.01,
    );
    ui::push_pixel_text(
        solids,
        "CLOSE",
        close.x + 20.0 * close_scale,
        close.y + 9.0 * close_scale,
        2.0 * close_scale,
        INK,
        ink_depth + 0.02,
    );
    for run in type_runs {
        if let Some(atlas) = content.font_atlas {
            font::draw(
                images, atlas, &run.value, run.x, run.y, run.size, run.color, run.depth,
            );
        } else {
            let scale = run.size.min(
                font::width(&run.value, run.size) / ui::pixel_text_width(&run.value, 1.0).max(1.0),
            );
            ui::push_pixel_text(
                solids, &run.value, run.x, run.y, scale, run.color, run.depth,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn win95_paper_hits_match_live_style_zero_pointer_trace() {
        // The original's PtInRect relay reported these client coordinates
        // and accepted the first and fourth article rectangles respectively.
        for size in [(640, 480), (1280, 800), (1920, 1080)] {
            let page = page_rect(size);
            let paper = pe_paper_rect(page);
            for (x, y, position) in [(319.0, 55.0, 4), (39.0, 95.0, 7)] {
                let point = Vec2::new(
                    paper.x + (x + 0.25) * paper.w / 640.0,
                    paper.y + (y + 0.25) * paper.h / 400.0,
                );
                assert_eq!(pe_hit_positions(page, 0, point), vec![position]);
            }
        }
    }

    #[test]
    #[ignore = "requires private native click-handler capture"]
    fn win95_scaled_paper_hits_match_native_x86() {
        let fixture: serde_json::Value = serde_json::from_slice(
            &std::fs::read(std::env::var_os("CITYGAME_PE_NEWSPAPER_HITS").unwrap()).unwrap(),
        )
        .unwrap();
        for size in [(640, 480), (1280, 800), (1920, 1080)] {
            let page = page_rect(size);
            let paper = pe_paper_rect(page);
            for case in fixture["cases"].as_array().unwrap() {
                let style = case["style"].as_u64().unwrap() as u8;
                let x = case["x"].as_i64().unwrap() as f32;
                let y = case["y"].as_i64().unwrap() as f32;
                // Interior of the native integer pixel preserves PtInRect's
                // inclusive left/top and exclusive right/bottom behavior.
                let point = Vec2::new(
                    paper.x + (x + 0.25) * paper.w / 640.0,
                    paper.y + (y + 0.25) * paper.h / 400.0,
                );
                let expected: Vec<u16> = case["positions"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|v| v.as_u64().unwrap() as u16)
                    .collect();
                assert_eq!(
                    pe_hit_positions(page, style, point),
                    expected,
                    "size={size:?} case={case}"
                );
            }
        }
    }

    #[test]
    fn win95_paper_draws_and_hits_the_same_native_story_region() {
        use crate::win95_newspaper::{
            NewspaperRect, PeLeadArticle, PeNewspaperPaint, PeSecondaryArticle,
        };
        let bar = NewspaperRect {
            left: 10,
            top: 90,
            right: 50,
            bottom: 93,
        };
        let paint = PeNewspaperPaint {
            style: 0,
            lead: PeLeadArticle {
                headline: b"Lead".to_vec(),
                picture: 0,
                columns: vec![],
                bars: vec![],
            },
            secondary: vec![PeSecondaryArticle {
                story_row: 1,
                headline: b"Story".to_vec(),
                columns: vec![],
                bars: vec![bar],
            }],
            tail: vec![],
        };
        let page = FloatRect {
            x: 10.0,
            y: 20.0,
            w: 1280.0,
            h: 800.0,
        };
        let mut solids = Vec::new();
        let mut images = Vec::new();
        draw_pe_paper(
            page,
            &paint,
            "Paper",
            "Date",
            None,
            None,
            None,
            None,
            &mut solids,
            &mut images,
            0.0,
        );
        assert!(
            solids
                .iter()
                .any(|v| (v.x, v.y, v.w, v.h) == (30.0, 200.0, 80.0, 6.0))
        );
        assert!(
            solids
                .iter()
                .any(|v| (v.x, v.y, v.w, v.h, v.color) == (32.0, 202.0, 76.0, 2.0, [1.0; 4]))
        );
        let region = pe_story_rect(page, 0, 7).unwrap();
        assert!(
            pe_hit_positions(
                page,
                0,
                Vec2::new(region.x + region.w / 2.0, region.y + region.h / 2.0)
            )
            .contains(&7)
        );
        assert!(!pe_hit_positions(page, 0, Vec2::new(page.x - 1.0, page.y)).contains(&7));
    }

    #[test]
    fn win95_article_detail_centers_grows_and_shares_control_geometry() {
        let page = FloatRect {
            x: 10.0,
            y: 20.0,
            w: 640.0,
            h: 400.0,
        };
        let short = "A short story.";
        let long = "A long paragraph about the city. ".repeat(300);
        let a = pe_detail_rect(page, short);
        let b = pe_detail_rect(page, &long);
        assert_eq!((a.w, b.w), (444.0, 444.0));
        assert!(a.h < b.h);
        assert_eq!(a.x + a.w * 0.5, 330.0);
        assert_eq!(a.y + a.h * 0.5, 220.0);
        let close = pe_detail_close_rect(page, short);
        assert!(close.x >= a.x && close.y >= a.y && close.x + close.w < a.x + a.w);
        assert_eq!(pe_detail_scroll_max(short), 0);
        assert!(pe_detail_scroll_max(&long) > 0);
        let mut solids = Vec::new();
        let mut images = Vec::new();
        draw_pe_detail(
            page,
            "Headline",
            short,
            0,
            None,
            &mut solids,
            &mut images,
            0.0,
        );
        assert!(
            solids
                .iter()
                .any(|v| (v.x, v.y, v.w, v.h) == (close.x, close.y, close.w, close.h))
        );
    }

    #[test]
    #[ignore = "requires private live Win95 article text captures"]
    fn win95_article_detail_approximates_two_captured_heights() {
        let root = std::path::PathBuf::from(std::env::var_os("CITYGAME_WIN95_LIVE_ROOT").unwrap());
        let page = FloatRect {
            x: 0.0,
            y: 0.0,
            w: 640.0,
            h: 400.0,
        };
        for (file, native_height) in [
            ("article-live/ui-body.bin", 430.0_f32),
            ("repaint-live/ui-body.bin", 334.0),
        ] {
            let body = std::fs::read_to_string(root.join(file))
                .unwrap()
                .replace('\n', "\r");
            let rect = pe_detail_rect(page, &body);
            assert!((rect.h - native_height).abs() <= 2.0, "{file}: {}", rect.h);
        }
    }

    #[test]
    fn mac_composed_paper_uses_native_rectangles_and_picture() {
        use crate::mac_newspaper::{MacAuxiliaryBox, MacLeadLayout, PaperHeadlineLine, PaperRect};
        let rect = PaperRect(10, 20, 13, 80);
        let empty = || MacAuxiliaryBox {
            lines: Vec::new(),
            filler: Vec::new(),
            ending_seed: 0,
        };
        let paint = super::super::newspaper_state::MacPaperPaint {
            family: 1,
            initial_seed: 0,
            lead: MacLeadLayout {
                headline: vec![PaperHeadlineLine {
                    text: b"Lead".to_vec(),
                    size: 24,
                    rect: PaperRect(0, 0, 30, 512),
                }],
                story_filler: vec![rect],
                picture_id: 403,
                picture_rect: PaperRect(50, 100, 150, 200),
                picture_filler: Vec::new(),
                after_lead_seed: 0,
                after_picture_seed: 0,
            },
            stories: Vec::new(),
            weather: empty(),
            survey: empty(),
        };
        let page = FloatRect {
            x: 10.0,
            y: 20.0,
            w: 1024.0,
            h: 640.0,
        };
        let mut solids = Vec::new();
        let mut images = Vec::new();
        draw_mac_paper(
            page,
            &paint,
            "Masthead",
            "Date",
            Some((7, [10, 20, 32, 16])),
            None,
            &mut solids,
            &mut images,
            1.0,
        );
        assert!(
            solids
                .iter()
                .any(|s| (s.x, s.y, s.w, s.h) == (50.0, 40.0, 120.0, 6.0))
        );
        assert_eq!(images.len(), 1);
        assert_eq!(
            (
                images[0].atlas,
                images[0].x,
                images[0].y,
                images[0].w,
                images[0].h
            ),
            (7, 278.0, 204.0, 64.0, 32.0)
        );
        let lead = mac_lead_hit_rect(page, 1);
        assert_eq!((lead.x, lead.y, lead.w, lead.h), (10.0, 20.0, 1024.0, 60.0));
        let close = mac_close_rect(page);
        assert_eq!(
            (close.x, close.y, close.w, close.h),
            (16.0, 26.0, 24.0, 24.0)
        );
    }

    #[test]
    fn mac_article_detail_wraps_and_scrolls_every_line() {
        let article = "A long article about the city. ".repeat(250);
        let all = detail_lines(&article);
        assert!(all.len() > DETAIL_VISIBLE_LINES);
        let last_start = detail_scroll_max(&article);
        assert_eq!(last_start + DETAIL_VISIBLE_LINES, all.len());
        for size in [(400, 360), (1024, 768), (2624, 1644)] {
            let page = page_rect(size);
            let detail = detail_rect(page);
            assert!(detail.x >= page.x && detail.y >= page.y);
            assert!(detail.x + detail.w <= page.x + page.w);
            assert!(detail.y + detail.h <= page.y + page.h);
            for button in [
                detail_close_rect(page),
                detail_scroll_rect(page, false),
                detail_scroll_rect(page, true),
            ] {
                assert!(button.x >= detail.x && button.y >= detail.y);
                assert!(button.x + button.w <= detail.x + detail.w);
                assert!(button.y + button.h <= detail.y + detail.h);
            }
        }
    }

    #[test]
    fn newspaper_columns_and_photographs_fit_the_page_at_small_and_retina_sizes() {
        for window in [(400, 360), (1024, 768), (2624, 1644)] {
            let page = page_rect(window);
            let statistics = [
                ("Population", "18,500".to_owned()),
                ("Treasury", "$20,000".to_owned()),
            ];
            let content = Content {
                masthead: "THE CHARLESTON COURIER",
                headline: "CITY BUILDS TOWARD A BRIGHTER FUTURE",
                story: "Residents are watching the city's next chapter. Essential services and local neighborhoods remain at the heart of the debate.",
                date: "AUGUST 1989",
                edition: "THE DAILY EDITION",
                weather: "CHILLY WEATHER",
                statistics: &statistics,
                picture: Some((0, [0, 0, 155, 100])),
                font_atlas: Some(1),
            };
            let (mut solids, mut images) = (Vec::new(), Vec::new());
            draw(page, &content, &mut solids, &mut images, 100.0);
            for solid in &solids {
                assert!(
                    solid.x >= 0.0
                        && solid.y >= 0.0
                        && solid.x + solid.w <= window.0 as f32 + 0.1
                        && solid.y + solid.h <= window.1 as f32 + 0.1
                );
            }
            let photo = images[0];
            assert!(
                photo.x >= page.x
                    && photo.y >= page.y
                    && photo.x + photo.w <= page.x + page.w
                    && photo.y + photo.h < page.y + page.h
            );
            assert!((photo.w / photo.h - 1.55).abs() < 0.001);
        }
    }

    #[test]
    fn newspaper_long_headlines_wrap_without_overrunning_their_columns() {
        for value in [
            "THE CITY'S SERVICES REPORT A PERIOD OF EXTRAORDINARILY STRONG AND SUSTAINED DEMAND",
            "A_SINGLE_UNBROKEN_HEADLINE_WITH_MANY_CHARACTERS_AND_NO_SPACES",
        ] {
            for line in lines(value, 190.0, 2.8) {
                assert!(font::width(&line, 2.8) <= 190.0);
            }
        }
    }
}
