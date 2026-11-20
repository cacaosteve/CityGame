struct VertexIn {
    @location(0) position: vec2<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) color: vec4<f32>,
};

struct VertexOut {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
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
    out.color = input.color;
    return out;
}

@group(0) @binding(0) var sprite_texture: texture_2d<f32>;
@group(0) @binding(1) var sprite_sampler: sampler;
@group(1) @binding(0) var sprite_indices: texture_2d<f32>;

@fragment
fn fs_main(input: VertexOut) -> FragmentOut {
    let texel = textureSample(sprite_texture, sprite_sampler, input.uv);
    if texel.a < 0.5 {
        discard;
    }
    let source = textureLoad(
        sprite_indices,
        vec2<i32>(input.uv * vec2<f32>(textureDimensions(sprite_indices))),
        0,
    );
    var out: FragmentOut;
    out.color = texel * input.color;
    let identity_color = all(input.color == vec4<f32>(1.0)) && texel.a >= 0.999;
    let known = source.g > 0.5 && identity_color;
    out.palette_index = vec4<u32>(u32(round(source.r * 255.0)), select(0u, 1u, known), 0u, 255u);
    return out;
}
