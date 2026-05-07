// Instanced PIE model shadow pass: render from the sun's POV into the shadow map.

struct ShadowUniforms {
    light_mvp: mat4x4<f32>,
};

@group(0) @binding(0)
var<uniform> shadow_uniforms: ShadowUniforms;

// Layout matches model.wgsl; only position and the per-instance matrix are used.
struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) tex_coord: vec2<f32>,
    @location(3) tangent: vec4<f32>,
    @location(4) model_col0: vec4<f32>,
    @location(5) model_col1: vec4<f32>,
    @location(6) model_col2: vec4<f32>,
    @location(7) model_col3: vec4<f32>,
    @location(8) team_color: vec4<f32>,
};

@vertex
fn vs_main(in: VertexInput) -> @builtin(position) vec4<f32> {
    let model_matrix = mat4x4<f32>(
        in.model_col0,
        in.model_col1,
        in.model_col2,
        in.model_col3,
    );
    let world_pos = model_matrix * vec4<f32>(in.position, 1.0);
    return shadow_uniforms.light_mvp * world_pos;
}
