//! CPU-side terrain mesh build: vertex/index/normal generation,
//! atlas-uv math, ground splatting votes, and decal tangent.

use glam::Vec3;
use wz_maplib::MapData;
use wz_maplib::constants::{TILE_MAX_HEIGHT, TILE_UNITS_F32 as TILE_UNITS};
use wz_maplib::terrain_types::TerrainTypeData;

use super::super::ground_types::GroundData;
use super::super::water::build_water_vertex_depths;
use super::{TerrainMesh, TerrainVertex};

/// Used to normalize height into 0..1 for height-based coloring fallback.
const MAX_HEIGHT: f32 = TILE_MAX_HEIGHT as f32;

/// Tile corner positions with graduated water lowering applied.
fn tile_corners_lowered(
    map: &MapData,
    tx: u32,
    ty: u32,
    water_depth: &[f32],
    vw: usize,
) -> (Vec3, Vec3, Vec3, Vec3) {
    let iv_tl = ty as usize * vw + tx as usize;
    let h_tl = get_height(map, tx, ty) - water_depth[iv_tl];
    let h_tr = get_height(map, tx + 1, ty) - water_depth[iv_tl + 1];
    let h_bl = get_height(map, tx, ty + 1) - water_depth[iv_tl + vw];
    let h_br = get_height(map, tx + 1, ty + 1) - water_depth[iv_tl + vw + 1];

    let x0 = tx as f32 * TILE_UNITS;
    let x1 = (tx + 1) as f32 * TILE_UNITS;
    let z0 = ty as f32 * TILE_UNITS;
    let z1 = (ty + 1) as f32 * TILE_UNITS;

    (
        Vec3::new(x0, h_tl, z0),
        Vec3::new(x1, h_tr, z0),
        Vec3::new(x0, h_bl, z1),
        Vec3::new(x1, h_br, z1),
    )
}

impl TerrainMesh {
    /// Build terrain mesh from map data with smooth per-vertex normals.
    ///
    /// Each tile is a quad (4 vertices, 2 triangles). Normals are averaged
    /// from all face normals sharing a vertex position to match WZ2100's
    /// Gouraud shading across tile boundaries.
    ///
    /// When `ground_data` is provided, populates ground-splatting fields
    /// for Medium/High rendering using WZ2100's per-vertex voting so
    /// adjacent tiles agree on ground type at shared edges.
    pub fn from_map(
        map: &MapData,
        ground_data: Option<&GroundData>,
        terrain_types: Option<&TerrainTypeData>,
    ) -> Self {
        let w = map.width;
        let h = map.height;
        let tile_count = (w * h) as usize;

        // Interior water vertices drop by the full offset; shore-adjacent
        // ones drop by a fraction so basins slope naturally without
        // distorting steep cliffs at the shoreline.
        let water_depth: Vec<f32> = if let Some(ttp) = terrain_types {
            build_water_vertex_depths(map, ttp)
        } else {
            vec![0.0; ((w + 1) * (h + 1)) as usize]
        };

        let vw = (w + 1) as usize;
        let vh = (h + 1) as usize;
        let mut normal_accum = vec![Vec3::ZERO; vw * vh];

        for ty in 0..h {
            for tx in 0..w {
                let tile = match map.tile(tx, ty) {
                    Some(t) => *t,
                    None => continue,
                };

                let (p_tl, p_tr, p_bl, p_br) = tile_corners_lowered(map, tx, ty, &water_depth, vw);

                let i_tl = ty as usize * vw + tx as usize;
                let i_tr = i_tl + 1;
                let i_bl = i_tl + vw;
                let i_br = i_bl + 1;

                if tile.tri_flip() {
                    // TR to BL diagonal.
                    let n1 = face_normal(p_tl, p_tr, p_bl);
                    let n2 = face_normal(p_tr, p_br, p_bl);
                    normal_accum[i_tl] += n1;
                    normal_accum[i_tr] += n1;
                    normal_accum[i_bl] += n1;
                    normal_accum[i_tr] += n2;
                    normal_accum[i_br] += n2;
                    normal_accum[i_bl] += n2;
                } else {
                    // TL to BR diagonal.
                    let n1 = face_normal(p_tl, p_tr, p_br);
                    let n2 = face_normal(p_tl, p_br, p_bl);
                    normal_accum[i_tl] += n1;
                    normal_accum[i_tr] += n1;
                    normal_accum[i_br] += n1;
                    normal_accum[i_tl] += n2;
                    normal_accum[i_br] += n2;
                    normal_accum[i_bl] += n2;
                }
            }
        }

        for n in &mut normal_accum {
            *n = n.normalize_or_zero();
            if *n == Vec3::ZERO {
                *n = Vec3::Y;
            }
        }

        let ground_grid = ground_data.map(|gd| gd.build_ground_grid(map));

        let mut vertices = Vec::with_capacity(tile_count * 4);
        let mut indices = Vec::with_capacity(tile_count * 6);

        for ty in 0..h {
            for tx in 0..w {
                let tile = match map.tile(tx, ty) {
                    Some(t) => *t,
                    None => continue,
                };

                let (p_tl, p_tr, p_bl, p_br) = tile_corners_lowered(map, tx, ty, &water_depth, vw);

                let i_tl = ty as usize * vw + tx as usize;
                let i_tr = i_tl + 1;
                let i_bl = i_tl + vw;
                let i_br = i_bl + 1;

                let n_tl = normal_accum[i_tl];
                let n_tr = normal_accum[i_tr];
                let n_bl = normal_accum[i_bl];
                let n_br = normal_accum[i_br];

                let base_idx = vertices.len() as u32;
                let tile_index = tile.texture_id() as f32;
                let uvs = compute_tile_uvs(tile);

                let (ground_indices, tile_no) =
                    if let (Some(gd), Some(gg)) = (ground_data, &ground_grid) {
                        let g_tl = gg[i_tl] as u32;
                        let g_tr = gg[i_tr] as u32;
                        let g_bl = gg[i_bl] as u32;
                        let g_br = gg[i_br] as u32;
                        let gi = [g_tl, g_bl, g_tr, g_br];
                        let tno = if gd.is_decal(tile.texture_id() as u32) {
                            tile.texture_id() as i32
                        } else {
                            -1i32
                        };
                        (gi, tno)
                    } else {
                        ([0u32; 4], -1i32)
                    };

                let decal_tangent = if tile_no >= 0 {
                    compute_decal_tangent(p_tl, p_tr, p_bl, uvs[0], uvs[1], uvs[2])
                } else {
                    [0.0, 0.0, 0.0, 1.0]
                };

                // Ground index slot per corner: TL=0, BL=1, TR=2, BR=3.
                let w_tl = [1.0, 0.0, 0.0, 0.0f32];
                let w_tr = [0.0, 0.0, 1.0, 0.0f32];
                let w_bl = [0.0, 1.0, 0.0, 0.0f32];
                let w_br = [0.0, 0.0, 0.0, 1.0f32];

                vertices.push(make_vertex_ext(
                    p_tl,
                    n_tl,
                    uvs[0],
                    p_tl.y,
                    tile_index,
                    ground_indices,
                    w_tl,
                    tile_no,
                    decal_tangent,
                ));
                vertices.push(make_vertex_ext(
                    p_tr,
                    n_tr,
                    uvs[1],
                    p_tr.y,
                    tile_index,
                    ground_indices,
                    w_tr,
                    tile_no,
                    decal_tangent,
                ));
                vertices.push(make_vertex_ext(
                    p_bl,
                    n_bl,
                    uvs[2],
                    p_bl.y,
                    tile_index,
                    ground_indices,
                    w_bl,
                    tile_no,
                    decal_tangent,
                ));
                vertices.push(make_vertex_ext(
                    p_br,
                    n_br,
                    uvs[3],
                    p_br.y,
                    tile_index,
                    ground_indices,
                    w_br,
                    tile_no,
                    decal_tangent,
                ));

                if tile.tri_flip() {
                    indices.extend_from_slice(&[
                        base_idx,
                        base_idx + 1,
                        base_idx + 2, // TL, TR, BL
                        base_idx + 1,
                        base_idx + 3,
                        base_idx + 2, // TR, BR, BL
                    ]);
                } else {
                    indices.extend_from_slice(&[
                        base_idx,
                        base_idx + 1,
                        base_idx + 3, // TL, TR, BR
                        base_idx,
                        base_idx + 3,
                        base_idx + 2, // TL, BR, BL
                    ]);
                }
            }
        }

        Self { vertices, indices }
    }

    /// Re-emit vertices for tiles in the inclusive rect
    /// `[min_tx..=max_tx, min_ty..=max_ty]` in the same row-major,
    /// 4-vertex-per-tile layout as `from_map`.
    ///
    /// Reuses caller-supplied `water_depth` from a prior full rebuild so
    /// the partial update skips the global blurred-depth pass. Ground
    /// votes are recomputed locally so a brush that just changed a tile
    /// texture sees the new vote. Corner normals are recomputed using all
    /// tiles in `[min_tx-1..=max_tx+1, min_ty-1..=max_ty+1]` (clamped) so
    /// shading stays consistent with neighbouring unchanged geometry.
    pub fn build_tile_rect_vertices(
        map: &MapData,
        ground_data: Option<&GroundData>,
        water_depth: &[f32],
        min_tx: u32,
        min_ty: u32,
        max_tx: u32,
        max_ty: u32,
    ) -> Vec<TerrainVertex> {
        let w = map.width;
        let h = map.height;
        let vw = (w + 1) as usize;

        // Accumulator covers corner vertices [min_tx..=max_tx+1, min_ty..=max_ty+1].
        let vmin_x = min_tx as usize;
        let vmin_y = min_ty as usize;
        let vmax_x = (max_tx + 1) as usize;
        let vmax_y = (max_ty + 1) as usize;
        let lvw = vmax_x - vmin_x + 1;
        let lvh = vmax_y - vmin_y + 1;
        let mut normal_accum = vec![Vec3::ZERO; lvw * lvh];

        let local = |vx: usize, vy: usize| -> Option<usize> {
            if vx < vmin_x || vx > vmax_x || vy < vmin_y || vy > vmax_y {
                None
            } else {
                Some((vy - vmin_y) * lvw + (vx - vmin_x))
            }
        };

        // Recompute corner ground votes locally. A brush that changes a
        // tile's texture also changes the corner vote, and the cached grid
        // wouldn't catch it until the next full rebuild.
        let local_ground: Option<Vec<u8>> = ground_data.map(|gd| {
            let mut grid = vec![0u8; lvw * lvh];
            for vy in vmin_y..=vmax_y {
                for vx in vmin_x..=vmax_x {
                    let i = (vy - vmin_y) * lvw + (vx - vmin_x);
                    grid[i] = gd.vertex_ground_type(map, vx as u32, vy as u32);
                }
            }
            grid
        });

        // Tile (tx, ty) shares corners with vertices (tx, ty), (tx+1, ty),
        // (tx, ty+1), (tx+1, ty+1), so iterate one tile wider than the
        // corner-vertex rect to pick up every face touching a rect corner.
        let nrect_xlo = vmin_x.saturating_sub(1);
        let nrect_ylo = vmin_y.saturating_sub(1);
        let nrect_xhi = vmax_x.min(w.saturating_sub(1) as usize);
        let nrect_yhi = vmax_y.min(h.saturating_sub(1) as usize);

        for ty in nrect_ylo..=nrect_yhi {
            for tx in nrect_xlo..=nrect_xhi {
                let tile = match map.tile(tx as u32, ty as u32) {
                    Some(t) => *t,
                    None => continue,
                };

                let (p_tl, p_tr, p_bl, p_br) =
                    tile_corners_lowered(map, tx as u32, ty as u32, water_depth, vw);

                // Match from_map's accumulation exactly: `acc += n1+n2` and
                // `acc += n1; acc += n2` differ by a ULP because float add
                // isn't associative, so keep separate adds in the same order.
                if tile.tri_flip() {
                    let n1 = face_normal(p_tl, p_tr, p_bl);
                    let n2 = face_normal(p_tr, p_br, p_bl);
                    if let Some(i) = local(tx, ty) {
                        normal_accum[i] += n1;
                    }
                    if let Some(i) = local(tx + 1, ty) {
                        normal_accum[i] += n1;
                    }
                    if let Some(i) = local(tx, ty + 1) {
                        normal_accum[i] += n1;
                    }
                    if let Some(i) = local(tx + 1, ty) {
                        normal_accum[i] += n2;
                    }
                    if let Some(i) = local(tx + 1, ty + 1) {
                        normal_accum[i] += n2;
                    }
                    if let Some(i) = local(tx, ty + 1) {
                        normal_accum[i] += n2;
                    }
                } else {
                    let n1 = face_normal(p_tl, p_tr, p_br);
                    let n2 = face_normal(p_tl, p_br, p_bl);
                    if let Some(i) = local(tx, ty) {
                        normal_accum[i] += n1;
                    }
                    if let Some(i) = local(tx + 1, ty) {
                        normal_accum[i] += n1;
                    }
                    if let Some(i) = local(tx + 1, ty + 1) {
                        normal_accum[i] += n1;
                    }
                    if let Some(i) = local(tx, ty) {
                        normal_accum[i] += n2;
                    }
                    if let Some(i) = local(tx + 1, ty + 1) {
                        normal_accum[i] += n2;
                    }
                    if let Some(i) = local(tx, ty + 1) {
                        normal_accum[i] += n2;
                    }
                }
            }
        }

        for n in &mut normal_accum {
            *n = n.normalize_or_zero();
            if *n == Vec3::ZERO {
                *n = Vec3::Y;
            }
        }

        let tile_count = ((max_tx - min_tx + 1) * (max_ty - min_ty + 1)) as usize;
        let mut vertices = Vec::with_capacity(tile_count * 4);

        for ty in min_ty..=max_ty {
            for tx in min_tx..=max_tx {
                // Loop bounds keep us in range; the fallback emits four zero
                // vertices so the per-row buffer write still gets a
                // well-formed row.
                let Some(tile) = map.tile(tx, ty).copied() else {
                    let zv = make_vertex_ext(
                        Vec3::ZERO,
                        Vec3::Y,
                        [0.0; 2],
                        0.0,
                        0.0,
                        [0; 4],
                        [0.0; 4],
                        -1,
                        [0.0, 0.0, 0.0, 1.0],
                    );
                    vertices.extend_from_slice(&[zv, zv, zv, zv]);
                    continue;
                };

                let (p_tl, p_tr, p_bl, p_br) = tile_corners_lowered(map, tx, ty, water_depth, vw);

                let vx = tx as usize;
                let vy = ty as usize;
                let n_tl = normal_accum[local(vx, vy).expect("vx,vy in rect")];
                let n_tr = normal_accum[local(vx + 1, vy).expect("vx+1,vy in rect")];
                let n_bl = normal_accum[local(vx, vy + 1).expect("vx,vy+1 in rect")];
                let n_br = normal_accum[local(vx + 1, vy + 1).expect("vx+1,vy+1 in rect")];

                let tile_index = tile.texture_id() as f32;
                let uvs = compute_tile_uvs(tile);

                let (ground_indices, tile_no) =
                    if let (Some(gd), Some(lg)) = (ground_data, local_ground.as_ref()) {
                        let g_tl = lg[local(vx, vy).expect("vx,vy in rect")] as u32;
                        let g_tr = lg[local(vx + 1, vy).expect("vx+1,vy in rect")] as u32;
                        let g_bl = lg[local(vx, vy + 1).expect("vx,vy+1 in rect")] as u32;
                        let g_br = lg[local(vx + 1, vy + 1).expect("vx+1,vy+1 in rect")] as u32;
                        let gi = [g_tl, g_bl, g_tr, g_br];
                        let tno = if gd.is_decal(tile.texture_id() as u32) {
                            tile.texture_id() as i32
                        } else {
                            -1i32
                        };
                        (gi, tno)
                    } else {
                        ([0u32; 4], -1i32)
                    };

                let decal_tangent = if tile_no >= 0 {
                    compute_decal_tangent(p_tl, p_tr, p_bl, uvs[0], uvs[1], uvs[2])
                } else {
                    [0.0, 0.0, 0.0, 1.0]
                };

                let w_tl = [1.0, 0.0, 0.0, 0.0f32];
                let w_tr = [0.0, 0.0, 1.0, 0.0f32];
                let w_bl = [0.0, 1.0, 0.0, 0.0f32];
                let w_br = [0.0, 0.0, 0.0, 1.0f32];

                vertices.push(make_vertex_ext(
                    p_tl,
                    n_tl,
                    uvs[0],
                    p_tl.y,
                    tile_index,
                    ground_indices,
                    w_tl,
                    tile_no,
                    decal_tangent,
                ));
                vertices.push(make_vertex_ext(
                    p_tr,
                    n_tr,
                    uvs[1],
                    p_tr.y,
                    tile_index,
                    ground_indices,
                    w_tr,
                    tile_no,
                    decal_tangent,
                ));
                vertices.push(make_vertex_ext(
                    p_bl,
                    n_bl,
                    uvs[2],
                    p_bl.y,
                    tile_index,
                    ground_indices,
                    w_bl,
                    tile_no,
                    decal_tangent,
                ));
                vertices.push(make_vertex_ext(
                    p_br,
                    n_br,
                    uvs[3],
                    p_br.y,
                    tile_index,
                    ground_indices,
                    w_br,
                    tile_no,
                    decal_tangent,
                ));
            }
        }

        vertices
    }
}

/// Compute tile UVs accounting for rotation and flip flags.
///
/// Mirrors `FlaME`'s `GetTileRotatedTexCoords`: converts WZ2100 binary
/// flags to (`SwitchedAxes`, `ResultXFlip`, `ResultYFlip`), applies
/// `Reverse()`, then assigns UVs per corner. Skipping `Reverse` produces
/// wrong UVs when `SwitchedAxes` is true and `XFlip != YFlip`.
fn compute_tile_uvs(tile: wz_maplib::map_data::MapTile) -> [[f32; 2]; 4] {
    let rot = tile.rotation();
    let old_flip_x = tile.x_flip();
    // WZ2100's "flipZ" lives in our y_flip bit.
    let old_flip_z = tile.y_flip();

    let (switched_axes, mut result_x_flip, mut result_y_flip) = match rot {
        1 => (true, true, false),
        2 => (false, true, true),
        3 => (true, false, true),
        // 0 and any out-of-range rotation
        _ => (false, false, false),
    };

    if old_flip_x {
        if switched_axes {
            result_y_flip = !result_y_flip;
        } else {
            result_x_flip = !result_x_flip;
        }
    }

    if old_flip_z {
        if switched_axes {
            result_x_flip = !result_x_flip;
        } else {
            result_y_flip = !result_y_flip;
        }
    }

    // FlaME's Reverse(): toggle both when SwitchedAxes and XFlip != YFlip.
    if switched_axes && (result_x_flip ^ result_y_flip) {
        result_x_flip = !result_x_flip;
        result_y_flip = !result_y_flip;
    }

    let (ax, bx, cx, dx);
    let (ay, by, cy, dy);

    if switched_axes {
        if result_x_flip {
            ax = 1.0f32;
            bx = 1.0;
            cx = 0.0;
            dx = 0.0;
        } else {
            ax = 0.0f32;
            bx = 0.0;
            cx = 1.0;
            dx = 1.0;
        }
        if result_y_flip {
            ay = 1.0f32;
            by = 0.0;
            cy = 1.0;
            dy = 0.0;
        } else {
            ay = 0.0f32;
            by = 1.0;
            cy = 0.0;
            dy = 1.0;
        }
    } else {
        if result_x_flip {
            ax = 1.0f32;
            bx = 0.0;
            cx = 1.0;
            dx = 0.0;
        } else {
            ax = 0.0f32;
            bx = 1.0;
            cx = 0.0;
            dx = 1.0;
        }
        if result_y_flip {
            ay = 1.0f32;
            by = 1.0;
            cy = 0.0;
            dy = 0.0;
        } else {
            ay = 0.0f32;
            by = 0.0;
            cy = 1.0;
            dy = 1.0;
        }
    }

    // [TL, TR, BL, BR]
    [[ax, ay], [bx, by], [cx, cy], [dx, dy]]
}

/// Vertex height with clamping at the map edges.
fn get_height(map: &MapData, x: u32, y: u32) -> f32 {
    let cx = x.min(map.width.saturating_sub(1));
    let cy = y.min(map.height.saturating_sub(1));
    map.tile(cx, cy).map_or(0.0, |t| t.height as f32)
}

/// Face normal from three triangle vertices.
///
/// Uses `edge2.cross(edge1)` so XZ-plane terrain produces upward-facing
/// normals. Reversing the operands inverts lighting.
fn face_normal(v0: Vec3, v1: Vec3, v2: Vec3) -> Vec3 {
    let edge1 = v1 - v0;
    let edge2 = v2 - v0;
    edge2.cross(edge1).normalize_or_zero()
}

fn make_vertex_ext(
    pos: Vec3,
    normal: Vec3,
    uv: [f32; 2],
    height: f32,
    tile_index: f32,
    ground_indices: [u32; 4],
    ground_weights: [f32; 4],
    tile_no: i32,
    decal_tangent: [f32; 4],
) -> TerrainVertex {
    TerrainVertex {
        position: pos.to_array(),
        normal: normal.to_array(),
        tex_coord: uv,
        height_color: (height / MAX_HEIGHT).clamp(0.0, 1.0),
        tile_index,
        ground_indices,
        ground_weights,
        tile_no,
        decal_tangent,
    }
}

/// Tangent + handedness for a decal tile quad, derived from triangle
/// edges and UV deltas. Matches WZ2100's `vertexTangent` attribute used
/// by `terrain_combined.vert`.
fn compute_decal_tangent(
    p0: Vec3,
    p1: Vec3,
    p2: Vec3,
    uv0: [f32; 2],
    uv1: [f32; 2],
    uv2: [f32; 2],
) -> [f32; 4] {
    let e1 = p1 - p0;
    let e2 = p2 - p0;
    let duv1 = glam::Vec2::new(uv1[0] - uv0[0], uv1[1] - uv0[1]);
    let duv2 = glam::Vec2::new(uv2[0] - uv0[0], uv2[1] - uv0[1]);
    let denom = duv1.x * duv2.y - duv2.x * duv1.y;
    if denom.abs() < 1e-8 {
        // Degenerate UV: fall back to the X axis.
        return [1.0, 0.0, 0.0, 1.0];
    }
    let r = 1.0 / denom;
    let tangent = (e1 * duv2.y - e2 * duv1.y) * r;
    let bitangent = (e2 * duv1.x - e1 * duv2.x) * r;

    let t = tangent.normalize_or_zero();
    let normal = e2.cross(e1).normalize_or_zero();
    let w = if normal.cross(t).dot(bitangent) < 0.0 {
        -1.0
    } else {
        1.0
    };
    [t.x, t.y, t.z, w]
}

#[cfg(test)]
mod tests {
    use super::*;
    use wz_maplib::map_data::MapTile;

    #[test]
    fn face_normal_flat_xz_plane_points_up() {
        let v0 = Vec3::new(0.0, 0.0, 0.0);
        let v1 = Vec3::new(1.0, 0.0, 0.0);
        let v2 = Vec3::new(0.0, 0.0, 1.0);
        let n = face_normal(v0, v1, v2);
        assert!(n.y > 0.9, "expected upward normal, got {n:?}");
        assert!(n.x.abs() < 0.01);
        assert!(n.z.abs() < 0.01);
    }

    #[test]
    fn face_normal_degenerate_triangle_returns_zero() {
        let v0 = Vec3::new(0.0, 0.0, 0.0);
        let v1 = Vec3::new(1.0, 0.0, 0.0);
        let v2 = Vec3::new(2.0, 0.0, 0.0);
        let n = face_normal(v0, v1, v2);
        assert_eq!(n, Vec3::ZERO);
    }

    #[test]
    fn face_normal_sloped_terrain() {
        let v0 = Vec3::new(0.0, 0.0, 0.0);
        let v1 = Vec3::new(1.0, 1.0, 0.0);
        let v2 = Vec3::new(0.0, 0.0, 1.0);
        let n = face_normal(v0, v1, v2);
        assert!(n.y > 0.0, "expected positive Y in normal, got {n:?}");
        assert!(
            (n.length() - 1.0).abs() < 1e-5,
            "normal should be unit length"
        );
    }

    #[test]
    fn compute_tile_uvs_no_transform() {
        let tile = MapTile {
            height: 0,
            texture: MapTile::make_texture(0, false, false, 0, false),
        };
        let uvs = compute_tile_uvs(tile);
        assert_eq!(uvs[0], [0.0, 0.0]); // TL
        assert_eq!(uvs[1], [1.0, 0.0]); // TR
        assert_eq!(uvs[2], [0.0, 1.0]); // BL
        assert_eq!(uvs[3], [1.0, 1.0]); // BR
    }

    #[test]
    fn compute_tile_uvs_rotation_1() {
        let tile = MapTile {
            height: 0,
            texture: MapTile::make_texture(0, false, false, 1, false),
        };
        let uvs = compute_tile_uvs(tile);
        assert_eq!(uvs[0], [0.0, 1.0]); // TL
        assert_eq!(uvs[1], [0.0, 0.0]); // TR
        assert_eq!(uvs[2], [1.0, 1.0]); // BL
        assert_eq!(uvs[3], [1.0, 0.0]); // BR
    }

    #[test]
    fn compute_tile_uvs_rotation_2() {
        let tile = MapTile {
            height: 0,
            texture: MapTile::make_texture(0, false, false, 2, false),
        };
        let uvs = compute_tile_uvs(tile);
        assert_eq!(uvs[0], [1.0, 1.0]); // TL
        assert_eq!(uvs[1], [0.0, 1.0]); // TR
        assert_eq!(uvs[2], [1.0, 0.0]); // BL
        assert_eq!(uvs[3], [0.0, 0.0]); // BR
    }

    #[test]
    fn compute_tile_uvs_rotation_3() {
        let tile = MapTile {
            height: 0,
            texture: MapTile::make_texture(0, false, false, 3, false),
        };
        let uvs = compute_tile_uvs(tile);
        assert_eq!(uvs[0], [1.0, 0.0]); // TL
        assert_eq!(uvs[1], [1.0, 1.0]); // TR
        assert_eq!(uvs[2], [0.0, 0.0]); // BL
        assert_eq!(uvs[3], [0.0, 1.0]); // BR
    }

    #[test]
    fn compute_tile_uvs_x_flip() {
        let tile = MapTile {
            height: 0,
            texture: MapTile::make_texture(0, true, false, 0, false),
        };
        let uvs = compute_tile_uvs(tile);
        assert_eq!(uvs[0], [1.0, 0.0]); // TL
        assert_eq!(uvs[1], [0.0, 0.0]); // TR
        assert_eq!(uvs[2], [1.0, 1.0]); // BL
        assert_eq!(uvs[3], [0.0, 1.0]); // BR
    }

    #[test]
    fn compute_tile_uvs_y_flip() {
        let tile = MapTile {
            height: 0,
            texture: MapTile::make_texture(0, false, true, 0, false),
        };
        let uvs = compute_tile_uvs(tile);
        assert_eq!(uvs[0], [0.0, 1.0]); // TL
        assert_eq!(uvs[1], [1.0, 1.0]); // TR
        assert_eq!(uvs[2], [0.0, 0.0]); // BL
        assert_eq!(uvs[3], [1.0, 0.0]); // BR
    }

    #[test]
    fn compute_tile_uvs_all_rotations_are_distinct() {
        let mut all_uvs = Vec::new();
        for rot in 0..4u8 {
            let tile = MapTile {
                height: 0,
                texture: MapTile::make_texture(0, false, false, rot, false),
            };
            all_uvs.push(compute_tile_uvs(tile));
        }
        for i in 0..4 {
            for j in (i + 1)..4 {
                assert_ne!(
                    all_uvs[i], all_uvs[j],
                    "rotations {i} and {j} produced same UVs"
                );
            }
        }
    }

    #[test]
    fn make_vertex_clamps_height_color() {
        let v = make_vertex_ext(
            Vec3::ZERO,
            Vec3::Y,
            [0.0, 0.0],
            -10.0,
            0.0,
            [0; 4],
            [0.0; 4],
            -1,
            [0.0; 4],
        );
        assert_eq!(v.height_color, 0.0);

        let v2 = make_vertex_ext(
            Vec3::ZERO,
            Vec3::Y,
            [0.0, 0.0],
            MAX_HEIGHT * 2.0,
            0.0,
            [0; 4],
            [0.0; 4],
            -1,
            [0.0; 4],
        );
        assert_eq!(v2.height_color, 1.0);
    }

    #[test]
    fn terrain_mesh_from_flat_map() {
        let map = MapData::new(2, 2);
        let mesh = TerrainMesh::from_map(&map, None, None);
        assert_eq!(mesh.vertices.len(), 4 * 4); // 4 tiles x 4 verts
        assert_eq!(mesh.indices.len(), 4 * 6); // 4 tiles x 6 indices
    }

    #[test]
    fn terrain_mesh_normals_point_up_on_flat() {
        let map = MapData::new(2, 2);
        let mesh = TerrainMesh::from_map(&map, None, None);
        for v in &mesh.vertices {
            assert!(
                v.normal[1] > 0.9,
                "expected upward normal, got {:?}",
                v.normal
            );
        }
    }

    #[test]
    fn terrain_mesh_indices_reference_only_local_quad_vertices() {
        // Every 6-index group describes one tile and may only reference
        // its own 4 vertices (base..base+3). A regression that mixed
        // vertices across tiles would corrupt rendering on the GPU.
        let map = varied_map(5, 5);
        let mesh = TerrainMesh::from_map(&map, None, None);
        assert_eq!(mesh.indices.len() % 6, 0);
        for (tile_idx, chunk) in mesh.indices.chunks_exact(6).enumerate() {
            let base = (tile_idx * 4) as u32;
            for &i in chunk {
                assert!(
                    (base..base + 4).contains(&i),
                    "tile {tile_idx} index {i} outside [{base}, {})",
                    base + 4,
                );
            }
        }
        // Sanity: one quad per tile, 4 verts each.
        assert_eq!(mesh.vertices.len(), (map.width * map.height * 4) as usize);
    }

    #[test]
    fn compute_decal_tangent_degenerate_uvs_returns_fallback() {
        let p0 = Vec3::new(0.0, 0.0, 0.0);
        let p1 = Vec3::new(1.0, 0.0, 0.0);
        let p2 = Vec3::new(0.0, 0.0, 1.0);
        let uv = [0.5, 0.5];
        let t = compute_decal_tangent(p0, p1, p2, uv, uv, uv);
        assert_eq!(t, [1.0, 0.0, 0.0, 1.0], "expected X-axis fallback tangent");
    }

    #[test]
    fn compute_decal_tangent_parallel_uv_edges_returns_fallback() {
        let p0 = Vec3::new(0.0, 0.0, 0.0);
        let p1 = Vec3::new(1.0, 0.0, 0.0);
        let p2 = Vec3::new(2.0, 0.0, 0.0);
        let uv0 = [0.0, 0.0];
        let uv1 = [1.0, 0.0];
        let uv2 = [2.0, 0.0];
        let t = compute_decal_tangent(p0, p1, p2, uv0, uv1, uv2);
        assert_eq!(t, [1.0, 0.0, 0.0, 1.0], "expected X-axis fallback tangent");
    }

    /// Build a small map with varied heights, textures, and tri-flip flags
    /// so the partial path actually exercises the normal-accumulation and
    /// UV branches it shares with `from_map`.
    fn varied_map(w: u32, h: u32) -> MapData {
        let mut map = MapData::new(w, h);
        for ty in 0..h {
            for tx in 0..w {
                let tile = map.tile_mut(tx, ty).unwrap();
                tile.height = ((tx * 7 + ty * 11) % 200) as u16;
                let tri_flip = ((tx + ty) % 2) == 0;
                let rot = ((tx + ty * 3) % 4) as u8;
                tile.texture =
                    MapTile::make_texture(((tx + ty) % 16) as u16, false, false, rot, tri_flip);
            }
        }
        map
    }

    /// Compare every field of two terrain vertices, treating bit-identical
    /// position/normal as the bar (no fp tolerance: the partial path must
    /// produce the same arithmetic as `from_map` for the dirty rect).
    fn assert_vertex_eq(a: &TerrainVertex, b: &TerrainVertex, ctx: &str) {
        assert_eq!(a.position, b.position, "{ctx}: position");
        assert_eq!(a.normal, b.normal, "{ctx}: normal");
        assert_eq!(a.tex_coord, b.tex_coord, "{ctx}: tex_coord");
        assert_eq!(a.height_color, b.height_color, "{ctx}: height_color");
        assert_eq!(a.tile_index, b.tile_index, "{ctx}: tile_index");
        assert_eq!(a.tile_no, b.tile_no, "{ctx}: tile_no");
    }

    #[test]
    fn partial_rect_matches_full_rebuild_interior() {
        let map = varied_map(8, 8);
        let full = TerrainMesh::from_map(&map, None, None);
        let vw = (map.width + 1) as usize;
        let water_depth = vec![0.0; vw * (map.height + 1) as usize];
        let (min_tx, min_ty, max_tx, max_ty) = (3u32, 3u32, 5u32, 4u32);
        let partial = TerrainMesh::build_tile_rect_vertices(
            &map,
            None,
            &water_depth,
            min_tx,
            min_ty,
            max_tx,
            max_ty,
        );
        let w = map.width as usize;
        for ty in min_ty..=max_ty {
            for tx in min_tx..=max_tx {
                let full_base = (ty as usize * w + tx as usize) * 4;
                let row = (ty - min_ty) as usize;
                let col = (tx - min_tx) as usize;
                let row_tiles = (max_tx - min_tx + 1) as usize;
                let part_base = (row * row_tiles + col) * 4;
                for k in 0..4 {
                    assert_vertex_eq(
                        &partial[part_base + k],
                        &full.vertices[full_base + k],
                        &format!("tx={tx} ty={ty} k={k}"),
                    );
                }
            }
        }
    }

    #[test]
    fn partial_rect_matches_full_rebuild_corner() {
        // Top-left corner. Normal-accum pass must clamp the neighbour
        // expansion at 0,0 instead of underflowing.
        let map = varied_map(6, 6);
        let full = TerrainMesh::from_map(&map, None, None);
        let vw = (map.width + 1) as usize;
        let water_depth = vec![0.0; vw * (map.height + 1) as usize];
        let partial = TerrainMesh::build_tile_rect_vertices(&map, None, &water_depth, 0, 0, 1, 1);
        let w = map.width as usize;
        for ty in 0..=1usize {
            for tx in 0..=1usize {
                let full_base = (ty * w + tx) * 4;
                let part_base = (ty * 2 + tx) * 4;
                for k in 0..4 {
                    assert_vertex_eq(
                        &partial[part_base + k],
                        &full.vertices[full_base + k],
                        &format!("corner tx={tx} ty={ty} k={k}"),
                    );
                }
            }
        }
    }

    #[test]
    fn partial_rect_single_tile_at_far_corner() {
        // Bottom-right single tile. max_tx+1 in normal-accum must clamp
        // to map width, not run off the end.
        let map = varied_map(4, 4);
        let full = TerrainMesh::from_map(&map, None, None);
        let vw = (map.width + 1) as usize;
        let water_depth = vec![0.0; vw * (map.height + 1) as usize];
        let partial = TerrainMesh::build_tile_rect_vertices(&map, None, &water_depth, 3, 3, 3, 3);
        assert_eq!(partial.len(), 4);
        let full_base = (3 * map.width as usize + 3) * 4;
        for (k, partial_v) in partial.iter().enumerate().take(4) {
            assert_vertex_eq(
                partial_v,
                &full.vertices[full_base + k],
                &format!("far corner k={k}"),
            );
        }
    }
}
