use std::path::PathBuf;

fn wz2100_data_dir() -> Option<PathBuf> {
    let candidates = [PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../data")];
    candidates.into_iter().find(|p| p.exists())
}

/// Load a real WZ2100 map from disk and verify a write-read roundtrip.
///
/// Requires the WZ2100 `data/` directory to be present three levels above
/// the manifest dir. Run with `cargo test -- --ignored` when data is available.
#[test]
#[ignore = "requires WZ2100 data directory on disk"]
fn load_real_map_roundtrip() {
    let data_dir = wz2100_data_dir().expect("WZ2100 data directory not found");

    let map_dir = data_dir.join("mp/multiplay/maps/10c-Emergence");
    assert!(map_dir.exists(), "map directory not found: {map_dir:?}");

    let map = wz_maplib::io_wz::load_from_directory(&map_dir).expect("Failed to load map");

    assert!(map.map_data.width > 0);
    assert!(map.map_data.height > 0);
    assert_eq!(
        map.map_data.tiles.len(),
        (map.map_data.width * map.map_data.height) as usize,
    );
    assert!(!map.structures.is_empty(), "should have structures");
    assert!(!map.features.is_empty(), "should have features");
    assert!(map.terrain_types.is_some(), "should have terrain type data");

    let dir = std::env::temp_dir().join("wz_maplib_test_roundtrip");
    let _ = std::fs::remove_dir_all(&dir);
    wz_maplib::io_wz::save_to_directory(&map, &dir, wz_maplib::OutputFormat::Ver3)
        .expect("Failed to save map");

    let reloaded = wz_maplib::io_wz::load_from_directory(&dir).expect("Failed to reload map");
    assert_eq!(reloaded.map_data.width, map.map_data.width);
    assert_eq!(reloaded.map_data.height, map.map_data.height);
    assert_eq!(reloaded.map_data.tiles.len(), map.map_data.tiles.len());
    assert_eq!(reloaded.structures.len(), map.structures.len());
    assert_eq!(reloaded.features.len(), map.features.len());

    let _ = std::fs::remove_dir_all(&dir);
}

/// Parse a real PIE model file and validate its structure.
///
/// Run with `cargo test -- --ignored` when data is available.
#[test]
#[ignore = "requires WZ2100 data directory on disk"]
fn load_real_pie_model() {
    let data_dir = wz2100_data_dir().expect("WZ2100 data directory not found");

    let pie_path = data_dir.join("mp/components/prop/prsvtl1.pie");
    assert!(pie_path.exists(), "PIE file not found: {pie_path:?}");

    let content = std::fs::read_to_string(&pie_path).expect("Failed to read PIE file");
    let model = wz_pie::parse_pie(&content).expect("Failed to parse PIE model");

    assert_eq!(model.version, 3);
    assert!(!model.levels.is_empty(), "should have at least one level");

    let level = &model.levels[0];
    assert!(!level.vertices.is_empty(), "level should have vertices");
    assert!(!level.polygons.is_empty(), "level should have polygons");

    // Every polygon must have matching tex_coords for its indices.
    for poly in &level.polygons {
        assert_eq!(
            poly.tex_coords.len(),
            poly.indices.len(),
            "each vertex should have a tex coord"
        );
    }
}
