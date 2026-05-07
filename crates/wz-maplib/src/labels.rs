//! Script labels for positions, areas, and object references.
//!
//! WZ2100 maps use `labels.json` to define named markers referenced by
//! campaign scripts (`eventArea*` triggers, `getObject("label")` calls).

use serde::{Deserialize, Serialize};

use crate::objects::WorldPos;

/// A script label that marks a position, area, or object on the map.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ScriptLabel {
    Position {
        label: String,
        pos: [u32; 2],
    },
    Area {
        label: String,
        pos1: [u32; 2],
        pos2: [u32; 2],
    },
}

impl ScriptLabel {
    /// Returns the human-readable label name.
    pub fn label(&self) -> &str {
        match self {
            ScriptLabel::Position { label, .. } | ScriptLabel::Area { label, .. } => label,
        }
    }

    /// Returns the center position of this label in world coordinates.
    pub fn center(&self) -> WorldPos {
        match self {
            ScriptLabel::Position { pos, .. } => WorldPos {
                x: pos[0],
                y: pos[1],
            },
            ScriptLabel::Area { pos1, pos2, .. } => WorldPos {
                x: u32::midpoint(pos1[0], pos2[0]),
                y: u32::midpoint(pos1[1], pos2[1]),
            },
        }
    }

    /// Creates a new position label.
    pub fn new_position(label: String, x: u32, y: u32) -> Self {
        ScriptLabel::Position { label, pos: [x, y] }
    }

    /// Creates a new area label.
    pub fn new_area(label: String, x1: u32, y1: u32, x2: u32, y2: u32) -> Self {
        ScriptLabel::Area {
            label,
            pos1: [x1.min(x2), y1.min(y2)],
            pos2: [x1.max(x2), y1.max(y2)],
        }
    }
}

/// Read labels from a `labels.json` byte slice.
pub fn read_labels(data: &[u8]) -> Result<Vec<(String, ScriptLabel)>, crate::MapError> {
    let map: serde_json::Map<String, serde_json::Value> =
        serde_json::from_slice(data).map_err(|e| crate::MapError::Json {
            file: "labels.json".to_string(),
            source: e,
        })?;

    let mut labels = Vec::with_capacity(map.len());
    for (key, value) in map {
        // object_* labels reference object IDs, not map positions.
        if key.starts_with("object_") {
            continue;
        }
        let label: ScriptLabel =
            serde_json::from_value(value).map_err(|e| crate::MapError::Json {
                file: format!("labels.json[{key}]"),
                source: e,
            })?;
        labels.push((key, label));
    }

    Ok(labels)
}

/// Serialize labels to JSON bytes.
pub fn write_labels(labels: &[(String, ScriptLabel)]) -> Result<Vec<u8>, crate::MapError> {
    let mut map = serde_json::Map::new();
    for (key, label) in labels {
        let value = serde_json::to_value(label).map_err(|e| crate::MapError::Json {
            file: "labels.json".to_string(),
            source: e,
        })?;
        map.insert(key.clone(), value);
    }
    serde_json::to_vec_pretty(&map).map_err(|e| crate::MapError::Json {
        file: "labels.json".to_string(),
        source: e,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn position_label_center() {
        let label = ScriptLabel::new_position("startPos".into(), 3200, 4480);
        assert_eq!(label.label(), "startPos");
        let c = label.center();
        assert_eq!(c.x, 3200);
        assert_eq!(c.y, 4480);
    }

    #[test]
    fn area_label_center() {
        let label = ScriptLabel::new_area("zone1".into(), 1000, 2000, 3000, 4000);
        assert_eq!(label.label(), "zone1");
        let c = label.center();
        assert_eq!(c.x, 2000); // (1000+3000)/2
        assert_eq!(c.y, 3000); // (2000+4000)/2
    }

    #[test]
    fn area_label_normalizes_coords() {
        // Pass pos2 < pos1; new_area must swap.
        let label = ScriptLabel::new_area("zone".into(), 3000, 4000, 1000, 2000);
        match &label {
            ScriptLabel::Area { pos1, pos2, .. } => {
                assert_eq!(pos1[0], 1000);
                assert_eq!(pos1[1], 2000);
                assert_eq!(pos2[0], 3000);
                assert_eq!(pos2[1], 4000);
            }
            ScriptLabel::Position { .. } => panic!("expected Area variant"),
        }
    }

    #[test]
    fn labels_roundtrip() {
        let labels = vec![
            (
                "start".to_string(),
                ScriptLabel::new_position("start".into(), 100, 200),
            ),
            (
                "area1".to_string(),
                ScriptLabel::new_area("area1".into(), 0, 0, 500, 500),
            ),
        ];

        let bytes = write_labels(&labels).unwrap();
        let loaded = read_labels(&bytes).unwrap();

        assert_eq!(loaded.len(), 2);
        let start = loaded.iter().find(|(k, _)| k == "start").unwrap();
        assert_eq!(start.1.center().x, 100);
        let area = loaded.iter().find(|(k, _)| k == "area1").unwrap();
        assert_eq!(area.1.center().x, 250);
    }

    #[test]
    fn read_labels_skips_object_entries() {
        let json = br#"{
            "start": {"label": "start", "pos": [100, 200]},
            "object_tank": {"id": 42, "type": 1, "player": 0}
        }"#;
        let loaded = read_labels(json).unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].0, "start");
    }

    #[test]
    fn read_labels_empty_json() {
        let json = b"{}";
        let loaded = read_labels(json).unwrap();
        assert!(loaded.is_empty());
    }

    #[test]
    fn read_labels_invalid_json() {
        let json = b"not json";
        assert!(read_labels(json).is_err());
    }
}
