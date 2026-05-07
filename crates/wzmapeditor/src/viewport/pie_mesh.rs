//! Convert PIE models to GPU-ready mesh data (vertex/index buffers).

use glam::Vec3;
use wz_pie::PieModel;

/// A single model vertex sent to the GPU.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct ModelVertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub tex_coord: [f32; 2],
    /// Tangent vector for normal mapping. xyz = tangent direction, w = handedness sign.
    pub tangent: [f32; 4],
}

impl ModelVertex {
    pub fn desc() -> wgpu::VertexBufferLayout<'static> {
        const ATTRS: &[wgpu::VertexAttribute] = &[
            wgpu::VertexAttribute {
                offset: 0,
                shader_location: 0,
                format: wgpu::VertexFormat::Float32x3,
            },
            wgpu::VertexAttribute {
                offset: 12,
                shader_location: 1,
                format: wgpu::VertexFormat::Float32x3,
            },
            wgpu::VertexAttribute {
                offset: 24,
                shader_location: 2,
                format: wgpu::VertexFormat::Float32x2,
            },
            wgpu::VertexAttribute {
                offset: 32,
                shader_location: 3,
                format: wgpu::VertexFormat::Float32x4,
            },
        ];

        wgpu::VertexBufferLayout {
            array_stride: size_of::<ModelVertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: ATTRS,
        }
    }
}

/// Per-instance data for rendering multiple copies of the same model.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct ModelInstance {
    /// Model matrix columns (4x vec4).
    pub model_matrix: [[f32; 4]; 4],
    /// Team color tint (RGBA).
    pub team_color: [f32; 4],
}

impl ModelInstance {
    pub fn desc() -> wgpu::VertexBufferLayout<'static> {
        const ATTRS: &[wgpu::VertexAttribute] = &[
            // model_col0
            wgpu::VertexAttribute {
                offset: 0,
                shader_location: 4,
                format: wgpu::VertexFormat::Float32x4,
            },
            // model_col1
            wgpu::VertexAttribute {
                offset: 16,
                shader_location: 5,
                format: wgpu::VertexFormat::Float32x4,
            },
            // model_col2
            wgpu::VertexAttribute {
                offset: 32,
                shader_location: 6,
                format: wgpu::VertexFormat::Float32x4,
            },
            // model_col3
            wgpu::VertexAttribute {
                offset: 48,
                shader_location: 7,
                format: wgpu::VertexFormat::Float32x4,
            },
            // team_color
            wgpu::VertexAttribute {
                offset: 64,
                shader_location: 8,
                format: wgpu::VertexFormat::Float32x4,
            },
        ];

        wgpu::VertexBufferLayout {
            array_stride: size_of::<ModelInstance>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: ATTRS,
        }
    }
}

/// CPU-side mesh data built from a PIE model, ready to upload to GPU.
pub struct ModelMesh {
    pub vertices: Vec<ModelVertex>,
    pub indices: Vec<u32>,
    /// Axis-aligned bounding box: (min, max).
    pub aabb_min: Vec3,
    pub aabb_max: Vec3,
}

/// Player team colors for editor display.
///
/// Hex values from WZ2100 palette.txt, reordered so player 0 is orange
/// (most visible). WZ2100 assigns colors dynamically at game time, but
/// maps only store player indices. Negative alpha signals ghost preview
/// mode; otherwise alpha is unused.
pub const TEAM_COLORS: [[f32; 4]; 16] = [
    [
        0xFF as f32 / 255.0,
        0xB0 as f32 / 255.0,
        0x35 as f32 / 255.0,
        1.0,
    ], // 0: orange
    [
        0x10 as f32 / 255.0,
        0x70 as f32 / 255.0,
        0x10 as f32 / 255.0,
        1.0,
    ], // 1: green
    [
        0x90 as f32 / 255.0,
        0x90 as f32 / 255.0,
        0x90 as f32 / 255.0,
        1.0,
    ], // 2: grey
    [
        0x20 as f32 / 255.0,
        0x20 as f32 / 255.0,
        0x20 as f32 / 255.0,
        1.0,
    ], // 3: black
    [
        0x9B as f32 / 255.0,
        0x0F as f32 / 255.0,
        0x0F as f32 / 255.0,
        1.0,
    ], // 4: red
    [
        0x27 as f32 / 255.0,
        0x31 as f32 / 255.0,
        0xB9 as f32 / 255.0,
        1.0,
    ], // 5: blue
    [
        0xD0 as f32 / 255.0,
        0x10 as f32 / 255.0,
        0xB0 as f32 / 255.0,
        1.0,
    ], // 6: pink
    [
        0x20 as f32 / 255.0,
        0xD0 as f32 / 255.0,
        0xD0 as f32 / 255.0,
        1.0,
    ], // 7: cyan
    [
        0xF0 as f32 / 255.0,
        0xE8 as f32 / 255.0,
        0x10 as f32 / 255.0,
        1.0,
    ], // 8: yellow
    [
        0x70 as f32 / 255.0,
        0x00 as f32 / 255.0,
        0x74 as f32 / 255.0,
        1.0,
    ], // 9: purple
    [
        0xE0 as f32 / 255.0,
        0xE0 as f32 / 255.0,
        0xE0 as f32 / 255.0,
        1.0,
    ], // 10: white
    [
        0x20 as f32 / 255.0,
        0x20 as f32 / 255.0,
        0xFF as f32 / 255.0,
        1.0,
    ], // 11: bright blue
    [
        0x00 as f32 / 255.0,
        0xA0 as f32 / 255.0,
        0x00 as f32 / 255.0,
        1.0,
    ], // 12: neon green
    [
        0x40 as f32 / 255.0,
        0x00 as f32 / 255.0,
        0x00 as f32 / 255.0,
        1.0,
    ], // 13: infrared
    [
        0x10 as f32 / 255.0,
        0x00 as f32 / 255.0,
        0x40 as f32 / 255.0,
        1.0,
    ], // 14: ultraviolet
    [
        0x40 as f32 / 255.0,
        0x60 as f32 / 255.0,
        0x00 as f32 / 255.0,
        1.0,
    ], // 15: brown
];

/// Team color for a player index. Scavengers (-1) and out-of-range values
/// return dark red.
pub fn team_color(player: i8) -> [f32; 4] {
    if player < 0 || player as usize >= TEAM_COLORS.len() {
        [0.5, 0.1, 0.1, 1.0]
    } else {
        TEAM_COLORS[player as usize]
    }
}

/// Build GPU-ready mesh data, including all sub-levels.
///
/// PIE levels can represent parts of the same structure (e.g. the oil
/// derrick pump arm). All levels share one coordinate space; connectors
/// mount weapons/turrets, they do not position sub-levels.
pub fn build_mesh(pie: &PieModel) -> Option<ModelMesh> {
    if pie.levels.is_empty() {
        return None;
    }

    let mut combined_vertices = Vec::new();
    let mut combined_indices = Vec::new();
    let mut aabb_min = Vec3::splat(f32::MAX);
    let mut aabb_max = Vec3::splat(f32::MIN);

    for level in &pie.levels {
        if let Some(mesh) = build_mesh_from_level(pie, level) {
            let base_index = combined_vertices.len() as u32;

            combined_vertices.extend_from_slice(&mesh.vertices);
            for idx in mesh.indices {
                combined_indices.push(base_index + idx);
            }

            aabb_min = aabb_min.min(mesh.aabb_min);
            aabb_max = aabb_max.max(mesh.aabb_max);
        }
    }

    if combined_vertices.is_empty() {
        return None;
    }

    Some(ModelMesh {
        vertices: combined_vertices,
        indices: combined_indices,
        aabb_min,
        aabb_max,
    })
}

/// Triangle tangent from positions and UVs.
///
/// Returns `[tx, ty, tz, handedness]` where handedness is +-1, the sign
/// such that `bitangent = cross(normal, tangent) * handedness`.
fn compute_tangent(
    v0: Vec3,
    v1: Vec3,
    v2: Vec3,
    uv0: glam::Vec2,
    uv1: glam::Vec2,
    uv2: glam::Vec2,
    normal: Vec3,
) -> [f32; 4] {
    let edge1 = v1 - v0;
    let edge2 = v2 - v0;
    let duv1 = uv1 - uv0;
    let duv2 = uv2 - uv0;

    let det = duv1.x * duv2.y - duv1.y * duv2.x;
    if det.abs() < 1e-8 {
        // Degenerate UV mapping; fall back to any direction perpendicular to normal.
        let fallback = if normal.x.abs() < 0.9 {
            Vec3::X
        } else {
            Vec3::Y
        };
        let t = normal.cross(fallback).normalize_or_zero();
        return [t.x, t.y, t.z, 1.0];
    }

    let inv_det = 1.0 / det;
    let tangent = (edge1 * duv2.y - edge2 * duv1.y) * inv_det;
    let bitangent = (edge2 * duv1.x - edge1 * duv2.x) * inv_det;

    // Gram-Schmidt against normal.
    let t = (tangent - normal * normal.dot(tangent)).normalize_or_zero();

    let handedness = if normal.cross(t).dot(bitangent) < 0.0 {
        -1.0
    } else {
        1.0
    };

    [t.x, t.y, t.z, handedness]
}

fn build_mesh_from_level(pie: &PieModel, level: &wz_pie::PieLevel) -> Option<ModelMesh> {
    if level.vertices.is_empty() || level.polygons.is_empty() {
        return None;
    }

    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    let mut aabb_min = Vec3::splat(f32::MAX);
    let mut aabb_max = Vec3::splat(f32::MIN);

    // PIE Z is north, editor Z is south. Apply at the mesh level so the
    // model matrix stays a proper rotation (det = +1) with correct
    // normals and textures.
    let flip = |v: Vec3| Vec3::new(v.x, v.y, -v.z);

    for v in &level.vertices {
        let fv = flip(*v);
        aabb_min = aabb_min.min(fv);
        aabb_max = aabb_max.max(fv);
    }

    // PIE v2 UVs are pixel coords; v3+ UVs are already normalized (0..1).
    let needs_uv_normalize = pie.version < 3;
    let tex_w = if needs_uv_normalize {
        pie.texture_width.max(1) as f32
    } else {
        1.0
    };
    let tex_h = if needs_uv_normalize {
        pie.texture_height.max(1) as f32
    } else {
        1.0
    };

    for poly in &level.polygons {
        if poly.indices.len() < 3 {
            continue;
        }

        let i0 = poly.indices[0] as usize;
        let i1 = poly.indices[1] as usize;
        let i2 = poly.indices[2] as usize;

        if i0 >= level.vertices.len() || i1 >= level.vertices.len() || i2 >= level.vertices.len() {
            continue;
        }

        let v0 = flip(level.vertices[i0]);
        let v1 = flip(level.vertices[i1]);
        let v2 = flip(level.vertices[i2]);

        // Reverse winding so cross-product normals point outward after the Z-flip.
        let edge1 = v2 - v0;
        let edge2 = v1 - v0;
        let face_normal = edge1.cross(edge2).normalize_or_zero();

        // Reversed winding for UVs too.
        let face_tangent = if poly.tex_coords.len() >= 3 {
            let uv0 = glam::Vec2::new(poly.tex_coords[0].x / tex_w, poly.tex_coords[0].y / tex_h);
            let uv1 = glam::Vec2::new(poly.tex_coords[1].x / tex_w, poly.tex_coords[1].y / tex_h);
            let uv2 = glam::Vec2::new(poly.tex_coords[2].x / tex_w, poly.tex_coords[2].y / tex_h);
            compute_tangent(v0, v2, v1, uv0, uv2, uv1, face_normal)
        } else {
            [1.0, 0.0, 0.0, 1.0]
        };

        let base_vertex = vertices.len() as u32;

        for (vi, &idx) in poly.indices.iter().enumerate() {
            let vert_idx = idx as usize;
            if vert_idx >= level.vertices.len() {
                continue;
            }
            let pos = flip(level.vertices[vert_idx]);
            let uv = if vi < poly.tex_coords.len() {
                [poly.tex_coords[vi].x / tex_w, poly.tex_coords[vi].y / tex_h]
            } else {
                [0.0, 0.0]
            };

            vertices.push(ModelVertex {
                position: pos.to_array(),
                normal: face_normal.to_array(),
                tex_coord: uv,
                tangent: face_tangent,
            });
        }

        // Fan triangulation with reversed winding to match the Z-flipped positions.
        let n_verts = poly.indices.len() as u32;
        for i in 1..n_verts.saturating_sub(1) {
            indices.push(base_vertex);
            indices.push(base_vertex + i + 1);
            indices.push(base_vertex + i);
        }
    }

    if vertices.is_empty() {
        return None;
    }

    Some(ModelMesh {
        vertices,
        indices,
        aabb_min,
        aabb_max,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::Vec2;
    use wz_pie::{PieLevel, PiePolygon};

    fn make_triangle_pie() -> PieModel {
        PieModel {
            version: 3,
            model_type: 0,
            texture_page: "test.png".to_string(),
            texture_width: 256,
            texture_height: 256,
            texture_pages: vec!["test.png".to_string()],
            tcmask_pages: vec![],
            normal_page: None,
            specular_page: None,
            event_page: None,
            levels: vec![PieLevel {
                vertices: vec![
                    Vec3::new(0.0, 0.0, 0.0),
                    Vec3::new(100.0, 0.0, 0.0),
                    Vec3::new(50.0, 100.0, 0.0),
                ],
                polygons: vec![PiePolygon {
                    flags: 0x200,
                    indices: vec![0, 1, 2],
                    // PIE v3 UVs are already normalized (0..1)
                    tex_coords: vec![
                        Vec2::new(0.0, 0.0),
                        Vec2::new(1.0, 0.0),
                        Vec2::new(0.5, 1.0),
                    ],
                    anim_frames: None,
                    anim_rate: None,
                    anim_width: None,
                    anim_height: None,
                }],
                connectors: vec![],
            }],
        }
    }

    #[test]
    fn build_mesh_from_triangle() {
        let pie = make_triangle_pie();
        let mesh = build_mesh(&pie).expect("should build mesh");
        assert_eq!(mesh.vertices.len(), 3);
        assert_eq!(mesh.indices.len(), 3);
        // Reversed winding from Z-flip: [0, 2, 1] instead of [0, 1, 2]
        assert_eq!(mesh.indices, vec![0, 2, 1]);
    }

    #[test]
    fn build_mesh_computes_aabb() {
        let pie = make_triangle_pie();
        let mesh = build_mesh(&pie).expect("should build mesh");
        // Z is negated (PIE Z=north -> editor Z=south), but these vertices
        // have z=0 so AABB is unchanged.
        assert_eq!(mesh.aabb_min, Vec3::new(0.0, 0.0, 0.0));
        assert_eq!(mesh.aabb_max, Vec3::new(100.0, 100.0, 0.0));
    }

    #[test]
    fn build_mesh_v3_preserves_normalized_uvs() {
        // PIE v3 UVs are already in 0..1 range; verify they pass through unchanged.
        let pie = make_triangle_pie();
        let mesh = build_mesh(&pie).expect("should build mesh");
        // Vertex 0: (0.0, 0.0), Vertex 1: (1.0, 0.0), Vertex 2: (0.5, 1.0)
        assert!((mesh.vertices[0].tex_coord[0] - 0.0).abs() < 1e-5);
        assert!((mesh.vertices[0].tex_coord[1] - 0.0).abs() < 1e-5);
        assert!((mesh.vertices[1].tex_coord[0] - 1.0).abs() < 1e-5);
        assert!((mesh.vertices[1].tex_coord[1] - 0.0).abs() < 1e-5);
        assert!((mesh.vertices[2].tex_coord[0] - 0.5).abs() < 1e-5);
        assert!((mesh.vertices[2].tex_coord[1] - 1.0).abs() < 1e-5);
    }

    #[test]
    fn build_mesh_quad_fan_triangulation() {
        let pie = PieModel {
            version: 3,
            model_type: 0,
            texture_page: "test.png".to_string(),
            texture_width: 256,
            texture_height: 256,
            texture_pages: vec!["test.png".to_string()],
            tcmask_pages: vec![],
            normal_page: None,
            specular_page: None,
            event_page: None,
            levels: vec![PieLevel {
                vertices: vec![
                    Vec3::new(0.0, 0.0, 0.0),
                    Vec3::new(100.0, 0.0, 0.0),
                    Vec3::new(100.0, 0.0, 100.0),
                    Vec3::new(0.0, 0.0, 100.0),
                ],
                polygons: vec![PiePolygon {
                    flags: 0x200,
                    indices: vec![0, 1, 2, 3],
                    // PIE v3 UVs are already normalized (0..1)
                    tex_coords: vec![
                        Vec2::new(0.0, 0.0),
                        Vec2::new(1.0, 0.0),
                        Vec2::new(1.0, 1.0),
                        Vec2::new(0.0, 1.0),
                    ],
                    anim_frames: None,
                    anim_rate: None,
                    anim_width: None,
                    anim_height: None,
                }],
                connectors: vec![],
            }],
        };
        let mesh = build_mesh(&pie).expect("should build mesh");
        // 4 vertices (one per polygon vertex), 6 indices (2 triangles)
        assert_eq!(mesh.vertices.len(), 4);
        assert_eq!(mesh.indices.len(), 6);
        // Reversed winding from Z-flip
        assert_eq!(mesh.indices, vec![0, 2, 1, 0, 3, 2]);
    }

    #[test]
    fn team_color_valid_player() {
        let c = team_color(0);
        assert_eq!(c, TEAM_COLORS[0]);
    }

    #[test]
    fn team_color_scavenger() {
        let c = team_color(-1);
        assert_eq!(c, [0.5, 0.1, 0.1, 1.0]);
    }

    #[test]
    fn team_color_all_players() {
        for p in 0..16i8 {
            let c = team_color(p);
            assert_eq!(c, TEAM_COLORS[p as usize], "player {p} color mismatch");
        }
    }

    #[test]
    fn team_color_out_of_range() {
        // player >= 16 should get scavenger color
        let c = team_color(16);
        assert_eq!(c, [0.5, 0.1, 0.1, 1.0]);
        let c2 = team_color(127);
        assert_eq!(c2, [0.5, 0.1, 0.1, 1.0]);
    }

    #[test]
    fn build_mesh_empty_polygons_returns_none() {
        let pie = PieModel {
            version: 3,
            model_type: 0,
            texture_page: "test.png".to_string(),
            texture_width: 256,
            texture_height: 256,
            texture_pages: vec![],
            tcmask_pages: vec![],
            normal_page: None,
            specular_page: None,
            event_page: None,
            levels: vec![PieLevel {
                vertices: vec![Vec3::new(0.0, 0.0, 0.0)],
                polygons: vec![],
                connectors: vec![],
            }],
        };
        assert!(build_mesh(&pie).is_none());
    }

    #[test]
    fn build_mesh_empty_levels_returns_none() {
        let pie = PieModel {
            version: 3,
            model_type: 0,
            texture_page: "test.png".to_string(),
            texture_width: 256,
            texture_height: 256,
            texture_pages: vec![],
            tcmask_pages: vec![],
            normal_page: None,
            specular_page: None,
            event_page: None,
            levels: vec![],
        };
        assert!(build_mesh(&pie).is_none());
    }

    #[test]
    fn build_mesh_skips_invalid_vertex_indices() {
        let pie = PieModel {
            version: 3,
            model_type: 0,
            texture_page: "test.png".to_string(),
            texture_width: 256,
            texture_height: 256,
            texture_pages: vec![],
            tcmask_pages: vec![],
            normal_page: None,
            specular_page: None,
            event_page: None,
            levels: vec![PieLevel {
                vertices: vec![Vec3::ZERO, Vec3::X],
                polygons: vec![PiePolygon {
                    flags: 0x200,
                    indices: vec![0, 1, 99], // index 99 is out of bounds
                    tex_coords: vec![Vec2::ZERO, Vec2::ZERO, Vec2::ZERO],
                    anim_frames: None,
                    anim_rate: None,
                    anim_width: None,
                    anim_height: None,
                }],
                connectors: vec![],
            }],
        };
        // Should not crash; the polygon is skipped due to out-of-bounds index
        assert!(build_mesh(&pie).is_none());
    }

    #[test]
    fn build_mesh_skips_degenerate_polygon() {
        // Polygon with < 3 vertices should be skipped
        let pie = PieModel {
            version: 3,
            model_type: 0,
            texture_page: "test.png".to_string(),
            texture_width: 256,
            texture_height: 256,
            texture_pages: vec![],
            tcmask_pages: vec![],
            normal_page: None,
            specular_page: None,
            event_page: None,
            levels: vec![PieLevel {
                vertices: vec![Vec3::ZERO, Vec3::X],
                polygons: vec![PiePolygon {
                    flags: 0x200,
                    indices: vec![0, 1], // only 2 vertices - not a triangle
                    tex_coords: vec![Vec2::ZERO, Vec2::ZERO],
                    anim_frames: None,
                    anim_rate: None,
                    anim_width: None,
                    anim_height: None,
                }],
                connectors: vec![],
            }],
        };
        assert!(build_mesh(&pie).is_none());
    }

    #[test]
    fn build_mesh_pie_v2_normalizes_uvs() {
        // PIE v2 UVs are pixel coords, should be divided by texture dimensions
        let pie = PieModel {
            version: 2,
            model_type: 0,
            texture_page: "test.png".to_string(),
            texture_width: 256,
            texture_height: 128,
            texture_pages: vec![],
            tcmask_pages: vec![],
            normal_page: None,
            specular_page: None,
            event_page: None,
            levels: vec![PieLevel {
                vertices: vec![
                    Vec3::new(0.0, 0.0, 0.0),
                    Vec3::new(100.0, 0.0, 0.0),
                    Vec3::new(50.0, 100.0, 0.0),
                ],
                polygons: vec![PiePolygon {
                    flags: 0x200,
                    indices: vec![0, 1, 2],
                    tex_coords: vec![
                        Vec2::new(128.0, 64.0), // should become 0.5, 0.5
                        Vec2::new(256.0, 0.0),  // should become 1.0, 0.0
                        Vec2::new(0.0, 128.0),  // should become 0.0, 1.0
                    ],
                    anim_frames: None,
                    anim_rate: None,
                    anim_width: None,
                    anim_height: None,
                }],
                connectors: vec![],
            }],
        };
        let mesh = build_mesh(&pie).unwrap();
        assert!((mesh.vertices[0].tex_coord[0] - 0.5).abs() < 1e-5);
        assert!((mesh.vertices[0].tex_coord[1] - 0.5).abs() < 1e-5);
        assert!((mesh.vertices[1].tex_coord[0] - 1.0).abs() < 1e-5);
        assert!((mesh.vertices[2].tex_coord[1] - 1.0).abs() < 1e-5);
    }

    #[test]
    fn build_mesh_pentagon_fan_triangulation() {
        // 5-vertex polygon should produce 3 triangles (fan from vertex 0)
        let pie = PieModel {
            version: 3,
            model_type: 0,
            texture_page: "test.png".to_string(),
            texture_width: 256,
            texture_height: 256,
            texture_pages: vec![],
            tcmask_pages: vec![],
            normal_page: None,
            specular_page: None,
            event_page: None,
            levels: vec![PieLevel {
                vertices: vec![
                    Vec3::new(0.0, 0.0, 0.0),
                    Vec3::new(1.0, 0.0, 0.0),
                    Vec3::new(1.5, 0.0, 1.0),
                    Vec3::new(0.5, 0.0, 1.5),
                    Vec3::new(-0.5, 0.0, 0.5),
                ],
                polygons: vec![PiePolygon {
                    flags: 0x200,
                    indices: vec![0, 1, 2, 3, 4],
                    tex_coords: vec![Vec2::ZERO; 5],
                    anim_frames: None,
                    anim_rate: None,
                    anim_width: None,
                    anim_height: None,
                }],
                connectors: vec![],
            }],
        };
        let mesh = build_mesh(&pie).unwrap();
        assert_eq!(mesh.vertices.len(), 5);
        assert_eq!(mesh.indices.len(), 9); // 3 triangles × 3 indices
        // Fan: (0,1,2), (0,2,3), (0,3,4)
        // Reversed winding from Z-flip
        assert_eq!(mesh.indices, vec![0, 2, 1, 0, 3, 2, 0, 4, 3]);
    }

    #[test]
    fn tangent_computed_for_xz_plane_triangle() {
        // Triangle lying flat on the XZ plane with standard UV mapping.
        // Expected tangent should point along X (U increases with X).
        let v0 = Vec3::new(0.0, 0.0, 0.0);
        let v1 = Vec3::new(100.0, 0.0, 0.0);
        let v2 = Vec3::new(0.0, 0.0, 100.0);
        let uv0 = Vec2::new(0.0, 0.0);
        let uv1 = Vec2::new(1.0, 0.0);
        let uv2 = Vec2::new(0.0, 1.0);
        let normal = Vec3::new(0.0, 1.0, 0.0); // Y-up

        let t = compute_tangent(v0, v1, v2, uv0, uv1, uv2, normal);
        // Tangent should be approximately along +X
        assert!(t[0] > 0.9, "tangent X should be ~1.0, got {}", t[0]);
        assert!(t[1].abs() < 0.1, "tangent Y should be ~0, got {}", t[1]);
        assert!(t[2].abs() < 0.1, "tangent Z should be ~0, got {}", t[2]);
        // Handedness should be defined (+1 or -1)
        assert!(t[3].abs() > 0.5, "handedness should be ±1, got {}", t[3]);
    }

    #[test]
    fn tangent_fallback_for_degenerate_uv() {
        // All UVs at the same point - degenerate mapping.
        let v0 = Vec3::new(0.0, 0.0, 0.0);
        let v1 = Vec3::new(100.0, 0.0, 0.0);
        let v2 = Vec3::new(0.0, 100.0, 0.0);
        let uv = Vec2::new(0.5, 0.5);
        let normal = Vec3::new(0.0, 0.0, 1.0);

        let t = compute_tangent(v0, v1, v2, uv, uv, uv, normal);
        // Should produce a valid (non-zero) tangent perpendicular to normal.
        let len = (t[0] * t[0] + t[1] * t[1] + t[2] * t[2]).sqrt();
        assert!(
            (len - 1.0).abs() < 0.01,
            "degenerate tangent should be unit length, got {len}"
        );
        // Tangent should be perpendicular to the normal (Z-axis)
        let dot = t[0] * normal.x + t[1] * normal.y + t[2] * normal.z;
        assert!(
            dot.abs() < 0.01,
            "tangent should be perpendicular to normal, dot={dot}"
        );
    }

    #[test]
    fn mesh_vertices_have_tangent_field() {
        let pie = make_triangle_pie();
        let mesh = build_mesh(&pie).expect("should build mesh");
        // All vertices should have a tangent with unit-length xyz
        for v in &mesh.vertices {
            let len = (v.tangent[0] * v.tangent[0]
                + v.tangent[1] * v.tangent[1]
                + v.tangent[2] * v.tangent[2])
                .sqrt();
            assert!(
                (len - 1.0).abs() < 0.01,
                "tangent should be unit length, got {len}"
            );
            assert!(
                v.tangent[3].abs() > 0.5,
                "handedness should be ±1, got {}",
                v.tangent[3]
            );
        }
    }

    #[test]
    fn build_mesh_combines_multiple_levels() {
        // PIE with 2 levels, both in the same coordinate space.
        // Level 1 connector should NOT offset level 2 vertices.
        let pie = PieModel {
            version: 3,
            model_type: 0,
            texture_page: "test.png".to_string(),
            texture_width: 256,
            texture_height: 256,
            texture_pages: vec![],
            tcmask_pages: vec![],
            normal_page: None,
            specular_page: None,
            event_page: None,
            levels: vec![
                PieLevel {
                    vertices: vec![
                        Vec3::new(0.0, 0.0, 0.0),
                        Vec3::new(10.0, 0.0, 0.0),
                        Vec3::new(5.0, 10.0, 0.0),
                    ],
                    polygons: vec![PiePolygon {
                        flags: 0x200,
                        indices: vec![0, 1, 2],
                        tex_coords: vec![
                            Vec2::new(0.0, 0.0),
                            Vec2::new(1.0, 0.0),
                            Vec2::new(0.5, 1.0),
                        ],
                        anim_frames: None,
                        anim_rate: None,
                        anim_width: None,
                        anim_height: None,
                    }],
                    connectors: vec![Vec3::new(5.0, 100.0, 10.0)], // should be ignored
                },
                PieLevel {
                    vertices: vec![
                        Vec3::new(0.0, 20.0, 0.0),
                        Vec3::new(10.0, 20.0, 0.0),
                        Vec3::new(5.0, 30.0, 0.0),
                    ],
                    polygons: vec![PiePolygon {
                        flags: 0x200,
                        indices: vec![0, 1, 2],
                        tex_coords: vec![
                            Vec2::new(0.0, 0.0),
                            Vec2::new(1.0, 0.0),
                            Vec2::new(0.5, 1.0),
                        ],
                        anim_frames: None,
                        anim_rate: None,
                        anim_width: None,
                        anim_height: None,
                    }],
                    connectors: vec![],
                },
            ],
        };
        let mesh = build_mesh(&pie).expect("should build multi-level mesh");
        // Should have 6 vertices (3 per level), 6 indices (3 per level)
        assert_eq!(mesh.vertices.len(), 6);
        assert_eq!(mesh.indices.len(), 6);
        // Level 2 indices should be offset by 3 (base_index)
        // Reversed winding from Z-flip
        assert_eq!(mesh.indices, vec![0, 2, 1, 3, 5, 4]);
        // Level 2 vertex Y should NOT be offset by connector Y
        assert!(
            (mesh.vertices[3].position[1] - 20.0).abs() < 0.01,
            "level 2 vertex Y should be 20.0, got {}",
            mesh.vertices[3].position[1]
        );
    }
}
