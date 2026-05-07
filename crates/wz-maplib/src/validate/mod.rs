//! Map validation engine for pre-export checks.
//!
//! Validates a [`WzMap`] for issues that would cause problems in-game:
//! invalid positions, overlapping structures, missing objects, gateway errors, etc.
//!
//! The validation engine is pure logic with no GUI dependency. It takes an optional
//! [`StatsLookup`] trait object to check object names against the game stats database
//! without coupling this crate to `wz-stats`.

mod gateways;
mod helpers;
mod labels;
mod map_level;
mod multiplayer;
mod object_data;
mod object_positions;
mod terrain;
mod types;

#[cfg(test)]
mod test_support;

pub use helpers::{is_wall_or_defense, structure_packability};
pub use types::{
    FeatureInfo, IssueLocation, Severity, StatsLookup, StructureInfo, TemplateInfo,
    ValidationCategory, ValidationConfig, ValidationIssue, ValidationResults, WarningRule,
};

use crate::io_wz::WzMap;

/// Run all validation checks on a map.
///
/// Pass `stats` to enable checks against the game stats database (object name
/// lookups, structure types, template validation). Without stats, those checks
/// are skipped.
///
/// The `config` controls which warnings are enabled. Pass `&ValidationConfig::default()`
/// to enable all warnings.
#[must_use]
pub fn validate_map(
    map: &WzMap,
    stats: Option<&dyn StatsLookup>,
    config: &ValidationConfig,
) -> ValidationResults {
    let mut results = ValidationResults::default();
    map_level::validate_map_level(map, config, &mut results);
    terrain::validate_terrain(map, config, &mut results);
    object_positions::validate_object_positions(map, stats, config, &mut results);
    object_data::validate_object_data(map, stats, config, &mut results);
    multiplayer::validate_multiplayer(map, stats, config, &mut results);
    gateways::validate_gateways(map, config, &mut results);
    labels::validate_labels(map, config, &mut results);
    results
}

fn push_problem(
    results: &mut ValidationResults,
    category: ValidationCategory,
    message: String,
    location: IssueLocation,
) {
    results.issues.push(ValidationIssue {
        severity: Severity::Problem,
        category,
        message,
        location,
        rule: None,
    });
}

fn push_warning(
    results: &mut ValidationResults,
    rule: WarningRule,
    category: ValidationCategory,
    message: String,
    location: IssueLocation,
) {
    results.issues.push(ValidationIssue {
        severity: Severity::Warning,
        category,
        message,
        location,
        rule: Some(rule),
    });
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;
    use crate::constants::TILE_UNITS;
    use crate::map_data::Gateway;
    use test_support::*;

    #[test]
    fn validate_map_runs_all_categories() {
        let mut map = WzMap::new("", 1, 1); // too small, empty name, no TTP
        map.terrain_types = None;
        map.players = 2;
        map.structures
            .push(make_structure("Fake", 99999, 99999, 20));
        map.map_data.gateways.push(Gateway {
            x1: 200,
            y1: 200,
            x2: 200,
            y2: 200,
        });
        map.labels.push((
            "a".to_string(),
            crate::labels::ScriptLabel::new_position("dup".into(), 99999, 99999),
        ));
        map.labels.push((
            "b".to_string(),
            crate::labels::ScriptLabel::new_position("dup".into(), 99999, 99999),
        ));

        let r = validate_map(&map, None, &ValidationConfig::default());
        assert!(r.has_problems());
        let cats: HashSet<ValidationCategory> = r.issues.iter().map(|i| i.category).collect();
        assert!(cats.contains(&ValidationCategory::Map));
        assert!(cats.contains(&ValidationCategory::ObjectPositions));
        assert!(cats.contains(&ValidationCategory::ObjectData));
        assert!(cats.contains(&ValidationCategory::Gateways));
        assert!(cats.contains(&ValidationCategory::Labels));
    }

    #[test]
    fn validate_map_empty_map_minimal_issues() {
        let mut map = WzMap::new("TestMap", 64, 64);
        map.terrain_types = None;
        let r = validate_map(&map, None, &ValidationConfig::default());
        assert_eq!(r.problem_count(), 1);
        assert_eq!(r.warning_count(), 0);
    }

    #[test]
    fn validate_map_with_stats_catches_more() {
        let mut map = valid_map(64, 64);
        let center = 32 * TILE_UNITS;
        map.structures
            .push(make_structure("FakeBuilding", center, center, 0));

        let r_no_stats = validate_map(&map, None, &ValidationConfig::default());
        assert!(
            !r_no_stats
                .issues
                .iter()
                .any(|i| i.message.contains("not found"))
        );

        let stats = MockStats::new();
        let r_with_stats = validate_map(&map, Some(&stats), &ValidationConfig::default());
        assert!(
            r_with_stats
                .issues
                .iter()
                .any(|i| i.message.contains("not found"))
        );
    }

    #[test]
    fn validate_map_no_false_positives_on_stock_map() {
        let mut map = valid_map(64, 64);
        map.players = 4;
        map.map_name = "TestMap".to_string();

        let stats = MockStats::new()
            .with_structure("A0CommandCentre", "HQ", 2, 2)
            .with_structure("A0PowerGenerator", "POWER GENERATOR", 1, 1)
            .with_structure("A0ResearchFacility", "RESEARCH", 1, 1)
            .with_structure("A0LightFactory", "FACTORY", 3, 3)
            .with_feature("OilResource", "OIL RESOURCE")
            .with_template("ConstructorDroid", "DROID", true)
            .with_template("ViperMk1", "DROID", false);

        let center = 32 * TILE_UNITS;
        for p in 0..4i8 {
            let offset = (p as u32) * TILE_UNITS * 6;
            map.structures.push(make_structure_with_id(
                "A0CommandCentre",
                center + offset,
                center,
                p,
                (p as u32 + 1) * 100,
            ));
            map.structures.push(make_structure_with_id(
                "A0PowerGenerator",
                center + offset + TILE_UNITS * 2,
                center,
                p,
                (p as u32 + 1) * 100 + 1,
            ));
            map.droids.push(make_droid_with_id(
                "ConstructorDroid",
                center + offset,
                center + TILE_UNITS * 3,
                p,
                (p as u32 + 1) * 100 + 50,
            ));
        }
        map.features.push(make_feature_with_id(
            "OilResource",
            center,
            center + TILE_UNITS * 6,
            900,
        ));
        map.map_data.gateways.push(Gateway {
            x1: 10,
            y1: 10,
            x2: 10,
            y2: 20,
        });

        let r = validate_map(&map, Some(&stats), &ValidationConfig::default());
        assert_eq!(
            r.issues.len(),
            0,
            "Expected zero issues but got: {:?}",
            r.issues
        );
    }

    #[test]
    fn disabled_warning_does_not_suppress_problems() {
        // Problems are never suppressible. Disabling all warnings should
        // still report off-map objects (which are Problems).
        let mut config = ValidationConfig::default();
        for rule in WarningRule::ALL {
            config.disabled.insert(rule);
        }

        let mut map = valid_map(10, 10);
        map.structures
            .push(make_structure("A0LightFactory", 99999, 99999, 0));

        let r = validate_map(&map, None, &config);
        assert!(
            r.issues
                .iter()
                .any(|i| i.severity == Severity::Problem && i.message.contains("off the map")),
            "problems should not be suppressed even with all warnings disabled"
        );
    }
}
