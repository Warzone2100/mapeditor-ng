//! Weapon stat definitions from weapons.json.

use std::collections::HashMap;

use serde::Deserialize;

/// Stats for a weapon type.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WeaponStats {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default, rename = "mountModel")]
    pub mount_model: Option<String>,
    #[serde(default)]
    pub short_range: Option<u32>,
    #[serde(default)]
    pub long_range: Option<u32>,
    #[serde(default)]
    pub damage: Option<u32>,
    /// Player-buildable when truthy. WZ2100 stores `0` / `1`.
    #[serde(default, deserialize_with = "crate::bodies::deserialize_bool_int")]
    pub designable: bool,
    /// `Cyborg` / `SuperCyborg` marker for cyborg-only weapons.
    #[serde(default, rename = "usageClass")]
    pub usage_class: Option<String>,
    /// Set (>0) on VTOL-only weapons (bombs, VTOL cannons). Ground weapons
    /// omit the field.
    #[serde(default, rename = "numAttackRuns")]
    pub num_attack_runs: Option<u32>,
}

impl WeaponStats {
    pub fn pie_model(&self) -> Option<&str> {
        self.model.as_deref()
    }

    /// VTOL-only weapon (requires VTOL propulsion).
    pub fn is_vtol(&self) -> bool {
        self.num_attack_runs.is_some_and(|n| n > 0)
    }
}

/// Load weapon stats from a weapons.json document.
pub fn load_weapons(json_str: &str) -> Result<HashMap<String, WeaponStats>, crate::StatsError> {
    crate::loaders::load_stat_map(json_str, "weapons.json", |key, stats: &mut WeaponStats| {
        if stats.id.is_empty() {
            stats.id = key.to_string();
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mount_model_deserialized() {
        let json = r#"{
            "RailGun1Mk1": {
                "id": "RailGun1Mk1",
                "model": "GNLGSS.PIE",
                "mountModel": "TRLGSS.PIE",
                "designable": 1,
                "weaponClass": "KINETIC"
            },
            "Mortar3ROTARYMk1": {
                "id": "Mortar3ROTARYMk1",
                "model": "GNHMORT.PIE",
                "mountModel": "TRHRMORT.PIE",
                "designable": 1,
                "weaponClass": "KINETIC"
            }
        }"#;
        let weapons = load_weapons(json).unwrap();
        let rg = weapons.get("RailGun1Mk1").unwrap();
        assert_eq!(rg.mount_model.as_deref(), Some("TRLGSS.PIE"));
        assert_eq!(rg.model.as_deref(), Some("GNLGSS.PIE"));

        let mortar = weapons.get("Mortar3ROTARYMk1").unwrap();
        assert_eq!(mortar.mount_model.as_deref(), Some("TRHRMORT.PIE"));
    }
}
