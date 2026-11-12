//! Asset-independent CityGame start screen and scenario browser.
use super::scenario_text_for_city;
use crate::asset_profile::{AssetProfile, OriginalMedia, TitleMusicPreference};
use crate::city::City;
use crate::palette::AnimatedPalette;
use crate::renderer::UiSolid;
use crate::ui;
use std::collections::BTreeSet;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use super::terrain_generation;

const NAVY: [f32; 4] = [0.035, 0.075, 0.12, 1.0];
const PANEL: [f32; 4] = [0.075, 0.14, 0.19, 0.98];
const WHITE: [f32; 4] = [0.91, 0.95, 0.93, 1.0];
const MUTED: [f32; 4] = [0.56, 0.68, 0.7, 1.0];
const TEAL: [f32; 4] = [0.26, 0.84, 0.75, 1.0];
const GOLD: [f32; 4] = [1.0, 0.72, 0.29, 1.0];
const PREVIEW_SIZE: usize = 32;
const TERRAIN_PREVIEW_SIZE: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Page {
    Home,
    NewCity,
    Scenarios,
    Graphics,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Choice {
    Continue,
    NewCity,
    EditMap,
    LoadCity,
    Scenarios,
    Graphics,
    CycleMusic,
    Quit,
    StartGenerated,
    StartEditing,
    CycleDifficulty,
    CycleYear,
    SelectScenario(usize),
    PlayScenario,
    BrowseScenario,
    ImportOriginal,
    ChooseOriginal,
    ChooseReplacement,
    CycleGraphicsSource,
    CycleEffectsSource,
    CycleMusicSource,
    ToggleCityGameEffects,
    ToggleCityGameMusic,
    ToggleCityGameCityMusic,
    ToggleToolbarPalette,
    ChooseTileSet,
    ClearTileSet,
    PreferOriginal,
    PreferOurs,
    Back,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NewCityStartFlow {
    GenerateAndPlay,
    GenerateAndEdit,
}

impl Choice {
    pub fn new_city_start_flow(self) -> Option<NewCityStartFlow> {
        match self {
            Choice::StartGenerated => Some(NewCityStartFlow::GenerateAndPlay),
            Choice::StartEditing => Some(NewCityStartFlow::GenerateAndEdit),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct Rect {
    x: f32,
    y: f32,
    w: f32,
    h: f32,
}

impl Rect {
    fn contains(self, x: f32, y: f32) -> bool {
        x >= self.x && y >= self.y && x < self.x + self.w && y < self.y + self.h
    }
}

struct Button {
    choice: Choice,
    rect: Rect,
}

pub struct ScenarioEntry {
    pub path: PathBuf,
    pub title: String,
    pub brief: String,
    pub mac_action: Option<u16>,
    preview: Vec<[f32; 4]>,
}

pub struct TitleScreen {
    pub page: Page,
    pub city_name: String,
    pub difficulty_index: usize,
    pub year_index: usize,
    pub scenario_index: usize,
    pub scenario_offset: usize,
    pub scenarios: Vec<ScenarioEntry>,
    pub generation_seed: u32,
    pub status: Option<String>,
    terrain_preview: Vec<[f32; 4]>,
    buttons: Vec<Button>,
}

/// PE NewGameSetup choices: difficulty 1/2/3 starts with 20k/10k/10k cash.
/// Hard also initializes one 3% bond; new-city setup applies it to MISC.
pub const DIFFICULTIES: [(&str, u16, i32); 3] = [
    ("Easy", 1, 20_000),
    ("Medium", 2, 10_000),
    ("Hard", 3, 10_000),
];
pub const YEARS: [u16; 4] = [1900, 1950, 2000, 2050];

impl TitleScreen {
    pub(super) fn refresh_scenarios(&mut self, profile: &AssetProfile) {
        self.scenarios = discover_scenarios(profile);
        self.scenario_index = 0;
        self.scenario_offset = 0;
    }

    pub fn new(profile: &AssetProfile) -> Self {
        let generation_seed = random_generation_seed();
        Self {
            page: Page::Home,
            city_name: String::new(),
            difficulty_index: 0,
            year_index: 0,
            scenario_index: 0,
            scenario_offset: 0,
            scenarios: discover_scenarios(profile),
            generation_seed,
            status: None,
            terrain_preview: generated_terrain_preview(generation_seed),
            buttons: Vec::new(),
        }
    }

    /// City-name box in backing pixels, matching `new_city`'s drawn field.
    pub fn new_city_name_field_px(pixel_size: (u32, u32)) -> (f32, f32, f32, f32) {
        let w = pixel_size.0 as f32;
        let h = pixel_size.1 as f32;
        let s = (h / 760.0).clamp(0.75, 1.6);
        let panel = centered(w, h, 660.0 * s, 420.0 * s);
        (
            panel.x + 32.0 * s,
            panel.y + 127.0 * s,
            (panel.w - 64.0 * s).max(1.0),
            46.0 * s,
        )
    }

    pub fn point_in_new_city_name_field(pixel_size: (u32, u32), x: f32, y: f32) -> bool {
        let (field_x, field_y, field_w, field_h) = Self::new_city_name_field_px(pixel_size);
        x >= field_x && y >= field_y && x < field_x + field_w && y < field_y + field_h
    }

    /// SDL's text-input rectangle is in window points, not backing pixels.
    pub fn text_input_area_points(
        field_px: (f32, f32, f32, f32),
        pixel_size: (u32, u32),
        window_size: (u32, u32),
    ) -> (i32, i32, u32, u32) {
        let sx = if window_size.0 == 0 {
            1.0
        } else {
            pixel_size.0 as f32 / window_size.0 as f32
        };
        let sy = if window_size.1 == 0 {
            1.0
        } else {
            pixel_size.1 as f32 / window_size.1 as f32
        };
        let (x, y, w, h) = field_px;
        (
            (x / sx).round() as i32,
            (y / sy).round() as i32,
            (w / sx).round().max(1.0) as u32,
            (h / sy).round().max(1.0) as u32,
        )
    }

    pub fn set_page(&mut self, page: Page) {
        self.page = page;
        self.status = None;
    }

    pub fn refresh_generation_seed(&mut self) {
        self.generation_seed = random_generation_seed();
        self.terrain_preview = generated_terrain_preview(self.generation_seed);
    }

    pub fn back(&mut self) {
        self.set_page(Page::Home);
    }

    pub fn cycle_difficulty(&mut self) {
        self.difficulty_index = (self.difficulty_index + 1) % DIFFICULTIES.len();
    }

    pub fn append_city_name(&mut self, text: &str) {
        for ch in text.chars() {
            if self.city_name.chars().count() >= 31 {
                break;
            }
            if ch.is_ascii_alphanumeric() || matches!(ch, ' ' | '-' | '\'') {
                self.city_name.push(ch);
            }
        }
    }

    pub fn backspace_city_name(&mut self) {
        self.city_name.pop();
    }

    pub fn hit_test(&self, x: f32, y: f32) -> Option<Choice> {
        self.buttons
            .iter()
            .rev()
            .find(|button| button.rect.contains(x, y))
            .map(|button| button.choice)
    }

    pub fn scroll_scenarios(&mut self, delta: i32) {
        let max = self.scenarios.len().saturating_sub(5);
        self.scenario_offset = (self.scenario_offset as i32 - delta).clamp(0, max as i32) as usize;
    }

    pub fn render(&mut self, size: (u32, u32), time: f32, profile: &AssetProfile) -> Vec<UiSolid> {
        self.buttons.clear();
        let (w, h) = (size.0 as f32, size.1 as f32);
        let scale = (h / 760.0).clamp(0.75, 1.6);
        let mut out = Vec::new();
        backdrop(&mut out, w, h, time, scale);

        match self.page {
            Page::Home => self.home(&mut out, w, h, scale, profile),
            Page::NewCity => self.new_city(&mut out, w, h, scale),
            Page::Scenarios => self.scenarios(&mut out, w, h, scale),
            Page::Graphics => self.graphics(&mut out, w, h, scale, profile),
        }
        if let Some(status) = &self.status {
            let status = clip_label(status, 85);
            text(
                &mut out,
                &status,
                28.0 * scale,
                h - 58.0 * scale,
                2.0 * scale,
                GOLD,
            );
        }
        out
    }

    fn home(&mut self, out: &mut Vec<UiSolid>, w: f32, h: f32, s: f32, profile: &AssetProfile) {
        let panel = Rect {
            x: (w * 0.57).max(w - 500.0 * s),
            y: h * 0.12,
            w: (w * 0.38).min(440.0 * s),
            h: h * 0.75,
        };
        fill(out, panel, PANEL, 0.3);
        outline(out, panel, TEAL, 2.0 * s, 0.4);
        text(
            out,
            "START",
            panel.x + 25.0 * s,
            panel.y + 24.0 * s,
            4.0 * s,
            WHITE,
        );
        let labels = [
            (Choice::Continue, "Continue City"),
            (Choice::NewCity, "Start New City"),
            (Choice::EditMap, "Edit New Map"),
            (Choice::LoadCity, "Load Saved City"),
            (Choice::Scenarios, "Scenarios"),
            (Choice::Graphics, "Asset Library"),
            (Choice::Quit, "Quit"),
        ];
        let gap = (panel.h - 99.0 * s) / labels.len() as f32;
        for (i, (choice, label)) in labels.into_iter().enumerate() {
            let enabled = choice != Choice::Continue || profile.recent_city().is_some();
            self.button(
                out,
                choice,
                label,
                Rect {
                    x: panel.x + 22.0 * s,
                    y: panel.y + 68.0 * s + i as f32 * gap,
                    w: panel.w - 44.0 * s,
                    h: (gap - 8.0 * s).min(50.0 * s),
                },
                enabled,
                s,
            );
        }
        let source = if profile.original_root().is_some() {
            "GRAPHICS READY"
        } else {
            "SET UP GRAPHICS TO PLAY"
        };
        text(
            out,
            source,
            panel.x + 24.0 * s,
            panel.y + panel.h - 22.0 * s,
            2.0 * s,
            MUTED,
        );
        let music_label = match profile.title_music_preference() {
            TitleMusicPreference::Original => "MUSIC: ORIGINAL THEME",
            TitleMusicPreference::Replacement => "MUSIC: CITYGAME THEME",
            TitleMusicPreference::Off => "MUSIC: OFF",
        };
        self.button(
            out,
            Choice::CycleMusic,
            music_label,
            Rect {
                x: 25.0 * s,
                y: h - 69.0 * s,
                w: 245.0 * s,
                h: 31.0 * s,
            },
            true,
            s,
        );
    }

    fn new_city(&mut self, out: &mut Vec<UiSolid>, w: f32, h: f32, s: f32) {
        terrain_backdrop(out, w, h, &self.terrain_preview);
        let panel = centered(w, h, 660.0 * s, 420.0 * s);
        card(out, panel, s);
        text(
            out,
            "NEW CITY",
            panel.x + 30.0 * s,
            panel.y + 28.0 * s,
            5.0 * s,
            WHITE,
        );
        text(
            out,
            "CITY NAME",
            panel.x + 34.0 * s,
            panel.y + 100.0 * s,
            2.4 * s,
            MUTED,
        );
        let (field_x, field_y, field_w, field_h) =
            Self::new_city_name_field_px((w.round() as u32, h.round() as u32));
        let field = Rect {
            x: field_x,
            y: field_y,
            w: field_w,
            h: field_h,
        };
        fill(out, field, [0.035, 0.08, 0.11, 1.0], 0.6);
        outline(out, field, TEAL, 2.0 * s, 0.7);
        let city_name_text = if self.city_name.is_empty() {
            "TYPE CITY NAME"
        } else {
            self.city_name.as_str()
        };
        ui::push_pixel_text_preserving_case(
            out,
            &clip_label(city_name_text, 31),
            field.x + 12.0 * s,
            field.y + 12.0 * s,
            3.0 * s,
            if self.city_name.is_empty() {
                MUTED
            } else {
                WHITE
            },
            0.9,
        );
        text(
            out,
            "TYPE A NAME",
            field.x + 12.0 * s,
            field.y + 51.0 * s,
            1.8 * s,
            MUTED,
        );

        let (difficulty_name, _, starting_funds) = DIFFICULTIES[self.difficulty_index];
        text(
            out,
            "DIFFICULTY",
            panel.x + 38.0 * s,
            panel.y + 202.0 * s,
            1.8 * s,
            MUTED,
        );
        text(
            out,
            "STARTING YEAR",
            panel.x + 346.0 * s,
            panel.y + 202.0 * s,
            1.8 * s,
            MUTED,
        );
        self.button(
            out,
            Choice::CycleDifficulty,
            &difficulty_name.to_ascii_uppercase(),
            Rect {
                x: panel.x + 32.0 * s,
                y: panel.y + 222.0 * s,
                w: 286.0 * s,
                h: 44.0 * s,
            },
            true,
            s,
        );
        self.button(
            out,
            Choice::CycleYear,
            &YEARS[self.year_index].to_string(),
            Rect {
                x: panel.x + 340.0 * s,
                y: panel.y + 222.0 * s,
                w: 286.0 * s,
                h: 44.0 * s,
            },
            true,
            s,
        );
        text(
            out,
            &format!("${},000", starting_funds / 1_000),
            panel.x + 110.0 * s,
            panel.y + 274.0 * s,
            2.0 * s,
            GOLD,
        );
        self.button(
            out,
            Choice::StartGenerated,
            "Generate and Play",
            Rect {
                x: panel.x + 32.0 * s,
                y: panel.y + 316.0 * s,
                w: 286.0 * s,
                h: 50.0 * s,
            },
            true,
            s,
        );
        self.button(
            out,
            Choice::StartEditing,
            "Edit This Map",
            Rect {
                x: panel.x + 340.0 * s,
                y: panel.y + 316.0 * s,
                w: 286.0 * s,
                h: 50.0 * s,
            },
            true,
            s,
        );
        self.button(
            out,
            Choice::Back,
            "Back",
            Rect {
                x: panel.x + panel.w - 116.0 * s,
                y: panel.y + panel.h - 51.0 * s,
                w: 84.0 * s,
                h: 31.0 * s,
            },
            true,
            s,
        );
    }

    fn scenarios(&mut self, out: &mut Vec<UiSolid>, w: f32, h: f32, s: f32) {
        let panel = centered(
            w,
            h,
            (980.0 * s).min(w - 40.0 * s),
            (600.0 * s).min(h - 60.0 * s),
        );
        card(out, panel, s);
        text(
            out,
            "SCENARIOS",
            panel.x + 25.0 * s,
            panel.y + 23.0 * s,
            4.5 * s,
            WHITE,
        );
        let list = Rect {
            x: panel.x + 20.0 * s,
            y: panel.y + 70.0 * s,
            w: panel.w * 0.58,
            h: panel.h - 142.0 * s,
        };
        fill(out, list, [0.04, 0.09, 0.13, 1.0], 0.5);
        if self.scenarios.is_empty() {
            text(
                out,
                "NO IMPORTED SCENARIOS FOUND",
                list.x + 20.0 * s,
                list.y + 30.0 * s,
                2.3 * s,
                MUTED,
            );
        }
        let row_h = list.h / 5.0;
        for row in 0..5 {
            let index = self.scenario_offset + row;
            let Some(entry) = self.scenarios.get(index) else {
                break;
            };
            let rect = Rect {
                x: list.x + 8.0 * s,
                y: list.y + row as f32 * row_h + 4.0 * s,
                w: list.w - 16.0 * s,
                h: row_h - 7.0 * s,
            };
            fill(
                out,
                rect,
                if index == self.scenario_index {
                    [0.11, 0.31, 0.33, 1.0]
                } else {
                    [0.085, 0.17, 0.22, 1.0]
                },
                0.7,
            );
            let thumb = Rect {
                x: rect.x + 6.0 * s,
                y: rect.y + 6.0 * s,
                w: (rect.h - 12.0 * s).max(0.0),
                h: (rect.h - 12.0 * s).max(0.0),
            };
            preview(out, thumb, &entry.preview, 0.8);
            text(
                out,
                &clip_label(&entry.title, 23),
                thumb.x + thumb.w + 12.0 * s,
                rect.y + 11.0 * s,
                2.6 * s,
                WHITE,
            );
            self.buttons.push(Button {
                choice: Choice::SelectScenario(index),
                rect,
            });
        }
        let info = Rect {
            x: list.x + list.w + 13.0 * s,
            y: list.y,
            w: panel.w - list.w - 53.0 * s,
            h: list.h,
        };
        fill(out, info, [0.05, 0.12, 0.16, 1.0], 0.5);
        if let Some(entry) = self.scenarios.get(self.scenario_index) {
            text(
                out,
                &clip_label(&entry.title, 19),
                info.x + 15.0 * s,
                info.y + 15.0 * s,
                3.0 * s,
                GOLD,
            );
            let thumb = Rect {
                x: info.x + 15.0 * s,
                y: info.y + 48.0 * s,
                w: (info.w - 30.0 * s).min(170.0 * s),
                h: 170.0 * s,
            };
            preview(out, thumb, &entry.preview, 0.8);
            wrapped(
                out,
                &entry.brief,
                info.x + 15.0 * s,
                thumb.y + thumb.h + 18.0 * s,
                (info.w / (8.0 * s)) as usize,
                6,
                2.1 * s,
                MUTED,
            );
        }
        let y = panel.y + panel.h - 55.0 * s;
        self.button(
            out,
            Choice::PlayScenario,
            "Play Selected",
            Rect {
                x: panel.x + 20.0 * s,
                y,
                w: 185.0 * s,
                h: 35.0 * s,
            },
            !self.scenarios.is_empty(),
            s,
        );
        self.button(
            out,
            Choice::BrowseScenario,
            "Browse File",
            Rect {
                x: panel.x + 214.0 * s,
                y,
                w: 163.0 * s,
                h: 35.0 * s,
            },
            true,
            s,
        );
        self.button(
            out,
            Choice::Back,
            "Back",
            Rect {
                x: panel.x + panel.w - 120.0 * s,
                y,
                w: 99.0 * s,
                h: 35.0 * s,
            },
            true,
            s,
        );
        if self.scenario_offset > 0 {
            text(
                out,
                "UP",
                list.x + list.w - 42.0 * s,
                list.y + 4.0 * s,
                2.0 * s,
                GOLD,
            );
        }
        if self.scenario_offset + 5 < self.scenarios.len() {
            text(
                out,
                "SCROLL FOR MORE",
                list.x + 8.0 * s,
                list.y + list.h - 19.0 * s,
                1.7 * s,
                GOLD,
            );
        }
    }

    fn graphics(&mut self, out: &mut Vec<UiSolid>, w: f32, h: f32, s: f32, profile: &AssetProfile) {
        let panel = centered(w, h, 760.0 * s, 660.0 * s);
        card(out, panel, s);
        text(
            out,
            "ASSET LIBRARY",
            panel.x + 26.0 * s,
            panel.y + 22.0 * s,
            4.0 * s,
            WHITE,
        );
        for (index, media) in [
            OriginalMedia::Graphics,
            OriginalMedia::Effects,
            OriginalMedia::Music,
        ]
        .into_iter()
        .enumerate()
        {
            let kind = match media {
                OriginalMedia::Graphics => "GRAPHICS",
                OriginalMedia::Effects => "SOUNDS",
                OriginalMedia::Music => "MUSIC",
            };
            let source = profile
                .media_source(media)
                .and_then(|path| {
                    path.file_name()
                        .map(|name| name.to_string_lossy().into_owned())
                })
                .unwrap_or_else(|| "NONE".into());
            let label = format!("{kind}: {}", clip_label(&source, 52));
            text(
                out,
                &label,
                panel.x + 28.0 * s,
                panel.y + (70.0 + 22.0 * index as f32) * s,
                2.0 * s,
                MUTED,
            );
        }
        let choices = [
            (Choice::ImportOriginal, "Import Game Edition"),
            (Choice::ChooseOriginal, "Choose Extracted Graphics Pack"),
            (Choice::ChooseReplacement, "Choose CityGame Override Pack"),
            (Choice::CycleGraphicsSource, "Next Graphics Edition"),
            (Choice::CycleEffectsSource, "Next Sound Edition"),
            (Choice::CycleMusicSource, "Next Music Edition"),
            (
                if profile.preference().prefers_replacement() {
                    Choice::PreferOriginal
                } else {
                    Choice::PreferOurs
                },
                if profile.preference().prefers_replacement() {
                    "High-Res CityGame Graphics: ON"
                } else {
                    "High-Res CityGame Graphics: OFF"
                },
            ),
            (
                Choice::ToggleCityGameEffects,
                if profile.citygame_effects_enabled() {
                    "CityGame Sounds: ON"
                } else {
                    "CityGame Sounds: OFF"
                },
            ),
            (
                Choice::ToggleCityGameMusic,
                if profile.title_music_preference() == TitleMusicPreference::Replacement {
                    "CityGame Title Music: ON"
                } else {
                    "CityGame Title Music: OFF"
                },
            ),
            (
                Choice::ToggleCityGameCityMusic,
                if profile.citygame_city_music_enabled() {
                    "CityGame City Music: ON"
                } else {
                    "CityGame City Music: OFF"
                },
            ),
            (
                Choice::ToggleToolbarPalette,
                if profile.toolbar_palette_preference().uses_citygame_palette() {
                    "Toolbar Palette: CITYGAME"
                } else {
                    "Toolbar Palette: CLASSIC GRAY"
                },
            ),
            (Choice::ChooseTileSet, "Choose Alternate Tile Set"),
            (Choice::ClearTileSet, "Use Default Tile Set"),
        ];
        for (i, (choice, label)) in choices.into_iter().enumerate() {
            self.button(
                out,
                choice,
                label,
                Rect {
                    x: panel.x + 28.0 * s,
                    y: panel.y + 137.0 * s + i as f32 * 35.0 * s,
                    w: panel.w - 56.0 * s,
                    h: 31.0 * s,
                },
                true,
                s,
            );
        }
        self.button(
            out,
            Choice::Back,
            "Back",
            Rect {
                x: panel.x + panel.w - 121.0 * s,
                y: panel.y + panel.h - 53.0 * s,
                w: 93.0 * s,
                h: 33.0 * s,
            },
            true,
            s,
        );
    }

    fn button(
        &mut self,
        out: &mut Vec<UiSolid>,
        choice: Choice,
        label: &str,
        rect: Rect,
        enabled: bool,
        s: f32,
    ) {
        fill(
            out,
            rect,
            if enabled {
                [0.10, 0.25, 0.28, 1.0]
            } else {
                [0.09, 0.15, 0.18, 1.0]
            },
            0.6,
        );
        outline(out, rect, if enabled { TEAL } else { MUTED }, 1.5 * s, 0.7);
        let text_scale =
            (2.7 * s).min((rect.w - 18.0 * s) / ui::pixel_text_width(label, 1.0).max(1.0));
        text(
            out,
            label,
            rect.x + 11.0 * s,
            rect.y + (rect.h - 5.0 * text_scale) * 0.5,
            text_scale,
            if enabled { WHITE } else { MUTED },
        );
        if enabled {
            self.buttons.push(Button { choice, rect });
        }
    }
}

fn scenario_paths(roots: impl IntoIterator<Item = PathBuf>) -> BTreeSet<PathBuf> {
    let mut paths = BTreeSet::new();
    let include_mac = !crate::engine_fidelity::EngineFidelity::from_env().uses_win95();
    for root in roots {
        for directory in [
            root.join("catalog/scenarios"),
            root.join("SCENARIO"),
            root.join("scenarios"),
            root.clone(),
        ] {
            if let Ok(entries) = std::fs::read_dir(directory) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path
                        .extension()
                        .is_some_and(|ext| ext.eq_ignore_ascii_case("scn"))
                        || (include_mac && crate::mac_scenario::MacScenario::read(&path).is_some())
                    {
                        paths.insert(path);
                    }
                }
            }
        }
    }
    paths
}

fn scenario_palette(palette: &AnimatedPalette) -> [[f32; 4]; 256] {
    std::array::from_fn(|index| {
        let color = palette.color_for_index(index as u8, 0);
        [
            color.r as f32 / 255.0,
            color.g as f32 / 255.0,
            color.b as f32 / 255.0,
            color.a as f32 / 255.0,
        ]
    })
}

fn scenario_picture_preview(data: &[u8], palette: &[[f32; 4]; 256]) -> Option<Vec<[f32; 4]>> {
    if data.get(0..4)? != b"FORM" {
        return None;
    }
    let form_size = u32::from_be_bytes(data.get(4..8)?.try_into().ok()?) as usize;
    let form_end = 8usize.checked_add(form_size)?;
    if form_end > data.len() || form_end < 12 {
        return None;
    }
    let mut offset = 12usize;
    while offset.checked_add(8)? <= form_end {
        let tag = data.get(offset..offset + 4)?;
        let size = u32::from_be_bytes(data.get(offset + 4..offset + 8)?.try_into().ok()?) as usize;
        let start = offset + 8;
        let end = start.checked_add(size)?;
        let chunk = data.get(start..end)?;
        if tag == b"PICT" {
            // Win95 SCN PICT payload: 8-byte header, indexed rows with one
            // sentinel byte after each row. Width/height are little-endian.
            if chunk.len() < 10 || chunk[0] != 0x80 {
                return None;
            }
            let width = u16::from_le_bytes([chunk[4], chunk[5]]) as usize;
            let height = u16::from_le_bytes([chunk[6], chunk[7]]) as usize;
            if width == 0 || height == 0 || width > 256 || height > 256 {
                return None;
            }
            let stride = width.checked_add(1)?;
            if chunk.len() != 8usize.checked_add(stride.checked_mul(height)?)? {
                return None;
            }
            let mut colors = Vec::with_capacity(PREVIEW_SIZE * PREVIEW_SIZE);
            for y in 0..PREVIEW_SIZE {
                let sy = y * height / PREVIEW_SIZE;
                for x in 0..PREVIEW_SIZE {
                    let sx = x * width / PREVIEW_SIZE;
                    let index = chunk[8 + sy * stride + sx] as usize;
                    colors.push(palette[index]);
                }
            }
            return Some(colors);
        }
        offset = end;
    }
    None
}

fn generated_terrain_preview(seed: u32) -> Vec<[f32; 4]> {
    let sliders = ui::EditMapSliderValues::default();
    let mut game_rng =
        crate::mac_random::MacGameLcgRng::new(crate::mac_random::MacGameLcgRng::INITIAL_STATE);
    let (terrain, altm, tile_bits, buildings, _selected_water_height) =
        terrain_generation::generate(seed, true, true, sliders, &mut game_rng);
    let mut colors = Vec::with_capacity(TERRAIN_PREVIEW_SIZE * TERRAIN_PREVIEW_SIZE);
    for y in 0..TERRAIN_PREVIEW_SIZE {
        for x in 0..TERRAIN_PREVIEW_SIZE {
            let tx = x * crate::city::CITY_WIDTH / TERRAIN_PREVIEW_SIZE;
            let ty = y * crate::city::CITY_HEIGHT / TERRAIN_PREVIEW_SIZE;
            let index = tx * crate::city::CITY_HEIGHT + ty;
            let tile = terrain[index];
            let sample = altm[index];
            let is_water = tile_bits[index] & 0x04 != 0;
            let height = f32::from(
                sample
                    .land
                    .saturating_sub(crate::city::BLANK_MAP_BASE_HEIGHT),
            );
            let color = if is_water {
                [0.08, 0.28 + height * 0.015, 0.78, 1.0]
            } else if (0x06..=0x0c).contains(&buildings[index]) {
                [0.12, 0.42 + height * 0.012, 0.16, 1.0]
            } else if (0x06..=0x0c).contains(&tile) {
                [0.19, 0.38 + height * 0.012, 0.22, 1.0]
            } else {
                let shade = (0.55 + height * 0.035).clamp(0.45, 0.82);
                [shade, shade * 0.78, shade * 0.49, 1.0]
            };
            colors.push(color);
        }
    }
    colors
}

fn random_generation_seed() -> u32 {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    (now.as_nanos() as u32) ^ ((now.as_secs() >> 16) as u32)
}

fn terrain_backdrop(out: &mut Vec<UiSolid>, w: f32, h: f32, colors: &[[f32; 4]]) {
    fill(
        out,
        Rect {
            x: 0.0,
            y: 0.0,
            w,
            h,
        },
        [0.23, 0.12, 0.065, 1.0],
        0.0,
    );
    if colors.len() != TERRAIN_PREVIEW_SIZE * TERRAIN_PREVIEW_SIZE {
        return;
    }
    let half_w = w * 0.79 / (TERRAIN_PREVIEW_SIZE as f32 * 2.0);
    let half_h = h * 0.62 / (TERRAIN_PREVIEW_SIZE as f32 * 2.0);
    let center_x = w * 0.41;
    let center_y = h * 0.53;
    for diagonal in 0..(TERRAIN_PREVIEW_SIZE * 2 - 1) {
        let first_x = diagonal.saturating_sub(TERRAIN_PREVIEW_SIZE - 1);
        let last_x = diagonal.min(TERRAIN_PREVIEW_SIZE - 1);
        for x in first_x..=last_x {
            let y = diagonal - x;
            let color = colors[y * TERRAIN_PREVIEW_SIZE + x];
            let px = center_x + (x as f32 - y as f32) * half_w;
            let py = center_y + (x as f32 + y as f32 - TERRAIN_PREVIEW_SIZE as f32) * half_h;
            fill(
                out,
                Rect {
                    x: px - half_w,
                    y: py - half_h,
                    w: half_w * 2.0 + 0.35,
                    h: half_h * 2.0 + 0.35,
                },
                color,
                0.08,
            );
        }
    }
}

fn discover_scenarios(profile: &AssetProfile) -> Vec<ScenarioEntry> {
    let mut roots = BTreeSet::new();
    if let Some(root) = profile.original_root() {
        roots.insert(root);
    }
    for pack in profile.imported_packs() {
        roots.insert(pack.root.clone());
    }
    roots.insert(crate::paths::default_open_dialog_dir());
    let palettes: Vec<_> = roots
        .iter()
        .filter_map(|root| {
            AnimatedPalette::load(root)
                .ok()
                .map(|palette| (root.clone(), scenario_palette(&palette)))
        })
        .collect();
    scenario_paths(roots)
        .into_iter()
        .filter_map(|path| {
            let city = City::load(&path).ok()?;
            let mac = (city.kind == crate::city::CityFileKind::MacCity)
                .then(|| crate::mac_scenario::MacScenario::read(&path))
                .flatten();
            let title = city.name.trim().to_string();
            let title = if title.is_empty() {
                path.file_stem()?.to_string_lossy().into_owned()
            } else {
                title
            };
            let brief = mac
                .as_ref()
                .and_then(|scenario| scenario.text.clone())
                .or_else(|| scenario_text_for_city(&city).selection)
                .unwrap_or_else(|| "Open this scenario to read its goals and briefing.".into());
            let palette = palettes
                .iter()
                .find(|(root, _)| path.starts_with(root))
                .or_else(|| palettes.first())
                .map(|(_, palette)| palette);
            let preview = std::fs::read(&path)
                .ok()
                .and_then(|data| scenario_picture_preview(&data, palette?))
                .unwrap_or_else(|| city_map_preview(&city));
            Some(ScenarioEntry {
                path,
                title,
                brief,
                mac_action: mac.map(|scenario| scenario.start.action),
                preview,
            })
        })
        .collect()
}

fn city_map_preview(city: &City) -> Vec<[f32; 4]> {
    let mut colors = Vec::with_capacity(PREVIEW_SIZE * PREVIEW_SIZE);
    for y in 0..PREVIEW_SIZE {
        for x in 0..PREVIEW_SIZE {
            let tx = x * crate::city::CITY_WIDTH / PREVIEW_SIZE;
            let ty = y * crate::city::CITY_HEIGHT / PREVIEW_SIZE;
            let color = if city.building_at(tx, ty) != 0 {
                [0.83, 0.72, 0.53, 1.0]
            } else if (0x10..=0x1f).contains(&city.terrain_at(tx, ty)) {
                [0.12, 0.48, 0.73, 1.0]
            } else {
                [0.21, 0.47, 0.32, 1.0]
            };
            colors.push(color);
        }
    }
    colors
}

fn backdrop(out: &mut Vec<UiSolid>, w: f32, h: f32, time: f32, s: f32) {
    fill(
        out,
        Rect {
            x: 0.0,
            y: 0.0,
            w,
            h,
        },
        NAVY,
        0.0,
    );
    for i in 0..28 {
        let y = i as f32 * h / 28.0;
        fill(
            out,
            Rect {
                x: 0.0,
                y,
                w,
                h: h / 29.0,
            },
            [
                0.035 + i as f32 * 0.001,
                0.075 + i as f32 * 0.002,
                0.12 + i as f32 * 0.003,
                1.0,
            ],
            0.01,
        );
    }
    for i in 0..55 {
        let x = i as f32 * w / 55.0;
        let height = (h * (0.13 + ((i * 37 % 17) as f32) / 80.0)).min(h * 0.38);
        fill(
            out,
            Rect {
                x,
                y: h - height,
                w: w / 57.0 + 1.0,
                h: height,
            },
            [0.055, 0.14, 0.18, 1.0],
            0.08,
        );
        if i % 3 == 0 {
            for row in 0..3 {
                fill(
                    out,
                    Rect {
                        x: x + 5.0 * s,
                        y: h - height + 13.0 * s + row as f32 * 19.0 * s,
                        w: 3.0 * s,
                        h: 5.0 * s,
                    },
                    GOLD,
                    0.09,
                );
            }
        }
    }
    let logo_scale = (h / 90.0).clamp(6.0, 12.0);
    text(out, "CITYGAME", 45.0 * s, h * 0.19, logo_scale, WHITE);
    text(
        out,
        "BUILD A CITY. SHAPE ITS FUTURE.",
        52.0 * s,
        h * 0.19 + 68.0 * s,
        2.5 * s,
        MUTED,
    );
    let center_x = w * 0.29;
    let center_y = h * 0.55;
    for i in 0..20 {
        let a = (i as f32 / 20.0 * std::f32::consts::TAU) + time * 0.55;
        let x = center_x + a.cos() * 135.0 * s;
        let y = center_y + a.sin() * 80.0 * s;
        let color = if i % 4 == 0 { GOLD } else { TEAL };
        fill(
            out,
            Rect {
                x,
                y,
                w: 7.0 * s,
                h: 7.0 * s,
            },
            color,
            0.12,
        );
    }
    for i in 0..6 {
        let x = center_x - 103.0 * s + i as f32 * 39.0 * s;
        let height = (38 + (i * 19 % 60)) as f32 * s;
        fill(
            out,
            Rect {
                x,
                y: center_y + 54.0 * s - height,
                w: 31.0 * s,
                h: height,
            },
            [0.16, 0.38, 0.42, 1.0],
            0.13,
        );
        for row in 0..3 {
            fill(
                out,
                Rect {
                    x: x + 7.0 * s,
                    y: center_y + 48.0 * s - height + row as f32 * 16.0 * s,
                    w: 5.0 * s,
                    h: 5.0 * s,
                },
                GOLD,
                0.14,
            );
        }
    }
    text(out, "CITYGAME", 25.0 * s, h - 32.0 * s, 2.0 * s, MUTED);
}

fn card(out: &mut Vec<UiSolid>, rect: Rect, s: f32) {
    fill(out, rect, PANEL, 0.3);
    outline(out, rect, TEAL, 2.0 * s, 0.4);
}
fn centered(w: f32, h: f32, rw: f32, rh: f32) -> Rect {
    Rect {
        x: (w - rw) * 0.5,
        y: (h - rh) * 0.5,
        w: rw,
        h: rh,
    }
}
fn fill(out: &mut Vec<UiSolid>, rect: Rect, color: [f32; 4], depth: f32) {
    out.push(UiSolid {
        x: rect.x,
        y: rect.y,
        w: rect.w,
        h: rect.h,
        color,
        depth,
    });
}
fn outline(out: &mut Vec<UiSolid>, r: Rect, color: [f32; 4], t: f32, depth: f32) {
    fill(
        out,
        Rect {
            x: r.x,
            y: r.y,
            w: r.w,
            h: t,
        },
        color,
        depth,
    );
    fill(
        out,
        Rect {
            x: r.x,
            y: r.y + r.h - t,
            w: r.w,
            h: t,
        },
        color,
        depth,
    );
    fill(
        out,
        Rect {
            x: r.x,
            y: r.y,
            w: t,
            h: r.h,
        },
        color,
        depth,
    );
    fill(
        out,
        Rect {
            x: r.x + r.w - t,
            y: r.y,
            w: t,
            h: r.h,
        },
        color,
        depth,
    );
}
fn text(out: &mut Vec<UiSolid>, value: &str, x: f32, y: f32, scale: f32, color: [f32; 4]) {
    ui::push_pixel_text(out, value, x, y, scale, color, 0.9);
}
fn clip_label(value: &str, max: usize) -> String {
    value.chars().take(max).collect()
}
fn wrapped(
    out: &mut Vec<UiSolid>,
    value: &str,
    x: f32,
    y: f32,
    width: usize,
    lines: usize,
    scale: f32,
    color: [f32; 4],
) {
    let mut line = String::new();
    let mut row = 0;
    for word in value.split_whitespace() {
        if line.len() + word.len() + 1 > width.max(1) {
            text(
                out,
                &line,
                x,
                y + row as f32 * 15.0 * scale / 2.0,
                scale,
                color,
            );
            row += 1;
            if row >= lines {
                return;
            }
            line.clear();
        }
        if !line.is_empty() {
            line.push(' ');
        }
        line.push_str(word);
    }
    if row < lines {
        text(
            out,
            &line,
            x,
            y + row as f32 * 15.0 * scale / 2.0,
            scale,
            color,
        );
    }
}
fn preview(out: &mut Vec<UiSolid>, rect: Rect, colors: &[[f32; 4]], depth: f32) {
    if colors.len() != PREVIEW_SIZE * PREVIEW_SIZE {
        return;
    }
    let size = PREVIEW_SIZE as f32;
    for y in 0..PREVIEW_SIZE {
        for x in 0..PREVIEW_SIZE {
            fill(
                out,
                Rect {
                    x: rect.x + x as f32 * rect.w / size,
                    y: rect.y + y as f32 * rect.h / size,
                    w: rect.w / size + 0.2,
                    h: rect.h / size + 0.2,
                },
                colors[y * PREVIEW_SIZE + x],
                depth,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    #[ignore = "requires locally extracted Macintosh Scenario Disk data and forks"]
    fn mac_scenario_retail_catalog_and_fresh_start() {
        use super::*;
        use crate::app::tests::test_state;
        use crate::engine_fidelity::EngineFidelity;
        use crate::mac_scenario::MacScenario;
        let root = PathBuf::from(std::env::var_os("CITYGAME_MAC_SCENARIO_FIXTURES").unwrap());
        let paths = scenario_paths([root.clone()]);
        assert_eq!(paths.len(), 5);
        for (name, action, point, months, population) in [
            ("Charleston", 16, (68, 99), 60, 45_000),
            ("Dullsville", 0, (0, 0), 121, 20_000),
            ("Flint", 0, (0, 0), 60, 0),
            ("Hollywood", 8, (80, 52), 60, 100_000),
            ("Oakland", 12, (36, 48), 60, 50_000),
        ] {
            let path = root.join(name);
            assert!(paths.contains(&path));
            let scenario = MacScenario::read(&path).unwrap();
            assert_eq!(scenario.start.action, action);
            assert_eq!(scenario.start.point, point);
            assert_eq!(&scenario.start.goals[..2], &[months, population]);
            assert!(scenario.text.unwrap().contains(name));

            let saved_city = City::load(&path).unwrap();
            let mut state = test_state(saved_city);
            state.engine_fidelity = EngineFidelity::Mac68k;
            if crate::app::mac_loop_scheduler::scenario_action::mac_scenario_action_supported(
                action,
            ) {
                state.load_mac_scenario(&path, (1280, 800), 1.0).unwrap();
                assert_eq!(state.city.misc_u32_at(0x106c), Some(months));
                assert_eq!(state.city.misc_u32_at(0x1068).unwrap() as u8, 0xff);
                assert_ne!(state.city.path, path);
                assert_eq!(state.city.mac_pending_action(), u32::from(action));
                if action != 0 {
                    state.traffic.set_speed(crate::traffic::GameSpeed::Paused);
                    state.mac_process_rng = crate::mac_random::MacToolboxRng::new(1);
                    assert_eq!(
                        state
                            .advance_mac_frame_simulation(std::time::Duration::ZERO)
                            .days,
                        0
                    );
                    assert!(
                        crate::app::mac_loop_scheduler::scenario_action::mac_scenario_action_active(
                            &state.city
                        )
                    );
                }
                let save_path = state.city.path.clone();
                state.save_city(&save_path, (1280, 800), 1.0).unwrap();
                let reloaded = City::load(&save_path).unwrap();
                if action == crate::app::mac_loop_scheduler::scenario_action::HOLLYWOOD_ACTION {
                    let opening = [5, 2, 0, 80, 52, 15, 8, 8, 0x27, 0x71, 0, 0];
                    assert_eq!(reloaded.traffic_overlays().unwrap()[1].to_bytes(), opening);
                    assert_eq!(reloaded.text_overlay_at(80, 52), Some(0xca));
                    assert_eq!(state.mac_process_rng.seed(), 282_475_249);
                    state.load_city(&save_path, (1280, 800), 1.0).unwrap();
                    assert_eq!(state.city.misc_u32_at(0x04), Some(2));
                    let clock = state.game_clock;
                    state.traffic.set_speed(crate::traffic::GameSpeed::Cheetah);
                    let advance =
                        state.advance_mac_frame_simulation(std::time::Duration::from_secs(1));
                    assert_eq!(advance.days, 0);
                    assert_eq!(state.game_clock, clock);
                    assert_eq!(
                        state.city.traffic_overlays().unwrap()[1].to_bytes(),
                        [5, 7, 0, 80, 52, 14, 0, 0, 0, 0, 0, 0],
                    );
                    assert_eq!(state.mac_process_rng.seed(), 282_475_249);
                    let mut monster = state.city.traffic_overlays().unwrap()[1];
                    monster.state = 3;
                    state.city.set_mac_object_slot_record(1, monster);
                    let frame = std::time::Duration::from_secs(1);
                    assert_eq!(state.advance_mac_frame_simulation(frame).days, 0);
                    assert_eq!(state.city.traffic_overlays().unwrap()[1].kind, 0);
                    assert_eq!(state.city.text_overlay_at(80, 52), Some(0));
                    assert_eq!(state.city.misc_u32_at(0x04), Some(1));
                    assert_eq!(state.advance_mac_frame_simulation(frame).days, 1);
                } else if action
                    == crate::app::mac_loop_scheduler::scenario_action::HURRICANE_ACTION
                {
                    assert_eq!(reloaded.misc_u32_at(0x10b0), Some(49));
                    assert_eq!(reloaded.misc_u32_at(0x10b4), Some(59));
                    assert_eq!(
                        reloaded.traffic_density_slice(),
                        state.city.traffic_density_slice()
                    );
                    state.load_city(&save_path, (1280, 800), 1.0).unwrap();
                    assert_eq!(state.city.misc_u32_at(0x04), Some(2));
                    let clock = state.game_clock;
                    state.traffic.set_speed(crate::traffic::GameSpeed::Cheetah);
                    let frame = std::time::Duration::from_secs(1);
                    assert_eq!(state.advance_mac_frame_simulation(frame).days, 0);
                    assert_eq!(state.game_clock, clock);
                    assert_eq!(state.city.misc_u32_at(0x10b0), Some(48));
                    assert_eq!(state.city.misc_u32_at(0x10b4), Some(58));
                    for _ in 0..240 {
                        if state.city.misc_u32_at(0x04) == Some(1) {
                            break;
                        }
                        assert_eq!(state.advance_mac_frame_simulation(frame).days, 0);
                    }
                    assert_eq!(state.city.misc_u32_at(0x04), Some(1));
                    assert_eq!(state.advance_mac_frame_simulation(frame).days, 1);
                }
                std::fs::remove_file(&save_path).unwrap();
                assert_eq!(reloaded.misc_u32_at(0x106c), Some(months));
                assert!(reloaded.mac_scenario_active());
                assert_ne!(City::load(&path).unwrap().misc_u32_at(0x106c), Some(months));
            } else {
                assert!(state.load_mac_scenario(&path, (1280, 800), 1.0).is_err());
                assert_eq!(state.city.path, path);
            }
        }
    }

    use super::*;

    #[test]
    fn new_city_name_field_maps_backing_pixels_to_window_points() {
        let field = TitleScreen::new_city_name_field_px((2224, 1668));
        let (x, y, w, h) = field;
        assert!(x >= 0.0 && y >= 0.0 && w > 40.0 && h > 20.0);
        assert!(x + w <= 2224.0);
        assert!(y + h <= 1668.0);
        assert_eq!(
            TitleScreen::text_input_area_points(field, (2224, 1668), (1112, 834)),
            (
                (x / 2.0).round() as i32,
                (y / 2.0).round() as i32,
                (w / 2.0).round() as u32,
                (h / 2.0).round() as u32,
            )
        );
    }

    #[test]
    fn city_name_input_and_display_preserve_typed_capitalization() {
        let path =
            std::env::temp_dir().join(format!("citygame-title-case-{}.json", std::process::id()));
        let profile = AssetProfile::load_from(path).unwrap();
        let mut title = TitleScreen::new(&profile);
        title.set_page(Page::NewCity);
        title.append_city_name("New ");
        title.append_city_name("McKinley's Bay");
        assert_eq!(title.city_name, "New McKinley's Bay");
        let mixed: Vec<_> = title
            .render((1280, 800), 0.0, &profile)
            .iter()
            .map(|s| (s.x, s.y, s.w, s.h))
            .collect();
        title.city_name.make_ascii_uppercase();
        let upper: Vec<_> = title
            .render((1280, 800), 0.0, &profile)
            .iter()
            .map(|s| (s.x, s.y, s.w, s.h))
            .collect();
        assert_ne!(mixed, upper);
    }

    #[test]
    fn home_buttons_route_to_distinct_start_flows_without_original_art() {
        let path = std::env::temp_dir().join(format!("citygame-title-{}.json", std::process::id()));
        let profile = AssetProfile::load_from(path).unwrap();
        let mut title = TitleScreen::new(&profile);
        title.render((1280, 800), 0.0, &profile);
        assert!(
            !title
                .buttons
                .iter()
                .any(|button| button.choice == Choice::Continue)
        );
        for choice in [
            Choice::NewCity,
            Choice::EditMap,
            Choice::LoadCity,
            Choice::Scenarios,
            Choice::Graphics,
            Choice::Quit,
        ] {
            let rect = title
                .buttons
                .iter()
                .find(|button| button.choice == choice)
                .unwrap()
                .rect;
            assert_eq!(
                title.hit_test(rect.x + rect.w / 2.0, rect.y + rect.h / 2.0),
                Some(choice)
            );
        }
    }

    #[test]
    fn new_city_choices_use_the_generated_terrain_for_play_or_edit() {
        assert_eq!(
            Choice::StartGenerated.new_city_start_flow(),
            Some(NewCityStartFlow::GenerateAndPlay)
        );
        assert_eq!(
            Choice::StartEditing.new_city_start_flow(),
            Some(NewCityStartFlow::GenerateAndEdit)
        );
        assert_eq!(Choice::NewCity.new_city_start_flow(), None);
    }

    #[test]
    fn new_city_page_exposes_pe_difficulty_presets() {
        let path = std::env::temp_dir().join(format!(
            "citygame-title-difficulty-{}.json",
            std::process::id()
        ));
        let profile = AssetProfile::load_from(path).unwrap();
        let mut title = TitleScreen::new(&profile);
        title.set_page(Page::NewCity);
        let expected_labels = [
            "Difficulty: Easy ($20000)",
            "Difficulty: Medium ($10000)",
            "Difficulty: Hard ($10000)",
        ];
        for (index, expected_label) in expected_labels.into_iter().enumerate() {
            title.difficulty_index = index;
            title.render((1280, 800), 0.0, &profile);
            let button = title
                .buttons
                .iter()
                .find(|button| button.choice == Choice::CycleDifficulty)
                .unwrap();
            assert!(button.rect.w > 0.0);
            let (name, _, funds) = DIFFICULTIES[index];
            assert_eq!(format!("Difficulty: {name} (${funds})"), expected_label);
        }
        title.difficulty_index = 0;
        title.cycle_difficulty();
        assert_eq!(title.difficulty_index, 1);
        title.cycle_difficulty();
        assert_eq!(title.difficulty_index, 2);
        title.cycle_difficulty();
        assert_eq!(title.difficulty_index, 0);
    }

    #[test]
    fn scenario_list_selection_and_scroll_stay_in_bounds() {
        let path = std::env::temp_dir().join(format!(
            "citygame-title-scenarios-{}.json",
            std::process::id()
        ));
        let profile = AssetProfile::load_from(path).unwrap();
        let mut title = TitleScreen::new(&profile);
        title.scenarios = (0..9)
            .map(|number| ScenarioEntry {
                path: PathBuf::from(format!("scenario-{number}.scn")),
                title: format!("Scenario {number}"),
                brief: "A city challenge".to_string(),
                mac_action: None,
                preview: vec![[0.2, 0.5, 0.3, 1.0]; 24 * 24],
            })
            .collect();
        title.set_page(Page::Scenarios);
        title.scroll_scenarios(-100);
        assert_eq!(title.scenario_offset, 4);
        title.render((1280, 800), 0.0, &profile);
        let rect = title
            .buttons
            .iter()
            .find(|button| button.choice == Choice::SelectScenario(4))
            .unwrap()
            .rect;
        assert_eq!(
            title.hit_test(rect.x + 5.0, rect.y + 5.0),
            Some(Choice::SelectScenario(4))
        );
    }

    #[test]
    fn scenario_paths_include_lowercase_scenarios_directory() {
        let root = std::env::temp_dir().join(format!(
            "citygame-title-scenario-discovery-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        let scenario_dir = root.join("scenarios");
        std::fs::create_dir_all(&scenario_dir).expect("create scenario directory");
        let scenario_path = scenario_dir.join("DULLSVILLE.SCN");
        std::fs::write(&scenario_path, b"scenario fixture").expect("write scenario fixture");

        let paths = scenario_paths([root.clone()]);
        assert!(
            paths.contains(&scenario_path),
            "scenario enumeration should include the lowercase scenarios folder"
        );

        std::fs::remove_dir_all(root).expect("remove scenario fixture");
    }
}
