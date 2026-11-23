//! Win95 charter shown after finishing a newly edited map.
use super::session::{
    apply_new_city_title_selections, initialize_pe_charter_national_values, new_city_path_for_name,
    normalize_new_city_name,
};
use super::*;

pub(super) struct Setup {
    pub(super) name: String,
    pub(super) selected: bool,
    pub(super) difficulty: usize,
    pub(super) year: usize,
}

impl Default for Setup {
    fn default() -> Self {
        Self {
            name: "New City".into(),
            selected: true,
            difficulty: 0,
            year: 0,
        }
    }
}

fn rect(size: (u32, u32)) -> FloatRect {
    FloatRect {
        x: (size.0 as f32 - 480.0) * 0.5,
        y: (size.1 as f32 - 260.0) * 0.5,
        w: 480.0,
        h: 260.0,
    }
}
fn difficulty_rect(r: FloatRect, i: usize) -> FloatRect {
    FloatRect {
        x: r.x + 22.0,
        y: r.y + 92.0 + i as f32 * 31.0,
        w: 205.0,
        h: 27.0,
    }
}
fn year_rect(r: FloatRect, i: usize) -> FloatRect {
    FloatRect {
        x: r.x + 258.0,
        y: r.y + 92.0 + i as f32 * 31.0,
        w: 195.0,
        h: 27.0,
    }
}
fn done_rect(r: FloatRect) -> FloatRect {
    FloatRect {
        x: r.x + 190.0,
        y: r.y + 220.0,
        w: 100.0,
        h: 30.0,
    }
}

impl ViewerState {
    pub(super) fn begin_win95_new_city_setup(&mut self) {
        self.prepare_founding_edit_map_exit();
        self.win95_new_city_setup = Some(Setup::default());
    }

    pub(super) fn commit_win95_new_city_setup(&mut self) {
        let Some(setup) = self.win95_new_city_setup.take() else {
            return;
        };
        let (_, difficulty, funds) = title_screen::DIFFICULTIES[setup.difficulty];
        let year = title_screen::YEARS[setup.year];
        self.city.name = normalize_new_city_name(&setup.name, true);
        self.city.path = new_city_path_for_name(&self.city.path, &self.city.name);
        apply_new_city_title_selections(&mut self.city, Some(funds), Some(year), Some(difficulty));
        self.game_clock = game_clock_from_city(&self.city);
        initialize_pe_charter_national_values(&mut self.city);
        session::initialize_pe_new_city_national_graph(&mut self.city);
        self.city
            .pe_reseed_neighbor_fame_ranks_42e585(&mut self.pe_process_crt);
        initialize_hard_start_bond(&mut self.city, self.engine_fidelity);
    }

    pub(super) fn win95_setup_pointer(&mut self, point: Vec2, size: (u32, u32)) -> bool {
        let r = rect(size);
        let Some(setup) = self.win95_new_city_setup.as_mut() else {
            return false;
        };
        for i in 0..3 {
            if point_in_float_rect(point, difficulty_rect(r, i)) {
                setup.difficulty = i;
                return false;
            }
        }
        for i in 0..4 {
            if point_in_float_rect(point, year_rect(r, i)) {
                setup.year = i;
                return false;
            }
        }
        if point_in_float_rect(point, done_rect(r)) {
            self.commit_win95_new_city_setup();
            return true;
        }
        false
    }

    pub(super) fn push_win95_new_city_setup(&self, size: (u32, u32), solids: &mut Vec<UiSolid>) {
        let Some(setup) = &self.win95_new_city_setup else {
            return;
        };
        let r = rect(size);
        let depth = ui::UI_DEPTH + 250.0;
        push_ui_rect(solids, r, [0.66, 0.66, 0.66, 1.0], depth);
        push_ui_bevel(solids, r, 3.0, false, depth + 0.1);
        let field = FloatRect {
            x: r.x + 22.0,
            y: r.y + 22.0,
            w: 436.0,
            h: 36.0,
        };
        push_ui_rect(solids, field, [0.95, 0.95, 0.95, 1.0], depth + 0.2);
        push_ui_bevel(solids, field, 2.0, true, depth + 0.3);
        ui::push_pixel_text_preserving_case(
            solids,
            &format!("{}_", setup.name),
            field.x + 8.0,
            field.y + 10.0,
            2.0,
            [0.0, 0.0, 0.0, 1.0],
            depth + 0.4,
        );
        for (i, (label, _, _)) in title_screen::DIFFICULTIES.iter().enumerate() {
            let text = match *label {
                "Easy" => "Easy  $20,000",
                "Medium" => "Medium  $10,000",
                _ => "Hard  $10K Bond",
            };
            dialog_content::push_dialog_button(
                solids,
                difficulty_rect(r, i),
                text,
                setup.difficulty == i,
                depth + 0.4,
            );
        }
        for (i, year) in title_screen::YEARS.iter().enumerate() {
            dialog_content::push_dialog_button(
                solids,
                year_rect(r, i),
                &year.to_string(),
                setup.year == i,
                depth + 0.4,
            );
        }
        dialog_content::push_dialog_button(solids, done_rect(r), "Done", false, depth + 0.4);
    }
}
