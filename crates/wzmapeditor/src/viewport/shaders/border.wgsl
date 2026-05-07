// Build-margin overlay: dark fog over the 4-tile unbuildable map border.

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
    @location(0) world_pos: vec3<f32>,
};

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    // Lifted off the terrain to reduce z-fighting at distance.
    let offset_pos = in.position + in.normal * 2.0;
    out.clip_position = uniforms.mvp * vec4<f32>(offset_pos, 1.0);
    out.world_pos = in.position;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let border_offset = 4.0 * 128.0; // 4 tiles * 128 world units

    let map_w = uniforms.map_world_size.x;
    let map_h = uniforms.map_world_size.y;

    let d_left   = in.world_pos.x;
    let d_right  = map_w - in.world_pos.x;
    let d_top    = in.world_pos.z;
    let d_bottom = map_h - in.world_pos.z;
    let d_edge = min(min(d_left, d_right), min(d_top, d_bottom));

    if d_edge > border_offset + 16.0 {
        discard;
    }

    // Feather across tile 4 (the buildable-side edge).
    let tile_size = 128.0;
    let feather_start = 3.0 * tile_size;
    let feather = 1.0 - smoothstep(feather_start, border_offset + 16.0, d_edge);

    return vec4<f32>(0.0, 0.0, 0.0, feather * 0.75);
}
