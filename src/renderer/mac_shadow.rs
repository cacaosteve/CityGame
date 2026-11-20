//! Ordered, background-dependent compositing for the original Mac shadow.

use super::*;

pub(super) fn create_pipeline(
    device: &wgpu::Device,
    atlas_layout: &wgpu::BindGroupLayout,
    index_layout: &wgpu::BindGroupLayout,
    format: wgpu::TextureFormat,
) -> (wgpu::RenderPipeline, wgpu::BindGroupLayout) {
    let background_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("Mac shadow background"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: false },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 2,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Uint,
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
        ],
    });
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("Mac palette shadow shader"),
        source: wgpu::ShaderSource::Wgsl(include_str!("../mac_shadow.wgsl").into()),
    });
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("Mac palette shadow pipeline layout"),
        bind_group_layouts: &[Some(atlas_layout), Some(&background_layout), Some(index_layout)],
        immediate_size: 0,
    });
    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("Mac palette shadow pipeline"),
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            buffers: &[Vertex::layout()],
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs_main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            targets: &[
                Some(wgpu::ColorTargetState { format, blend: None, write_mask: wgpu::ColorWrites::ALL }),
                Some(wgpu::ColorTargetState { format: wgpu::TextureFormat::Rgba8Uint, blend: None, write_mask: wgpu::ColorWrites::ALL }),
            ],
        }),
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        multiview_mask: None,
        cache: None,
    });
    (pipeline, background_layout)
}

fn linear_component(byte: u8) -> f32 {
    let c = f32::from(byte) / 255.0;
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

impl Renderer<'_> {
    /// Draw the world in depth order, copying the background before each
    /// shadow so its palette operation can sample the already-drawn pixels.
    pub(super) fn render_world_with_mac_shadows(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        output: &wgpu::TextureView,
        sprites: &[&RenderSprite],
        clear: wgpu::Color,
    ) {
        ShadowContext {
            device: &self.device,
            config: &self.config,
            pipeline: &self.pipeline,
            mac_world_pipeline: &self.mac_world_pipeline,
            mac_shadow_pipeline: &self.mac_shadow_pipeline,
            mac_shadow_background_layout: &self.mac_shadow_background_layout,
            bind_group_layout: &self.bind_group_layout,
            unknown_index_bind_group: &self.mac_shadow_unknown_index_bind_group,
            atlases: &self.atlases,
            palette: &self.palette,
        }
        .render_world_with_mac_shadows(encoder, output, sprites, clear);
    }
}

struct ShadowContext<'a> {
    device: &'a wgpu::Device,
    config: &'a wgpu::SurfaceConfiguration,
    pipeline: &'a wgpu::RenderPipeline,
    mac_world_pipeline: &'a wgpu::RenderPipeline,
    mac_shadow_pipeline: &'a wgpu::RenderPipeline,
    mac_shadow_background_layout: &'a wgpu::BindGroupLayout,
    bind_group_layout: &'a wgpu::BindGroupLayout,
    unknown_index_bind_group: &'a wgpu::BindGroup,
    atlases: &'a [GpuAtlas],
    palette: &'a AnimatedPalette,
}

impl ShadowContext<'_> {
    fn build_vertices_from_refs(
        &self,
        sprites: &[&RenderSprite],
    ) -> (Vec<Vertex>, Vec<(usize, u32, u32)>) {
        Renderer::build_vertices_for_atlases(
            self.atlases,
            self.config.width,
            self.config.height,
            sprites,
        )
    }

    fn render_world_with_mac_shadows(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        output: &wgpu::TextureView,
        sprites: &[&RenderSprite],
        clear: wgpu::Color,
    ) {
        let size = wgpu::Extent3d {
            width: self.config.width,
            height: self.config.height,
            depth_or_array_layers: 1,
        };
        let textures: Vec<_> = (0..2)
            .map(|_| {
                self.device.create_texture(&wgpu::TextureDescriptor {
                    label: Some("Mac shadow intermediate"),
                    size,
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: self.config.format,
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                        | wgpu::TextureUsages::TEXTURE_BINDING
                        | wgpu::TextureUsages::COPY_SRC
                        | wgpu::TextureUsages::COPY_DST,
                    view_formats: &[],
                })
            })
            .collect();
        let views: Vec<_> = textures
            .iter()
            .map(|texture| texture.create_view(&wgpu::TextureViewDescriptor::default()))
            .collect();
        let index_textures: Vec<_> = (0..2)
            .map(|_| self.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("Mac palette index intermediate"),
                size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Uint,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING
                    | wgpu::TextureUsages::COPY_SRC
                    | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            }))
            .collect();
        let index_views: Vec<_> = index_textures
            .iter()
            .map(|texture| texture.create_view(&wgpu::TextureViewDescriptor::default()))
            .collect();
        let indices = [
            0x4f, 0x64, 0x65, 0x66, 0x67, 0x68, 0x69, 0x6a, 0x6b, 0x6c, 0x6d, 0x6e, 0x54, 0x6e,
        ];
        let colors: [[f32; 4]; 14] = indices.map(|index| {
            let color = self.palette.color_for_index(index, 0);
            [
                linear_component(color.r),
                linear_component(color.g),
                linear_component(color.b),
                1.0,
            ]
        });
        let palette_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Mac shadow palette"),
                contents: bytemuck::cast_slice(&colors),
                usage: wgpu::BufferUsages::UNIFORM,
            });
        let backgrounds: Vec<_> = views
            .iter()
            .enumerate()
            .map(|(index, view)| {
                self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("Mac shadow sampled background"),
                    layout: &self.mac_shadow_background_layout,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: wgpu::BindingResource::TextureView(view),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: palette_buffer.as_entire_binding(),
                        },
                        wgpu::BindGroupEntry {
                            binding: 2,
                            resource: wgpu::BindingResource::TextureView(&index_views[index]),
                        },
                    ],
                })
            })
            .collect();
        let mut ordered = sprites.to_vec();
        ordered.sort_by(|a, b| a.depth.total_cmp(&b.depth));
        let mut current = 0usize;
        let mut start = 0usize;
        let mut first = true;
        for (index, sprite) in ordered.iter().enumerate() {
            if sprite.effect != SpriteEffect::MacPaletteShadow {
                continue;
            }
            self.draw_mac_world_span(
                encoder,
                &views[current],
                &index_views[current],
                &ordered[start..index],
                first.then_some(clear),
            );
            first = false;
            let next = 1 - current;
            encoder.copy_texture_to_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &textures[current],
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                wgpu::TexelCopyTextureInfo {
                    texture: &textures[next],
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                size,
            );
            encoder.copy_texture_to_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &index_textures[current],
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                wgpu::TexelCopyTextureInfo {
                    texture: &index_textures[next],
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                size,
            );
            let (vertices, ranges) = self.build_vertices_from_refs(&[*sprite]);
            if !vertices.is_empty() {
                let buffer = self
                    .device
                    .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("Mac shadow mask vertices"),
                        contents: bytemuck::cast_slice(&vertices),
                        usage: wgpu::BufferUsages::VERTEX,
                    });
                let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("Mac palette shadow"),
                    color_attachments: &[
                        Some(wgpu::RenderPassColorAttachment {
                            view: &views[next], resolve_target: None, depth_slice: None,
                            ops: wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store },
                        }),
                        Some(wgpu::RenderPassColorAttachment {
                            view: &index_views[next], resolve_target: None, depth_slice: None,
                            ops: wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store },
                        }),
                    ],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                    multiview_mask: None,
                });
                pass.set_pipeline(&self.mac_shadow_pipeline);
                pass.set_vertex_buffer(0, buffer.slice(..));
                pass.set_bind_group(1, &backgrounds[current], &[]);
                for (atlas, first, count) in ranges {
                    if let Some(atlas) = self.atlases.get(atlas) {
                        pass.set_bind_group(0, &atlas.bind_group, &[]);
                        pass.set_bind_group(2, atlas.index_bind_group.as_ref().unwrap_or(self.unknown_index_bind_group), &[]);
                        pass.draw(first..first + count, 0..1);
                    }
                }
            }
            current = next;
            start = index + 1;
        }
        self.draw_mac_world_span(
            encoder,
            &views[current],
            &index_views[current],
            &ordered[start..],
            first.then_some(clear),
        );

        let sampler = self.device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Mac shadow output sampler"),
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });
        let bind = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Mac shadow composed output"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&views[current]),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });
        let vertex = |x, y, u, v| Vertex {
            position: [x, y],
            uv: [u, v],
            color: [1.0; 4],
        };
        let vertices = [
            vertex(-1.0, 1.0, 0.0, 0.0),
            vertex(1.0, 1.0, 1.0, 0.0),
            vertex(1.0, -1.0, 1.0, 1.0),
            vertex(-1.0, 1.0, 0.0, 0.0),
            vertex(1.0, -1.0, 1.0, 1.0),
            vertex(-1.0, -1.0, 0.0, 1.0),
        ];
        let buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Mac shadow output vertices"),
                contents: bytemuck::cast_slice(&vertices),
                usage: wgpu::BufferUsages::VERTEX,
            });
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Mac shadow output"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: output,
                resolve_target: None,
                depth_slice: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(clear),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &bind, &[]);
        pass.set_vertex_buffer(0, buffer.slice(..));
        pass.draw(0..6, 0..1);
    }

    fn draw_mac_world_span(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        target_indices: &wgpu::TextureView,
        sprites: &[&RenderSprite],
        clear: Option<wgpu::Color>,
    ) {
        if sprites.is_empty() && clear.is_none() {
            return;
        }
        let (vertices, ranges) = self.build_vertices_from_refs(sprites);
        let buffer = (!vertices.is_empty()).then(|| {
            self.device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Mac shadow world vertices"),
                    contents: bytemuck::cast_slice(&vertices),
                    usage: wgpu::BufferUsages::VERTEX,
                })
        });
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Mac shadow world segment"),
            color_attachments: &[
                Some(wgpu::RenderPassColorAttachment {
                    view: target, resolve_target: None, depth_slice: None,
                    ops: wgpu::Operations { load: clear.map_or(wgpu::LoadOp::Load, wgpu::LoadOp::Clear), store: wgpu::StoreOp::Store },
                }),
                Some(wgpu::RenderPassColorAttachment {
                    view: target_indices, resolve_target: None, depth_slice: None,
                    ops: wgpu::Operations { load: clear.map_or(wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT), |_| wgpu::LoadOp::Load), store: wgpu::StoreOp::Store },
                }),
            ],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        if let Some(buffer) = &buffer {
            pass.set_pipeline(&self.mac_world_pipeline);
            pass.set_vertex_buffer(0, buffer.slice(..));
            for (atlas, first, count) in ranges {
                if let Some(atlas) = self.atlases.get(atlas) {
                    pass.set_bind_group(0, &atlas.bind_group, &[]);
                    pass.set_bind_group(1, atlas.index_bind_group.as_ref().unwrap_or(self.unknown_index_bind_group), &[]);
                    pass.draw(first..first + count, 0..1);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "requires original Mac palette and animated-index raster replay"]
    fn animated_pixels_can_share_rgb_with_shadowed_palette_indices() {
        let root =
            std::path::PathBuf::from(std::env::var_os("CITYGAME_MAC_SELECTOR11_ACCEPTED").unwrap());
        let pack = std::path::PathBuf::from(std::env::var_os("CITYGAME_MAC_SHAPE_PACK").unwrap());
        let palette = AnimatedPalette::load(pack).unwrap();
        let mut water = RgbaImage::from_pixel(1, 1, image::Rgba([28, 28, 0, 255]));
        palette.bake_water_sprite_frame(&mut water, 5);
        assert_eq!(
            water.get_pixel(0, 0).0,
            palette.color_for_index(0x4f, 0).to_rgba()
        );
        let mut smoke = RgbaImage::from_pixel(1, 1, image::Rgba([53, 28, 0, 255]));
        palette.bake_smoke_sprite_frame(&mut smoke, 1201, 0);
        assert_eq!(
            smoke.get_pixel(0, 0).0,
            palette.color_for_index(0x68, 0).to_rgba()
        );

        let rows: Vec<serde_json::Value> = serde_json::from_slice(
            &std::fs::read(root.join("shadow-animated-index.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(rows.len(), 2);
        let decode = |value: &str| -> Vec<u8> {
            value
                .as_bytes()
                .chunks_exact(2)
                .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
                .collect()
        };
        for row in rows {
            let before = decode(row["before"].as_str().unwrap());
            let after = decode(row["after"].as_str().unwrap());
            let coverage = decode(row["coverage_after"].as_str().unwrap());
            let mut water_changed = 0;
            let mut ground_changed = 0;
            let mut covered_animated = [0; 3];
            for ((old, new), mask) in before.into_iter().zip(after).zip(coverage) {
                match old {
                    0xab | 0xbb | 0xc3 => {
                        assert_eq!(old, new, "animated index changed: {row}");
                        if mask == 0x54 {
                            covered_animated[match old {
                                0xab => 0,
                                0xbb => 1,
                                _ => 2,
                            }] += 1;
                        }
                    }
                    0x4f if new == 0x54 => water_changed += 1,
                    0x68 if new == 0x6e => ground_changed += 1,
                    _ => (),
                }
            }
            assert!(water_changed > 0 && ground_changed > 0, "{row}");
            assert!(covered_animated.into_iter().all(|count| count > 0), "{row}");
        }
    }

    #[test]
    #[ignore = "requires original Mac sprite pack and raster replay"]
    fn original_shadow_raster_matches_imported_sprite_mask() {
        let root =
            std::path::PathBuf::from(std::env::var_os("CITYGAME_MAC_SELECTOR11_ACCEPTED").unwrap());
        let pack = std::path::PathBuf::from(std::env::var_os("CITYGAME_MAC_SHAPE_PACK").unwrap());
        let atlas = pack.join("sprites/SPRT_00128_ShapeData");
        let manifest: crate::assets::AtlasManifest =
            serde_json::from_slice(&std::fs::read(atlas.join("atlas.json")).unwrap()).unwrap();
        let frame = manifest
            .frames
            .iter()
            .find(|frame| frame.id == 361)
            .unwrap();
        let image = image::open(atlas.join(&manifest.image)).unwrap().to_rgba8();
        let rows: Vec<serde_json::Value> = serde_json::from_slice(
            &std::fs::read(root.join("shadow-raster-reference.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(rows.len(), 8);
        let bytes = |s: &str| -> Vec<u8> {
            s.as_bytes()
                .chunks_exact(2)
                .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
                .collect()
        };
        for row in rows {
            let mut actual = bytes(row["before"].as_str().unwrap());
            let expected = bytes(row["after"].as_str().unwrap());
            let width = row["width"].as_u64().unwrap() as i32;
            let height = row["height"].as_u64().unwrap() as i32;
            let origin_x = row["x"].as_i64().unwrap() as i32;
            let origin_y = row["y"].as_i64().unwrap() as i32;
            let flip = row["flip"].as_bool().unwrap();
            for sy in 0..frame.h {
                for sx in 0..frame.w {
                    let x = origin_x
                        + if flip {
                            (frame.w - 1 - sx) as i32
                        } else {
                            sx as i32
                        };
                    let y = origin_y + sy as i32;
                    if x < 0
                        || y < 0
                        || x >= width
                        || y >= height
                        || image.get_pixel(frame.x + sx, frame.y + sy).0[3] < 128
                    {
                        continue;
                    }
                    let pixel = &mut actual[(y * width + x) as usize];
                    *pixel = match *pixel {
                        0x4f => 0x54,
                        0x64..=0x6e => 0x6e,
                        value => value,
                    };
                }
            }
            assert_eq!(actual, expected, "{row}");
        }
    }

    #[test]
    #[ignore = "requires a working wgpu adapter"]
    fn gpu_shadow_maps_only_matching_background_inside_mask() {
        let instance = wgpu::Instance::new(renderer_instance_descriptor());
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::LowPower,
            force_fallback_adapter: false,
            compatible_surface: None,
        }))
        .expect("wgpu adapter");
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            required_limits: wgpu::Limits::default(),
            required_features: wgpu::Features::empty(),
            memory_hints: wgpu::MemoryHints::Performance,
            experimental_features: wgpu::ExperimentalFeatures::disabled(),
            trace: wgpu::Trace::Off,
            label: Some("Mac shadow test device"),
        }))
        .unwrap();
        let atlas_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let format = wgpu::TextureFormat::Rgba8UnormSrgb;
        let index_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor { label: None,
            entries: &[wgpu::BindGroupLayoutEntry { binding: 0, visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture { sample_type: wgpu::TextureSampleType::Float { filterable: false },
                    view_dimension: wgpu::TextureViewDimension::D2, multisampled: false }, count: None }] });
        let (pipeline, background_layout) = create_pipeline(&device, &atlas_layout, &index_layout, format);
        let extent = wgpu::Extent3d {
            width: 4,
            height: 1,
            depth_or_array_layers: 1,
        };
        let texture = |label, usage| {
            device.create_texture(&wgpu::TextureDescriptor {
                label: Some(label),
                size: extent,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage,
                view_formats: &[],
            })
        };
        let bg = texture(
            "test background",
            wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_DST
                | wgpu::TextureUsages::COPY_SRC,
        );
        let mask = texture(
            "test mask",
            wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        );
        let index_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("test original sprite indices"), size: extent, mip_level_count: 1,
            sample_count: 1, dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let destination_indices = [79, 1, 0, 255, 104, 1, 0, 255, 187, 1, 0, 255, 195, 1, 0, 255];
        let bg_index = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("test background palette indices"), size: extent, mip_level_count: 1,
            sample_count: 1, dimension: wgpu::TextureDimension::D2, format: wgpu::TextureFormat::Rgba8Uint,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let out_index = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("test output palette indices"), size: extent, mip_level_count: 1,
            sample_count: 1, dimension: wgpu::TextureDimension::D2, format: wgpu::TextureFormat::Rgba8Uint,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let out = texture(
            "test output",
            wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::COPY_DST,
        );
        let copy_info = |texture| wgpu::TexelCopyTextureInfo {
            texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        };
        let layout = wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(16),
            rows_per_image: Some(1),
        };
        let pixels = [
            35, 39, 255, 255, 155, 135, 71, 255, 35, 39, 255, 255, 155, 135, 71, 255,
        ];
        let masks = [
            255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
        ];
        let indices = [79,255,0,255, 104,255,0,255, 187,255,0,255, 195,255,0,255];
        queue.write_texture(copy_info(&bg), &pixels, layout, extent);
        queue.write_texture(copy_info(&mask), &masks, layout, extent);
        queue.write_texture(copy_info(&index_texture), &indices, layout, extent);
        queue.write_texture(copy_info(&bg_index), &destination_indices, layout, extent);
        let mut colors = [[0.0f32; 4]; 14];
        let color = |r, g, b| {
            [
                linear_component(r),
                linear_component(g),
                linear_component(b),
                1.0,
            ]
        };
        colors[0] = color(35, 39, 255);
        colors[1] = color(155, 135, 71);
        colors[12] = color(0, 0, 203);
        colors[13] = color(95, 59, 19);
        let palette = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None,
            contents: bytemuck::cast_slice(&colors),
            usage: wgpu::BufferUsages::UNIFORM,
        });
        let bg_view = bg.create_view(&wgpu::TextureViewDescriptor::default());
        let mask_view = mask.create_view(&wgpu::TextureViewDescriptor::default());
        let out_view = out.create_view(&wgpu::TextureViewDescriptor::default());
        let index_view = index_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let bg_index_view = bg_index.create_view(&wgpu::TextureViewDescriptor::default());
        let out_index_view = out_index.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor::default());
        let atlas_bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &atlas_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&mask_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });
        let bg_bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &background_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&bg_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: palette.as_entire_binding(),
                },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(&bg_index_view) },
            ],
        });
        let index_bind = device.create_bind_group(&wgpu::BindGroupDescriptor { label: None, layout: &index_layout,
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&index_view) }] });
        let vertex = |x, y, u, v| Vertex {
            position: [x, y],
            uv: [u, v],
            color: [1.0; 4],
        };
        let vertices = [
            vertex(-1.0, 1.0, 0.0, 0.0),
            vertex(1.0, 1.0, 1.0, 0.0),
            vertex(1.0, -1.0, 1.0, 1.0),
            vertex(-1.0, 1.0, 0.0, 0.0),
            vertex(1.0, -1.0, 1.0, 1.0),
            vertex(-1.0, -1.0, 0.0, 1.0),
        ];
        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None,
            contents: bytemuck::cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let readback = device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: 256,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder =
            device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        encoder.copy_texture_to_texture(copy_info(&bg), copy_info(&out), extent);
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[
                    Some(wgpu::RenderPassColorAttachment { view: &out_view, resolve_target: None, depth_slice: None, ops: wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store } }),
                    Some(wgpu::RenderPassColorAttachment { view: &out_index_view, resolve_target: None, depth_slice: None, ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT), store: wgpu::StoreOp::Store } }),
                ],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&pipeline);
            pass.set_bind_group(0, &atlas_bind, &[]);
            pass.set_bind_group(1, &bg_bind, &[]);
            pass.set_bind_group(2, &index_bind, &[]);
            pass.set_vertex_buffer(0, vertex_buffer.slice(..));
            pass.draw(0..6, 0..1);
        }
        encoder.copy_texture_to_buffer(
            copy_info(&out),
            wgpu::TexelCopyBufferInfo {
                buffer: &readback,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(256),
                    rows_per_image: Some(1),
                },
            },
            extent,
        );
        queue.submit(std::iter::once(encoder.finish()));
        let (tx, rx) = std::sync::mpsc::channel();
        readback
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |result| {
                let _ = tx.send(result);
            });
        device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
        rx.recv().unwrap().unwrap();
        let mapped = readback.slice(..).get_mapped_range();
        assert_eq!(
            &mapped[..16],
            &[
                0, 0, 203, 255, 95, 59, 19, 255, 35, 39, 255, 255, 155, 135, 71, 255
            ]
        );
    }

    #[test]
    #[ignore = "requires a working wgpu adapter"]
    fn renderer_composites_multiple_ordered_shadows() {
        let instance = wgpu::Instance::new(renderer_instance_descriptor());
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::LowPower,
            force_fallback_adapter: false,
            compatible_surface: None,
        }))
        .unwrap();
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            required_limits: wgpu::Limits::default(),
            required_features: wgpu::Features::empty(),
            memory_hints: wgpu::MemoryHints::Performance,
            experimental_features: wgpu::ExperimentalFeatures::disabled(),
            trace: wgpu::Trace::Off,
            label: None,
        }))
        .unwrap();
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let format = wgpu::TextureFormat::Rgba8UnormSrgb;
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: None,
            source: wgpu::ShaderSource::Wgsl(include_str!("../shader.wgsl").into()),
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: None,
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[Vertex::layout()],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: Default::default(),
            depth_stencil: None,
            multisample: Default::default(),
            multiview_mask: None,
            cache: None,
        });
        let mac_world_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: None,
            source: wgpu::ShaderSource::Wgsl(include_str!("../mac_world.wgsl").into()),
        });
        let index_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor { label: None,
            entries: &[wgpu::BindGroupLayoutEntry { binding: 0, visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture { sample_type: wgpu::TextureSampleType::Float { filterable: false },
                    view_dimension: wgpu::TextureViewDimension::D2, multisampled: false }, count: None }] });
        let (shadow_pipeline, shadow_layout) = create_pipeline(&device, &layout, &index_layout, format);
        let mac_world_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None, bind_group_layouts: &[Some(&layout), Some(&index_layout)], immediate_size: 0,
        });
        let mac_world_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: None, layout: Some(&mac_world_layout),
            vertex: wgpu::VertexState { module: &mac_world_shader, entry_point: Some("vs_main"), compilation_options: Default::default(), buffers: &[Vertex::layout()] },
            fragment: Some(wgpu::FragmentState { module: &mac_world_shader, entry_point: Some("fs_main"), compilation_options: Default::default(), targets: &[
                Some(wgpu::ColorTargetState { format, blend: Some(wgpu::BlendState::ALPHA_BLENDING), write_mask: wgpu::ColorWrites::ALL }),
                Some(wgpu::ColorTargetState { format: wgpu::TextureFormat::Rgba8Uint, blend: None, write_mask: wgpu::ColorWrites::ALL }),
            ] }),
            primitive: Default::default(), depth_stencil: None, multisample: Default::default(), multiview_mask: None, cache: None,
        });
        let make_atlas = |id: u32, pixels: &[u8], width: u32| {
            let texture = device.create_texture(&wgpu::TextureDescriptor {
                label: None,
                size: wgpu::Extent3d {
                    width,
                    height: 1,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });
            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                pixels,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(width * 4),
                    rows_per_image: Some(1),
                },
                wgpu::Extent3d {
                    width,
                    height: 1,
                    depth_or_array_layers: 1,
                },
            );
            let view = texture.create_view(&Default::default());
            let sampler = device.create_sampler(&wgpu::SamplerDescriptor::default());
            let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: None,
                layout: &layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&sampler),
                    },
                ],
            });
            let index_pixels: Vec<u8> = pixels.chunks_exact(4).flat_map(|pixel| {
                if pixel[3] == 0 || !matches!(pixel[0], 79 | 100) { [0, 0, 0, 255] }
                else { [pixel[0], 255, 0, 255] }
            }).collect();
            let index_texture = device.create_texture(&wgpu::TextureDescriptor {
                label: None, size: wgpu::Extent3d { width, height: 1, depth_or_array_layers: 1 },
                mip_level_count: 1, sample_count: 1, dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST, view_formats: &[],
            });
            queue.write_texture(wgpu::TexelCopyTextureInfo { texture: &index_texture, mip_level: 0,
                origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All }, &index_pixels,
                wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(width * 4), rows_per_image: Some(1) },
                wgpu::Extent3d { width, height: 1, depth_or_array_layers: 1 });
            let index_view = index_texture.create_view(&Default::default());
            let index_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor { label: None,
                layout: &index_layout, entries: &[wgpu::BindGroupEntry { binding: 0,
                    resource: wgpu::BindingResource::TextureView(&index_view) }] });
            let frame = AtlasFrame {
                file: String::new(),
                index: 0,
                id,
                source_offset: 0,
                x: 0,
                y: 0,
                w: width,
                h: 1,
                logical_size: None,
                anchor: None,
            };
            GpuAtlas {
                width,
                height: 1,
                frames: [(id, frame)].into_iter().collect(),
                animation_counts: HashMap::new(),
                ids: vec![id],
                bind_group,
                index_bind_group: Some(index_bind_group),
            }
        };
        let atlases = [
            make_atlas(
                0,
                &[
                    79, 79, 79, 255, 100, 100, 100, 255, 255, 0, 0, 255, 100, 100, 100, 255,
                ],
                4,
            ),
            make_atlas(
                1,
                &[
                    255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 0, 0, 0, 0,
                ],
                4,
            ),
            make_atlas(2, &[79, 79, 79, 255], 1),
        ];
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: 4,
            height: 1,
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: wgpu::CompositeAlphaMode::Auto,
            desired_maximum_frame_latency: 2,
            view_formats: vec![],
        };
        let palette =
            AnimatedPalette::load(std::env::temp_dir().join("citygame-missing-neutral-palette"))
                .unwrap();
        let unknown_index = device.create_texture(&wgpu::TextureDescriptor {
            label: None, size: wgpu::Extent3d { width: 1, height: 1, depth_or_array_layers: 1 },
            mip_level_count: 1, sample_count: 1, dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST, view_formats: &[],
        });
        let unknown_index_view = unknown_index.create_view(&Default::default());
        let unknown_index_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None, layout: &index_layout,
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&unknown_index_view) }],
        });
        let context = ShadowContext {
            device: &device,
            config: &config,
            pipeline: &pipeline,
            mac_world_pipeline: &mac_world_pipeline,
            mac_shadow_pipeline: &shadow_pipeline,
            mac_shadow_background_layout: &shadow_layout,
            bind_group_layout: &layout,
            unknown_index_bind_group: &unknown_index_bind_group,
            atlases: &atlases,
            palette: &palette,
        };
        let sprite = |atlas, id, x, depth, effect| RenderSprite {
            atlas,
            sprite_id: id,
            anchor: Vec2::new(x, 1.0),
            scale: 1.0,
            depth,
            tint: [1.0; 4],
            flip_x: false,
            bottom_left_anchor: false,
            effect,
        };
        let sprites = [
            sprite(1, 1, 2.0, 3.0, SpriteEffect::MacPaletteShadow),
            sprite(0, 0, 2.0, 0.0, SpriteEffect::Normal),
            sprite(2, 2, 0.5, 2.0, SpriteEffect::Normal),
            sprite(1, 1, 2.0, 1.0, SpriteEffect::MacPaletteShadow),
        ];
        let refs: Vec<_> = sprites.iter().collect();
        let extent = wgpu::Extent3d {
            width: 4,
            height: 1,
            depth_or_array_layers: 1,
        };
        let output = device.create_texture(&wgpu::TextureDescriptor {
            label: None,
            size: extent,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let readback = device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: 256,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = device.create_command_encoder(&Default::default());
        context.render_world_with_mac_shadows(
            &mut encoder,
            &output.create_view(&Default::default()),
            &refs,
            wgpu::Color::BLACK,
        );
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &output,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &readback,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(256),
                    rows_per_image: Some(1),
                },
            },
            extent,
        );
        queue.submit(std::iter::once(encoder.finish()));
        let (tx, rx) = std::sync::mpsc::channel();
        readback
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |result| {
                let _ = tx.send(result);
            });
        device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
        rx.recv().unwrap().unwrap();
        let mapped = readback.slice(..).get_mapped_range();
        assert_eq!(
            &mapped[..16],
            &[
                84, 84, 84, 255, 110, 110, 110, 255, 255, 0, 0, 255, 100, 100, 100, 255
            ]
        );
    }
}
