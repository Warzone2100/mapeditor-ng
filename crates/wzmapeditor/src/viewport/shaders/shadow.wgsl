// Terrain shadow depth pass: render from sun's POV into the shadow map.

struct ShadowUniforms {
    light_mvp: mat4x4<f32>,
};

@group(0) @binding(0)
var<uniform> shadow_uniforms: ShadowUniforms;

// Only position is consumed; remaining fields match the terrain vertex layout.
struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) tex_coord: vec2<f32>,
    @location(3) height_color: f32,
    @location(4) tile_index: f32,
    @location(5) ground_indices: vec4<u32>,
    @location(6) ground_weights: vec4<f32>,
    @location(7) tile_no: i32,
    @location(8) decal_tangent: vec4<f32>,
};

@vertex
fn vs_main(in: VertexInput) -> @builtin(position) vec4<f32> {
    return shadow_uniforms.light_mvp * vec4<f32>(in.position, 1.0);
}
