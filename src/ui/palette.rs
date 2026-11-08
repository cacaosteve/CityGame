use super::*;

#[derive(Debug, Clone)]
pub struct UiLayer {
    pub images: Vec<UiImage>,
    pub solids: Vec<UiSolid>,
}

impl UiLayer {
    /// Shift every solid/image by `origin` (used to dock the Tools palette in the main window).
    pub fn offset_by(&mut self, origin: Vec2) {
        for solid in &mut self.solids {
            solid.x += origin.x;
            solid.y += origin.y;
        }
        for image in &mut self.images {
            image.x += origin.x;
            image.y += origin.y;
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct ButtonRect {
    action: UiAction,
    src: [u32; 4],
    x: f32,
    y: f32,
    w: f32,
    h: f32,
}

#[allow(clippy::too_many_arguments)]
pub fn build_palette_ui(
    renderer: &Renderer<'_>,
    logical_size: (u32, u32),
    pixel_scale: f32,
    mode: AppMode,
    active_tool: EditTool,
    pressed: Option<UiAction>,
    open_menu: Option<ToolMenu>,
    disabled: &[UiAction],
    checked: &[UiAction],
    edit_map_sliders: Option<EditMapSliderValues>,
) -> UiLayer {
    let m = UiMetrics::for_logical_height(logical_size.1.max(logical_size.0));
    let mut images = Vec::new();
    let mut solids = Vec::new();

    let (atlas, src_w, src_h) = match mode {
        AppMode::CityView => (renderer.ui_city_atlas(), CITY_PALETTE_W, CITY_PALETTE_H),
        AppMode::EditNewMap => (renderer.ui_edit_atlas(), EDIT_PALETTE_W, EDIT_PALETTE_H),
    };

    if renderer.has_sprite(atlas, 1) {
        images.push(UiImage {
            atlas,
            src_x: 0,
            src_y: 0,
            src_w: src_w * PALETTE_ART_SCALE,
            src_h: src_h * PALETTE_ART_SCALE,
            x: 0.0,
            y: 0.0,
            w: m.palette_width(mode) * pixel_scale,
            h: m.palette_height(mode) * pixel_scale,
            depth: UI_DEPTH,
            tint: [1.0, 1.0, 1.0, 1.0],
        });
    }

    for button in palette_button_rects(logical_size, pixel_scale, mode) {
        if disabled.contains(&button.action) {
            push_disabled_button_state(&mut images, &mut solids, atlas, button);
        } else if checked.contains(&button.action)
            || pressed == Some(button.action)
            || matches!(button.action, UiAction::OpenMenu(menu) if open_menu == Some(menu))
            || action_matches_active_tool(button.action, active_tool)
        {
            push_pressed_button_state(&mut images, &mut solids, atlas, button, pixel_scale);
        }
    }

    if mode == AppMode::EditNewMap {
        if let Some(values) = edit_map_sliders {
            push_edit_map_slider_handles(&mut solids, logical_size, pixel_scale, values);
        }

        let done = backing_rect(logical_size, pixel_scale, Vec2::ZERO, EDIT_DONE_BUTTON);
        solids.push(UiSolid {
            x: done.x,
            y: done.y,
            w: done.w,
            h: done.h,
            color: [0.30, 0.94, 0.84, 0.24],
            depth: UI_DEPTH + 1.0,
        });
        if pressed == Some(UiAction::DoneEditMap) {
            solids.push(UiSolid {
                x: done.x,
                y: done.y,
                w: done.w,
                h: done.h,
                color: [0.02, 0.10, 0.15, 0.46],
                depth: UI_DEPTH + 6.0,
            });
        }
    }

    if let Some(menu) = open_menu {
        push_tool_menu_popup(
            &mut solids,
            logical_size,
            pixel_scale,
            mode,
            menu,
            active_tool,
            disabled,
            renderer.dark_appearance(),
        );
    }

    UiLayer { images, solids }
}

fn push_edit_map_slider_handles(
    solids: &mut Vec<UiSolid>,
    logical_size: (u32, u32),
    pixel_scale: f32,
    values: EditMapSliderValues,
) {
    let scale = UiMetrics::for_logical_height(logical_size.1).palette_scale * pixel_scale;
    for slider in EDIT_MAP_SLIDERS {
        let rect = backing_rect(logical_size, pixel_scale, Vec2::ZERO, slider.src);
        let value = values.value(slider.slider).min(EDIT_MAP_SLIDER_MAX);
        let offset = f32::from(EDIT_MAP_SLIDER_MAX - value) * scale;
        let knob = ScreenRect {
            x: rect.x + 2.0 * scale,
            y: rect.y + offset - scale,
            w: 11.0 * scale,
            h: 3.0 * scale,
        };
        solids.push(UiSolid {
            x: knob.x,
            y: knob.y,
            w: knob.w,
            h: knob.h,
            color: [0.02, 0.10, 0.15, 0.85],
            depth: UI_DEPTH + 5.0,
        });
        solids.push(UiSolid {
            x: knob.x + scale,
            y: knob.y + scale,
            w: (knob.w - 2.0 * scale).max(scale),
            h: scale,
            color: [0.34, 0.82, 0.78, 0.98],
            depth: UI_DEPTH + 6.0,
        });
    }
}

fn action_matches_active_tool(action: UiAction, active_tool: EditTool) -> bool {
    match action {
        UiAction::SelectTool(tool) => tool_family(tool) == tool_family(active_tool),
        UiAction::OpenMenu(menu) => tool_menu_items(menu)
            .iter()
            .any(|item| tool_family(item.tool) == tool_family(active_tool)),
        _ => false,
    }
}

fn tool_family(tool: EditTool) -> EditTool {
    match tool {
        EditTool::ZoneResidentialDense => EditTool::ZoneResidential,
        EditTool::ZoneCommercialDense => EditTool::ZoneCommercial,
        EditTool::ZoneIndustrialDense => EditTool::ZoneIndustrial,
        EditTool::Highway | EditTool::Tunnel | EditTool::Onramp | EditTool::BusDepot => {
            EditTool::Road
        }
        EditTool::Subway | EditTool::RailDepot | EditTool::SubStation | EditTool::SubToRail => {
            EditTool::Rail
        }
        EditTool::PowerPlant
        | EditTool::PowerPlantCoal
        | EditTool::PowerPlantHydro
        | EditTool::PowerPlantOil
        | EditTool::PowerPlantGas
        | EditTool::PowerPlantNuclear
        | EditTool::PowerPlantWind
        | EditTool::PowerPlantSolar
        | EditTool::PowerPlantMicrowave
        | EditTool::PowerPlantFusion => EditTool::Power,
        EditTool::Seaport | EditTool::Airport => EditTool::Seaport,
        EditTool::DispatchPolice | EditTool::DispatchFirefighters | EditTool::DispatchMilitary => {
            EditTool::DispatchPolice
        }
        EditTool::MayorHouse
        | EditTool::CityHall
        | EditTool::Statue
        | EditTool::BraunLlamaDome
        | EditTool::Arcologies
        | EditTool::ArcologyPlymouth
        | EditTool::ArcologyForest
        | EditTool::ArcologyDarco
        | EditTool::ArcologyLaunch => EditTool::MayorHouse,
        EditTool::DeZone => EditTool::Bulldoze,
        _ => tool,
    }
}

fn push_tool_menu_popup(
    solids: &mut Vec<UiSolid>,
    logical_size: (u32, u32),
    pixel_scale: f32,
    mode: AppMode,
    menu: ToolMenu,
    active_tool: EditTool,
    disabled: &[UiAction],
    dark: bool,
) {
    let Some(button) = menu_button_rect(logical_size, pixel_scale, mode, menu) else {
        return;
    };
    let metrics = UiMetrics::for_logical_height(logical_size.1.max(logical_size.0));
    let scale = (metrics.palette_scale * pixel_scale).round().max(1.0);
    let items = tool_menu_items(menu);
    let padding_x = 4.0 * scale;
    let padding_y = 2.0 * scale;
    let item_h = 9.0 * scale;
    let text_w = items
        .iter()
        .map(|item| menu_text_width(item.label, scale))
        .fold(0.0, f32::max);
    let menu_w = text_w + padding_x * 2.0;
    let menu_h = items.len() as f32 * item_h;
    let screen_w = logical_size.0 as f32 * pixel_scale;
    let screen_h = logical_size.1 as f32 * pixel_scale;
    let x = (button.x + button.w - scale).clamp(0.0, (screen_w - menu_w).max(0.0));
    let y = button.y.clamp(0.0, (screen_h - menu_h).max(0.0));
    let depth = UI_DEPTH + 20.0;
    solids.push(UiSolid {
        x,
        y,
        w: menu_w,
        h: menu_h,
        color: if dark {
            [0.10, 0.18, 0.25, 0.98]
        } else {
            [0.82, 0.88, 0.90, 0.99]
        },
        depth,
    });
    push_rect_outline(solids, x, y, menu_w, menu_h, scale, depth + 0.1);

    for (index, item) in items.iter().enumerate() {
        let item_y = y + index as f32 * item_h;
        let item_action = UiAction::SelectTool(item.tool);
        let item_disabled = disabled.contains(&item_action);
        if menu_item_matches_active_tool(item.tool, active_tool) && !item_disabled {
            solids.push(UiSolid {
                x: x + scale,
                y: item_y + scale,
                w: (menu_w - 2.0 * scale).max(1.0),
                h: (item_h - 2.0 * scale).max(1.0),
                color: if dark {
                    [0.08, 0.40, 0.43, 1.0]
                } else {
                    [0.30, 0.65, 0.63, 1.0]
                },
                depth: depth + 0.2,
            });
        }
        push_pixel_text(
            solids,
            item.label,
            x + padding_x,
            item_y + padding_y,
            scale,
            if item_disabled {
                if dark {
                    [0.34, 0.47, 0.52, 1.0]
                } else {
                    [0.34, 0.42, 0.45, 1.0]
                }
            } else {
                if dark {
                    [0.86, 0.94, 0.94, 1.0]
                } else {
                    [0.08, 0.16, 0.20, 1.0]
                }
            },
            depth + 0.4,
        );
    }
}

fn menu_item_matches_active_tool(item_tool: EditTool, active_tool: EditTool) -> bool {
    match item_tool {
        EditTool::PowerPlant => matches!(
            active_tool,
            EditTool::PowerPlant
                | EditTool::PowerPlantCoal
                | EditTool::PowerPlantHydro
                | EditTool::PowerPlantOil
                | EditTool::PowerPlantGas
                | EditTool::PowerPlantNuclear
                | EditTool::PowerPlantWind
                | EditTool::PowerPlantSolar
                | EditTool::PowerPlantMicrowave
                | EditTool::PowerPlantFusion
        ),
        EditTool::Arcologies => matches!(
            active_tool,
            EditTool::Arcologies
                | EditTool::ArcologyPlymouth
                | EditTool::ArcologyForest
                | EditTool::ArcologyDarco
                | EditTool::ArcologyLaunch
        ),
        _ => item_tool == active_tool,
    }
}

fn push_rect_outline(
    solids: &mut Vec<UiSolid>,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    line: f32,
    depth: f32,
) {
    let dark = [0.03, 0.12, 0.18, 1.0];
    solids.push(UiSolid {
        x,
        y,
        w,
        h: line,
        color: dark,
        depth,
    });
    solids.push(UiSolid {
        x,
        y,
        w: line,
        h,
        color: dark,
        depth,
    });
    solids.push(UiSolid {
        x,
        y: y + h - line,
        w,
        h: line,
        color: dark,
        depth,
    });
    solids.push(UiSolid {
        x: x + w - line,
        y,
        w: line,
        h,
        color: dark,
        depth,
    });
}

fn menu_item_rects(
    logical_size: (u32, u32),
    pixel_scale: f32,
    mode: AppMode,
    menu: ToolMenu,
) -> Vec<(ToolMenuItem, ScreenRect)> {
    let Some(button) = menu_button_rect(logical_size, pixel_scale, mode, menu) else {
        return Vec::new();
    };
    let metrics = UiMetrics::for_logical_height(logical_size.1.max(logical_size.0));
    let scale = (metrics.palette_scale * pixel_scale).round().max(1.0);
    let items = tool_menu_items(menu);
    let padding_x = 4.0 * scale;
    let item_h = 9.0 * scale;
    let text_w = items
        .iter()
        .map(|item| menu_text_width(item.label, scale))
        .fold(0.0, f32::max);
    let menu_w = text_w + padding_x * 2.0;
    let menu_h = items.len() as f32 * item_h;
    let screen_w = logical_size.0 as f32 * pixel_scale;
    let screen_h = logical_size.1 as f32 * pixel_scale;
    let x = (button.x + button.w - scale).clamp(0.0, (screen_w - menu_w).max(0.0));
    let y = button.y.clamp(0.0, (screen_h - menu_h).max(0.0));

    items
        .iter()
        .enumerate()
        .map(|(index, item)| {
            (
                *item,
                ScreenRect {
                    x,
                    y: y + index as f32 * item_h,
                    w: menu_w,
                    h: item_h,
                },
            )
        })
        .collect()
}

fn menu_button_rect(
    logical_size: (u32, u32),
    pixel_scale: f32,
    mode: AppMode,
    menu: ToolMenu,
) -> Option<ButtonRect> {
    palette_button_rects(logical_size, pixel_scale, mode)
        .into_iter()
        .find(|button| button.action == UiAction::OpenMenu(menu))
}

fn menu_text_width(text: &str, scale: f32) -> f32 {
    pixel_text_width(text, scale)
}

pub fn pixel_text_width(text: &str, scale: f32) -> f32 {
    let mut width = 0.0;
    for ch in text.chars() {
        width += (if ch == ' ' { 3.0 } else { 4.0 }) * scale;
    }
    (width - scale).max(0.0)
}

fn push_menu_text(solids: &mut Vec<UiSolid>, text: &str, x: f32, y: f32, scale: f32, depth: f32) {
    push_pixel_text(solids, text, x, y, scale, [0.0, 0.0, 0.0, 1.0], depth);
}

pub fn push_pixel_text(
    solids: &mut Vec<UiSolid>,
    text: &str,
    x: f32,
    y: f32,
    scale: f32,
    color: [f32; 4],
    depth: f32,
) {
    push_pixel_glyphs(
        solids,
        text.chars().map(|ch| ch.to_ascii_uppercase()),
        x,
        y,
        scale,
        color,
        depth,
    );
}

/// Editable names keep their typed case; lowercase descenders extend two pixels below the baseline.
pub fn push_pixel_text_preserving_case(
    solids: &mut Vec<UiSolid>,
    text: &str,
    x: f32,
    y: f32,
    scale: f32,
    color: [f32; 4],
    depth: f32,
) {
    push_pixel_glyphs(solids, text.chars(), x, y, scale, color, depth);
}

fn push_pixel_glyphs(
    solids: &mut Vec<UiSolid>,
    chars: impl Iterator<Item = char>,
    x: f32,
    y: f32,
    scale: f32,
    color: [f32; 4],
    depth: f32,
) {
    let mut cursor = x;
    for ch in chars {
        if ch == ' ' {
            cursor += 3.0 * scale;
            continue;
        }
        let glyph = case_sensitive_glyph(ch);
        for (row, bits) in glyph.iter().enumerate() {
            for col in 0..3 {
                if bits & (1 << (2 - col)) == 0 {
                    continue;
                }
                solids.push(UiSolid {
                    x: cursor + col as f32 * scale,
                    y: y + row as f32 * scale,
                    w: scale,
                    h: scale,
                    color,
                    depth,
                });
            }
        }
        cursor += 4.0 * scale;
    }
}

fn case_sensitive_glyph(ch: char) -> [u8; 7] {
    match ch {
        'a' => [0, 0, 0b011, 0b101, 0b111, 0, 0],
        'b' => [0b100, 0b100, 0b110, 0b101, 0b110, 0, 0],
        'c' => [0, 0, 0b011, 0b100, 0b011, 0, 0],
        'd' => [0b001, 0b001, 0b011, 0b101, 0b011, 0, 0],
        'e' => [0, 0, 0b011, 0b110, 0b011, 0, 0],
        'f' => [0b011, 0b010, 0b111, 0b010, 0b010, 0, 0],
        'g' => [0, 0, 0b011, 0b101, 0b011, 0b001, 0b110],
        'h' => [0b100, 0b100, 0b110, 0b101, 0b101, 0, 0],
        'i' => [0b010, 0, 0b010, 0b010, 0b010, 0, 0],
        'j' => [0b001, 0, 0b001, 0b001, 0b001, 0b101, 0b010],
        'k' => [0b100, 0b100, 0b101, 0b110, 0b101, 0, 0],
        'l' => [0b110, 0b010, 0b010, 0b010, 0b011, 0, 0],
        'm' => [0, 0, 0b111, 0b111, 0b101, 0, 0],
        'n' => [0, 0, 0b110, 0b101, 0b101, 0, 0],
        'o' => [0, 0, 0b010, 0b101, 0b010, 0, 0],
        'p' => [0, 0, 0b110, 0b101, 0b110, 0b100, 0b100],
        'q' => [0, 0, 0b011, 0b101, 0b011, 0b001, 0b001],
        'r' => [0, 0, 0b110, 0b101, 0b100, 0, 0],
        's' => [0, 0, 0b011, 0b010, 0b110, 0, 0],
        't' => [0b010, 0b010, 0b111, 0b010, 0b011, 0, 0],
        'u' => [0, 0, 0b101, 0b101, 0b011, 0, 0],
        'v' => [0, 0, 0b101, 0b101, 0b010, 0, 0],
        'w' => [0, 0, 0b101, 0b111, 0b111, 0, 0],
        'x' => [0, 0, 0b101, 0b010, 0b101, 0, 0],
        'y' => [0, 0, 0b101, 0b101, 0b011, 0b001, 0b110],
        'z' => [0, 0, 0b111, 0b010, 0b111, 0, 0],
        _ => {
            let rows = menu_glyph(ch);
            [rows[0], rows[1], rows[2], rows[3], rows[4], 0, 0]
        }
    }
}

fn menu_glyph(ch: char) -> [u8; 5] {
    match ch {
        'A' => [0b010, 0b101, 0b111, 0b101, 0b101],
        'B' => [0b110, 0b101, 0b110, 0b101, 0b110],
        'C' => [0b011, 0b100, 0b100, 0b100, 0b011],
        'D' => [0b110, 0b101, 0b101, 0b101, 0b110],
        'E' => [0b111, 0b100, 0b110, 0b100, 0b111],
        'F' => [0b111, 0b100, 0b110, 0b100, 0b100],
        'G' => [0b011, 0b100, 0b101, 0b101, 0b011],
        'H' => [0b101, 0b101, 0b111, 0b101, 0b101],
        'I' => [0b111, 0b010, 0b010, 0b010, 0b111],
        'J' => [0b001, 0b001, 0b001, 0b101, 0b010],
        'K' => [0b101, 0b101, 0b110, 0b101, 0b101],
        'L' => [0b100, 0b100, 0b100, 0b100, 0b111],
        'M' => [0b101, 0b111, 0b111, 0b101, 0b101],
        'N' => [0b101, 0b111, 0b111, 0b111, 0b101],
        'O' => [0b010, 0b101, 0b101, 0b101, 0b010],
        'P' => [0b110, 0b101, 0b110, 0b100, 0b100],
        'Q' => [0b010, 0b101, 0b101, 0b111, 0b011],
        'R' => [0b110, 0b101, 0b110, 0b101, 0b101],
        'S' => [0b011, 0b100, 0b010, 0b001, 0b110],
        'T' => [0b111, 0b010, 0b010, 0b010, 0b010],
        'U' => [0b101, 0b101, 0b101, 0b101, 0b111],
        'V' => [0b101, 0b101, 0b101, 0b101, 0b010],
        'W' => [0b101, 0b101, 0b111, 0b111, 0b101],
        'X' => [0b101, 0b101, 0b010, 0b101, 0b101],
        'Y' => [0b101, 0b101, 0b010, 0b010, 0b010],
        'Z' => [0b111, 0b001, 0b010, 0b100, 0b111],
        '/' => [0b001, 0b001, 0b010, 0b100, 0b100],
        '-' => [0b000, 0b000, 0b111, 0b000, 0b000],
        '.' => [0b000, 0b000, 0b000, 0b000, 0b010],
        ',' => [0b000, 0b000, 0b000, 0b010, 0b100],
        ':' => [0b000, 0b010, 0b000, 0b010, 0b000],
        '(' => [0b001, 0b010, 0b010, 0b010, 0b001],
        ')' => [0b100, 0b010, 0b010, 0b010, 0b100],
        '\'' => [0b010, 0b010, 0b000, 0b000, 0b000],
        '<' => [0b001, 0b010, 0b100, 0b010, 0b001],
        '>' => [0b100, 0b010, 0b001, 0b010, 0b100],
        '$' => [0b111, 0b110, 0b111, 0b011, 0b111],
        '0' => [0b111, 0b101, 0b101, 0b101, 0b111],
        '1' => [0b010, 0b110, 0b010, 0b010, 0b111],
        '2' => [0b111, 0b001, 0b111, 0b100, 0b111],
        '3' => [0b111, 0b001, 0b111, 0b001, 0b111],
        '4' => [0b101, 0b101, 0b111, 0b001, 0b001],
        '5' => [0b111, 0b100, 0b111, 0b001, 0b111],
        '6' => [0b111, 0b100, 0b111, 0b101, 0b111],
        '7' => [0b111, 0b001, 0b010, 0b100, 0b100],
        '8' => [0b111, 0b101, 0b111, 0b101, 0b111],
        '9' => [0b111, 0b101, 0b111, 0b001, 0b111],
        _ => [0b111, 0b001, 0b010, 0b000, 0b010],
    }
}

fn push_disabled_button_state(
    images: &mut Vec<UiImage>,
    solids: &mut Vec<UiSolid>,
    atlas: usize,
    button: ButtonRect,
) {
    push_button_face(solids, button, UI_DEPTH + 7.0, false);
    push_button_image(
        images,
        atlas,
        button,
        Vec2::ZERO,
        UI_DEPTH + 8.0,
        [1.0, 1.0, 1.0, 1.0],
    );
    solids.push(UiSolid {
        x: button.x,
        y: button.y,
        w: button.w,
        h: button.h,
        color: [0.62, 0.62, 0.62, 0.46],
        depth: UI_DEPTH + 9.0,
    });
}

fn push_pressed_button_state(
    images: &mut Vec<UiImage>,
    solids: &mut Vec<UiSolid>,
    atlas: usize,
    button: ButtonRect,
    pixel_scale: f32,
) {
    push_button_face(solids, button, UI_DEPTH + 5.0, true);
    let inset = source_pixel_scale(button, pixel_scale);
    push_button_image_clipped(
        images,
        atlas,
        button,
        Vec2::new(inset, inset),
        UI_DEPTH + 6.0,
        [1.0, 1.0, 1.0, 1.0],
    );
}

fn source_pixel_scale(button: ButtonRect, pixel_scale: f32) -> f32 {
    (button.w / button.src[2].max(1) as f32)
        .min(button.h / button.src[3].max(1) as f32)
        .min(2.0 * pixel_scale.max(1.0))
        .max(1.0)
        .round()
}

fn push_button_image(
    images: &mut Vec<UiImage>,
    atlas: usize,
    button: ButtonRect,
    offset: Vec2,
    depth: f32,
    tint: [f32; 4],
) {
    images.push(UiImage {
        atlas,
        src_x: button.src[0] * PALETTE_ART_SCALE,
        src_y: button.src[1] * PALETTE_ART_SCALE,
        src_w: button.src[2] * PALETTE_ART_SCALE,
        src_h: button.src[3] * PALETTE_ART_SCALE,
        x: button.x + offset.x,
        y: button.y + offset.y,
        w: (button.w - offset.x).max(1.0),
        h: (button.h - offset.y).max(1.0),
        depth,
        tint,
    });
}

fn push_button_image_clipped(
    images: &mut Vec<UiImage>,
    atlas: usize,
    button: ButtonRect,
    offset: Vec2,
    depth: f32,
    tint: [f32; 4],
) {
    let scale_x = button.w / button.src[2].max(1) as f32;
    let scale_y = button.h / button.src[3].max(1) as f32;
    let texture_per_screen_x = PALETTE_ART_SCALE as f32 / scale_x;
    let texture_per_screen_y = PALETTE_ART_SCALE as f32 / scale_y;
    let crop_w = (offset.x * texture_per_screen_x).round() as u32;
    let crop_h = (offset.y * texture_per_screen_y).round() as u32;
    let full_src_w = button.src[2] * PALETTE_ART_SCALE;
    let full_src_h = button.src[3] * PALETTE_ART_SCALE;
    let src_w = full_src_w.saturating_sub(crop_w).max(1);
    let src_h = full_src_h.saturating_sub(crop_h).max(1);
    images.push(UiImage {
        atlas,
        src_x: button.src[0] * PALETTE_ART_SCALE,
        src_y: button.src[1] * PALETTE_ART_SCALE,
        src_w,
        src_h,
        x: button.x + offset.x,
        y: button.y + offset.y,
        w: src_w as f32 / texture_per_screen_x,
        h: src_h as f32 / texture_per_screen_y,
        depth,
        tint,
    });
}

fn push_button_face(solids: &mut Vec<UiSolid>, button: ButtonRect, depth: f32, pressed: bool) {
    solids.push(UiSolid {
        x: button.x,
        y: button.y,
        w: button.w,
        h: button.h,
        color: if pressed {
            [0.10, 0.37, 0.41, 1.0]
        } else {
            [0.16, 0.28, 0.36, 1.0]
        },
        depth,
    });

    let line = (button.w.min(button.h) / button.src[2].min(button.src[3]) as f32).max(1.0);
    let (top_left, bottom_right) = if pressed {
        ([0.03, 0.10, 0.15, 1.0], [0.35, 0.86, 0.80, 1.0])
    } else {
        ([0.48, 0.69, 0.74, 1.0], [0.05, 0.12, 0.18, 1.0])
    };
    solids.push(UiSolid {
        x: button.x,
        y: button.y,
        w: button.w,
        h: line,
        color: top_left,
        depth: depth + 0.1,
    });
    solids.push(UiSolid {
        x: button.x,
        y: button.y,
        w: line,
        h: button.h,
        color: top_left,
        depth: depth + 0.1,
    });
    solids.push(UiSolid {
        x: button.x,
        y: button.y + button.h - line,
        w: button.w,
        h: line,
        color: bottom_right,
        depth: depth + 0.1,
    });
    solids.push(UiSolid {
        x: button.x + button.w - line,
        y: button.y,
        w: line,
        h: button.h,
        color: bottom_right,
        depth: depth + 0.1,
    });
}

pub fn palette_hit_test(
    screen: Vec2,
    logical_size: (u32, u32),
    pixel_scale: f32,
    mode: AppMode,
    open_menu: Option<ToolMenu>,
) -> Option<UiAction> {
    palette_hit_test_padded(screen, logical_size, pixel_scale, mode, open_menu, 0.0)
}

/// Hit-test a palette drawn at `origin` inside the main window (docked / single-window).
pub fn palette_hit_test_at(
    screen: Vec2,
    origin: Vec2,
    logical_size: (u32, u32),
    pixel_scale: f32,
    mode: AppMode,
    open_menu: Option<ToolMenu>,
    hit_pad: f32,
) -> Option<UiAction> {
    palette_hit_test_padded(
        screen - origin,
        logical_size,
        pixel_scale,
        mode,
        open_menu,
        hit_pad,
    )
}

pub fn palette_hit_test_padded(
    screen: Vec2,
    logical_size: (u32, u32),
    pixel_scale: f32,
    mode: AppMode,
    open_menu: Option<ToolMenu>,
    hit_pad: f32,
) -> Option<UiAction> {
    let pad = hit_pad.max(0.0);
    if let Some(menu) = open_menu {
        for (item, rect) in menu_item_rects(logical_size, pixel_scale, mode, menu) {
            if point_in_rect(screen, inflate_rect(rect, pad)) {
                return Some(UiAction::SelectTool(item.tool));
            }
        }
    }
    for button in palette_button_rects(logical_size, pixel_scale, mode) {
        if point_in_rect(
            screen,
            inflate_rect(
                ScreenRect {
                    x: button.x,
                    y: button.y,
                    w: button.w,
                    h: button.h,
                },
                pad,
            ),
        ) {
            return Some(button.action);
        }
    }
    if mode == AppMode::EditNewMap {
        let done = backing_rect(logical_size, pixel_scale, Vec2::ZERO, EDIT_DONE_BUTTON);
        if point_in_rect(screen, inflate_rect(done, pad)) {
            return Some(UiAction::DoneEditMap);
        }
    }
    None
}

/// True when `screen` lies inside the docked palette chrome (including empty chrome).
pub fn point_in_docked_palette(
    screen: Vec2,
    origin: Vec2,
    logical_size: (u32, u32),
    pixel_scale: f32,
) -> bool {
    let local = screen - origin;
    local.x >= 0.0
        && local.y >= 0.0
        && local.x < logical_size.0 as f32 * pixel_scale.max(1.0)
        && local.y < logical_size.1 as f32 * pixel_scale.max(1.0)
}

fn inflate_rect(rect: ScreenRect, pad: f32) -> ScreenRect {
    if pad <= 0.0 {
        return rect;
    }
    ScreenRect {
        x: rect.x - pad,
        y: rect.y - pad,
        w: rect.w + pad * 2.0,
        h: rect.h + pad * 2.0,
    }
}

pub fn edit_map_slider_hit_test(
    screen: Vec2,
    logical_size: (u32, u32),
    pixel_scale: f32,
    mode: AppMode,
    open_menu: Option<ToolMenu>,
) -> Option<(EditMapSlider, u8)> {
    if mode != AppMode::EditNewMap || open_menu.is_some() {
        return None;
    }
    let scale = UiMetrics::for_logical_height(logical_size.1).palette_scale * pixel_scale;
    for slider in EDIT_MAP_SLIDERS {
        let rect = backing_rect(logical_size, pixel_scale, Vec2::ZERO, slider.src);
        if point_in_rect(screen, rect) {
            let mac_y = ((screen.y - rect.y) / scale).floor();
            let value = EDIT_MAP_SLIDER_MAX.saturating_sub(mac_y.clamp(0.0, 47.0) as u8);
            return Some((slider.slider, value));
        }
    }
    None
}

pub fn palette_hover_label(
    screen: Vec2,
    logical_size: (u32, u32),
    pixel_scale: f32,
    mode: AppMode,
    open_menu: Option<ToolMenu>,
) -> Option<&'static str> {
    if let Some((slider, _)) =
        edit_map_slider_hit_test(screen, logical_size, pixel_scale, mode, open_menu)
    {
        return Some(slider.label());
    }
    palette_hit_test(screen, logical_size, pixel_scale, mode, open_menu).map(UiAction::label)
}

fn point_in_rect(point: Vec2, rect: ScreenRect) -> bool {
    point.x >= rect.x && point.x < rect.x + rect.w && point.y >= rect.y && point.y < rect.y + rect.h
}

pub fn status_title(
    city_name: &str,
    speed: crate::traffic::GameSpeed,
    mode: AppMode,
    active_tool: EditTool,
) -> String {
    let mode_label = match mode {
        AppMode::CityView => city_name.to_string(),
        AppMode::EditNewMap => format!("Edit Map — {}", active_tool.label()),
    };
    format!("{mode_label}  [{}]", speed.label())
}

fn palette_button_rects(
    logical_size: (u32, u32),
    pixel_scale: f32,
    mode: AppMode,
) -> Vec<ButtonRect> {
    let entries = match mode {
        AppMode::CityView => CITY_PALETTE_BUTTONS,
        AppMode::EditNewMap => EDIT_PALETTE_BUTTONS,
    };
    entries
        .iter()
        .map(|entry| {
            let rect = backing_rect(logical_size, pixel_scale, Vec2::ZERO, entry.src);
            ButtonRect {
                action: entry.action,
                src: entry.src,
                x: rect.x,
                y: rect.y,
                w: rect.w,
                h: rect.h,
            }
        })
        .collect()
}

#[derive(Debug, Clone, Copy)]
struct ScreenRect {
    x: f32,
    y: f32,
    w: f32,
    h: f32,
}

fn backing_rect(
    logical_size: (u32, u32),
    pixel_scale: f32,
    origin: Vec2,
    src: [u32; 4],
) -> ScreenRect {
    let m = UiMetrics::for_logical_height(logical_size.1.max(logical_size.0));
    let logical = ScreenRect {
        x: origin.x + src[0] as f32 * m.palette_scale,
        y: origin.y + src[1] as f32 * m.palette_scale,
        w: src[2] as f32 * m.palette_scale,
        h: src[3] as f32 * m.palette_scale,
    };
    ScreenRect {
        x: logical.x * pixel_scale,
        y: logical.y * pixel_scale,
        w: logical.w * pixel_scale,
        h: logical.h * pixel_scale,
    }
}

#[cfg(test)]
mod tests;
