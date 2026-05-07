//! Uniform buffer management for the 3D viewport.

/// Uniform buffer data sent to shaders each frame.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct Uniforms {
    pub(crate) mvp: [[f32; 4]; 4],
    pub(crate) sun_direction: [f32; 4],
    /// Brush preview: [`world_x`, `world_z`, `radius_world`, active (0 or 1)]
    pub(crate) brush_highlight: [f32; 4],
    /// Extra mirrored brush highlights (up to 3 additional mirrors).
    pub(crate) brush_highlight_extra: [[f32; 4]; 3],
    /// Camera world position (xyz), w unused.
    pub(crate) camera_pos: [f32; 4],
    /// Fog color (rgb), fog enabled flag (a > 0.5 = on).
    pub(crate) fog_color: [f32; 4],
    /// Fog params: x = start distance, y = end distance, z = time (seconds), w = unused.
    pub(crate) fog_params: [f32; 4],
    /// Light-space MVP for shadow mapping.
    pub(crate) shadow_mvp: [[f32; 4]; 4],
    /// Map size in world units for lightmap UV computation.
    pub(crate) map_world_size: [f32; 4],
}

/// GPU uniform buffer, staging buffer, and bind group.
pub struct UniformState {
    pub buffer: wgpu::Buffer,
    /// Encoder-copy staging buffer for DX12 compatibility.
    pub staging_buffer: wgpu::Buffer,
    pub bind_group: wgpu::BindGroup,
    /// Retained for thumbnail bind group creation.
    pub(crate) bind_group_layout: wgpu::BindGroupLayout,
}

impl UniformState {
    /// Create uniform buffers, layout, and initial bind group.
    pub fn new(
        device: &wgpu::Device,
        lightmap_view: &wgpu::TextureView,
        lightmap_sampler: &wgpu::Sampler,
        model_sampler: &wgpu::Sampler,
    ) -> Self {
        let uniform_size = size_of::<Uniforms>() as u64;
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("terrain_uniforms"),
            size: uniform_size,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let staging_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("uniform_staging"),
            size: uniform_size,
            usage: wgpu::BufferUsages::COPY_SRC | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("uniform_bind_group_layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        let bind_group = Self::create_bind_group(
            device,
            &bind_group_layout,
            &buffer,
            lightmap_view,
            lightmap_sampler,
            model_sampler,
        );

        Self {
            buffer,
            staging_buffer,
            bind_group,
            bind_group_layout,
        }
    }

    /// Create the group-0 bind group: uniform buffer, lightmap texture +
    /// sampler, shared model sampler. Reused during init, lightmap upload
    /// (on resize), and thumbnail setup.
    pub(crate) fn create_bind_group(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        uniform_buffer: &wgpu::Buffer,
        lightmap_view: &wgpu::TextureView,
        lightmap_sampler: &wgpu::Sampler,
        model_sampler: &wgpu::Sampler,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("uniform_bind_group"),
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(lightmap_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(lightmap_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::Sampler(model_sampler),
                },
            ],
        })
    }
}
