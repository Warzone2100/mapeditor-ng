// Tile-boundary grid overlay drawn on top of the terrain.

struct Uniforms {
    mvp: mat4x4<f32>,
    sun_direction: vec4<f32>,
    brush_highlight: vec4<f32>,
    brush_highlight_extra: array<vec4<f32>, 3>,
    camera_pos: vec4<f32>,
    fog_color: vec4<f32>,
    fog_params: vec4<f32>,
    shadow_mvp: mat4x4<f32>,
    map_world_size: vec4<f32>,
};

@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) tex_coord: vec2<f32>,
    @location(3) height_color: f32,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) tex_coord: vec2<f32>,
};

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    // Lifted off the terrain to avoid z-fighting.
    let offset_pos = in.position + in.normal * 0.5;
    out.clip_position = uniforms.mvp * vec4<f32>(offset_pos, 1.0);
    out.tex_coord = in.tex_coord;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let uv = in.tex_coord;
    let line_width = 0.03;

    let near_edge_x = min(uv.x, 1.0 - uv.x);
    let near_edge_y = min(uv.y, 1.0 - uv.y);
    let near_edge = min(near_edge_x, near_edge_y);

    if near_edge > line_width {
        discard;
    }

    let alpha = 1.0 - smoothstep(0.0, line_width, near_edge);
    return vec4<f32>(0.0, 0.0, 0.0, alpha * 0.3);
}
