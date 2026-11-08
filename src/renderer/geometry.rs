use super::*;

impl<'window> Renderer<'window> {
    pub(super) fn build_vertices_from_refs(
        &self,
        sprites: &[&RenderSprite],
    ) -> (Vec<Vertex>, Vec<(usize, u32, u32)>) {
        let mut vertices = Vec::new();
        let mut ranges = Vec::new();
        if sprites.is_empty() {
            return (vertices, ranges);
        }

        let mut sorted: Vec<&RenderSprite> = sprites.to_vec();
        sorted.sort_by(|left, right| left.depth.total_cmp(&right.depth));

        let mut current_atlas = sorted[0].atlas;
        let mut range_start = 0u32;

        let mut flush_range = |atlas_index: usize, start: u32, verts: &Vec<Vertex>| {
            let count = verts.len() as u32 - start;
            if count > 0 {
                ranges.push((atlas_index, start, count));
            }
        };

        for sprite in sorted {
            if sprite.atlas != current_atlas {
                flush_range(current_atlas, range_start, &vertices);
                current_atlas = sprite.atlas;
                range_start = vertices.len() as u32;
            }
            if let Some(atlas) = self.atlases.get(sprite.atlas)
                && let Some(frame) = atlas.frames.get(&sprite.sprite_id)
            {
                push_sprite_vertices(
                    &mut vertices,
                    frame,
                    atlas.width,
                    atlas.height,
                    sprite.anchor,
                    sprite.scale,
                    sprite.flip_x,
                    sprite.bottom_left_anchor,
                    sprite.tint,
                    self.config.width as f32,
                    self.config.height as f32,
                );
            }
        }
        flush_range(current_atlas, range_start, &vertices);

        (vertices, ranges)
    }

    pub(super) fn draw_ordered_ui(
        &self,
        pass: &mut wgpu::RenderPass<'_>,
        ui_images: &[UiImage],
        ui_solids: &[UiSolid],
    ) {
        let mut items = Vec::with_capacity(ui_images.len() + ui_solids.len());
        items.extend(
            ui_images
                .iter()
                .enumerate()
                .map(|(index, image)| OrderedUi::Image { index, image }),
        );
        items.extend(
            ui_solids
                .iter()
                .enumerate()
                .map(|(index, solid)| OrderedUi::Solid { index, solid }),
        );
        items.sort_by(|left, right| {
            left.depth()
                .total_cmp(&right.depth())
                .then_with(|| left.index().cmp(&right.index()))
        });

        let screen_w = self.config.width as f32;
        let screen_h = self.config.height as f32;
        for item in items {
            match item {
                OrderedUi::Image { image, .. } => {
                    let Some(atlas) = self.atlases.get(image.atlas) else {
                        continue;
                    };
                    let mut vertices = Vec::with_capacity(6);
                    push_ui_image_vertices(
                        &mut vertices,
                        image,
                        atlas.width,
                        atlas.height,
                        screen_w,
                        screen_h,
                    );
                    if vertices.is_empty() {
                        continue;
                    }
                    let buffer =
                        self.device
                            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                                label: Some("ordered ui image vertex buffer"),
                                contents: bytemuck::cast_slice(&vertices),
                                usage: wgpu::BufferUsages::VERTEX,
                            });
                    pass.set_pipeline(&self.pipeline);
                    pass.set_bind_group(0, &atlas.bind_group, &[]);
                    pass.set_vertex_buffer(0, buffer.slice(..));
                    pass.draw(0..vertices.len() as u32, 0..1);
                }
                OrderedUi::Solid { solid, .. } => {
                    let vertices =
                        build_ui_vertices(std::slice::from_ref(solid), screen_w, screen_h);
                    if vertices.is_empty() {
                        continue;
                    }
                    let buffer =
                        self.device
                            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                                label: Some("ordered ui solid vertex buffer"),
                                contents: bytemuck::cast_slice(&vertices),
                                usage: wgpu::BufferUsages::VERTEX,
                            });
                    pass.set_pipeline(&self.ui_pipeline);
                    pass.set_vertex_buffer(0, buffer.slice(..));
                    pass.draw(0..vertices.len() as u32, 0..1);
                }
            }
        }
    }
}
enum OrderedUi<'a> {
    Image { index: usize, image: &'a UiImage },
    Solid { index: usize, solid: &'a UiSolid },
}

impl OrderedUi<'_> {
    fn depth(&self) -> f32 {
        match self {
            Self::Image { image, .. } => image.depth,
            Self::Solid { solid, .. } => solid.depth,
        }
    }

    fn index(&self) -> usize {
        match self {
            Self::Image { index, .. } | Self::Solid { index, .. } => *index,
        }
    }
}

fn push_ui_image_vertices(
    vertices: &mut Vec<Vertex>,
    image: &UiImage,
    atlas_w: u32,
    atlas_h: u32,
    screen_w: f32,
    screen_h: f32,
) {
    let x0 = image.x;
    let y0 = image.y;
    let x1 = image.x + image.w;
    let y1 = image.y + image.h;

    let u0 = image.src_x as f32 / atlas_w as f32;
    let u1 = (image.src_x + image.src_w) as f32 / atlas_w as f32;
    let v0 = image.src_y as f32 / atlas_h as f32;
    let v1 = (image.src_y + image.src_h) as f32 / atlas_h as f32;

    let p0 = to_ndc(x0, y0, screen_w, screen_h);
    let p1 = to_ndc(x1, y0, screen_w, screen_h);
    let p2 = to_ndc(x1, y1, screen_w, screen_h);
    let p3 = to_ndc(x0, y1, screen_w, screen_h);
    let tint = image.tint;

    vertices.extend_from_slice(&[
        Vertex {
            position: p0,
            uv: [u0, v0],
            color: tint,
        },
        Vertex {
            position: p1,
            uv: [u1, v0],
            color: tint,
        },
        Vertex {
            position: p2,
            uv: [u1, v1],
            color: tint,
        },
        Vertex {
            position: p0,
            uv: [u0, v0],
            color: tint,
        },
        Vertex {
            position: p2,
            uv: [u1, v1],
            color: tint,
        },
        Vertex {
            position: p3,
            uv: [u0, v1],
            color: tint,
        },
    ]);
}

#[allow(clippy::too_many_arguments)]
pub(super) fn push_sprite_vertices(
    vertices: &mut Vec<Vertex>,
    frame: &AtlasFrame,
    atlas_w: u32,
    atlas_h: u32,
    anchor: Vec2,
    scale: f32,
    flip_x: bool,
    bottom_left_anchor: bool,
    tint: [f32; 4],
    screen_w: f32,
    screen_h: f32,
) {
    let (logical_w, logical_h) = frame.display_size();
    let w = logical_w as f32 * scale;
    let h = logical_h as f32 * scale;
    let (x0, y0) = if let Some([source_x, source_y]) = frame.anchor {
        let source_x = if flip_x {
            logical_w as f32 - source_x
        } else {
            source_x
        };
        (anchor.x - source_x * scale, anchor.y - source_y * scale)
    } else if bottom_left_anchor {
        (anchor.x, anchor.y - h)
    } else {
        (anchor.x - w * 0.5, anchor.y - h)
    };
    let x1 = x0 + w;
    let y1 = y0 + h;

    let mut u0 = frame.x as f32 / atlas_w as f32;
    let mut u1 = (frame.x + frame.w) as f32 / atlas_w as f32;
    if flip_x {
        std::mem::swap(&mut u0, &mut u1);
    }
    let v0 = frame.y as f32 / atlas_h as f32;
    let v1 = (frame.y + frame.h) as f32 / atlas_h as f32;

    let p0 = to_ndc(x0, y0, screen_w, screen_h);
    let p1 = to_ndc(x1, y0, screen_w, screen_h);
    let p2 = to_ndc(x1, y1, screen_w, screen_h);
    let p3 = to_ndc(x0, y1, screen_w, screen_h);

    vertices.extend_from_slice(&[
        Vertex {
            position: p0,
            uv: [u0, v0],
            color: tint,
        },
        Vertex {
            position: p1,
            uv: [u1, v0],
            color: tint,
        },
        Vertex {
            position: p2,
            uv: [u1, v1],
            color: tint,
        },
        Vertex {
            position: p0,
            uv: [u0, v0],
            color: tint,
        },
        Vertex {
            position: p2,
            uv: [u1, v1],
            color: tint,
        },
        Vertex {
            position: p3,
            uv: [u0, v1],
            color: tint,
        },
    ]);
}

pub(super) fn to_ndc(x: f32, y: f32, screen_w: f32, screen_h: f32) -> [f32; 2] {
    [(x / screen_w) * 2.0 - 1.0, 1.0 - (y / screen_h) * 2.0]
}

fn build_ui_vertices(solids: &[UiSolid], screen_w: f32, screen_h: f32) -> Vec<UiVertex> {
    let mut sorted: Vec<&UiSolid> = solids.iter().collect();
    sorted.sort_by(|left, right| left.depth.total_cmp(&right.depth));
    let mut vertices = Vec::with_capacity(sorted.len() * 6);
    for solid in sorted {
        let x0 = solid.x;
        let y0 = solid.y;
        let x1 = solid.x + solid.w;
        let y1 = solid.y + solid.h;
        let color = solid.color;
        let p0 = to_ndc(x0, y0, screen_w, screen_h);
        let p1 = to_ndc(x1, y0, screen_w, screen_h);
        let p2 = to_ndc(x1, y1, screen_w, screen_h);
        let p3 = to_ndc(x0, y1, screen_w, screen_h);
        vertices.extend_from_slice(&[
            UiVertex {
                position: p0,
                color,
            },
            UiVertex {
                position: p1,
                color,
            },
            UiVertex {
                position: p2,
                color,
            },
            UiVertex {
                position: p0,
                color,
            },
            UiVertex {
                position: p2,
                color,
            },
            UiVertex {
                position: p3,
                color,
            },
        ]);
    }
    vertices
}
