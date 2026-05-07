//! Render pipeline grouping for the 3D viewport.

/// All viewport render pipelines.
pub struct PipelineSet {
    pub terrain: wgpu::RenderPipeline,
    pub terrain_medium: wgpu::RenderPipeline,
    pub terrain_high: wgpu::RenderPipeline,
    pub grid: wgpu::RenderPipeline,
    pub border: wgpu::RenderPipeline,
    pub sky: wgpu::RenderPipeline,
    pub water: wgpu::RenderPipeline,
    pub shadow_terrain: wgpu::RenderPipeline,
    pub shadow_model: wgpu::RenderPipeline,
}
