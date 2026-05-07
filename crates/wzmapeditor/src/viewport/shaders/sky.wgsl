// Sky gradient on a fullscreen triangle: blue zenith to tileset fog at the horizon.

struct Uniforms {
    mvp: mat4x4<f32>,
    sun_direction: vec4<f32>,
    brush_highlight: vec4<f32>,
    brush_highlight_extra: array<vec4<f32>, 3>,
    camera_pos: vec4<f32>,
    fog_color: vec4<f32>,      // rgb = fog/horizon color, a = fog enabled
    fog_params: vec4<f32>,     // x = fog start, y = fog end, z = time, w = unused
    shadow_mvp: mat4x4<f32>,
    map_world_size: vec4<f32>,
};

@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vi: u32) -> VertexOutput {
    // Fullscreen triangle: (-1,-1), (3,-1), (-1,3).
    let x = f32(i32(vi & 1u) * 4 - 1);
    let y = f32(i32(vi >> 1u) * 4 - 1);

    var out: VertexOutput;
    // z=1 places the sky on the far plane.
    out.position = vec4<f32>(x, y, 1.0, 1.0);
    // 0 at top, 1 at bottom.
    out.uv = vec2<f32>((x + 1.0) * 0.5, (1.0 - y) * 0.5);
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let sky_zenith = vec3<f32>(0.35, 0.55, 0.85);
    // Warm haze fallback when fog is disabled.
    let horizon = select(
        vec3<f32>(0.69, 0.56, 0.37),
        uniforms.fog_color.rgb,
        uniforms.fog_color.a > 0.5,
    );

    // pow(t, 0.5) compresses the gradient toward the horizon for a thicker haze band.
    let t = in.uv.y;
    let sky = mix(sky_zenith, horizon, pow(t, 0.5));

    // Approximate sun screen-Y from sun_direction.y; not a real projection.
    let sun_screen_y = 1.0 - clamp(uniforms.sun_direction.y * 0.5 + 0.5, 0.0, 1.0);
    let sun_dist = abs(t - sun_screen_y);
    let sun_glow = exp(-sun_dist * sun_dist * 8.0) * 0.15;
    let sun_color = vec3<f32>(1.0, 0.9, 0.7);

    let final_color = sky + sun_color * sun_glow;

    return vec4<f32>(final_color, 1.0);
}
