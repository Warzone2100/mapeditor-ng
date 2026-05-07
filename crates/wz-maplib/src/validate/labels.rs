//! Label validation: bounds, duplicates.

use std::collections::HashSet;

use crate::constants::TILE_UNITS;
use crate::io_wz::WzMap;
use crate::labels::ScriptLabel;

use super::push_warning;
use super::types::{
    IssueLocation, ValidationCategory, ValidationConfig, ValidationResults, WarningRule,
};

pub(super) fn validate_labels(
    map: &WzMap,
    config: &ValidationConfig,
    results: &mut ValidationResults,
) {
    let cat = ValidationCategory::Labels;
    let max_x = map.map_data.width * TILE_UNITS;
    let max_y = map.map_data.height * TILE_UNITS;
    let check_bounds = config.is_enabled(WarningRule::LabelOutOfBounds);
    let check_duplicates = config.is_enabled(WarningRule::DuplicateLabelNames);

    let mut seen_names: HashSet<&str> = HashSet::new();

    for (key, label) in &map.labels {
        if check_bounds {
            match label {
                ScriptLabel::Position { pos, .. } => {
                    if pos[0] >= max_x || pos[1] >= max_y {
                        push_warning(
                            results,
                            WarningRule::LabelOutOfBounds,
                            cat,
                            format!(
                                "Label \"{key}\" position ({}, {}) is out of map bounds.",
                                pos[0], pos[1]
                            ),
                            IssueLocation::WorldPos {
                                x: pos[0],
                                y: pos[1],
                            },
                        );
                    }
                }
                ScriptLabel::Area { pos1, pos2, .. } => {
                    if pos1[0] >= max_x || pos1[1] >= max_y || pos2[0] >= max_x || pos2[1] >= max_y
                    {
                        push_warning(
                            results,
                            WarningRule::LabelOutOfBounds,
                            cat,
                            format!(
                                "Label \"{key}\" area ({},{})-({},{}) extends beyond map bounds.",
                                pos1[0], pos1[1], pos2[0], pos2[1]
                            ),
                            IssueLocation::WorldPos {
                                x: pos1[0],
                                y: pos1[1],
                            },
                        );
                    }
                }
            }
        }

        if check_duplicates {
            let label_name = label.label();
            if !seen_names.insert(label_name) {
                push_warning(
                    results,
                    WarningRule::DuplicateLabelNames,
                    cat,
                    format!("Duplicate label name \"{label_name}\"."),
                    IssueLocation::None,
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::validate::test_support::valid_map;

    #[test]
    fn label_within_bounds_ok() {
        let mut map = valid_map(64, 64);
        let center = 32 * TILE_UNITS;
        map.labels.push((
            "start".to_string(),
            ScriptLabel::new_position("start".into(), center, center),
        ));
        let mut results = ValidationResults::default();
        validate_labels(&map, &ValidationConfig::default(), &mut results);
        assert!(results.issues.is_empty(), "issues: {:?}", results.issues);
    }

    #[test]
    fn position_label_out_of_bounds() {
        let mut map = valid_map(32, 32);
        let off = 33 * TILE_UNITS;
        map.labels.push((
            "bad".to_string(),
            ScriptLabel::new_position("bad".into(), off, off),
        ));
        let mut results = ValidationResults::default();
        validate_labels(&map, &ValidationConfig::default(), &mut results);
        assert!(
            results
                .issues
                .iter()
                .any(|i| i.message.contains("out of map bounds"))
        );
    }

    #[test]
    fn area_label_partially_out_of_bounds() {
        let mut map = valid_map(32, 32);
        let inside = 16 * TILE_UNITS;
        let outside = 33 * TILE_UNITS;
        map.labels.push((
            "zone".to_string(),
            ScriptLabel::new_area("zone".into(), inside, inside, outside, outside),
        ));
        let mut results = ValidationResults::default();
        validate_labels(&map, &ValidationConfig::default(), &mut results);
        assert!(
            results
                .issues
                .iter()
                .any(|i| i.message.contains("extends beyond"))
        );
    }

    #[test]
    fn duplicate_label_names_warns() {
        let mut map = valid_map(64, 64);
        let center = 32 * TILE_UNITS;
        map.labels.push((
            "start_a".to_string(),
            ScriptLabel::new_position("start".into(), center, center),
        ));
        map.labels.push((
            "start_b".to_string(),
            ScriptLabel::new_position("start".into(), center + TILE_UNITS, center),
        ));
        let mut results = ValidationResults::default();
        validate_labels(&map, &ValidationConfig::default(), &mut results);
        assert!(
            results
                .issues
                .iter()
                .any(|i| i.message.contains("Duplicate label"))
        );
    }

    #[test]
    fn unique_label_names_ok() {
        let mut map = valid_map(64, 64);
        let center = 32 * TILE_UNITS;
        map.labels.push((
            "start".to_string(),
            ScriptLabel::new_position("start".into(), center, center),
        ));
        map.labels.push((
            "end".to_string(),
            ScriptLabel::new_position("end".into(), center + TILE_UNITS, center),
        ));
        let mut results = ValidationResults::default();
        validate_labels(&map, &ValidationConfig::default(), &mut results);
        assert!(
            !results
                .issues
                .iter()
                .any(|i| i.message.contains("Duplicate")),
            "issues: {:?}",
            results.issues
        );
    }

    #[test]
    fn label_at_map_boundary_ok() {
        let mut map = valid_map(32, 32);
        let max_valid = 32 * TILE_UNITS - 1;
        map.labels.push((
            "edge".to_string(),
            ScriptLabel::new_position("edge".into(), max_valid, max_valid),
        ));
        let mut results = ValidationResults::default();
        validate_labels(&map, &ValidationConfig::default(), &mut results);
        assert!(
            !results
                .issues
                .iter()
                .any(|i| i.message.contains("out of map")),
            "issues: {:?}",
            results.issues
        );
    }
}
