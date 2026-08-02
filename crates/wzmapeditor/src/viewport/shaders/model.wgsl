// PIE model shader: per-instance transforms, WZ2100 lighting,
// TCMask team colors, normal mapping, Gaussian specular, shadows, fog.

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
// Declared but deliberately unsampled: group 0 is shared with the terrain
// passes, so the layout carries the lightmap either way, and models take no
// per-tile brightness from it (see the light term below).
@group(0) @binding(1)
var lightmap_texture: texture_2d<f32>;
@group(0) @binding(2)
var lightmap_sampler: sampler;
// Sampler lives in the per-frame group rather than per-model so DX12's
// 2048-entry sampler descriptor heap doesn't exhaust once a few hundred
// PIE models load.
@group(0) @binding(3)
var model_sampler: sampler;

// 4-layer array: 0=diffuse, 1=tcmask, 2=normal, 3=specular.
// Stored Rgba8Unorm and shaded as-is: WZ2100 lights gamma-encoded texels and
// writes gamma-encoded output, so no transfer curve is applied anywhere here.
@group(1) @binding(0)
var model_atlas: texture_2d_array<f32>;

@group(2) @binding(0)
var shadow_map: texture_depth_2d;
@group(2) @binding(1)
var shadow_sampler: sampler_comparison;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) tex_coord: vec2<f32>,
    @location(3) tangent: vec4<f32>,
    // Per-instance model matrix as 4 vec4 columns.
    @location(4) model_col0: vec4<f32>,
    @location(5) model_col1: vec4<f32>,
    @location(6) model_col2: vec4<f32>,
    @location(7) model_col3: vec4<f32>,
    @location(8) team_color: vec4<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) tex_coord: vec2<f32>,
    @location(1) world_normal: vec3<f32>,
    @location(2) team_color: vec4<f32>,
    @location(3) world_pos: vec3<f32>,
    @location(4) world_tangent: vec3<f32>,
    @location(5) tangent_handedness: f32,
};

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    let model_matrix = mat4x4<f32>(
        in.model_col0,
        in.model_col1,
        in.model_col2,
        in.model_col3,
    );

    let world_pos = model_matrix * vec4<f32>(in.position, 1.0);

    let normal_matrix = mat3x3<f32>(
        model_matrix[0].xyz,
        model_matrix[1].xyz,
        model_matrix[2].xyz,
    );
    let world_normal = normalize(normal_matrix * in.normal);
    let world_tangent = normalize(normal_matrix * in.tangent.xyz);

    var out: VertexOutput;
    out.clip_position = uniforms.mvp * world_pos;
    out.tex_coord = in.tex_coord;
    out.world_normal = world_normal;
    out.team_color = in.team_color;
    out.world_pos = world_pos.xyz;
    out.world_tangent = world_tangent;
    out.tangent_handedness = in.tangent.w;
    return out;
}

const ALPHA_CUTOFF: f32 = 0.1;

// WZ2100 patches WZ_MIP_LOAD_BIAS into every texture lookup except the
// lightmap; the default "High" LOD distance setting makes it -0.5.
const MIP_LOAD_BIAS: f32 = -0.5;

const GAUSSIAN_SHININESS: f32 = 0.33;   // WZ2100 tcmask_instanced.frag shininess
const AMBIENT: f32 = 0.5;               // WZ2100 piedraw.cpp LIGHT_AMBIENT

// Floor on shadow visibility, per WZ2100 shadow_mapping.glsl. The classic
// branch has no term that the shadow does not scale, so leaving visibility
// unclamped would take every self-shadowed facet to black.
const MIN_SHADOW_VISIBILITY: f32 = 0.5;

// 3x3 PCF shadow with depth bias.
fn compute_shadow(world_pos: vec3<f32>, n_dot_l: f32) -> f32 {
    let shadow_pos = uniforms.shadow_mvp * vec4<f32>(world_pos, 1.0);
    let shadow_ndc = shadow_pos.xyz / shadow_pos.w;

    let shadow_uv = vec2<f32>(
        shadow_ndc.x * 0.5 + 0.5,
        -shadow_ndc.y * 0.5 + 0.5,
    );
    let shadow_depth = shadow_ndc.z;

    // WebGPU bans textureSampleCompare in non-uniform control flow, so the
    // out-of-bounds case folds into select() rather than an early return.
    let in_bounds = shadow_uv.x >= 0.0 && shadow_uv.x <= 1.0
        && shadow_uv.y >= 0.0 && shadow_uv.y <= 1.0;

    let texel_size = 1.0 / f32(textureDimensions(shadow_map).x);
    var visibility = 0.0;
    // Slope-scaled bias per shadow_mapping.glsl, expressed in world units;
    // the whole-map shadow frustum is 3x the map's larger extent deep
    // (compute_shadow_mvp). Tighter than the terrain's, matching upstream's
    // smaller model offset, so small objects don't peter-pan.
    let depth_range = 3.0 * max(uniforms.map_world_size.x, uniforms.map_world_size.y);
    let slope = sqrt(max(1.0 - n_dot_l * n_dot_l, 0.0)) / max(n_dot_l, 0.1);
    let bias = min(1.0 + 4.0 * slope, 30.0) / max(depth_range, 1.0);
    for (var y = -1i; y <= 1i; y++) {
        for (var x = -1i; x <= 1i; x++) {
            let offset = vec2<f32>(f32(x), f32(y)) * texel_size;
            visibility += textureSampleCompare(
                shadow_map,
                shadow_sampler,
                shadow_uv + offset,
                shadow_depth - bias,
            );
        }
    }
    let pcf = mix(MIN_SHADOW_VISIBILITY, 1.0, visibility / 9.0);
    return select(1.0, pcf, in_bounds);
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let tex_color = textureSampleBias(model_atlas, model_sampler, in.tex_coord, 0, MIP_LOAD_BIAS);

    if tex_color.a < ALPHA_CUTOFF {
        discard;
    }

    // team_color.a encodes mode: <0 = ghost (alpha = -a), >1.5 = selected.
    let is_ghost = in.team_color.a < 0.0;
    let is_selected = in.team_color.a > 1.5;
    let ghost_alpha = select(1.0, -in.team_color.a, is_ghost);

    // Filler maps for missing normal/specular are uploaded with alpha=0;
    // real maps have alpha=1, so .a is a presence flag.
    let normal_sample = textureSampleBias(model_atlas, model_sampler, in.tex_coord, 2, MIP_LOAD_BIAS);
    let specular_sample = textureSampleBias(model_atlas, model_sampler, in.tex_coord, 3, MIP_LOAD_BIAS);
    let has_normalmap = normal_sample.a > 0.5;
    let has_specularmap = specular_sample.a > 0.5;

    var N = normalize(in.world_normal);
    if has_normalmap {
        let T = normalize(in.world_tangent);
        let B = cross(N, T) * in.tangent_handedness;
        let tbn = mat3x3<f32>(T, N, B);

        // WZ2100 XZY swizzle with Y-invert: normalFromMap.xzy then N.y = -N.y.
        var nm = normal_sample.xyz * 2.0 - 1.0;
        nm = vec3<f32>(nm.x, nm.z, -nm.y);
        N = normalize(tbn * nm);
    }

    let sun_dir = normalize(uniforms.sun_direction.xyz);

    // HQ (specular map): ambient*diffuse + diffuse*lambert*diffuse + specular.
    // Classic: ambient*diffuse*2 with ambient=0.5 (no directional term).
    var light: vec3<f32>;
    var specular_contrib = vec3<f32>(0.0);

    // Classic scales its single light term by visibility; HQ folds visibility
    // into the diffuse term and leaves ambient unshadowed.
    let shadow = compute_shadow(in.world_pos, max(dot(N, sun_dir), 0.0));

    // No lightmap term: tcmask_instanced.frag reads only the lightmap's *rgb*
    // for models (point-light colour, zero in an ordinary scene) and never its
    // per-tile brightness, which belongs to the terrain alone. Scaling models
    // by it made them track the ground's occlusion, so a structure standing in
    // a valley darkened with the valley floor instead of catching the sun.
    if has_specularmap {
        let lambertTerm = max(dot(N, sun_dir), 0.0);

        // Upstream forces visibility to 1 for the ambient term on HQ models
        // ("exclude double multiply"), so only the diffuse term is shadowed.
        let ambient_light = vec3<f32>(AMBIENT) * tex_color.rgb;
        let diffuse_light = tex_color.rgb * lambertTerm * shadow;
        light = ambient_light + diffuse_light;

        // Gaussian specular per tcmask.frag lines 103-107.
        if lambertTerm > 0.0 {
            let view_dir = normalize(uniforms.camera_pos.xyz - in.world_pos);
            let H = normalize(sun_dir + view_dir);
            let NdotH = clamp(dot(H, N), -1.0, 1.0);
            let exponent = acos(NdotH) / GAUSSIAN_SHININESS;
            let gaussianTerm = exp(-(exponent * exponent));

            let spec_value = specular_sample.r;
            specular_contrib = vec3<f32>(spec_value * gaussianTerm * lambertTerm);
        }
    } else {
        // tcmask_instanced.frag classic: emissive is zero, the diffuse term is
        // gated off by vanillaFactor, and ambient is doubled, so albedo scaled
        // by visibility is the entire light term.
        light = tex_color.rgb * (AMBIENT * 2.0) * shadow;
    }

    // Grain merge per tcmask_instanced.frag. This is the upstream blend and
    // behaves now that shading is gamma-space, where mask and diffuse share
    // a transfer curve.
    let mask_alpha = textureSampleBias(model_atlas, model_sampler, in.tex_coord, 1, MIP_LOAD_BIAS).r;
    var lit_color = light + specular_contrib + (in.team_color.rgb - 0.5) * mask_alpha;

    if uniforms.fog_color.a > 0.5 {
        let dist = distance(in.world_pos, uniforms.camera_pos.xyz);
        let fog_start = uniforms.fog_params.x;
        let fog_end = uniforms.fog_params.y;
        let fog_factor = clamp((fog_end - dist) / (fog_end - fog_start), 0.0, 1.0);
        lit_color = mix(uniforms.fog_color.rgb, lit_color, fog_factor);
    }

    // Ghost tints toward team_color (caller passes green for valid, red for invalid).
    if is_ghost {
        lit_color = mix(lit_color, in.team_color.rgb, 0.4);
    }

    if is_selected {
        let view_dir = normalize(uniforms.camera_pos.xyz - in.world_pos);
        let rim = 1.0 - max(dot(view_dir, N), 0.0);
        let time = uniforms.fog_params.z;
        let pulse = 0.75 + 0.25 * sin(time * 4.0);
        let highlight_color = vec3<f32>(0.4, 0.7, 1.0);
        let rim_glow = highlight_color * rim * rim * pulse * 1.6;
        lit_color = lit_color + rim_glow;
        lit_color = mix(lit_color, highlight_color, 0.15);
        lit_color = lit_color * 1.15;
    }

    return vec4<f32>(lit_color, tex_color.a * ghost_alpha);
}
