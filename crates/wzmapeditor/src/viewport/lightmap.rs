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

/// Matches WZ2100's `calcTileIllum` scan radius.
const AO_SCAN_RADIUS: i32 = 8;

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
/// `terrain_types` applies the same dug heights here.
pub fn compute_lightmap(map: &MapData, terrain_types: Option<&TerrainTypeData>) -> Lightmap {
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
            let ao = tile_ambient_occlusion(&heights, w, h, tx, ty);
            data[(ty * w + tx) as usize] =
                (ao * MAX_BRIGHTNESS).clamp(MIN_BRIGHTNESS, MAX_BRIGHTNESS) as u8;
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
/// so flat horizon = 1.0 and a 90 degree wall = 0.0.
fn tile_ambient_occlusion(heights: &[f32], w: u32, h: u32, tx: u32, ty: u32) -> f32 {
    const DIRS: [(i32, i32); 8] = [
        (1, 0),
        (1, 1),
        (0, 1),
        (-1, 1),
        (-1, 0),
        (-1, -1),
        (0, -1),
        (1, -1),
    ];

    let w_i = w as i32;
    let h_i = h as i32;
    let base_h = heights[(ty * w + tx) as usize];
    let mut ao_sum = 0.0f32;

    for &(ddx, ddy) in &DIRS {
        let mut max_tangent = 0.0f32;

        for step in 1..=AO_SCAN_RADIUS {
            let sx = tx as i32 + ddx * step;
            let sy = ty as i32 + ddy * step;

            if sx < 0 || sx >= w_i || sy < 0 || sy >= h_i {
                break;
            }

            let sample_h = heights[(sy as u32 * w + sx as u32) as usize];
            let dh = sample_h - base_h;
            let dist = step as f32 * TILE_UNITS_F32;
            max_tangent = max_tangent.max(dh / dist);
        }

        // 1 - sin(elevation_angle).
        ao_sum += 1.0 - max_tangent / (max_tangent * max_tangent + 1.0).sqrt();
    }

    (ao_sum / DIRS.len() as f32).clamp(0.25, 1.0)
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
        let lm = compute_lightmap(&map, None);
        assert_eq!(lm.width, 16);
        assert_eq!(lm.height, 16);
        assert_eq!(lm.data.len(), 16 * 16);
    }

    #[test]
    fn lightmap_brightness_within_range() {
        let map = MapData::new(8, 8);
        let lm = compute_lightmap(&map, None);
        for ty in 0..8u32 {
            for tx in 0..8u32 {
                let b = lm.data[(ty * 8 + tx) as usize];
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
        let map = MapData::new(8, 8);
        let lm = compute_lightmap(&map, None);
        assert_eq!(lm.data[4 * 8 + 4], MAX_BRIGHTNESS as u8);
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

        let lm = compute_lightmap(&map, Some(&ttp));
        let edge = lm.data[0];
        let shore_adjacent = lm.data[8 + 1];
        assert!(
            shore_adjacent < edge || lm.data.iter().any(|&b| b < MAX_BRIGHTNESS as u8),
            "dug riverbed should occlude somewhere: edge {edge}, inner {shore_adjacent}"
        );
    }
}
