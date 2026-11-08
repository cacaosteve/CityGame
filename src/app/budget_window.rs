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
}

pub(super) fn entries_for_city(
    city: &City,
    fidelity: EngineFidelity,
) -> [Entry; BUDGET_TOTAL_COUNT] {
    let tax_bases = budget_tax_bases_for_city_with_fidelity(
        city,
        live_population_stats_for_city(city),
        fidelity,
    );
    let services = budget_service_rows_for_city(city);
    std::array::from_fn(|index| {
        let offset = budget_row_misc_offset_for_city(city, index, 0);
        let values = offset
            .map(|offset| city.budget_misc_u32_values(offset, BUDGET_SERVICE_MISC_STRIDE))
            .unwrap_or_default();
        let field = |n: usize| values.get(n).copied().unwrap_or(0);
        let setting = if index < BUDGET_TAX_COUNT {
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

#[cfg(test)]
mod tests {
    use super::*;

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
}
