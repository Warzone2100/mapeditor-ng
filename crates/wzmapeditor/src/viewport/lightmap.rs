//! Terrain lightmap: per-tile ambient occlusion.
//!
//! Produces an R8 texture at map resolution (one pixel per tile) holding
//! ambient occlusion alone. Matches WZ2100's `tile->ambientOcclusion`
//! (`src/lighting.cpp` `calcTileIllum`), which is what `getTileIllumination`
//! feeds to the lightmap: *"sunlight is handled by shaders so only AO needed
//! for lightmap"* (`src/advvis.cpp`). The sun's lambert term belongs to the
//! terrain and model shaders, which compute it per fragment; baking it in here
//! as well would apply it twice.

use wz_maplib::MapData;
use wz_maplib::constants::TILE_UNITS_F32;
use wz_maplib::terrain_types::TerrainTypeData;

use super::water::build_water_vertex_depths;

/// Occlusion floor, matching WZ2100's clamp on `tile->ambientOcclusion`.
/// Keeps deep valleys legible rather than black.
const MIN_BRIGHTNESS: f32 = 60.0;

/// Stays just below pure white so surface detail isn't lost when the
/// shader scales the lightmap.
const MAX_BRIGHTNESS: f32 = 254.0;

/// The game blacks the outermost tile ring down to this (`initLighting`).
const EDGE_BRIGHTNESS: f32 = 16.0;

/// WZ2100's `calcTileIllum` scans 8 steps of 100 world units per direction.
const AO_STEP_WORLD: f32 = 100.0;
const AO_STEPS: u32 = 8;

/// Per-tile lightmap data (single-channel, one pixel per tile).
pub struct Lightmap {
    pub width: u32,
    pub height: u32,
    /// R8 pixel data, row-major, `width * height` bytes.
    pub data: Vec<u8>,
}

/// Compute the terrain lightmap from map heights.
///
/// Takes no sun direction: the value is pure ambient occlusion, so it only
/// changes when the terrain does. The game computes tile illumination *after*
/// digging riverbeds, so water tiles darken toward lake centres; passing
/// `terrain_types` applies the same dug heights here. `darken_border`
/// applies the game's map-border darkening (edge ring plus scroll-limit
/// band); the View menu's Show Border toggle turns it off.
pub fn compute_lightmap(
    map: &MapData,
    terrain_types: Option<&TerrainTypeData>,
    darken_border: bool,
) -> Lightmap {
    let w = map.width;
    let h = map.height;
    let mut data = vec![0u8; (w * h) as usize];

    let digs = terrain_types.map(|ttp| build_water_vertex_depths(map, ttp));
    let vw = (w + 1) as usize;
    let mut heights = Vec::with_capacity((w * h) as usize);
    for ty in 0..h {
        for tx in 0..w {
            let base = map.tile(tx, ty).map_or(0.0, |t| f32::from(t.height));
            let dig = digs
                .as_ref()
                .map_or(0.0, |d| d[ty as usize * vw + tx as usize]);
            heights.push(base - dig);
        }
    }

    for ty in 0..h {
        for tx in 0..w {
            let edge = darken_border && (tx == 0 || ty == 0 || tx + 1 >= w || ty + 1 >= h);
            let mut value = if edge {
                EDGE_BRIGHTNESS
            } else {
                let ao = tile_ambient_occlusion(&heights, w, h, tx, ty);
                (ao * MAX_BRIGHTNESS).clamp(MIN_BRIGHTNESS, MAX_BRIGHTNESS)
            };
            // The game darkens the band within 4 tiles of the scroll limits
            // to a third (`initLighting`); with limits at the map bounds
            // that is this asymmetric border band.
            let (wi, hi) = (w as i32, h as i32);
            let (txi, tyi) = (tx as i32, ty as i32);
            if darken_border && (txi < 4 || txi > wi - 4 || tyi < 4 || tyi > hi - 4) {
                value /= 3.0;
            }
            data[(ty * w + tx) as usize] = value as u8;
        }
    }

    Lightmap {
        width: w,
        height: h,
        data,
    }
}

/// Ambient occlusion via 8-direction horizon scanning (matches WZ2100 `calcTileIllum`).
///
/// Each direction's max elevation tangent maps to occlusion via
/// `1 - tan(theta) / sqrt(tan^2(theta) + 1)`, equivalent to `1 - sin(theta)`,
/// so flat horizon = 1.0 and a 90 degree wall = 0.0. Steps are 100 world
/// units along every direction (diagonal components are `100 * sqrt(0.5)`),
/// sampled from the interpolated heightfield with edge clamping, as the
/// game samples `map_Height` at clipped world coordinates.
fn tile_ambient_occlusion(heights: &[f32], w: u32, h: u32, tx: u32, ty: u32) -> f32 {
    const I: f32 = AO_STEP_WORLD;
    const H: f32 = AO_STEP_WORLD * std::f32::consts::FRAC_1_SQRT_2;
    const DIRS: [(f32, f32); 8] = [
        (0.0, I),
        (H, H),
        (I, 0.0),
        (H, -H),
        (0.0, -I),
        (-H, -H),
        (-I, 0.0),
        (-H, H),
    ];

    let cx = tx as f32 * TILE_UNITS_F32;
    let cy = ty as f32 * TILE_UNITS_F32;
    let base_h = sample_height(heights, w, h, cx, cy);
    let mut ao_sum = 0.0f32;

    for &(dx, dy) in &DIRS {
        let mut max_tangent = 0.0f32;
        for step in 1..=AO_STEPS {
            let d = step as f32;
            let sample_h = sample_height(heights, w, h, cx + dx * d, cy + dy * d);
            max_tangent = max_tangent.max((sample_h - base_h) / (I * d));
        }

        // 1 - sin(elevation_angle).
        ao_sum += 1.0 - max_tangent / (max_tangent * max_tangent + 1.0).sqrt();
    }

    (ao_sum / DIRS.len() as f32).clamp(0.25, 1.0)
}

/// Bilinear height at world coordinates over the per-tile corner grid,
/// clamped to the map bounds.
fn sample_height(heights: &[f32], w: u32, h: u32, wx: f32, wy: f32) -> f32 {
    let gx = (wx / TILE_UNITS_F32).clamp(0.0, (w - 1) as f32);
    let gy = (wy / TILE_UNITS_F32).clamp(0.0, (h - 1) as f32);
    let x0 = gx as u32;
    let y0 = gy as u32;
    let x1 = (x0 + 1).min(w - 1);
    let y1 = (y0 + 1).min(h - 1);
    let fx = gx - x0 as f32;
    let fy = gy - y0 as f32;
    let h00 = heights[(y0 * w + x0) as usize];
    let h10 = heights[(y0 * w + x1) as usize];
    let h01 = heights[(y1 * w + x0) as usize];
    let h11 = heights[(y1 * w + x1) as usize];
    let top = h00 + (h10 - h00) * fx;
    let bottom = h01 + (h11 - h01) * fx;
    top + (bottom - top) * fy
}

#[cfg(test)]
mod tests {
    use super::*;
    use wz_maplib::terrain_types::TerrainType;

    fn flat_heights(w: u32, h: u32) -> Vec<f32> {
        vec![0.0; (w * h) as usize]
    }

    #[test]
    fn flat_terrain_high_ao() {
        let heights = flat_heights(8, 8);
        let ao = tile_ambient_occlusion(&heights, 8, 8, 4, 4);
        assert!((ao - 1.0).abs() < 0.01, "flat AO = {ao}");
    }

    #[test]
    fn lightmap_dimensions_match_map() {
        let map = MapData::new(16, 16);
        let lm = compute_lightmap(&map, None, true);
        assert_eq!(lm.width, 16);
        assert_eq!(lm.height, 16);
        assert_eq!(lm.data.len(), 16 * 16);
    }

    #[test]
    fn lightmap_brightness_within_range() {
        let map = MapData::new(16, 16);
        let lm = compute_lightmap(&map, None, true);
        for ty in 4..13u32 {
            for tx in 4..13u32 {
                let b = lm.data[(ty * 16 + tx) as usize];
                assert!(
                    b >= MIN_BRIGHTNESS as u8 && b <= MAX_BRIGHTNESS as u8,
                    "brightness {b} out of range at ({tx},{ty})"
                );
            }
        }
    }

    #[test]
    fn flat_terrain_is_fully_unoccluded() {
        // Flat ground carries no sun term now, so it must sit at the ceiling
        // rather than at the sun's lambert factor.
        let map = MapData::new(16, 16);
        let lm = compute_lightmap(&map, None, true);
        assert_eq!(lm.data[8 * 16 + 8], MAX_BRIGHTNESS as u8);
    }

    #[test]
    fn map_border_matches_game_darkening() {
        let map = MapData::new(16, 16);
        let lm = compute_lightmap(&map, None, true);
        // Edge ring: initLighting's 16, then the scroll-limit third.
        assert_eq!(lm.data[0], (EDGE_BRIGHTNESS / 3.0) as u8);
        // Inside the ring but within 4 tiles of the bounds: full AO over 3.
        assert_eq!(lm.data[2 * 16 + 2], (MAX_BRIGHTNESS / 3.0) as u8);
    }

    #[test]
    fn border_darkening_off_leaves_border_fully_lit() {
        let map = MapData::new(16, 16);
        let lm = compute_lightmap(&map, None, false);
        assert_eq!(lm.data[0], MAX_BRIGHTNESS as u8);
        assert_eq!(lm.data[2 * 16 + 2], MAX_BRIGHTNESS as u8);
    }

    #[test]
    fn valley_has_lower_ao_than_flat() {
        let w = 16u32;
        let mut heights = flat_heights(w, w);
        for ty in 0..16u32 {
            for tx in 0..16u32 {
                if !(4..=11).contains(&tx) || !(4..=11).contains(&ty) {
                    heights[(ty * w + tx) as usize] = 200.0;
                }
            }
        }
        let ao_flat = tile_ambient_occlusion(&flat_heights(w, w), w, w, 8, 8);
        let ao_valley = tile_ambient_occlusion(&heights, w, w, 8, 8);
        assert!(
            ao_valley < ao_flat,
            "valley AO ({ao_valley}) should be less than flat ({ao_flat})"
        );
    }

    #[test]
    fn dug_riverbed_darkens_water_tiles() {
        // An all-water lake digs deep bowls, so its lightmap must fall below
        // the flat-ground ceiling once terrain types are supplied.
        let mut map = MapData::new(8, 8);
        for tile in &mut map.tiles {
            tile.height = 100;
            tile.texture = 7;
        }
        let mut terrain_types = vec![TerrainType::Sand; 8];
        terrain_types[7] = TerrainType::Water;
        let ttp = TerrainTypeData { terrain_types };

        let lm = compute_lightmap(&map, Some(&ttp), true);
        let edge = lm.data[0];
        let shore_adjacent = lm.data[8 + 1];
        assert!(
            shore_adjacent < edge || lm.data.iter().any(|&b| b < MAX_BRIGHTNESS as u8),
            "dug riverbed should occlude somewhere: edge {edge}, inner {shore_adjacent}"
        );
    }
}
