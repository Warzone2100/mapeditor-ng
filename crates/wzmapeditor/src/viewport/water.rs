//! Water surface mesh and riverbed generation, matching WZ2100.
//!
//! At load the game digs every interior water vertex hundreds of units down
//! into a riverbed (`generateRiverbed`, `src/map.cpp`) and then draws a water
//! sheet across the whole grid at `waterLevel = height - 128/3`; under land
//! the sheet sits below the surface and the depth buffer clips it, which is
//! what carves the shoreline. Both halves are mirrored here so the viewport
//! shows what the game will show.

use wz_maplib::MapData;
use wz_maplib::constants::TILE_UNITS_F32 as TILE_UNITS;
use wz_maplib::terrain_types::{TerrainType, TerrainTypeData};

/// Water surface offset below terrain height: WZ2100's `world_coord(1)/3`.
pub const WATER_LEVEL_OFFSET: f32 = 128.0 / 3.0;

/// WZ2100 `src/map.cpp` riverbed depth bounds, in world units.
const WATER_MIN_DEPTH: i32 = 500;
const WATER_MAX_DEPTH: i32 = 900;

/// WZ2100's Mersenne Twister (`src/random.cpp`), which is the standard
/// MT19937. Replicated so the riverbed jitter is bit-identical to the game's
/// for the same map.
struct MersenneTwister {
    state: [u32; 624],
    offset: usize,
}

impl MersenneTwister {
    fn new(seed: u32) -> Self {
        let mut state = [0u32; 624];
        state[0] = seed;
        for i in 1..624 {
            state[i] = 0x6C07_8965u32
                .wrapping_mul(state[i - 1] ^ (state[i - 1] >> 30))
                .wrapping_add(i as u32);
        }
        Self { state, offset: 624 }
    }

    fn u32(&mut self) -> u32 {
        if self.offset == 624 {
            self.generate();
        }
        let mut ret = self.state[self.offset];
        self.offset += 1;
        ret ^= ret >> 11;
        ret ^= (ret << 7) & 0x9D2C_5680;
        ret ^= (ret << 15) & 0xEFC6_0000;
        ret ^= ret >> 18;
        ret
    }

    fn generate(&mut self) {
        self.offset = 0;
        for i in 0..624 {
            let v = (self.state[i] & 0x8000_0000) | (self.state[(i + 1) % 624] & 0x7FFF_FFFF);
            self.state[i] = self.state[(i + 397) % 624] ^ (v >> 1) ^ ((v & 1) * 0x9908_B0DF);
        }
    }
}

/// A single water vertex sent to the GPU.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct WaterVertex {
    pub position: [f32; 3],
    /// Water level minus riverbed height in world units (WZ2100's `vertex.w`).
    /// Negative under land, where the sheet is depth-clipped anyway.
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

/// WZ2100 `isWaterVertex`: an interior vertex whose 4 surrounding tiles are
/// all water. Only these are dug into the riverbed.
fn is_water_vertex(map: &MapData, terrain_types: &TerrainTypeData, x: u32, y: u32) -> bool {
    if x < 1 || y < 1 || x > map.width - 1 || y > map.height - 1 {
        return false;
    }
    is_water_tile(map, terrain_types, x, y)
        && is_water_tile(map, terrain_types, x - 1, y)
        && is_water_tile(map, terrain_types, x, y - 1)
        && is_water_tile(map, terrain_types, x - 1, y - 1)
}

/// Per-vertex riverbed dig depths for the `(w+1) x (h+1)` grid.
///
/// Ports WZ2100's `generateRiverbed` exactly, integer math and jitter
/// included: a distance-from-shore diffusion over water vertices, then a dig
/// of roughly 100 world units at the shore deepening past 500 mid-lake.
/// Vertices that are not surrounded by water stay at 0.
pub fn build_water_vertex_depths(map: &MapData, terrain_types: &TerrainTypeData) -> Vec<f32> {
    let w = map.width as usize;
    let h = map.height as usize;
    let vw = w + 1;
    let vh = h + 1;
    let mut out = vec![0.0f32; vw * vh];

    // The game's grid is tile-indexed (w x h); our extra right/bottom vertex
    // row and column mirror its never-dug border.
    let mut idx = vec![0i32; w * h];
    let mut any_water = false;
    for y in 0..h {
        for x in 0..w {
            if is_water_vertex(map, terrain_types, x as u32, y as u32) {
                idx[y * w + x] = 100;
                any_water = true;
            }
        }
    }
    if !any_water {
        return out;
    }

    let mut max_idx;
    let mut passes = 0;
    loop {
        max_idx = 1;
        // In-place smoothing over already-updated left/up neighbours, and the
        // asymmetric bounds skipping a 2-wide right/bottom band, both match
        // the game -- the dig must reproduce its bed exactly.
        for y in 1..h.saturating_sub(2) {
            for x in 1..w.saturating_sub(2) {
                let i = y * w + x;
                if idx[i] > 0 {
                    idx[i] = (idx[i - 1] + idx[i - w] + idx[i + w] + idx[i + 1]) / 4;
                    max_idx = max_idx.max(idx[i]);
                }
            }
        }
        passes += 1;
        if max_idx <= 90 || passes >= 20 {
            break;
        }
    }

    let mut mt = MersenneTwister::new(12345);
    for y in 0..h {
        for x in 0..w {
            let v = idx[y * w + x].clamp(1, max_idx);
            if is_water_vertex(map, terrain_types, x as u32, y as u32) {
                let jitter = (mt.u32() % (max_idx as u32 / 6 + 1)) as i32;
                let l = (WATER_MAX_DEPTH + 1 - WATER_MIN_DEPTH) * (max_idx - v - jitter);
                out[y * vw + x] = (WATER_MIN_DEPTH - l / max_idx) as f32;
            }
        }
    }

    out
}

impl WaterMesh {
    /// Build the water sheet: one vertex per grid point at
    /// `height - WATER_LEVEL_OFFSET`, quads over the whole map.
    ///
    /// The per-vertex `depth` is water level minus the dug riverbed height,
    /// exactly WZ2100's `waterHeight - pos.y`; the shader maps it to shore
    /// translucency.
    pub fn from_map(map: &MapData, terrain_types: &TerrainTypeData) -> Self {
        let w = map.width;
        let h = map.height;

        let any_water = (0..h).any(|ty| (0..w).any(|tx| is_water_tile(map, terrain_types, tx, ty)));
        if !any_water {
            return Self {
                vertices: Vec::new(),
                indices: Vec::new(),
            };
        }

        let digs = build_water_vertex_depths(map, terrain_types);
        let vw = (w + 1) as usize;

        let mut vertices = Vec::with_capacity(vw * (h as usize + 1));
        for vy in 0..=h {
            for vx in 0..=w {
                let surface = vertex_height(map, vx, vy) - WATER_LEVEL_OFFSET;
                let dig = digs[vy as usize * vw + vx as usize];
                vertices.push(WaterVertex {
                    position: [vx as f32 * TILE_UNITS, surface, vy as f32 * TILE_UNITS],
                    depth: dig - WATER_LEVEL_OFFSET,
                });
            }
        }

        let mut indices = Vec::with_capacity((w * h * 6) as usize);
        for ty in 0..h {
            for tx in 0..w {
                let tl = ty * (w + 1) + tx;
                let tr = tl + 1;
                let bl = tl + w + 1;
                let br = bl + 1;
                indices.extend_from_slice(&[tl, tr, br, tl, br, bl]);
            }
        }

        Self { vertices, indices }
    }
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

    /// An 8x8 map that is entirely water at height 100.
    fn make_lake_map() -> (MapData, TerrainTypeData) {
        let mut map = MapData::new(8, 8);
        for tile in &mut map.tiles {
            tile.height = 100;
            tile.texture = 7;
        }
        let mut terrain_types = vec![TerrainType::Sand; 8];
        terrain_types[7] = TerrainType::Water;
        (map, TerrainTypeData { terrain_types })
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
    fn mersenne_twister_matches_reference() {
        // Canonical MT19937 outputs for the default seed; WZ2100's
        // implementation is the standard algorithm, so these pin equivalence.
        let mut mt = MersenneTwister::new(5489);
        assert_eq!(mt.u32(), 3_499_211_612);
        assert_eq!(mt.u32(), 581_869_302);
        assert_eq!(mt.u32(), 3_890_346_734);
    }

    #[test]
    fn no_dig_without_surrounded_vertices() {
        // A 2-tile pond has no vertex with water on all 4 sides.
        let (map, ttp) = make_test_map();
        let digs = build_water_vertex_depths(&map, &ttp);
        assert!(digs.iter().all(|&d| d == 0.0));
    }

    #[test]
    fn lake_interior_is_dug_within_game_bounds() {
        let (map, ttp) = make_lake_map();
        let digs = build_water_vertex_depths(&map, &ttp);
        let vw = 9;

        // Map border vertices are never water vertices.
        assert_eq!(digs[0], 0.0);
        assert_eq!(digs[4], 0.0);

        // Interior digs land between the shore minimum (~100) and
        // WATER_MIN_DEPTH plus the jitter margin.
        let centre = digs[4 * vw + 4];
        assert!(
            centre > 90.0 && centre < 620.0,
            "centre dig {centre} outside expected range"
        );
    }

    #[test]
    fn dig_is_deterministic() {
        let (map, ttp) = make_lake_map();
        assert_eq!(
            build_water_vertex_depths(&map, &ttp),
            build_water_vertex_depths(&map, &ttp)
        );
    }

    #[test]
    fn water_sheet_covers_grid_below_terrain() {
        let (map, ttp) = make_test_map();
        let mesh = WaterMesh::from_map(&map, &ttp);

        assert_eq!(mesh.vertices.len(), 5 * 5);
        assert_eq!(mesh.indices.len(), 4 * 4 * 6);
        for v in &mesh.vertices {
            assert!(
                (v.position[1] - (100.0 - WATER_LEVEL_OFFSET)).abs() < 0.001,
                "sheet must sit WATER_LEVEL_OFFSET below terrain, got {}",
                v.position[1]
            );
        }
    }

    #[test]
    fn sheet_depth_is_water_level_minus_riverbed() {
        let (map, ttp) = make_lake_map();
        let mesh = WaterMesh::from_map(&map, &ttp);
        let digs = build_water_vertex_depths(&map, &ttp);
        let vw = 9;

        // Undug corner: sheet is below the (undug) terrain.
        assert!((mesh.vertices[0].depth - -WATER_LEVEL_OFFSET).abs() < 0.001);

        // Dug centre: depth equals dig minus surface offset.
        let centre = 4 * vw + 4;
        assert!((mesh.vertices[centre].depth - (digs[centre] - WATER_LEVEL_OFFSET)).abs() < 0.001);
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
}
