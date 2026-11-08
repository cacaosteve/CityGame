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
