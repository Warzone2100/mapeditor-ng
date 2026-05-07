//! Map-level validation: tileset, dimensions, name.

use crate::constants::{MAP_MAX_HEIGHT, MAP_MAX_WIDTH, MAP_MAX_WZ_EXPORT, MAP_NAME_MAX_LEN};
use crate::io_wz::WzMap;

use super::helpers::strip_player_prefix;
use super::types::{IssueLocation, ValidationCategory, ValidationConfig, WarningRule};
use super::{push_problem, push_warning};

pub(super) fn validate_map_level(
    map: &WzMap,
    config: &ValidationConfig,
    results: &mut super::types::ValidationResults,
) {
    let cat = ValidationCategory::Map;

    if map.terrain_types.is_none() {
        push_problem(
            results,
            cat,
            "No terrain type data (TTP) loaded. The game requires terrain types.".into(),
            IssueLocation::None,
        );
    }

    let w = map.map_data.width;
    let h = map.map_data.height;

    if w < 2 || h < 2 {
        push_problem(
            results,
            cat,
            format!("Map dimensions {w}x{h} are too small. Minimum is 2x2."),
            IssueLocation::None,
        );
    }
    if w > MAP_MAX_WIDTH || h > MAP_MAX_HEIGHT {
        push_problem(
            results,
            cat,
            format!(
                "Map dimensions {w}x{h} exceed absolute maximum {MAP_MAX_WIDTH}x{MAP_MAX_HEIGHT}."
            ),
            IssueLocation::None,
        );
    }

    if config.is_enabled(WarningRule::WzExportDimensionLimit)
        && (w > MAP_MAX_WZ_EXPORT || h > MAP_MAX_WZ_EXPORT)
        && w <= MAP_MAX_WIDTH
        && h <= MAP_MAX_HEIGHT
    {
        push_warning(
            results,
            WarningRule::WzExportDimensionLimit,
            cat,
            format!(
                "Map dimensions {w}x{h} exceed the .wz export limit of {MAP_MAX_WZ_EXPORT}x{MAP_MAX_WZ_EXPORT}."
            ),
            IssueLocation::None,
        );
    }

    // Strip the "Nc-" player-count prefix so name rules apply to the base
    // (e.g. "2c-Roughness" -> "Roughness").
    let raw_name = &map.map_name;
    let base_name = strip_player_prefix(raw_name);
    if base_name.is_empty() {
        if config.is_enabled(WarningRule::MapNameEmpty) {
            push_warning(
                results,
                WarningRule::MapNameEmpty,
                cat,
                "Map name is empty.".into(),
                IssueLocation::None,
            );
        }
    } else {
        if config.is_enabled(WarningRule::MapNameTooLong) && base_name.len() > MAP_NAME_MAX_LEN {
            push_warning(
                results,
                WarningRule::MapNameTooLong,
                cat,
                format!(
                    "Map base name \"{}\" is {} characters (max {MAP_NAME_MAX_LEN}).",
                    base_name,
                    base_name.len()
                ),
                IssueLocation::None,
            );
        }
        if config.is_enabled(WarningRule::MapNameInvalidStart)
            && !base_name.starts_with(|c: char| c.is_ascii_alphabetic())
        {
            push_warning(
                results,
                WarningRule::MapNameInvalidStart,
                cat,
                format!("Map base name \"{base_name}\" must start with a letter."),
                IssueLocation::None,
            );
        }
        if config.is_enabled(WarningRule::MapNameInvalidChars)
            && !base_name
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
        {
            push_warning(
                results,
                WarningRule::MapNameInvalidChars,
                cat,
                format!(
                    "Map base name \"{base_name}\" contains invalid characters. Only letters, digits, underscores, and hyphens are allowed.",
                ),
                IssueLocation::None,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constants::TILE_UNITS;
    use crate::map_data::Gateway;
    use crate::validate::test_support::*;
    use crate::validate::validate_map;

    #[test]
    fn valid_empty_map_no_issues() {
        let map = valid_map(64, 64);
        let r = validate_map(&map, None, &ValidationConfig::default());
        assert_eq!(r.issues.len(), 0);
    }

    #[test]
    fn valid_populated_map_no_issues() {
        let mut map = valid_map(64, 64);
        map.players = 4;
        let center = 32 * TILE_UNITS + 64; // well inside the map
        map.structures
            .push(make_structure("A0PowerGenerator", center, center, 0));
        map.droids
            .push(make_droid("ConstructorDroid", center, center, 0));
        map.features
            .push(make_feature("OilResource", center + 128, center));
        map.map_data.gateways.push(Gateway {
            x1: 10,
            y1: 10,
            x2: 10,
            y2: 20,
        });
        let r = validate_map(&map, None, &ValidationConfig::default());
        assert_eq!(r.issues.len(), 0, "issues: {:?}", r.issues);
    }

    #[test]
    fn map_no_terrain_types_is_problem() {
        use crate::validate::types::Severity;
        let mut map = WzMap::new("TestMap", 64, 64);
        map.terrain_types = None;
        let r = validate_map(&map, None, &ValidationConfig::default());
        assert!(r.has_problems());
        assert!(r.issues.iter().any(|i| i.severity == Severity::Problem
            && i.category == ValidationCategory::Map
            && i.message.contains("terrain type")));
    }

    #[test]
    fn map_dimensions_at_wz_limit_ok() {
        let map = valid_map(250, 250);
        let mut results = super::super::types::ValidationResults::default();
        validate_map_level(&map, &ValidationConfig::default(), &mut results);
        assert!(
            !results
                .issues
                .iter()
                .any(|i| i.message.contains("dimensions")),
            "issues: {:?}",
            results.issues
        );
    }

    #[test]
    fn map_dimensions_exceed_wz_limit_warns() {
        let map = valid_map(251, 100);
        let mut results = super::super::types::ValidationResults::default();
        validate_map_level(&map, &ValidationConfig::default(), &mut results);
        assert!(
            results
                .issues
                .iter()
                .any(|i| i.severity == super::super::types::Severity::Warning
                    && i.message.contains("export limit"))
        );
    }

    #[test]
    fn map_dimensions_both_exceed_wz_limit() {
        let map = valid_map(251, 251);
        let mut results = super::super::types::ValidationResults::default();
        validate_map_level(&map, &ValidationConfig::default(), &mut results);
        assert!(
            results
                .issues
                .iter()
                .any(|i| i.severity == super::super::types::Severity::Warning
                    && i.message.contains("export limit"))
        );
    }

    #[test]
    fn map_dimensions_at_absolute_max_ok() {
        let map = valid_map(256, 256);
        let mut results = super::super::types::ValidationResults::default();
        validate_map_level(&map, &ValidationConfig::default(), &mut results);
        assert!(
            !results
                .issues
                .iter()
                .any(|i| i.severity == super::super::types::Severity::Problem
                    && i.message.contains("absolute maximum"))
        );
    }

    #[test]
    fn map_dimensions_exceed_absolute_max() {
        let map = valid_map(257, 257);
        let mut results = super::super::types::ValidationResults::default();
        validate_map_level(&map, &ValidationConfig::default(), &mut results);
        assert!(
            results
                .issues
                .iter()
                .any(|i| i.severity == super::super::types::Severity::Problem
                    && i.message.contains("exceed absolute maximum"))
        );
    }

    #[test]
    fn map_dimensions_too_small() {
        let map = valid_map(1, 1);
        let mut results = super::super::types::ValidationResults::default();
        validate_map_level(&map, &ValidationConfig::default(), &mut results);
        assert!(
            results
                .issues
                .iter()
                .any(|i| i.severity == super::super::types::Severity::Problem
                    && i.message.contains("too small"))
        );
    }

    #[test]
    fn map_name_empty_warns() {
        let mut map = valid_map(64, 64);
        map.map_name = String::new();
        let mut results = super::super::types::ValidationResults::default();
        validate_map_level(&map, &ValidationConfig::default(), &mut results);
        assert!(
            results
                .issues
                .iter()
                .any(|i| i.message.contains("name is empty"))
        );
    }

    #[test]
    fn map_name_valid_chars() {
        let mut map = valid_map(64, 64);
        map.map_name = "MyMap-01_test".to_string();
        let mut results = super::super::types::ValidationResults::default();
        validate_map_level(&map, &ValidationConfig::default(), &mut results);
        assert!(
            !results
                .issues
                .iter()
                .any(|i| i.message.contains("base name")),
            "issues: {:?}",
            results.issues
        );
    }

    #[test]
    fn map_name_with_player_prefix_ok() {
        let mut map = valid_map(64, 64);
        map.map_name = "2c-Roughness".to_string();
        let mut results = super::super::types::ValidationResults::default();
        validate_map_level(&map, &ValidationConfig::default(), &mut results);
        assert!(
            !results
                .issues
                .iter()
                .any(|i| i.message.contains("start with a letter")),
            "issues: {:?}",
            results.issues
        );
    }

    #[test]
    fn map_name_with_ten_player_prefix_ok() {
        let mut map = valid_map(64, 64);
        map.map_name = "10c-WaterLoop".to_string();
        let mut results = super::super::types::ValidationResults::default();
        validate_map_level(&map, &ValidationConfig::default(), &mut results);
        assert!(
            !results
                .issues
                .iter()
                .any(|i| i.message.contains("start with a letter")),
            "issues: {:?}",
            results.issues
        );
    }

    #[test]
    fn map_name_starts_with_digit_no_prefix_warns() {
        let mut map = valid_map(64, 64);
        // Not a valid "Nc-" prefix; just starts with a digit.
        map.map_name = "1BadName".to_string();
        let mut results = super::super::types::ValidationResults::default();
        validate_map_level(&map, &ValidationConfig::default(), &mut results);
        assert!(
            results
                .issues
                .iter()
                .any(|i| i.message.contains("start with a letter"))
        );
    }

    #[test]
    fn map_name_special_chars_warns() {
        let mut map = valid_map(64, 64);
        map.map_name = "My Map!".to_string();
        let mut results = super::super::types::ValidationResults::default();
        validate_map_level(&map, &ValidationConfig::default(), &mut results);
        assert!(
            results
                .issues
                .iter()
                .any(|i| i.message.contains("invalid characters"))
        );
    }

    #[test]
    fn map_name_too_long_warns() {
        let mut map = valid_map(64, 64);
        map.map_name = "A".repeat(17);
        let mut results = super::super::types::ValidationResults::default();
        validate_map_level(&map, &ValidationConfig::default(), &mut results);
        assert!(
            results
                .issues
                .iter()
                .any(|i| i.message.contains("17 characters"))
        );
    }

    #[test]
    fn map_name_exactly_max_length_ok() {
        let mut map = valid_map(64, 64);
        map.map_name = "A".repeat(MAP_NAME_MAX_LEN);
        let mut results = super::super::types::ValidationResults::default();
        validate_map_level(&map, &ValidationConfig::default(), &mut results);
        assert!(
            !results
                .issues
                .iter()
                .any(|i| i.message.contains("characters")),
            "issues: {:?}",
            results.issues
        );
    }
}
