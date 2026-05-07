//! Water surface mesh generation from terrain type data.
//!
//! Each connected water body becomes a flat plane. Terrain is lowered at
//! water vertices by a graduated offset (see `terrain.rs`), and shore tiles
//! (non-water adjacent to water) are included so the depth buffer clips
//! water where terrain rises above it; alpha fading produces smooth curved
//! shoreline edges.

use std::collections::{HashMap, VecDeque};
use wz_maplib::MapData;
use wz_maplib::constants::TILE_UNITS_F32 as TILE_UNITS;
use wz_maplib::terrain_types::{TerrainType, TerrainTypeData};

/// Matches WZ2100's `world_coord(1)/3` (~42.67). Interior water vertices
/// (all 4 adjacent tiles are water) get the full offset; shore vertices
/// get a sqrt-graduated fraction.
pub const WATER_DEPTH_OFFSET: f32 = 42.0;

/// Clearance above average lowered terrain for the water surface plane.
/// Must clear interior terrain (lowered ~42 units) but stay below shore
/// terrain (~21 units lowered).
const WATER_SURFACE_CLEARANCE: f32 = 10.0;

/// A single water vertex sent to the GPU.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct WaterVertex {
    pub position: [f32; 3],
    /// How far below the water surface the terrain is at this vertex.
    /// Used by the shader for alpha fading and depth-dependent effects.
    pub depth: f32,
}

impl WaterVertex {
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
                format: wgpu::VertexFormat::Float32,
            },
        ];

        wgpu::VertexBufferLayout {
            array_stride: size_of::<WaterVertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: ATTRS,
        }
    }
}

/// CPU-side water mesh ready to upload to the GPU.
#[derive(Debug)]
pub struct WaterMesh {
    pub vertices: Vec<WaterVertex>,
    pub indices: Vec<u32>,
}

/// Per-vertex water depth offsets for the `(w+1) x (h+1)` vertex grid.
///
/// Depth is graduated by how many of the 4 adjacent tiles are water, then
/// blurred twice so the terrain slopes gradually into water basins instead
/// of stepping at tile edges. Shared with `terrain.rs`.
pub fn build_water_vertex_depths(map: &MapData, terrain_types: &TerrainTypeData) -> Vec<f32> {
    let vw = (map.width + 1) as usize;
    let vh = (map.height + 1) as usize;

    let mut depths: Vec<f32> = (0..vh)
        .flat_map(|vy| {
            (0..vw).map(move |vx| {
                let mut count = 0u32;
                let map_vw = vw as u32;
                let map_vh = vh as u32;
                // 4 adjacent tiles around this vertex (matches WZ2100 isWater).
                if vx > 0
                    && vy > 0
                    && is_water_tile(map, terrain_types, vx as u32 - 1, vy as u32 - 1)
                {
                    count += 1;
                }
                if (vx as u32) < map_vw - 1
                    && vy > 0
                    && is_water_tile(map, terrain_types, vx as u32, vy as u32 - 1)
                {
                    count += 1;
                }
                if vx > 0
                    && (vy as u32) < map_vh - 1
                    && is_water_tile(map, terrain_types, vx as u32 - 1, vy as u32)
                {
                    count += 1;
                }
                if (vx as u32) < map_vw - 1
                    && (vy as u32) < map_vh - 1
                    && is_water_tile(map, terrain_types, vx as u32, vy as u32)
                {
                    count += 1;
                }
                WATER_DEPTH_OFFSET * (count as f32 / 4.0).sqrt()
            })
        })
        .collect();

    // Two blur passes; without them depth clips in a staircase pattern at
    // tile boundaries. Scratch buffer is ping-ponged to avoid cloning the
    // full vertex grid each pass.
    let mut scratch: Vec<f32> = vec![0.0; depths.len()];
    for _ in 0..2 {
        scratch.copy_from_slice(&depths);
        for vy in 0..vh {
            for vx in 0..vw {
                let idx = vy * vw + vx;
                let mut sum = scratch[idx];
                let mut count = 1.0_f32;
                if vx > 0 {
                    sum += scratch[idx - 1];
                    count += 1.0;
                }
                if vx + 1 < vw {
                    sum += scratch[idx + 1];
                    count += 1.0;
                }
                if vy > 0 {
                    sum += scratch[idx - vw];
                    count += 1.0;
                }
                if vy + 1 < vh {
                    sum += scratch[idx + vw];
                    count += 1.0;
                }
                depths[idx] = sum / count;
            }
        }
    }

    depths
}

impl WaterMesh {
    /// Build a flat water mesh per connected water body with shore expansion.
    ///
    /// At shore tiles the flat water plane sits above lowered water terrain
    /// but below un-lowered shore terrain, so the depth buffer clips water
    /// against the rising shore for smooth curved contours.
    pub fn from_map(map: &MapData, terrain_types: &TerrainTypeData) -> Self {
        let w = map.width;
        let h = map.height;
        let vw = (w + 1) as usize;

        let water_depths = build_water_vertex_depths(map, terrain_types);

        let tile_count = (w * h) as usize;
        let mut body_id = vec![0u32; tile_count];
        let mut body_levels: Vec<f32> = Vec::new();
        let mut current_body = 0u32;

        for start_y in 0..h {
            for start_x in 0..w {
                let start_idx = (start_y * w + start_x) as usize;
                if body_id[start_idx] != 0 || !is_water_tile(map, terrain_types, start_x, start_y) {
                    continue;
                }

                current_body += 1;
                let mut queue = VecDeque::new();
                queue.push_back((start_x, start_y));
                body_id[start_idx] = current_body;

                let mut sum_lowered_h = 0.0_f64;
                let mut vertex_count = 0u32;

                while let Some((tx, ty)) = queue.pop_front() {
                    let corners = [(tx, ty), (tx + 1, ty), (tx, ty + 1), (tx + 1, ty + 1)];
                    for &(vx, vy) in &corners {
                        let original_h = vertex_height(map, vx, vy);
                        let depth = water_depths[vy as usize * vw + vx as usize];
                        let lowered_h = original_h - depth;
                        sum_lowered_h += lowered_h as f64;
                        vertex_count += 1;
                    }

                    for (nx, ny) in neighbors_4(tx, ty, w, h) {
                        let ni = (ny * w + nx) as usize;
                        if body_id[ni] == 0 && is_water_tile(map, terrain_types, nx, ny) {
                            body_id[ni] = current_body;
                            queue.push_back((nx, ny));
                        }
                    }
                }

                // Average is dominated by interior vertices (lowered ~42 units).
                let avg = (sum_lowered_h / vertex_count.max(1) as f64) as f32;
                body_levels.push(avg + WATER_SURFACE_CLEARANCE);
            }
        }

        // Two layers of shore expansion widen the alpha fade zone for
        // organic curved shorelines instead of staircase edges.
        let mut shore_body = vec![0u32; tile_count];

        // Layer 1: non-water tiles directly adjacent to water tiles.
        for ty in 0..h {
            for tx in 0..w {
                let ti = (ty * w + tx) as usize;
                if body_id[ti] != 0 {
                    continue;
                }
                for (nx, ny) in neighbors_4(tx, ty, w, h) {
                    let ni = (ny * w + nx) as usize;
                    if body_id[ni] != 0 {
                        shore_body[ti] = body_id[ni];
                        break;
                    }
                }
            }
        }

        // Layer 2: non-water tiles adjacent to layer-1 shore tiles.
        let shore_layer1 = shore_body.clone();
        for ty in 0..h {
            for tx in 0..w {
                let ti = (ty * w + tx) as usize;
                if body_id[ti] != 0 || shore_body[ti] != 0 {
                    continue;
                }
                for (nx, ny) in neighbors_4(tx, ty, w, h) {
                    let ni = (ny * w + nx) as usize;
                    if shore_layer1[ni] != 0 {
                        shore_body[ti] = shore_layer1[ni];
                        break;
                    }
                }
            }
        }

        let mut vertex_map: HashMap<(u32, u32, u32), u32> = HashMap::new();
        let mut vertices: Vec<WaterVertex> = Vec::new();
        let mut indices: Vec<u32> = Vec::new();

        for ty in 0..h {
            for tx in 0..w {
                let ti = (ty * w + tx) as usize;
                let bid = if body_id[ti] != 0 {
                    body_id[ti]
                } else {
                    shore_body[ti]
                };
                if bid == 0 {
                    continue;
                }

                let water_level = body_levels[(bid - 1) as usize];
                let corners = [(tx, ty), (tx + 1, ty), (tx, ty + 1), (tx + 1, ty + 1)];
                let mut ci = [0u32; 4];

                for (i, &(vx, vy)) in corners.iter().enumerate() {
                    let idx = vertex_map.entry((vx, vy, bid)).or_insert_with(|| {
                        let depth = water_depths[vy as usize * vw + vx as usize];

                        // All vertices sit at the flat water_level. Shore
                        // vertices (depth=0) fade to transparent via alpha.
                        // Anchoring shore vertices at terrain height instead
                        // would let water triangles climb cliff faces.
                        let idx = vertices.len() as u32;
                        vertices.push(WaterVertex {
                            position: [vx as f32 * TILE_UNITS, water_level, vy as f32 * TILE_UNITS],
                            depth,
                        });
                        idx
                    });
                    ci[i] = *idx;
                }

                indices.extend_from_slice(&[ci[0], ci[1], ci[3], ci[0], ci[3], ci[2]]);
            }
        }

        Self { vertices, indices }
    }
}

fn neighbors_4(tx: u32, ty: u32, w: u32, h: u32) -> impl Iterator<Item = (u32, u32)> {
    let mut out = [(0u32, 0u32); 4];
    let mut n = 0;
    if tx > 0 {
        out[n] = (tx - 1, ty);
        n += 1;
    }
    if tx + 1 < w {
        out[n] = (tx + 1, ty);
        n += 1;
    }
    if ty > 0 {
        out[n] = (tx, ty - 1);
        n += 1;
    }
    if ty + 1 < h {
        out[n] = (tx, ty + 1);
        n += 1;
    }
    out.into_iter().take(n)
}

pub fn is_water_tile(map: &MapData, terrain_types: &TerrainTypeData, tx: u32, ty: u32) -> bool {
    map.tile(tx, ty).and_then(|t| {
        terrain_types
            .terrain_types
            .get(t.texture_id() as usize)
            .copied()
    }) == Some(TerrainType::Water)
}

fn vertex_height(map: &MapData, x: u32, y: u32) -> f32 {
    let cx = x.min(map.width.saturating_sub(1));
    let cy = y.min(map.height.saturating_sub(1));
    map.tile(cx, cy).map_or(0.0, |t| t.height as f32)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Create a 4x4 map where tile (1,1) and (2,1) are water (`texture_id=7`).
    fn make_test_map() -> (MapData, TerrainTypeData) {
        let mut map = MapData::new(4, 4);
        // Set all tiles to height 100, texture 0 (sand).
        for tile in &mut map.tiles {
            tile.height = 100;
        }
        // Mark tiles (1,1) and (2,1) as water by setting texture_id=7.
        map.tiles[5_usize].texture = 7;
        map.tiles[6_usize].texture = 7;

        // TerrainTypeData: index 7 = Water, others = Sand.
        let mut terrain_types = vec![TerrainType::Sand; 8];
        terrain_types[7] = TerrainType::Water;
        let ttp = TerrainTypeData { terrain_types };

        (map, ttp)
    }

    #[test]
    fn is_water_tile_identifies_water() {
        let (map, ttp) = make_test_map();
        assert!(is_water_tile(&map, &ttp, 1, 1));
        assert!(is_water_tile(&map, &ttp, 2, 1));
        assert!(!is_water_tile(&map, &ttp, 0, 0));
        assert!(!is_water_tile(&map, &ttp, 3, 3));
    }

    #[test]
    fn neighbors_4_interior() {
        let neighbors: Vec<_> = neighbors_4(2, 2, 4, 4).collect();
        assert_eq!(neighbors.len(), 4);
        assert!(neighbors.contains(&(1, 2)));
        assert!(neighbors.contains(&(3, 2)));
        assert!(neighbors.contains(&(2, 1)));
        assert!(neighbors.contains(&(2, 3)));
    }

    #[test]
    fn neighbors_4_corner() {
        let neighbors: Vec<_> = neighbors_4(0, 0, 4, 4).collect();
        assert_eq!(neighbors.len(), 2);
        assert!(neighbors.contains(&(1, 0)));
        assert!(neighbors.contains(&(0, 1)));
    }

    #[test]
    fn water_vertex_depths_graduated() {
        let (map, ttp) = make_test_map();
        let depths = build_water_vertex_depths(&map, &ttp);
        let vw = 5; // 4+1

        // Vertex (0,0): no adjacent water tiles → depth ~0 (blur may spread slightly).
        assert!(depths[0] < 5.0, "far vertex should have near-zero depth");

        // Vertex (2,2): shared by water tiles (1,1) and (2,1) plus two
        // non-water tiles → interior-ish. After blur, should be moderate.
        let v22 = depths[2 * vw + 2];
        assert!(
            v22 > 5.0,
            "water-adjacent vertex should have nonzero depth: {v22}"
        );

        // All depths should be in [0, WATER_DEPTH_OFFSET].
        for &d in &depths {
            assert!(d >= 0.0, "depth must be non-negative");
            assert!(
                d <= WATER_DEPTH_OFFSET + 1.0,
                "depth must not exceed max offset"
            );
        }
    }

    #[test]
    fn water_mesh_finds_one_body() {
        let (map, ttp) = make_test_map();
        let mesh = WaterMesh::from_map(&map, &ttp);

        // Two water tiles + shore expansion should produce a non-empty mesh.
        assert!(!mesh.vertices.is_empty(), "water mesh should have vertices");
        assert!(!mesh.indices.is_empty(), "water mesh should have indices");
        // Index count must be a multiple of 3 (triangles).
        assert_eq!(
            mesh.indices.len() % 3,
            0,
            "indices must form complete triangles"
        );
    }

    #[test]
    fn water_mesh_vertices_at_flat_level() {
        let (map, ttp) = make_test_map();
        let mesh = WaterMesh::from_map(&map, &ttp);

        // All vertices should be at the same Y (flat water plane per body).
        let y_values: Vec<f32> = mesh.vertices.iter().map(|v| v.position[1]).collect();
        let first_y = y_values[0];
        for (i, &y) in y_values.iter().enumerate() {
            assert!(
                (y - first_y).abs() < 0.001,
                "vertex {i} Y={y} differs from first Y={first_y}, water plane must be flat"
            );
        }
    }

    #[test]
    fn water_mesh_no_water_produces_empty() {
        let map = MapData::new(4, 4);
        let ttp = TerrainTypeData {
            terrain_types: vec![TerrainType::Sand; 8],
        };
        let mesh = WaterMesh::from_map(&map, &ttp);
        assert!(mesh.vertices.is_empty());
        assert!(mesh.indices.is_empty());
    }

    #[test]
    fn water_depth_blur_smooths_boundary() {
        let (map, ttp) = make_test_map();
        let depths = build_water_vertex_depths(&map, &ttp);
        let vw = 5;

        // After 2 blur passes, vertices 1 tile away from water should have
        // nonzero depth (the blur spreads lowering outward).
        // Vertex (1,0) is above water tile (1,1) - should get some blur spread.
        // The `0 * vw` keeps the row,col indexing pattern consistent with
        // the surrounding code; the explicit form documents the layout.
        #[expect(clippy::erasing_op, reason = "row * stride + col layout")]
        let v10 = depths[0 * vw + 1];
        // Vertex (0,0) is 2 tiles from water - should be near zero.
        let v00 = depths[0];
        assert!(
            v10 > v00,
            "vertex closer to water should have higher depth after blur: v10={v10}, v00={v00}"
        );
    }
}
