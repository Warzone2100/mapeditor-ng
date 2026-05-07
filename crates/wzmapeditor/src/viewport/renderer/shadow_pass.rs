//! Shadow depth pass: bind group layouts, pipelines, and per-frame encoding.
//!
//! The shadow pass renders terrain plus model instances into a 2048x2048
//! depth-only target. Front-face culling combined with a small constant +
//! slope-scaled bias controls acne; the cached depth is sampled by the main
//! terrain/model passes through a comparison sampler.

use glam::Mat4;

use super::super::camera::Camera;
use super::super::model_gpu::ModelResources;
use super::super::pie_mesh::{ModelInstance, ModelVertex};
use super::super::pipelines::{self, DepthConfig};
use super::super::render_types::RenderSettings;
use super::super::shadow::{self, ShadowResources, ShadowUniforms};
use super::super::terrain::TerrainVertex;
use super::super::uniforms::{UniformState, Uniforms};
use super::util::BindGroupLayoutBuilder;

/// Constant depth-buffer bias. Pairs with [`SHADOW_SLOPE_BIAS`] to suppress
/// shadow acne on surfaces nearly perpendicular to the light.
const SHADOW_DEPTH_BIAS: i32 = 2;

/// Slope-scaled depth bias. 2.0 covers moderate terrain slopes without
/// visible peter-panning.
const SHADOW_SLOPE_BIAS: f32 = 2.0;

/// All resources produced by the shadow-pass setup.
pub(super) struct ShadowPassBuild {
    pub bind_group_layout: wgpu::BindGroupLayout,
    pub resources: ShadowResources,
    pub terrain_pipeline: wgpu::RenderPipeline,
    pub model_pipeline: wgpu::RenderPipeline,
}

/// Build the shadow bind group layouts, depth resources, and depth-only pipelines.
pub(super) fn build(device: &wgpu::Device) -> ShadowPassBuild {
    let bind_group_layout = BindGroupLayoutBuilder::new("shadow_bind_group_layout")
        .depth_texture(0, wgpu::ShaderStages::FRAGMENT)
        .sampler_comparison(1, wgpu::ShaderStages::FRAGMENT)
        .build(device);

    let uniform_layout = BindGroupLayoutBuilder::new("shadow_uniform_layout")
        .uniform_buffer(0, wgpu::ShaderStages::VERTEX)
        .build(device);

    let resources = shadow::create_shadow_resources(device, &bind_group_layout, &uniform_layout);

    let terrain_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("shadow_shader"),
        source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/shadow.wgsl").into()),
    });
    let model_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("shadow_model_shader"),
        source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/shadow_model.wgsl").into()),
    });

    let pipeline_layout =
        pipelines::create_pipeline_layout(device, "shadow_pipeline_layout", &[&uniform_layout]);

    let bias = wgpu::DepthBiasState {
        constant: SHADOW_DEPTH_BIAS,
        slope_scale: SHADOW_SLOPE_BIAS,
        clamp: 0.0,
    };

    let terrain_pipeline = pipelines::create_depth_only_pipeline(
        device,
        "shadow_pipeline",
        &pipeline_layout,
        &terrain_shader,
        &[TerrainVertex::desc()],
        Some(wgpu::Face::Front),
        DepthConfig::WriteBiased(bias),
    );
    let model_pipeline = pipelines::create_depth_only_pipeline(
        device,
        "shadow_model_pipeline",
        &pipeline_layout,
        &model_shader,
        &[ModelVertex::desc(), ModelInstance::desc()],
        Some(wgpu::Face::Front),
        DepthConfig::WriteBiased(bias),
    );

    ShadowPassBuild {
        bind_group_layout,
        resources,
        terrain_pipeline,
        model_pipeline,
    }
}

/// Encode the shadow depth pass for terrain and models into `encoder`.
///
/// Skips work when the depth texture is already populated and the dirty
/// flag is clear; the cached depth is sampled by the main pass directly.
pub(super) fn encode(
    encoder: &mut wgpu::CommandEncoder,
    shadow: &ShadowResources,
    terrain_pipeline: &wgpu::RenderPipeline,
    model_pipeline: &wgpu::RenderPipeline,
    terrain_gpu: &super::super::render_types::TerrainGpuData,
    models: &ModelResources,
) {
    let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("shadow_pass"),
        color_attachments: &[],
        depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
            view: &shadow.depth_view,
            depth_ops: Some(wgpu::Operations {
                load: wgpu::LoadOp::Clear(1.0),
                store: wgpu::StoreOp::Store,
            }),
            stencil_ops: None,
        }),
        timestamp_writes: None,
        occlusion_query_set: None,
        multiview_mask: None,
    });

    pass.set_pipeline(terrain_pipeline);
    pass.set_bind_group(0, &shadow.uniform_bind_group, &[]);
    pass.set_vertex_buffer(0, terrain_gpu.vertex_buffer.slice(..));
    pass.set_index_buffer(
        terrain_gpu.index_buffer.slice(..),
        wgpu::IndexFormat::Uint32,
    );
    pass.draw_indexed(0..terrain_gpu.index_count, 0, 0..1);

    if !models.draw_calls.is_empty() {
        pass.set_pipeline(model_pipeline);
        pass.set_bind_group(0, &shadow.uniform_bind_group, &[]);

        for draw_call in &models.draw_calls {
            let key: &str = draw_call.model_key.as_ref();
            let gpu_model = models.cache.get(key);
            let inst_buf = models.instance_buffer(key);
            if let (Some(gpu_model), Some(inst_buf)) = (gpu_model, inst_buf) {
                pass.set_vertex_buffer(0, gpu_model.vertex_buffer.slice(..));
                pass.set_vertex_buffer(1, inst_buf.slice(..));
                pass.set_index_buffer(gpu_model.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(0..gpu_model.index_count, 0, 0..draw_call.instance_count);
            }
        }
    }

    drop(pass);
}

impl super::EditorRenderer {
    /// Run the shadow depth pass for terrain and models.
    ///
    /// Skips work when the depth texture is already populated and the dirty
    /// flag is clear; the cached depth is sampled by the main pass directly.
    pub fn run_shadow_pass(&mut self, encoder: &mut wgpu::CommandEncoder, dirty: bool) {
        let Some(terrain_gpu) = &self.terrain_gpu else {
            return;
        };
        if !self.settings.shadows_enabled {
            return;
        }
        if !dirty && self.shadow_initialized {
            return;
        }
        encode(
            encoder,
            &self.shadow,
            &self.pipelines.shadow_terrain,
            &self.pipelines.shadow_model,
            terrain_gpu,
            &self.models,
        );
        self.shadow_initialized = true;
    }

    /// Update the uniform buffer with the current camera MVP and rendering params.
    pub fn update_uniforms(
        &mut self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        camera: &super::super::camera::Camera,
        brush_highlight: [f32; 4],
        brush_highlight_extra: [[f32; 4]; 3],
    ) {
        let shadow_mvp = self
            .shadow_mvp_cache
            .get(self.settings.sun_direction, self.map_dims);
        let time = self.start_time.elapsed().as_secs_f32();
        let uniforms = build_uniforms(
            &self.settings,
            camera,
            self.map_dims,
            shadow_mvp,
            time,
            brush_highlight,
            brush_highlight_extra,
        );
        write_frame_uniforms(
            queue,
            encoder,
            &self.uniforms,
            &self.shadow,
            &uniforms,
            shadow_mvp,
        );
    }
}

/// Build the per-frame `Uniforms` payload from the camera, settings, and shadow MVP.
fn build_uniforms(
    settings: &RenderSettings,
    camera: &Camera,
    map_dims: (u32, u32),
    shadow_mvp: Mat4,
    time: f32,
    brush_highlight: [f32; 4],
    brush_highlight_extra: [[f32; 4]; 3],
) -> Uniforms {
    let mvp = camera.view_projection_matrix();
    Uniforms {
        mvp: mvp.to_cols_array_2d(),
        sun_direction: [
            settings.sun_direction[0],
            settings.sun_direction[1],
            settings.sun_direction[2],
            0.0,
        ],
        brush_highlight,
        brush_highlight_extra,
        camera_pos: [camera.position.x, camera.position.y, camera.position.z, 0.0],
        fog_color: [
            settings.fog_color[0],
            settings.fog_color[1],
            settings.fog_color[2],
            if settings.fog_enabled { 1.0 } else { 0.0 },
        ],
        fog_params: [settings.fog_start, settings.fog_end, time, 0.0],
        shadow_mvp: shadow_mvp.to_cols_array_2d(),
        map_world_size: [
            map_dims.0 as f32 * wz_maplib::constants::TILE_UNITS_F32,
            map_dims.1 as f32 * wz_maplib::constants::TILE_UNITS_F32,
            0.0,
            0.0,
        ],
    }
}

/// Stage the camera and shadow uniform buffers for the frame.
///
/// Camera uniforms go via `queue.write_buffer` to the staging buffer
/// followed by `encoder.copy_buffer_to_buffer` so the copy lands in the
/// same command stream as the render pass. The shadow uniform uses a plain
/// `queue.write_buffer` because the shadow pass runs after this copy in
/// the same encoder.
fn write_frame_uniforms(
    queue: &wgpu::Queue,
    encoder: &mut wgpu::CommandEncoder,
    uniforms_state: &UniformState,
    shadow: &ShadowResources,
    uniforms: &Uniforms,
    shadow_mvp: Mat4,
) {
    let bytes = bytemuck::bytes_of(uniforms);
    queue.write_buffer(&uniforms_state.staging_buffer, 0, bytes);
    encoder.copy_buffer_to_buffer(
        &uniforms_state.staging_buffer,
        0,
        &uniforms_state.buffer,
        0,
        bytes.len() as u64,
    );
    let shadow_uniforms = ShadowUniforms {
        light_mvp: shadow_mvp.to_cols_array_2d(),
    };
    queue.write_buffer(
        &shadow.uniform_buffer,
        0,
        bytemuck::bytes_of(&shadow_uniforms),
    );
}
