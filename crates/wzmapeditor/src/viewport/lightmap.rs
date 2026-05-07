//! Terrain lightmap: per-tile sun illumination and ambient occlusion.
//!
//! Produces an R8 texture at map resolution (one pixel per tile), storing
//! `sun_diffuse * ambient_occlusion`. Matches WZ2100's `src/lighting.cpp`.

use glam::Vec3;
use wz_maplib::MapData;
use wz_maplib::constants::TILE_UNITS_F32;

/// Matches WZ2100's `MIN_ILLUM` floor; prevents fully-black tiles in valleys.
const MIN_BRIGHTNESS: f32 = 24.0;

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

/// Compute terrain lightmap from map heights and sun direction.
pub fn compute_lightmap(map: &MapData, sun_dir: Vec3) -> Lightmap {
    let w = map.width;
    let h = map.height;
    let sun = sun_dir.normalize();
    let mut data = vec![0u8; (w * h) as usize];

    for ty in 0..h {
        for tx in 0..w {
            let normal = tile_normal(map, tx, ty);
            let diffuse = normal.dot(sun).max(0.0);
            let ao = tile_ambient_occlusion(map, tx, ty);
            let brightness =
                (diffuse * ao * MAX_BRIGHTNESS).clamp(MIN_BRIGHTNESS, MAX_BRIGHTNESS) as u8;

            data[(ty * w + tx) as usize] = brightness;
        }
    }

    Lightmap {
        width: w,
        height: h,
        data,
    }
}

fn tile_normal(map: &MapData, tx: u32, ty: u32) -> Vec3 {
    let w = map.width;
    let h = map.height;

    let get_h = |x: u32, y: u32| -> f32 {
        map.tile(x.min(w - 1), y.min(h - 1))
            .map_or(0.0, |t| t.height as f32)
    };

    let hc = get_h(tx, ty);
    let hx = if tx + 1 < w { get_h(tx + 1, ty) } else { hc };
    let hz = if ty + 1 < h { get_h(tx, ty + 1) } else { hc };
    let hxn = if tx > 0 { get_h(tx - 1, ty) } else { hc };
    let hzn = if ty > 0 { get_h(tx, ty - 1) } else { hc };

    let dx = (hxn - hx) / (2.0 * TILE_UNITS_F32);
    let dz = (hzn - hz) / (2.0 * TILE_UNITS_F32);
    Vec3::new(dx, 1.0, dz).normalize()
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
    fn flat_terrain_normal_points_up() {
        let map = MapData::new(8, 8);
        let n = tile_normal(&map, 4, 4);
        assert!((n.y - 1.0).abs() < 0.01, "normal = {n}");
    }

    #[test]
    fn lightmap_dimensions_match_map() {
        let map = MapData::new(16, 16);
        let sun = Vec3::new(0.286, 0.763, 0.572).normalize();
        let lm = compute_lightmap(&map, sun);
        assert_eq!(lm.width, 16);
        assert_eq!(lm.height, 16);
        assert_eq!(lm.data.len(), 16 * 16);
    }

    #[test]
    fn lightmap_brightness_within_range() {
        let map = MapData::new(8, 8);
        let sun = Vec3::new(0.286, 0.763, 0.572).normalize();
        let lm = compute_lightmap(&map, sun);
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
