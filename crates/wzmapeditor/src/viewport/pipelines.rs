//! Pipeline builder helpers to reduce wgpu render pipeline boilerplate.

/// Depth/stencil configuration for a render pipeline.
#[derive(Clone, Copy)]
pub(crate) enum DepthConfig {
    /// Depth writes enabled, `Less` compare, no bias.
    WriteDefault,
    /// Depth writes disabled (overlay), `LessEqual` compare, no bias.
    ReadOnly,
    /// Depth writes disabled (overlay), `LessEqual` compare, custom bias.
    ReadOnlyBiased(wgpu::DepthBiasState),
    /// Depth writes enabled, `Less` compare, custom bias (shadow maps).
    WriteBiased(wgpu::DepthBiasState),
}

impl DepthConfig {
    fn to_state(self) -> wgpu::DepthStencilState {
        match self {
            Self::WriteDefault => wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::Less),
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            },
            Self::ReadOnly => wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: Some(false),
                depth_compare: Some(wgpu::CompareFunction::LessEqual),
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            },
            Self::ReadOnlyBiased(bias) => wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: Some(false),
                depth_compare: Some(wgpu::CompareFunction::LessEqual),
                stencil: wgpu::StencilState::default(),
                bias,
            },
            Self::WriteBiased(bias) => wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::Less),
                stencil: wgpu::StencilState::default(),
                bias,
            },
        }
    }
}

pub(crate) struct PipelineDesc<'a> {
    pub label: &'a str,
    pub layout: &'a wgpu::PipelineLayout,
    pub shader: &'a wgpu::ShaderModule,
    pub vertex_buffers: &'a [wgpu::VertexBufferLayout<'a>],
    pub format: wgpu::TextureFormat,
    pub blend: Option<wgpu::BlendState>,
    pub cull_mode: Option<wgpu::Face>,
    pub depth: DepthConfig,
}

pub(crate) fn create_render_pipeline(
    device: &wgpu::Device,
    desc: &PipelineDesc<'_>,
) -> wgpu::RenderPipeline {
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(desc.label),
        layout: Some(desc.layout),
        vertex: wgpu::VertexState {
            module: desc.shader,
            entry_point: Some("vs_main"),
            buffers: desc.vertex_buffers,
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: desc.shader,
            entry_point: Some("fs_main"),
            targets: &[Some(wgpu::ColorTargetState {
                format: desc.format,
                blend: desc.blend,
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            strip_index_format: None,
            front_face: wgpu::FrontFace::Ccw,
            cull_mode: desc.cull_mode,
            polygon_mode: wgpu::PolygonMode::Fill,
            unclipped_depth: false,
            conservative: false,
        },
        depth_stencil: Some(desc.depth.to_state()),
        multisample: wgpu::MultisampleState::default(),
        multiview_mask: None,
        cache: None,
    })
}

/// Build a depth-only render pipeline (no fragment shader) for shadow passes.
pub(crate) fn create_depth_only_pipeline(
    device: &wgpu::Device,
    label: &str,
    layout: &wgpu::PipelineLayout,
    shader: &wgpu::ShaderModule,
    vertex_buffers: &[wgpu::VertexBufferLayout<'_>],
    cull_mode: Option<wgpu::Face>,
    depth: DepthConfig,
) -> wgpu::RenderPipeline {
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(label),
        layout: Some(layout),
        vertex: wgpu::VertexState {
            module: shader,
            entry_point: Some("vs_main"),
            buffers: vertex_buffers,
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        },
        fragment: None,
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            strip_index_format: None,
            front_face: wgpu::FrontFace::Ccw,
            cull_mode,
            polygon_mode: wgpu::PolygonMode::Fill,
            unclipped_depth: false,
            conservative: false,
        },
        depth_stencil: Some(depth.to_state()),
        multisample: wgpu::MultisampleState::default(),
        multiview_mask: None,
        cache: None,
    })
}

pub(crate) fn create_pipeline_layout(
    device: &wgpu::Device,
    label: &str,
    bind_group_layouts: &[&wgpu::BindGroupLayout],
) -> wgpu::PipelineLayout {
    // wgpu 29 expects `&[Option<&BindGroupLayout>]`; wrap each layout here
    // so callers keep passing `&[&layout]`.
    let wrapped: Vec<Option<&wgpu::BindGroupLayout>> =
        bind_group_layouts.iter().copied().map(Some).collect();
    device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some(label),
        bind_group_layouts: &wrapped,
        immediate_size: 0,
    })
}
