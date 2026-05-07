//! JSON object file reader and writer.

use serde::Deserialize;

use crate::MapError;
use crate::objects::{Droid, Feature, Structure, WorldPos};

// Handles v1 (named map) and v2 (versioned array). Both versions use
// "rotation" but with different shapes (see resolve_direction). Droids
// use "template" instead of "name".
#[derive(Debug, Deserialize)]
struct ObjectEntry {
    #[serde(alias = "template")]
    name: String,
    #[serde(default)]
    id: Option<u32>,
    #[serde(default)]
    position: Vec<u32>,
    #[serde(default)]
    rotation: Option<serde_json::Value>,
    #[serde(default)]
    player: Option<serde_json::Value>,
    #[serde(default)]
    startpos: Option<u32>,
    #[serde(default)]
    modules: Option<u8>,
}

impl ObjectEntry {
    /// Direction is stored in WZ2100's internal 0-65535 range (not degrees).
    /// v1 wraps it in `[yaw, pitch, roll]`; v2 uses a scalar or `[value]`.
    fn resolve_direction(&self) -> u16 {
        if let Some(ref rot) = self.rotation {
            if let Some(n) = rot.as_u64() {
                return n as u16;
            }
            if let Some(arr) = rot.as_array()
                && let Some(first) = arr.first().and_then(serde_json::Value::as_u64)
            {
                return first as u16;
            }
        }
        0
    }

    fn resolve_player(&self) -> i8 {
        if let Some(ref val) = self.player {
            if let Some(s) = val.as_str()
                && s.eq_ignore_ascii_case("scavenger")
            {
                return -1;
            }
            if let Some(n) = val.as_i64() {
                return n as i8;
            }
        }
        if let Some(sp) = self.startpos {
            return sp as i8;
        }
        0
    }

    fn resolve_position(&self) -> WorldPos {
        WorldPos {
            x: self.position.first().copied().unwrap_or(0),
            y: self.position.get(1).copied().unwrap_or(0),
        }
    }
}

/// Parse a JSON file that can be either:
/// - v1 format: `{ "key1": { "name": ..., ... }, "key2": { ... } }` (named map)
/// - v2 format: `{ "version": 2, "<container>": [ { "name": ..., ... }, ... ] }` (versioned array)
fn parse_object_entries(json_str: &str, container_key: &str) -> Result<Vec<ObjectEntry>, MapError> {
    let value: serde_json::Value = serde_json::from_str(json_str).map_err(|e| MapError::Json {
        file: container_key.into(),
        source: e,
    })?;

    if let Some(obj) = value.as_object() {
        if obj.contains_key("version") {
            if let Some(arr) = obj.get(container_key) {
                let entries: Vec<ObjectEntry> =
                    serde_json::from_value(arr.clone()).map_err(|e| MapError::Json {
                        file: container_key.into(),
                        source: e,
                    })?;
                return Ok(entries);
            }
            return Ok(Vec::new());
        }

        let mut entries = Vec::new();
        for (_key, val) in obj {
            match serde_json::from_value::<ObjectEntry>(val.clone()) {
                Ok(entry) => entries.push(entry),
                Err(e) => {
                    log::warn!("Skipping malformed entry: {e}");
                }
            }
        }
        return Ok(entries);
    }

    if value.is_array() {
        let entries: Vec<ObjectEntry> =
            serde_json::from_value(value).map_err(|e| MapError::Json {
                file: container_key.into(),
                source: e,
            })?;
        return Ok(entries);
    }

    Ok(Vec::new())
}

/// Read structures from a JSON string (struct.json).
pub fn read_structures(json_str: &str) -> Result<Vec<Structure>, MapError> {
    let entries = parse_object_entries(json_str, "structures")?;
    Ok(entries
        .into_iter()
        .map(|e| {
            let position = e.resolve_position();
            let direction = e.resolve_direction();
            let player = e.resolve_player();
            Structure {
                name: e.name,
                position,
                direction,
                player,
                modules: e.modules.unwrap_or(0),
                id: e.id,
            }
        })
        .collect())
}

/// Write structures to JSON string (v2 format).
pub fn write_structures(structures: &[Structure]) -> Result<String, MapError> {
    let entries: Vec<serde_json::Value> = structures
        .iter()
        .map(|s| {
            let mut obj = serde_json::json!({
                "name": s.name,
                "position": [s.position.x, s.position.y],
                "rotation": s.direction,
            });
            if s.player == -1 {
                obj["player"] = serde_json::json!("scavenger");
            } else {
                obj["startpos"] = serde_json::json!(s.player);
            }
            if s.modules > 0 {
                obj["modules"] = serde_json::json!(s.modules);
            }
            if let Some(id) = s.id {
                obj["id"] = serde_json::json!(id);
            }
            obj
        })
        .collect();

    let file = serde_json::json!({
        "version": 2,
        "structures": entries,
    });

    serde_json::to_string_pretty(&file).map_err(|e| MapError::Json {
        file: "struct.json".into(),
        source: e,
    })
}

/// Read droids from a JSON string (droid.json).
pub fn read_droids(json_str: &str) -> Result<Vec<Droid>, MapError> {
    let entries = parse_object_entries(json_str, "droids")?;
    Ok(entries
        .into_iter()
        .map(|e| {
            let position = e.resolve_position();
            let direction = e.resolve_direction();
            let player = e.resolve_player();
            Droid {
                name: e.name,
                position,
                direction,
                player,
                id: e.id,
            }
        })
        .collect())
}

/// Write droids to JSON string (v2 format).
pub fn write_droids(droids: &[Droid]) -> Result<String, MapError> {
    let entries: Vec<serde_json::Value> = droids
        .iter()
        .map(|d| {
            let mut obj = serde_json::json!({
                "template": d.name,
                "position": [d.position.x, d.position.y],
                "rotation": d.direction,
            });
            if d.player == -1 {
                obj["player"] = serde_json::json!("scavenger");
            } else {
                obj["startpos"] = serde_json::json!(d.player);
            }
            if let Some(id) = d.id {
                obj["id"] = serde_json::json!(id);
            }
            obj
        })
        .collect();

    let file = serde_json::json!({
        "version": 2,
        "droids": entries,
    });

    serde_json::to_string_pretty(&file).map_err(|e| MapError::Json {
        file: "droid.json".into(),
        source: e,
    })
}

/// Read features from a JSON string (feature.json).
pub fn read_features(json_str: &str) -> Result<Vec<Feature>, MapError> {
    let entries = parse_object_entries(json_str, "features")?;
    Ok(entries
        .into_iter()
        .map(|e| {
            let has_player = e.player.is_some() || e.startpos.is_some();
            let position = e.resolve_position();
            let direction = e.resolve_direction();
            let player = if has_player {
                Some(e.resolve_player())
            } else {
                None
            };
            Feature {
                name: e.name,
                position,
                direction,
                id: e.id,
                player,
            }
        })
        .collect())
}

/// Write features to JSON string (v2 format).
pub fn write_features(features: &[Feature]) -> Result<String, MapError> {
    let entries: Vec<serde_json::Value> = features
        .iter()
        .map(|f| {
            let mut obj = serde_json::json!({
                "name": f.name,
                "position": [f.position.x, f.position.y],
                "rotation": f.direction,
            });
            if let Some(player) = f.player {
                if player == -1 {
                    obj["player"] = serde_json::json!("scavenger");
                } else {
                    obj["startpos"] = serde_json::json!(player);
                }
            }
            if let Some(id) = f.id {
                obj["id"] = serde_json::json!(id);
            }
            obj
        })
        .collect();

    let file = serde_json::json!({
        "version": 2,
        "features": entries,
    });

    serde_json::to_string_pretty(&file).map_err(|e| MapError::Json {
        file: "feature.json".into(),
        source: e,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_structures_roundtrip() {
        let structs = vec![
            Structure {
                name: "A0PowerGenerator".to_string(),
                position: WorldPos { x: 3200, y: 4480 },
                direction: 0,
                player: 0,
                modules: 1,
                id: None,
            },
            Structure {
                name: "A0CommandCentre".to_string(),
                position: WorldPos { x: 1280, y: 1280 },
                direction: 16384, // 90° in WZ2100 internal encoding
                player: -1,
                modules: 0,
                id: Some(42),
            },
        ];

        let json = write_structures(&structs).unwrap();
        let loaded = read_structures(&json).unwrap();

        assert_eq!(loaded.len(), 2);
        assert_eq!(loaded[0].name, "A0PowerGenerator");
        assert_eq!(loaded[0].position.x, 3200);
        assert_eq!(loaded[0].modules, 1);
        assert_eq!(loaded[1].player, -1);
        assert_eq!(loaded[1].direction, 16384);
        assert_eq!(loaded[1].id, Some(42));
    }

    #[test]
    fn test_droids_roundtrip() {
        let droids = vec![Droid {
            name: "ViperMG".to_string(),
            position: WorldPos { x: 2000, y: 3000 },
            direction: 32768, // 180° in internal encoding
            player: 1,
            id: None,
        }];

        let json = write_droids(&droids).unwrap();
        // Droids use "template", not "name"; and "rotation", not "direction".
        assert!(json.contains("\"template\""));
        assert!(!json.contains("\"name\""));
        assert!(json.contains("\"rotation\""));
        assert!(!json.contains("\"direction\""));

        let loaded = read_droids(&json).unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].name, "ViperMG");
        assert_eq!(loaded[0].direction, 32768);
        assert_eq!(loaded[0].player, 1);
    }

    #[test]
    fn test_v1_named_map_format() {
        let json = r#"{
            "structure_1": {
                "name": "A0PowerGenerator",
                "position": [3200, 4480, 0],
                "rotation": [0, 0, 0],
                "startpos": 2,
                "modules": 1
            },
            "structure_2": {
                "name": "A0CommandCentre",
                "position": [1280, 1280, 0],
                "rotation": [16384, 0, 0],
                "player": "scavenger"
            }
        }"#;

        let loaded = read_structures(json).unwrap();
        assert_eq!(loaded.len(), 2);

        // Order is not guaranteed: v1 HashMap iteration.
        let pg = loaded
            .iter()
            .find(|s| s.name == "A0PowerGenerator")
            .unwrap();
        assert_eq!(pg.position.x, 3200);
        assert_eq!(pg.player, 2);
        assert_eq!(pg.modules, 1);

        let cc = loaded.iter().find(|s| s.name == "A0CommandCentre").unwrap();
        assert_eq!(cc.player, -1);
        // Rotation kept in native 0-65535 encoding, not degrees.
        assert_eq!(cc.direction, 16384);
    }

    #[test]
    fn test_features_roundtrip() {
        let features = vec![
            Feature {
                name: "OilResource".to_string(),
                position: WorldPos { x: 5120, y: 6400 },
                direction: 0,
                id: Some(100),
                player: None,
            },
            Feature {
                name: "OilDrum".to_string(),
                position: WorldPos { x: 1024, y: 2048 },
                direction: 45,
                id: None,
                player: Some(0),
            },
        ];

        let json = write_features(&features).unwrap();
        let loaded = read_features(&json).unwrap();

        assert_eq!(loaded.len(), 2);
        let oil = loaded.iter().find(|f| f.name == "OilResource").unwrap();
        assert_eq!(oil.position.x, 5120);
        assert_eq!(oil.id, Some(100));
        assert!(oil.player.is_none());

        let drum = loaded.iter().find(|f| f.name == "OilDrum").unwrap();
        assert_eq!(drum.player, Some(0));
    }

    #[test]
    fn test_features_scavenger_player() {
        let features = vec![Feature {
            name: "Crate".to_string(),
            position: WorldPos { x: 100, y: 200 },
            direction: 0,
            id: None,
            player: Some(-1),
        }];

        let json = write_features(&features).unwrap();
        assert!(json.contains("scavenger"));
        let loaded = read_features(&json).unwrap();
        assert_eq!(loaded[0].player, Some(-1));
    }

    #[test]
    fn test_v1_rotation_preserved() {
        let json = r#"{
            "droid_1": {
                "name": "ViperMG",
                "position": [1000, 2000],
                "rotation": [32768, 0, 0],
                "startpos": 0
            }
        }"#;
        let loaded = read_droids(json).unwrap();
        assert_eq!(loaded[0].direction, 32768); // 180° in internal encoding
    }

    #[test]
    fn test_v1_rotation_270() {
        let json = r#"{
            "unit_1": {
                "name": "Tank",
                "position": [500, 600],
                "rotation": [49152, 0, 0],
                "startpos": 1
            }
        }"#;
        let loaded = read_droids(json).unwrap();
        assert_eq!(loaded[0].direction, 49152); // 270° in internal encoding
    }

    #[test]
    fn test_v1_droid_template_field() {
        let json = r#"{
            "droid_1": {
                "template": "ViperMG",
                "position": [1000, 2000],
                "rotation": [0, 0, 0],
                "startpos": 0
            }
        }"#;
        let loaded = read_droids(json).unwrap();
        assert_eq!(loaded[0].name, "ViperMG");
    }

    #[test]
    fn test_v2_rotation_scalar() {
        let json = r#"{"version": 2, "droids": [{"template": "ViperMG", "position": [1000, 2000], "rotation": 32768, "startpos": 0}]}"#;
        let loaded = read_droids(json).unwrap();
        assert_eq!(loaded[0].direction, 32768);
    }

    #[test]
    fn test_v2_rotation_single_array() {
        // Single-element array form, equivalent to scalar.
        let json = r#"{"version": 2, "droids": [{"template": "ViperMG", "position": [1000, 2000], "rotation": [16384], "startpos": 0}]}"#;
        let loaded = read_droids(json).unwrap();
        assert_eq!(loaded[0].direction, 16384);
    }

    #[test]
    fn test_player_scavenger_string() {
        let json = r#"{"version": 2, "structures": [{"name": "Wall", "position": [100, 200], "rotation": 0, "player": "scavenger"}]}"#;
        let loaded = read_structures(json).unwrap();
        assert_eq!(loaded[0].player, -1);
    }

    #[test]
    fn test_player_numeric() {
        let json = r#"{"version": 2, "structures": [{"name": "Wall", "position": [100, 200], "rotation": 0, "player": 3}]}"#;
        let loaded = read_structures(json).unwrap();
        assert_eq!(loaded[0].player, 3);
    }

    #[test]
    fn test_player_startpos_fallback() {
        let json = r#"{"version": 2, "structures": [{"name": "Wall", "position": [100, 200], "rotation": 0, "startpos": 5}]}"#;
        let loaded = read_structures(json).unwrap();
        assert_eq!(loaded[0].player, 5);
    }

    #[test]
    fn test_empty_v2_format() {
        let json = r#"{"version": 2, "structures": []}"#;
        let loaded = read_structures(json).unwrap();
        assert!(loaded.is_empty());
    }

    #[test]
    fn test_empty_v1_format() {
        let json = r"{}";
        let loaded = read_structures(json).unwrap();
        assert!(loaded.is_empty());
    }

    #[test]
    fn test_bare_array_format() {
        let json = r#"[{"name": "Wall", "position": [100, 200]}]"#;
        let loaded = read_structures(json).unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].name, "Wall");
        assert_eq!(loaded[0].direction, 0);
        assert_eq!(loaded[0].player, 0);
    }

    #[test]
    fn test_missing_position_defaults_to_zero() {
        let json = r#"{"version": 2, "structures": [{"name": "Wall"}]}"#;
        let loaded = read_structures(json).unwrap();
        assert_eq!(loaded[0].position.x, 0);
        assert_eq!(loaded[0].position.y, 0);
    }

    #[test]
    fn test_modules_field() {
        let json = r#"{"version": 2, "structures": [{"name": "A0PowerGenerator", "position": [100, 200], "modules": 2}]}"#;
        let loaded = read_structures(json).unwrap();
        assert_eq!(loaded[0].modules, 2);
    }

    #[test]
    fn test_modules_default_zero() {
        let json = r#"{"version": 2, "structures": [{"name": "A0PowerGenerator", "position": [100, 200]}]}"#;
        let loaded = read_structures(json).unwrap();
        assert_eq!(loaded[0].modules, 0);
    }

    #[test]
    fn test_scavenger_roundtrip_structures() {
        let structs = vec![Structure {
            name: "Wall".to_string(),
            position: WorldPos { x: 100, y: 200 },
            direction: 0,
            player: -1,
            modules: 0,
            id: None,
        }];
        let json = write_structures(&structs).unwrap();
        assert!(json.contains("\"scavenger\""));
        let loaded = read_structures(&json).unwrap();
        assert_eq!(loaded[0].player, -1);
    }

    #[test]
    fn test_scavenger_roundtrip_droids() {
        let droids = vec![Droid {
            name: "Truck".to_string(),
            position: WorldPos { x: 500, y: 600 },
            direction: 16384, // 90° in internal encoding
            player: -1,
            id: None,
        }];
        let json = write_droids(&droids).unwrap();
        assert!(json.contains("\"scavenger\""));
        let loaded = read_droids(&json).unwrap();
        assert_eq!(loaded[0].player, -1);
    }
}
