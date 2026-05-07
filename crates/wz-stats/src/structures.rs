//! Structure stat definitions from structure.json.

use serde::Deserialize;
use std::collections::HashMap;

/// Stats for a structure type (from structure.json).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
#[derive(Default)]
pub struct StructureStats {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(rename = "type")]
    pub structure_type: Option<String>,
    #[serde(default)]
    pub strength: Option<String>,
    #[serde(default)]
    pub width: Option<u32>,
    #[serde(default)]
    pub breadth: Option<u32>,
    #[serde(default)]
    pub hitpoints: Option<u32>,
    #[serde(default)]
    pub armour: Option<u32>,
    #[serde(default)]
    pub thermal: Option<u32>,
    #[serde(default, rename = "sensorID")]
    pub sensor_id: Option<String>,
    #[serde(default, rename = "ecmID")]
    pub ecm_id: Option<String>,
    /// Weapon stat names mounted on this structure.
    #[serde(default)]
    pub weapons: Vec<String>,
    /// Snaps to and replaces a wall beneath it. Source of truth for
    /// "wall tower" and "hardpoint" detection.
    #[serde(default)]
    pub combines_with_wall: bool,
    /// PIE model names: `[level0, module, level0+module, ...]`.
    #[serde(default)]
    pub structure_model: Option<Vec<String>>,
    #[serde(default)]
    pub base_model: Option<String>,
    #[serde(default, rename = "imdName")]
    pub imd_name: Option<String>,
    #[serde(default, rename = "baseIMD")]
    pub base_imd: Option<String>,
}

impl StructureStats {
    pub fn pie_model(&self) -> Option<&str> {
        self.pie_model_for_modules(0)
    }

    /// PIE model for a given module count. Factories, power generators, and
    /// research facilities store multiple models in `structureModel`: index
    /// `modules * 2` is the combined model for that upgrade level, matching
    /// WZ2100's `capacity * 2`.
    pub fn pie_model_for_modules(&self, modules: u8) -> Option<&str> {
        if let Some(ref models) = self.structure_model {
            if modules > 0 && models.len() > 1 {
                let idx = (modules as usize * 2).min(models.len() - 1);
                return models.get(idx).map(String::as_str);
            }
            if let Some(first) = models.first() {
                return Some(first.as_str());
            }
        }
        self.imd_name.as_deref()
    }

    /// PIE model for a wall-variant index (0=straight, 1=cross, 2=T,
    /// 3=L-corner), matching `pIMD[wall.type]` in WZ2100 at
    /// `src/structure.cpp:1377` and `:1786`. Clamps to the last entry when
    /// `structureModel` has fewer models, matching how the game handles
    /// single-model wall stats like `BaBa` and `Tank Trap`.
    pub fn pie_model_for_wall_type(&self, wall_type: u8) -> Option<&str> {
        if let Some(ref models) = self.structure_model {
            if models.is_empty() {
                return self.imd_name.as_deref();
            }
            let idx = (wall_type as usize).min(models.len() - 1);
            return models.get(idx).map(String::as_str);
        }
        self.imd_name.as_deref()
    }
}

/// Load structure stats from a structure.json document keyed by structure id.
/// Underscore-prefixed metadata keys (e.g. `_config_`) are skipped.
pub fn load_structures(
    json_str: &str,
) -> Result<HashMap<String, StructureStats>, crate::StatsError> {
    crate::loaders::load_stat_map(
        json_str,
        "structure.json",
        |key, stats: &mut StructureStats| {
            if stats.id.is_empty() {
                stats.id = key.to_string();
            }
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stats_with_models(structure_type: &str, models: &[&str]) -> StructureStats {
        StructureStats {
            id: "test".into(),
            name: "Test".into(),
            structure_type: Some(structure_type.into()),
            structure_model: Some(models.iter().map(|s| (*s).into()).collect()),
            ..Default::default()
        }
    }

    #[test]
    fn factory_stat_uses_times_two_stride() {
        let s = stats_with_models(
            "FACTORY",
            &["base.pie", "mod.pie", "l1.pie", "mod2.pie", "l2.pie"],
        );
        assert_eq!(s.pie_model_for_modules(0), Some("base.pie"));
        assert_eq!(s.pie_model_for_modules(1), Some("l1.pie"));
        assert_eq!(s.pie_model_for_modules(2), Some("l2.pie"));
    }

    #[test]
    fn single_model_stat_returns_first_regardless_of_modules() {
        let s = stats_with_models("WALL", &["only.pie"]);
        assert_eq!(s.pie_model_for_modules(0), Some("only.pie"));
        assert_eq!(s.pie_model_for_modules(3), Some("only.pie"));
    }

    #[test]
    fn wall_type_indexes_structure_model_directly() {
        let s = stats_with_models(
            "WALL",
            &[
                "straight.pie",
                "cross.pie",
                "t_junction.pie",
                "l_corner.pie",
            ],
        );
        assert_eq!(s.pie_model_for_wall_type(0), Some("straight.pie"));
        assert_eq!(s.pie_model_for_wall_type(1), Some("cross.pie"));
        assert_eq!(s.pie_model_for_wall_type(2), Some("t_junction.pie"));
        assert_eq!(s.pie_model_for_wall_type(3), Some("l_corner.pie"));
    }

    #[test]
    fn wall_type_clamps_single_model_walls() {
        let s = stats_with_models("WALL", &["only.pie"]);
        for wt in 0u8..4 {
            assert_eq!(s.pie_model_for_wall_type(wt), Some("only.pie"));
        }
    }
}
