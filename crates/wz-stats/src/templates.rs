//! Droid template definitions from templates.json.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// A droid template that assembles body + propulsion + weapons.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplateStats {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub id: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub body: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub propulsion: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub weapons: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, rename = "type", skip_serializing_if = "Option::is_none")]
    pub droid_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub construct: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sensor: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repair: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ecm: Option<String>,
    /// Commander brain. References a weapon via `brain.turret`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub brain: Option<String>,
}

impl TemplateStats {
    /// Falls back to `id` when `name` is not set.
    pub fn display_name(&self) -> &str {
        self.name.as_deref().unwrap_or(&self.id)
    }
}

/// Load template stats from a templates.json document.
pub fn load_templates(json_str: &str) -> Result<HashMap<String, TemplateStats>, crate::StatsError> {
    crate::loaders::load_stat_map(
        json_str,
        "templates.json",
        |key, stats: &mut TemplateStats| {
            if stats.id.is_empty() {
                stats.id = key.to_string();
            }
        },
    )
}

/// Serialize templates to the WZ2100 templates.json schema (a JSON object
/// keyed by template id). Any `id` field on the template is skipped in the
/// emitted body because WZ2100's loader uses the outer JSON key as the
/// canonical id.
#[expect(clippy::implicit_hasher, reason = "only called with std HashMap")]
pub fn serialize_templates(
    templates: &HashMap<String, TemplateStats>,
) -> Result<String, serde_json::Error> {
    // BTreeMap gives deterministic alphabetical output, matching how WZ2100
    // itself emits templates.json.
    let sorted: std::collections::BTreeMap<&str, &TemplateStats> =
        templates.iter().map(|(k, v)| (k.as_str(), v)).collect();
    serde_json::to_string_pretty(&sorted)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_preserves_fields() {
        let mut stats = TemplateStats {
            id: "MyCustom".into(),
            body: "Body1REC".into(),
            propulsion: "HoverRotary".into(),
            weapons: vec!["MG1Mk1".into(), "Cannon1Mk1".into()],
            name: Some("MyCustom".into()),
            droid_type: Some("WEAPON".into()),
            construct: None,
            sensor: None,
            repair: None,
            ecm: None,
            brain: None,
        };
        let mut map = HashMap::new();
        map.insert("MyCustom".to_string(), stats.clone());

        let json = serialize_templates(&map).unwrap();
        let back = load_templates(&json).unwrap();
        let parsed = back.get("MyCustom").expect("round-trip preserves key");
        stats.id = "MyCustom".into();
        assert_eq!(parsed.body, stats.body);
        assert_eq!(parsed.propulsion, stats.propulsion);
        assert_eq!(parsed.weapons, stats.weapons);
        assert_eq!(parsed.droid_type, stats.droid_type);
    }

    #[test]
    fn optional_turrets_round_trip() {
        let stats = TemplateStats {
            id: "BuilderDroid".into(),
            body: "Body1REC".into(),
            propulsion: "wheeled01".into(),
            weapons: vec![],
            name: Some("BuilderDroid".into()),
            droid_type: Some("CONSTRUCT".into()),
            construct: Some("Spade1Mk1".into()),
            sensor: None,
            repair: None,
            ecm: None,
            brain: None,
        };
        let mut map = HashMap::new();
        map.insert("BuilderDroid".to_string(), stats);

        let json = serialize_templates(&map).unwrap();
        let back = load_templates(&json).unwrap();
        let parsed = back.get("BuilderDroid").unwrap();
        assert_eq!(parsed.construct.as_deref(), Some("Spade1Mk1"));
        assert!(parsed.weapons.is_empty());
    }
}
