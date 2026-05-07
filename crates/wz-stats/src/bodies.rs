//! Body stat definitions from body.json.

use std::collections::HashMap;

use serde::Deserialize;

/// Per-propulsion extra model entry (left/right PIE models).
#[derive(Debug, Clone, Deserialize)]
pub struct PropExtraModel {
    #[serde(default)]
    pub left: Option<String>,
    #[serde(default)]
    pub right: Option<String>,
}

/// Stats for a droid body type.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BodyStats {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub hitpoints: Option<u32>,
    #[serde(default)]
    pub weight: Option<u32>,
    #[serde(default)]
    pub size: Option<String>,
    #[serde(default, rename = "buildPower")]
    pub build_power: Option<u32>,
    #[serde(default, rename = "armourKinetic")]
    pub armour_kinetic: Option<u32>,
    #[serde(default, rename = "armourHeat")]
    pub armour_heat: Option<u32>,
    /// Turret slot count (0-3). Absent means 1 in WZ2100.
    #[serde(default, rename = "weaponSlots")]
    pub weapon_slots: Option<u8>,
    /// "Droids", "Cyborgs", "Transports", or "Babas" (scavengers). WZ2100
    /// stores this under the JSON key `class` but exposes it as `bodyClass`.
    #[serde(default, rename = "class")]
    pub body_class: Option<String>,
    /// Player-buildable when truthy. WZ2100 stores `0` / `1`; missing or zero
    /// marks scavenger and AI-only bodies.
    #[serde(default, deserialize_with = "deserialize_bool_int")]
    pub designable: bool,
    /// `Cyborg` / `SuperCyborg` marker for cyborg-only components.
    #[serde(default, rename = "usageClass")]
    pub usage_class: Option<String>,
    /// Body-specific propulsion PIE models keyed by propulsion stat name.
    #[serde(default, rename = "propulsionExtraModels")]
    pub propulsion_extra_models: HashMap<String, PropExtraModel>,
}

/// Accepts WZ2100's `0` / `1` integers as well as JSON booleans.
pub(crate) fn deserialize_bool_int<'de, D>(de: D) -> Result<bool, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::Deserialize;
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum BoolOrInt {
        Bool(bool),
        Int(i64),
    }
    Ok(match BoolOrInt::deserialize(de)? {
        BoolOrInt::Bool(b) => b,
        BoolOrInt::Int(i) => i != 0,
    })
}

impl BodyStats {
    /// Turret slot count, clamped to 3 and defaulting to 1 to match WZ2100.
    pub fn weapon_slot_count(&self) -> u8 {
        self.weapon_slots.unwrap_or(1).min(3)
    }

    pub fn is_cyborg(&self) -> bool {
        self.body_class.as_deref() == Some("Cyborgs")
            || self.size.as_deref().map(str::to_uppercase).as_deref() == Some("CYBORG")
    }

    pub fn is_super_cyborg(&self) -> bool {
        self.usage_class.as_deref() == Some("SuperCyborg")
    }

    pub fn is_transporter(&self) -> bool {
        self.body_class.as_deref() == Some("Transports")
    }

    /// Scavenger (Babas) chassis. These are never player-buildable.
    pub fn is_scavenger(&self) -> bool {
        self.body_class.as_deref() == Some("Babas")
    }
}

impl BodyStats {
    pub fn pie_model(&self) -> Option<&str> {
        self.model.as_deref()
    }

    pub fn propulsion_model(&self, propulsion_name: &str) -> Option<&str> {
        self.propulsion_extra_models
            .get(propulsion_name)
            .and_then(|e| e.left.as_deref())
    }
}

/// Load body stats from a body.json document.
pub fn load_bodies(json_str: &str) -> Result<HashMap<String, BodyStats>, crate::StatsError> {
    crate::loaders::load_stat_map(json_str, "body.json", |key, stats: &mut BodyStats| {
        if stats.id.is_empty() {
            stats.id = key.to_string();
        }
    })
}
