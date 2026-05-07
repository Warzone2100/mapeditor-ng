//! Terrain validation: tile heights, texture indices, water triangle direction.

use crate::constants::TILE_MAX_HEIGHT;
use crate::io_wz::WzMap;
use crate::terrain_types::TerrainType;

use super::push_warning;
use super::types::{
    IssueLocation, ValidationCategory, ValidationConfig, ValidationResults, WarningRule,
};

/// Terrain checks: tile heights, texture indices, water triangle direction.
pub(super) fn validate_terrain(
    map: &WzMap,
    config: &ValidationConfig,
    results: &mut ValidationResults,
) {
    let cat = ValidationCategory::Terrain;
    let md = &map.map_data;
    let ttp_count = map.terrain_types.as_ref().map(|t| t.terrain_types.len());

    let check_height = config.is_enabled(WarningRule::TileHeightOverflow);
    let check_texture = config.is_enabled(WarningRule::TileTextureOutOfRange);
    let check_water_tri = config.is_enabled(WarningRule::WaterTriangleDirection);

    if !check_height && !check_texture && !check_water_tri {
        return;
    }

    for ty in 0..md.height {
        for tx in 0..md.width {
            let Some(tile) = md.tile(tx, ty) else {
                continue;
            };

            if check_height && tile.height > TILE_MAX_HEIGHT {
                push_warning(
                    results,
                    WarningRule::TileHeightOverflow,
                    cat,
                    format!(
                        "Tile ({tx}, {ty}) height {} exceeds maximum {TILE_MAX_HEIGHT}.",
                        tile.height
                    ),
                    IssueLocation::TilePos { x: tx, y: ty },
                );
            }

            let tex_id = tile.texture_id() as usize;
            if check_texture
                && let Some(count) = ttp_count
                && count > 0
                && tex_id >= count
            {
                push_warning(
                    results,
                    WarningRule::TileTextureOutOfRange,
                    cat,
                    format!(
                        "Tile ({tx}, {ty}) texture index {tex_id} exceeds terrain type count {count}.",
                    ),
                    IssueLocation::TilePos { x: tx, y: ty },
                );
            }

            // FlaME's ValidateMap_WaterTris: WZ2100 renders water with a
            // fixed triangle pattern and ignores tri_flip, so a flipped water
            // tile mismatches the terrain mesh.
            if check_water_tri
                && let Some(ref ttp) = map.terrain_types
                && let Some(&tt) = ttp.terrain_types.get(tex_id)
                && tt == TerrainType::Water
                && tile.tri_flip()
            {
                push_warning(
                    results,
                    WarningRule::WaterTriangleDirection,
                    cat,
                    format!(
                        "Water tile ({tx}, {ty}) has incorrect triangle direction. This may cause graphical glitches."
                    ),
                    IssueLocation::TilePos { x: tx, y: ty },
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map_data::MapTile;
    use crate::terrain_types::TerrainTypeData;
    use crate::validate::test_support::valid_map;

    #[test]
    fn tile_heights_all_valid() {
        let mut map = valid_map(4, 4);
        map.map_data.tile_mut(0, 0).unwrap().height = 0;
        map.map_data.tile_mut(1, 1).unwrap().height = TILE_MAX_HEIGHT;
        let mut results = ValidationResults::default();
        validate_terrain(&map, &ValidationConfig::default(), &mut results);
        assert!(
            !results.issues.iter().any(|i| i.message.contains("height")),
            "issues: {:?}",
            results.issues
        );
    }

    #[test]
    fn tile_height_exceeds_max() {
        let mut map = valid_map(4, 4);
        map.map_data.tile_mut(2, 3).unwrap().height = 511;
        let mut results = ValidationResults::default();
        validate_terrain(&map, &ValidationConfig::default(), &mut results);
        assert!(
            results
                .issues
                .iter()
                .any(|i| i.message.contains("height 511"))
        );
    }

    #[test]
    fn tile_height_at_max_ok() {
        let mut map = valid_map(4, 4);
        map.map_data.tile_mut(1, 1).unwrap().height = TILE_MAX_HEIGHT;
        let mut results = ValidationResults::default();
        validate_terrain(&map, &ValidationConfig::default(), &mut results);
        assert!(
            !results.issues.iter().any(|i| i.message.contains("height")),
            "issues: {:?}",
            results.issues
        );
    }

    #[test]
    fn multiple_tiles_over_max_each_reported() {
        let mut map = valid_map(4, 4);
        map.map_data.tile_mut(0, 0).unwrap().height = 600;
        map.map_data.tile_mut(1, 1).unwrap().height = 700;
        map.map_data.tile_mut(2, 2).unwrap().height = 800;
        let mut results = ValidationResults::default();
        validate_terrain(&map, &ValidationConfig::default(), &mut results);
        let height_issues: Vec<_> = results
            .issues
            .iter()
            .filter(|i| i.message.contains("height"))
            .collect();
        assert_eq!(height_issues.len(), 3);
    }

    #[test]
    fn tile_texture_index_in_range_ok() {
        let mut map = valid_map(4, 4);
        map.map_data.tile_mut(0, 0).unwrap().texture =
            MapTile::make_texture(77, false, false, 0, false);
        let mut results = ValidationResults::default();
        validate_terrain(&map, &ValidationConfig::default(), &mut results);
        assert!(
            !results
                .issues
                .iter()
                .any(|i| i.message.contains("texture index")),
            "issues: {:?}",
            results.issues
        );
    }

    #[test]
    fn tile_texture_index_exceeds_ttp_count() {
        let mut map = valid_map(4, 4);
        map.map_data.tile_mut(0, 0).unwrap().texture =
            MapTile::make_texture(78, false, false, 0, false);
        let mut results = ValidationResults::default();
        validate_terrain(&map, &ValidationConfig::default(), &mut results);
        assert!(
            results
                .issues
                .iter()
                .any(|i| i.message.contains("texture index 78"))
        );
    }

    #[test]
    fn tile_texture_index_no_ttp_skipped() {
        let mut map = WzMap::new("TestMap", 4, 4);
        map.terrain_types = None;
        map.map_data.tile_mut(0, 0).unwrap().texture =
            MapTile::make_texture(0x3E7 & 0x1FF, false, false, 0, false);
        let mut results = ValidationResults::default();
        validate_terrain(&map, &ValidationConfig::default(), &mut results);
        assert!(
            !results
                .issues
                .iter()
                .any(|i| i.message.contains("texture index"))
        );
    }

    #[test]
    fn water_tile_triangle_direction_correct() {
        let mut map = valid_map(4, 4);
        let mut ttp = map.terrain_types.take().unwrap();
        ttp.terrain_types[1] = TerrainType::Water;
        map.terrain_types = Some(ttp);
        // Water with tri_flip=false is the correct default.
        map.map_data.tile_mut(0, 0).unwrap().texture =
            MapTile::make_texture(1, false, false, 0, false);
        let mut results = ValidationResults::default();
        validate_terrain(&map, &ValidationConfig::default(), &mut results);
        assert!(
            !results
                .issues
                .iter()
                .any(|i| i.message.contains("triangle direction")),
            "issues: {:?}",
            results.issues
        );
    }

    #[test]
    fn water_tile_triangle_direction_incorrect() {
        let mut map = valid_map(4, 4);
        let mut ttp = map.terrain_types.take().unwrap();
        ttp.terrain_types[1] = TerrainType::Water;
        map.terrain_types = Some(ttp);
        // Water with tri_flip=true is incorrect; FlaME flags these.
        map.map_data.tile_mut(0, 0).unwrap().texture =
            MapTile::make_texture(1, false, false, 0, true);
        let mut results = ValidationResults::default();
        validate_terrain(&map, &ValidationConfig::default(), &mut results);
        let water_issues: Vec<_> = results
            .issues
            .iter()
            .filter(|i| i.message.contains("triangle direction"))
            .collect();
        assert_eq!(water_issues.len(), 1);
        assert!(water_issues[0].message.contains("(0, 0)"));
    }

    #[test]
    fn disabled_water_tri_suppresses_terrain_warning() {
        let mut config = ValidationConfig::default();
        config.disabled.insert(WarningRule::WaterTriangleDirection);

        let mut map = WzMap::new("Test", 4, 4);
        let mut ttp = TerrainTypeData {
            terrain_types: vec![TerrainType::Sand; 78],
        };
        ttp.terrain_types[1] = TerrainType::Water;
        map.terrain_types = Some(ttp);
        let tile = map.map_data.tile_mut(1, 1).unwrap();
        tile.texture = MapTile::make_texture(1, false, false, 0, true);

        let r = crate::validate::validate_map(&map, None, &ValidationConfig::default());
        assert!(
            r.issues
                .iter()
                .any(|i| i.message.contains("triangle direction"))
        );

        let r = crate::validate::validate_map(&map, None, &config);
        assert!(
            !r.issues
                .iter()
                .any(|i| i.message.contains("triangle direction"))
        );
    }
}
