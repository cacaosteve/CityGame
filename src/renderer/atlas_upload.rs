use super::*;

impl<'window> Renderer<'window> {
    pub(super) fn upload_atlas(&self, source: &AtlasSource) -> Result<GpuAtlas> {
        let started = std::time::Instant::now();
        let mut rgba = source.load_rgba()?;
        let (mut width, mut height) = rgba.dimensions();
        let mut frames: HashMap<u32, AtlasFrame> = source
            .manifest
            .frames
            .iter()
            .cloned()
            .map(|frame| (frame.id, frame))
            .collect();
        let mut animation_counts = HashMap::new();
        normalize_smoke_building_sprites(&mut rgba, &source.manifest.frames);
        bake_base_animated_sprites(
            &mut rgba,
            &mut animation_counts,
            &self.palette,
            &source.manifest.frames,
        );
        bake_traffic_variants_for_source(
            source,
            &mut rgba,
            &mut frames,
            &self.palette,
            &mut width,
            &mut height,
            &mut animation_counts,
        );
        let texture_size = wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some(&format!("{} texture", source.name)),
            size: texture_size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        self.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &rgba,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(width * 4),
                rows_per_image: Some(height),
            },
            texture_size,
        );

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let min_filter = atlas_min_filter(frames.values());
        let sampler = self.device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("pixel sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        });
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some(&format!("{} bind group", source.name)),
            layout: &self.bind_group_layout,
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

        let mut ids: Vec<u32> = frames.keys().copied().collect();
        ids.sort_unstable();
        ids.retain(|id| *id < ANIM_SPRITE_STRIDE);

        log::info!(
            "atlas {}: {}x{} ({} ms)",
            source.name,
            width,
            height,
            started.elapsed().as_millis()
        );

        Ok(GpuAtlas {
            width,
            height,
            frames,
            animation_counts,
            ids,
            bind_group,
        })
    }

    pub(super) fn upload_animation_atlas(
        &self,
        sources: &[AtlasSource],
    ) -> Result<Option<GpuAtlas>> {
        let Some(built) = build_animation_atlas(sources, &self.palette)? else {
            return Ok(None);
        };

        let started = std::time::Instant::now();
        let texture_size = wgpu::Extent3d {
            width: built.width,
            height: built.height,
            depth_or_array_layers: 1,
        };
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("water smoke animation atlas texture"),
            size: texture_size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        self.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &built.rgba,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(built.width * 4),
                rows_per_image: Some(built.height),
            },
            texture_size,
        );

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let min_filter = atlas_min_filter(built.frames.values());
        let sampler = self.device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("water smoke animation atlas sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        });
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("water smoke animation atlas bind group"),
            layout: &self.bind_group_layout,
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

        log::info!(
            "atlas water_smoke_animation: {}x{} ({} sprites, {} ms)",
            built.width,
            built.height,
            built.ids.len(),
            started.elapsed().as_millis()
        );

        Ok(Some(GpuAtlas {
            width: built.width,
            height: built.height,
            frames: built.frames,
            animation_counts: built.animation_counts,
            ids: built.ids,
            bind_group,
        }))
    }

    pub(super) fn upload_traffic_atlas(&self, sources: &[AtlasSource]) -> Result<Option<GpuAtlas>> {
        let Some(built) = build_traffic_atlas(sources, &self.palette)? else {
            return Ok(None);
        };

        let started = std::time::Instant::now();
        let texture_size = wgpu::Extent3d {
            width: built.width,
            height: built.height,
            depth_or_array_layers: 1,
        };
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("traffic animation atlas texture"),
            size: texture_size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        self.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &built.rgba,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(built.width * 4),
                rows_per_image: Some(built.height),
            },
            texture_size,
        );

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let min_filter = atlas_min_filter(built.frames.values());
        let sampler = self.device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("traffic animation atlas sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        });
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("traffic animation atlas bind group"),
            layout: &self.bind_group_layout,
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

        log::info!(
            "atlas traffic_animation: {}x{} ({} sprites, {} ms)",
            built.width,
            built.height,
            built.ids.len(),
            started.elapsed().as_millis()
        );

        Ok(Some(GpuAtlas {
            width: built.width,
            height: built.height,
            frames: built.frames,
            animation_counts: built.animation_counts,
            ids: built.ids,
            bind_group,
        }))
    }

    pub(super) fn upload_ui_atlas(&self, source: &AtlasSource) -> Result<GpuAtlas> {
        let rgba = source.load_rgba()?;
        let (width, height) = rgba.dimensions();
        let frames: HashMap<u32, AtlasFrame> = source
            .manifest
            .frames
            .iter()
            .cloned()
            .map(|frame| (frame.id, frame))
            .collect();

        let texture_size = wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some(&format!("{} ui texture", source.name)),
            size: texture_size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        self.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &rgba,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(width * 4),
                rows_per_image: Some(height),
            },
            texture_size,
        );

        let texture_view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let filter = if source.name == "newspaper_type" {
            wgpu::FilterMode::Linear
        } else {
            wgpu::FilterMode::Nearest
        };
        let sampler = self.device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some(&format!("{} ui sampler", source.name)),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: filter,
            min_filter: filter,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        });

        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some(&format!("{} ui bind group", source.name)),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });

        let mut ids: Vec<u32> = frames.keys().copied().collect();
        ids.sort_unstable();

        Ok(GpuAtlas {
            width,
            height,
            frames,
            animation_counts: HashMap::new(),
            ids,
            bind_group,
        })
    }
}
