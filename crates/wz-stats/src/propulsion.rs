//! Propulsion stat definitions from propulsion.json.

use std::collections::HashMap;

use serde::Deserialize;

/// Stats for a propulsion type.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PropulsionStats {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub speed: Option<u32>,
    #[serde(default)]
    pub weight: Option<u32>,
    #[serde(default, rename = "type")]
    pub propulsion_type: Option<String>,
    /// Player-buildable when truthy. WZ2100 stores `0` / `1`.
    #[serde(default, deserialize_with = "crate::bodies::deserialize_bool_int")]
    pub designable: bool,
    /// `Cyborg` / `SuperCyborg` marker for cyborg-only legged propulsion.
    #[serde(default, rename = "usageClass")]
    pub usage_class: Option<String>,
}

impl PropulsionStats {
    pub fn pie_model(&self) -> Option<&str> {
        self.model.as_deref()
    }
}

/// Load propulsion stats from a propulsion.json document.
pub fn load_propulsion(
    json_str: &str,
) -> Result<HashMap<String, PropulsionStats>, crate::StatsError> {
    crate::loaders::load_stat_map(
        json_str,
        "propulsion.json",
        |key, stats: &mut PropulsionStats| {
            if stats.id.is_empty() {
                stats.id = key.to_string();
            }
        },
    )
}
