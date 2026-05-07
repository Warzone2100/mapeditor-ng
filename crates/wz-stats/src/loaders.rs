//! Shared deserialization for the per-file JSON stat maps.
//!
//! Every stats file (`body.json`, `structure.json`, ...) is a JSON object
//! keyed by stat id, with optional underscore-prefixed metadata entries
//! that must be skipped. [`load_stat_map`] centralises that pattern so
//! the per-stat modules only define their structs and a one-line wrapper.

use std::collections::HashMap;

use serde::de::DeserializeOwned;

use crate::StatsError;

/// Parse a JSON stats map, skipping `_`-prefixed metadata keys and per-entry parse errors.
///
/// `file_name` is the source file (e.g. `"body.json"`); it appears in
/// [`StatsError::Parse`] and in skip warnings logged for malformed entries.
/// `post` runs after a value is successfully deserialized and is the seam
/// stat types use to back-fill an `id` field from the JSON key.
///
/// # Errors
/// Returns [`StatsError::Parse`] if the top-level JSON is not an object.
/// Per-entry parse failures are logged and the entry is dropped.
pub(crate) fn load_stat_map<T, F>(
    json_str: &str,
    file_name: &str,
    mut post: F,
) -> Result<HashMap<String, T>, StatsError>
where
    T: DeserializeOwned,
    F: FnMut(&str, &mut T),
{
    let raw: HashMap<String, serde_json::Value> =
        serde_json::from_str(json_str).map_err(|e| StatsError::Parse {
            file: file_name.to_string(),
            source: e,
        })?;
    let mut result = HashMap::with_capacity(raw.len());
    for (key, value) in raw {
        if key.starts_with('_') {
            continue;
        }
        match serde_json::from_value::<T>(value) {
            Ok(mut stats) => {
                post(&key, &mut stats);
                result.insert(key, stats);
            }
            Err(e) => {
                log::warn!("Skipping {file_name} '{key}': {e}");
            }
        }
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    use serde::Deserialize;

    #[derive(Debug, Deserialize, PartialEq, Eq)]
    struct Toy {
        #[serde(default)]
        id: String,
        value: i32,
    }

    #[test]
    fn skips_underscore_keys() {
        let json = r#"{"_meta": {"value": 99}, "real": {"value": 1}}"#;
        let out: HashMap<String, Toy> = load_stat_map(json, "toy.json", |_, _| {}).unwrap();
        assert!(out.contains_key("real"));
        assert!(!out.contains_key("_meta"));
    }

    #[test]
    fn skips_malformed_entries_but_keeps_others() {
        let json = r#"{"good": {"value": 1}, "bad": {"value": "nope"}}"#;
        let out: HashMap<String, Toy> = load_stat_map(json, "toy.json", |_, _| {}).unwrap();
        assert_eq!(out.len(), 1);
        assert!(out.contains_key("good"));
    }

    #[test]
    fn post_callback_runs_after_parse() {
        let json = r#"{"alpha": {"value": 1}}"#;
        let out: HashMap<String, Toy> = load_stat_map(json, "toy.json", |key, stats: &mut Toy| {
            if stats.id.is_empty() {
                stats.id = key.to_string();
            }
        })
        .unwrap();
        assert_eq!(out.get("alpha").unwrap().id, "alpha");
    }

    #[test]
    fn top_level_parse_error_propagates() {
        let err = load_stat_map::<Toy, _>("not an object", "toy.json", |_, _| {}).unwrap_err();
        match err {
            StatsError::Parse { file, .. } => assert_eq!(file, "toy.json"),
            StatsError::Io { .. } => panic!("expected Parse error, got Io"),
        }
    }
}
