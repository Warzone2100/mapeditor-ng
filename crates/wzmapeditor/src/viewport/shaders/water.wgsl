// Water surface shader matching WZ2100 medium/high quality rendering.
// Terrain is lowered at water tiles; water plane sits just above the basin
// and shore tiles fade via depth-based alpha.
//
// Constants (from WZ2100 src/terrain.cpp + shaders):
//   512.0  UV tile scale (1/4 * 1/128 of world coords)
//   80.0   layer 1 X drift divisor
//   40.0   Y drift divisor (opposite sign per layer)

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

// page-80-water-1.png and page-81-water-2.png. A 1x1 fallback is bound when
// textures aren't loaded; we detect it via textureDimensions and switch to
// procedural noise.
@group(1) @binding(0)
var water_tex1: texture_2d<f32>;
@group(1) @binding(1)
var water_tex2: texture_2d<f32>;
@group(1) @binding(2)
var water_sampler: sampler;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) depth: f32,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_pos: vec3<f32>,
    @location(1) depth: f32,
};

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.clip_position = uniforms.mvp * vec4<f32>(in.position, 1.0);
    out.world_pos = in.position;
    out.depth = in.depth;
    return out;
}

fn hash(p: vec2<f32>) -> f32 {
    let h = dot(p, vec2<f32>(127.1, 311.7));
    return fract(sin(h) * 43758.5453);
}

fn hash2(p: vec2<f32>) -> f32 {
    let h = dot(p, vec2<f32>(269.5, 183.3));
    return fract(sin(h) * 28637.1257);
}

fn noise(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);

    let a = hash(i);
    let b = hash(i + vec2<f32>(1.0, 0.0));
    let c = hash(i + vec2<f32>(0.0, 1.0));
    let d = hash(i + vec2<f32>(1.0, 1.0));

    return mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
}

fn noise2(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);

    let a = hash2(i);
    let b = hash2(i + vec2<f32>(1.0, 0.0));
    let c = hash2(i + vec2<f32>(0.0, 1.0));
    let d = hash2(i + vec2<f32>(1.0, 1.0));

    return mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
}

// Analytical (dN/dx, dN/dy) of bilinear-smoothstep noise. Finite differences
// over an eps that straddles a grid boundary produce tile-aligned specular lines.
// Smoothstep u = 3f^2 - 2f^3, du/df = 6f(1-f).
fn noise_grad(p: vec2<f32>) -> vec2<f32> {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    let du = 6.0 * f * (1.0 - f);

    let a = hash(i);
    let b = hash(i + vec2<f32>(1.0, 0.0));
    let c = hash(i + vec2<f32>(0.0, 1.0));
    let d = hash(i + vec2<f32>(1.0, 1.0));

    let dx = du.x * (mix(b - a, d - c, u.y));
    let dy = du.y * (mix(c - a, d - b, u.x));
    return vec2<f32>(dx, dy);
}

fn noise2_grad(p: vec2<f32>) -> vec2<f32> {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    let du = 6.0 * f * (1.0 - f);

    let a = hash2(i);
    let b = hash2(i + vec2<f32>(1.0, 0.0));
    let c = hash2(i + vec2<f32>(0.0, 1.0));
    let d = hash2(i + vec2<f32>(1.0, 1.0));

    let dx = du.x * (mix(b - a, d - c, u.y));
    let dy = du.y * (mix(c - a, d - b, u.x));
    return vec2<f32>(dx, dy);
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let time = uniforms.fog_params.z;

    let uv1 = vec2<f32>(
        in.world_pos.x / 512.0 + time / 80.0,
        -in.world_pos.z / 512.0 + time / 40.0,
    );
    let uv2 = vec2<f32>(
        in.world_pos.x / 512.0,
        -in.world_pos.z / 512.0 - time / 40.0,
    );

    let tex_dims = textureDimensions(water_tex1);
    let has_texture = tex_dims.x > 1u;

    // Single-octave value noise has grid-aligned artifacts that pow(128) specular
    // turns into lines. Two octaves at non-integer ratios (2.17x) decorrelate the grids.
    var tex_val1: f32;
    var tex_val2: f32;
    if has_texture {
        tex_val1 = textureSample(water_tex1, water_sampler, uv1).r;
        tex_val2 = textureSample(water_tex2, water_sampler, uv2).r;
    } else {
        // Frequencies 13.7 / 11.3 are non-integer so they don't align with the
        // 128-unit tile grid (UV 0.25). Octave offsets prevent inter-octave correlation.
        let p1 = uv1 * 13.7;
        let p2 = uv2 * 11.3;
        tex_val1 = noise(p1) * 0.65 + noise(p1 * 2.17 + vec2<f32>(5.3, 7.1)) * 0.35;
        tex_val2 = noise2(p2) * 0.65 + noise2(p2 * 2.17 + vec2<f32>(3.7, 9.2)) * 0.35;
    }

    // WZ2100 blend: noise = tex1.r * tex2.r.
    let wave = tex_val1 * tex_val2;

    // Smooth normals via product rule on 2-octave FBM: d(f*g)/dx = f'*g + f*g'.
    var nx: f32;
    var nz: f32;
    if has_texture {
        let eps = 0.02;
        let s_xp = textureSample(water_tex1, water_sampler, uv1 + vec2<f32>(eps, 0.0)).r
                  * textureSample(water_tex2, water_sampler, uv2 + vec2<f32>(eps, 0.0)).r;
        let s_xn = textureSample(water_tex1, water_sampler, uv1 - vec2<f32>(eps, 0.0)).r
                  * textureSample(water_tex2, water_sampler, uv2 - vec2<f32>(eps, 0.0)).r;
        let s_zp = textureSample(water_tex1, water_sampler, uv1 + vec2<f32>(0.0, eps)).r
                  * textureSample(water_tex2, water_sampler, uv2 + vec2<f32>(0.0, eps)).r;
        let s_zn = textureSample(water_tex1, water_sampler, uv1 - vec2<f32>(0.0, eps)).r
                  * textureSample(water_tex2, water_sampler, uv2 - vec2<f32>(0.0, eps)).r;
        nx = s_xp - s_xn;
        nz = s_zp - s_zn;
    } else {
        let p1 = uv1 * 13.7;
        let p2 = uv2 * 11.3;
        // Chain rule applies the 2.17 frequency factor to the second octave.
        let n1 = noise(p1) * 0.65 + noise(p1 * 2.17 + vec2<f32>(5.3, 7.1)) * 0.35;
        let n2 = noise2(p2) * 0.65 + noise2(p2 * 2.17 + vec2<f32>(3.7, 9.2)) * 0.35;
        let g1 = (noise_grad(p1) * 0.65 + noise_grad(p1 * 2.17 + vec2<f32>(5.3, 7.1)) * (0.35 * 2.17)) * 13.7;
        let g2 = (noise2_grad(p2) * 0.65 + noise2_grad(p2 * 2.17 + vec2<f32>(3.7, 9.2)) * (0.35 * 2.17)) * 11.3;
        nx = g1.x * n2 + n1 * g2.x;
        nz = g1.y * n2 + n1 * g2.y;
    }

    // Y=30.0 keeps the surface nearly flat for WZ2100's calm pond look;
    // higher Y = flatter, only near-perfect reflections become visible sparkles.
    let N = normalize(vec3<f32>(nx, 30.0, nz));

    let water_base = vec3<f32>(0.18, 0.33, 0.42);
    var water_color = water_base + vec3<f32>(wave * wave * 0.04);

    let sun_dir = normalize(uniforms.sun_direction.xyz);
    let eye_dir = normalize(uniforms.camera_pos.xyz - in.world_pos);
    let half_vec = normalize(sun_dir + eye_dir);

    // 0.45 ambient + 0.55 diffuse Lambert.
    let diffuse = max(dot(N, sun_dir), 0.0) * 0.55 + 0.45;
    water_color *= diffuse;

    let n_dot_h = max(dot(N, half_vec), 0.0);
    let spec = pow(n_dot_h, 128.0);

    let reflect_dir = reflect(-sun_dir, N);
    let r_dot_h = max(dot(reflect_dir, half_vec), 0.0);
    let refl = pow(r_dot_h, 14.0);

    let specular = spec * 0.25 + refl * 0.04;
    water_color += vec3<f32>(1.0, 1.0, 1.0) * specular;

    let eye_dot_up = max(dot(eye_dir, vec3<f32>(0.0, 1.0, 0.0)), 0.0);
    let fresnel = pow(1.0 - eye_dot_up, 4.0) * 0.1;
    water_color = mix(water_color, (water_color + vec3<f32>(1.0, 0.8, 0.63)) * 0.5, fresnel);

    // Depth ranges: 0 (outer shore, 0 water neighbors), 21 (1 neighbor),
    // 30 (2 neighbors), 42 (fully interior). smoothstep 0-25 fades across
    // the 2-tile shore zone.
    let shore_fade = smoothstep(0.0, 25.0, in.depth);
    let alpha = shore_fade;

    if uniforms.fog_color.a > 0.5 {
        let dist = distance(in.world_pos, uniforms.camera_pos.xyz);
        let fog_start = uniforms.fog_params.x;
        let fog_end = uniforms.fog_params.y;
        let fog_factor = clamp((fog_end - dist) / (fog_end - fog_start), 0.0, 1.0);
        water_color = mix(uniforms.fog_color.rgb, water_color, fog_factor);
    }

    return vec4<f32>(water_color, alpha);
}
