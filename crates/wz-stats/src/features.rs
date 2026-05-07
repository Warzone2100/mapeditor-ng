//! Feature stat definitions from features.json.

use serde::Deserialize;
use std::collections::HashMap;

/// Stats for a feature type (from features.json).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FeatureStats {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(rename = "type")]
    pub feature_type: Option<String>,
    #[serde(default)]
    pub hitpoints: Option<u32>,
    #[serde(default)]
    pub armour: Option<u32>,
    #[serde(default)]
    pub model: Option<String>,
    /// Legacy field; superseded by `model` in newer stats.
    #[serde(default, rename = "imdName")]
    pub imd_name: Option<String>,
    #[serde(default, rename = "baseIMD")]
    pub base_imd: Option<String>,
    #[serde(default)]
    pub line_of_sight: Option<u32>,
    #[serde(default)]
    pub start_visible: Option<u32>,
    #[serde(default)]
    pub width: Option<u32>,
    #[serde(default)]
    pub breadth: Option<u32>,
}

impl FeatureStats {
    /// Primary PIE model name, falling back to the legacy `imdName` field.
    pub fn pie_model(&self) -> Option<&str> {
        self.model.as_deref().or(self.imd_name.as_deref())
    }
}

/// Load feature stats from a features.json document keyed by feature id.
pub fn load_features(json_str: &str) -> Result<HashMap<String, FeatureStats>, crate::StatsError> {
    crate::loaders::load_stat_map(
        json_str,
        "features.json",
        |key, stats: &mut FeatureStats| {
            if stats.id.is_empty() {
                stats.id = key.to_string();
            }
        },
    )
}
