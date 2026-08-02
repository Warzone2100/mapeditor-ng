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
/// changes when the terrain does.
pub fn compute_lightmap(map: &MapData) -> Lightmap {
    let w = map.width;
    let h = map.height;
    let mut data = vec![0u8; (w * h) as usize];

    for ty in 0..h {
        for tx in 0..w {
            let ao = tile_ambient_occlusion(map, tx, ty);
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
fn tile_ambient_occlusion(map: &MapData, tx: u32, ty: u32) -> f32 {
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

    let w = map.width as i32;
    let h = map.height as i32;
    let base_h = map.tile(tx, ty).map_or(0.0, |t| t.height as f32);
    let mut ao_sum = 0.0f32;

    for &(ddx, ddy) in &DIRS {
        let mut max_tangent = 0.0f32;

        for step in 1..=AO_SCAN_RADIUS {
            let sx = tx as i32 + ddx * step;
            let sy = ty as i32 + ddy * step;

            if sx < 0 || sx >= w || sy < 0 || sy >= h {
                break;
            }

            let sample_h = map
                .tile(sx as u32, sy as u32)
                .map_or(0.0, |t| t.height as f32);
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

    #[test]
    fn flat_terrain_high_ao() {
        let map = MapData::new(8, 8);
        let ao = tile_ambient_occlusion(&map, 4, 4);
        assert!((ao - 1.0).abs() < 0.01, "flat AO = {ao}");
    }

    #[test]
    fn lightmap_dimensions_match_map() {
        let map = MapData::new(16, 16);
        let lm = compute_lightmap(&map);
        assert_eq!(lm.width, 16);
        assert_eq!(lm.height, 16);
        assert_eq!(lm.data.len(), 16 * 16);
    }

    #[test]
    fn lightmap_brightness_within_range() {
        let map = MapData::new(8, 8);
        let lm = compute_lightmap(&map);
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
        let lm = compute_lightmap(&map);
        assert_eq!(lm.data[4 * 8 + 4], MAX_BRIGHTNESS as u8);
    }

    #[test]
    fn valley_has_lower_ao_than_flat() {
        let mut map = MapData::new(16, 16);
        for ty in 0..16u32 {
            for tx in 0..16u32 {
                if let Some(tile) = map.tile_mut(tx, ty)
                    && (!(4..=11).contains(&tx) || !(4..=11).contains(&ty))
                {
                    tile.height = 200;
                }
            }
        }
        let ao_flat = {
            let flat_map = MapData::new(16, 16);
            tile_ambient_occlusion(&flat_map, 8, 8)
        };
        let ao_valley = tile_ambient_occlusion(&map, 8, 8);
        assert!(
            ao_valley < ao_flat,
            "valley AO ({ao_valley}) should be less than flat ({ao_flat})"
        );
    }
}
