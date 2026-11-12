use crate::assets::{
    AtlasFrame, AtlasSource, load_replacement_sprite_source, load_sprite_sources,
    load_ui_palette_sources,
};
use crate::palette::{
    ANIM_SPRITE_STRIDE, AnimatedPalette, TRAFFIC_CAR_COLOR_INDICES, animated_sprite_id,
    is_smoke_plume_pixel, is_smoke_sprite, is_traffic_sprite, is_water_sprite,
    should_animate_sprite, traffic_animation_band_count, traffic_color_index_for_slot,
    traffic_colored_sprite_id,
};
use anyhow::{Context, Result};
use bytemuck::{Pod, Zeroable};
use glam::Vec2;
use image::RgbaImage;
use sdl3::video::Window;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use wgpu::util::DeviceExt;
use wgpu::{InstanceDescriptor, SurfaceConfiguration};

mod atlas_upload;
mod geometry;
mod sprite_baking;
use sprite_baking::*;

/// Sprites at or above this depth are treated as HUD icons and drawn after UI solids.
const UI_SPRITE_DEPTH_MIN: f32 = 1_000_000.0;
const ANIMATION_ATLAS_MAX_WIDTH: u32 = 4096;
const TRAFFIC_ATLAS_MAX_WIDTH: u32 = 4096;
const ANIMATION_ATLAS_MAX_HEIGHT: u32 = 16_384;
const TRAFFIC_ATLAS_ANIM_FRAMES: u8 = 8;

#[derive(Debug, Clone)]
pub struct RenderSprite {
    pub atlas: usize,
    pub sprite_id: u32,
    pub anchor: Vec2,
    pub scale: f32,
    pub depth: f32,
    pub tint: [f32; 4],
    pub flip_x: bool,
    /// When true, `anchor` is the sprite bottom-left (OpenSC2K traffic origin) instead of bottom-center.
    pub bottom_left_anchor: bool,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
struct Vertex {
    position: [f32; 2],
    uv: [f32; 2],
    color: [f32; 4],
}

impl Vertex {
    const ATTRS: [wgpu::VertexAttribute; 3] =
        wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x2, 2 => Float32x4];

    fn layout() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &Self::ATTRS,
        }
    }
}

struct GpuAtlas {
    width: u32,
    height: u32,
    frames: HashMap<u32, AtlasFrame>,
    animation_counts: HashMap<u32, u8>,
    ids: Vec<u32>,
    bind_group: wgpu::BindGroup,
}

struct BuiltAtlas {
    rgba: RgbaImage,
    width: u32,
    height: u32,
    frames: HashMap<u32, AtlasFrame>,
    animation_counts: HashMap<u32, u8>,
    ids: Vec<u32>,
}

fn atlas_min_filter<'a>(frames: impl IntoIterator<Item = &'a AtlasFrame>) -> wgpu::FilterMode {
    if frames.into_iter().any(|frame| {
        let (logical_w, logical_h) = frame.display_size();
        frame.w > logical_w || frame.h > logical_h
    }) {
        wgpu::FilterMode::Linear
    } else {
        wgpu::FilterMode::Nearest
    }
}

fn renderer_instance_descriptor() -> InstanceDescriptor {
    #[cfg(target_os = "linux")]
    {
        // Legacy Intel Mesa Vulkan drivers (such as Iris 5100) can be useful
        // even when their Vulkan compliance version is not reported as
        // conformant. Keep environment overrides available for diagnostics.
        let mut descriptor = InstanceDescriptor::new_without_display_handle();
        descriptor.backends = wgpu::Backends::VULKAN;
        descriptor.flags |= wgpu::InstanceFlags::ALLOW_UNDERLYING_NONCOMPLIANT_ADAPTER;
        descriptor.with_env()
    }

    #[cfg(not(target_os = "linux"))]
    {
        InstanceDescriptor::new_without_display_handle_from_env()
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
struct UiVertex {
    position: [f32; 2],
    color: [f32; 4],
}

impl UiVertex {
    const ATTRS: [wgpu::VertexAttribute; 2] =
        wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x4];

    fn layout() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<UiVertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &Self::ATTRS,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct UiImage {
    pub atlas: usize,
    pub src_x: u32,
    pub src_y: u32,
    pub src_w: u32,
    pub src_h: u32,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub depth: f32,
    pub tint: [f32; 4],
}

#[derive(Debug, Clone, Copy)]
pub struct UiSolid {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub color: [f32; 4],
    pub depth: f32,
}

pub struct Renderer<'window> {
    surface: wgpu::Surface<'window>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: SurfaceConfiguration,
    pipeline: wgpu::RenderPipeline,
    ui_pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    atlases: Vec<GpuAtlas>,
    animation_atlas: Option<usize>,
    traffic_atlas: Option<usize>,
    ui_city_atlas: usize,
    ui_edit_atlas: usize,
    ui_citygame_city_atlas: usize,
    ui_citygame_edit_atlas: usize,
    ui_citygame_light_city_atlas: usize,
    ui_citygame_light_edit_atlas: usize,
    ui_classic_city_atlas: usize,
    ui_classic_edit_atlas: usize,
    citygame_toolbar_palette: bool,
    dark_appearance: bool,
    asset_root: PathBuf,
    palette: AnimatedPalette,
}

impl<'window> Renderer<'window> {
    /// A solids-only renderer for the CityGame start screen. It needs no imported art.
    pub fn new_title(window: &'window Window) -> Result<Self> {
        Self::new_internal(window, None)
    }

    pub fn new(window: &'window Window, asset_root: &Path) -> Result<Self> {
        Self::new_with_profile(window, asset_root, None, false)
    }

    pub fn new_with_profile(
        window: &'window Window,
        asset_root: &Path,
        replacement_root: Option<&Path>,
        prefer_replacement: bool,
    ) -> Result<Self> {
        Self::new_with_selection(
            window,
            asset_root,
            replacement_root,
            None,
            prefer_replacement,
        )
    }

    pub fn new_with_selection(
        window: &'window Window,
        asset_root: &Path,
        replacement_root: Option<&Path>,
        tile_set_root: Option<&Path>,
        prefer_replacement: bool,
    ) -> Result<Self> {
        Self::new_internal(
            window,
            Some((
                asset_root,
                replacement_root,
                tile_set_root,
                prefer_replacement,
            )),
        )
    }

    /// Create a renderer and load the full graphics library once (no double atlas upload).
    pub fn new_with_library(
        window: &'window Window,
        graphics_roots: &[std::path::PathBuf],
        replacement_root: Option<&Path>,
        tile_set_root: Option<&Path>,
        prefer_replacement: bool,
    ) -> Result<Self> {
        let mut renderer = Self::new_internal(window, None)?;
        renderer.reload_with_library(
            graphics_roots,
            replacement_root,
            tile_set_root,
            prefer_replacement,
        )?;
        Ok(renderer)
    }

    fn new_internal(
        window: &'window Window,
        assets: Option<(&Path, Option<&Path>, Option<&Path>, bool)>,
    ) -> Result<Self> {
        let (mut width, mut height) = window.size_in_pixels();
        if width == 0 || height == 0 {
            let (logical_width, logical_height) = window.size();
            width = logical_width.max(1);
            height = logical_height.max(1);
            log::warn!(
                "window backing size was 0x0; using logical size {width}x{height} for initial surface"
            );
        }
        let instance_descriptor = renderer_instance_descriptor();
        log::info!(
            "wgpu instance: backends={:?}, flags={:?}",
            instance_descriptor.backends,
            instance_descriptor.flags
        );
        let instance = wgpu::Instance::new(instance_descriptor);
        let surface =
            create_surface::create_surface(&instance, window).map_err(anyhow::Error::msg)?;
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            force_fallback_adapter: false,
            compatible_surface: Some(&surface),
        }))
        .context("requesting wgpu adapter")?;

        log::info!("wgpu adapter: {:?}", adapter.get_info());

        // Prefer the adapter's reported limits so older Apple GPUs (e.g. A12 /
        // max_inter_stage_shader_variables = 15) can create a device. Limits::default()
        // asks for 16 and fails on those chips.
        let required_limits = adapter.limits();
        log::info!(
            "wgpu limits: max_inter_stage_shader_variables={}",
            required_limits.max_inter_stage_shader_variables
        );

        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            required_limits,
            required_features: wgpu::Features::empty(),
            memory_hints: wgpu::MemoryHints::Performance,
            experimental_features: wgpu::ExperimentalFeatures::disabled(),
            trace: wgpu::Trace::Off,
            label: Some("sc2000 device"),
        }))
        .context("requesting wgpu device")?;

        let capabilities = surface.get_capabilities(&adapter);
        let surface_format = *capabilities
            .formats
            .iter()
            .find(|format| format.is_srgb())
            .unwrap_or(&capabilities.formats[0]);
        let present_mode = capabilities
            .present_modes
            .iter()
            .copied()
            .find(|mode| *mode == wgpu::PresentMode::Mailbox)
            .or_else(|| {
                capabilities
                    .present_modes
                    .iter()
                    .copied()
                    .find(|mode| *mode == wgpu::PresentMode::Fifo)
            })
            .or_else(|| {
                capabilities
                    .present_modes
                    .iter()
                    .copied()
                    .find(|mode| *mode == wgpu::PresentMode::Immediate)
            })
            .unwrap_or(capabilities.present_modes[0]);
        let config = SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: width.max(1),
            height: height.max(1),
            present_mode,
            alpha_mode: wgpu::CompositeAlphaMode::Auto,
            desired_maximum_frame_latency: 2,
            view_formats: vec![],
        };
        surface.configure(&device, &config);

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("sprite texture bind group layout"),
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

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("sprite shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("sprite pipeline layout"),
            bind_group_layouts: &[Some(&bind_group_layout)],
            immediate_size: 0,
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("sprite pipeline"),
            layout: Some(&pipeline_layout),
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
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                unclipped_depth: false,
                polygon_mode: wgpu::PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        let ui_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("ui shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("ui_shader.wgsl").into()),
        });
        let ui_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("ui pipeline layout"),
            bind_group_layouts: &[],
            immediate_size: 0,
        });
        let ui_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("ui pipeline"),
            layout: Some(&ui_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &ui_shader,
                entry_point: Some("vs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[UiVertex::layout()],
            },
            fragment: Some(wgpu::FragmentState {
                module: &ui_shader,
                entry_point: Some("fs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                unclipped_depth: false,
                polygon_mode: wgpu::PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        let asset_root = assets.map(|(root, _, _, _)| root).unwrap_or(Path::new(""));
        let palette = AnimatedPalette::load(asset_root)?;
        let mut renderer = Self {
            surface,
            device,
            queue,
            config,
            pipeline,
            ui_pipeline,
            bind_group_layout,
            atlases: Vec::new(),
            animation_atlas: None,
            traffic_atlas: None,
            ui_city_atlas: 0,
            ui_edit_atlas: 0,
            ui_citygame_city_atlas: 0,
            ui_citygame_edit_atlas: 0,
            ui_citygame_light_city_atlas: 0,
            ui_citygame_light_edit_atlas: 0,
            ui_classic_city_atlas: 0,
            ui_classic_edit_atlas: 0,
            citygame_toolbar_palette: true,
            dark_appearance: false,
            asset_root: asset_root.to_path_buf(),
            palette,
        };
        if let Some((asset_root, replacement_root, tile_set_root, prefer_replacement)) = assets {
            renderer.reload_with_selection(
                asset_root,
                replacement_root,
                tile_set_root,
                prefer_replacement,
            )?;
        }
        Ok(renderer)
    }

    pub fn reload_assets(&mut self, asset_root: &Path) -> Result<()> {
        self.reload_asset_profile(asset_root, None, false)
    }

    pub fn reload_asset_profile(
        &mut self,
        asset_root: &Path,
        replacement_root: Option<&Path>,
        prefer_replacement: bool,
    ) -> Result<()> {
        self.reload_with_selection(asset_root, replacement_root, None, prefer_replacement)
    }

    pub fn reload_with_selection(
        &mut self,
        asset_root: &Path,
        replacement_root: Option<&Path>,
        tile_set_root: Option<&Path>,
        prefer_replacement: bool,
    ) -> Result<()> {
        self.reload_with_library(
            &[asset_root.to_path_buf()],
            replacement_root,
            tile_set_root,
            prefer_replacement,
        )
    }

    pub fn reload_with_library(
        &mut self,
        graphics_roots: &[std::path::PathBuf],
        replacement_root: Option<&Path>,
        tile_set_root: Option<&Path>,
        prefer_replacement: bool,
    ) -> Result<()> {
        let asset_root = graphics_roots
            .first()
            .context("no original graphics source")?;
        self.asset_root = asset_root.to_path_buf();
        self.palette = AnimatedPalette::load(asset_root)?;
        let originals = load_sprite_sources(asset_root)?;
        let mut fallback = Vec::new();
        let mut covered: HashSet<u32> = originals
            .iter()
            .flat_map(|source| source.manifest.frames.iter().map(|frame| frame.id))
            .collect();
        for root in graphics_roots.iter().skip(1) {
            for source in load_sprite_sources(root)? {
                if source
                    .manifest
                    .frames
                    .iter()
                    .any(|frame| !covered.contains(&frame.id))
                {
                    covered.extend(source.manifest.frames.iter().map(|frame| frame.id));
                    fallback.push(source);
                }
            }
        }
        let tile_set = tile_set_root
            .map(load_sprite_sources)
            .transpose()?
            .unwrap_or_default();
        let replacement = if prefer_replacement {
            replacement_root
                .map(load_replacement_sprite_source)
                .transpose()?
                .flatten()
        } else {
            None
        };
        // Traffic is always rendered with the original Win95/XTRF masks. Prefer the extracted
        // Win95 pack even when another edition is selected for regular graphics: editions reuse
        // traffic IDs for different art, so first-source-wins would otherwise bake the wrong
        // masks. Keep this atlas independent of replacement art and optional tile sets too.
        let mut traffic_sources = Vec::new();
        for root in graphics_roots
            .iter()
            .filter(|root| root.join("win95_tiles").is_dir())
        {
            match crate::win95_import::load_traffic_mask_source(root) {
                Ok(Some(source)) => {
                    traffic_sources.push(source);
                    break;
                }
                Ok(None) => {}
                Err(error) => log::warn!("could not restore Win95 traffic masks: {error:#}"),
            }
        }
        traffic_sources.extend(originals.iter().cloned());
        traffic_sources.extend(fallback.iter().cloned());
        let mut sources = Vec::with_capacity(
            originals.len() + fallback.len() + tile_set.len() + usize::from(replacement.is_some()),
        );
        if prefer_replacement && let Some(source) = replacement {
            sources.push(source);
        }
        sources.extend(tile_set);
        sources.extend(originals);
        sources.extend(fallback);
        let toolbar_root = graphics_roots.iter()
            .find(|root| root.join("pict/PICT_00500.png").is_file())
            .unwrap_or(asset_root);
        let ui_sources = load_ui_palette_sources(toolbar_root)?;
        let mut atlases = Vec::with_capacity(sources.len() + ui_sources.len() + 2);
        for source in &sources {
            atlases.push(self.upload_atlas(source)?);
        }
        self.animation_atlas = None;
        if let Some(animation_atlas) = self.upload_animation_atlas(&sources)? {
            self.animation_atlas = Some(atlases.len());
            atlases.push(animation_atlas);
        }
        self.traffic_atlas = None;
        if let Some(traffic_atlas) = self.upload_traffic_atlas(&traffic_sources)? {
            self.traffic_atlas = Some(atlases.len());
            atlases.push(traffic_atlas);
        }
        self.ui_citygame_city_atlas = atlases.len();
        self.ui_citygame_edit_atlas = self.ui_citygame_city_atlas + 1;
        self.ui_classic_city_atlas = self.ui_citygame_city_atlas + 2;
        self.ui_classic_edit_atlas = self.ui_classic_city_atlas + 1;
        self.ui_citygame_light_city_atlas = self.ui_classic_city_atlas + 2;
        self.ui_citygame_light_edit_atlas = self.ui_citygame_light_city_atlas + 1;
        for source in &ui_sources {
            atlases.push(self.upload_ui_atlas(source)?);
        }
        if let Some(pictures) = crate::assets::load_newspaper_pictures(graphics_roots)? {
            atlases.push(self.upload_ui_atlas(&pictures)?);
        }
        atlases.push(self.upload_ui_atlas(&crate::newspaper_font::atlas()?)?);
        if self.citygame_toolbar_palette {
            self.select_citygame_toolbar_palette();
        } else {
            self.select_classic_toolbar_palette();
        }
        self.atlases = atlases;
        Ok(())
    }

    pub fn set_citygame_toolbar_palette(&mut self, enabled: bool) {
        self.citygame_toolbar_palette = enabled;
        if enabled {
            self.select_citygame_toolbar_palette();
        } else {
            self.select_classic_toolbar_palette();
        }
    }

    pub fn set_dark_appearance(&mut self, dark: bool) {
        self.dark_appearance = dark;
        if self.citygame_toolbar_palette {
            self.select_citygame_toolbar_palette();
        }
    }

    pub fn dark_appearance(&self) -> bool {
        self.dark_appearance
    }

    fn select_citygame_toolbar_palette(&mut self) {
        if self.dark_appearance {
            self.ui_city_atlas = self.ui_citygame_city_atlas;
            self.ui_edit_atlas = self.ui_citygame_edit_atlas;
        } else {
            self.ui_city_atlas = self.ui_citygame_light_city_atlas;
            self.ui_edit_atlas = self.ui_citygame_light_edit_atlas;
        }
    }

    fn select_classic_toolbar_palette(&mut self) {
        self.ui_city_atlas = self.ui_classic_city_atlas;
        self.ui_edit_atlas = self.ui_classic_edit_atlas;
    }

    pub fn ui_city_atlas(&self) -> usize {
        self.ui_city_atlas
    }

    pub fn ui_edit_atlas(&self) -> usize {
        self.ui_edit_atlas
    }

    pub fn animation_frame_count(&self, atlas: usize, base_sprite_id: u32) -> u8 {
        self.atlases
            .get(atlas)
            .and_then(|atlas| atlas.animation_counts.get(&base_sprite_id).copied())
            .unwrap_or(1)
    }

    pub fn resolve_sprite_id(&self, atlas: usize, base_sprite_id: u32, anim_frame: u8) -> u32 {
        let frame_count = self.animation_frame_count(atlas, base_sprite_id);
        if frame_count <= 1 {
            return base_sprite_id;
        }
        let frame = anim_frame % frame_count;
        let resolved = animated_sprite_id(base_sprite_id, frame);
        if self.has_sprite(atlas, resolved) {
            resolved
        } else {
            base_sprite_id
        }
    }

    pub fn resolve_sprite_frame(
        &self,
        atlas: usize,
        base_sprite_id: u32,
        anim_frame: u8,
    ) -> (usize, u32) {
        // Smoke buildings keep their base shape; only a transparent plume overlay animates.
        if is_smoke_sprite(base_sprite_id) {
            return (atlas, base_sprite_id);
        }

        let source_count = self.animation_frame_count(atlas, base_sprite_id);
        let compact_count = self.animation_atlas.and_then(|animation_atlas| {
            self.atlases
                .get(animation_atlas)
                .and_then(|atlas| atlas.animation_counts.get(&base_sprite_id).copied())
                .map(|count| (animation_atlas, count))
        });
        let (target_atlas, frame_count) = compact_count.unwrap_or((atlas, source_count));
        if frame_count <= 1 {
            return (atlas, base_sprite_id);
        }

        let frame = anim_frame % frame_count;
        let resolved = animated_sprite_id(base_sprite_id, frame);
        if self.has_sprite(target_atlas, resolved) {
            (target_atlas, resolved)
        } else {
            (atlas, base_sprite_id)
        }
    }

    pub fn resolve_smoke_overlay_frame(
        &self,
        base_sprite_id: u32,
        anim_frame: u8,
    ) -> Option<(usize, u32)> {
        if !is_smoke_sprite(base_sprite_id) {
            return None;
        }
        let atlas = self.animation_atlas?;
        let frame_count = self
            .atlases
            .get(atlas)
            .and_then(|atlas| atlas.animation_counts.get(&base_sprite_id).copied())?;
        if frame_count <= 1 {
            return None;
        }
        let resolved = animated_sprite_id(base_sprite_id, anim_frame % frame_count);
        self.has_sprite(atlas, resolved)
            .then_some((atlas, resolved))
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        let width = width.max(1);
        let height = height.max(1);
        if self.config.width == width && self.config.height == height {
            return;
        }
        self.config.width = width;
        self.config.height = height;
        self.surface.configure(&self.device, &self.config);
    }

    pub fn sync_surface(&mut self, width: u32, height: u32) {
        self.resize(width.max(1), height.max(1));
    }

    pub fn size(&self) -> (u32, u32) {
        (self.config.width, self.config.height)
    }

    pub fn has_sprite(&self, atlas: usize, sprite_id: u32) -> bool {
        self.atlases
            .get(atlas)
            .is_some_and(|atlas| atlas.frames.contains_key(&sprite_id))
    }

    pub fn atlas_for_sprite(&self, sprite_id: u32) -> Option<usize> {
        self.atlases
            .iter()
            .position(|atlas| atlas.frames.contains_key(&sprite_id))
    }

    pub fn sprite_frame_rect(&self, atlas: usize, sprite_id: u32) -> Option<[u32; 4]> {
        self.atlases.get(atlas).and_then(|atlas| {
            atlas
                .frames
                .get(&sprite_id)
                .map(|frame| [frame.x, frame.y, frame.w, frame.h])
        })
    }

    pub fn traffic_atlas_for_sprite(&self, sprite_id: u32) -> Option<usize> {
        self.traffic_atlas
            .filter(|atlas| self.has_sprite(*atlas, sprite_id))
    }

    pub fn sprite_size(&self, atlas: usize, sprite_id: u32) -> Option<(u32, u32)> {
        self.atlases
            .get(atlas)
            .and_then(|atlas| atlas.frames.get(&sprite_id).map(AtlasFrame::display_size))
    }

    pub fn traffic_tint(&self, color_slot: u8, anim_frame: u8) -> [f32; 4] {
        self.palette.traffic_tint_rgba(color_slot, anim_frame)
    }

    #[allow(dead_code)]
    pub fn sprite_id_by_index(&self, atlas: usize, index: usize) -> Option<u32> {
        self.atlases
            .get(atlas)
            .and_then(|atlas| atlas.ids.get(index % atlas.ids.len()).copied())
    }

    pub fn render(
        &mut self,
        sprites: &[RenderSprite],
        ui_solids: &[UiSolid],
        ui_images: &[UiImage],
    ) -> Result<bool> {
        self.render_with_clear_color(sprites, ui_solids, ui_images, [0.20, 0.08, 0.02, 1.0])
    }

    pub fn render_with_clear_color(
        &mut self,
        sprites: &[RenderSprite],
        ui_solids: &[UiSolid],
        ui_images: &[UiImage],
        clear_color: [f64; 4],
    ) -> Result<bool> {
        for attempt in 0..4 {
            match self.surface.get_current_texture() {
                wgpu::CurrentSurfaceTexture::Success(frame)
                | wgpu::CurrentSurfaceTexture::Suboptimal(frame) => {
                    self.present_frame(frame, sprites, ui_solids, ui_images, clear_color)?;
                    return Ok(true);
                }
                wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => {
                    if attempt < 3 {
                        std::thread::sleep(std::time::Duration::from_millis(4));
                    }
                }
                wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                    self.surface.configure(&self.device, &self.config);
                    if attempt < 3 {
                        std::thread::sleep(std::time::Duration::from_millis(1));
                    }
                }
                wgpu::CurrentSurfaceTexture::Validation => {
                    return Err(anyhow::anyhow!("surface validation error"));
                }
            }
        }
        Ok(false)
    }

    fn present_frame(
        &mut self,
        frame: wgpu::SurfaceTexture,
        sprites: &[RenderSprite],
        ui_solids: &[UiSolid],
        ui_images: &[UiImage],
        clear_color: [f64; 4],
    ) -> Result<()> {
        let output = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("frame encoder"),
            });

        let (world_sprites, ui_sprites): (Vec<&RenderSprite>, Vec<&RenderSprite>) = sprites
            .iter()
            .partition(|sprite| sprite.depth < UI_SPRITE_DEPTH_MIN);

        let (world_vertices, world_ranges) = self.build_vertices_from_refs(&world_sprites);
        let world_buffer = (!world_vertices.is_empty()).then(|| {
            self.device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("world sprite vertex buffer"),
                    contents: bytemuck::cast_slice(&world_vertices),
                    usage: wgpu::BufferUsages::VERTEX,
                })
        });

        let (ui_vertices, ui_ranges) = self.build_vertices_from_refs(&ui_sprites);
        let ui_sprite_buffer = (!ui_vertices.is_empty()).then(|| {
            self.device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("ui sprite vertex buffer"),
                    contents: bytemuck::cast_slice(&ui_vertices),
                    usage: wgpu::BufferUsages::VERTEX,
                })
        });

        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("main render pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &output,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: clear_color[0],
                            g: clear_color[1],
                            b: clear_color[2],
                            a: clear_color[3],
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });

            if let Some(vertex_buffer) = &world_buffer {
                pass.set_pipeline(&self.pipeline);
                pass.set_vertex_buffer(0, vertex_buffer.slice(..));
                for (atlas_index, start, count) in world_ranges {
                    if let Some(atlas) = self.atlases.get(atlas_index) {
                        pass.set_bind_group(0, &atlas.bind_group, &[]);
                        pass.draw(start..start + count, 0..1);
                    }
                }
            }

            if let Some(vertex_buffer) = &ui_sprite_buffer {
                pass.set_pipeline(&self.pipeline);
                pass.set_vertex_buffer(0, vertex_buffer.slice(..));
                for (atlas_index, start, count) in ui_ranges {
                    if let Some(atlas) = self.atlases.get(atlas_index) {
                        pass.set_bind_group(0, &atlas.bind_group, &[]);
                        pass.draw(start..start + count, 0..1);
                    }
                }
            }

            self.draw_ordered_ui(&mut pass, ui_images, ui_solids);
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        frame.present();
        Ok(())
    }
}

mod create_surface {
    use sdl3::video::Window;
    use wgpu::rwh::{HasDisplayHandle, HasWindowHandle};

    struct SyncWindow<'a>(&'a Window);

    unsafe impl<'a> Send for SyncWindow<'a> {}
    unsafe impl<'a> Sync for SyncWindow<'a> {}

    impl<'a> HasWindowHandle for SyncWindow<'a> {
        fn window_handle(&self) -> Result<wgpu::rwh::WindowHandle<'_>, wgpu::rwh::HandleError> {
            self.0.window_handle()
        }
    }

    impl<'a> HasDisplayHandle for SyncWindow<'a> {
        fn display_handle(&self) -> Result<wgpu::rwh::DisplayHandle<'_>, wgpu::rwh::HandleError> {
            self.0.display_handle()
        }
    }

    pub fn create_surface<'a>(
        instance: &wgpu::Instance,
        window: &'a Window,
    ) -> Result<wgpu::Surface<'a>, String> {
        instance
            .create_surface(SyncWindow(window))
            .map_err(|err| err.to_string())
    }
}

#[cfg(test)]
mod tests;
