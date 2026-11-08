use crate::traffic::GameSpeed;

/// Win95 `dwCityDays % 25` SimCalendar days per month.
pub const DAYS_PER_MONTH: u8 = 25;
const DAYS_PER_YEAR: u32 = 300;

const MONTHS_PER_SECOND_AT_LLAMA: f32 = 0.5;
const DAYS_PER_SECOND_AT_LLAMA: f32 = MONTHS_PER_SECOND_AT_LLAMA * DAYS_PER_MONTH as f32;

const MONTH_NAMES: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct GameClockAdvance {
    pub days: u32,
    pub months: u32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GameClock {
    year: u16,
    month: u8,
    /// SimCalendar day `0..=24` (`dwCityDays % 25`).
    day: u8,
    accumulator: f32,
}

impl GameClock {
    pub fn new(year: u16, month: u8) -> Self {
        Self::new_with_day(year, month, 0)
    }

    pub fn new_with_day(year: u16, month: u8, day: u8) -> Self {
        Self {
            year,
            month: month.clamp(1, 12),
            day: day.min(DAYS_PER_MONTH - 1),
            accumulator: 0.0,
        }
    }

    pub fn from_city_age(founded_year: u16, age_days: u32) -> Self {
        let year = founded_year.saturating_add((age_days / DAYS_PER_YEAR) as u16);
        let month = ((age_days % DAYS_PER_YEAR) / u32::from(DAYS_PER_MONTH) + 1) as u8;
        let day = (age_days % u32::from(DAYS_PER_MONTH)) as u8;
        Self::new_with_day(year, month, day)
    }

    pub fn default_start() -> Self {
        Self::new(1900, 1)
    }

    pub fn year(self) -> u16 {
        self.year
    }

    pub fn month(self) -> u8 {
        self.month
    }

    pub fn day(self) -> u8 {
        self.day
    }

    /// Display day `1..=25` (`dwCityDays % 25 + 1` in sc2kfix).
    pub fn display_day(self) -> u8 {
        self.day + 1
    }

    pub fn status_line(self) -> String {
        format!(
            "{} {} {}",
            MONTH_NAMES[self.month as usize - 1],
            self.display_day(),
            self.year
        )
    }

    pub fn month_year_label(self) -> String {
        format!("{} {}", MONTH_NAMES[self.month as usize - 1], self.year)
    }

    pub fn tick(&mut self, delta_seconds: f32, speed: GameSpeed) -> GameClockAdvance {
        let multiplier = speed.tick_multiplier();
        if multiplier <= 0.0 {
            return GameClockAdvance::default();
        }
        self.accumulator += delta_seconds * multiplier * DAYS_PER_SECOND_AT_LLAMA;
        let mut advance = GameClockAdvance::default();
        while self.accumulator >= 1.0 {
            self.accumulator -= 1.0;
            if self.advance_day() {
                advance.months += 1;
            }
            advance.days += 1;
        }
        advance
    }

    /// Calendar after advancing `days` SimCalendar days (month/year wrap included).
    pub fn wrapping_add_days(self, days: u32) -> Self {
        let mut clock = self;
        for _ in 0..days {
            clock.advance_day();
        }
        clock
    }

    fn advance_day(&mut self) -> bool {
        self.day += 1;
        if self.day >= DAYS_PER_MONTH {
            self.day = 0;
            self.advance_month();
            true
        } else {
            false
        }
    }

    fn advance_month(&mut self) {
        if self.month >= 12 {
            self.month = 1;
            self.year = self.year.saturating_add(1);
        } else {
            self.month += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn advances_month_at_llama_speed() {
        let mut clock = GameClock::default_start();
        let advance = clock.tick(2.5, GameSpeed::Llama);
        assert_eq!(advance.months, 1);
        assert_eq!(advance.days, 31);
        assert_eq!(clock.month(), 2);
        assert_eq!(clock.day(), 6);
        assert_eq!(clock.status_line(), "Feb 7 1900");
    }

    #[test]
    fn twenty_five_llama_days_wrap_exactly_one_month() {
        let mut clock = GameClock::default_start();
        let advance = clock.tick(2.0, GameSpeed::Llama);
        assert_eq!(advance.days, 25);
        assert_eq!(advance.months, 1);
        assert_eq!(clock.month(), 2);
        assert_eq!(clock.day(), 0);
        assert_eq!(clock.year(), 1900);
        assert_eq!(clock.status_line(), "Feb 1 1900");
    }

    #[test]
    fn paused_does_not_advance() {
        let mut clock = GameClock::default_start();
        assert_eq!(
            clock.tick(10.0, GameSpeed::Paused),
            GameClockAdvance::default()
        );
        assert_eq!(clock.month(), 1);
        assert_eq!(clock.day(), 0);
    }

    #[test]
    fn city_age_restores_sim_calendar_day() {
        let clock = GameClock::from_city_age(1900, 350 + 13);
        assert_eq!(clock.year(), 1901);
        assert_eq!(clock.month(), 3);
        assert_eq!(clock.day(), 13);
        assert_eq!(clock.display_day(), 14);
    }

    #[test]
    fn wrapping_add_days_crosses_month_and_year() {
        let clock = GameClock::new_with_day(1900, 12, 24);
        let next = clock.wrapping_add_days(2);
        assert_eq!(next.year(), 1901);
        assert_eq!(next.month(), 1);
        assert_eq!(next.day(), 1);
        assert_eq!(next.status_line(), "Jan 2 1901");
    }
}
