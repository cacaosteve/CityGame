struct VertexIn {
    @location(0) position: vec2<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) color: vec4<f32>,
};
struct VertexOut {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};
struct ShadowPalette {
    colors: array<vec4<f32>, 14>,
};
struct FragmentOut {
    @location(0) color: vec4<f32>,
    @location(1) palette_index: vec4<u32>,
};

@vertex
fn vs_main(input: VertexIn) -> VertexOut {
    var out: VertexOut;
    out.position = vec4<f32>(input.position, 0.0, 1.0);
    out.uv = input.uv;
    return out;
}

@group(0) @binding(0) var sprite_texture: texture_2d<f32>;
@group(0) @binding(1) var sprite_sampler: sampler;
@group(1) @binding(0) var background: texture_2d<f32>;
@group(1) @binding(1) var<uniform> palette: ShadowPalette;
@group(1) @binding(2) var background_indices: texture_2d<u32>;
@group(2) @binding(0) var source_indices: texture_2d<f32>;

@fragment
fn fs_main(input: VertexOut) -> FragmentOut {
    if textureSample(sprite_texture, sprite_sampler, input.uv).a < 0.5 {
        discard;
    }
    let position = vec2<i32>(input.position.xy);
    let source = textureLoad(background_indices, position, 0);
    if source.g == 0u || (source.r != 79u && (source.r < 100u || source.r > 110u)) {
        discard;
    }
    let mapped_index = select(13u, 12u, source.r == 79u);
    var out: FragmentOut;
    out.color = vec4<f32>(palette.colors[mapped_index].rgb, textureLoad(background, position, 0).a);
    out.palette_index = vec4<u32>(select(110u, 84u, source.r == 79u), 1u, 0u, 255u);
    return out;
}
