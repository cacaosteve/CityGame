//! Recovered Win95 newspaper drawing workers.
//!
//! The original text and layout dispatcher are still being reconstructed. This
//! module keeps the filler worker independent of the authored newspaper UI.

#[derive(Debug)]
pub(crate) struct PeArchiveOpenError {
    pub(crate) path: std::path::PathBuf,
    pub(crate) error: std::io::Error,
}

impl std::fmt::Display for PeArchiveOpenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "opening {}: {}", self.path.display(), self.error)
    }
}

impl std::error::Error for PeArchiveOpenError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.error)
    }
}

/// Date and price selected by the PE's paper paint routine at 0x477d8f.
/// The newspaper prints a fictional Sunday day number, not the simulation
/// day shown in the status bar. Out-of-range native table indices are rejected.
pub(crate) fn pe_newspaper_header(founded: u16, age: u32, wording: u8) -> Option<(String, u32)> {
    const MONTHS: [&str; 12] = [
        "January",
        "February",
        "March",
        "April",
        "May",
        "June",
        "July",
        "August",
        "September",
        "October",
        "November",
        "December",
    ];
    if founded > i16::MAX as u16
        || age > i32::MAX as u32
        || age / 300 > i16::MAX as u32
        || wording > 2
    {
        return None;
    }
    let month = (age / 25 % 12) as i32;
    let elapsed = (age / 300) as i32;
    let season = ((month + 1) % 12) / 3;
    let year = i32::from(founded) + elapsed;
    let day = (year + month + season) % 7 + 1;
    let era = ((i32::from(founded) - 1900) / 50 + elapsed / 50).min(4);
    if era < 0 {
        return None;
    }
    Some((
        format!("Sunday {day}, {} {year}", MONTHS[month as usize]),
        377 + u32::from(wording) * 5 + era as u32,
    ))
}

pub(crate) fn pe_newspaper_price(resource: u32) -> Option<&'static str> {
    const PRICES: [&str; 15] = [
        "One Cent",
        "Five Cents",
        "Twenty-Five Cents",
        "One Dollar",
        "Five Dollars",
        "A Single Penny",
        "Only A Nickel",
        "Still A Quarter",
        "Just A Dollar",
        "Merely Five Bucks",
        "Price 1¢",
        "Price 5¢",
        "Price 25¢",
        "Price $1",
        "Price $5",
    ];
    PRICES.get(resource.checked_sub(377)? as usize).copied()
}

/// Resource indices in bytes 4 and 3 of each five-byte paper record.
pub(crate) fn pe_newspaper_tail_labels(
    weather: u8,
    opinion: u8,
) -> Option<(&'static str, &'static str)> {
    const WEATHER: [&str; 6] = [
        "Weather Corner",
        "Weather Report",
        "Today's Weather",
        "Weather with Merle",
        "Weather Forecast",
        "Weather Talk",
    ];
    const OPINION: [&str; 6] = [
        "Fred's Opinion",
        "Editor's Corner",
        "Commentary",
        "Opinion Poll",
        "Survey",
        "MisSim's Advice",
    ];
    Some((
        *WEATHER.get(usize::from(weather))?,
        *OPINION.get(usize::from(opinion))?,
    ))
}

/// PE `0x47b400` formats the live heat, wind, and humidity bytes.
pub(crate) fn pe_newspaper_weather_line(heat: u8, wind: u8, humidity: u8) -> String {
    format!("{}°F {wind}mph {humidity}mm", i32::from(heat) - 100)
}

pub(crate) fn pe_newspaper_opinion_lines(
    kind: u8,
    approval: u16,
    counts: [u16; 7],
    order: [u16; 7],
) -> Vec<String> {
    if kind <= 2 {
        return if approval == 0 {
            Vec::new()
        } else {
            vec![format!("Mayor Rated {approval}%")]
        };
    }
    if kind > 4 {
        return Vec::new();
    }
    const ISSUES: [&str; 7] = [
        "Traffic", "Pollute", "Crime", "Taxes", "Unemp.", "Educate", "Health",
    ];
    counts
        .into_iter()
        .zip(order)
        .take(4)
        .take_while(|(count, _)| *count != 0)
        .filter_map(|(count, index)| {
            ISSUES
                .get(usize::from(index))
                .map(|label| format!("{label} {count}%"))
        })
        .collect()
}

use crate::win95_random::Win95CrtRand;
use anyhow::{Context, Result, ensure};

/// Seed arithmetic shared by the PE newspaper layout and story workers.
/// The layout uses the full paper index; the story worker sign-extends its
/// low WORD. `slot` is the signed WORD position within the paper.
#[derive(Clone, Copy, Debug)]
pub(crate) struct PeNewspaperSeed {
    pub days: i32,
    pub startup_tick: i16,
    pub paper: i32,
    pub slot: i16,
}

impl PeNewspaperSeed {
    pub(crate) fn layout(self) -> u32 {
        (self.days / 25)
            .wrapping_add(i32::from(self.startup_tick))
            .wrapping_add(self.paper.wrapping_mul(500)) as u32
    }

    pub(crate) fn story(self) -> u32 {
        (self.days / 25)
            .wrapping_add(i32::from(self.startup_tick))
            .wrapping_add(i32::from(self.paper as i16).wrapping_mul(500))
            .wrapping_add(i32::from(self.slot) * 7) as u32
    }
}

/// PE article click worker `0x479b90` maps a paper position to a story row.
/// Paper style 2 has no clickable article in positions 5 and 6. Positions
/// outside the worker's eleven-entry table are rejected by this safe API.
pub(crate) fn pe_article_story_row(position: u16, paper_style: u8) -> Option<usize> {
    if paper_style == 2 && matches!(position, 5 | 6) {
        return None;
    }
    [0, 0, 0, 0, 0, 8, 7, 1, 2, 3, 4]
        .get(usize::from(position))
        .copied()
}

/// The seven rectangles visited by the PE `0x477b20` mouse handler, in
/// 640-by-400 newspaper coordinates. Right and bottom edges are exclusive.
const PE_ARTICLE_RECTS: [[[i32; 4]; 7]; 3] = [
    [
        [0, 36, 640, 76],
        [0, 300, 213, 400],
        [426, 300, 640, 400],
        [0, 76, 213, 300],
        [426, 76, 640, 300],
        [213, 176, 320, 400],
        [320, 176, 426, 400],
    ],
    [
        [0, 0, 640, 37],
        [0, 300, 128, 400],
        [0, 73, 128, 300],
        [128, 73, 640, 186],
        [128, 287, 384, 400],
        [384, 186, 512, 400],
        [512, 186, 640, 400],
    ],
    [
        [0, 30, 128, 400],
        [256, 30, 384, 135],
        [512, 24, 640, 339],
        [384, 24, 512, 400],
        [128, 240, 256, 400],
        [256, 135, 384, 400],
        [512, 339, 640, 400],
    ],
];

/// Masthead, date, price and illustration rectangles initialized immediately
/// before the seven clickable article rectangles by PE 0x476ff0.
const PE_HEADER_RECTS: [[[i32; 4]; 4]; 3] = [
    [
        [0, 6, 640, 36],
        [5, 16, 218, 31],
        [315, 16, 635, 31],
        [243, 76, 456, 176],
    ],
    [
        [0, 37, 640, 73],
        [421, 53, 635, 68],
        [5, 53, 218, 68],
        [178, 186, 434, 286],
    ],
    [
        [0, 0, 384, 30],
        [0, 0, 635, 12],
        [0, 12, 635, 24],
        [128, 30, 256, 240],
    ],
];

pub(crate) fn pe_newspaper_layout_rect(style: u8, position: u16) -> Option<NewspaperRect> {
    let raw = if position < 4 {
        *PE_HEADER_RECTS
            .get(usize::from(style))?
            .get(usize::from(position))?
    } else {
        *PE_ARTICLE_RECTS
            .get(usize::from(style))?
            .get(usize::from(position - 4))?
    };
    Some(NewspaperRect {
        left: raw[0],
        top: raw[1],
        right: raw[2],
        bottom: raw[3],
    })
}

/// Positions for which the native handler calls its article worker. The
/// worker itself can reject a position for a particular paper style.
pub(crate) fn pe_newspaper_hit_positions(style: u8, x: i32, y: i32) -> Vec<u16> {
    let Some(rects) = PE_ARTICLE_RECTS.get(usize::from(style)) else {
        return Vec::new();
    };
    rects
        .iter()
        .enumerate()
        .filter_map(|(index, &[left, top, right, bottom])| {
            (left <= x && x < right && top <= y && y < bottom).then_some(index as u16 + 4)
        })
        .collect()
}

/// PE `0x47b380`: search around half the signed first byte of a C string.
/// The original uses that byte as a length despite passing C strings to GDI.
/// Return None if its search would read beyond the terminating NUL.
pub(crate) fn pe_headline_split_offset(text: &[u8]) -> Option<usize> {
    let first = *text.first()?;
    if first == 0 || first > i8::MAX as u8 || text.contains(&0) {
        return None;
    }
    let midpoint = usize::from(first / 2);
    let byte_at = |offset: usize| {
        text.get(offset)
            .copied()
            .or_else(|| (offset == text.len()).then_some(0))
    };
    if byte_at(midpoint)? == b' ' {
        return Some(midpoint);
    }
    for distance in 1..midpoint {
        if byte_at(midpoint + distance)? == b' ' {
            return Some(midpoint + distance);
        }
        if byte_at(midpoint - distance)? == b' ' {
            return Some(midpoint - distance);
        }
    }
    Some(midpoint)
}

/// Diagnostic model of the fallback branch in PE `0x47afb0`, with a supplied
/// fit callback. The actual text draw helper returns success unconditionally,
/// so normal native painting does not enter this branch.
pub(crate) fn pe_headline_layout_with_fit(
    text: &[u8],
    top: i32,
    font: i16,
    mut fits: impl FnMut(&[u8], i16) -> bool,
) -> Option<(i32, i16, bool)> {
    if text.contains(&0) || font <= 0 {
        return None;
    }
    let advance = |size: i16| i32::from(size) * 5 / 4;
    if fits(text, font) {
        return Some((top.wrapping_add(advance(font)), font, true));
    }
    let split = pe_headline_split_offset(text)?;
    if split == 0 || split >= text.len() {
        return None;
    }
    let mut modified = text.to_vec();
    let first = modified[0];
    modified[0] = split as u8;
    let mut size = font;
    loop {
        if fits(&modified, size) {
            let first_top = top.wrapping_add(advance(size));
            modified[split] = first.wrapping_sub(split as u8);
            let second_fits = fits(&modified[split..], size);
            let final_top = if second_fits {
                first_top.wrapping_add(advance(size))
            } else {
                first_top
            };
            return Some((final_top, size, second_fits));
        }
        if size == 0 {
            return None;
        }
        size = (i32::from(size) * 6 / 7) as i16;
    }
}

/// PE `0x47a030`: trim complete trailing words until the selected GDI font
/// measures within the available width. The callback supplies that measurement.
/// Input and output are OEM bytes; the measurement callback receives ANSI.
pub(crate) fn pe_truncate_headline_ascii(
    text: &[u8],
    width: i32,
    mut measure: impl FnMut(&[u8]) -> i32,
) -> Option<Vec<u8>> {
    if width < 0 || text.contains(&0) {
        return None;
    }
    let mut result: Vec<_> = text
        .iter()
        .copied()
        .map(crate::win95_oem::oem_to_ansi)
        .collect();
    while measure(&result) > width {
        if result.is_empty() {
            return None;
        }
        let cut = result.iter().rposition(|&c| c == b' ').unwrap_or(0);
        result.truncate(cut);
    }
    Some(
        result
            .into_iter()
            .map(crate::win95_oem::ansi_to_oem)
            .collect(),
    )
}

/// Native picture-resource lookup for newspaper story IDs 0..=66. Zero
/// selects a random illustration in the imported 400..=411 resource range.
const PE_STORY_PICTURES: [u16; 67] = [
    0, 0, 404, 404, 405, 402, 406, 0, 0, 0, 0, 404, 406, 408, 0, 0, 410, 401, 411, 0, 0, 0, 408, 0,
    408, 409, 400, 406, 408, 408, 408, 411, 411, 409, 403, 411, 410, 404, 402, 403, 407, 0, 0, 0,
    0, 411, 403, 410, 408, 403, 403, 403, 411, 411, 403, 403, 403, 403, 411, 410, 401, 411, 404,
    407, 407, 0, 0,
];

/// PE `0x4782b3..0x4782f0`, when the paper has no cached picture object.
/// It reseeds the shared CRT even for a fixed picture; only zero entries draw.
/// Call at the native paint boundary, not on every presentation frame.
pub(crate) fn pe_newspaper_first_picture(
    story_id: usize,
    seed: PeNewspaperSeed,
    crt: &mut Win95CrtRand,
) -> Option<u16> {
    let picture = *PE_STORY_PICTURES.get(story_id)?;
    *crt = Win95CrtRand::new(seed.layout().wrapping_add(21));
    Some(if picture == 0 {
        400 + crt.random_u15() % 12
    } else {
        picture
    })
}

/// User-supplied Win95 DATA archive sections used by the PE story expander.
/// Numeric resources are big-endian even though the directory is little-endian.
pub(crate) struct PeNewspaperData {
    starts: Vec<u16>,
    counts: Vec<u16>,
    offsets: Vec<u32>,
    text: Vec<u8>,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct PeSecondaryArticle {
    pub story_row: usize,
    pub headline: Vec<u8>,
    pub columns: Vec<NewspaperRect>,
    pub bars: Vec<NewspaperRect>,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct PeLeadArticle {
    pub headline: Vec<u8>,
    pub picture: u16,
    pub columns: Vec<NewspaperRect>,
    pub bars: Vec<NewspaperRect>,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct PeNewspaperTailSection {
    pub story_row: usize,
    pub headline: Option<Vec<u8>>,
    pub filler: NewspaperRect,
    pub bars: Vec<NewspaperRect>,
    pub crt_after: u32,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct PeNewspaperPaint {
    pub style: u8,
    pub lead: PeLeadArticle,
    pub secondary: Vec<PeSecondaryArticle>,
    pub tail: Vec<PeNewspaperTailSection>,
}

impl PeNewspaperData {
    /// Compose the recovered native lead-through-tail paint region using the
    /// live city age and selected paper record. The caller supplies the saved
    /// first-document tick and the current GDI text-width operation. Failed
    /// unsupported text or geometry leaves the city and CRT untouched.
    pub(crate) fn render_paper_region_ascii(
        &self,
        city: &mut crate::city::City,
        paper: usize,
        startup_tick: i16,
        crt: &mut Win95CrtRand,
        cached_picture: Option<u16>,
        context: Option<&PeHeadlineContext<'_>>,
        mut measure: impl FnMut(&[u8]) -> i32,
    ) -> Option<PeNewspaperPaint> {
        let tables = city.pe_newspaper_tables()?;
        let style = *tables.papers.get(paper.checked_mul(5)?.checked_add(1)?)? as u8;
        let seed = PeNewspaperSeed {
            days: city.city_age_days()? as i32,
            startup_tick,
            paper: i32::try_from(paper).ok()?,
            slot: 0,
        };
        let mut next_city = city.clone();
        let mut next_crt = *crt;
        let lead = self.render_lead_article_ascii(
            &mut next_city,
            style,
            seed,
            &mut next_crt,
            cached_picture,
            context,
            &mut measure,
        )?;
        let secondary = self.render_secondary_articles_ascii(
            &mut next_city,
            style,
            seed,
            &mut next_crt,
            context,
            &mut measure,
        )?;
        let tail =
            self.render_paper_tail_ascii(&mut next_city, style, seed, &mut next_crt, context)?;
        *city = next_city;
        *crt = next_crt;
        Some(PeNewspaperPaint {
            style,
            lead,
            secondary,
            tail,
        })
    }

    /// Weather and opinion regions at `0x478523..0x478b7e`, after the four
    /// secondary articles. Text draw calls succeed on the pinned PE; their
    /// visual strings and GDI rasterization are outside this state worker.
    pub(crate) fn render_paper_tail_ascii(
        &self,
        city: &mut crate::city::City,
        style: u8,
        seed: PeNewspaperSeed,
        crt: &mut Win95CrtRand,
        context: Option<&PeHeadlineContext<'_>>,
    ) -> Option<Vec<PeNewspaperTailSection>> {
        let rects = PE_ARTICLE_RECTS.get(usize::from(style))?;
        let paper_index = usize::try_from(seed.paper).ok()?;
        let tables = city.pe_newspaper_tables()?;
        let paper_type = *tables
            .papers
            .get(paper_index.checked_mul(5)?.checked_add(3)?)? as u8;
        let mut next_crt = *crt;
        let mut updates = Vec::new();
        let mut sections = Vec::new();
        for (row, rect_index, seed_offset) in [(7usize, 2usize, 42u32), (8, 1, 35)] {
            let [left, top, right, bottom] = rects[rect_index];
            let mut filler_top = top;
            let headline = if style != 2 {
                let (story_id, mut story) = city.pe_newspaper_text_row(row)?;
                let story_id = usize::try_from(story_id).ok()?;
                next_crt = Win95CrtRand::new(seed.layout().wrapping_add(seed_offset));
                let mut cache = PeTextChoiceCache::default();
                let text = self.render_headline_ascii(
                    story_id,
                    &mut next_crt,
                    &mut story,
                    context,
                    &mut cache,
                )?;
                updates.push((row, story));
                // Weather: label, headline, and temperature. Opinion: label,
                // headline, then an optional approval or ranked-issue line.
                let extra = if row == 7 {
                    1
                } else if paper_type <= 2 {
                    usize::from(city.pe_mayor_approval() != 0)
                } else if paper_type <= 4 {
                    city.pe_opinion_poll_results()
                        .0
                        .iter()
                        .take(4)
                        .take_while(|&&count| count != 0)
                        .count()
                } else {
                    0
                };
                filler_top = filler_top.wrapping_add((2 + extra as i32) * 15);
                Some(text)
            } else {
                None
            };
            let filler = NewspaperRect {
                left,
                top: filler_top,
                right,
                bottom,
            };
            let bars = pe_newspaper_filler_47aca0(filler, &mut next_crt)?;
            sections.push(PeNewspaperTailSection {
                story_row: row,
                headline,
                filler,
                bars,
                crt_after: next_crt.seed(),
            });
        }
        for (row, story) in updates {
            city.set_pe_newspaper_text_row(row, story)?;
        }
        *crt = next_crt;
        Some(sections)
    }

    /// Paint the lead and picture regions through `0x47843b`. A cached
    /// picture skips the native picture reseed and selection on redraw.
    pub(crate) fn render_lead_article_ascii(
        &self,
        city: &mut crate::city::City,
        style: u8,
        seed: PeNewspaperSeed,
        crt: &mut Win95CrtRand,
        cached_picture: Option<u16>,
        context: Option<&PeHeadlineContext<'_>>,
        mut measure: impl FnMut(&[u8]) -> i32,
    ) -> Option<PeLeadArticle> {
        let [left, top, right, bottom] = *PE_ARTICLE_RECTS.get(usize::from(style))?.first()?;
        let (story_id, mut story) = city.pe_newspaper_text_row(0)?;
        let story_id = usize::try_from(story_id).ok()?;
        let mut next_crt = Win95CrtRand::new(seed.layout().wrapping_add(28));
        let mut cache = PeTextChoiceCache::default();
        let headline =
            self.render_headline_ascii(story_id, &mut next_crt, &mut story, context, &mut cache)?;
        let headline = pe_truncate_headline_ascii(&headline, right - left, &mut measure)?;
        let mut columns = Vec::new();
        let mut bars = Vec::new();
        if style == 2 {
            let rect = NewspaperRect {
                left,
                top,
                right,
                bottom,
            };
            (columns, bars) = pe_newspaper_headline_columns(rect, 10, &mut next_crt)?;
        }
        let picture = match cached_picture {
            Some(picture) => picture,
            None => pe_newspaper_first_picture(story_id, seed, &mut next_crt)?,
        };
        if style == 2 {
            let picture_rect = NewspaperRect {
                left: 128,
                top: 30,
                right: 256,
                bottom: 240,
            };
            bars.extend(pe_newspaper_filler_47aca0(picture_rect, &mut next_crt)?);
            columns.push(picture_rect);
        }
        city.set_pe_newspaper_text_row(0, story)?;
        *crt = next_crt;
        Some(PeLeadArticle {
            headline,
            picture,
            columns,
            bars,
        })
    }

    /// Paint the four secondary articles in the native `0x47845c` order.
    /// The caller supplies the width of text in the device context's selected
    /// font; without matching GDI metrics the text truncation remains a probe.
    /// City rows and the shared CRT are committed only if every article succeeds.
    pub(crate) fn render_secondary_articles_ascii(
        &self,
        city: &mut crate::city::City,
        style: u8,
        seed: PeNewspaperSeed,
        crt: &mut Win95CrtRand,
        context: Option<&PeHeadlineContext<'_>>,
        mut measure: impl FnMut(&[u8]) -> i32,
    ) -> Option<Vec<PeSecondaryArticle>> {
        let rects = PE_ARTICLE_RECTS.get(usize::from(style))?;
        let mut next_crt = *crt;
        let mut updates = Vec::new();
        let mut articles = Vec::new();
        for position in 7..=10usize {
            let row = position - 6;
            let (story_id, mut story) = city.pe_newspaper_text_row(row)?;
            let story_id = usize::try_from(story_id).ok()?;
            next_crt = Win95CrtRand::new(seed.layout().wrapping_add((position as u32) * 7));
            let mut cache = PeTextChoiceCache::default();
            let headline = self.render_headline_ascii(
                story_id,
                &mut next_crt,
                &mut story,
                context,
                &mut cache,
            )?;
            let [left, top, right, bottom] = rects[position - 4];
            let headline = pe_truncate_headline_ascii(&headline, right - left, &mut measure)?;
            let rect = NewspaperRect {
                left,
                top,
                right,
                bottom,
            };
            let (columns, bars) = pe_newspaper_headline_columns(rect, 10, &mut next_crt)?;
            updates.push((row, story));
            articles.push(PeSecondaryArticle {
                story_row: row,
                headline,
                columns,
                bars,
            });
        }
        for (row, story) in updates {
            city.set_pe_newspaper_text_row(row, story)?;
        }
        *crt = next_crt;
        Some(articles)
    }

    /// Resolve an article position against a city's live story table and
    /// commit its generated choices only after both text workers succeed.
    pub(crate) fn render_city_article_ascii(
        &self,
        city: &mut crate::city::City,
        paper_style: u8,
        seed: PeNewspaperSeed,
        crt: &mut Win95CrtRand,
        context: Option<&PeHeadlineContext<'_>>,
    ) -> Option<(Vec<u8>, Vec<u8>)> {
        let slot = u16::try_from(seed.slot).ok()?;
        let row = pe_article_story_row(slot, paper_style)?;
        let (id, mut story) = city.pe_newspaper_text_row(row)?;
        let id = usize::try_from(id).ok()?;
        let mut next_crt = *crt;
        let text = self.render_seeded_story_ascii(id, seed, &mut next_crt, &mut story, context)?;
        city.set_pe_newspaper_text_row(row, story)?;
        *crt = next_crt;
        Some(text)
    }

    /// The story builder at 0x479ce9 and 0x479d38 reseeds before each
    /// caller. The headline's choice cache and story row survive that reseed.
    /// Unsupported text leaves all caller-owned state unchanged.
    pub(crate) fn render_seeded_story_ascii(
        &self,
        story_id: usize,
        seed: PeNewspaperSeed,
        crt: &mut Win95CrtRand,
        story: &mut PeHeadlineState,
        context: Option<&PeHeadlineContext<'_>>,
    ) -> Option<(Vec<u8>, Vec<u8>)> {
        let mut next_crt = Win95CrtRand::new(seed.story());
        let mut next_story = *story;
        let mut cache = PeTextChoiceCache::default();
        let headline = self.render_headline_ascii(
            story_id,
            &mut next_crt,
            &mut next_story,
            context,
            &mut cache,
        )?;
        next_crt = Win95CrtRand::new(seed.story());
        let body = self.render_body_ascii(
            story_id,
            &mut next_crt,
            &mut next_story,
            context,
            &mut cache,
        )?;
        *crt = next_crt;
        *story = next_story;
        Some((headline, body))
    }

    pub(crate) fn load(data: &std::path::Path, index: &std::path::Path) -> Result<Self> {
        use std::io::Read;
        let open = |path: &std::path::Path| {
            std::fs::File::open(path).map_err(|error| PeArchiveOpenError {
                path: path.to_path_buf(),
                error,
            })
        };
        let mut data_file = open(data)?;
        let mut index_file = open(index)?;
        let mut bytes = Vec::new();
        let mut directory = Vec::new();
        index_file
            .read_to_end(&mut directory)
            .with_context(|| format!("reading {}", index.display()))?;
        data_file
            .read_to_end(&mut bytes)
            .with_context(|| format!("reading {}", data.display()))?;
        Self::from_bytes(&bytes, &directory)
    }

    fn from_bytes(data: &[u8], index: &[u8]) -> Result<Self> {
        ensure!(
            !index.is_empty() && index.len().is_multiple_of(8),
            "invalid DATA index length"
        );
        let mut entries = Vec::new();
        for row in index.chunks_exact(8) {
            let id = u32::from_le_bytes(row[..4].try_into().unwrap());
            let offset = u32::from_le_bytes(row[4..].try_into().unwrap()) as usize;
            ensure!(offset <= data.len(), "DATA section offset outside file");
            if let Some(&(_, previous)) = entries.last() {
                ensure!(offset >= previous, "DATA section offsets are unordered");
            }
            ensure!(
                !entries.iter().any(|&(other, _)| other == id),
                "duplicate DATA section"
            );
            entries.push((id, offset));
        }
        let section = |id| -> Result<&[u8]> {
            let n = entries
                .iter()
                .position(|&(entry, _)| entry == id)
                .with_context(|| format!("missing DATA section {id}"))?;
            let end = entries.get(n + 1).map_or(data.len(), |&(_, offset)| offset);
            Ok(&data[entries[n].1..end])
        };
        let starts = section(1000)?;
        let counts = section(1001)?;
        let offsets = section(1002)?;
        let text = section(1003)?;
        ensure!(
            starts.len() == 500 && counts.len() == 500 && offsets.len() == 10_000,
            "unexpected newspaper table sizes"
        );
        ensure!(!text.is_empty(), "empty newspaper text section");
        let decode_words = |bytes: &[u8]| -> Vec<u16> {
            bytes
                .chunks_exact(2)
                .map(|b| u16::from_be_bytes([b[0], b[1]]))
                .collect()
        };
        let starts = decode_words(starts);
        let counts = decode_words(counts);
        let offsets: Vec<u32> = offsets
            .chunks_exact(4)
            .map(|b| u32::from_be_bytes(b.try_into().unwrap()))
            .collect();
        for (&start, &count) in starts.iter().zip(&counts) {
            let end = usize::from(start) + usize::from(count);
            ensure!(
                end <= offsets.len(),
                "newspaper variant range outside offset table"
            );
            for &offset in &offsets[usize::from(start)..end] {
                let tail = text
                    .get(offset as usize..)
                    .context("newspaper text offset outside section")?;
                ensure!(tail.contains(&0), "unterminated newspaper text");
            }
        }
        Ok(Self {
            starts,
            counts,
            offsets,
            text: text.to_vec(),
        })
    }

    /// Encoded variant bytes, excluding the terminating zero.
    pub(crate) fn variant(&self, story: usize, variant: usize) -> Option<&[u8]> {
        let start = usize::from(*self.starts.get(story)?);
        if variant >= usize::from(*self.counts.get(story)?) {
            return None;
        }
        self.encoded_string(start + variant)
    }

    fn encoded_string(&self, id: usize) -> Option<&[u8]> {
        let offset = *self.offsets.get(id)? as usize;
        let tail = self.text.get(offset..)?;
        Some(&tail[..tail.iter().position(|&b| b == 0)?])
    }

    /// PE `0x479f24`: choose the encoded string before recursive expansion.
    /// A populated story consumes one CRT draw even when it has one variant.
    /// Missing and empty story slots return `None` without touching the stream;
    /// the native code expects callers to supply valid, nonempty slots.
    pub(crate) fn select_headline_variant<'a>(
        &'a self,
        story: usize,
        crt: &mut Win95CrtRand,
    ) -> Option<(u16, &'a [u8])> {
        let start = *self.starts.get(story)?;
        let count = *self.counts.get(story)?;
        if count == 0 {
            return None;
        }
        let choice = crt.random_u15() % count;
        let encoded = self.variant(story, usize::from(choice))?;
        Some((start.wrapping_add(choice), encoded))
    }

    /// PE `0x479ef0`: select, expand, and format a headline using the
    /// measured US ANSI 1252 / OEM 437 character conversions.
    pub(crate) fn render_headline_ascii(
        &self,
        story_id: usize,
        crt: &mut Win95CrtRand,
        story: &mut PeHeadlineState,
        context: Option<&PeHeadlineContext<'_>>,
        cache: &mut PeTextChoiceCache,
    ) -> Option<Vec<u8>> {
        let mut next_crt = *crt;
        let mut next_story = *story;
        let mut next_cache = cache.clone();
        let (id, _) = self.select_headline_variant(story_id, &mut next_crt)?;
        let mut text = self.expand_headline_with_cache(
            usize::from(id),
            &mut next_crt,
            &mut next_story,
            context,
            &mut next_cache,
        )?;
        format_pe_headline_ascii(&mut text)?;
        *crt = next_crt;
        *story = next_story;
        *cache = next_cache;
        Some(text)
    }

    /// PE `0x47a100`: independently select and format a body, retaining the
    /// cache created by the earlier headline. Text bytes use OEM 437.
    pub(crate) fn render_body_ascii(
        &self,
        story_id: usize,
        crt: &mut Win95CrtRand,
        story: &mut PeHeadlineState,
        context: Option<&PeHeadlineContext<'_>>,
        cache: &mut PeTextChoiceCache,
    ) -> Option<Vec<u8>> {
        let mut next_crt = *crt;
        let mut next_story = *story;
        let mut next_cache = cache.clone();
        let (id, _) = self.select_headline_variant(story_id, &mut next_crt)?;
        let mut text = self.expand_body_with_cache(
            usize::from(id),
            &mut next_crt,
            &mut next_story,
            context,
            &mut next_cache,
        )?;
        format_pe_body_ascii(&mut text)?;
        *crt = next_crt;
        *story = next_story;
        *cache = next_cache;
        Some(text)
    }

    /// Expand a PE headline's encoded string. Unsupported contextual commands
    /// return `None` transactionally; neither the CRT nor story state changes.
    pub(crate) fn expand_headline(
        &self,
        id: usize,
        crt: &mut Win95CrtRand,
        story: &mut PeHeadlineState,
    ) -> Option<Vec<u8>> {
        self.expand_headline_with_context(id, crt, story, None)
    }

    /// Context strings are already encoded in the Win95 OEM code page.
    pub(crate) fn expand_headline_with_context(
        &self,
        id: usize,
        crt: &mut Win95CrtRand,
        story: &mut PeHeadlineState,
        context: Option<&PeHeadlineContext<'_>>,
    ) -> Option<Vec<u8>> {
        let mut cache = PeTextChoiceCache::default();
        self.expand_headline_with_cache(id, crt, story, context, &mut cache)
    }

    /// The native headline caller clears the process choice cache first.
    pub(crate) fn expand_headline_with_cache(
        &self,
        id: usize,
        crt: &mut Win95CrtRand,
        story: &mut PeHeadlineState,
        context: Option<&PeHeadlineContext<'_>>,
        cache: &mut PeTextChoiceCache,
    ) -> Option<Vec<u8>> {
        let mut reset = PeTextChoiceCache::default();
        let output =
            self.expand_with_context(id, crt, story, context, PeTextMode::Headline, &mut reset)?;
        *cache = reset;
        Some(output)
    }

    /// PE mode 1 skips to the first `+`, then expands the body in mode 2.
    pub(crate) fn expand_body_with_context(
        &self,
        id: usize,
        crt: &mut Win95CrtRand,
        story: &mut PeHeadlineState,
        context: Option<&PeHeadlineContext<'_>>,
    ) -> Option<Vec<u8>> {
        let mut cache = PeTextChoiceCache::default();
        self.expand_body_with_cache(id, crt, story, context, &mut cache)
    }

    /// Native body expansion retains choices from the preceding headline.
    pub(crate) fn expand_body_with_cache(
        &self,
        id: usize,
        crt: &mut Win95CrtRand,
        story: &mut PeHeadlineState,
        context: Option<&PeHeadlineContext<'_>>,
        cache: &mut PeTextChoiceCache,
    ) -> Option<Vec<u8>> {
        self.expand_with_context(id, crt, story, context, PeTextMode::BodyStart, cache)
    }

    fn expand_with_context(
        &self,
        id: usize,
        crt: &mut Win95CrtRand,
        story: &mut PeHeadlineState,
        context: Option<&PeHeadlineContext<'_>>,
        mode: PeTextMode,
        cache: &mut PeTextChoiceCache,
    ) -> Option<Vec<u8>> {
        let mut next_crt = *crt;
        let mut next_story = *story;
        let mut next_cache = cache.clone();
        let mut output = Vec::new();
        self.expand_headline_inner(
            id,
            &mut next_crt,
            &mut next_story,
            &mut next_cache.choices,
            &mut output,
            0,
            context,
            mode,
        )?;
        *crt = next_crt;
        *story = next_story;
        *cache = next_cache;
        Some(output)
    }

    fn expand_headline_inner(
        &self,
        id: usize,
        crt: &mut Win95CrtRand,
        story: &mut PeHeadlineState,
        cache: &mut [u8; 250],
        output: &mut Vec<u8>,
        depth: usize,
        context: Option<&PeHeadlineContext<'_>>,
        mode: PeTextMode,
    ) -> Option<()> {
        if depth >= 64 || output.len() > 32_000 {
            return None;
        }
        let input = self.encoded_string(id)?;
        let (mut position, nested_mode) = match mode {
            PeTextMode::Headline => (0, PeTextMode::Headline),
            PeTextMode::BodyStart => match input.iter().position(|&b| b == b'+') {
                Some(index) => (index + 1, PeTextMode::Body),
                None => return Some(()),
            },
            PeTextMode::Body => (0, PeTextMode::Body),
        };
        while position < input.len() {
            let byte = input[position];
            position += 1;
            if byte == b'+' && nested_mode == PeTextMode::Headline {
                break;
            }
            let dictionary = match byte {
                1..=31 => Some(usize::from(byte)),
                127 => Some(32),
                158..=159 => Some(33 + usize::from(byte - 158)),
                169..=172 => Some(35 + usize::from(byte - 169)),
                174..=180 => Some(39 + usize::from(byte - 174)),
                184..=197 => Some(46 + usize::from(byte - 184)),
                200..=207 => Some(60 + usize::from(byte - 200)),
                217..=221 => Some(68 + usize::from(byte - 217)),
                223 => Some(73),
                238..=255 => Some(74 + usize::from(byte - 238)),
                _ => None,
            };
            if let Some(fragment) = dictionary {
                self.expand_headline_inner(
                    fragment,
                    crt,
                    story,
                    cache,
                    output,
                    depth + 1,
                    context,
                    nested_mode,
                )?;
                continue;
            }
            match byte {
                b'$' => output.extend_from_slice(story.param.to_string().as_bytes()),
                b'-' => output.extend_from_slice(b"\n     "),
                b'\\' => {
                    output.push(*input.get(position)?);
                    position += 1;
                }
                b'%' | b'<' => {
                    let bound = *input.get(position)?;
                    position += 1;
                    let candidate = if bound >= 2 {
                        (crt.random_u15() % u16::from(bound - 1) + 2) as u8
                    } else {
                        0
                    };
                    let value = if byte == b'<' {
                        if story.number == u8::MAX {
                            story.number = candidate;
                        }
                        story.number
                    } else {
                        candidate
                    };
                    output.extend_from_slice(value.to_string().as_bytes());
                }
                b'*' | b'[' | b'&' | b'@' | b'^' => {
                    let group = usize::from(*input.get(position)?);
                    position += 1;
                    let count = *self.counts.get(group)?;
                    if count == 0 || count > u16::from(u8::MAX) {
                        return None;
                    }
                    let choice = if byte == b'&' {
                        if u16::from(story.param) >= count {
                            story.param = 0;
                        }
                        story.param
                    } else {
                        let drawn = (crt.random_u15() % count) as u8;
                        match byte {
                            b'*' => drawn,
                            b'[' => {
                                let slot = cache.get_mut(group)?;
                                if *slot == u8::MAX {
                                    *slot = drawn;
                                }
                                *slot
                            }
                            b'@' => {
                                if u16::from(story.choice_a) >= count {
                                    story.choice_a = drawn;
                                }
                                story.choice_a
                            }
                            b'^' => {
                                if u16::from(story.choice_b) >= count {
                                    story.choice_b = drawn;
                                }
                                story.choice_b
                            }
                            _ => unreachable!(),
                        }
                    };
                    let nested = usize::from(*self.starts.get(group)?) + usize::from(choice);
                    self.expand_headline_inner(
                        nested,
                        crt,
                        story,
                        cache,
                        output,
                        depth + 1,
                        context,
                        nested_mode,
                    )?;
                }
                b'=' | b'>' | b'~' => {
                    let context = context?;
                    if let Some(state) = context.mutable_state {
                        let mut state = state.borrow_mut();
                        let index = match byte {
                            b'=' => None,
                            b'>' => Some(251 + usize::from(story.param)),
                            b'~' => Some(0),
                            _ => unreachable!(),
                        };
                        let value = match index {
                            None => &mut state.city,
                            Some(index) => state.labels.get_mut(index)?.as_mut()?,
                        };
                        let converted: Vec<u8> = value
                            .iter()
                            .copied()
                            .map(crate::win95_oem::ansi_to_oem)
                            .collect();
                        *value = converted
                            .iter()
                            .copied()
                            .map(crate::win95_oem::oem_to_ansi)
                            .collect();
                        output.extend_from_slice(&converted);
                        if output.len() > 32_000 {
                            return None;
                        }
                        continue;
                    }
                    let value = match byte {
                        b'=' => context.city_name_oem,
                        b'>' => context
                            .labels_oem
                            .get(251 + usize::from(story.param))?
                            .as_deref()?,
                        b'~' => context.labels_oem.first()?.as_deref()?,
                        _ => unreachable!(),
                    };
                    if value.contains(&0) {
                        return None;
                    }
                    output.extend_from_slice(value);
                }
                _ => output.push(byte),
            }
            if output.len() > 32_000 {
                return None;
            }
        }
        Some(())
    }
}

fn format_pe_headline_ascii(text: &mut [u8]) -> Option<()> {
    // The PE iterates through the expander's return value, which is one less
    // than the byte length. Its last byte is left to code-page conversion.
    let processed = text.len().saturating_sub(1);
    for byte in &mut text[..processed] {
        if *byte == 0xd5 {
            *byte = b'\'';
        }
    }
    for byte in text.iter_mut() {
        *byte = crate::win95_oem::oem_to_ansi(*byte);
    }
    let mut inside_word = false;
    for index in 0..processed {
        let byte = text[index];
        if byte == b'\'' {
            if index > 0 && text[index - 1] == b'L' {
                inside_word = false;
            }
        } else if !inside_word && crate::win95_oem::is_lower(byte) {
            text[index] = crate::win95_oem::upper(byte);
            inside_word = true;
        } else {
            inside_word = crate::win95_oem::is_alnum(byte);
        }
    }
    for byte in text.iter_mut() {
        *byte = crate::win95_oem::ansi_to_oem(*byte);
    }
    Some(())
}

fn format_pe_body_ascii(text: &mut [u8]) -> Option<()> {
    for byte in text.iter_mut() {
        *byte = match *byte {
            0xd4 | 0xd5 => b'\'',
            0xd2 | 0xd3 => b'"',
            value => value,
        };
    }
    let mut separators: i16 = 0;
    for byte in text.iter_mut() {
        if separators >= 2 && byte.is_ascii_lowercase() {
            *byte = byte.to_ascii_uppercase();
            separators = 0;
        } else if matches!(*byte, b' ' | b'.' | b'!' | b'?' | b'"') {
            separators = separators.wrapping_add(1);
        } else if *byte == b',' {
            separators = -1;
        } else {
            separators = 0;
        }
    }
    Some(())
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct PeTextChoiceCache {
    choices: [u8; 250],
}

impl Default for PeTextChoiceCache {
    fn default() -> Self {
        Self {
            choices: [u8::MAX; 250],
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum PeTextMode {
    Headline,
    BodyStart,
    Body,
}

pub(crate) struct PeHeadlineContext<'a> {
    pub city_name_oem: &'a [u8],
    pub labels_oem: &'a [Option<&'a [u8]>],
    pub mutable_state: Option<&'a std::cell::RefCell<PeHeadlineContextState>>,
}

/// ANSI source buffers modified by the PE's in-place OEM conversions.
pub(crate) struct PeHeadlineContextState {
    pub city: Vec<u8>,
    pub labels: Vec<Option<Vec<u8>>>,
}

impl PeHeadlineContextState {
    pub(crate) fn from_city(city: &crate::city::City) -> Option<Self> {
        fn ansi(text: &str) -> Option<Vec<u8>> {
            if !crate::city::win1252::can_encode(text) || text.contains('\0') {
                return None;
            }
            Some(crate::city::win1252::encode(text, usize::MAX))
        }
        Some(Self {
            city: ansi(&city.name)?,
            labels: (0..=u8::MAX)
                .map(|index| city.label_at(index).and_then(ansi))
                .collect(),
        })
    }

    pub(crate) fn apply_to_city(self, city: &mut crate::city::City) {
        city.name = crate::city::win1252::decode(&self.city);
        for (index, label) in self.labels.into_iter().enumerate() {
            if let Some(label) = label {
                let text = crate::city::win1252::decode(&label);
                if city.label_at(index as u8) != Some(text.as_str()) {
                    city.set_label_untracked(index as u8, text);
                }
            }
        }
    }
}

/// The four byte fields in a PE newspaper story row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct PeHeadlineState {
    pub param: u8,
    pub choice_b: u8,
    pub choice_a: u8,
    pub number: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct NewspaperRect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

/// Normal native path through `0x47afb0`: its text-drawing callee returns 1
/// regardless of measured width, so the split/retry branch is not taken.
pub(crate) fn pe_newspaper_headline_columns(
    rect: NewspaperRect,
    font_height: i16,
    crt: &mut Win95CrtRand,
) -> Option<(Vec<NewspaperRect>, Vec<NewspaperRect>)> {
    let text_bottom = rect.top.wrapping_add(i32::from(font_height) * 5 / 4);
    pe_newspaper_columns_47aeb0(rect, text_bottom, crt)
}

/// PE `0x47aeb0` splits the area below a measured headline into columns,
/// painting each with `0x47aca0` in order. `text_bottom` must come from the
/// native headline layout; it is an input to this isolated worker, not a
/// captured value to replay during ordinary gameplay.
pub(crate) fn pe_newspaper_columns_47aeb0(
    rect: NewspaperRect,
    text_bottom: i32,
    crt: &mut Win95CrtRand,
) -> Option<(Vec<NewspaperRect>, Vec<NewspaperRect>)> {
    let left = i16::try_from(rect.left).ok()?;
    let right = i16::try_from(rect.right).ok()?;
    let width = i32::from(right).checked_sub(i32::from(left))?;
    if !(2..=i32::from(i16::MAX)).contains(&width) {
        return None;
    }
    let count = width / 130 + 1;
    let mut next_crt = *crt;
    let mut columns = Vec::new();
    let mut bars = Vec::new();
    for index in 0..count {
        let column = NewspaperRect {
            left: i32::from(left) + width * index / count,
            top: text_bottom,
            right: i32::from(left) + width * (index + 1) / count,
            bottom: rect.bottom,
        };
        bars.extend(pe_newspaper_filler_47aca0(column, &mut next_crt)?);
        columns.push(column);
    }
    *crt = next_crt;
    Some((columns, bars))
}

/// PE `0x47aca0`: fill a newspaper column with randomized horizontal bars.
///
/// Coordinates are native Win32 RECT coordinates. This accepts ordinary
/// positive-width screen rectangles; unusually wide or tall rectangles return
/// `None` rather than emulating the original's signed WORD overflow or division
/// by zero. An empty or inverted vertical span returns an empty drawing.
///
/// The shared CRT is advanced for the native line-group and short-line choices.
/// The caller must invoke this at the original paint boundary with the actual
/// newspaper rectangle; the standalone worker is not yet wired to the UI.
#[allow(dead_code)]
pub(crate) fn pe_newspaper_filler_47aca0(
    rect: NewspaperRect,
    crt: &mut Win95CrtRand,
) -> Option<Vec<NewspaperRect>> {
    if rect.top >= rect.bottom {
        return Some(Vec::new());
    }
    let width = rect.right.checked_sub(rect.left)?;
    let height = rect.bottom.checked_sub(rect.top)?;
    if !(2..=32_767).contains(&width) || height > 32_767 {
        return None;
    }
    let rows = height / 5;
    let short_width = width * 2 / 3;
    let mut row = 0;
    let mut bars = Vec::new();
    loop {
        let bar = |row: i32, left: i32, right: i32| NewspaperRect {
            left: rect.left.wrapping_add(left),
            top: rect.top.wrapping_add(row * 5).wrapping_add(2),
            right,
            bottom: rect.top.wrapping_add(row * 5).wrapping_add(5),
        };
        bars.push(bar(row, 13, rect.right.wrapping_sub(3)));
        let additional = crt.random_u15() % 6;
        for _ in 0..additional {
            row += 1;
            if row >= rows {
                break;
            }
            bars.push(bar(row, 3, rect.right.wrapping_sub(3)));
        }
        row += 1;
        if row < rows {
            let shortening = i32::from(crt.random_u15()) % short_width;
            bars.push(bar(
                row,
                3,
                rect.right.wrapping_sub(shortening).wrapping_sub(3),
            ));
        }
        row += 1;
        if row >= rows {
            break;
        }
    }
    Some(bars)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn archive_opens_data_before_index_and_reports_the_failed_open() {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "citygame-pe-archive-{}-{nonce}",
            std::process::id()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let data = root.join("DATA_USA.DAT");
        let index = root.join("DATA_USA.IDX");
        let error = match PeNewspaperData::load(&data, &index) {
            Err(e) => e,
            Ok(_) => panic!("missing files loaded"),
        };
        assert_eq!(
            error.downcast_ref::<PeArchiveOpenError>().unwrap().path,
            data
        );
        std::fs::write(&data, []).unwrap();
        let error = match PeNewspaperData::load(&data, &index) {
            Err(e) => e,
            Ok(_) => panic!("missing index loaded"),
        };
        assert_eq!(
            error.downcast_ref::<PeArchiveOpenError>().unwrap().path,
            index
        );
        std::fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn archive_opens_index_before_attempting_data_read() {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "citygame-pe-archive-order-{}-{nonce}",
            std::process::id()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let index = root.join("missing.idx");
        // A directory can be opened on Unix, but cannot be read as a file.
        let error = match PeNewspaperData::load(&root, &index) {
            Err(e) => e,
            Ok(_) => panic!("directory loaded"),
        };
        assert_eq!(
            error.downcast_ref::<PeArchiveOpenError>().unwrap().path,
            index
        );
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn pe_paper_header_uses_sunday_and_paper_price_wording() {
        assert_eq!(
            pe_newspaper_header(1900, 0, 2),
            Some(("Sunday 4, January 1900".into(), 387))
        );
        assert_eq!(pe_newspaper_price(387), Some("Price 1¢"));
        assert_eq!(
            pe_newspaper_header(1950, 300, 0),
            Some(("Sunday 6, January 1951".into(), 378))
        );
        assert_eq!(pe_newspaper_header(1900, 0, 3), None);
    }

    #[test]
    fn pe_paper_tail_uses_saved_headings_and_weather_bytes() {
        assert_eq!(
            pe_newspaper_tail_labels(3, 4),
            Some(("Weather with Merle", "Survey"))
        );
        assert_eq!(pe_newspaper_tail_labels(6, 0), None);
        assert_eq!(pe_newspaper_weather_line(150, 10, 15), "50°F 10mph 15mm");
        assert_eq!(pe_newspaper_weather_line(0, 0, 0), "-100°F 0mph 0mm");
        assert_eq!(
            pe_newspaper_opinion_lines(1, 65, [0; 7], [0; 7]),
            ["Mayor Rated 65%"]
        );
        assert_eq!(
            pe_newspaper_opinion_lines(4, 0, [30, 20, 0, 9, 0, 0, 0], [2, 0, 1, 3, 4, 5, 6]),
            ["Crime 30%", "Traffic 20%"]
        );
    }

    #[test]
    #[ignore = "requires private original-executable weather corpus"]
    fn pe_paper_weather_line_matches_native_x86() {
        let file = std::env::var_os("CITYGAME_PE_NEWSPAPER_WEATHER").unwrap();
        let cases: Vec<serde_json::Value> =
            serde_json::from_slice(&std::fs::read(file).unwrap()).unwrap();
        assert_eq!(cases.len(), 54);
        for case in cases {
            let byte = |key: &str| case[key].as_u64().unwrap() as u8;
            assert_eq!(
                pe_newspaper_weather_line(byte("heat"), byte("wind"), byte("humidity")),
                case["text_latin1"].as_str().unwrap(),
                "{case}"
            );
        }
    }

    #[test]
    #[ignore = "requires private original-executable header corpus"]
    fn pe_paper_header_matches_native_x86() {
        let file = std::env::var_os("CITYGAME_PE_NEWSPAPER_HEADERS").unwrap();
        let cases: Vec<serde_json::Value> =
            serde_json::from_slice(&std::fs::read(file).unwrap()).unwrap();
        let months = [
            "January",
            "February",
            "March",
            "April",
            "May",
            "June",
            "July",
            "August",
            "September",
            "October",
            "November",
            "December",
        ];
        assert_eq!(cases.len(), 216);
        for case in cases {
            let number = |key: &str| case[key].as_u64().unwrap();
            let founded = number("founded");
            let year = founded + number("elapsed_years");
            let expected = format!(
                "Sunday {}, {} {year}",
                number("day"),
                months[number("month") as usize]
            );
            assert_eq!(
                pe_newspaper_header(
                    founded as u16,
                    number("age") as u32,
                    number("wording") as u8
                ),
                Some((expected, number("price_resource") as u32)),
                "{case}"
            );
        }
    }

    #[test]
    #[ignore = "requires original DATA and private native paper-tail fixtures"]
    fn pe_paper_through_tail_matches_native_region() {
        let dir = std::path::PathBuf::from(std::env::var_os("CITYGAME_WIN95_DATA_DIR").unwrap());
        let archive =
            PeNewspaperData::load(&dir.join("data_usa.dat"), &dir.join("data_usa.idx")).unwrap();
        let cases: serde_json::Value = serde_json::from_slice(
            &std::fs::read(std::env::var_os("CITYGAME_PE_NEWSPAPER_TAIL").unwrap()).unwrap(),
        )
        .unwrap();
        let rect = |v: &serde_json::Value| {
            let n: Vec<_> = v
                .as_array()
                .unwrap()
                .iter()
                .map(|x| x.as_i64().unwrap() as i32)
                .collect();
            NewspaperRect {
                left: n[0],
                top: n[1],
                right: n[2],
                bottom: n[3],
            }
        };
        let mut city =
            crate::city::City::new_blank("/tmp/citygame-pe-tail.sc2", "Civic Haven").unwrap();
        let mut tables = crate::city::PeNewspaperTables {
            papers: [0; 30],
            stories: [0; 54],
        };
        for (row, id) in (0..=8).zip([42, 42, 43, 44, 5, 0, 0, 0, 44]) {
            tables.stories[row * 6..row * 6 + 6].copy_from_slice(&[id, 0, 0, 255, 255, 255]);
        }
        let mut labels: [Option<&[u8]>; 256] = [None; 256];
        for (index, label) in [0, 251, 252, 253, 254, 255].into_iter().zip([
            b"The Region".as_slice(),
            b"North Harbor",
            b"East Bay",
            b"West Ridge",
            b"South Port",
            b"Centre",
        ]) {
            labels[index] = Some(label);
        }
        let context = PeHeadlineContext {
            city_name_oem: b"Civic Haven",
            labels_oem: &labels,
            mutable_state: None,
        };
        let hash = |bytes: &[u8]| {
            bytes.iter().fold(0xcbf29ce484222325u64, |v, b| {
                (v ^ u64::from(*b)).wrapping_mul(0x100000001b3)
            })
        };
        for case in cases.as_array().unwrap() {
            tables.papers[3] = case["paper_type"].as_u64().unwrap() as u32;
            tables.papers[1] = case["style"].as_u64().unwrap() as u32;
            city.install_pe_newspaper_tables(tables.clone());
            city.set_misc_u32_at(0x10, 175);
            city.set_pe_mayor_approval(case["approval"].as_u64().unwrap() as u16);
            let counts: Vec<_> = case["counts"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_u64().unwrap() as u16)
                .collect();
            city.set_pe_opinion_poll_results_for_test(counts.try_into().unwrap());
            let mut composed_city = city.clone();
            let base = case["base"].as_u64().unwrap() as u32;
            let seed = PeNewspaperSeed {
                days: 175,
                startup_tick: (base as i16) - 7,
                paper: 0,
                slot: 0,
            };
            let style = case["style"].as_u64().unwrap() as u8;
            let cached = case["cached"].as_bool().unwrap();
            let mut crt = Win95CrtRand::new(99);
            let lead = archive
                .render_lead_article_ascii(
                    &mut city,
                    style,
                    seed,
                    &mut crt,
                    cached.then_some(409),
                    Some(&context),
                    |_| 0,
                )
                .unwrap();
            let articles = archive
                .render_secondary_articles_ascii(
                    &mut city,
                    style,
                    seed,
                    &mut crt,
                    Some(&context),
                    |_| 0,
                )
                .unwrap();
            assert_eq!(
                u64::from(crt.seed()),
                case["checkpoints"]["0x478523"].as_u64().unwrap(),
                "{case}"
            );
            let tail = archive
                .render_paper_tail_ascii(&mut city, style, seed, &mut crt, Some(&context))
                .unwrap();
            let mut composed_crt = Win95CrtRand::new(99);
            let composed = archive
                .render_paper_region_ascii(
                    &mut composed_city,
                    0,
                    (base as i16) - 7,
                    &mut composed_crt,
                    cached.then_some(409),
                    Some(&context),
                    |_| 0,
                )
                .unwrap();
            assert_eq!(composed.style, style);
            assert_eq!(composed.lead, lead);
            assert_eq!(composed.secondary, articles);
            assert_eq!(composed.tail, tail);
            assert_eq!(composed_crt.seed(), crt.seed());
            assert_eq!(
                composed_city.pe_newspaper_tables().unwrap().stories,
                city.pe_newspaper_tables().unwrap().stories
            );
            assert_eq!(
                u64::from(tail[0].crt_after),
                case["checkpoints"]["0x47881f"].as_u64().unwrap(),
                "{case}"
            );
            let actual_heads = std::iter::once(&lead.headline)
                .chain(articles.iter().map(|v| &v.headline))
                .chain(tail.iter().filter_map(|v| v.headline.as_ref()));
            let expected_heads = case["headlines"].as_array().unwrap();
            let mut count = 0;
            for (headline, expected) in actual_heads.zip(expected_heads) {
                assert_eq!(
                    (headline.len() as u64, hash(headline)),
                    (
                        expected["length"].as_u64().unwrap(),
                        expected["hash"].as_u64().unwrap()
                    ),
                    "{case}"
                );
                count += 1;
            }
            assert_eq!(count, expected_heads.len());
            let mut actual_bytes = Vec::new();
            for row in 0..=8 {
                let (id, state) = city.pe_newspaper_text_row(row).unwrap();
                actual_bytes.extend_from_slice(&id.to_le_bytes());
                actual_bytes.extend_from_slice(&0u16.to_le_bytes());
                actual_bytes.extend_from_slice(&[
                    state.param,
                    state.choice_b,
                    state.choice_a,
                    state.number,
                ]);
            }
            let expected_bytes: Vec<_> = case["story_bytes"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_u64().unwrap() as u8)
                .collect();
            assert_eq!(actual_bytes, expected_bytes, "{case}");
            let columns: Vec<_> = lead
                .columns
                .iter()
                .copied()
                .chain(articles.iter().flat_map(|v| v.columns.iter().copied()))
                .chain(tail.iter().map(|v| v.filler))
                .collect();
            let bars: Vec<_> = lead
                .bars
                .iter()
                .copied()
                .chain(articles.iter().flat_map(|v| v.bars.iter().copied()))
                .chain(tail.iter().flat_map(|v| v.bars.iter().copied()))
                .collect();
            let expected_columns: Vec<_> = case["columns"]
                .as_array()
                .unwrap()
                .iter()
                .map(rect)
                .collect();
            let expected_bars: Vec<_> = case["bars"].as_array().unwrap().iter().map(rect).collect();
            assert_eq!(columns, expected_columns, "{case}");
            assert_eq!(bars, expected_bars, "{case}");
            assert_eq!(
                u64::from(crt.seed()),
                case["crt"].as_u64().unwrap(),
                "{case}"
            );
        }
        assert_eq!(cases.as_array().unwrap().len(), 54);
        tables.stories[4 * 6] = 999;
        city.install_pe_newspaper_tables(tables);
        let before = city.pe_newspaper_tables().unwrap().stories;
        let mut failed_crt = Win95CrtRand::new(99);
        assert!(
            archive
                .render_paper_region_ascii(
                    &mut city,
                    0,
                    123,
                    &mut failed_crt,
                    None,
                    Some(&context),
                    |_| 0
                )
                .is_none()
        );
        assert_eq!(city.pe_newspaper_tables().unwrap().stories, before);
        assert_eq!(failed_crt.seed(), 99);
    }

    #[test]
    #[ignore = "requires original DATA and private native lead-loop fixtures"]
    fn pe_lead_and_secondary_match_native_paint_region() {
        let dir = std::path::PathBuf::from(std::env::var_os("CITYGAME_WIN95_DATA_DIR").unwrap());
        let archive =
            PeNewspaperData::load(&dir.join("data_usa.dat"), &dir.join("data_usa.idx")).unwrap();
        let cases: serde_json::Value = serde_json::from_slice(
            &std::fs::read(std::env::var_os("CITYGAME_PE_NEWSPAPER_LEAD").unwrap()).unwrap(),
        )
        .unwrap();
        let rect = |v: &serde_json::Value| {
            let n: Vec<_> = v
                .as_array()
                .unwrap()
                .iter()
                .map(|x| x.as_i64().unwrap() as i32)
                .collect();
            NewspaperRect {
                left: n[0],
                top: n[1],
                right: n[2],
                bottom: n[3],
            }
        };
        let mut city =
            crate::city::City::new_blank("/tmp/citygame-pe-lead.sc2", "Civic Haven").unwrap();
        let mut tables = crate::city::PeNewspaperTables {
            papers: [0; 30],
            stories: [0; 54],
        };
        for (row, id) in (0..=4).zip([42, 42, 43, 44, 5]) {
            tables.stories[row * 6..row * 6 + 6].copy_from_slice(&[id, 0, 0, 255, 255, 255]);
        }
        let mut labels: [Option<&[u8]>; 256] = [None; 256];
        for (index, label) in [0, 251, 252, 253, 254, 255].into_iter().zip([
            b"The Region".as_slice(),
            b"North Harbor",
            b"East Bay",
            b"West Ridge",
            b"South Port",
            b"Centre",
        ]) {
            labels[index] = Some(label);
        }
        let context = PeHeadlineContext {
            city_name_oem: b"Civic Haven",
            labels_oem: &labels,
            mutable_state: None,
        };
        let hash = |bytes: &[u8]| {
            bytes.iter().fold(0xcbf29ce484222325u64, |v, b| {
                (v ^ u64::from(*b)).wrapping_mul(0x100000001b3)
            })
        };
        for case in cases.as_array().unwrap() {
            city.install_pe_newspaper_tables(tables.clone());
            let base = case["base"].as_u64().unwrap() as u32;
            let seed = PeNewspaperSeed {
                days: 0,
                startup_tick: base as i16,
                paper: 0,
                slot: 0,
            };
            let style = case["style"].as_u64().unwrap() as u8;
            let cached = case["cached"].as_bool().unwrap();
            let mut crt = Win95CrtRand::new(99);
            let lead = archive
                .render_lead_article_ascii(
                    &mut city,
                    style,
                    seed,
                    &mut crt,
                    cached.then_some(409),
                    Some(&context),
                    |_| 0,
                )
                .unwrap();
            let expected_picture = if cached {
                409
            } else {
                case["picture"][0].as_u64().unwrap() as u16
            };
            assert_eq!(lead.picture, expected_picture, "{case}");
            assert_eq!(
                u64::from(crt.seed()),
                case["checkpoints"]["0x47843b"].as_u64().unwrap(),
                "{case}"
            );
            let articles = archive
                .render_secondary_articles_ascii(
                    &mut city,
                    style,
                    seed,
                    &mut crt,
                    Some(&context),
                    |_| 0,
                )
                .unwrap();
            let actual_heads =
                std::iter::once(&lead.headline).chain(articles.iter().map(|v| &v.headline));
            for (headline, expected) in actual_heads.zip(case["headlines"].as_array().unwrap()) {
                assert_eq!(
                    (headline.len() as u64, hash(headline)),
                    (
                        expected["length"].as_u64().unwrap(),
                        expected["hash"].as_u64().unwrap()
                    ),
                    "{case}"
                );
            }
            let mut actual_bytes = Vec::new();
            for row in 0..=4 {
                let (id, state) = city.pe_newspaper_text_row(row).unwrap();
                actual_bytes.extend_from_slice(&id.to_le_bytes());
                actual_bytes.extend_from_slice(&0u16.to_le_bytes());
                actual_bytes.extend_from_slice(&[
                    state.param,
                    state.choice_b,
                    state.choice_a,
                    state.number,
                ]);
            }
            let expected_bytes: Vec<_> = case["story_bytes"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_u64().unwrap() as u8)
                .collect();
            assert_eq!(actual_bytes, expected_bytes, "{case}");
            let columns: Vec<_> = lead
                .columns
                .iter()
                .copied()
                .chain(articles.iter().flat_map(|v| v.columns.iter().copied()))
                .collect();
            let bars: Vec<_> = lead
                .bars
                .iter()
                .copied()
                .chain(articles.iter().flat_map(|v| v.bars.iter().copied()))
                .collect();
            let expected_columns: Vec<_> = case["columns"]
                .as_array()
                .unwrap()
                .iter()
                .map(rect)
                .collect();
            let expected_bars: Vec<_> = case["bars"].as_array().unwrap().iter().map(rect).collect();
            assert_eq!(columns, expected_columns, "{case}");
            assert_eq!(bars, expected_bars, "{case}");
            assert_eq!(
                u64::from(crt.seed()),
                case["crt"].as_u64().unwrap(),
                "{case}"
            );
        }
        assert_eq!(cases.as_array().unwrap().len(), 18);
    }

    #[test]
    #[ignore = "requires original DATA and private native full secondary-loop fixtures"]
    fn pe_secondary_articles_match_native_headlines_and_loop() {
        let dir = std::path::PathBuf::from(std::env::var_os("CITYGAME_WIN95_DATA_DIR").unwrap());
        let archive =
            PeNewspaperData::load(&dir.join("data_usa.dat"), &dir.join("data_usa.idx")).unwrap();
        let cases: serde_json::Value = serde_json::from_slice(
            &std::fs::read(std::env::var_os("CITYGAME_PE_NEWSPAPER_SECONDARY_FULL").unwrap())
                .unwrap(),
        )
        .unwrap();
        let rect = |v: &serde_json::Value| {
            let n: Vec<_> = v
                .as_array()
                .unwrap()
                .iter()
                .map(|x| x.as_i64().unwrap() as i32)
                .collect();
            NewspaperRect {
                left: n[0],
                top: n[1],
                right: n[2],
                bottom: n[3],
            }
        };
        let mut city =
            crate::city::City::new_blank("/tmp/citygame-pe-secondary-full.sc2", "Civic Haven")
                .unwrap();
        let mut tables = crate::city::PeNewspaperTables {
            papers: [0; 30],
            stories: [0; 54],
        };
        for (row, id) in (1..=4).zip([42, 43, 44, 5]) {
            tables.stories[row * 6..row * 6 + 6].copy_from_slice(&[id, 0, 0, 255, 255, 255]);
        }
        let mut labels: [Option<&[u8]>; 256] = [None; 256];
        for (index, label) in [0, 251, 252, 253, 254, 255].into_iter().zip([
            b"The Region".as_slice(),
            b"North Harbor",
            b"East Bay",
            b"West Ridge",
            b"South Port",
            b"Centre",
        ]) {
            labels[index] = Some(label);
        }
        let context = PeHeadlineContext {
            city_name_oem: b"Civic Haven",
            labels_oem: &labels,
            mutable_state: None,
        };
        let hash = |bytes: &[u8]| {
            bytes.iter().fold(0xcbf29ce484222325u64, |v, b| {
                (v ^ u64::from(*b)).wrapping_mul(0x100000001b3)
            })
        };
        for case in cases.as_array().unwrap() {
            city.install_pe_newspaper_tables(tables.clone());
            let base = case["base"].as_u64().unwrap() as u32;
            let seed = PeNewspaperSeed {
                days: 0,
                startup_tick: base as i16,
                paper: 0,
                slot: 0,
            };
            let mut crt = Win95CrtRand::new(99);
            let articles = archive
                .render_secondary_articles_ascii(
                    &mut city,
                    case["style"].as_u64().unwrap() as u8,
                    seed,
                    &mut crt,
                    Some(&context),
                    |_| 0,
                )
                .unwrap();
            for (article, expected) in articles.iter().zip(case["headlines"].as_array().unwrap()) {
                assert_eq!(
                    (article.headline.len() as u64, hash(&article.headline)),
                    (
                        expected["length"].as_u64().unwrap(),
                        expected["hash"].as_u64().unwrap()
                    ),
                    "{case}"
                );
            }
            let expected_bytes: Vec<_> = case["story_bytes"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_u64().unwrap() as u8)
                .collect();
            let mut actual_bytes = Vec::new();
            for row in 1..=4 {
                let (id, state) = city.pe_newspaper_text_row(row).unwrap();
                actual_bytes.extend_from_slice(&id.to_le_bytes());
                actual_bytes.extend_from_slice(&0u16.to_le_bytes());
                actual_bytes.extend_from_slice(&[
                    state.param,
                    state.choice_b,
                    state.choice_a,
                    state.number,
                ]);
            }
            assert_eq!(actual_bytes, expected_bytes, "{case}");
            let columns: Vec<_> = articles
                .iter()
                .flat_map(|v| v.columns.iter().copied())
                .collect();
            let bars: Vec<_> = articles
                .iter()
                .flat_map(|v| v.bars.iter().copied())
                .collect();
            let expected_columns: Vec<_> = case["columns"]
                .as_array()
                .unwrap()
                .iter()
                .map(rect)
                .collect();
            let expected_bars: Vec<_> = case["bars"].as_array().unwrap().iter().map(rect).collect();
            assert_eq!(columns, expected_columns, "{case}");
            assert_eq!(bars, expected_bars, "{case}");
            assert_eq!(
                u64::from(crt.seed()),
                case["crt"].as_u64().unwrap(),
                "{case}"
            );
        }
        assert_eq!(cases.as_array().unwrap().len(), 9);
    }

    #[test]
    #[ignore = "requires original DATA and private native secondary-loop fixtures"]
    fn pe_secondary_articles_match_native_loop() {
        let dir = std::path::PathBuf::from(std::env::var_os("CITYGAME_WIN95_DATA_DIR").unwrap());
        let archive =
            PeNewspaperData::load(&dir.join("data_usa.dat"), &dir.join("data_usa.idx")).unwrap();
        let cases: serde_json::Value = serde_json::from_slice(
            &std::fs::read(std::env::var_os("CITYGAME_PE_NEWSPAPER_SECONDARY").unwrap()).unwrap(),
        )
        .unwrap();
        let rect = |v: &serde_json::Value| {
            let n: Vec<_> = v
                .as_array()
                .unwrap()
                .iter()
                .map(|x| x.as_i64().unwrap() as i32)
                .collect();
            NewspaperRect {
                left: n[0],
                top: n[1],
                right: n[2],
                bottom: n[3],
            }
        };
        let mut city =
            crate::city::City::new_blank("/tmp/citygame-pe-secondary.sc2", "Civic Haven").unwrap();
        let mut tables = crate::city::PeNewspaperTables {
            papers: [0; 30],
            stories: [0; 54],
        };
        for row in 1..=4 {
            tables.stories[row * 6..row * 6 + 6].copy_from_slice(&[42, 0, 0, 255, 255, 255]);
        }
        let labels: [Option<&[u8]>; 256] = [None; 256];
        let context = PeHeadlineContext {
            city_name_oem: b"Civic Haven",
            labels_oem: &labels,
            mutable_state: None,
        };
        for case in cases.as_array().unwrap() {
            city.install_pe_newspaper_tables(tables.clone());
            let base = case["base"].as_u64().unwrap() as u32;
            let seed = PeNewspaperSeed {
                days: 0,
                startup_tick: base as i16,
                paper: 0,
                slot: 0,
            };
            assert_eq!(seed.layout(), base);
            let mut crt = Win95CrtRand::new(99);
            let articles = archive
                .render_secondary_articles_ascii(
                    &mut city,
                    case["style"].as_u64().unwrap() as u8,
                    seed,
                    &mut crt,
                    Some(&context),
                    |_| 0,
                )
                .unwrap();
            let rows: Vec<_> = articles.iter().map(|v| v.story_row as u64).collect();
            let columns: Vec<_> = articles
                .iter()
                .flat_map(|v| v.columns.iter().copied())
                .collect();
            let bars: Vec<_> = articles
                .iter()
                .flat_map(|v| v.bars.iter().copied())
                .collect();
            let expected_rows: Vec<_> = case["rows"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_u64().unwrap())
                .collect();
            let expected_columns: Vec<_> = case["columns"]
                .as_array()
                .unwrap()
                .iter()
                .map(rect)
                .collect();
            let expected_bars: Vec<_> = case["bars"].as_array().unwrap().iter().map(rect).collect();
            assert_eq!(rows, expected_rows, "{case}");
            assert_eq!(columns, expected_columns, "{case}");
            assert_eq!(bars, expected_bars, "{case}");
            assert_eq!(
                u64::from(crt.seed()),
                case["crt"].as_u64().unwrap(),
                "{case}"
            );
        }
        assert_eq!(cases.as_array().unwrap().len(), 9);
    }

    #[test]
    fn pe_headline_truncation_removes_whole_trailing_words() {
        assert_eq!(
            pe_truncate_headline_ascii(b"ONE TWO THREE", 20, |s| s.len() as i32 * 3),
            Some(b"ONE".to_vec())
        );
        assert_eq!(
            pe_truncate_headline_ascii(b"VERYLONG", 3, |s| s.len() as i32 * 3),
            Some(Vec::new())
        );
        assert_eq!(pe_truncate_headline_ascii(b"A\0B", 10, |_| 0), None);
    }

    #[test]
    #[ignore = "requires private native headline truncation fixtures"]
    fn pe_headline_truncation_matches_native_x86() {
        let cases: serde_json::Value = serde_json::from_slice(
            &std::fs::read(std::env::var_os("CITYGAME_PE_NEWSPAPER_TRUNCATE").unwrap()).unwrap(),
        )
        .unwrap();
        for case in cases.as_array().unwrap() {
            let mut calls = Vec::new();
            let actual = pe_truncate_headline_ascii(
                case["text"].as_str().unwrap().as_bytes(),
                case["width"].as_i64().unwrap() as i32,
                |s| {
                    calls.push(s.len() as u64);
                    s.len() as i32 * 3
                },
            )
            .unwrap();
            assert_eq!(
                actual,
                case["result"].as_str().unwrap().as_bytes(),
                "{case}"
            );
            let expected: Vec<_> = case["calls"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_u64().unwrap())
                .collect();
            assert_eq!(calls, expected, "{case}");
        }
        assert_eq!(cases.as_array().unwrap().len(), 240);
    }

    #[test]
    #[ignore = "requires private native headline layout fixtures"]
    fn pe_headline_layout_matches_native_x86() {
        let cases: serde_json::Value = serde_json::from_slice(
            &std::fs::read(std::env::var_os("CITYGAME_PE_NEWSPAPER_HEADLINE_LAYOUT").unwrap())
                .unwrap(),
        )
        .unwrap();
        for case in cases.as_array().unwrap() {
            let text = case["text"].as_str().unwrap().as_bytes();
            let width = case["width"].as_i64().unwrap() as i32;
            let font = case["font"].as_i64().unwrap() as i16;
            let mut calls = Vec::new();
            let (top, _, _) = pe_headline_layout_with_fit(text, 76, font, |line, size| {
                let ok = line.len() as i32 * i32::from(size) <= width * 2;
                calls.push((line.len() as u64, i64::from(size), ok));
                ok
            })
            .unwrap();
            let expected: Vec<_> = case["calls"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| {
                    (
                        v["count"].as_u64().unwrap(),
                        v["size"].as_i64().unwrap(),
                        v["ok"].as_bool().unwrap(),
                    )
                })
                .collect();
            assert_eq!(calls, expected, "{case}");
            assert_eq!(i64::from(top), case["top"].as_i64().unwrap(), "{case}");
        }
        assert_eq!(cases.as_array().unwrap().len(), 150);
    }

    #[test]
    fn pe_headline_split_prefers_right_on_equal_distance() {
        let mut text = [b'A'; 80];
        text[31] = b' ';
        text[33] = b' ';
        assert_eq!(pe_headline_split_offset(&text), Some(33));
        assert_eq!(pe_headline_split_offset(b"short"), None);
        assert_eq!(pe_headline_split_offset(&[0x80; 128]), None);
    }

    #[test]
    #[ignore = "requires private native headline split fixtures"]
    fn pe_headline_split_matches_native_x86() {
        let cases: serde_json::Value = serde_json::from_slice(
            &std::fs::read(std::env::var_os("CITYGAME_PE_NEWSPAPER_SPLITS").unwrap()).unwrap(),
        )
        .unwrap();
        for case in cases.as_array().unwrap() {
            assert_eq!(
                pe_headline_split_offset(case["text"].as_str().unwrap().as_bytes()),
                Some(case["split"].as_u64().unwrap() as usize),
                "{case}"
            );
        }
        assert_eq!(cases.as_array().unwrap().len(), 512);
    }

    #[test]
    fn pe_columns_split_at_native_130_pixel_threshold() {
        let mut crt = Win95CrtRand::new(123);
        let rect = NewspaperRect {
            left: 0,
            top: 76,
            right: 213,
            bottom: 300,
        };
        let (columns, _) = pe_newspaper_columns_47aeb0(rect, 88, &mut crt).unwrap();
        assert_eq!(
            columns,
            vec![
                NewspaperRect {
                    left: 0,
                    top: 88,
                    right: 106,
                    bottom: 300
                },
                NewspaperRect {
                    left: 106,
                    top: 88,
                    right: 213,
                    bottom: 300
                }
            ]
        );
        assert_eq!(crt.seed(), 4_107_142_183);
        let before = crt;
        assert!(
            pe_newspaper_columns_47aeb0(NewspaperRect { right: 1, ..rect }, 88, &mut crt).is_none()
        );
        assert_eq!(crt.seed(), before.seed());
    }

    #[test]
    #[ignore = "requires private native column fixtures"]
    fn pe_columns_match_native_x86() {
        let cases: serde_json::Value = serde_json::from_slice(
            &std::fs::read(std::env::var_os("CITYGAME_PE_NEWSPAPER_COLUMNS").unwrap()).unwrap(),
        )
        .unwrap();
        let rect = |value: &serde_json::Value| {
            let n: Vec<_> = value
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_i64().unwrap() as i32)
                .collect();
            NewspaperRect {
                left: n[0],
                top: n[1],
                right: n[2],
                bottom: n[3],
            }
        };
        for case in cases.as_array().unwrap() {
            let mut crt = Win95CrtRand::new(case["seed"].as_u64().unwrap() as u32);
            let (columns, bars) = pe_newspaper_columns_47aeb0(
                rect(&case["rect"]),
                case["text_bottom"].as_i64().unwrap() as i32,
                &mut crt,
            )
            .unwrap();
            let expected_columns: Vec<_> = case["columns"]
                .as_array()
                .unwrap()
                .iter()
                .map(rect)
                .collect();
            let expected_bars: Vec<_> = case["bars"].as_array().unwrap().iter().map(rect).collect();
            assert_eq!(columns, expected_columns, "{case}");
            assert_eq!(bars, expected_bars, "{case}");
            assert_eq!(
                u64::from(crt.seed()),
                case["crt"].as_u64().unwrap(),
                "{case}"
            );
            let mut normal_crt = Win95CrtRand::new(case["seed"].as_u64().unwrap() as u32);
            let (normal_columns, normal_bars) =
                pe_newspaper_headline_columns(rect(&case["rect"]), 10, &mut normal_crt).unwrap();
            assert_eq!(normal_columns, expected_columns, "{case}");
            assert_eq!(normal_bars, expected_bars, "{case}");
            assert_eq!(normal_crt.seed(), crt.seed(), "{case}");
        }
        assert_eq!(cases.as_array().unwrap().len(), 252);
    }

    #[test]
    fn pe_picture_selection_reseeds_even_for_fixed_resources() {
        let seed = PeNewspaperSeed {
            days: 0,
            startup_tick: 123,
            paper: 0,
            slot: 0,
        };
        let mut crt = Win95CrtRand::new(9);
        assert_eq!(pe_newspaper_first_picture(2, seed, &mut crt), Some(404));
        assert_eq!(crt.seed(), 144);
        assert_eq!(pe_newspaper_first_picture(0, seed, &mut crt), Some(404));
        assert_eq!(crt.seed(), 33_348_883);
        assert_eq!(pe_newspaper_first_picture(67, seed, &mut crt), None);
        assert_eq!(crt.seed(), 33_348_883);
    }

    #[test]
    #[ignore = "requires private x86 picture fixtures"]
    fn pe_picture_selection_matches_native_x86() {
        let cases: serde_json::Value = serde_json::from_slice(
            &std::fs::read(std::env::var_os("CITYGAME_PE_NEWSPAPER_PICTURES").unwrap()).unwrap(),
        )
        .unwrap();
        for case in cases.as_array().unwrap() {
            let base = case["base"].as_u64().unwrap() as u32;
            let seed = PeNewspaperSeed {
                days: 0,
                startup_tick: base as i16,
                paper: 0,
                slot: 0,
            };
            assert_eq!(seed.layout(), base);
            let mut crt = Win95CrtRand::new(0x12345678);
            let picture = pe_newspaper_first_picture(
                case["story"].as_u64().unwrap() as usize,
                seed,
                &mut crt,
            )
            .unwrap();
            assert_eq!(
                u64::from(picture),
                case["picture"].as_u64().unwrap(),
                "{case}"
            );
            assert_eq!(
                u64::from(crt.seed()),
                case["crt"].as_u64().unwrap(),
                "{case}"
            );
        }
        assert_eq!(cases.as_array().unwrap().len(), 201);
    }

    #[test]
    #[ignore = "requires private native caller corpus and original DATA"]
    fn pe_city_article_persists_native_story_choices() {
        let dir = std::path::PathBuf::from(std::env::var_os("CITYGAME_WIN95_DATA_DIR").unwrap());
        let archive =
            PeNewspaperData::load(&dir.join("data_usa.dat"), &dir.join("data_usa.idx")).unwrap();
        let fixture: serde_json::Value = serde_json::from_slice(
            &std::fs::read(std::env::var_os("CITYGAME_PE_NEWSPAPER_CALLERS").unwrap()).unwrap(),
        )
        .unwrap();
        let case = fixture["cases"]
            .as_array()
            .unwrap()
            .iter()
            .find(|v| v["story"] == 4 && v["seed"] == 1)
            .unwrap();
        let mut labels: [Option<&[u8]>; 256] = [None; 256];
        for (slot, name) in [0, 251, 252, 253, 254, 255]
            .into_iter()
            .zip(fixture["context"]["labels"].as_array().unwrap())
        {
            labels[slot] = Some(name.as_str().unwrap().as_bytes());
        }
        let context = PeHeadlineContext {
            city_name_oem: fixture["context"]["city"].as_str().unwrap().as_bytes(),
            labels_oem: &labels,
            mutable_state: None,
        };
        let path =
            std::env::temp_dir().join(format!("citygame-pe-article-{}.sc2", std::process::id()));
        let mut city = crate::city::City::new_blank(&path, "Civic Haven").unwrap();
        let mut tables = crate::city::PeNewspaperTables {
            papers: [0; 30],
            stories: [0; 54],
        };
        tables.stories[8 * 6..9 * 6].copy_from_slice(&[4, 47, 0, 255, 255, 255]);
        city.install_pe_newspaper_tables(tables);
        let seed = PeNewspaperSeed {
            days: 0,
            startup_tick: -34,
            paper: 0,
            slot: 5,
        };
        let mut crt = Win95CrtRand::new(99);
        let before = city.pe_newspaper_text_row(8);
        assert!(
            archive
                .render_city_article_ascii(&mut city, 2, seed, &mut crt, Some(&context))
                .is_none()
        );
        assert_eq!(city.pe_newspaper_text_row(8), before);
        assert_eq!(crt.seed(), 99);
        let (headline, body) = archive
            .render_city_article_ascii(&mut city, 0, seed, &mut crt, Some(&context))
            .unwrap();
        let hash = |bytes: &[u8]| {
            bytes.iter().fold(0xcbf29ce484222325u64, |v, b| {
                (v ^ u64::from(*b)).wrapping_mul(0x100000001b3)
            })
        };
        assert_eq!(
            (headline.len() as u64, hash(&headline)),
            (
                case["output_len"].as_u64().unwrap(),
                case["output_fnv64"].as_u64().unwrap()
            )
        );
        assert_eq!(
            (body.len() as u64, hash(&body), u64::from(crt.seed())),
            (
                case["body"]["output_len"].as_u64().unwrap(),
                case["body"]["output_fnv64"].as_u64().unwrap(),
                case["body"]["final_crt"].as_u64().unwrap()
            )
        );
        let (id, state) = city.pe_newspaper_text_row(8).unwrap();
        assert_eq!(id, 4);
        assert_eq!(
            [state.param, state.choice_b, state.choice_a, state.number],
            [0, 255, 43, 255]
        );
        assert_eq!(city.pe_newspaper_story_priority_at(8), Some(47));
        city.save(&path).unwrap();
        let loaded = crate::city::City::load(&path).unwrap();
        assert_eq!(loaded.pe_newspaper_text_row(8), Some((id, state)));
        assert_eq!(loaded.pe_newspaper_story_priority_at(8), Some(47));
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn pe_article_positions_use_native_story_rows() {
        assert_eq!(pe_article_story_row(5, 0), Some(8));
        assert_eq!(pe_article_story_row(6, 1), Some(7));
        assert_eq!(pe_article_story_row(5, 2), None);
        assert_eq!(pe_article_story_row(6, 2), None);
        assert_eq!(pe_article_story_row(10, 2), Some(4));
        assert_eq!(pe_article_story_row(11, 0), None);
    }

    #[test]
    #[ignore = "requires private native rectangle-initializer capture"]
    fn pe_newspaper_all_layout_rectangles_match_native_x86() {
        let cases: serde_json::Value = serde_json::from_slice(
            &std::fs::read(std::env::var_os("CITYGAME_PE_NEWSPAPER_ALL_RECTS").unwrap()).unwrap(),
        )
        .unwrap();
        for (style, rows) in cases.as_array().unwrap().iter().enumerate() {
            assert_eq!(rows.as_array().unwrap().len(), 11);
            for (position, row) in rows.as_array().unwrap().iter().enumerate() {
                let values: Vec<i32> = row
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|v| v.as_i64().unwrap() as i32)
                    .collect();
                let actual = pe_newspaper_layout_rect(style as u8, position as u16).unwrap();
                assert_eq!(
                    [actual.left, actual.top, actual.right, actual.bottom],
                    values.as_slice(),
                    "style={style} position={position}"
                );
            }
        }
        assert_eq!(cases.as_array().unwrap().len(), 3);
        assert!(pe_newspaper_layout_rect(3, 0).is_none());
        assert!(pe_newspaper_layout_rect(0, 11).is_none());
    }

    #[test]
    fn pe_newspaper_hit_rectangles_exclude_right_and_bottom_edges() {
        assert_eq!(pe_newspaper_hit_positions(0, 0, 36), vec![4]);
        assert_eq!(pe_newspaper_hit_positions(0, 639, 75), vec![4]);
        assert_eq!(pe_newspaper_hit_positions(0, 640, 75), Vec::<u16>::new());
        assert_eq!(pe_newspaper_hit_positions(0, 0, 76), vec![7]);
        assert!(pe_newspaper_hit_positions(3, 0, 36).is_empty());
    }

    #[test]
    #[ignore = "requires private native click-handler fixtures"]
    fn pe_newspaper_hits_match_native_x86() {
        let fixture: serde_json::Value = serde_json::from_slice(
            &std::fs::read(std::env::var_os("CITYGAME_PE_NEWSPAPER_HITS").unwrap()).unwrap(),
        )
        .unwrap();
        for case in fixture["cases"].as_array().unwrap() {
            let expected: Vec<_> = case["positions"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_u64().unwrap() as u16)
                .collect();
            let actual = pe_newspaper_hit_positions(
                case["style"].as_u64().unwrap() as u8,
                case["x"].as_i64().unwrap() as i32,
                case["y"].as_i64().unwrap() as i32,
            );
            assert_eq!(actual, expected, "{case}");
        }
        assert_eq!(fixture["cases"].as_array().unwrap().len(), 143);
    }

    #[test]
    #[ignore = "requires private native article-position fixtures"]
    fn pe_article_positions_match_native_x86() {
        let cases: serde_json::Value = serde_json::from_slice(
            &std::fs::read(std::env::var_os("CITYGAME_PE_NEWSPAPER_ARTICLE_SLOTS").unwrap())
                .unwrap(),
        )
        .unwrap();
        for case in cases.as_array().unwrap() {
            let position = case["slot"].as_u64().unwrap() as u16;
            let style = case["style"].as_u64().unwrap() as u8;
            let row = case["row"].as_u64().map(|v| v as usize);
            assert_eq!(pe_article_story_row(position, style), row);
            if row.is_some() {
                let seed = PeNewspaperSeed {
                    days: 1042,
                    startup_tick: -690,
                    paper: 2,
                    slot: position as i16,
                };
                assert_eq!(u64::from(seed.story()), case["seed"].as_u64().unwrap());
            }
        }
        assert_eq!(cases.as_array().unwrap().len(), 33);
    }

    #[test]
    #[ignore = "requires private x86 instruction fixtures"]
    fn pe_newspaper_seeds_match_native_x86() {
        let cases: serde_json::Value = serde_json::from_slice(
            &std::fs::read(std::env::var_os("CITYGAME_PE_NEWSPAPER_SEEDS").unwrap()).unwrap(),
        )
        .unwrap();
        for case in cases.as_array().unwrap() {
            let seed = PeNewspaperSeed {
                days: case["days"].as_i64().unwrap() as i32,
                startup_tick: case["tick"].as_i64().unwrap() as i16,
                paper: case["paper"].as_i64().unwrap() as i32,
                slot: case["slot"].as_i64().unwrap() as i16,
            };
            assert_eq!(u64::from(seed.layout()), case["layout"].as_u64().unwrap());
            assert_eq!(u64::from(seed.story()), case["story"].as_u64().unwrap());
        }
        assert_eq!(cases.as_array().unwrap().len(), 1200);
    }

    #[test]
    #[ignore = "requires private native caller corpus and original DATA"]
    fn pe_seeded_story_matches_native_callers() {
        let dir = std::path::PathBuf::from(std::env::var_os("CITYGAME_WIN95_DATA_DIR").unwrap());
        let archive =
            PeNewspaperData::load(&dir.join("data_usa.dat"), &dir.join("data_usa.idx")).unwrap();
        let fixture: serde_json::Value = serde_json::from_slice(
            &std::fs::read(std::env::var_os("CITYGAME_PE_NEWSPAPER_CALLERS").unwrap()).unwrap(),
        )
        .unwrap();
        let input = &fixture["context"];
        let mut labels: [Option<&[u8]>; 256] = [None; 256];
        for (slot, name) in [0, 251, 252, 253, 254, 255]
            .into_iter()
            .zip(input["labels"].as_array().unwrap())
        {
            labels[slot] = Some(name.as_str().unwrap().as_bytes());
        }
        let context = PeHeadlineContext {
            city_name_oem: input["city"].as_str().unwrap().as_bytes(),
            labels_oem: &labels,
            mutable_state: None,
        };
        let hash = |bytes: &[u8]| {
            bytes.iter().fold(0xcbf29ce484222325u64, |v, b| {
                (v ^ u64::from(*b)).wrapping_mul(0x100000001b3)
            })
        };
        let mut matched = 0;
        for case in fixture["cases"].as_array().unwrap() {
            let id = case["story"].as_u64().unwrap() as usize;
            let seed = PeNewspaperSeed {
                days: 0,
                startup_tick: case["seed"].as_u64().unwrap() as i16,
                paper: 0,
                slot: 0,
            };
            let mut crt = Win95CrtRand::new(99);
            let mut story = PeHeadlineState {
                param: 0,
                choice_b: 255,
                choice_a: 255,
                number: 255,
            };
            let Some((headline, body)) =
                archive.render_seeded_story_ascii(id, seed, &mut crt, &mut story, Some(&context))
            else {
                continue;
            };
            assert_eq!(
                (headline.len() as u64, hash(&headline)),
                (
                    case["output_len"].as_u64().unwrap(),
                    case["output_fnv64"].as_u64().unwrap()
                ),
                "headline {id}"
            );
            let expected = &case["body"];
            assert_eq!(
                (body.len() as u64, hash(&body), u64::from(crt.seed())),
                (
                    expected["output_len"].as_u64().unwrap(),
                    expected["output_fnv64"].as_u64().unwrap(),
                    expected["final_crt"].as_u64().unwrap()
                ),
                "body {id}"
            );
            let row: Vec<_> = expected["row_bytes"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_u64().unwrap() as u8)
                .collect();
            assert_eq!(
                [story.param, story.choice_b, story.choice_a, story.number].as_slice(),
                row,
                "story {id}"
            );
            matched += 1;
        }
        assert_eq!(matched, 621);
    }

    #[test]
    #[ignore = "requires private native lossy-conversion corpus and original DATA"]
    fn pe_lossy_substitutions_match_native_x86() {
        let dir = std::path::PathBuf::from(std::env::var_os("CITYGAME_WIN95_DATA_DIR").unwrap());
        let archive =
            PeNewspaperData::load(&dir.join("data_usa.dat"), &dir.join("data_usa.idx")).unwrap();
        let cases: serde_json::Value = serde_json::from_slice(
            &std::fs::read(std::env::var_os("CITYGAME_PE_LOSSY_CORPUS").unwrap()).unwrap(),
        )
        .unwrap();
        let bytes = |hex: &str| -> Vec<u8> {
            hex.as_bytes()
                .chunks_exact(2)
                .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
                .collect()
        };
        for case in cases.as_array().unwrap() {
            let before = case["before"].as_array().unwrap();
            let mut labels = vec![None; 256];
            for (slot, value) in [0, 251, 252, 253, 254, 255].into_iter().zip(&before[1..]) {
                labels[slot] = Some(bytes(value.as_str().unwrap()));
            }
            let state = std::cell::RefCell::new(PeHeadlineContextState {
                city: bytes(before[0].as_str().unwrap()),
                labels,
            });
            let context = PeHeadlineContext {
                city_name_oem: &[],
                labels_oem: &[],
                mutable_state: Some(&state),
            };
            let mut crt = Win95CrtRand::new(123);
            let mut story = PeHeadlineState {
                param: 0,
                choice_b: 255,
                choice_a: 255,
                number: 255,
            };
            let id = case["id"].as_u64().unwrap() as usize;
            let text = archive
                .expand_headline_with_cache(
                    id,
                    &mut crt,
                    &mut story,
                    Some(&context),
                    &mut PeTextChoiceCache::default(),
                )
                .unwrap();
            assert_eq!(text, bytes(case["output"].as_str().unwrap()), "output {id}");
            assert_eq!(
                u64::from(crt.seed()),
                case["crt"].as_u64().unwrap(),
                "crt {id}"
            );
            let after = case["after"].as_array().unwrap();
            let state = state.into_inner();
            assert_eq!(state.city, bytes(after[0].as_str().unwrap()), "city {id}");
            for (slot, value) in [0, 251, 252, 253, 254, 255].into_iter().zip(&after[1..]) {
                assert_eq!(
                    state.labels[slot].as_ref().unwrap(),
                    &bytes(value.as_str().unwrap()),
                    "label {slot} in {id}"
                );
            }
            let row: Vec<u8> = case["row"]
                .as_array()
                .unwrap()
                .iter()
                .map(|x| x.as_u64().unwrap() as u8)
                .collect();
            assert_eq!(
                [story.param, story.choice_b, story.choice_a, story.number].as_slice(),
                row,
                "row {id}"
            );
        }
        assert_eq!(cases.as_array().unwrap().len(), 261);
    }

    #[test]
    fn pe_ascii_headline_and_body_formatting_follow_original_rules() {
        let mut headline = b"the cat's tale".to_vec();
        format_pe_headline_ascii(&mut headline).unwrap();
        assert_eq!(headline, b"The Cat's Tale");
        let mut apostrophe = b"l'echo".to_vec();
        format_pe_headline_ascii(&mut apostrophe).unwrap();
        assert_eq!(apostrophe, b"L'Echo");
        let mut body = b"hello.  world! a,b. test".to_vec();
        format_pe_body_ascii(&mut body).unwrap();
        assert_eq!(body, b"hello.  World! A,b. Test");
        let mut extended = [0x80];
        format_pe_body_ascii(&mut extended).unwrap();
        assert_eq!(extended, [0x80]);
    }

    #[test]
    #[ignore = "requires private OEM-aware native corpus and original DATA"]
    fn pe_oem_story_callers_match_native_x86() {
        let dir = std::path::PathBuf::from(std::env::var_os("CITYGAME_WIN95_DATA_DIR").unwrap());
        let archive =
            PeNewspaperData::load(&dir.join("data_usa.dat"), &dir.join("data_usa.idx")).unwrap();
        let cases: Vec<serde_json::Value> = serde_json::from_slice(
            &std::fs::read(std::env::var_os("CITYGAME_PE_OEM_CALLERS").unwrap()).unwrap(),
        )
        .unwrap();
        assert_eq!(cases.len(), 414);
        for case in cases {
            let ctx = &case["context"];
            let name = crate::win95_oem::encode_lossless(ctx["city"].as_str().unwrap()).unwrap();
            let mut owned = vec![None; 256];
            for (slot, value) in [0, 251, 252, 253, 254, 255]
                .into_iter()
                .zip(ctx["labels"].as_array().unwrap())
            {
                owned[slot] =
                    Some(crate::win95_oem::encode_lossless(value.as_str().unwrap()).unwrap());
            }
            let labels: Vec<_> = owned.iter().map(|value| value.as_deref()).collect();
            let context = PeHeadlineContext {
                city_name_oem: &name,
                labels_oem: &labels,
                mutable_state: None,
            };
            let mut story = PeHeadlineState {
                param: 0,
                choice_b: 255,
                choice_a: 255,
                number: 255,
            };
            let mut crt = Win95CrtRand::new(0);
            let seed = PeNewspaperSeed {
                days: 0,
                startup_tick: 123,
                paper: 0,
                slot: 0,
            };
            let id = case["story"].as_u64().unwrap() as usize;
            let (headline, body) = archive
                .render_seeded_story_ascii(id, seed, &mut crt, &mut story, Some(&context))
                .unwrap();
            let hex = |bytes: &[u8]| bytes.iter().map(|b| format!("{b:02x}")).collect::<String>();
            assert_eq!(
                hex(&headline),
                case["debug_output_hex"].as_str().unwrap(),
                "headline story {id} context {ctx}"
            );
            assert_eq!(
                hex(&body),
                case["body"]["debug_output_hex"].as_str().unwrap(),
                "body story {id} context {ctx}"
            );
            assert_eq!(
                crt.seed() as u64,
                case["body"]["final_crt"].as_u64().unwrap(),
                "story {id}"
            );
        }
    }

    #[test]
    #[ignore = "requires private native caller corpus and original DATA"]
    fn pe_ascii_newspaper_callers_match_native_x86() {
        let dir = std::path::PathBuf::from(std::env::var_os("CITYGAME_WIN95_DATA_DIR").unwrap());
        let archive =
            PeNewspaperData::load(&dir.join("data_usa.dat"), &dir.join("data_usa.idx")).unwrap();
        let fixture: serde_json::Value = serde_json::from_slice(
            &std::fs::read(std::env::var_os("CITYGAME_PE_NEWSPAPER_CALLERS").unwrap()).unwrap(),
        )
        .unwrap();
        let input = &fixture["context"];
        let mut labels: [Option<&[u8]>; 256] = [None; 256];
        for (slot, name) in [0, 251, 252, 253, 254, 255]
            .into_iter()
            .zip(input["labels"].as_array().unwrap())
        {
            labels[slot] = Some(name.as_str().unwrap().as_bytes());
        }
        let context = PeHeadlineContext {
            city_name_oem: input["city"].as_str().unwrap().as_bytes(),
            labels_oem: &labels,
            mutable_state: None,
        };
        let (mut headlines, mut bodies) = (0, 0);
        for case in fixture["cases"].as_array().unwrap() {
            let id = case["story"].as_u64().unwrap() as usize;
            let mut crt = Win95CrtRand::new(case["seed"].as_u64().unwrap() as u32);
            let mut story = PeHeadlineState {
                param: 0,
                choice_b: 255,
                choice_a: 255,
                number: 255,
            };
            let mut cache = PeTextChoiceCache::default();
            for body in [false, true] {
                let text = if body {
                    archive.render_body_ascii(id, &mut crt, &mut story, Some(&context), &mut cache)
                } else {
                    archive.render_headline_ascii(
                        id,
                        &mut crt,
                        &mut story,
                        Some(&context),
                        &mut cache,
                    )
                };
                let Some(text) = text else {
                    break;
                };
                let expected = if body { &case["body"] } else { case };
                let hash = text.iter().fold(0xcbf29ce484222325u64, |hash, byte| {
                    (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3)
                });
                assert_eq!(
                    text.len() as u64,
                    expected["output_len"].as_u64().unwrap(),
                    "story {id}, body {body}"
                );
                assert_eq!(
                    hash,
                    expected["output_fnv64"].as_u64().unwrap(),
                    "story {id}, body {body}"
                );
                assert_eq!(
                    crt.seed(),
                    expected["final_crt"].as_u64().unwrap() as u32,
                    "story {id}, body {body}"
                );
                let row: Vec<_> = expected["row_bytes"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|v| v.as_u64().unwrap() as u8)
                    .collect();
                let choices: Vec<_> = expected["cache"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|v| v.as_u64().unwrap() as u8)
                    .collect();
                assert_eq!(
                    [story.param, story.choice_b, story.choice_a, story.number].as_slice(),
                    row,
                    "story {id}, body {body}"
                );
                assert_eq!(cache.choices.as_slice(), choices, "story {id}, body {body}");
                if body {
                    bodies += 1;
                } else {
                    headlines += 1;
                }
            }
        }
        println!("matched {headlines} headlines and {bodies} bodies");
        assert!(headlines >= 50 && bodies >= 50);
    }

    fn synthetic_archive() -> (Vec<u8>, Vec<u8>) {
        let mut data = vec![0u8; 11_000];
        data[..2].copy_from_slice(&1u16.to_be_bytes());
        data[500..502].copy_from_slice(&2u16.to_be_bytes());
        data[1008..1012].copy_from_slice(&2u32.to_be_bytes());
        data.extend_from_slice(b"A\0B\0");
        let mut index = Vec::new();
        for (id, offset) in [(1000u32, 0u32), (1001, 500), (1002, 1000), (1003, 11_000)] {
            index.extend_from_slice(&id.to_le_bytes());
            index.extend_from_slice(&offset.to_le_bytes());
        }
        (data, index)
    }

    #[test]
    fn newspaper_archive_reads_big_endian_tables_and_checks_bounds() {
        let (data, index) = synthetic_archive();
        let archive = PeNewspaperData::from_bytes(&data, &index).unwrap();
        assert_eq!(archive.variant(0, 0), Some(&b"A"[..]));
        assert_eq!(archive.variant(0, 1), Some(&b"B"[..]));
        assert_eq!(archive.variant(0, 2), None);
        assert_eq!(archive.variant(250, 0), None);

        let mut unordered = index.clone();
        unordered[12..16].copy_from_slice(&12_000u32.to_le_bytes());
        assert!(PeNewspaperData::from_bytes(&data, &unordered).is_err());
        let mut bad_offset = data.clone();
        bad_offset[1008..1012].copy_from_slice(&99u32.to_be_bytes());
        assert!(PeNewspaperData::from_bytes(&bad_offset, &index).is_err());
        let mut unterminated = data;
        *unterminated.last_mut().unwrap() = b'!';
        assert!(PeNewspaperData::from_bytes(&unterminated, &index).is_err());
    }

    #[test]
    fn pe_headline_selection_advances_shared_crt_only_for_nonempty_stories() {
        let (mut data, index) = synthetic_archive();
        // Story 1 has one variant, while story 2 has none.
        data[502..504].copy_from_slice(&1u16.to_be_bytes());
        let archive = PeNewspaperData::from_bytes(&data, &index).unwrap();
        let mut crt = Win95CrtRand::new(1);
        assert_eq!(
            archive.select_headline_variant(0, &mut crt),
            Some((2, &b"B"[..]))
        );
        assert_eq!(crt.seed(), 2_745_024);
        let mut singleton = Win95CrtRand::new(1);
        assert_eq!(
            archive.select_headline_variant(1, &mut singleton),
            Some((0, &b"A"[..]))
        );
        assert_eq!(singleton.seed(), 2_745_024);
        assert_eq!(archive.select_headline_variant(2, &mut singleton), None);
        assert_eq!(archive.select_headline_variant(250, &mut singleton), None);
        assert_eq!(singleton.seed(), 2_745_024);
    }

    #[test]
    fn pe_headline_expansion_rejects_missing_context_transactionally() {
        let (mut data, index) = synthetic_archive();
        data[11_000] = b'='; // City name requires context not yet supplied.
        let archive = PeNewspaperData::from_bytes(&data, &index).unwrap();
        let mut crt = Win95CrtRand::new(123);
        let mut state = PeHeadlineState {
            param: 0,
            choice_b: 255,
            choice_a: 255,
            number: 255,
        };
        let before = state;
        assert_eq!(archive.expand_headline(1, &mut crt, &mut state), None);
        assert_eq!(crt.seed(), 123);
        assert_eq!(state, before);
        let labels: [Option<&[u8]>; 256] = [None; 256];
        let context = PeHeadlineContext {
            city_name_oem: b"Civic Haven",
            labels_oem: &labels,
            mutable_state: None,
        };
        assert_eq!(
            archive.expand_headline_with_context(1, &mut crt, &mut state, Some(&context)),
            Some(b"Civic Haven".to_vec())
        );
        assert_eq!(crt.seed(), 123);
    }

    #[test]
    fn pe_body_expansion_skips_headline_and_keeps_later_separators() {
        let (data, index) = synthetic_archive();
        let mut archive = PeNewspaperData::from_bytes(&data, &index).unwrap();
        archive.text = b"Heading+Body+More\0Plain\0".to_vec();
        archive.offsets[1] = 0;
        archive.offsets[2] = 18;
        let mut crt = Win95CrtRand::new(123);
        let mut story = PeHeadlineState {
            param: 0,
            choice_b: 255,
            choice_a: 255,
            number: 255,
        };
        assert_eq!(
            archive.expand_headline(1, &mut crt, &mut story),
            Some(b"Heading".to_vec())
        );
        assert_eq!(
            archive.expand_body_with_context(1, &mut crt, &mut story, None),
            Some(b"Body+More".to_vec())
        );
        assert_eq!(
            archive.expand_body_with_context(2, &mut crt, &mut story, None),
            Some(Vec::new())
        );
        assert_eq!(crt.seed(), 123);
    }

    #[test]
    fn pe_headline_and_body_share_cached_choice_but_both_draw() {
        let (data, index) = synthetic_archive();
        let mut archive = PeNewspaperData::from_bytes(&data, &index).unwrap();
        archive.text = b"[\x01+[\x01\0A\0B\0".to_vec();
        archive.offsets[0] = 0;
        archive.offsets[1] = 6;
        archive.offsets[2] = 8;
        archive.starts[1] = 1;
        archive.counts[1] = 2;
        let mut crt = Win95CrtRand::new(2);
        let mut expected_crt = crt;
        expected_crt.random_u15();
        expected_crt.random_u15();
        let mut story = PeHeadlineState {
            param: 0,
            choice_b: 255,
            choice_a: 255,
            number: 255,
        };
        let mut cache = PeTextChoiceCache::default();
        assert_eq!(
            archive.expand_headline_with_cache(0, &mut crt, &mut story, None, &mut cache),
            Some(b"B".to_vec())
        );
        assert_eq!(
            archive.expand_body_with_cache(0, &mut crt, &mut story, None, &mut cache),
            Some(b"B".to_vec())
        );
        assert_eq!(crt, expected_crt);
    }

    #[test]
    #[ignore = "requires private native joined-expansion fixtures and original DATA"]
    fn pe_headline_body_sequence_matches_native_x86() {
        let dir = std::path::PathBuf::from(std::env::var_os("CITYGAME_WIN95_DATA_DIR").unwrap());
        let archive =
            PeNewspaperData::load(&dir.join("data_usa.dat"), &dir.join("data_usa.idx")).unwrap();
        let fixture: serde_json::Value = serde_json::from_slice(
            &std::fs::read(std::env::var_os("CITYGAME_PE_NEWSPAPER_JOINED").unwrap()).unwrap(),
        )
        .unwrap();
        let input = &fixture["context"];
        let mut labels: [Option<&[u8]>; 256] = [None; 256];
        for (slot, name) in [0, 251, 252, 253, 254, 255]
            .into_iter()
            .zip(input["labels"].as_array().unwrap())
        {
            labels[slot] = Some(name.as_str().unwrap().as_bytes());
        }
        let context = PeHeadlineContext {
            city_name_oem: input["city"].as_str().unwrap().as_bytes(),
            labels_oem: &labels,
            mutable_state: None,
        };
        for case in fixture["cases"].as_array().unwrap() {
            let id = case["string_id"].as_u64().unwrap() as usize;
            let mut crt = Win95CrtRand::new(case["seed"].as_u64().unwrap() as u32);
            let mut story = PeHeadlineState {
                param: 0,
                choice_b: 255,
                choice_a: 255,
                number: 255,
            };
            let mut cache = PeTextChoiceCache { choices: [0; 250] };
            for body in [false, true] {
                let output = if body {
                    archive.expand_body_with_cache(
                        id,
                        &mut crt,
                        &mut story,
                        Some(&context),
                        &mut cache,
                    )
                } else {
                    archive.expand_headline_with_cache(
                        id,
                        &mut crt,
                        &mut story,
                        Some(&context),
                        &mut cache,
                    )
                }
                .unwrap_or_else(|| panic!("unsupported string {id}"));
                let expected = if body { &case["body"] } else { case };
                let hash = output.iter().fold(0xcbf29ce484222325u64, |hash, byte| {
                    (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3)
                });
                assert_eq!(
                    output.len() as u64,
                    expected["output_len"].as_u64().unwrap(),
                    "string {id}, body {body}"
                );
                assert_eq!(
                    hash,
                    expected["output_fnv64"].as_u64().unwrap(),
                    "string {id}, body {body}"
                );
                assert_eq!(
                    crt.seed(),
                    expected["final_crt"].as_u64().unwrap() as u32,
                    "string {id}, body {body}"
                );
                let row: Vec<_> = expected["row_bytes"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|v| v.as_u64().unwrap() as u8)
                    .collect();
                let choices: Vec<_> = expected["cache"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|v| v.as_u64().unwrap() as u8)
                    .collect();
                assert_eq!(
                    [story.param, story.choice_b, story.choice_a, story.number].as_slice(),
                    row,
                    "string {id}, body {body}"
                );
                assert_eq!(
                    cache.choices.as_slice(),
                    choices,
                    "string {id}, body {body}"
                );
            }
        }
    }

    #[test]
    #[ignore = "requires private native context fingerprints and original DATA"]
    fn pe_headline_expansion_matches_native_context_corpus() {
        let dir = std::path::PathBuf::from(std::env::var_os("CITYGAME_WIN95_DATA_DIR").unwrap());
        let archive =
            PeNewspaperData::load(&dir.join("data_usa.dat"), &dir.join("data_usa.idx")).unwrap();
        let fixture: serde_json::Value = serde_json::from_slice(
            &std::fs::read(std::env::var_os("CITYGAME_PE_NEWSPAPER_CONTEXT").unwrap()).unwrap(),
        )
        .unwrap();
        let input = &fixture["context"];
        let city = input["city"].as_str().unwrap().as_bytes();
        let names = input["labels"].as_array().unwrap();
        let mut labels: [Option<&[u8]>; 256] = [None; 256];
        for (slot, name) in [0, 251, 252, 253, 254, 255].into_iter().zip(names) {
            labels[slot] = Some(name.as_str().unwrap().as_bytes());
        }
        let context = PeHeadlineContext {
            city_name_oem: city,
            labels_oem: &labels,
            mutable_state: None,
        };
        for case in fixture["cases"].as_array().unwrap() {
            let id = case["string_id"].as_u64().unwrap() as usize;
            let mut crt = Win95CrtRand::new(case["seed"].as_u64().unwrap() as u32);
            let mut story = PeHeadlineState {
                param: 0,
                choice_b: 255,
                choice_a: 255,
                number: 255,
            };
            let output = archive
                .expand_headline_with_context(id, &mut crt, &mut story, Some(&context))
                .unwrap_or_else(|| panic!("unsupported native string {id}"));
            let hash = output.iter().fold(0xcbf29ce484222325u64, |hash, byte| {
                (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3)
            });
            assert_eq!(
                output.len() as u64,
                case["output_len"].as_u64().unwrap(),
                "string {id}"
            );
            assert_eq!(hash, case["output_fnv64"].as_u64().unwrap(), "string {id}");
            assert_eq!(
                crt.seed(),
                case["final_crt"].as_u64().unwrap() as u32,
                "string {id}"
            );
            let fields: Vec<_> = case["row_bytes"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_u64().unwrap() as u8)
                .collect();
            assert_eq!(
                [story.param, story.choice_b, story.choice_a, story.number].as_slice(),
                fields,
                "string {id}"
            );
        }
    }

    #[test]
    #[ignore = "requires private native context fingerprints and original DATA"]
    fn pe_body_expansion_matches_native_context_corpus() {
        let dir = std::path::PathBuf::from(std::env::var_os("CITYGAME_WIN95_DATA_DIR").unwrap());
        let archive =
            PeNewspaperData::load(&dir.join("data_usa.dat"), &dir.join("data_usa.idx")).unwrap();
        let fixture: serde_json::Value = serde_json::from_slice(
            &std::fs::read(std::env::var_os("CITYGAME_PE_NEWSPAPER_BODY").unwrap()).unwrap(),
        )
        .unwrap();
        let input = &fixture["context"];
        let city = input["city"].as_str().unwrap().as_bytes();
        let names = input["labels"].as_array().unwrap();
        let mut labels: [Option<&[u8]>; 256] = [None; 256];
        for (slot, name) in [0, 251, 252, 253, 254, 255].into_iter().zip(names) {
            labels[slot] = Some(name.as_str().unwrap().as_bytes());
        }
        let context = PeHeadlineContext {
            city_name_oem: city,
            labels_oem: &labels,
            mutable_state: None,
        };
        for case in fixture["cases"].as_array().unwrap() {
            let id = case["string_id"].as_u64().unwrap() as usize;
            let mut crt = Win95CrtRand::new(case["seed"].as_u64().unwrap() as u32);
            let mut story = PeHeadlineState {
                param: 0,
                choice_b: 255,
                choice_a: 255,
                number: 255,
            };
            let output = archive
                .expand_body_with_context(id, &mut crt, &mut story, Some(&context))
                .unwrap_or_else(|| panic!("unsupported native body string {id}"));
            let hash = output.iter().fold(0xcbf29ce484222325u64, |hash, byte| {
                (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3)
            });
            assert_eq!(
                output.len() as u64,
                case["output_len"].as_u64().unwrap(),
                "string {id}"
            );
            assert_eq!(hash, case["output_fnv64"].as_u64().unwrap(), "string {id}");
            assert_eq!(
                crt.seed(),
                case["final_crt"].as_u64().unwrap() as u32,
                "string {id}"
            );
            let fields: Vec<_> = case["row_bytes"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_u64().unwrap() as u8)
                .collect();
            assert_eq!(
                [story.param, story.choice_b, story.choice_a, story.number].as_slice(),
                fields,
                "string {id}"
            );
        }
    }

    #[test]
    #[ignore = "requires private native context edge cases and original DATA"]
    fn pe_headline_expansion_matches_native_context_edges() {
        let dir = std::path::PathBuf::from(std::env::var_os("CITYGAME_WIN95_DATA_DIR").unwrap());
        let archive =
            PeNewspaperData::load(&dir.join("data_usa.dat"), &dir.join("data_usa.idx")).unwrap();
        let cases: serde_json::Value = serde_json::from_slice(
            &std::fs::read(std::env::var_os("CITYGAME_PE_NEWSPAPER_CONTEXT_EDGES").unwrap())
                .unwrap(),
        )
        .unwrap();
        for case in cases.as_array().unwrap() {
            let id = case["string_id"].as_u64().unwrap() as usize;
            let source = &case["context"];
            let mut labels: [Option<&[u8]>; 256] = [None; 256];
            for (slot, name) in [0, 251, 252, 253, 254, 255]
                .into_iter()
                .zip(source["labels"].as_array().unwrap())
            {
                labels[slot] = Some(name.as_str().unwrap().as_bytes());
            }
            let context = PeHeadlineContext {
                city_name_oem: source["city"].as_str().unwrap().as_bytes(),
                labels_oem: &labels,
                mutable_state: None,
            };
            let mut crt = Win95CrtRand::new(case["seed"].as_u64().unwrap() as u32);
            let mut story = PeHeadlineState {
                param: case["param"].as_u64().unwrap() as u8,
                choice_b: 255,
                choice_a: 255,
                number: 255,
            };
            let output = archive
                .expand_headline_with_context(id, &mut crt, &mut story, Some(&context))
                .unwrap();
            let hash = output.iter().fold(0xcbf29ce484222325u64, |hash, byte| {
                (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3)
            });
            assert_eq!(
                output.len() as u64,
                case["output_len"].as_u64().unwrap(),
                "string {id}"
            );
            assert_eq!(hash, case["output_fnv64"].as_u64().unwrap(), "string {id}");
            assert_eq!(
                crt.seed(),
                case["final_crt"].as_u64().unwrap() as u32,
                "string {id}"
            );
            let fields: Vec<_> = case["row_bytes"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_u64().unwrap() as u8)
                .collect();
            assert_eq!(
                [story.param, story.choice_b, story.choice_a, story.number].as_slice(),
                fields,
                "string {id}"
            );
        }
    }

    #[test]
    #[ignore = "requires private native expansion fingerprints and original DATA"]
    fn pe_headline_expansion_matches_supported_native_cases() {
        let dir = std::path::PathBuf::from(std::env::var_os("CITYGAME_WIN95_DATA_DIR").unwrap());
        let archive =
            PeNewspaperData::load(&dir.join("data_usa.dat"), &dir.join("data_usa.idx")).unwrap();
        let cases: serde_json::Value = serde_json::from_slice(
            &std::fs::read(std::env::var_os("CITYGAME_PE_NEWSPAPER_EXPANSION").unwrap()).unwrap(),
        )
        .unwrap();
        let mut matched = 0;
        for case in cases.as_array().unwrap() {
            let id = case["string_id"].as_u64().unwrap() as usize;
            let mut crt = Win95CrtRand::new(case["seed"].as_u64().unwrap() as u32);
            let mut story = PeHeadlineState {
                param: 0,
                choice_b: 255,
                choice_a: 255,
                number: 255,
            };
            let Some(output) = archive.expand_headline(id, &mut crt, &mut story) else {
                continue;
            };
            let hash = output.iter().fold(0xcbf29ce484222325u64, |hash, byte| {
                (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3)
            });
            assert_eq!(
                output.len() as u64,
                case["output_len"].as_u64().unwrap(),
                "string {id}"
            );
            assert_eq!(hash, case["output_fnv64"].as_u64().unwrap(), "string {id}");
            assert_eq!(
                crt.seed(),
                case["final_crt"].as_u64().unwrap() as u32,
                "string {id}"
            );
            let fields: Vec<_> = case["row_bytes"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_u64().unwrap() as u8)
                .collect();
            assert_eq!(
                [story.param, story.choice_b, story.choice_a, story.number].as_slice(),
                fields,
                "string {id}"
            );
            matched += 1;
        }
        assert!(matched >= 100, "only {matched} supported cases matched");
    }

    #[test]
    #[ignore = "requires private native headline selection and original DATA fixtures"]
    fn pe_headline_selection_matches_native_x86_corpus() {
        let data_dir =
            std::path::PathBuf::from(std::env::var_os("CITYGAME_WIN95_DATA_DIR").unwrap());
        let archive = PeNewspaperData::load(
            &data_dir.join("data_usa.dat"),
            &data_dir.join("data_usa.idx"),
        )
        .unwrap();
        let path = std::env::var_os("CITYGAME_PE_NEWSPAPER_SELECTION").unwrap();
        let cases: serde_json::Value =
            serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        for case in cases.as_array().unwrap() {
            let story = case["story"].as_u64().unwrap() as usize;
            let mut crt = Win95CrtRand::new(case["seed"].as_u64().unwrap() as u32);
            let (id, bytes) = archive.select_headline_variant(story, &mut crt).unwrap();
            assert_eq!(
                u64::from(id),
                case["string_id"].as_u64().unwrap(),
                "story {story}"
            );
            assert_eq!(
                crt.seed(),
                case["final_crt"].as_u64().unwrap() as u32,
                "story {story}"
            );
            assert_eq!(
                Some(bytes),
                archive.variant(story, usize::from(id - archive.starts[story]))
            );
        }
    }

    #[test]
    #[ignore = "requires user-supplied Win95 DATA_USA archive"]
    fn newspaper_archive_reads_original_data_tables() {
        let root = std::path::PathBuf::from(std::env::var_os("CITYGAME_WIN95_DATA_DIR").unwrap());
        let archive =
            PeNewspaperData::load(&root.join("data_usa.dat"), &root.join("data_usa.idx")).unwrap();
        assert_eq!(archive.starts.len(), 250);
        assert_eq!(
            archive.counts.iter().filter(|&&count| count != 0).count(),
            207
        );
        assert_eq!(archive.offsets.len(), 2_500);
        assert!(archive.variant(0, 0).is_some());
        assert!(archive.variant(0, 1).is_none());
    }

    #[test]
    fn pe_filler_matches_live_newspaper_rectangle_and_crt() {
        let mut crt = Win95CrtRand::new(3_457_711_295);
        let input = NewspaperRect {
            left: 0,
            top: 330,
            right: 213,
            bottom: 400,
        };
        let actual = pe_newspaper_filler_47aca0(input, &mut crt).unwrap();
        let coordinates = [
            (13, 332, 210, 335),
            (3, 337, 210, 340),
            (3, 342, 210, 345),
            (3, 347, 210, 350),
            (3, 352, 210, 355),
            (3, 357, 151, 360),
            (13, 362, 210, 365),
            (3, 367, 210, 370),
            (3, 372, 210, 375),
            (3, 377, 210, 380),
            (3, 382, 210, 385),
            (3, 387, 105, 390),
            (13, 392, 210, 395),
            (3, 397, 210, 400),
        ];
        let expected: Vec<_> = coordinates
            .into_iter()
            .map(|(left, top, right, bottom)| NewspaperRect {
                left,
                top,
                right,
                bottom,
            })
            .collect();
        assert_eq!(actual, expected);
        assert_eq!(crt.seed(), 279_793_450);
    }

    #[test]
    fn pe_filler_handles_empty_and_subrow_heights() {
        let mut crt = Win95CrtRand::new(123);
        let mut rect = NewspaperRect {
            left: 10,
            top: 30,
            right: 80,
            bottom: 30,
        };
        assert!(
            pe_newspaper_filler_47aca0(rect, &mut crt)
                .unwrap()
                .is_empty()
        );
        assert_eq!(crt.seed(), 123);
        rect.bottom = 35;
        assert_eq!(
            pe_newspaper_filler_47aca0(rect, &mut crt).unwrap(),
            vec![NewspaperRect {
                left: 23,
                top: 32,
                right: 77,
                bottom: 35
            }]
        );
        assert_eq!(crt.seed(), 28_854_610);
    }

    #[test]
    #[ignore = "requires private native x86 layout fixtures"]
    fn pe_filler_matches_native_x86_geometry_corpus() {
        let path = std::env::var_os("CITYGAME_PE_NEWSPAPER_GEOMETRY")
            .expect("set CITYGAME_PE_NEWSPAPER_GEOMETRY to the private fixture");
        let cases: serde_json::Value =
            serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        for case in cases.as_array().unwrap() {
            let seed = case["seed"].as_u64().unwrap() as u32;
            let width = case["width"].as_i64().unwrap() as i32;
            let height = case["height"].as_i64().unwrap() as i32;
            let mut crt = Win95CrtRand::new(seed);
            let rect = NewspaperRect {
                left: 10,
                top: 20,
                right: 10 + width,
                bottom: 20 + height,
            };
            let actual = pe_newspaper_filler_47aca0(rect, &mut crt).unwrap();
            let expected: Vec<_> = case["rects"]
                .as_array()
                .unwrap()
                .iter()
                .map(|r| NewspaperRect {
                    left: r[0].as_i64().unwrap() as i32,
                    top: r[1].as_i64().unwrap() as i32,
                    right: r[2].as_i64().unwrap() as i32,
                    bottom: r[3].as_i64().unwrap() as i32,
                })
                .collect();
            assert_eq!(
                actual, expected,
                "seed {seed}, width {width}, height {height}"
            );
            assert_eq!(crt.seed(), case["final"].as_u64().unwrap() as u32);
        }
    }
}
