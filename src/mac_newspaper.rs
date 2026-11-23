//! Mac CODE 7 newspaper token expansion. Resources are supplied by the user.

use anyhow::{Context, Result, ensure};

/// DATA 1000–1003: big-endian group starts, counts, token offsets, and bytes.
pub(crate) struct MacPaperTokens {
    starts: Vec<u16>,
    counts: Vec<u16>,
    offsets: Vec<u32>,
    bytes: Vec<u8>,
}

/// Display strings from the user's Mac 1.2 STRS resource. The resource's
/// offsets are part of that build's table; no original text is embedded here.
#[derive(Debug)]
pub(crate) struct MacPaperDisplayStrings {
    pub(crate) weather: [Vec<u8>; 6],
    pub(crate) survey: [Vec<u8>; 6],
    pub(crate) topics: [Vec<u8>; 7],
    pub(crate) mayor_prefix: Vec<u8>,
    pub(crate) status: Option<MacStatusDisplayStrings>,
}

/// Status strings from the same user-supplied Mac STRS resource.
#[derive(Debug)]
pub(crate) struct MacStatusDisplayStrings {
    pub(crate) stories: [Vec<u8>; 15],
    pub(crate) weather: [Vec<u8>; 3],
    pub(crate) actions: [Vec<u8>; 17],
    pub(crate) paused: Vec<u8>,
}

impl MacStatusDisplayStrings {
    fn from_strs(bytes: &[u8]) -> Result<Self> {
        fn strings<const N: usize>(bytes: &[u8], offsets: [usize; N]) -> Result<[Vec<u8>; N]> {
            let mut result: [Vec<u8>; N] = std::array::from_fn(|_| Vec::new());
            for (output, offset) in result.iter_mut().zip(offsets) {
                let size = usize::from(*bytes.get(offset).context("missing Mac status string")?);
                ensure!(size > 0 && size <= 63, "invalid Mac status string length");
                *output = bytes
                    .get(offset + 1..offset + 1 + size)
                    .context("truncated Mac status string")?
                    .to_vec();
            }
            Ok(result)
        }
        Ok(Self {
            stories: strings(
                bytes,
                [
                    0x32da, 0x32ee, 0x3308, 0x3320, 0x333a, 0x3352, 0x3366, 0x337e, 0x3398, 0x33b2,
                    0x33c8, 0x33e2, 0x33fa, 0x3410, 0x342c,
                ],
            )?,
            weather: strings(bytes, [0x345c, 0x346e, 0x3480])?,
            actions: strings(
                bytes,
                [
                    0x1658, 0x165e, 0x1664, 0x166a, 0x1670, 0x167a, 0x1680, 0x168c, 0x1694, 0x169c,
                    0x16a6, 0x16b0, 0x16b8, 0x16c4, 0x16d0, 0x16dc, 0x16ec,
                ],
            )?,
            paused: strings(bytes, [0x16f6])?.into_iter().next().unwrap(),
        })
    }
}

impl MacPaperDisplayStrings {
    pub(crate) fn from_strs(bytes: &[u8]) -> Result<Self> {
        fn strings<const N: usize>(bytes: &[u8], offsets: [usize; N]) -> Result<[Vec<u8>; N]> {
            let mut result: [Vec<u8>; N] = std::array::from_fn(|_| Vec::new());
            for (output, offset) in result.iter_mut().zip(offsets) {
                let size = usize::from(*bytes.get(offset).context("missing Mac paper string")?);
                ensure!(size > 0 && size <= 63, "invalid Mac paper string length");
                *output = bytes
                    .get(offset + 1..offset + 1 + size)
                    .context("truncated Mac paper string")?
                    .to_vec();
            }
            Ok(result)
        }
        Ok(Self {
            weather: strings(bytes, [0x3218, 0x3228, 0x3238, 0x3248, 0x325c, 0x326e])?,
            survey: strings(bytes, [0x31c6, 0x31d6, 0x31e6, 0x31f2, 0x3200, 0x3208])?,
            topics: strings(
                bytes,
                [0x3298, 0x32a2, 0x32ac, 0x32b4, 0x32bc, 0x32c6, 0x32d0],
            )?,
            mayor_prefix: strings(bytes, [0x30b2])?.into_iter().next().unwrap(),
            status: MacStatusDisplayStrings::from_strs(bytes).ok(),
        })
    }
}

#[derive(Clone, Debug)]
pub(crate) struct MacPaperSlot {
    pub value: u8,
    pub choice5: u8,
    pub choice6: u8,
    pub number: u8,
}

impl Default for MacPaperSlot {
    fn default() -> Self {
        Self {
            value: 0,
            choice5: 255,
            choice6: 255,
            number: 255,
        }
    }
}

pub(crate) struct MacPaperContext<'a> {
    pub slot: &'a mut MacPaperSlot,
    pub group_choices: &'a mut [u8; 256],
    pub auxiliary: &'a [u8],
    /// Pascal string at the start of the Mac label table (the mayor name).
    pub mayor_name: &'a [u8],
    /// The original uses 25-byte Pascal-string records, indexed by value + 251.
    pub labels: &'a [u8],
}

/// CODE 7's paper renderer overwrites QuickDraw's process-global RandSeed
/// before each story. `offset` is selected by the position on the page.
pub(crate) fn story_seed(age: i32, paper_seed: i16, page: i16, offset: i32) -> u32 {
    (age / 25)
        .wrapping_add(i32::from(paper_seed))
        .wrapping_add(i32::from(page).wrapping_mul(500))
        .wrapping_add(offset) as u32
}

/// Order observed in the first Quadra paper page: main, four short stories,
/// lower right, then lower left. Different layouts can omit positions.
pub(crate) const FIRST_PAGE_STORY_OFFSETS: [i32; 7] = [28, 49, 56, 63, 70, 42, 35];

/// QuickDraw rectangle in top, left, bottom, right order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PaperRect(pub i16, pub i16, pub i16, pub i16);

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PaperHeadlineLine {
    pub(crate) text: Vec<u8>,
    pub(crate) size: u16,
    pub(crate) rect: PaperRect,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PaperStoryLayout {
    pub(crate) headline: Vec<PaperHeadlineLine>,
    pub(crate) columns: Vec<PaperRect>,
    pub(crate) filler: Vec<PaperRect>,
}

/// CODE 7's four short-story positions (layout records 7 through 10).
/// Each tuple contains the initial QuickDraw rectangle and point size.
const SHORT_STORY_LAYOUTS: [[(PaperRect, u16); 4]; 3] = [
    [
        (PaperRect(81, 0, 220, 170), 12),
        (PaperRect(81, 341, 220, 512), 12),
        (PaperRect(181, 170, 320, 256), 10),
        (PaperRect(181, 256, 320, 341), 10),
    ],
    [
        (PaperRect(57, 102, 138, 512), 14),
        (PaperRect(239, 102, 320, 307), 12),
        (PaperRect(138, 307, 320, 409), 12),
        (PaperRect(138, 409, 320, 512), 12),
    ],
    [
        (PaperRect(24, 307, 320, 409), 10),
        (PaperRect(194, 102, 320, 204), 10),
        (PaperRect(112, 204, 320, 307), 10),
        (PaperRect(270, 409, 320, 512), 10),
    ],
];

#[derive(Debug)]
pub(crate) struct MacShortStory {
    pub(crate) layout: PaperStoryLayout,
    pub(crate) ending_seed: u32,
}

const WEATHER_BOXES: [(PaperRect, u16); 3] = [
    (PaperRect(220, 341, 320, 512), 12),
    (PaperRect(57, 0, 220, 102), 10),
    (PaperRect(24, 409, 270, 512), 0),
];
const SURVEY_BOXES: [(PaperRect, u16); 3] = [
    (PaperRect(220, 0, 320, 170), 12),
    (PaperRect(220, 0, 320, 102), 10),
    (PaperRect(30, 204, 112, 307), 0),
];

#[derive(Debug)]
pub(crate) struct MacAuxiliaryBox {
    pub(crate) lines: Vec<PaperHeadlineLine>,
    pub(crate) filler: Vec<PaperRect>,
    pub(crate) ending_seed: u32,
}

/// Weather/survey headings try three point sizes but advance their box by
/// five quarters of the original size, even when a smaller size fitted.
fn auxiliary_line(
    text: &[u8],
    rect: &mut PaperRect,
    size: u16,
    font: &crate::mac_font_metrics::MacFontFamily,
    lines: &mut Vec<PaperHeadlineLine>,
) -> Result<bool> {
    for current in [size, size * 4 / 5, size * 2 / 3] {
        if font.width(current, text)? <= (i32::from(rect.3) - i32::from(rect.1)) as u32 {
            lines.push(PaperHeadlineLine {
                text: text.to_vec(),
                size: current,
                rect: *rect,
            });
            rect.0 = i16::try_from(i32::from(rect.0) + i32::from(size) * 5 / 4)?;
            return Ok(true);
        }
    }
    Ok(false)
}

pub(crate) fn layout_weather_box(
    data: &MacPaperTokens,
    font: &crate::mac_font_metrics::MacFontFamily,
    strings: &MacPaperDisplayStrings,
    layout: usize,
    heading: usize,
    age: i32,
    tick: i16,
    page: i16,
    group: usize,
    slot: &mut MacPaperSlot,
    auxiliary: &[u8],
    mayor: &[u8],
    labels: &[u8],
    weather: [u8; 3],
    rng: &mut crate::mac_random::MacToolboxRng,
) -> Result<MacAuxiliaryBox> {
    let &(mut rect, size) = WEATHER_BOXES
        .get(layout)
        .context("invalid Mac paper layout")?;
    let mut lines = Vec::new();
    if size > 0 {
        auxiliary_line(
            strings
                .weather
                .get(heading)
                .context("invalid weather heading")?,
            &mut rect,
            size,
            font,
            &mut lines,
        )?;
        *rng = crate::mac_random::MacToolboxRng::new(story_seed(age, tick, page, 42));
        let mut groups = [255; 256];
        let mut context = MacPaperContext {
            slot,
            group_choices: &mut groups,
            auxiliary,
            mayor_name: mayor,
            labels,
        };
        let headline = data.headline(group, &mut context, &mut || rng.random_u16())?;
        auxiliary_line(&headline, &mut rect, size, font, &mut lines)?;
        let mut numbers = (i16::from(weather[0]) - 100).to_string().into_bytes();
        numbers.extend([0xa1, b'F', b' ']);
        numbers.extend(weather[1].to_string().bytes());
        numbers.extend(b"mph ");
        numbers.extend(weather[2].to_string().bytes());
        numbers.extend(b"mm");
        auxiliary_line(&numbers, &mut rect, size, font, &mut lines)?;
    }
    let filler = placeholder_lines(rect, &mut || rng.random_u16())?;
    Ok(MacAuxiliaryBox {
        lines,
        filler,
        ending_seed: rng.seed(),
    })
}

pub(crate) fn layout_survey_box(
    data: &MacPaperTokens,
    font: &crate::mac_font_metrics::MacFontFamily,
    strings: &MacPaperDisplayStrings,
    layout: usize,
    heading: usize,
    age: i32,
    tick: i16,
    page: i16,
    group: usize,
    slot: &mut MacPaperSlot,
    auxiliary: &[u8],
    mayor: &[u8],
    labels: &[u8],
    approval: u16,
    counts: [u16; 7],
    order: [u16; 7],
    rng: &mut crate::mac_random::MacToolboxRng,
) -> Result<MacAuxiliaryBox> {
    let &(mut rect, size) = SURVEY_BOXES
        .get(layout)
        .context("invalid Mac paper layout")?;
    let mut lines = Vec::new();
    if size > 0 {
        auxiliary_line(
            strings
                .survey
                .get(heading)
                .context("invalid survey heading")?,
            &mut rect,
            size,
            font,
            &mut lines,
        )?;
        *rng = crate::mac_random::MacToolboxRng::new(story_seed(age, tick, page, 35));
        let mut groups = [255; 256];
        let mut context = MacPaperContext {
            slot,
            group_choices: &mut groups,
            auxiliary,
            mayor_name: mayor,
            labels,
        };
        let headline = data.headline(group, &mut context, &mut || rng.random_u16())?;
        auxiliary_line(&headline, &mut rect, size, font, &mut lines)?;
        if heading < 2 && approval > 0 {
            let mut text = strings.mayor_prefix.clone();
            text.extend(approval.to_string().bytes());
            auxiliary_line(&text, &mut rect, size, font, &mut lines)?;
        } else if (2..5).contains(&heading) {
            for i in 0..4 {
                if counts[i] == 0 {
                    break;
                }
                let mut text = strings
                    .topics
                    .get(usize::from(order[i]))
                    .context("invalid survey topic")?
                    .clone();
                text.extend(counts[i].to_string().bytes());
                auxiliary_line(&text, &mut rect, size, font, &mut lines)?;
            }
        }
    }
    let filler = placeholder_lines(rect, &mut || rng.random_u16())?;
    Ok(MacAuxiliaryBox {
        lines,
        filler,
        ending_seed: rng.seed(),
    })
}

const LEAD_LAYOUTS: [(PaperRect, u16, PaperRect, bool); 3] = [
    (
        PaperRect(36, 0, 81, 512),
        36,
        PaperRect(81, 170, 181, 341),
        false,
    ),
    (
        PaperRect(0, 0, 30, 512),
        24,
        PaperRect(138, 102, 238, 307),
        false,
    ),
    (
        PaperRect(30, 0, 320, 102),
        10,
        PaperRect(30, 102, 194, 204),
        true,
    ),
];

pub(crate) fn lead_story_rect(family: usize) -> Option<PaperRect> {
    LEAD_LAYOUTS.get(family).map(|entry| entry.0)
}

pub(crate) fn story_rect(family: usize, position: usize) -> Option<PaperRect> {
    match position {
        4 => lead_story_rect(family),
        5 => SURVEY_BOXES.get(family).map(|entry| entry.0),
        6 => WEATHER_BOXES.get(family).map(|entry| entry.0),
        7..=10 => SHORT_STORY_LAYOUTS
            .get(family)
            .map(|entries| entries[position - 7].0),
        _ => None,
    }
}

/// Story group to PICT ID; zero means choose one of 400–411 at random.
const LEAD_PICTURE_IDS: [u16; 67] = [
    0, 0, 404, 404, 405, 402, 406, 0, 0, 0, 0, 404, 406, 408, 0, 0, 410, 401, 411, 0, 0, 0, 408, 0,
    408, 409, 400, 406, 408, 408, 408, 411, 411, 409, 403, 411, 410, 404, 402, 403, 407, 0, 0, 0,
    0, 411, 403, 410, 408, 403, 403, 403, 411, 411, 403, 403, 403, 403, 411, 410, 401, 411, 404,
    407, 407, 0, 0,
];

#[derive(Debug)]
pub(crate) struct MacLeadLayout {
    pub(crate) headline: Vec<PaperHeadlineLine>,
    pub(crate) story_filler: Vec<PaperRect>,
    pub(crate) picture_id: u16,
    pub(crate) picture_rect: PaperRect,
    pub(crate) picture_filler: Vec<PaperRect>,
    pub(crate) after_lead_seed: u32,
    pub(crate) after_picture_seed: u32,
}

/// CODE 7's lead headline and picture area. The caller supplies the original
/// PICT-resource availability, since missing pictures trigger filler draws.
pub(crate) fn layout_lead_and_picture(
    data: &MacPaperTokens,
    font: &crate::mac_font_metrics::MacFontFamily,
    layout: usize,
    age: i32,
    tick: i16,
    page: i16,
    group: usize,
    slot: &mut MacPaperSlot,
    auxiliary: &[u8],
    mayor: &[u8],
    labels: &[u8],
    picture_exists: &impl Fn(u16) -> bool,
    rng: &mut crate::mac_random::MacToolboxRng,
) -> Result<MacLeadLayout> {
    let &(_, _, picture_rect, always_filler) = LEAD_LAYOUTS
        .get(layout)
        .context("invalid Mac paper layout")?;
    ensure!(group < LEAD_PICTURE_IDS.len(), "invalid Mac story group");
    *rng = crate::mac_random::MacToolboxRng::new(story_seed(age, tick, page, 28));
    let mut groups = [255; 256];
    let mut context = MacPaperContext {
        slot,
        group_choices: &mut groups,
        auxiliary,
        mayor_name: mayor,
        labels,
    };
    let text = data.headline(group, &mut context, &mut || rng.random_u16())?;
    let (headline, story_filler) = layout_lead_text(&text, font, layout, rng)?;
    let after_lead_seed = rng.seed();
    let (picture_id, picture_filler) = layout_picture(
        group,
        picture_rect,
        always_filler,
        story_seed(age, tick, page, 21),
        picture_exists,
        rng,
    )?;
    Ok(MacLeadLayout {
        headline,
        story_filler,
        picture_id,
        picture_rect,
        picture_filler,
        after_lead_seed,
        after_picture_seed: rng.seed(),
    })
}

fn layout_picture(
    group: usize,
    rect: PaperRect,
    always_filler: bool,
    seed: u32,
    picture_exists: &impl Fn(u16) -> bool,
    rng: &mut crate::mac_random::MacToolboxRng,
) -> Result<(u16, Vec<PaperRect>)> {
    let picture = *LEAD_PICTURE_IDS
        .get(group)
        .context("invalid Mac story group")?;
    *rng = crate::mac_random::MacToolboxRng::new(seed);
    let id = if picture == 0 {
        400 + (rng.random_u16() & 0x7fff) % 12
    } else {
        picture
    };
    let filler = if always_filler || !picture_exists(id) {
        placeholder_lines(rect, &mut || rng.random_u16())?
    } else {
        Vec::new()
    };
    Ok((id, filler))
}

fn layout_lead_text(
    text: &[u8],
    font: &crate::mac_font_metrics::MacFontFamily,
    layout: usize,
    rng: &mut crate::mac_random::MacToolboxRng,
) -> Result<(Vec<PaperHeadlineLine>, Vec<PaperRect>)> {
    let &(rect, size, _, _) = LEAD_LAYOUTS
        .get(layout)
        .context("invalid Mac paper layout")?;
    if size <= 15 {
        let result = layout_story(
            text,
            rect,
            size,
            &mut |size, text| font.width(size, text),
            &mut || rng.random_u16(),
        )?;
        Ok((result.headline, result.filler))
    } else {
        let mut box_rect = rect;
        let mut lines = Vec::new();
        for (index, point_size) in [size, size * 2 / 3, size / 2].into_iter().enumerate() {
            if index > 0 {
                let PaperRect(top, left, bottom, right) = box_rect;
                let height = i32::from(bottom) - i32::from(top);
                let target = if index == 1 {
                    i32::from(size) * 10 / 12
                } else {
                    i32::from(size) * 3 / 8
                };
                let inset = (height - target) / 2;
                box_rect = PaperRect(
                    i16::try_from(i32::from(top) + inset)?,
                    left,
                    i16::try_from(i32::from(bottom) - inset)?,
                    right,
                );
            }
            let PaperRect(_, left, _, right) = box_rect;
            if font.width(point_size, &text)? <= (i32::from(right) - i32::from(left)) as u32 {
                lines.push(PaperHeadlineLine {
                    text: text.to_vec(),
                    size: point_size,
                    rect: box_rect,
                });
                break;
            }
        }
        Ok((lines, Vec::new()))
    }
}

/// The four-story loop of CODE 7 `FUN_342d8`. The surrounding paper paint
/// also draws a lead, picture, weather and survey, and is handled separately.
pub(crate) fn layout_short_stories(
    data: &MacPaperTokens,
    font: &crate::mac_font_metrics::MacFontFamily,
    layout: usize,
    age: i32,
    tick: i16,
    page: i16,
    groups: [usize; 4],
    slots: &mut [MacPaperSlot; 4],
    auxiliary: &[u8],
    mayor: &[u8],
    labels: &[u8],
    rng: &mut crate::mac_random::MacToolboxRng,
) -> Result<Vec<MacShortStory>> {
    let positions = SHORT_STORY_LAYOUTS
        .get(layout)
        .context("invalid Mac paper layout")?;
    let mut result = Vec::with_capacity(4);
    for (i, (rect, size)) in positions.iter().copied().enumerate() {
        *rng =
            crate::mac_random::MacToolboxRng::new(story_seed(age, tick, page, (i as i32 + 7) * 7));
        let mut choices = [255; 256];
        let mut context = MacPaperContext {
            slot: &mut slots[i],
            group_choices: &mut choices,
            auxiliary,
            mayor_name: mayor,
            labels,
        };
        let headline = data.headline(groups[i], &mut context, &mut || rng.random_u16())?;
        let layout = layout_story(
            &headline,
            rect,
            size,
            &mut |size, text| font.width(size, text),
            &mut || rng.random_u16(),
        )?;
        result.push(MacShortStory {
            layout,
            ending_seed: rng.seed(),
        });
    }
    Ok(result)
}

/// CODE 7 `FUN_34f5c` / `FUN_34fe2`: fit the headline and draw the
/// remaining article as columns of randomized placeholder lines.
pub(crate) fn layout_story(
    text: &[u8],
    rect: PaperRect,
    size: u16,
    width: &mut impl FnMut(u16, &[u8]) -> Result<u32>,
    rng: &mut impl FnMut() -> u16,
) -> Result<PaperStoryLayout> {
    ensure!(text.len() <= 255, "headline exceeds Pascal string capacity");
    ensure!(size > 0, "invalid newspaper font size");
    let PaperRect(mut top, left, bottom, right) = rect;
    let available = i32::from(right) - i32::from(left);
    ensure!(available >= 0, "invalid story rectangle");
    let mut lines = Vec::new();
    let mut draw = |bytes: &[u8], point_size: u16, top: &mut i16| -> Result<bool> {
        if width(point_size, bytes)? > available as u32 {
            return Ok(false);
        }
        lines.push(PaperHeadlineLine {
            text: bytes.to_vec(),
            size: point_size,
            rect: PaperRect(*top, left, bottom, right),
        });
        *top = i16::try_from(i32::from(*top) + i32::from(point_size) * 5 / 4)?;
        Ok(true)
    };
    if !draw(text, size, &mut top)? {
        // FUN_35134 searches outward from the midpoint, checking the
        // right-hand candidate first. Indexes refer to a Pascal string.
        let middle = text.len() / 2;
        let pascal_byte = |index: usize| -> Option<u8> {
            if index == 0 {
                Some(text.len() as u8)
            } else {
                text.get(index - 1).copied()
            }
        };
        let mut split = middle;
        if pascal_byte(middle) != Some(b' ') {
            for distance in 1..middle {
                if pascal_byte(middle + distance) == Some(b' ') {
                    split = middle + distance;
                    break;
                }
                if pascal_byte(middle - distance) == Some(b' ') {
                    split = middle - distance;
                    break;
                }
            }
        }
        let mut current = size;
        loop {
            if draw(&text[..split], current, &mut top)? {
                let _ = draw(&text[split..], current, &mut top)?;
                break;
            }
            current = current.saturating_mul(6) / 7;
            ensure!(current > 0, "headline cannot fit at a positive size");
        }
    }
    let column_count = available / 130 + 1;
    let mut columns = Vec::with_capacity(column_count as usize);
    let mut filler = Vec::new();
    for i in 0..column_count {
        let column = PaperRect(
            top,
            i16::try_from(i32::from(left) + available * i / column_count)?,
            bottom,
            i16::try_from(i32::from(left) + available * (i + 1) / column_count)?,
        );
        filler.extend(placeholder_lines(column, rng)?);
        columns.push(column);
    }
    Ok(PaperStoryLayout {
        headline: lines,
        columns,
        filler,
    })
}

/// CODE 7 `FUN_34e22`: draw placeholder lines in the remaining article box.
/// Each long group draws Random once, and each shortened line draws once.
pub(crate) fn placeholder_lines(
    rect: PaperRect,
    rng: &mut impl FnMut() -> u16,
) -> Result<Vec<PaperRect>> {
    let PaperRect(top, left, bottom, right) = rect;
    if top >= bottom {
        return Ok(Vec::new());
    }
    let rows = (i32::from(bottom) - i32::from(top)) / 5;
    let short_range = (i32::from(right) - i32::from(left)) * 2 / 3;
    let mut lines = Vec::new();
    let mut row = 0i32;
    loop {
        let make = |row: i32, x: i32, right: i32| -> Result<PaperRect> {
            Ok(PaperRect(
                i16::try_from(i32::from(top) + 5 * row + 2)?,
                i16::try_from(x)?,
                i16::try_from(i32::from(top) + 5 * row + 5)?,
                i16::try_from(right)?,
            ))
        };
        lines.push(make(row, i32::from(left) + 13, i32::from(right) - 3)?);
        let mut additional = i32::from((rng() & 0x7fff) % 6);
        while additional > 0 {
            additional -= 1;
            row += 1;
            if row >= rows {
                break;
            }
            lines.push(make(row, i32::from(left) + 3, i32::from(right) - 3)?);
        }
        row += 1;
        if row < rows {
            ensure!(short_range > 0, "invalid Mac newspaper filler width");
            let shortened = i32::from(rng() & 0x7fff) % short_range;
            lines.push(make(
                row,
                i32::from(left) + 3,
                i32::from(right) - 3 - shortened,
            )?);
        }
        row += 1;
        if row >= rows {
            break;
        }
    }
    Ok(lines)
}

impl MacPaperTokens {
    pub fn load(paths: &[std::path::PathBuf; 4]) -> Result<Self> {
        let bytes = paths
            .iter()
            .map(std::fs::read)
            .collect::<std::io::Result<Vec<_>>>()?;
        Self::from_resources(&bytes[0], &bytes[1], &bytes[2], &bytes[3])
    }

    pub fn from_resources(
        starts: &[u8],
        counts: &[u8],
        offsets: &[u8],
        bytes: &[u8],
    ) -> Result<Self> {
        ensure!(
            starts.len() % 2 == 0 && counts.len() % 2 == 0 && offsets.len() % 4 == 0,
            "unaligned Mac newspaper tables"
        );
        Ok(Self {
            starts: starts
                .chunks_exact(2)
                .map(|x| u16::from_be_bytes([x[0], x[1]]))
                .collect(),
            counts: counts
                .chunks_exact(2)
                .map(|x| u16::from_be_bytes([x[0], x[1]]))
                .collect(),
            offsets: offsets
                .chunks_exact(4)
                .map(|x| u32::from_be_bytes([x[0], x[1], x[2], x[3]]))
                .collect(),
            bytes: bytes.to_vec(),
        })
    }

    /// CODE 7 `FUN_33b90`: choose and expand a headline, then capitalize it.
    /// Its group cache is reset for each headline, but slot caches persist.
    pub fn headline(
        &self,
        group: usize,
        cx: &mut MacPaperContext<'_>,
        rng: &mut impl FnMut() -> u16,
    ) -> Result<Vec<u8>> {
        cx.group_choices[..250].fill(255);
        let count = *self
            .counts
            .get(group)
            .context("Mac newspaper headline group count")?;
        ensure!(
            count > 0 && count <= 32767,
            "invalid Mac newspaper headline group count"
        );
        let start = *self
            .starts
            .get(group)
            .context("Mac newspaper headline group start")? as usize;
        let token = start + usize::from((rng() & 0x7fff) % count);
        let mut result = self.expand(token, 0, cx, rng)?;
        // The original Pascal-string loop stops before the last byte.
        let mut in_word = false;
        let n = result.len().saturating_sub(1);
        for byte in &mut result[..n] {
            if !in_word && byte.is_ascii_lowercase() {
                byte.make_ascii_uppercase();
                in_word = true;
            } else {
                in_word = byte.is_ascii_alphabetic() || *byte == b'\'' || *byte == 0xd5;
            }
        }
        Ok(result)
    }

    /// CODE 7 `FUN_33c6c`: select the same token after reseeding and expand
    /// its article suffix. Headline caches remain live. The caller displays
    /// bytes after the first expanded byte, using the returned length minus one.
    pub fn article(
        &self,
        group: usize,
        cx: &mut MacPaperContext<'_>,
        rng: &mut impl FnMut() -> u16,
    ) -> Result<Vec<u8>> {
        let count = *self
            .counts
            .get(group)
            .context("Mac newspaper article group count")?;
        ensure!(
            count > 0 && count <= 32767,
            "invalid Mac newspaper article group count"
        );
        let start = *self
            .starts
            .get(group)
            .context("Mac newspaper article group start")? as usize;
        let token = start + usize::from((rng() & 0x7fff) % count);
        let mut bytes = self.expand(token, 1, cx, rng)?;
        let mut punctuation = 0i16;
        let end = bytes.len().saturating_sub(1);
        if end > 1 {
            for byte in &mut bytes[1..end] {
                if punctuation >= 2 && byte.is_ascii_lowercase() {
                    byte.make_ascii_uppercase();
                    punctuation = 0;
                } else if matches!(*byte, b' ' | b'.' | b'!' | b'?' | b'"' | 0xd2) {
                    punctuation += 1;
                } else if *byte == b',' {
                    punctuation = -1;
                } else {
                    punctuation = 0;
                }
            }
        }
        Ok(bytes.get(1..).unwrap_or_default().to_vec())
    }

    /// Mode 0 selects the portion before '+', mode 1 the portion after it,
    /// and mode 2 expands the whole token. RNG returns a QuickDraw Random word.
    pub fn expand(
        &self,
        token: usize,
        mode: u8,
        context: &mut MacPaperContext<'_>,
        rng: &mut impl FnMut() -> u16,
    ) -> Result<Vec<u8>> {
        ensure!(mode <= 2, "invalid Mac newspaper expansion mode");
        let mut out = Vec::new();
        self.expand_inner(token, mode, context, rng, &mut out, 0)?;
        Ok(out)
    }

    fn expand_inner(
        &self,
        token: usize,
        mut mode: u8,
        cx: &mut MacPaperContext<'_>,
        rng: &mut impl FnMut() -> u16,
        out: &mut Vec<u8>,
        depth: usize,
    ) -> Result<()> {
        ensure!(
            depth < 64 && out.len() < 65536,
            "Mac newspaper expansion limit exceeded"
        );
        let offset = *self
            .offsets
            .get(token)
            .context("Mac newspaper token index")? as usize;
        let remaining = self
            .bytes
            .get(offset..)
            .context("Mac newspaper token offset")?;
        let text = remaining;
        let mut i = if mode == 1 {
            match text.iter().position(|&b| b == 0 || b == b'+') {
                Some(i) if text[i] == b'+' => {
                    mode = 2;
                    i + 1
                }
                Some(_) => return Ok(()),
                None => anyhow::bail!("unterminated Mac newspaper token"),
            }
        } else {
            0
        };
        loop {
            let c = *text.get(i).context("unterminated Mac newspaper token")?;
            i += 1;
            if c == 0 || (mode == 0 && c == b'+') {
                break;
            }
            match c {
                b'\\' => {
                    let b = *text.get(i).context("missing Mac newspaper escape")?;
                    i += 1;
                    out.push(b);
                }
                b'-' => out.extend_from_slice(b"\r     "),
                b'=' => out.extend_from_slice(cx.auxiliary),
                b'~' => out.extend_from_slice(cx.mayor_name),
                b'>' => {
                    let offset = (usize::from(cx.slot.value) + 251) * 25;
                    let len = *cx.labels.get(offset).context("Mac newspaper label index")? as usize;
                    ensure!(len <= 24, "invalid Mac newspaper label length");
                    out.extend_from_slice(
                        cx.labels
                            .get(offset + 1..offset + 1 + len)
                            .context("Mac newspaper label")?,
                    );
                }
                b'$' => out.extend_from_slice(cx.slot.value.to_string().as_bytes()),
                b'%' | b'<' => {
                    let limit = *text.get(i).context("missing Mac newspaper number limit")?;
                    i += 1;
                    let choice = if limit < 2 {
                        0
                    } else {
                        ((rng() & 0x7fff) % (u16::from(limit) - 1) + 2) as u8
                    };
                    let value = if c == b'<' {
                        if cx.slot.number == 255 {
                            cx.slot.number = choice;
                        }
                        cx.slot.number
                    } else {
                        choice
                    };
                    out.extend_from_slice(value.to_string().as_bytes());
                }
                b'*' | b'&' | b'^' | b'@' | b'[' => {
                    let group = *text.get(i).context("missing Mac newspaper group")? as usize;
                    i += 1;
                    let count = *self
                        .counts
                        .get(group)
                        .context("Mac newspaper group count")?;
                    ensure!(
                        count > 0 && count <= 32767,
                        "invalid Mac newspaper group count"
                    );
                    let start = *self
                        .starts
                        .get(group)
                        .context("Mac newspaper group start")?
                        as usize;
                    let choice = if c == b'&' {
                        if u16::from(cx.slot.value) >= count {
                            cx.slot.value = 0;
                        }
                        cx.slot.value
                    } else {
                        let candidate = ((rng() & 0x7fff) % count) as u8;
                        match c {
                            b'^' => {
                                if u16::from(cx.slot.choice5) >= count {
                                    cx.slot.choice5 = candidate;
                                }
                                cx.slot.choice5
                            }
                            b'@' => {
                                if u16::from(cx.slot.choice6) >= count {
                                    cx.slot.choice6 = candidate;
                                }
                                cx.slot.choice6
                            }
                            b'[' => {
                                if cx.group_choices[group] == 255 {
                                    cx.group_choices[group] = candidate;
                                }
                                cx.group_choices[group]
                            }
                            _ => candidate,
                        }
                    };
                    self.expand_inner(start + usize::from(choice), mode, cx, rng, out, depth + 1)?;
                }
                _ => {
                    let mapped = match c {
                        1..=31 => usize::from(c),
                        0x80..=0xd1 => usize::from(c) - 0x60,
                        0xd6..=0xff => usize::from(c) - 0x64,
                        _ => 0,
                    };
                    if mapped == 0 {
                        out.push(c);
                    } else {
                        let start = *self
                            .starts
                            .get(67)
                            .context("Mac newspaper fallback group")?
                            as usize;
                        self.expand_inner(start + mapped - 1, mode, cx, rng, out, depth + 1)?;
                    }
                }
            }
            ensure!(out.len() < 65536, "Mac newspaper output limit exceeded");
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "requires private Mac STRS resource and Quadra RAM snapshot"]
    fn status_strings_match_live_pointer_tables() -> Result<()> {
        let strings = MacStatusDisplayStrings::from_strs(&std::fs::read(std::env::var(
            "CITYGAME_MAC_STRS",
        )?)?)?;
        let ram = std::fs::read(std::env::var("CITYGAME_MAC_STATUS_RAM")?)?;
        let read32 = |offset: usize| {
            u32::from_be_bytes(ram[offset..offset + 4].try_into().unwrap()) as usize
        };
        let a5 = read32(0x904) & 0xffffff;
        for (offset, values) in [
            (0x35ca, strings.stories.as_slice()),
            (0x358a, strings.weather.as_slice()),
            (0x4d7c, strings.actions.as_slice()),
        ] {
            for (index, expected) in values.iter().enumerate() {
                let ptr = read32(a5 - offset + index * 4) & 0xffffff;
                assert_eq!(
                    &ram[ptr + 1..ptr + 1 + usize::from(ram[ptr])],
                    expected,
                    "table {offset:x} index {index}"
                );
            }
        }
        let paused = read32(a5 - 0x4d2c) & 0xffffff;
        assert_eq!(
            &ram[paused + 1..paused + 1 + usize::from(ram[paused])],
            strings.paused
        );
        Ok(())
    }

    #[test]
    #[ignore = "requires private Quadra captures, original STRS and System font"]
    fn weather_and_survey_match_live_quadra_paints() -> Result<()> {
        let root = std::path::PathBuf::from(std::env::var("CITYGAME_MAC_PAPER_WIDTHS")?);
        let capture = root.join("../mac-paper-live-layout");
        let font = crate::mac_font_metrics::MacFontFamily::from_suitcase(
            &std::fs::read(root.join("newyork.bin"))?,
            2,
        )?;
        let strings = MacPaperDisplayStrings::from_strs(&std::fs::read(std::env::var(
            "CITYGAME_MAC_STRS",
        )?)?)?;
        let log = std::fs::read_to_string(capture.join("paper-layout.csv"))?;
        let rows: Vec<Vec<&str>> = log.lines().map(|line| line.split(',').collect()).collect();
        for (page, family, snapshot, paint) in [
            (0i16, 1usize, "after-paper.ram", 0usize),
            (1, 0, "chronicle.ram", 2),
        ] {
            let ram = std::fs::read(capture.join(snapshot))?;
            let u32_at = |p: usize| u32::from_be_bytes(ram[p..p + 4].try_into().unwrap()) as usize;
            let u16_at = |p: usize| u16::from_be_bytes(ram[p..p + 2].try_into().unwrap());
            let a5 = u32_at(0x904);
            for (offset, source) in [
                (0x375a, strings.weather.as_slice()),
                (0x3772, strings.survey.as_slice()),
                (0x3700, strings.topics.as_slice()),
            ] {
                for (i, expected) in source.iter().enumerate() {
                    let p = u32_at(a5 - offset + i * 4) & 0xffffff;
                    assert_eq!(
                        &ram[p + 1..p + 1 + usize::from(ram[p])],
                        expected,
                        "page {page} string {offset:x}/{i}"
                    );
                }
            }
            let table = |off: usize, len: usize| {
                let p = u32_at(a5 - off) & 0xffffff;
                &ram[p..p + len]
            };
            let data = MacPaperTokens::from_resources(
                table(0x36e4, 500),
                table(0x36e0, 500),
                table(0x36dc, 10000),
                table(0x36d8, 149362),
            )?;
            let labels = u32_at(a5 - 0x2bc) & 0xffffff;
            let aux = a5 - 0x2a14;
            let records = u32_at(a5 - 0xfa6) & 0xffffff;
            let paper = u32_at(a5 - 0xfa2) & 0xffffff;
            let slot = |i: usize| {
                let b = &ram[records + i * 8..records + i * 8 + 8];
                (
                    usize::from(u16::from_be_bytes([b[0], b[1]])),
                    MacPaperSlot {
                        value: b[4],
                        choice5: b[5],
                        choice6: b[6],
                        number: b[7],
                    },
                )
            };
            let (weather_group, mut weather_slot) = slot(7);
            let (survey_group, mut survey_slot) = slot(8);
            let age = i32::from_be_bytes(ram[a5 - 0x105c..a5 - 0x1058].try_into()?);
            let tick = i16::from_be_bytes(ram[a5 - 0x36cc..a5 - 0x36ca].try_into()?);
            let aux_text = &ram[aux + 1..aux + 1 + usize::from(ram[aux])];
            let mayor = &ram[labels + 1..labels + 1 + usize::from(ram[labels])];
            let all_labels = &ram[labels..labels + 25 * 507];
            let mut rng = crate::mac_random::MacToolboxRng::new(1);
            let weather = layout_weather_box(
                &data,
                &font,
                &strings,
                family,
                usize::from(ram[paper + page as usize * 6 + 4]),
                age,
                tick,
                page,
                weather_group,
                &mut weather_slot,
                aux_text,
                mayor,
                all_labels,
                ram[a5 - 0xf7c..a5 - 0xf79].try_into()?,
                &mut rng,
            )?;
            let counts = std::array::from_fn(|i| u16_at(a5 - 0x355a + i * 2));
            let order = std::array::from_fn(|i| u16_at(a5 - 0x3568 + i * 2));
            let survey = layout_survey_box(
                &data,
                &font,
                &strings,
                family,
                usize::from(ram[paper + page as usize * 6 + 3]),
                age,
                tick,
                page,
                survey_group,
                &mut survey_slot,
                aux_text,
                mayor,
                all_labels,
                u16_at(a5 - 0x357e),
                counts,
                order,
                &mut rng,
            )?;
            let begin = rows
                .iter()
                .enumerate()
                .filter(|(_, r)| r[0] == "begin")
                .nth(paint)
                .unwrap()
                .0;
            let end = (begin + 1..rows.len())
                .find(|&i| rows[i][0] == "end")
                .unwrap();
            let anchor = |off| {
                (begin..end)
                    .find(|&i| {
                        rows[i][0] == "random"
                            && rows[i][1] == "00822"
                            && u32::from_str_radix(rows[i][2], 16).ok()
                                == Some(story_seed(age, tick, page, off))
                    })
                    .unwrap()
            };
            let w = anchor(42);
            let s = anchor(35);
            let last_random = rows[w..s].iter().rev().find(|r| r[0] == "random").unwrap();
            let mut native =
                crate::mac_random::MacToolboxRng::new(u32::from_str_radix(last_random[2], 16)?);
            native.random_u16();
            assert_eq!(
                weather.ending_seed,
                native.seed(),
                "page {page} weather RNG"
            );
            assert_eq!(
                survey.ending_seed,
                u32::from_str_radix(rows[end][1], 16)?,
                "page {page} final RNG"
            );
            let widths = |range: std::ops::Range<usize>| {
                rows[range]
                    .iter()
                    .filter(|r| r[0] == "width")
                    .map(|r| r[2].parse::<u32>().unwrap())
                    .collect::<Vec<_>>()
            };
            let weather_widths = widths(w.saturating_sub(2)..s);
            let survey_widths = widths(s.saturating_sub(2)..end);
            for line in &weather.lines {
                assert!(
                    weather_widths.contains(&font.width(line.size, &line.text)?),
                    "page {page} weather line"
                );
            }
            for line in &survey.lines {
                assert!(
                    survey_widths.contains(&font.width(line.size, &line.text)?),
                    "page {page} survey line"
                );
            }
        }
        Ok(())
    }

    #[test]
    fn picture_selection_reseeds_and_only_draws_when_needed() -> Result<()> {
        let rect = PaperRect(0, 0, 20, 100);
        let mut rng = crate::mac_random::MacToolboxRng::new(99);
        let (id, filler) = layout_picture(50, rect, false, 12345, &|_| true, &mut rng)?;
        assert_eq!((id, filler.len(), rng.seed()), (403, 0, 12345));
        let mut expected = crate::mac_random::MacToolboxRng::new(12345);
        let random_id = 400 + (expected.random_u16() & 0x7fff) % 12;
        let (id, filler) = layout_picture(0, rect, false, 12345, &|_| true, &mut rng)?;
        assert_eq!(
            (id, filler.len(), rng.seed()),
            (random_id, 0, expected.seed())
        );
        let wanted = placeholder_lines(rect, &mut || expected.random_u16())?;
        let (_, filler) = layout_picture(0, rect, false, 12345, &|_| false, &mut rng)?;
        assert_eq!(filler, wanted);
        assert_eq!(rng.seed(), expected.seed());
        Ok(())
    }

    #[test]
    #[ignore = "requires private CODE 7 replay and original System font"]
    fn lead_fitting_matches_original_worker() -> Result<()> {
        let root = std::path::PathBuf::from(std::env::var("CITYGAME_MAC_PAPER_WIDTHS")?);
        let font = crate::mac_font_metrics::MacFontFamily::from_suitcase(
            &std::fs::read(root.join("newyork.bin"))?,
            2,
        )?;
        let cases: Vec<serde_json::Value> =
            serde_json::from_slice(&std::fs::read(root.join("lead-native.json"))?)?;
        assert_eq!(cases.len(), 18);
        let decode = |hex: &str| -> Result<Vec<u8>> {
            hex.as_bytes()
                .chunks_exact(2)
                .map(|p| Ok(u8::from_str_radix(std::str::from_utf8(p)?, 16)?))
                .collect()
        };
        let rect = |v: &serde_json::Value| {
            let n = |i: usize| v[i].as_i64().unwrap() as i16;
            PaperRect(n(0), n(1), n(2), n(3))
        };
        for case in cases {
            let mut rng = crate::mac_random::MacToolboxRng::new(12345);
            let (headline, filler) = layout_lead_text(
                &decode(case["text"].as_str().unwrap())?,
                &font,
                case["family"].as_u64().unwrap() as usize,
                &mut rng,
            )?;
            let expected = case["headline"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| {
                    Ok(PaperHeadlineLine {
                        text: decode(v[0].as_str().unwrap())?,
                        size: v[1].as_u64().unwrap() as u16,
                        rect: rect(&v[2]),
                    })
                })
                .collect::<Result<Vec<_>>>()?;
            assert_eq!(headline, expected, "{case}");
            assert_eq!(
                filler,
                case["filler"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(rect)
                    .collect::<Vec<_>>(),
                "{case}"
            );
            assert_eq!(rng.seed(), case["seed"].as_u64().unwrap() as u32, "{case}");
        }
        Ok(())
    }

    #[test]
    #[ignore = "requires private CODE 7 replay and original Macintosh resources"]
    fn picture_selection_and_filler_match_original_worker() -> Result<()> {
        let root = std::path::PathBuf::from(std::env::var("CITYGAME_MAC_PAPER_WIDTHS")?);
        let ram = std::fs::read(root.join("../mac-paper-live-layout/after-paper.ram"))?;
        let u32_at = |p: usize| u32::from_be_bytes(ram[p..p + 4].try_into().unwrap()) as usize;
        let a5 = u32_at(0x904);
        let table = |off: usize, len: usize| {
            let p = u32_at(a5 - off) & 0xffffff;
            &ram[p..p + len]
        };
        let data = MacPaperTokens::from_resources(
            table(0x36e4, 500),
            table(0x36e0, 500),
            table(0x36dc, 10000),
            table(0x36d8, 149362),
        )?;
        let font = crate::mac_font_metrics::MacFontFamily::from_suitcase(
            &std::fs::read(root.join("newyork.bin"))?,
            2,
        )?;
        let labels = u32_at(a5 - 0x2bc) & 0xffffff;
        let auxiliary = a5 - 0x2a14;
        let cases: Vec<serde_json::Value> =
            serde_json::from_slice(&std::fs::read(root.join("picture-native.json"))?)?;
        assert_eq!(cases.len(), 12);
        for case in cases {
            let layout = case["family"].as_u64().unwrap() as usize;
            let group = case["group"].as_u64().unwrap() as usize;
            let available = case["available"].as_bool().unwrap();
            let mut slot = MacPaperSlot::default();
            let mut rng = crate::mac_random::MacToolboxRng::new(42);
            let lead = layout_lead_and_picture(
                &data,
                &font,
                layout,
                28257,
                4689,
                0,
                group,
                &mut slot,
                &ram[auxiliary + 1..auxiliary + 1 + usize::from(ram[auxiliary])],
                &ram[labels + 1..labels + 1 + usize::from(ram[labels])],
                &ram[labels..labels + 25 * 507],
                &|_| available,
                &mut rng,
            )?;
            let rect = |v: &serde_json::Value| {
                let n = |i: usize| v[i].as_i64().unwrap() as i16;
                PaperRect(n(0), n(1), n(2), n(3))
            };
            assert_eq!(
                lead.picture_id,
                case["picture"].as_u64().unwrap() as u16,
                "{case}"
            );
            assert_eq!(
                lead.picture_filler,
                case["filler"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(rect)
                    .collect::<Vec<_>>(),
                "{case}"
            );
            assert_eq!(
                lead.after_picture_seed,
                case["seed"].as_u64().unwrap() as u32,
                "{case}"
            );
        }
        Ok(())
    }

    #[test]
    #[ignore = "requires private Quadra paper captures and original System font"]
    fn lead_headline_matches_live_quadra_paints() -> Result<()> {
        let root = std::path::PathBuf::from(std::env::var("CITYGAME_MAC_PAPER_WIDTHS")?);
        let capture = root.join("../mac-paper-live-layout");
        let font = crate::mac_font_metrics::MacFontFamily::from_suitcase(
            &std::fs::read(root.join("newyork.bin"))?,
            2,
        )?;
        let log = std::fs::read_to_string(capture.join("paper-layout.csv"))?;
        let rows: Vec<Vec<&str>> = log.lines().map(|line| line.split(',').collect()).collect();
        for (page, family, snapshot, paint) in [
            (0i16, 1usize, "after-paper.ram", 0usize),
            (1, 0, "chronicle.ram", 2),
        ] {
            let ram = std::fs::read(capture.join(snapshot))?;
            let u32_at = |p: usize| u32::from_be_bytes(ram[p..p + 4].try_into().unwrap()) as usize;
            let a5 = u32_at(0x904);
            let table = |off: usize, len: usize| {
                let p = u32_at(a5 - off) & 0xffffff;
                &ram[p..p + len]
            };
            let data = MacPaperTokens::from_resources(
                table(0x36e4, 500),
                table(0x36e0, 500),
                table(0x36dc, 10000),
                table(0x36d8, 149362),
            )?;
            let records = u32_at(a5 - 0xfa6) & 0xffffff;
            let b = &ram[records..records + 8];
            let group = usize::from(u16::from_be_bytes([b[0], b[1]]));
            let mut slot = MacPaperSlot {
                value: b[4],
                choice5: b[5],
                choice6: b[6],
                number: b[7],
            };
            let labels = u32_at(a5 - 0x2bc) & 0xffffff;
            let auxiliary = a5 - 0x2a14;
            let age = i32::from_be_bytes(ram[a5 - 0x105c..a5 - 0x1058].try_into()?);
            let tick = i16::from_be_bytes(ram[a5 - 0x36cc..a5 - 0x36ca].try_into()?);
            let mut rng = crate::mac_random::MacToolboxRng::new(1);
            let lead = layout_lead_and_picture(
                &data,
                &font,
                family,
                age,
                tick,
                page,
                group,
                &mut slot,
                &ram[auxiliary + 1..auxiliary + 1 + usize::from(ram[auxiliary])],
                &ram[labels + 1..labels + 1 + usize::from(ram[labels])],
                &ram[labels..labels + 25 * 507],
                &|id| id == 403,
                &mut rng,
            )?;
            let begin = rows
                .iter()
                .enumerate()
                .filter(|(_, row)| row[0] == "begin")
                .nth(paint)
                .unwrap()
                .0;
            let anchor = (begin..rows.len())
                .find(|&n| rows[n][0] == "random" && rows[n][1] == "00822")
                .unwrap();
            let measurement = (anchor + 1..rows.len())
                .find(|&n| rows[n][0] == "width")
                .unwrap();
            assert_eq!(lead.headline.len(), 1);
            assert_eq!(
                font.width(lead.headline[0].size, &lead.headline[0].text)?,
                rows[measurement][2].parse::<u32>()?
            );
            assert_eq!(
                lead.after_lead_seed,
                u32::from_str_radix(rows[measurement][1], 16)?
            );
            assert!(lead.story_filler.is_empty());
            assert_eq!(lead.picture_id, 403);
            assert!(lead.picture_filler.is_empty());
            assert_eq!(lead.after_picture_seed, story_seed(age, tick, page, 21));
        }
        Ok(())
    }

    #[test]
    #[ignore = "requires private Quadra paper captures and original System font"]
    fn four_short_stories_match_live_quadra_paints() -> Result<()> {
        let root = std::path::PathBuf::from(std::env::var("CITYGAME_MAC_PAPER_WIDTHS")?);
        let capture = root.join("../mac-paper-live-layout");
        let font = crate::mac_font_metrics::MacFontFamily::from_suitcase(
            &std::fs::read(root.join("newyork.bin"))?,
            2,
        )?;
        let log = std::fs::read_to_string(capture.join("paper-layout.csv"))?;
        let rows: Vec<Vec<&str>> = log.lines().map(|line| line.split(',').collect()).collect();
        for (page, family, snapshot, paint) in [
            (0i16, 1usize, "after-paper.ram", 0usize),
            (1, 0, "chronicle.ram", 2),
        ] {
            let ram = std::fs::read(capture.join(snapshot))?;
            let u32_at = |p: usize| u32::from_be_bytes(ram[p..p + 4].try_into().unwrap()) as usize;
            let a5 = u32_at(0x904);
            let table = |off: usize, len: usize| {
                let p = u32_at(a5 - off) & 0xffffff;
                &ram[p..p + len]
            };
            let data = MacPaperTokens::from_resources(
                table(0x36e4, 500),
                table(0x36e0, 500),
                table(0x36dc, 10000),
                table(0x36d8, 149362),
            )?;
            let records = u32_at(a5 - 0xfa6) & 0xffffff;
            let mut groups = [0usize; 4];
            let mut slots: [MacPaperSlot; 4] = std::array::from_fn(|i| {
                let b = &ram[records + (i + 1) * 8..records + (i + 2) * 8];
                groups[i] = usize::from(u16::from_be_bytes([b[0], b[1]]));
                MacPaperSlot {
                    value: b[4],
                    choice5: b[5],
                    choice6: b[6],
                    number: b[7],
                }
            });
            let labels = u32_at(a5 - 0x2bc) & 0xffffff;
            let auxiliary = a5 - 0x2a14;
            let age = i32::from_be_bytes(ram[a5 - 0x105c..a5 - 0x1058].try_into()?);
            let tick = i16::from_be_bytes(ram[a5 - 0x36cc..a5 - 0x36ca].try_into()?);
            let mut rng = crate::mac_random::MacToolboxRng::new(1);
            let result = layout_short_stories(
                &data,
                &font,
                family,
                age,
                tick,
                page,
                groups,
                &mut slots,
                &ram[auxiliary + 1..auxiliary + 1 + usize::from(ram[auxiliary])],
                &ram[labels + 1..labels + 1 + usize::from(ram[labels])],
                &ram[labels..labels + 25 * 507],
                &mut rng,
            )?;
            let begin = rows
                .iter()
                .enumerate()
                .filter(|(_, row)| row[0] == "begin")
                .nth(paint)
                .unwrap()
                .0;
            let end = rows
                .iter()
                .enumerate()
                .skip(begin + 1)
                .find(|(_, row)| row[0] == "end")
                .unwrap()
                .0;
            let mut final_seed = 0;
            for i in 0..4 {
                let seed = story_seed(age, tick, page, (i as i32 + 7) * 7);
                let start = (begin..end)
                    .find(|&n| {
                        rows[n][0] == "random"
                            && rows[n][1] == "00822"
                            && u32::from_str_radix(rows[n][2], 16).ok() == Some(seed)
                    })
                    .context("missing native short story")?;
                let filler = (start..end)
                    .find(|&n| rows[n][0] == "filler")
                    .context("missing native filler")?;
                let finish = (filler..end)
                    .find(|&n| match rows[n][0] {
                        "filler" => false,
                        "random" => rows[n][1] != "01af2" && rows[n][1] != "01b6c",
                        _ => true,
                    })
                    .unwrap_or(end);
                let expected: Vec<_> = rows[filler..finish]
                    .iter()
                    .filter(|r| r[0] == "filler")
                    .map(|r| {
                        PaperRect(
                            r[2].parse().unwrap(),
                            r[3].parse().unwrap(),
                            r[4].parse().unwrap(),
                            r[5].parse().unwrap(),
                        )
                    })
                    .collect();
                assert_eq!(result[i].layout.columns, expected, "page {page} story {i}");
                let measured: Vec<u32> = rows[start..filler]
                    .iter()
                    .filter(|r| r[0] == "width")
                    .map(|r| r[2].parse().unwrap())
                    .collect();
                for line in &result[i].layout.headline {
                    assert!(
                        measured.contains(&font.width(line.size, &line.text)?),
                        "page {page} story {i} headline width"
                    );
                }
                let last = rows[filler..finish]
                    .iter()
                    .rev()
                    .find(|r| r[0] == "random")
                    .unwrap();
                let mut native =
                    crate::mac_random::MacToolboxRng::new(u32::from_str_radix(last[2], 16)?);
                native.random_u16();
                final_seed = native.seed();
                assert_eq!(
                    result[i].ending_seed, final_seed,
                    "page {page} story {i} RNG"
                );
            }
            assert_eq!(rng.seed(), final_seed, "page {page} after four stories");
        }
        Ok(())
    }

    #[test]
    fn story_split_can_leave_only_the_first_headline_line() {
        let mut draws = 0;
        let layout = layout_story(
            b"AB CD EF",
            PaperRect(0, 0, 20, 40),
            10,
            &mut |size, text| Ok(u32::from(size) * text.len() as u32),
            &mut || {
                draws += 1;
                0
            },
        )
        .unwrap();
        assert_eq!(
            layout.headline,
            vec![PaperHeadlineLine {
                text: b"AB ".to_vec(),
                size: 10,
                rect: PaperRect(0, 0, 20, 40),
            }]
        );
        assert_eq!(layout.columns, vec![PaperRect(12, 0, 20, 40)]);
        assert_eq!(draws, 1);
    }

    #[test]
    #[ignore = "requires private Quadra CODE 7 replay and original System font"]
    fn story_layout_matches_quadras_code7_worker() -> Result<()> {
        let root = std::path::PathBuf::from(std::env::var("CITYGAME_MAC_PAPER_WIDTHS")?);
        let font = crate::mac_font_metrics::MacFontFamily::from_suitcase(
            &std::fs::read(root.join("newyork.bin"))?,
            2,
        )?;
        let cases: Vec<serde_json::Value> =
            serde_json::from_slice(&std::fs::read(root.join("story-layout-native.json"))?)?;
        fn decode(hex: &str) -> Result<Vec<u8>> {
            hex.as_bytes()
                .chunks_exact(2)
                .map(|part| Ok(u8::from_str_radix(std::str::from_utf8(part)?, 16)?))
                .collect()
        }
        fn rect(value: &serde_json::Value) -> PaperRect {
            let a = value.as_array().unwrap();
            let n = |i: usize| i16::try_from(a[i].as_i64().unwrap()).unwrap();
            PaperRect(n(0), n(1), n(2), n(3))
        }
        assert_eq!(cases.len(), 36);
        for (index, case) in cases.iter().enumerate() {
            let input = case["input"].as_array().unwrap();
            let text = decode(input[0].as_str().unwrap())?;
            let size = input[1].as_u64().unwrap() as u16;
            let mut rng = crate::mac_random::MacToolboxRng::new(12345);
            let mut calls = 0;
            let layout = layout_story(
                &text,
                rect(&input[2]),
                size,
                &mut |size, text| font.width(size, text),
                &mut || {
                    calls += 1;
                    rng.random_u16()
                },
            )?;
            let expected = case["headline"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| {
                    Ok(PaperHeadlineLine {
                        text: decode(v[0].as_str().unwrap())?,
                        size: v[1].as_u64().unwrap() as u16,
                        rect: rect(&v[2]),
                    })
                })
                .collect::<Result<Vec<_>>>()?;
            assert_eq!(layout.headline, expected, "headline {index}");
            assert_eq!(
                layout.columns,
                case["columns"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(rect)
                    .collect::<Vec<_>>(),
                "columns {index}"
            );
            assert_eq!(
                layout.filler,
                case["filler"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(rect)
                    .collect::<Vec<_>>(),
                "filler {index}"
            );
            assert_eq!(
                rng.seed(),
                case["seed"].as_u64().unwrap() as u32,
                "seed {index}"
            );
            assert_eq!(
                calls,
                case["calls"].as_u64().unwrap(),
                "random count {index}"
            );
        }
        Ok(())
    }

    #[test]
    #[ignore = "requires the private Quadra Fire newspaper RAM capture and native replay output"]
    fn quadra_fire_headline_matches_native_replay() -> Result<()> {
        let capture = std::path::PathBuf::from(
            std::env::var_os("CITYGAME_MAC_PAPER_CAPTURE").context("CITYGAME_MAC_PAPER_CAPTURE")?,
        );
        let ram = std::fs::read(capture.join("../manual-fire-protected-full/newspaper.ram"))?;
        let u32_at = |p: usize| u32::from_be_bytes(ram[p..p + 4].try_into().unwrap()) as usize;
        let a5 = u32_at(0x904);
        let table = |displacement: usize, size: usize| {
            let address = u32_at(a5 - displacement) & 0xffffff;
            &ram[address..address + size]
        };
        let data = MacPaperTokens::from_resources(
            table(0x36e4, 500),
            table(0x36e0, 500),
            table(0x36dc, 10000),
            table(0x36d8, 149362),
        )?;
        let slots = u32_at(a5 - 0xfa6) & 0xffffff;
        let auxiliary = a5 - 0x2a14;
        let labels = u32_at(a5 - 0x2bc) & 0xffffff;
        for (index, offset, expected_calls, expected_seed) in [
            (0, 28, 2, 0x4d8278b9),
            (1, 49, 2, 0x2f154e81),
            (2, 56, 1, 0x0339453e),
            (3, 63, 1, 0x033b10cf),
            (4, 70, 4, 0x2db9980a),
            (7, 42, 1, 0x0335ae1c),
            (8, 35, 1, 0x0333e28b),
        ] {
            let source = &ram[slots + index * 8..slots + index * 8 + 8];
            let group = usize::from(u16::from_be_bytes([source[0], source[1]]));
            let mut slot = MacPaperSlot {
                value: source[4],
                choice5: source[5],
                choice6: source[6],
                number: source[7],
            };
            let mut choices = [255; 256];
            let mut cx = MacPaperContext {
                slot: &mut slot,
                group_choices: &mut choices,
                auxiliary: &ram[auxiliary + 1..auxiliary + 1 + usize::from(ram[auxiliary])],
                mayor_name: &ram[labels + 1..labels + 1 + usize::from(ram[labels])],
                labels: &ram[labels..labels + 25 * 507],
            };
            let mut rng = crate::mac_random::MacToolboxRng::new(story_seed(28257, 2032, 0, offset));
            let mut calls = 0;
            let text = data.headline(group, &mut cx, &mut || {
                calls += 1;
                rng.random_u16()
            })?;
            assert_eq!(
                text,
                std::fs::read(capture.join(format!("native-headline-{index}.bin")))?,
                "slot {index}"
            );
            assert_eq!(calls, expected_calls, "slot {index}");
            assert_eq!(rng.seed(), expected_seed, "slot {index}");
            let (body_calls, body_seed) = match index {
                0 => (17, 0x7bb76cf6),
                1 => (15, 0x5528e1dc),
                2 => (16, 0x49a0e126),
                3 => (13, 0x6e7eb023),
                4 => (21, 0x284716a5),
                7 => (12, 0x1ae31e54),
                8 => (30, 0x5a69092c),
                _ => unreachable!(),
            };
            let mut rng = crate::mac_random::MacToolboxRng::new(story_seed(28257, 2032, 0, offset));
            let mut calls = 0;
            let body = data.article(group, &mut cx, &mut || {
                calls += 1;
                rng.random_u16()
            })?;
            assert_eq!(
                body,
                std::fs::read(capture.join(format!("native-body-{index}.bin")))?,
                "body slot {index}"
            );
            assert_eq!(calls, body_calls, "body slot {index}");
            assert_eq!(rng.seed(), body_seed, "body slot {index}");
        }
        Ok(())
    }

    #[test]
    fn quadra_placeholder_line_replay() -> Result<()> {
        let mut rng = crate::mac_random::MacToolboxRng::new(0xc7d);
        let mut calls = 0;
        let lines = placeholder_lines(PaperRect(10, 20, 60, 100), &mut || {
            calls += 1;
            rng.random_u16()
        })?;
        assert_eq!(
            lines,
            vec![
                PaperRect(12, 33, 15, 97),
                PaperRect(17, 23, 20, 97),
                PaperRect(22, 23, 25, 97),
                PaperRect(27, 23, 30, 97),
                PaperRect(32, 23, 35, 46),
                PaperRect(37, 33, 40, 97),
                PaperRect(42, 23, 45, 97),
                PaperRect(47, 23, 50, 97),
                PaperRect(52, 23, 55, 97),
                PaperRect(57, 23, 60, 97)
            ]
        );
        assert_eq!(calls, 3);
        assert_eq!(rng.seed(), 0x55c99b64);
        Ok(())
    }

    #[test]
    fn quadra_story_seeds_and_native_headline_wrapper() -> Result<()> {
        let seeds = FIRST_PAGE_STORY_OFFSETS.map(|offset| story_seed(28257, 2032, 0, offset));
        assert_eq!(seeds, [0xc76, 0xc8b, 0xc92, 0xc99, 0xca0, 0xc84, 0xc7d]);
        let mut starts = vec![0; 16];
        let mut counts = vec![0; 16];
        starts[14..16].copy_from_slice(&1u16.to_be_bytes());
        counts[14..16].copy_from_slice(&1u16.to_be_bytes());
        let mut offsets = vec![0; 8];
        offsets[4..8].copy_from_slice(&0u32.to_be_bytes());
        let data =
            MacPaperTokens::from_resources(&starts, &counts, &offsets, b"hello, world's end! z\0")?;
        let mut slot = MacPaperSlot::default();
        let mut choices = [0; 256];
        let mut calls = 0;
        let mut cx = MacPaperContext {
            slot: &mut slot,
            group_choices: &mut choices,
            auxiliary: b"",
            mayor_name: b"",
            labels: b"",
        };
        let text = data.headline(7, &mut cx, &mut || {
            calls += 1;
            17
        })?;
        assert_eq!(text, b"Hello, World's End! z");
        assert_eq!(calls, 1);
        assert_eq!(cx.group_choices[249], 255);
        assert_eq!(cx.group_choices[250], 0);
        Ok(())
    }

    #[test]
    fn native_replay_escape_cached_choice_and_fallback() -> Result<()> {
        let mut starts = vec![0; 140];
        let mut counts = vec![0; 140];
        starts[14..16].copy_from_slice(&3u16.to_be_bytes());
        counts[14..16].copy_from_slice(&1u16.to_be_bytes());
        starts[134..136].copy_from_slice(&1u16.to_be_bytes());
        let mut offsets = vec![0; 33 * 4];
        let mut pool = vec![0];
        for (id, bytes) in [
            (1, &b"\\Q@\x07@\x07\x80"[..]),
            (3, &b"quartz"[..]),
            (32, &b"frag"[..]),
        ] {
            offsets[id * 4..id * 4 + 4].copy_from_slice(&(pool.len() as u32).to_be_bytes());
            pool.extend_from_slice(bytes);
            pool.push(0);
        }
        let data = MacPaperTokens::from_resources(&starts, &counts, &offsets, &pool)?;
        let mut slot = MacPaperSlot::default();
        let mut choices = [255; 256];
        let mut calls = 0;
        let mut cx = MacPaperContext {
            slot: &mut slot,
            group_choices: &mut choices,
            auxiliary: b"",
            mayor_name: b"",
            labels: b"",
        };
        let result = data.expand(1, 0, &mut cx, &mut || {
            calls += 1;
            17
        })?;
        assert_eq!(result, b"Qquartzquartzfrag");
        assert_eq!(calls, 2);
        Ok(())
    }

    #[test]
    fn segments_and_cached_numbers_consume_native_draws() -> Result<()> {
        let data =
            MacPaperTokens::from_resources(&[], &[], &0u32.to_be_bytes(), b"before+<\x05/<\x05\0")?;
        let mut slot = MacPaperSlot::default();
        let mut choices = [255; 256];
        let mut calls = 0;
        let mut cx = MacPaperContext {
            slot: &mut slot,
            group_choices: &mut choices,
            auxiliary: b"",
            mayor_name: b"",
            labels: b"",
        };
        assert_eq!(data.expand(0, 0, &mut cx, &mut || 0)?, b"before");
        assert_eq!(
            data.expand(0, 1, &mut cx, &mut || {
                calls += 1;
                calls
            })?,
            b"3/3"
        );
        assert_eq!(calls, 2);
        Ok(())
    }
}
