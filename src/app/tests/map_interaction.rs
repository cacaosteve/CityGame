use super::*;
use crate::app::map_interaction::{
    MapInteractionFeedback, begin_map_pointer_interaction, move_map_pointer_interaction,
};

const WINDOW: (u32, u32) = (1024, 768);
const SCALE: f32 = 1.0;

fn fixture(tool: EditTool, mode: AppMode, fidelity: EngineFidelity) -> ViewerState {
    let mut state = test_state(City::new_blank("map-input.sc2", "Map input").unwrap());
    state.tool = tool;
    state.mode = mode;
    state.engine_fidelity = fidelity;
    state.city.set_city_funds(100_000);
    state
}

fn tile_screen(state: &ViewerState, tile: (usize, usize)) -> Vec2 {
    let (anchor, _, _, _) = state.structure_anchor(WINDOW, SCALE, tile.0, tile.1);
    // Picking uses the projected tile anchor rather than the image footprint.
    let screen = anchor;
    assert_eq!(state.tile_at_screen(screen, WINDOW, SCALE), Some(tile));
    screen
}

#[test]
fn map_down_matrix_retains_mouse_painting_only_on_the_audited_no_action_paths() {
    // City tools that retain the entry flag when Shift blocks the drag.
    let shift_gated = [
        EditTool::Road,
        EditTool::Rail,
        EditTool::Power,
        EditTool::Pipes,
        EditTool::Highway,
        EditTool::RaiseTerrain,
        EditTool::LowerTerrain,
        EditTool::LevelTerrain,
        EditTool::StretchTerrain,
        EditTool::SpreadTerrain,
    ];
    let tools = [
        EditTool::Query,
        EditTool::ZoneResidential,
        EditTool::Road,
        EditTool::Rail,
        EditTool::Power,
        EditTool::Pipes,
        EditTool::Highway,
        EditTool::RaiseTerrain,
        EditTool::LowerTerrain,
        EditTool::LevelTerrain,
        EditTool::StretchTerrain,
        EditTool::SpreadTerrain,
        EditTool::Trees,
        EditTool::Water,
        EditTool::Forest,
        EditTool::SmallPark,
        EditTool::Sign,
    ];
    for fidelity in [EngineFidelity::Win95, EngineFidelity::Mac68k] {
        for mode in [AppMode::CityView, AppMode::EditNewMap] {
            for tool in tools {
                for shift in [false, true] {
                    // False also covers the fresh state used by the touch caller.
                    for initial_painting in [false, true] {
                        let mut state = fixture(tool, mode, fidelity);
                        let screen = tile_screen(&state, (20, 20));
                        let mut painting = initial_painting;
                        let mut paint_tile = Some((11, 12));
                        begin_map_pointer_interaction(
                            &mut state,
                            screen,
                            WINDOW,
                            SCALE,
                            &mut painting,
                            &mut paint_tile,
                            shift,
                            |_, _| {},
                        );
                        let retains = mode == AppMode::CityView
                            && (tool == EditTool::Query || (shift && shift_gated.contains(&tool)));
                        assert_eq!(
                            painting,
                            if retains { initial_painting } else { true },
                            "{mode:?} {fidelity:?} {tool:?} shift={shift} prior={initial_painting}"
                        );
                        assert!(
                            paint_tile.is_none() || paint_tile == Some((20, 20)),
                            "stale paint tile must be cleared: {paint_tile:?}"
                        );
                        assert_eq!(state.hover_tile, Some((20, 20)));
                    }
                }
            }
        }
    }
}

#[test]
fn valid_network_down_respects_shift_without_resetting_prior_mouse_painting() {
    for fidelity in [EngineFidelity::Win95, EngineFidelity::Mac68k] {
        for tool in [EditTool::Road, EditTool::Pipes, EditTool::Highway] {
            for shift in [false, true] {
                for initial_painting in [false, true] {
                    let mut state = fixture(tool, AppMode::CityView, fidelity);
                    let screen = tile_screen(&state, (20, 20));
                    let mut painting = initial_painting;
                    let mut paint_tile = Some((10, 10));
                    begin_map_pointer_interaction(
                        &mut state,
                        screen,
                        WINDOW,
                        SCALE,
                        &mut painting,
                        &mut paint_tile,
                        shift,
                        |_, effect| panic!("unexpected feedback: {effect:?}"),
                    );
                    assert_eq!(painting, !shift || initial_painting);
                    assert_eq!(paint_tile, None);
                    assert_eq!(
                        state.network_drag_active(),
                        !shift && tool == EditTool::Road
                    );
                    assert_eq!(
                        state.underground_drag_active(),
                        !shift && tool == EditTool::Pipes
                    );
                    assert_eq!(
                        state.highway_drag_active(),
                        !shift && tool == EditTool::Highway
                    );
                }
            }
        }
    }
}

#[test]
fn query_title_feedback_observes_the_completed_inspection() {
    let mut state = fixture(EditTool::Query, AppMode::CityView, EngineFidelity::Win95);
    let screen = tile_screen(&state, (20, 20));
    let mut painting = true;
    let mut paint_tile = Some((1, 1));
    let mut effects = Vec::new();
    begin_map_pointer_interaction(
        &mut state,
        screen,
        WINDOW,
        SCALE,
        &mut painting,
        &mut paint_tile,
        false,
        |state, effect| {
            assert!(state.query_is_inspect);
            assert_eq!(state.hover_tile, Some((20, 20)));
            assert!(state.city_info_dialog.is_some());
            effects.push(effect);
        },
    );
    assert_eq!(effects, [MapInteractionFeedback::UpdateTitle]);
    assert!(painting);
    assert_eq!(paint_tile, None);
}

#[test]
fn sign_feedback_starts_text_input_after_creating_the_sign() {
    let mut state = fixture(EditTool::Sign, AppMode::CityView, EngineFidelity::Win95);
    let screen = tile_screen(&state, (20, 20));
    let mut painting = false;
    let mut paint_tile = None;
    let mut effects = Vec::new();
    begin_map_pointer_interaction(
        &mut state,
        screen,
        WINDOW,
        SCALE,
        &mut painting,
        &mut paint_tile,
        false,
        |state, effect| {
            assert!(state.sign_dialog.is_some());
            assert!(state.city.text_overlay_at(20, 20).is_some_and(is_user_sign));
            effects.push(effect);
        },
    );
    assert_eq!(effects, [MapInteractionFeedback::StartSignInput]);
    assert!(painting);
    assert_eq!(paint_tile, Some((20, 20)));
}

#[test]
fn placement_sound_feedback_observes_the_completed_build() {
    let mut state = fixture(
        EditTool::SmallPark,
        AppMode::CityView,
        EngineFidelity::Win95,
    );
    let screen = tile_screen(&state, (20, 20));
    let mut painting = false;
    let mut paint_tile = None;
    let mut effects = Vec::new();
    begin_map_pointer_interaction(
        &mut state,
        screen,
        WINDOW,
        SCALE,
        &mut painting,
        &mut paint_tile,
        false,
        |state, effect| {
            assert_eq!(state.city.building_at(20, 20), SMALL_PARK_BUILDING_ID);
            effects.push(effect);
        },
    );
    assert_eq!(
        effects,
        [MapInteractionFeedback::PlaySound(PlacementSound::Cheers)]
    );
    assert!(painting);
}

#[test]
fn painting_motion_keeps_zone_priority_over_an_existing_network_drag() {
    let mut state = fixture(
        EditTool::ZoneResidential,
        AppMode::CityView,
        EngineFidelity::Win95,
    );
    state.begin_zone_drag(20, 20, ZONE_RESIDENTIAL_LIGHT);
    state.begin_network_drag(18, 18, NetworkKind::Road);
    let screen = tile_screen(&state, (22, 22));
    let mut paint_tile = Some((20, 20));
    let mut effects = Vec::new();
    move_map_pointer_interaction(
        &mut state,
        screen,
        WINDOW,
        SCALE,
        true,
        &mut paint_tile,
        false,
        |state, effect| {
            assert_eq!(state.zone_drag.as_ref().unwrap().current, (22, 22));
            assert_eq!(state.network_drag.as_ref().unwrap().current, (18, 18));
            effects.push(effect);
        },
    );
    assert_eq!(effects, [MapInteractionFeedback::UpdateTitle]);
    assert_eq!(paint_tile, Some((20, 20)));
}

#[test]
fn idle_motion_updates_hover_without_painting_or_moving_the_camera() {
    let mut state = fixture(
        EditTool::ZoneResidential,
        AppMode::CityView,
        EngineFidelity::Win95,
    );
    state.begin_zone_drag(20, 20, ZONE_RESIDENTIAL_LIGHT);
    let screen = tile_screen(&state, (22, 22));
    let camera = state.camera;
    let mut paint_tile = Some((20, 20));
    move_map_pointer_interaction(
        &mut state,
        screen,
        WINDOW,
        SCALE,
        false,
        &mut paint_tile,
        false,
        |_, effect| panic!("unexpected feedback: {effect:?}"),
    );
    assert_eq!(state.hover_tile, Some((22, 22)));
    assert_eq!(state.zone_drag.as_ref().unwrap().current, (20, 20));
    assert_eq!(paint_tile, Some((20, 20)));
    assert_eq!(state.camera, camera);
}

#[test]
fn power_pointer_drag_builds_on_first_press_motion_and_release_in_both_city_layouts() {
    for kind in [CityFileKind::MacCity, CityFileKind::WinSc2] {
        for end in [(24, 20), (20, 24)] {
            let mut state = fixture(EditTool::Power, AppMode::CityView, EngineFidelity::Win95);
            state.city.kind = kind;
            let start = (20, 20);
            let mut painting = false;
            let mut paint_tile = None;
            let screen = tile_screen(&state, start);
            begin_map_pointer_interaction(
                &mut state,
                screen,
                WINDOW,
                SCALE,
                &mut painting,
                &mut paint_tile,
                false,
                |_, _| {},
            );
            assert!(painting);
            assert!(state.network_drag_active());
            let screen = tile_screen(&state, end);
            move_map_pointer_interaction(
                &mut state,
                screen,
                WINDOW,
                SCALE,
                painting,
                &mut paint_tile,
                false,
                |_, _| {},
            );
            let outcome = state.release_left_mouse_drag(false);
            assert_eq!(
                outcome.network,
                Some((NetworkKind::Power, 5)),
                "{kind:?} {end:?}"
            );
            assert!(!state.network_drag_active());
            for (x, y) in network_drag_path_for_kind(start, end, NetworkKind::Power) {
                assert!(state.city.tile_bits_at(x, y).unwrap().powerable());
                assert_eq!(
                    state.city.building_at(x, y),
                    if end.0 == start.0 { 0x0e } else { 0x0f }
                );
            }
            assert_eq!(state.city.city_funds(), 100_000 - 5 * 2);
        }
    }
}
