//! Budget presentation only: signed cash values and one shared geometry for drawing/hit tests.
use super::*;
use crate::newspaper_font as typeface;

const WIDTH: f32 = 1000.0;
const HEIGHT: f32 = 810.0;
const INK: [f32; 4] = [0.011, 0.018, 0.024, 1.0];
const MUTED: [f32; 4] = [0.15, 0.18, 0.20, 1.0];
const NAVY: [f32; 4] = [0.016, 0.042, 0.065, 1.0];
const WHITE: [f32; 4] = [0.98, 0.98, 0.96, 1.0];
const LINE: [f32; 4] = [0.65, 0.69, 0.69, 1.0];
const INCOME: [f32; 4] = [0.015, 0.20, 0.11, 1.0];
const EXPENSE: [f32; 4] = [0.38, 0.035, 0.025, 1.0];

const LABELS: [&str; BUDGET_TOTAL_COUNT] = [
    "Residential taxes",
    "Commercial taxes",
    "Industrial taxes",
    "City ordinances",
    "Bond interest",
    "Police department",
    "Fire department",
    "Health & welfare",
    "Schools",
    "Colleges",
    "Road maintenance",
    "Highways",
    "Bridges",
    "Rail network",
    "Subway",
    "Tunnels",
];

#[derive(Clone, Debug)]
pub(super) struct Entry {
    label: &'static str,
    setting: Option<u32>,
    year_to_date: Option<i64>,
    mac_ytd_pre_month_division: Option<i64>,
    mac_estimate_pre_month_division: Option<i64>,
    annual: i64,
}

pub(super) struct Content<'a> {
    pub city: &'a str,
    pub date: &'a str,
    pub funds: i64,
    pub bonds: u32,
    pub rating: &'a str,
    pub auto_budget: bool,
    pub can_issue: bool,
    pub can_repay: bool,
    pub entries: &'a [Entry; BUDGET_TOTAL_COUNT],
    pub font_atlas: Option<usize>,
    pub dark_appearance: bool,
    pub mac_layout: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct MacCashflowSummary {
    year_to_date: i64,
    estimated: i64,
    end_of_year_funds: i64,
}

pub(super) fn entries_for_city(
    city: &City,
    fidelity: EngineFidelity,
) -> [Entry; BUDGET_TOTAL_COUNT] {
    entries_for_city_for_month(city, fidelity, 1, false)
}

pub(super) fn entries_for_city_for_month(
    city: &City,
    fidelity: EngineFidelity,
    month: u8,
    annual_budget_review: bool,
) -> [Entry; BUDGET_TOTAL_COUNT] {
    let tax_bases = budget_tax_bases_for_city_with_fidelity(
        city,
        live_population_stats_for_city(city),
        fidelity,
    );
    let services = budget_service_rows_for_city(city);
    let month_index = i32::from(month.saturating_sub(1).min(11));
    std::array::from_fn(|index| {
        let offset = budget_row_misc_offset_for_city(city, index, 0);
        let values = offset
            .map(|offset| city.budget_misc_u32_values(offset, BUDGET_SERVICE_MISC_STRIDE))
            .unwrap_or_default();
        let field = |n: usize| values.get(n).copied().unwrap_or(0);
        let setting = if !fidelity.uses_win95() && index < BUDGET_TAX_COUNT {
            Some(mac_budget_tax_rate_raw_for_city(city, index).min(MAX_TAX_RATE))
        } else if !fidelity.uses_win95() && index >= BUDGET_SERVICE_ROW_START {
            Some(
                mac_budget_service_funding_raw_for_city(city, index - BUDGET_SERVICE_ROW_START)
                    .min(100),
            )
        } else if index < BUDGET_TAX_COUNT {
            Some(tax_rate_for_city(city, index))
        } else if index >= BUDGET_SERVICE_ROW_START {
            Some(services[index - BUDGET_SERVICE_ROW_START].funding)
        } else {
            None
        };
        let (annual, year_to_date) = if !fidelity.uses_win95() {
            // Mac stores signed raw numerators and settles each row with its own
            // CODE 4 divisor. Keep its accumulator interpretation separate from
            // the PE's FUN_004734A0 row walk, even where divisor values agree.
            let divisor = i64::from(PE_BUDGET_ROW_DIVISORS[index]);
            let annual_raw = (field(0) as i32).wrapping_mul(field(1) as i32);
            (
                i64::from(annual_raw) / divisor,
                Some(i64::from(field(2) as i32) / (divisor * 12)),
            )
        } else if index < BUDGET_TAX_COUNT {
            let current = if budget_row_is_unset(city, index) {
                tax_bases[index]
            } else {
                field(0)
            };
            let divisor = i64::from(PE_BUDGET_ROW_DIVISORS[index]) * 12;
            (
                i64::from(budget_row_annual_amount(index, current, setting.unwrap())),
                Some(i64::from(field(2) as i32) / divisor),
            )
        } else if index == BUDGET_ORDINANCE_ROW {
            let divisor = i64::from(PE_BUDGET_ROW_DIVISORS[index]) * 12;
            (
                ordinance_year_end_delta_for_city(city, live_population_stats_for_city(city)),
                Some(i64::from(field(2) as i32) / divisor),
            )
        } else if index == BUDGET_BOND_ROW {
            let divisor = i64::from(PE_BUDGET_ROW_DIVISORS[index]) * 12;
            let settled = i64::from(field(2) as i32) / divisor;
            (-annual_bond_interest_for_city(city), Some(-settled))
        } else {
            let row = services[index - BUDGET_SERVICE_ROW_START];
            let divisor = i64::from(PE_BUDGET_ROW_DIVISORS[index]) * 12;
            let settled = i64::from(field(2) as i32) / divisor;
            (
                -i64::from(budget_row_annual_amount(index, row.current, row.funding)),
                Some(-settled),
            )
        };
        Entry {
            label: LABELS[index],
            setting,
            year_to_date,
            mac_ytd_pre_month_division: (!fidelity.uses_win95())
                .then(|| i64::from(field(2) as i32) / i64::from(PE_BUDGET_ROW_DIVISORS[index])),
            mac_estimate_pre_month_division: (!fidelity.uses_win95()).then(|| {
                let divisor = i64::from(PE_BUDGET_ROW_DIVISORS[index]);
                let current_raw = (field(0) as i32).wrapping_mul(field(1) as i32);
                let projected_raw = if annual_budget_review {
                    current_raw.wrapping_mul(12)
                } else {
                    (field(2) as i32).wrapping_add(current_raw.wrapping_mul(11 - month_index))
                };
                i64::from(projected_raw) / divisor
            }),
            annual,
        }
    })
}

pub(super) fn page_rect(window: (u32, u32)) -> FloatRect {
    let scale = ((window.0 as f32 - 32.0).max(1.0) / WIDTH)
        .min((window.1 as f32 - 32.0).max(1.0) / HEIGHT)
        .min(1.6);
    FloatRect {
        x: (window.0 as f32 - WIDTH * scale) * 0.5,
        y: (window.1 as f32 - HEIGHT * scale) * 0.5,
        w: WIDTH * scale,
        h: HEIGHT * scale,
    }
}

pub(super) fn page_rect_for_engine(window: (u32, u32), fidelity: EngineFidelity) -> FloatRect {
    if fidelity.uses_win95() {
        return page_rect(window);
    }
    let scale = (window.0 as f32 / 640.0)
        .min(window.1 as f32 / 480.0)
        .max(0.25);
    let (w, h) = (328.0 * scale, 257.0 * scale);
    FloatRect {
        x: (window.0 as f32 - w) * 0.5,
        y: (window.1 as f32 - h) * 0.5,
        w,
        h,
    }
}

fn mac_area(page: FloatRect, x: f32, y: f32, w: f32, h: f32) -> FloatRect {
    let scale = page.w / 328.0;
    FloatRect {
        x: page.x + x * scale,
        y: page.y + y * scale,
        w: w * scale,
        h: h * scale,
    }
}

pub(super) fn mac_close_rect(page: FloatRect) -> FloatRect {
    mac_area(page, 284.0, 4.0, 36.0, 16.0)
}

pub(super) fn mac_help_rect(page: FloatRect) -> FloatRect {
    mac_area(page, 284.0, 22.0, 36.0, 16.0)
}

pub(super) fn mac_tax_adjust_rect(page: FloatRect, increase: bool) -> FloatRect {
    mac_area(page, 166.0, if increase { 43.0 } else { 51.0 }, 15.0, 8.0)
}

fn mac_service_row_y(service_index: usize) -> Option<f32> {
    match service_index {
        0 => Some(117.0),
        1 => Some(135.0),
        2 => Some(153.0),
        3..=4 => Some(172.0),
        5..=10 => Some(191.0),
        _ => None,
    }
}

pub(super) fn mac_service_adjust_rect(
    page: FloatRect,
    service_index: usize,
    increase: bool,
) -> Option<FloatRect> {
    let y = mac_service_row_y(service_index)?;
    Some(mac_area(
        page,
        166.0,
        y + if increase { -12.0 } else { -4.0 },
        15.0,
        8.0,
    ))
}

fn area(page: FloatRect, x: f32, y: f32, w: f32, h: f32) -> FloatRect {
    let s = page.w / WIDTH;
    FloatRect {
        x: page.x + x * s,
        y: page.y + y * s,
        w: w * s,
        h: h * s,
    }
}

pub(super) fn close_rect(page: FloatRect) -> FloatRect {
    area(page, 888.0, 24.0, 84.0, 34.0)
}
fn row_y(index: usize) -> f32 {
    231.0 + index as f32 * 25.5
}
fn adjust_rect(page: FloatRect, row: usize, increase: bool) -> FloatRect {
    area(
        page,
        if increase { 540.0 } else { 426.0 },
        row_y(row) - 1.0,
        24.0,
        22.0,
    )
}
pub(super) fn tax_adjust_rect(page: FloatRect, index: usize, increase: bool) -> FloatRect {
    adjust_rect(page, index, increase)
}
pub(super) fn service_adjust_rect(page: FloatRect, index: usize, increase: bool) -> FloatRect {
    adjust_rect(page, BUDGET_SERVICE_ROW_START + index, increase)
}
pub(super) fn auto_rect(page: FloatRect) -> FloatRect {
    area(page, 28.0, 716.0, 210.0, 36.0)
}
pub(super) fn bond_rect(page: FloatRect, issue: bool) -> FloatRect {
    area(page, if issue { 586.0 } else { 784.0 }, 716.0, 188.0, 36.0)
}

fn money(value: i64) -> String {
    let digits = value.unsigned_abs().to_string();
    let mut grouped = String::new();
    for (i, ch) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            grouped.push(',');
        }
        grouped.push(ch);
    }
    format!("{}${grouped}", if value < 0 { "-" } else { "" })
}
fn cash_color(value: i64) -> [f32; 4] {
    if value < 0 {
        EXPENSE
    } else if value > 0 {
        INCOME
    } else {
        MUTED
    }
}

fn mac_cashflow_summary(
    entries: &[Entry; BUDGET_TOTAL_COUNT],
    current_funds: i64,
) -> MacCashflowSummary {
    let total = |value: fn(&Entry) -> i64| {
        entries
            .iter()
            .fold(0_i32, |sum, entry| sum.wrapping_add(value(entry) as i32)) as i64
    };
    // CODE 4 export 225 first divides each saved +0x68 YTD or +0x6c projected
    // row accumulator by that row's signed divisor, sums those dwords, then
    // divides the combined total by 12. Dividing each row by divisor * 12
    // first loses remainder contributions that the native panel retains.
    let mac_ytd_accumulator = entries.iter().try_fold(0_i32, |sum, entry| {
        entry
            .mac_ytd_pre_month_division
            .map(|value| sum.wrapping_add(value as i32))
    });
    let year_to_date = mac_ytd_accumulator
        .map(|accumulator| i64::from(accumulator / 12))
        .unwrap_or_else(|| total(|entry| entry.year_to_date.unwrap_or(0)));
    let mac_estimate_accumulator = entries.iter().try_fold(0_i32, |sum, entry| {
        entry
            .mac_estimate_pre_month_division
            .map(|value| sum.wrapping_add(value as i32))
    });
    let estimated = mac_estimate_accumulator
        .map(|accumulator| i64::from(accumulator / 12))
        .unwrap_or_else(|| total(|entry| entry.annual));
    let end_of_year_funds = (current_funds as i32).wrapping_add(year_to_date as i32) as i64;
    MacCashflowSummary {
        year_to_date,
        estimated,
        end_of_year_funds,
    }
}

struct Canvas<'a> {
    page: FloatRect,
    solids: &'a mut Vec<UiSolid>,
    images: &'a mut Vec<UiImage>,
    atlas: Option<usize>,
    depth: f32,
}
impl Canvas<'_> {
    fn rect(&mut self, x: f32, y: f32, w: f32, h: f32, color: [f32; 4]) {
        push_ui_rect(
            self.solids,
            area(self.page, x, y, w, h),
            color,
            self.depth + 0.1,
        );
    }
    fn text(
        &mut self,
        value: &str,
        x: f32,
        y: f32,
        width: f32,
        em: f32,
        color: [f32; 4],
        right: bool,
    ) {
        let size = (em / 7.2).min(width / typeface::width(value, 1.0).max(1.0));
        let x = x + if right {
            width - typeface::width(value, size)
        } else {
            0.0
        };
        let p = area(self.page, x, y, 0.0, 0.0);
        let scale = self.page.w / WIDTH;
        if let Some(atlas) = self.atlas {
            typeface::draw(
                self.images,
                atlas,
                value,
                p.x,
                p.y,
                size * scale,
                color,
                self.depth + 0.3,
            );
        } else {
            let pixel_scale =
                (size * scale).min(width * scale / ui::pixel_text_width(value, 1.0).max(1.0));
            ui::push_pixel_text(
                self.solids,
                value,
                p.x,
                p.y,
                pixel_scale,
                color,
                self.depth + 0.3,
            );
        }
    }
    fn button(&mut self, rect: FloatRect, label: &str, active: bool, enabled: bool) {
        let scale = self.page.w / WIDTH;
        let x = (rect.x - self.page.x) / scale;
        let y = (rect.y - self.page.y) / scale;
        let w = rect.w / scale;
        let h = rect.h / scale;
        let fill = if active {
            NAVY
        } else if enabled {
            WHITE
        } else {
            [0.80, 0.82, 0.81, 1.0]
        };
        self.rect(x, y, w, h, fill);
        self.rect(x, y, w, 0.8, LINE);
        self.rect(x, y + h - 0.8, w, 0.8, LINE);
        self.rect(x, y, 0.8, h, LINE);
        self.rect(x + w - 0.8, y, 0.8, h, LINE);
        let color = if active {
            WHITE
        } else if enabled {
            INK
        } else {
            MUTED
        };
        let size = (17.0_f32 / 7.2).min((w - 16.0) / typeface::width(label, 1.0).max(1.0));
        let label_x = x + (w - typeface::width(label, size)) * 0.5;
        self.text(label, label_x, y + 9.0, w - 8.0, size * 7.2, color, false);
    }
    fn adjust(&mut self, row: usize, increase: bool, enabled: bool) {
        let rect = adjust_rect(self.page, row, increase);
        self.button(rect, "", false, enabled);
        let x = if increase { 540.0 } else { 426.0 };
        let y = row_y(row) - 1.0;
        let color = if enabled { INK } else { LINE };
        // Geometry avoids missing '+' glyphs in the legacy menu font fallback.
        self.rect(x + 7.0, y + 10.0, 10.0, 1.5, color);
        if increase {
            self.rect(x + 11.0, y + 6.0, 1.5, 10.0, color);
        }
    }
}

pub(super) fn draw(
    page: FloatRect,
    content: &Content<'_>,
    solids: &mut Vec<UiSolid>,
    images: &mut Vec<UiImage>,
    depth: f32,
) {
    if content.mac_layout {
        draw_mac(page, content, solids, images, depth);
        return;
    }
    push_ui_rect(
        solids,
        FloatRect {
            x: page.x + 7.0,
            y: page.y + 9.0,
            ..page
        },
        [0.0, 0.0, 0.0, 0.35],
        depth,
    );
    push_ui_rect(solids, page, [0.90, 0.91, 0.89, 1.0], depth + 0.05);
    let mut c = Canvas {
        page,
        solids,
        images,
        atlas: content.font_atlas,
        depth,
    };
    let (header, header_text) = if content.dark_appearance {
        ([0.055, 0.105, 0.135, 1.0], [0.80, 0.95, 0.93, 1.0])
    } else {
        (NAVY, WHITE)
    };
    c.rect(0.0, 0.0, WIDTH, 88.0, header);
    c.text("City Budget", 28.0, 18.0, 750.0, 34.0, header_text, false);
    c.text(
        &format!("{}  /  {}", content.city, content.date),
        28.0,
        60.0,
        800.0,
        15.0,
        header_text,
        false,
    );
    c.button(close_rect(page), "Done", false, true);
    let income: i64 = content.entries.iter().map(|row| row.annual.max(0)).sum();
    let expense: i64 = content.entries.iter().map(|row| row.annual.min(0)).sum();
    let annual_net = income + expense;
    for (x, label, value, color) in [
        (28.0, "CURRENT FUNDS", money(content.funds), INK),
        (
            352.0,
            "PLANNED ANNUAL BALANCE",
            money(annual_net),
            cash_color(annual_net),
        ),
        (
            676.0,
            "OUTSTANDING BONDS",
            format!("{}  /  {}", content.bonds, content.rating),
            INK,
        ),
    ] {
        c.rect(x, 106.0, 296.0, 66.0, WHITE);
        c.text(label, x + 14.0, 116.0, 268.0, 13.0, MUTED, false);
        c.text(&value, x + 14.0, 137.0, 268.0, 28.0, color, false);
    }
    c.rect(28.0, 190.0, 944.0, 31.0, [0.78, 0.82, 0.82, 1.0]);
    c.text("CATEGORY", 42.0, 199.0, 350.0, 14.0, INK, false);
    c.text("RATE / FUNDING", 421.0, 199.0, 145.0, 14.0, INK, false);
    c.text("YEAR TO DATE", 584.0, 199.0, 151.0, 14.0, INK, true);
    c.text("ANNUAL BUDGET", 766.0, 199.0, 192.0, 14.0, INK, true);
    for (index, row) in content.entries.iter().enumerate() {
        let y = row_y(index);
        c.rect(
            28.0,
            y - 5.0,
            944.0,
            25.5,
            if index % 2 == 0 {
                WHITE
            } else {
                [0.86, 0.88, 0.86, 1.0]
            },
        );
        if index == BUDGET_SERVICE_ROW_START || index == BUDGET_TAX_COUNT {
            c.rect(28.0, y - 5.0, 944.0, 1.0, LINE);
        }
        c.text(row.label, 42.0, y, 360.0, 19.0, INK, false);
        if let Some(setting) = row.setting {
            c.adjust(index, false, setting > 0);
            c.adjust(
                index,
                true,
                setting
                    < if index < BUDGET_TAX_COUNT {
                        MAX_TAX_RATE
                    } else {
                        100
                    },
            );
            let value = format!("{setting}%");
            let width = typeface::width(&value, 19.0 / 7.2);
            c.text(
                &value,
                452.0 + (86.0 - width) * 0.5,
                y,
                86.0,
                19.0,
                INK,
                false,
            );
        }
        let ytd = row
            .year_to_date
            .map(money)
            .unwrap_or_else(|| "—".to_owned());
        c.text(
            &ytd,
            584.0,
            y,
            151.0,
            19.0,
            row.year_to_date.map(cash_color).unwrap_or(MUTED),
            true,
        );
        c.text(
            &money(row.annual),
            766.0,
            y,
            192.0,
            19.0,
            cash_color(row.annual),
            true,
        );
    }
    c.rect(28.0, 650.0, 944.0, 1.0, LINE);
    c.text("Annual income", 28.0, 668.0, 150.0, 16.0, MUTED, false);
    c.text(&money(income), 180.0, 665.0, 206.0, 23.0, INCOME, false);
    c.text("Annual spending", 454.0, 668.0, 170.0, 16.0, MUTED, false);
    c.text(&money(expense), 632.0, 665.0, 340.0, 23.0, EXPENSE, true);
    c.button(
        auto_rect(page),
        if content.auto_budget {
            "Auto Budget: On"
        } else {
            "Auto Budget: Off"
        },
        content.auto_budget,
        true,
    );
    c.text(
        "Bonds are $10,000 each",
        258.0,
        727.0,
        310.0,
        15.0,
        MUTED,
        false,
    );
    c.button(
        bond_rect(page, true),
        "Issue bond",
        false,
        content.can_issue,
    );
    c.button(
        bond_rect(page, false),
        "Repay bond",
        false,
        content.can_repay,
    );
    c.text(
        "Annual budget uses current rates and funding. A dash means no recorded YTD amount.",
        28.0,
        776.0,
        944.0,
        13.0,
        MUTED,
        false,
    );
}

fn draw_mac(
    page: FloatRect,
    content: &Content<'_>,
    solids: &mut Vec<UiSolid>,
    images: &mut Vec<UiImage>,
    depth: f32,
) {
    let factor = WIDTH / 328.0;
    let mut c = Canvas {
        page,
        solids,
        images,
        atlas: content.font_atlas,
        depth,
    };
    let rect = |x: f32, y: f32, w: f32, h: f32| mac_area(page, x, y, w, h);
    push_ui_rect(
        c.solids,
        FloatRect {
            x: page.x + 4.0,
            y: page.y + 5.0,
            ..page
        },
        [0.0, 0.0, 0.0, 0.32],
        depth,
    );
    push_ui_rect(c.solids, page, [0.82, 0.83, 0.81, 1.0], depth + 0.05);
    push_ui_rect(
        c.solids,
        rect(3.0, 3.0, 322.0, 22.0),
        [0.92, 0.92, 0.90, 1.0],
        depth + 0.08,
    );
    c.text(
        content.city,
        7.0 * factor,
        12.0 * factor,
        145.0 * factor,
        9.0 * factor,
        INK,
        false,
    );
    let year = content
        .date
        .split_whitespace()
        .last()
        .unwrap_or(content.date);
    c.text(
        &format!("{year} Budget"),
        7.0 * factor,
        23.0 * factor,
        145.0 * factor,
        9.0 * factor,
        INK,
        false,
    );
    c.text(
        content.date,
        7.0 * factor,
        34.0 * factor,
        145.0 * factor,
        9.0 * factor,
        INK,
        false,
    );
    let close = mac_close_rect(page);
    push_ui_rect(c.solids, close, [0.96, 0.96, 0.94, 1.0], depth + 0.1);
    c.text(
        "Done",
        284.0 * factor,
        8.0 * factor,
        36.0 * factor,
        9.0 * factor,
        INK,
        true,
    );
    let help = mac_help_rect(page);
    push_ui_rect(c.solids, help, [0.96, 0.96, 0.94, 1.0], depth + 0.1);
    c.text(
        "Help",
        284.0 * factor,
        26.0 * factor,
        36.0 * factor,
        9.0 * factor,
        INK,
        true,
    );
    c.text(
        "Year End",
        180.0 * factor,
        10.0 * factor,
        42.0 * factor,
        8.0 * factor,
        INK,
        false,
    );
    c.text(
        year,
        179.0 * factor,
        20.0 * factor,
        42.0 * factor,
        8.0 * factor,
        INK,
        true,
    );
    c.text(
        &year.parse::<u32>().map_or_else(
            |_| "Estimate".to_owned(),
            |year| year.saturating_add(1).to_string(),
        ),
        225.0 * factor,
        20.0 * factor,
        48.0 * factor,
        8.0 * factor,
        INK,
        true,
    );
    c.text(
        "Expense",
        179.0 * factor,
        29.0 * factor,
        42.0 * factor,
        8.0 * factor,
        INK,
        true,
    );
    c.text(
        "Estimate",
        225.0 * factor,
        29.0 * factor,
        48.0 * factor,
        8.0 * factor,
        INK,
        true,
    );

    let sum = |range: std::ops::Range<usize>, pick: fn(&Entry) -> i64| {
        range.map(|i| pick(&content.entries[i])).sum::<i64>()
    };
    let average_setting = |range: std::ops::Range<usize>| -> Option<u32> {
        let values: Vec<_> = range.filter_map(|i| content.entries[i].setting).collect();
        (!values.is_empty()).then(|| values.iter().sum::<u32>() / values.len() as u32)
    };
    let rows = [
        (
            "Property Taxes",
            average_setting(0..3),
            sum(0..3, |e| e.annual),
            sum(0..3, |e| e.year_to_date.unwrap_or(0)),
            Some(0usize),
        ),
        (
            "City Ordinances",
            None,
            content.entries[3].annual,
            content.entries[3].year_to_date.unwrap_or(0),
            None,
        ),
        (
            "Bond Payments",
            None,
            content.entries[4].annual,
            content.entries[4].year_to_date.unwrap_or(0),
            None,
        ),
        (
            "Police Department",
            content.entries[5].setting,
            content.entries[5].annual,
            content.entries[5].year_to_date.unwrap_or(0),
            Some(1usize),
        ),
        (
            "Fire Department",
            content.entries[6].setting,
            content.entries[6].annual,
            content.entries[6].year_to_date.unwrap_or(0),
            Some(2usize),
        ),
        (
            "Health & Welfare",
            content.entries[7].setting,
            content.entries[7].annual,
            content.entries[7].year_to_date.unwrap_or(0),
            Some(3usize),
        ),
        (
            "Education",
            average_setting(8..10),
            sum(8..10, |e| e.annual),
            sum(8..10, |e| e.year_to_date.unwrap_or(0)),
            Some(4usize),
        ),
        (
            "Transit Authority",
            average_setting(10..16),
            sum(10..16, |e| e.annual),
            sum(10..16, |e| e.year_to_date.unwrap_or(0)),
            Some(5usize),
        ),
    ];
    for (index, (label, setting, annual, ytd, control)) in rows.into_iter().enumerate() {
        let y = [53.0, 72.0, 90.0, 117.0, 135.0, 153.0, 172.0, 191.0][index];
        let row_top = (y - 8.0) * factor;
        c.rect(5.0 * factor, row_top, 157.0 * factor, 16.0 * factor, WHITE);
        c.rect(181.0 * factor, row_top, 44.0 * factor, 16.0 * factor, WHITE);
        c.rect(227.0 * factor, row_top, 45.0 * factor, 16.0 * factor, WHITE);
        c.rect(275.0 * factor, row_top, 19.0 * factor, 16.0 * factor, WHITE);
        c.rect(300.0 * factor, row_top, 18.0 * factor, 16.0 * factor, WHITE);
        for (x, width) in [
            (5.0, 157.0),
            (181.0, 44.0),
            (227.0, 45.0),
            (275.0, 19.0),
            (300.0, 18.0),
        ] {
            c.rect(x * factor, row_top, width * factor, 0.7 * factor, LINE);
            c.rect(
                x * factor,
                row_top + 15.3 * factor,
                width * factor,
                0.7 * factor,
                LINE,
            );
            c.rect(x * factor, row_top, 0.7 * factor, 16.0 * factor, LINE);
            c.rect(
                (x + width - 0.7) * factor,
                row_top,
                0.7 * factor,
                16.0 * factor,
                LINE,
            );
        }
        // Native rows expose a small calendar control and a help target. Their
        // input actions remain unimplemented until the original hit tests are
        // traced, but preserve the captured table's visual affordances.
        c.rect(
            278.0 * factor,
            (y - 5.0) * factor,
            13.0 * factor,
            1.0 * factor,
            INK,
        );
        c.rect(
            278.0 * factor,
            (y - 5.0) * factor,
            1.0 * factor,
            9.0 * factor,
            INK,
        );
        c.rect(
            290.0 * factor,
            (y - 5.0) * factor,
            1.0 * factor,
            9.0 * factor,
            INK,
        );
        c.rect(
            278.0 * factor,
            (y + 3.0) * factor,
            13.0 * factor,
            1.0 * factor,
            INK,
        );
        c.text(
            "?",
            303.0 * factor,
            (y - 5.0) * factor,
            12.0 * factor,
            9.0 * factor,
            INK,
            true,
        );
        if index == 3 {
            c.rect(
                5.0 * factor,
                (y - 8.0) * factor,
                318.0 * factor,
                1.0 * factor,
                LINE,
            );
        }
        c.text(
            label,
            6.0 * factor,
            (y - 4.0) * factor,
            119.0 * factor,
            9.0 * factor,
            INK,
            false,
        );
        if let Some(setting) = setting {
            c.text(
                &format!("%{setting}"),
                129.0 * factor,
                (y - 4.0) * factor,
                34.0 * factor,
                9.0 * factor,
                INK,
                true,
            );
        }
        if let Some(service) = control {
            let row = if index == 0 { 53.0 } else { y };
            let spinner = rect(166.0, row - 9.0, 15.0, 18.0);
            push_ui_rect(c.solids, spinner, [0.96, 0.96, 0.94, 1.0], depth + 0.1);
            push_ui_rect(c.solids, rect(166.0, row, 15.0, 0.7), LINE, depth + 0.12);
            let x = (166.0 + 5.5) * factor;
            c.rect(x, (row - 6.0) * factor, 4.0 * factor, 1.0 * factor, INK);
            c.rect(
                x + 1.5 * factor,
                (row - 7.5) * factor,
                1.0 * factor,
                4.0 * factor,
                INK,
            );
            c.rect(x, (row + 3.0) * factor, 4.0 * factor, 1.0 * factor, INK);
            let _ = service;
        }
        c.text(
            &money(ytd),
            184.0 * factor,
            (y - 4.0) * factor,
            39.0 * factor,
            9.0 * factor,
            INCOME,
            true,
        );
        c.text(
            &money(annual),
            226.0 * factor,
            (y - 4.0) * factor,
            46.0 * factor,
            9.0 * factor,
            cash_color(annual),
            true,
        );
    }
    c.rect(
        5.0 * factor,
        207.0 * factor,
        313.0 * factor,
        0.8 * factor,
        LINE,
    );
    let summary = mac_cashflow_summary(content.entries, content.funds);
    c.text(
        "Year to Date Cashflow",
        8.0 * factor,
        214.0 * factor,
        144.0 * factor,
        9.0 * factor,
        INK,
        false,
    );
    c.text(
        &money(summary.year_to_date),
        184.0 * factor,
        214.0 * factor,
        82.0 * factor,
        9.0 * factor,
        cash_color(summary.year_to_date),
        true,
    );
    c.text(
        "Estimated Cashflow",
        8.0 * factor,
        224.0 * factor,
        144.0 * factor,
        9.0 * factor,
        INK,
        false,
    );
    c.text(
        &money(summary.estimated),
        184.0 * factor,
        224.0 * factor,
        82.0 * factor,
        9.0 * factor,
        cash_color(summary.estimated),
        true,
    );
    c.text(
        "Current Funds",
        8.0 * factor,
        234.0 * factor,
        144.0 * factor,
        9.0 * factor,
        INK,
        false,
    );
    c.text(
        &money(content.funds),
        184.0 * factor,
        234.0 * factor,
        82.0 * factor,
        9.0 * factor,
        INK,
        true,
    );
    c.text(
        "End of Year Funds",
        8.0 * factor,
        244.0 * factor,
        144.0 * factor,
        9.0 * factor,
        INK,
        false,
    );
    c.text(
        &money(summary.end_of_year_funds),
        184.0 * factor,
        244.0 * factor,
        82.0 * factor,
        9.0 * factor,
        INK,
        true,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mac_budget_cashflow_summary_matches_captured_reviews_and_live_edit() {
        let mut entries = std::array::from_fn(|i| Entry {
            label: LABELS[i],
            setting: None,
            year_to_date: Some(0),
            mac_ytd_pre_month_division: None,
            mac_estimate_pre_month_division: None,
            annual: 0,
        });
        entries[0].year_to_date = Some(221);
        entries[0].annual = 221;
        entries[1].year_to_date = Some(49);
        entries[1].annual = 49;
        entries[2].year_to_date = Some(249);
        entries[2].annual = 227;
        entries[5].year_to_date = Some(-100);
        entries[5].annual = -100;
        entries[6].year_to_date = Some(-100);
        entries[6].annual = -100;
        entries[8].year_to_date = Some(-25);
        entries[8].annual = -25;
        entries[10].year_to_date = Some(-23);
        entries[10].annual = -23;
        assert_eq!(
            mac_cashflow_summary(&entries, -9_604),
            MacCashflowSummary {
                year_to_date: 271,
                estimated: 249,
                end_of_year_funds: -9_333,
            }
        );

        for entry in &mut entries {
            entry.year_to_date = Some(0);
            entry.annual = 0;
        }
        entries[0].year_to_date = Some(-211);
        entries[0].annual = -217;
        assert_eq!(
            mac_cashflow_summary(&entries, 30_358),
            MacCashflowSummary {
                year_to_date: -211,
                estimated: -217,
                end_of_year_funds: 30_147,
            },
            "January 2047 Quadra screenshot also adds YTD to current funds"
        );

        for entry in &mut entries {
            entry.year_to_date = Some(0);
            entry.annual = 0;
        }
        entries[0].year_to_date = Some(225);
        entries[0].annual = 228;
        entries[1].year_to_date = Some(42);
        entries[1].annual = 42;
        entries[2].year_to_date = Some(259);
        entries[2].annual = 263;
        entries[5].year_to_date = Some(-100);
        entries[5].annual = -100;
        entries[6].year_to_date = Some(-100);
        entries[6].annual = -100;
        entries[8].year_to_date = Some(-25);
        entries[8].annual = -25;
        entries[10].year_to_date = Some(-23);
        entries[10].annual = -23;
        assert_eq!(
            mac_cashflow_summary(&entries, -8_661),
            MacCashflowSummary {
                year_to_date: 278,
                estimated: 285,
                end_of_year_funds: -8_383,
            },
            "January 2000 AutoDebt RAM confirms YTD-based end-of-year funds"
        );

        for entry in &mut entries {
            entry.year_to_date = Some(0);
            entry.annual = 0;
        }
        entries[0].year_to_date = Some(228);
        entries[0].annual = 228;
        entries[1].year_to_date = Some(42);
        entries[1].annual = 42;
        entries[2].year_to_date = Some(233);
        entries[2].annual = 233;
        entries[5].year_to_date = Some(-100);
        entries[5].annual = -95;
        entries[6].year_to_date = Some(-100);
        entries[6].annual = -100;
        entries[8].year_to_date = Some(-25);
        entries[8].annual = -25;
        entries[10].year_to_date = Some(-23);
        entries[10].annual = -23;
        assert_eq!(
            mac_cashflow_summary(&entries, -8_105),
            MacCashflowSummary {
                year_to_date: 255,
                estimated: 260,
                end_of_year_funds: -7_850,
            },
            "January 2002 Police decrease changes estimate, not YTD or end-of-year funds"
        );

        for entry in &mut entries {
            entry.year_to_date = Some(0);
            entry.annual = 0;
        }
        entries[0].year_to_date = Some(228);
        entries[0].annual = 228;
        entries[1].year_to_date = Some(42);
        entries[1].annual = 42;
        entries[2].year_to_date = Some(212);
        entries[2].annual = 242;
        entries[5].year_to_date = Some(-95);
        entries[5].annual = -95;
        entries[6].year_to_date = Some(-100);
        entries[6].annual = -100;
        entries[8].year_to_date = Some(-25);
        entries[8].annual = -25;
        entries[10].year_to_date = Some(-23);
        entries[10].annual = -23;
        assert_eq!(
            mac_cashflow_summary(&entries, -6_301),
            MacCashflowSummary {
                year_to_date: 239,
                estimated: 269,
                end_of_year_funds: -6_062,
            },
            "January 2009 AutoDebt RAM confirms YTD-based end-of-year funds"
        );

        for entry in &mut entries {
            entry.year_to_date = Some(0);
            entry.annual = 0;
        }
        entries[0].year_to_date = Some(225);
        entries[0].annual = 228;
        entries[1].year_to_date = Some(42);
        entries[1].annual = 42;
        entries[2].year_to_date = Some(236);
        entries[2].annual = 257;
        entries[5].year_to_date = Some(-95);
        entries[5].annual = -95;
        entries[6].year_to_date = Some(-100);
        entries[6].annual = -100;
        entries[8].year_to_date = Some(-25);
        entries[8].annual = -25;
        entries[10].year_to_date = Some(-23);
        entries[10].annual = -23;
        assert_eq!(
            mac_cashflow_summary(&entries, -6_062),
            MacCashflowSummary {
                year_to_date: 260,
                estimated: 284,
                end_of_year_funds: -5_802,
            },
            "January 2010 AutoDebt RAM confirms YTD-based end-of-year funds"
        );

        for entry in &mut entries {
            entry.year_to_date = Some(0);
            entry.annual = 0;
        }
        entries[0].year_to_date = Some(227);
        entries[0].annual = 221;
        entries[1].year_to_date = Some(42);
        entries[1].annual = 42;
        entries[2].year_to_date = Some(255);
        entries[2].annual = 174;
        entries[5].year_to_date = Some(-95);
        entries[5].annual = -95;
        entries[6].year_to_date = Some(-100);
        entries[6].annual = -100;
        entries[8].year_to_date = Some(-25);
        entries[8].annual = -25;
        entries[10].year_to_date = Some(-23);
        entries[10].annual = -23;
        assert_eq!(
            mac_cashflow_summary(&entries, -1_100),
            MacCashflowSummary {
                year_to_date: 281,
                estimated: 194,
                end_of_year_funds: -819,
            },
            "January 2029 Quadra RAM and paused screen agree on all three totals"
        );

        for entry in &mut entries {
            entry.year_to_date = Some(0);
            entry.annual = 0;
        }
        entries[10].year_to_date = Some(-18);
        entries[10].annual = -18;
        assert_eq!(
            mac_cashflow_summary(&entries, -59_687),
            MacCashflowSummary {
                year_to_date: -18,
                estimated: -18,
                end_of_year_funds: -59_705,
            },
            "January 2294 AutoDebt screen and RAM agree on Transit and end funds"
        );

        for entry in &mut entries {
            entry.year_to_date = Some(0);
            entry.annual = 0;
        }
        entries[0].year_to_date = Some(i64::from(i32::MAX));
        entries[1].year_to_date = Some(1);
        assert_eq!(
            mac_cashflow_summary(&entries, 0).year_to_date,
            i64::from(i32::MIN),
            "Mac summary aggregation follows the row dword representation"
        );
        assert_eq!(
            mac_cashflow_summary(&entries, 0).end_of_year_funds,
            i64::from(i32::MIN),
            "end-of-year funds add the wrapped year-to-date total"
        );

        entries[5].annual = i64::from(i32::MAX);
        entries[6].annual = 1;
        assert_eq!(
            mac_cashflow_summary(&entries, 0).estimated,
            i64::from(i32::MIN),
            "Mac summary aggregation follows the row dword representation"
        );
    }

    #[test]
    fn mac_budget_summary_divides_combined_row_accumulator_after_dividing_rows() {
        let mut entries = std::array::from_fn(|i| Entry {
            label: LABELS[i],
            setting: Some(0),
            year_to_date: Some(0),
            mac_ytd_pre_month_division: Some(0),
            mac_estimate_pre_month_division: Some(0),
            annual: 0,
        });
        let captures = [
            (
                [
                    2622, 593, 2529, 0, 0, -1200, -1200, 0, -300, 0, -283, 0, 0, 0, 0, 0,
                ],
                [
                    2609, 593, 2576, 0, 0, -1200, -1200, 0, -300, 0, -283, 0, 0, 0, 0, 0,
                ],
                [
                    218, 49, 210, 0, 0, -100, -100, 0, -25, 0, -23, 0, 0, 0, 0, 0,
                ],
                -8863,
                230,
                232,
            ),
            (
                [
                    2685, 593, 2479, 0, 0, -1200, -1200, 0, -300, 0, -283, 0, 0, 0, 0, 0,
                ],
                [
                    2609, 593, 2542, 0, 0, -1200, -1200, 0, -300, 0, -283, 0, 0, 0, 0, 0,
                ],
                [
                    223, 49, 206, 0, 0, -100, -100, 0, -25, 0, -23, 0, 0, 0, 0, 0,
                ],
                -7406,
                231,
                230,
            ),
            (
                [
                    2698, 504, 2758, 0, 0, -1200, -1200, 0, -300, 0, -283, 0, 0, 0, 0, 0,
                ],
                [
                    2688, 504, 3057, 0, 0, -1200, -1200, 0, -300, 0, -283, 0, 0, 0, 0, 0,
                ],
                [
                    224, 42, 229, 0, 0, -100, -100, 0, -25, 0, -23, 0, 0, 0, 0, 0,
                ],
                -5032,
                248,
                272,
            ),
            (
                [
                    2715, 504, 2893, 0, 0, -1200, -1200, 0, -300, 0, -283, 0, 0, 0, 0, 0,
                ],
                [
                    2788, 504, 2676, 0, 0, -1200, -1200, 0, -300, 0, -283, 0, 0, 0, 0, 0,
                ],
                [
                    226, 42, 241, 0, 0, -100, -100, 0, -25, 0, -23, 0, 0, 0, 0, 0,
                ],
                -4785,
                260,
                248,
            ),
            (
                [
                    2788, 504, 2831, 0, 0, -1200, -1200, 0, -300, 0, -283, 0, 0, 0, 0, 0,
                ],
                [
                    2788, 504, 2721, 0, 0, -1200, -1200, 0, -300, 0, -283, 0, 0, 0, 0, 0,
                ],
                [
                    232, 42, 235, 0, 0, -100, -100, 0, -25, 0, -23, 0, 0, 0, 0, 0,
                ],
                -4524,
                261,
                252,
            ),
            (
                [
                    2786, 504, 2401, 0, 0, -1200, -1200, 0, -300, 0, -283, 0, 0, 0, 0, 0,
                ],
                [
                    2788, 504, 2598, 0, 0, -1200, -1200, 0, -300, 0, -283, 0, 0, 0, 0, 0,
                ],
                [
                    232, 42, 200, 0, 0, -100, -100, 0, -25, 0, -23, 0, 0, 0, 0, 0,
                ],
                -4263,
                225,
                242,
            ),
        ];

        for (
            ytd_pre_month_division,
            estimate_pre_month_division,
            row_ytd,
            funds,
            expected_ytd,
            expected_estimate,
        ) in captures
        {
            for (entry, ((ytd_pre_month_division, estimate_pre_month_division), row_ytd)) in
                entries.iter_mut().zip(
                    ytd_pre_month_division
                        .into_iter()
                        .zip(estimate_pre_month_division)
                        .zip(row_ytd),
                )
            {
                entry.mac_ytd_pre_month_division = Some(ytd_pre_month_division);
                entry.mac_estimate_pre_month_division = Some(estimate_pre_month_division);
                entry.year_to_date = Some(row_ytd);
            }
            let summary = mac_cashflow_summary(&entries, funds);
            assert_eq!(summary.year_to_date, expected_ytd);
            assert_eq!(summary.estimated, expected_estimate);
            assert_eq!(summary.end_of_year_funds, funds + expected_ytd);
        }
    }

    #[test]
    fn mac_january_2012_budget_estimate_combines_row_contributions_before_twelfths() {
        let row_ytd = [
            224, 42, 207, 0, 0, -100, -100, 0, -25, 0, -23, 0, 0, 0, 0, 0,
        ];
        let row_estimate = [
            224, 42, 223, 0, 0, -100, -100, 0, -25, 0, -23, 0, 0, 0, 0, 0,
        ];
        let ytd_pre_month_division = [
            2697, 504, 2492, 0, 0, -1200, -1200, 0, -300, 0, -283, 0, 0, 0, 0, 0,
        ];
        let estimate_pre_month_division = [
            2688, 504, 2676, 0, 0, -1200, -1200, 0, -300, 0, -283, 0, 0, 0, 0, 0,
        ];
        let entries = std::array::from_fn(|index| Entry {
            label: LABELS[index],
            setting: Some(0),
            year_to_date: Some(row_ytd[index]),
            mac_ytd_pre_month_division: Some(ytd_pre_month_division[index]),
            mac_estimate_pre_month_division: Some(estimate_pre_month_division[index]),
            annual: row_estimate[index],
        });

        assert_eq!(row_estimate.into_iter().sum::<i64>(), 241);
        assert_eq!(
            mac_cashflow_summary(&entries, -5539),
            MacCashflowSummary {
                year_to_date: 225,
                estimated: 240,
                end_of_year_funds: -5314,
            },
            "R13 January 2012 panel uses the combined pre-division accumulators"
        );
    }

    #[test]
    fn mac_january_2013_budget_estimate_combines_row_contributions_before_twelfths() {
        let row_ytd = [
            224, 42, 198, 0, 0, -100, -100, 0, -25, 0, -23, 0, 0, 0, 0, 0,
        ];
        let row_estimate = [
            224, 42, 186, 0, 0, -100, -100, 0, -25, 0, -23, 0, 0, 0, 0, 0,
        ];
        let ytd_pre_month_division = [
            2695, 504, 2380, 0, 0, -1200, -1200, 0, -300, 0, -283, 0, 0, 0, 0, 0,
        ];
        let estimate_pre_month_division = [
            2699, 504, 2240, 0, 0, -1200, -1200, 0, -300, 0, -283, 0, 0, 0, 0, 0,
        ];
        let entries = std::array::from_fn(|index| Entry {
            label: LABELS[index],
            setting: Some(0),
            year_to_date: Some(row_ytd[index]),
            mac_ytd_pre_month_division: Some(ytd_pre_month_division[index]),
            mac_estimate_pre_month_division: Some(estimate_pre_month_division[index]),
            annual: row_estimate[index],
        });

        assert_eq!(row_estimate.into_iter().sum::<i64>(), 204);
        assert_eq!(
            mac_cashflow_summary(&entries, -5314),
            MacCashflowSummary {
                year_to_date: 216,
                estimated: 205,
                end_of_year_funds: -5098,
            },
            "R13 January 2013 panel uses the combined pre-division accumulators"
        );
    }

    #[test]
    fn mac_january_2020_budget_summary_combines_ytd_and_estimate_before_twelfths() {
        let row_ytd = [
            219, 42, 222, 0, 0, -100, -100, 0, -25, 0, -23, 0, 0, 0, 0, 0,
        ];
        let row_estimate = [
            209, 42, 238, 0, 0, -100, -100, 0, -25, 0, -23, 0, 0, 0, 0, 0,
        ];
        let ytd_pre_month_division = [
            2632, 504, 2665, 0, 0, -1200, -1200, 0, -300, 0, -283, 0, 0, 0, 0, 0,
        ];
        let estimate_pre_month_division = [
            2508, 504, 2856, 0, 0, -1200, -1200, 0, -300, 0, -283, 0, 0, 0, 0, 0,
        ];
        let entries = std::array::from_fn(|index| Entry {
            label: LABELS[index],
            setting: Some(0),
            year_to_date: Some(row_ytd[index]),
            mac_ytd_pre_month_division: Some(ytd_pre_month_division[index]),
            mac_estimate_pre_month_division: Some(estimate_pre_month_division[index]),
            annual: row_estimate[index],
        });

        assert_eq!(row_ytd.into_iter().sum::<i64>(), 235);
        assert_eq!(row_estimate.into_iter().sum::<i64>(), 241);
        assert_eq!(
            mac_cashflow_summary(&entries, -3482),
            MacCashflowSummary {
                year_to_date: 234,
                estimated: 240,
                end_of_year_funds: -3248,
            },
            "R13 January 2020 combines each native accumulator before dividing by 12"
        );
    }

    #[test]
    fn mac_january_2026_budget_summary_matches_native_accumulator_totals() {
        let row_ytd = [
            228, 42, 259, 0, 0, -100, -100, 0, -25, 0, -23, 0, 0, 0, 0, 0,
        ];
        let row_estimate = [
            228, 42, 245, 0, 0, -100, -100, 0, -25, 0, -23, 0, 0, 0, 0, 0,
        ];
        let ytd_pre_month_division = [
            2742, 504, 3114, 0, 0, -1200, -1200, 0, -300, 0, -283, 0, 0, 0, 0, 0,
        ];
        let estimate_pre_month_division = [
            2744, 504, 2945, 0, 0, -1200, -1200, 0, -300, 0, -283, 0, 0, 0, 0, 0,
        ];
        let entries = std::array::from_fn(|index| Entry {
            label: LABELS[index],
            setting: Some(0),
            year_to_date: Some(row_ytd[index]),
            mac_ytd_pre_month_division: Some(ytd_pre_month_division[index]),
            mac_estimate_pre_month_division: Some(estimate_pre_month_division[index]),
            annual: row_estimate[index],
        });

        assert_eq!(row_ytd.into_iter().sum::<i64>(), 281);
        assert_eq!(row_estimate.into_iter().sum::<i64>(), 267);
        assert_eq!(
            mac_cashflow_summary(&entries, -1966),
            MacCashflowSummary {
                year_to_date: 281,
                estimated: 267,
                end_of_year_funds: -1685,
            },
            "R13 January 2026 agrees with the captured native cashflow totals"
        );
    }

    #[test]
    fn mac_january_2034_budget_summary_combines_ytd_before_twelfths() {
        let row_ytd = [
            228, 42, 219, 0, 0, -100, -100, 0, -25, 0, -23, 0, 0, 0, 0, 0,
        ];
        let row_estimate = [
            226, 42, 227, 0, 0, -100, -100, 0, -25, 0, -23, 0, 0, 0, 0, 0,
        ];
        let ytd_pre_month_division = [
            2742, 504, 2628, 0, 0, -1200, -1200, 0, -300, 0, -283, 0, 0, 0, 0, 0,
        ];
        let estimate_pre_month_division = [
            2721, 504, 2732, 0, 0, -1200, -1200, 0, -300, 0, -283, 0, 0, 0, 0, 0,
        ];
        let entries = std::array::from_fn(|index| Entry {
            label: LABELS[index],
            setting: Some(0),
            year_to_date: Some(row_ytd[index]),
            mac_ytd_pre_month_division: Some(ytd_pre_month_division[index]),
            mac_estimate_pre_month_division: Some(estimate_pre_month_division[index]),
            annual: row_estimate[index],
        });

        assert_eq!(row_ytd.into_iter().sum::<i64>(), 241);
        assert_eq!(row_estimate.into_iter().sum::<i64>(), 247);
        assert_eq!(
            mac_cashflow_summary(&entries, 120),
            MacCashflowSummary {
                year_to_date: 240,
                estimated: 247,
                end_of_year_funds: 360,
            },
            "R13 January 2034 combines native YTD contributions before dividing by 12"
        );
    }

    #[test]
    fn mac_january_2036_budget_summary_combines_ytd_before_twelfths() {
        let row_ytd = [
            227, 42, 233, 0, 0, -100, -100, 0, -25, 0, -23, 0, 0, 0, 0, 0,
        ];
        let row_estimate = [
            228, 42, 247, 0, 0, -100, -100, 0, -25, 0, -23, 0, 0, 0, 0, 0,
        ];
        let ytd_pre_month_division = [
            2733, 504, 2806, 0, 0, -1200, -1200, 0, -300, 0, -283, 0, 0, 0, 0, 0,
        ];
        let estimate_pre_month_division = [
            2744, 504, 2968, 0, 0, -1200, -1200, 0, -300, 0, -283, 0, 0, 0, 0, 0,
        ];
        let entries = std::array::from_fn(|index| Entry {
            label: LABELS[index],
            setting: Some(0),
            year_to_date: Some(row_ytd[index]),
            mac_ytd_pre_month_division: Some(ytd_pre_month_division[index]),
            mac_estimate_pre_month_division: Some(estimate_pre_month_division[index]),
            annual: row_estimate[index],
        });

        assert_eq!(row_ytd.into_iter().sum::<i64>(), 254);
        assert_eq!(row_estimate.into_iter().sum::<i64>(), 269);
        assert_eq!(
            mac_cashflow_summary(&entries, 575),
            MacCashflowSummary {
                year_to_date: 255,
                estimated: 269,
                end_of_year_funds: 830,
            },
            "R13 January 2036 combines native YTD contributions before dividing by 12"
        );
    }

    #[test]
    fn mac_january_2037_budget_summary_combines_estimate_before_twelfths() {
        let row_ytd = [
            212, 35, 254, 0, 0, -100, -100, 0, -25, 0, -23, 0, 0, 0, 0, 0,
        ];
        let row_estimate = [
            189, 30, 243, 0, 0, -100, -100, 0, -25, 0, -23, 0, 0, 0, 0, 0,
        ];
        let ytd_pre_month_division = [
            2552, 420, 3053, 0, 0, -1200, -1200, 0, -300, 0, -283, 0, 0, 0, 0, 0,
        ];
        let estimate_pre_month_division = [
            2273, 369, 2923, 0, 0, -1200, -1200, 0, -300, 0, -283, 0, 0, 0, 0, 0,
        ];
        let entries = std::array::from_fn(|index| Entry {
            label: LABELS[index],
            setting: Some(0),
            year_to_date: Some(row_ytd[index]),
            mac_ytd_pre_month_division: Some(ytd_pre_month_division[index]),
            mac_estimate_pre_month_division: Some(estimate_pre_month_division[index]),
            annual: row_estimate[index],
        });

        assert_eq!(row_ytd.into_iter().sum::<i64>(), 253);
        assert_eq!(row_estimate.into_iter().sum::<i64>(), 214);
        assert_eq!(
            mac_cashflow_summary(&entries, 829),
            MacCashflowSummary {
                year_to_date: 253,
                estimated: 215,
                end_of_year_funds: 1082,
            },
            "R13 January 2037 combines native estimate contributions before dividing by 12"
        );
    }

    #[test]
    fn mac_budget_estimate_projection_uses_saved_ytd_and_remaining_months() {
        let mut city = City::new_blank("/tmp/mac-budget-projection.SC2", "Budget projection")
            .expect("blank city");
        let offset = budget_row_misc_offset_for_city(&city, 0, 0).expect("tax row");
        let mut values = vec![0; BUDGET_SERVICE_MISC_STRIDE];
        values[0] = 1_000;
        values[1] = 5;
        values[2] = 10_000;
        city.set_budget_misc_u32_values(offset, &values);

        let midyear = entries_for_city_for_month(&city, EngineFidelity::Mac68k, 6, false);
        assert_eq!(
            midyear[0].mac_estimate_pre_month_division,
            Some(40_000 / 75)
        );

        let annual_review = entries_for_city_for_month(&city, EngineFidelity::Mac68k, 1, true);
        assert_eq!(
            annual_review[0].mac_estimate_pre_month_division,
            Some(60_000 / 75)
        );
    }

    #[test]
    fn mac_budget_window_displays_zeroed_native_rows() {
        let mut city = City::new_blank("/tmp/budget-window-zero.SC2", "Zero budget").unwrap();
        for row in [0, BUDGET_SERVICE_ROW_START] {
            let offset = budget_row_misc_offset_for_city(&city, row, 0).unwrap();
            city.set_budget_misc_u32_values(offset, &[0; BUDGET_SERVICE_MISC_STRIDE]);
        }
        let mac = entries_for_city(&city, EngineFidelity::Mac68k);
        let win = entries_for_city(&city, EngineFidelity::Win95);
        assert_eq!(mac[0].setting, Some(0));
        assert_eq!(mac[BUDGET_SERVICE_ROW_START].setting, Some(0));
        assert_eq!(win[0].setting, Some(DEFAULT_TAX_RATE));
        assert_eq!(win[BUDGET_SERVICE_ROW_START].setting, Some(100));
    }

    #[test]
    fn budget_window_formats_signed_money_and_large_balances() {
        assert_eq!(money(-1234567), "-$1,234,567");
        assert_eq!(money(0), "$0");
        assert_eq!(money(i64::MIN), "-$9,223,372,036,854,775,808");
    }

    #[test]
    fn budget_window_cash_columns_use_their_engines_accumulator_units() {
        let mut city = City::new_blank("/tmp/budget-window.SC2", "Budget test").unwrap();
        let offset = budget_row_misc_offset_for_city(&city, 0, 0).unwrap();
        let mut values = vec![0; BUDGET_SERVICE_MISC_STRIDE];
        values[0] = 750;
        values[1] = 8;
        values[2] = 9000;
        city.set_budget_misc_u32_values(offset, &values);
        let windows = entries_for_city(&city, EngineFidelity::Win95);
        let mac = entries_for_city(&city, EngineFidelity::Mac68k);
        assert_eq!(windows[0].annual, 80);
        assert_eq!(mac[0].annual, 80);
        assert_eq!(windows[0].year_to_date, Some(10));
        assert_eq!(mac[0].year_to_date, Some(10));
        assert_eq!(
            windows[BUDGET_ORDINANCE_ROW].year_to_date,
            Some(0),
            "PE ordinances share the raw row accumulator and settlement divisor"
        );
    }

    #[test]
    fn budget_window_content_and_controls_fit_small_and_retina_windows() {
        let entries = std::array::from_fn(|i| Entry {
            label: LABELS[i],
            setting: Some(100),
            year_to_date: Some(-9_999_999_999),
            mac_ytd_pre_month_division: None,
            mac_estimate_pre_month_division: None,
            annual: 9_999_999_999,
        });
        for window in [(400, 360), (1024, 768), (2624, 1644)] {
            let page = page_rect(window);
            let content = Content {
                city: "A city with a very long name",
                date: "August 1989",
                funds: -2_000_000_000,
                bonds: 50,
                rating: "AAA",
                auto_budget: true,
                can_issue: false,
                can_repay: false,
                entries: &entries,
                font_atlas: Some(0),
                dark_appearance: false,
                mac_layout: false,
            };
            let (mut solids, mut images) = (Vec::new(), Vec::new());
            draw(page, &content, &mut solids, &mut images, 100.0);
            for image in images {
                assert!(image.x >= page.x - 0.5 && image.y >= page.y - 0.5);
                assert!(
                    image.x + image.w <= page.x + page.w + 0.5
                        && image.y + image.h <= page.y + page.h + 0.5
                );
            }
            for solid in solids {
                assert!(
                    solid.x >= 0.0
                        && solid.y >= 0.0
                        && solid.x + solid.w <= window.0 as f32
                        && solid.y + solid.h <= window.1 as f32
                );
            }
            for row in 0..BUDGET_TOTAL_COUNT {
                let left = adjust_rect(page, row, false);
                let right = adjust_rect(page, row, true);
                assert!(left.x + left.w < right.x && right.x + right.w < page.x + page.w);
            }
        }
    }

    #[test]
    fn mac_budget_window_uses_quadra_640_by_480_geometry() {
        let page = page_rect_for_engine((640, 480), EngineFidelity::Mac68k);
        assert!((page.x - 156.0).abs() < 0.01);
        assert!((page.y - 111.5).abs() < 0.01);
        assert!((page.w - 328.0).abs() < 0.01);
        assert!((page.h - 257.0).abs() < 0.01);
        assert!(point_in_float_rect(
            Vec2::new(458.0, 123.5),
            mac_close_rect(page)
        ));
        assert!(point_in_float_rect(
            Vec2::new(329.0, 228.0),
            mac_service_adjust_rect(page, 0, false).unwrap()
        ));
        assert!(!point_in_float_rect(
            Vec2::new(329.0, 228.0),
            service_adjust_rect(page_rect((640, 480)), 0, false)
        ));
    }
}
