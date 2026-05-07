//! Terrain mesh: CPU builder in `cpu`, shared `TerrainVertex` and GPU
//! vertex layout here.

mod cpu;

/// A terrain vertex with fields for all quality modes.
///
/// Classic samples the atlas with `tile_index` + `tex_coord`. Medium/High
/// splat ground textures via `ground_indices` + `ground_weights`. High
/// also uses `tile_no` and `decal_tangent` for decal-array sampling with
/// normal-map blending.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct TerrainVertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub tex_coord: [f32; 2],
    /// Normalized 0..1 height for the coloring fallback.
    pub height_color: f32,
    /// Atlas index for Classic mode.
    pub tile_index: f32,
    /// Per-corner ground-type indices for Medium/High splatting; same for
    /// all 4 vertices of a tile.
    pub ground_indices: [u32; 4],
    /// Per-corner blend weights matching `ground_indices`. Each corner
    /// vertex has weight 1.0 in its own slot.
    pub ground_weights: [f32; 4],
    /// Decal layer index for High mode. Negative means no decal.
    /// Shaders treat `tile_no >= 0` as the decal flag.
    pub tile_no: i32,
    /// Tangent xyz plus handedness w. High mode uses this to transform
    /// decal normals into ground tangent space (mirrors WZ2100's
    /// `vertexTangent`).
    pub decal_tangent: [f32; 4],
}

impl TerrainVertex {
    pub fn desc() -> wgpu::VertexBufferLayout<'static> {
        const ATTRS: &[wgpu::VertexAttribute] = &[
            // position
            wgpu::VertexAttribute {
                offset: 0,
                shader_location: 0,
                format: wgpu::VertexFormat::Float32x3,
            },
            // normal
            wgpu::VertexAttribute {
                offset: 12,
                shader_location: 1,
                format: wgpu::VertexFormat::Float32x3,
            },
            // tex_coord
            wgpu::VertexAttribute {
                offset: 24,
                shader_location: 2,
                format: wgpu::VertexFormat::Float32x2,
            },
            // height_color
            wgpu::VertexAttribute {
                offset: 32,
                shader_location: 3,
                format: wgpu::VertexFormat::Float32,
            },
            // tile_index
            wgpu::VertexAttribute {
                offset: 36,
                shader_location: 4,
                format: wgpu::VertexFormat::Float32,
            },
            // ground_indices (uvec4)
            wgpu::VertexAttribute {
                offset: 40,
                shader_location: 5,
                format: wgpu::VertexFormat::Uint32x4,
            },
            // ground_weights (vec4)
            wgpu::VertexAttribute {
                offset: 56,
                shader_location: 6,
                format: wgpu::VertexFormat::Float32x4,
            },
            // tile_no (i32: negative = no decal, non-negative = decal layer)
            wgpu::VertexAttribute {
                offset: 72,
                shader_location: 7,
                format: wgpu::VertexFormat::Sint32,
            },
            // decal_tangent (vec4: tangent.xyz + handedness sign w)
            wgpu::VertexAttribute {
                offset: 76,
                shader_location: 8,
                format: wgpu::VertexFormat::Float32x4,
            },
        ];

        wgpu::VertexBufferLayout {
            array_stride: size_of::<TerrainVertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: ATTRS,
        }
    }
}

/// CPU-side terrain mesh, ready for GPU upload.
pub struct TerrainMesh {
    pub vertices: Vec<TerrainVertex>,
    pub indices: Vec<u32>,
}

#[cfg(test)]
mod layout_tests {
    use super::TerrainVertex;
    use std::mem::offset_of;

    // Guards the offsets baked into TerrainVertex::desc(). Any field
    // reorder or padding change trips this before it reaches the GPU.
    #[test]
    fn terrain_vertex_layout_matches_desc() {
        assert_eq!(offset_of!(TerrainVertex, position), 0);
        assert_eq!(offset_of!(TerrainVertex, normal), 12);
        assert_eq!(offset_of!(TerrainVertex, tex_coord), 24);
        assert_eq!(offset_of!(TerrainVertex, height_color), 32);
        assert_eq!(offset_of!(TerrainVertex, tile_index), 36);
        assert_eq!(offset_of!(TerrainVertex, ground_indices), 40);
        assert_eq!(offset_of!(TerrainVertex, ground_weights), 56);
        assert_eq!(offset_of!(TerrainVertex, tile_no), 72);
        assert_eq!(offset_of!(TerrainVertex, decal_tangent), 76);
        assert_eq!(size_of::<TerrainVertex>(), 92);
    }
}
