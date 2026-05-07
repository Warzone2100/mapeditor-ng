// Weather particles: camera-facing billboard quads for rain and snow.

struct Uniforms {
    mvp: mat4x4<f32>,
    sun_direction: vec4<f32>,
    brush_highlight: vec4<f32>,
    brush_highlight_extra: array<vec4<f32>, 3>,
    camera_pos: vec4<f32>,
    fog_color: vec4<f32>,
    fog_params: vec4<f32>,     // x = fog start, y = fog end, z = time, w = unused
    shadow_mvp: mat4x4<f32>,
    map_world_size: vec4<f32>,
};

@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) alpha: f32,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) alpha: f32,
    @location(1) world_pos: vec3<f32>,
};

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.clip_position = uniforms.mvp * vec4<f32>(in.position, 1.0);
    out.alpha = in.alpha;
    out.world_pos = in.position;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // Pale blue-grey approximating WZ2100's white-on-overcast particle.
    var color = vec4<f32>(0.85, 0.88, 0.92, in.alpha);

    if uniforms.fog_color.a > 0.5 {
        let dist = distance(in.world_pos, uniforms.camera_pos.xyz);
        let fog_start = uniforms.fog_params.x;
        let fog_end = uniforms.fog_params.y;
        let fog_factor = clamp((dist - fog_start) / (fog_end - fog_start), 0.0, 1.0);
        // Half-strength alpha falloff keeps particles visible deeper into fog.
        color = vec4<f32>(
            mix(color.rgb, uniforms.fog_color.rgb, fog_factor),
            color.a * (1.0 - fog_factor * 0.5),
        );
    }

    return color;
}
