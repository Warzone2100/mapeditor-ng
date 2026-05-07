//! Terrain speed modifier table from `terraintable.json`.
//!
//! Maps (terrain type, propulsion class) pairs to per-tile speed factors.

use std::collections::HashMap;

use serde::Deserialize;

const TERRAIN_TYPE_COUNT: usize = 12;
const PROPULSION_CLASS_COUNT: usize = 7;

/// Discriminants match WZ2100's `PROPULSION_TYPE` enum from `statsdef.h`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum PropulsionClass {
    #[default]
    Wheeled = 0,
    Tracked = 1,
    HalfTracked = 2,
    Hover = 3,
    Legged = 4,
    Lift = 5,
    Propellor = 6,
}

impl PropulsionClass {
    /// Classes relevant for ground-unit heatmap visualization. Excludes
    /// Lift (VTOL) and Propellor, which ignore terrain.
    pub const GROUND: [Self; 5] = [
        Self::Wheeled,
        Self::Tracked,
        Self::HalfTracked,
        Self::Hover,
        Self::Legged,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Wheeled => "W",
            Self::Tracked => "T",
            Self::HalfTracked => "HT",
            Self::Hover => "Hv",
            Self::Legged => "Lg",
            Self::Lift => "Lf",
            Self::Propellor => "Pr",
        }
    }

    pub fn display_name(self) -> &'static str {
        match self {
            Self::Wheeled => "Wheeled",
            Self::Tracked => "Tracked",
            Self::HalfTracked => "Half-Tracked",
            Self::Hover => "Hover",
            Self::Legged => "Legs",
            Self::Lift => "Lift (VTOL)",
            Self::Propellor => "Propellor",
        }
    }
}

impl std::fmt::Display for PropulsionClass {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.display_name())
    }
}

/// Per-terrain-type speed factors for each propulsion class. Values are
/// raw percentages (100 = normal, 150 = 50% faster).
#[derive(Debug, Clone)]
pub struct TerrainTable {
    /// `[terrain_type_id][propulsion_class_index]` indexed as raw percent.
    /// Terrain ids 0-11 match `wz_maplib::terrain_types::TerrainType`;
    /// propulsion indices match `PropulsionClass` discriminants.
    speed_factors: [[u16; PROPULSION_CLASS_COUNT]; TERRAIN_TYPE_COUNT],
}

impl TerrainTable {
    /// Speed factor as a raw percentage (100 = normal, 60 = slow, 150 = fast).
    pub fn speed_factor(&self, terrain_type: u16, propulsion: PropulsionClass) -> u16 {
        let tt = (terrain_type as usize).min(TERRAIN_TYPE_COUNT - 1);
        self.speed_factors[tt][propulsion as usize]
    }

    /// Speed factors for one propulsion class normalized to floats
    /// (100 -> 1.0). Suitable for direct GPU uniform upload.
    pub fn speed_column(&self, propulsion: PropulsionClass) -> [f32; TERRAIN_TYPE_COUNT] {
        let col = propulsion as usize;
        let mut out = [0.0_f32; TERRAIN_TYPE_COUNT];
        for (i, row) in self.speed_factors.iter().enumerate() {
            out[i] = f32::from(row[col]) / 100.0;
        }
        out
    }
}

#[derive(Deserialize)]
struct RawTerrainEntry {
    id: u16,
    #[serde(rename = "speedFactor")]
    speed_factor: RawSpeedFactor,
}

#[derive(Deserialize)]
struct RawSpeedFactor {
    wheeled: u16,
    tracked: u16,
    legged: u16,
    hover: u16,
    lift: u16,
    propellor: u16,
    #[serde(rename = "half-tracked")]
    half_tracked: u16,
}

/// Parse `terraintable.json` into a `TerrainTable`.
pub fn load_terrain_table(json_str: &str) -> Result<TerrainTable, crate::StatsError> {
    let raw: HashMap<String, RawTerrainEntry> =
        serde_json::from_str(json_str).map_err(|e| crate::StatsError::Parse {
            file: "terraintable.json".to_string(),
            source: e,
        })?;

    // Default to 100% (normal speed) for any terrain type missing from the file.
    let mut speed_factors = [[100_u16; PROPULSION_CLASS_COUNT]; TERRAIN_TYPE_COUNT];

    for (name, entry) in &raw {
        let id = entry.id as usize;
        if id >= TERRAIN_TYPE_COUNT {
            log::warn!(
                "Terrain table entry '{name}' has id {id} >= {TERRAIN_TYPE_COUNT}, skipping"
            );
            continue;
        }
        let sf = &entry.speed_factor;
        speed_factors[id] = [
            sf.wheeled,
            sf.tracked,
            sf.half_tracked,
            sf.hover,
            sf.legged,
            sf.lift,
            sf.propellor,
        ];
    }

    log::info!("Loaded terrain speed table ({} entries)", raw.len());
    Ok(TerrainTable { speed_factors })
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_JSON: &str = r#"{
        "sand": {
            "id": 0,
            "comment": "Sand",
            "speedFactor": {
                "wheeled": 100, "tracked": 100, "legged": 100,
                "hover": 150, "lift": 250, "propellor": 100,
                "half-tracked": 100
            }
        },
        "water": {
            "id": 7,
            "comment": "Water",
            "speedFactor": {
                "wheeled": 60, "tracked": 60, "legged": 60,
                "hover": 150, "lift": 250, "propellor": 100,
                "half-tracked": 60
            }
        }
    }"#;

    #[test]
    fn parse_terrain_table() {
        let table = load_terrain_table(TEST_JSON).unwrap();
        assert_eq!(table.speed_factor(0, PropulsionClass::Wheeled), 100);
        assert_eq!(table.speed_factor(0, PropulsionClass::Hover), 150);
        assert_eq!(table.speed_factor(7, PropulsionClass::Wheeled), 60);
        assert_eq!(table.speed_factor(7, PropulsionClass::Hover), 150);
    }

    #[test]
    fn speed_column_normalized() {
        let table = load_terrain_table(TEST_JSON).unwrap();
        let col = table.speed_column(PropulsionClass::Wheeled);
        assert!((col[0] - 1.0).abs() < f32::EPSILON); // Sand = 100%
        assert!((col[7] - 0.6).abs() < f32::EPSILON); // Water = 60%
    }
}
