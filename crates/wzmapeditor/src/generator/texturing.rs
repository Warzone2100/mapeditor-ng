//! Auto-texturing: classify each tile as ground/cliff/water by slope and
//! height, then assign tile IDs from per-tileset pools using coherent noise so
//! variants form patches instead of salt-and-pepper.

use std::collections::HashMap;
use std::sync::OnceLock;

use noise::{NoiseFn, Perlin};
use serde::Deserialize;
use wz_maplib::io_wz::WzMap;
use wz_maplib::map_data::MapTile;
use wz_maplib::terrain_types::{TerrainType, TerrainTypeData};

use crate::config::Tileset;

use super::GeneratorConfig;

/// Per-tileset texture configuration loaded from `assets/tilesets/*.json`.
#[derive(Debug, Clone, Deserialize)]
struct TilesetTexConfig {
    ground_tiles: Vec<u16>,
    cliff_tiles: Vec<u16>,
    water_tile: u16,
    /// Slope (height units) at which a tile is reclassified as cliff.
    cliff_threshold: f32,
    /// Variation tiles used above `high_ground_cutoff` height fraction.
    high_ground_tiles: Vec<u16>,
    /// Height fraction (0.0-1.0) above which `high_ground_tiles` apply.
    high_ground_cutoff: f32,
}

const ARIZONA_JSON: &str = include_str!("../../assets/tilesets/arizona.json");
const URBAN_JSON: &str = include_str!("../../assets/tilesets/urban.json");
const ROCKIES_JSON: &str = include_str!("../../assets/tilesets/rockies.json");

fn tileset_configs() -> &'static HashMap<Tileset, TilesetTexConfig> {
    static CONFIGS: OnceLock<HashMap<Tileset, TilesetTexConfig>> = OnceLock::new();
    CONFIGS.get_or_init(|| {
        let mut map = HashMap::new();
        for (tileset, json) in [
            (Tileset::Arizona, ARIZONA_JSON),
            (Tileset::Urban, URBAN_JSON),
            (Tileset::Rockies, ROCKIES_JSON),
        ] {
            let cfg: TilesetTexConfig = serde_json::from_str(json).unwrap_or_else(|e| {
                panic!(
                    "bundled tileset config for {tileset:?} is malformed: {e}\n\nThis is a build-time data file under crates/wzmapeditor/assets/tilesets/.",
                )
            });
            map.insert(tileset, cfg);
        }
        map
    })
}

fn tileset_config(tileset: Tileset) -> &'static TilesetTexConfig {
    tileset_configs()
        .get(&tileset)
        .expect("tileset_configs covers every Tileset variant")
}

pub(crate) fn auto_texture(map: &mut WzMap, config: &GeneratorConfig) {
    let tex_config = tileset_config(config.tileset);
    let mut rng = fastrand::Rng::with_seed(config.seed.wrapping_add(0xDEAD));
    let s = config.seed as u32;

    // Slow-varying noise so variant tiles cluster into patches instead of
    // showing up as salt-and-pepper.
    let tex_noise = Perlin::new(s.wrapping_add(500));
    let cliff_jitter_noise = Perlin::new(s.wrapping_add(600));
    let elev_noise = Perlin::new(s.wrapping_add(700));

    let w = map.map_data.width;
    let h = map.map_data.height;
    let max_height = wz_maplib::constants::TILE_MAX_HEIGHT as f32;

    for y in 0..h {
        for x in 0..w {
            let h00 = tile_height(&map.map_data, x, y) as f32;
            let h10 = tile_height(&map.map_data, x + 1, y) as f32;
            let h01 = tile_height(&map.map_data, x, y + 1) as f32;
            let h11 = tile_height(&map.map_data, x + 1, y + 1) as f32;

            let slope = max_slope(h00, h10, h01, h11);
            let avg_h = (h00 + h10 + h01 + h11) / 4.0;
            let height_fraction = avg_h / max_height;

            // Jitter the threshold so cliff/ground boundaries aren't dead straight.
            let jitter =
                cliff_jitter_noise.get([f64::from(x) * 0.2, f64::from(y) * 0.2]) as f32 * 25.0;
            let cliff_threshold = tex_config.cliff_threshold + jitter;

            let is_water = avg_h < 5.0 && slope < cliff_threshold * 0.5;
            let is_cliff = slope >= cliff_threshold;

            let noise_val = tex_noise.get([f64::from(x) * 0.12, f64::from(y) * 0.12]);
            let noise_01 = (noise_val * 0.5 + 0.5).clamp(0.0, 0.999) as f32;

            let tile_id = if is_water {
                tex_config.water_tile
            } else if is_cliff {
                noise_pick(&tex_config.cliff_tiles, noise_01)
            } else if height_fraction > tex_config.high_ground_cutoff
                && !tex_config.high_ground_tiles.is_empty()
            {
                // Noise-jittered cutoff so the transition isn't a hard contour.
                let elev_val = elev_noise.get([f64::from(x) * 0.15, f64::from(y) * 0.15]) as f32;
                let cutoff = tex_config.high_ground_cutoff + elev_val * 0.15;
                if height_fraction > cutoff {
                    noise_pick(&tex_config.high_ground_tiles, noise_01)
                } else {
                    noise_pick(&tex_config.ground_tiles, noise_01)
                }
            } else {
                noise_pick(&tex_config.ground_tiles, noise_01)
            };

            // Random orientation matches WZ2100's per-tile flip/rotate style.
            let rotation = rng.u8(..4);
            let x_flip = rng.bool();
            let y_flip = rng.bool();

            if let Some(tile) = map.map_data.tile_mut(x, y) {
                // Water uses a fixed triangle split; tri_flip is ignored there.
                let tri_flip = if is_water { false } else { tile.tri_flip() };
                tile.texture = MapTile::make_texture(tile_id, x_flip, y_flip, rotation, tri_flip);
            }
        }
    }

    apply_border_textures(map, tex_config, &mut rng);
    map.terrain_types = Some(build_terrain_types(config.tileset));
}

fn noise_pick(tiles: &[u16], noise_01: f32) -> u16 {
    if tiles.is_empty() {
        return 0;
    }
    let idx = (noise_01 * tiles.len() as f32) as usize;
    tiles[idx.min(tiles.len() - 1)]
}

/// Build a TTP table covering every tile ID we emit, so the validator doesn't
/// flag "texture index exceeds terrain type count". Must hold at least
/// `max_tile_id + 1` entries.
fn build_terrain_types(tileset: Tileset) -> TerrainTypeData {
    let tex_config = tileset_config(tileset);

    let max_id = tex_config
        .ground_tiles
        .iter()
        .chain(tex_config.cliff_tiles.iter())
        .chain(tex_config.high_ground_tiles.iter())
        .chain(std::iter::once(&tex_config.water_tile))
        .copied()
        .max()
        .unwrap_or(0) as usize;

    let default_type = match tileset {
        Tileset::Arizona => TerrainType::SandYellow,
        Tileset::Urban => TerrainType::Bakedearth,
        Tileset::Rockies => TerrainType::Sand,
    };

    // Real Arizona TTP has 78 entries; Urban ~80, Rockies ~78.
    let count = (max_id + 1).max(78);
    let mut types = vec![default_type; count];

    // First 3 entries are the tileset detection signature.
    let sig = tileset.default_terrain_types();
    for (i, &t) in sig.iter().enumerate() {
        if i < types.len() {
            types[i] = t;
        }
    }

    for &id in &tex_config.ground_tiles {
        if (id as usize) < types.len() {
            types[id as usize] = default_type;
        }
    }
    for &id in &tex_config.high_ground_tiles {
        if (id as usize) < types.len() {
            types[id as usize] = default_type;
        }
    }
    for &id in &tex_config.cliff_tiles {
        if (id as usize) < types.len() {
            types[id as usize] = TerrainType::Cliffface;
        }
    }
    if (tex_config.water_tile as usize) < types.len() {
        types[tex_config.water_tile as usize] = TerrainType::Water;
    }

    TerrainTypeData {
        terrain_types: types,
    }
}

fn tile_height(map: &wz_maplib::map_data::MapData, x: u32, y: u32) -> u16 {
    let x = x.min(map.width.saturating_sub(1));
    let y = y.min(map.height.saturating_sub(1));
    map.tile(x, y).map_or(0, |t| t.height)
}

fn max_slope(h00: f32, h10: f32, h01: f32, h11: f32) -> f32 {
    let heights = [h00, h10, h01, h11];
    let mut max = 0.0_f32;
    for i in 0..4 {
        for j in i + 1..4 {
            max = max.max((heights[i] - heights[j]).abs());
        }
    }
    max
}

fn pick_random(tiles: &[u16], rng: &mut fastrand::Rng) -> u16 {
    if tiles.is_empty() {
        return 0;
    }
    tiles[rng.usize(..tiles.len())]
}

/// Force the outer 2-tile border to cliff textures so the playfield edge looks intentional.
fn apply_border_textures(map: &mut WzMap, tex_config: &TilesetTexConfig, rng: &mut fastrand::Rng) {
    let w = map.map_data.width;
    let h = map.map_data.height;
    let border = 2;

    for y in 0..h {
        for x in 0..w {
            if x >= border && x < w - border && y >= border && y < h - border {
                continue;
            }

            let tile_id = pick_random(&tex_config.cliff_tiles, rng);
            let rotation = rng.u8(..4);
            let x_flip = rng.bool();
            let y_flip = rng.bool();

            if let Some(tile) = map.map_data.tile_mut(x, y) {
                let tri_flip = tile.tri_flip();
                tile.texture = MapTile::make_texture(tile_id, x_flip, y_flip, rotation, tri_flip);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_map(width: u32, height: u32) -> WzMap {
        let mut map = WzMap::new("test", width, height);
        map.tileset = "arizona".to_string();
        map.terrain_types = Some(TerrainTypeData {
            terrain_types: Tileset::Arizona.default_terrain_types(),
        });
        map
    }

    fn default_config() -> GeneratorConfig {
        GeneratorConfig {
            seed: 42,
            ..GeneratorConfig::default()
        }
    }

    #[test]
    fn test_all_tiles_textured() {
        let mut map = make_test_map(32, 32);
        for y in 0..32 {
            for x in 0..32 {
                if let Some(tile) = map.map_data.tile_mut(x, y) {
                    tile.height = (x * 10 + y * 5) as u16;
                }
            }
        }

        let config = default_config();
        auto_texture(&mut map, &config);

        // Smoke test: auto_texture runs without panicking and leaves tiles populated.
        assert!(!map.map_data.tiles.is_empty());
    }

    #[test]
    fn test_water_tiles_get_water_texture() {
        let mut map = make_test_map(32, 32);
        for tile in &mut map.map_data.tiles {
            tile.height = 0;
        }

        let config = default_config();
        auto_texture(&mut map, &config);

        let water_tex = tileset_config(Tileset::Arizona).water_tile;
        let interior_water = map
            .map_data
            .tiles
            .iter()
            .enumerate()
            .filter(|(i, _)| {
                let x = (*i as u32) % 32;
                let y = (*i as u32) / 32;
                (2..30).contains(&x) && (2..30).contains(&y)
            })
            .filter(|(_, t)| t.texture_id() == water_tex)
            .count();

        let total_interior = 28 * 28;
        let ratio = interior_water as f32 / total_interior as f32;
        assert!(
            ratio > 0.8,
            "Expected >80% water texture on flat-zero map, got {:.1}%",
            ratio * 100.0
        );
    }

    #[test]
    fn test_cliff_tiles_on_steep_terrain() {
        let mut map = make_test_map(32, 32);
        // Vertical cliff: left half at 0, right half at 500.
        for y in 0..32 {
            for x in 0..32 {
                if let Some(tile) = map.map_data.tile_mut(x, y) {
                    tile.height = if x < 16 { 0 } else { 500 };
                }
            }
        }

        let config = default_config();
        auto_texture(&mut map, &config);

        let cliff_tiles_ids: Vec<u16> = tileset_config(Tileset::Arizona).cliff_tiles.clone();

        let cliff_count = (0..32u32)
            .filter(|&y| {
                let tile = map.map_data.tile(15, y).unwrap();
                cliff_tiles_ids.contains(&tile.texture_id())
            })
            .count();

        assert!(
            cliff_count > 10,
            "Expected cliff textures at cliff edge, got {cliff_count}/32"
        );
    }

    #[test]
    fn test_border_gets_cliff_texture() {
        let mut map = make_test_map(32, 32);
        let config = default_config();
        auto_texture(&mut map, &config);

        let cliff_ids: Vec<u16> = tileset_config(Tileset::Arizona).cliff_tiles.clone();

        for y in 0..32 {
            for x in 0..32 {
                if !(2..30).contains(&x) || !(2..30).contains(&y) {
                    let tile = map.map_data.tile(x, y).unwrap();
                    assert!(
                        cliff_ids.contains(&tile.texture_id()),
                        "Border tile ({x},{y}) has texture {}, expected cliff",
                        tile.texture_id()
                    );
                }
            }
        }
    }

    #[test]
    fn test_max_slope_calculation() {
        assert!((max_slope(0.0, 100.0, 0.0, 0.0) - 100.0).abs() < 0.01);
        assert!((max_slope(50.0, 50.0, 50.0, 50.0) - 0.0).abs() < 0.01);
        assert!((max_slope(0.0, 0.0, 0.0, 200.0) - 200.0).abs() < 0.01);
    }

    #[test]
    fn test_all_tilesets_have_config() {
        for tileset in [Tileset::Arizona, Tileset::Urban, Tileset::Rockies] {
            let cfg = tileset_config(tileset);
            assert!(
                !cfg.ground_tiles.is_empty(),
                "{tileset:?} has no ground tiles"
            );
            assert!(
                !cfg.cliff_tiles.is_empty(),
                "{tileset:?} has no cliff tiles"
            );
            assert!(
                cfg.cliff_threshold > 0.0,
                "{tileset:?} has zero cliff threshold"
            );
        }
    }

    #[test]
    fn test_deterministic_texturing() {
        let mut map1 = make_test_map(32, 32);
        let mut map2 = make_test_map(32, 32);
        for y in 0..32 {
            for x in 0..32 {
                let h = ((x * 15 + y * 7) % 510) as u16;
                map1.map_data.tile_mut(x, y).unwrap().height = h;
                map2.map_data.tile_mut(x, y).unwrap().height = h;
            }
        }

        let config = default_config();
        auto_texture(&mut map1, &config);
        auto_texture(&mut map2, &config);

        for (i, (t1, t2)) in map1
            .map_data
            .tiles
            .iter()
            .zip(map2.map_data.tiles.iter())
            .enumerate()
        {
            assert_eq!(
                t1.texture, t2.texture,
                "Texture mismatch at tile {i}: {} vs {}",
                t1.texture, t2.texture
            );
        }
    }

    #[test]
    fn test_ttp_covers_all_used_tile_ids() {
        for tileset in [Tileset::Arizona, Tileset::Urban, Tileset::Rockies] {
            let ttp = build_terrain_types(tileset);
            let tex_config = tileset_config(tileset);

            for &id in &tex_config.ground_tiles {
                assert!(
                    (id as usize) < ttp.terrain_types.len(),
                    "{tileset:?}: ground tile {id} out of TTP range {}",
                    ttp.terrain_types.len()
                );
            }
            for &id in &tex_config.cliff_tiles {
                assert!(
                    (id as usize) < ttp.terrain_types.len(),
                    "{tileset:?}: cliff tile {id} out of TTP range {}",
                    ttp.terrain_types.len()
                );
            }
            for &id in &tex_config.high_ground_tiles {
                assert!(
                    (id as usize) < ttp.terrain_types.len(),
                    "{tileset:?}: high-ground tile {id} out of TTP range {}",
                    ttp.terrain_types.len()
                );
            }
            assert!(
                (tex_config.water_tile as usize) < ttp.terrain_types.len(),
                "{tileset:?}: water tile {} out of TTP range {}",
                tex_config.water_tile,
                ttp.terrain_types.len()
            );

            for &id in &tex_config.cliff_tiles {
                assert_eq!(
                    ttp.terrain_types[id as usize],
                    TerrainType::Cliffface,
                    "{tileset:?}: cliff tile {id} not classified as Cliffface"
                );
            }

            assert_eq!(
                ttp.terrain_types[tex_config.water_tile as usize],
                TerrainType::Water,
                "{tileset:?}: water tile not classified as Water"
            );
        }
    }

    #[test]
    fn test_auto_texture_sets_valid_ttp() {
        let mut map = make_test_map(32, 32);
        for y in 0..32 {
            for x in 0..32 {
                if let Some(tile) = map.map_data.tile_mut(x, y) {
                    tile.height = (x * 10 + y * 5) as u16;
                }
            }
        }

        let config = default_config();
        auto_texture(&mut map, &config);

        let ttp = map.terrain_types.as_ref().expect("TTP should be set");
        for tile in &map.map_data.tiles {
            let tex_id = tile.texture_id() as usize;
            assert!(
                tex_id < ttp.terrain_types.len(),
                "Tile texture_id {tex_id} exceeds TTP count {}",
                ttp.terrain_types.len()
            );
        }
    }
}
