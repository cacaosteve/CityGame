use super::*;

fn palette_index_provenance(
    image: &RgbaImage,
    palette: &AnimatedPalette,
    replacement_art: bool,
) -> RgbaImage {
    let mut provenance = RgbaImage::new(image.width(), image.height());
    if replacement_art {
        return provenance;
    }
    let mut indices: HashMap<[u8; 3], Option<u8>> = HashMap::new();
    for index in 0..=u8::MAX {
        let color = palette.base_color_for_index(index);
        indices
            .entry([color.r, color.g, color.b])
            .and_modify(|candidate| *candidate = None)
            .or_insert(Some(index));
    }
    for (x, y, pixel) in image.enumerate_pixels() {
        if pixel.0[3] < 128 {
            continue;
        }
        if let Some(Some(index)) = indices.get(&[pixel.0[0], pixel.0[1], pixel.0[2]]) {
            provenance.put_pixel(x, y, image::Rgba([*index, 255, 0, 255]));
        }
    }
    provenance
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "requires the original Mac palette export"]
    fn animated_rgb_collisions_keep_their_prebake_palette_identity() {
        let pack = std::path::PathBuf::from(
            std::env::var_os("CITYGAME_MAC_SHAPE_PACK").expect("CITYGAME_MAC_SHAPE_PACK"),
        );
        let palette = AnimatedPalette::load(&pack).unwrap();
        let mut water =
            RgbaImage::from_pixel(1, 1, palette.base_color_for_index(187).to_rgba().into());
        let water_indices = palette_index_provenance(&water, &palette, false);
        palette.bake_water_sprite_frame(&mut water, 5);
        assert_eq!(
            water.get_pixel(0, 0).0,
            palette.base_color_for_index(79).to_rgba()
        );
        assert_eq!(water_indices.get_pixel(0, 0).0[..2], [187, 255]);

        let mut smoke =
            RgbaImage::from_pixel(1, 1, palette.base_color_for_index(195).to_rgba().into());
        let smoke_indices = palette_index_provenance(&smoke, &palette, false);
        palette.bake_smoke_sprite_frame(&mut smoke, 1201, 0);
        assert_eq!(
            smoke.get_pixel(0, 0).0,
            palette.base_color_for_index(104).to_rgba()
        );
        assert_eq!(smoke_indices.get_pixel(0, 0).0[..2], [195, 255]);
    }

    #[test]
    #[ignore = "requires the extracted original Mac sprite atlases"]
    fn original_mac_atlases_recover_shadow_and_animated_indices() {
        let pack = std::path::PathBuf::from(
            std::env::var_os("CITYGAME_MAC_SHAPE_PACK").expect("CITYGAME_MAC_SHAPE_PACK"),
        );
        let palette = AnimatedPalette::load(&pack).unwrap();
        for index in std::iter::once(79).chain(100..=110).chain([171, 187, 195]) {
            let mut pixel = RgbaImage::new(1, 1);
            pixel.put_pixel(0, 0, palette.base_color_for_index(index).to_rgba().into());
            let provenance = palette_index_provenance(&pixel, &palette, false);
            assert_eq!(provenance.get_pixel(0, 0).0[..2], [index, 255]);
        }
        let mut counts = [0usize; 256];
        for source in crate::assets::load_sprite_sources(&pack).unwrap() {
            let image = source.load_rgba().unwrap();
            let provenance = palette_index_provenance(&image, &palette, false);
            for pixel in provenance.pixels().filter(|pixel| pixel.0[1] != 0) {
                counts[pixel.0[0] as usize] += 1;
            }
        }
        for index in std::iter::once(79).chain(100..=110).chain([171, 187, 195]) {
            assert!(
                counts[index] > 0,
                "palette index {index} absent from source atlases"
            );
        }
    }
}

impl<'window> Renderer<'window> {
    pub(super) fn upload_atlas(&self, source: &AtlasSource) -> Result<GpuAtlas> {
        let started = std::time::Instant::now();
        let mut rgba = source.load_rgba()?;
        let source_indices = self.mac_sprite_pack.then(|| {
            palette_index_provenance(
                &rgba,
                &self.palette,
                source.name.starts_with("replacement_"),
            )
        });
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
        let index_bind_group = source_indices.map(|source_indices| {
            let mut index_pixels =
                image::RgbaImage::from_pixel(width, height, image::Rgba([0, 0, 0, 0]));
            image::imageops::replace(&mut index_pixels, &source_indices, 0, 0);
            let index_texture = self.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("sprite source palette indices"),
                size: texture_size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });
            self.queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &index_texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                &index_pixels,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(width * 4),
                    rows_per_image: Some(height),
                },
                texture_size,
            );
            let index_view = index_texture.create_view(&Default::default());
            self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("sprite source palette index bind group"),
                layout: &self.mac_shadow_index_layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&index_view),
                }],
            })
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
            index_bind_group,
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

        let source_index_view = built.source_indices.as_ref().map(|indices| {
            let index_texture = self.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("animated sprite source indices"),
                size: texture_size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });
            self.queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &index_texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                indices,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(built.width * 4),
                    rows_per_image: Some(built.height),
                },
                texture_size,
            );
            index_texture.create_view(&Default::default())
        });
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
        let index_bind_group = source_index_view.map(|view| {
            self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("animated sprite source index bind group"),
                layout: &self.mac_shadow_index_layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                }],
            })
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
            index_bind_group,
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
            index_bind_group: None,
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
            index_bind_group: None,
        })
    }
}
