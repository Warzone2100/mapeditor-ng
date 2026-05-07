//! Object data validation: names, player numbers, IDs, modules, structure counts.

use std::collections::HashMap;

use crate::constants::{MAX_PLAYERS, MAX_STRUCTURES_PER_TYPE, PLAYER_SCAVENGERS};
use crate::io_wz::WzMap;

use super::helpers::accepts_modules;
use super::types::{
    IssueLocation, StatsLookup, ValidationCategory, ValidationConfig, ValidationResults,
    WarningRule,
};
use super::{push_problem, push_warning};

/// Object data checks: names, player numbers, IDs, modules, structure counts.
pub(super) fn validate_object_data(
    map: &WzMap,
    stats: Option<&dyn StatsLookup>,
    config: &ValidationConfig,
    results: &mut ValidationResults,
) {
    let cat = ValidationCategory::ObjectData;
    let check_names = config.is_enabled(WarningRule::UnknownObjectName);
    let check_zero_id = config.is_enabled(WarningRule::ZeroObjectId);
    let check_dup_ids = config.is_enabled(WarningRule::DuplicateObjectIds);
    let check_modules = config.is_enabled(WarningRule::InvalidModules);
    let check_struct_count = config.is_enabled(WarningRule::StructureCountExceeded);

    let mut seen_ids: HashMap<u32, Vec<String>> = HashMap::new();

    for s in &map.structures {
        if check_names
            && let Some(stats) = stats
            && stats.structure_info(&s.name).is_none()
        {
            push_warning(
                results,
                WarningRule::UnknownObjectName,
                cat,
                format!(
                    "Structure \"{}\" at ({}, {}) not found in stats database.",
                    s.name, s.position.x, s.position.y
                ),
                IssueLocation::WorldPos {
                    x: s.position.x,
                    y: s.position.y,
                },
            );
        }

        if s.player < PLAYER_SCAVENGERS || s.player >= MAX_PLAYERS as i8 {
            push_problem(
                results,
                cat,
                format!(
                    "Structure \"{}\" at ({}, {}) has invalid player {}.",
                    s.name, s.position.x, s.position.y, s.player
                ),
                IssueLocation::WorldPos {
                    x: s.position.x,
                    y: s.position.y,
                },
            );
        }

        if let Some(id) = s.id {
            if id == 0 {
                if check_zero_id {
                    push_warning(
                        results,
                        WarningRule::ZeroObjectId,
                        cat,
                        format!(
                            "Structure \"{}\" at ({}, {}) has invalid ID 0.",
                            s.name, s.position.x, s.position.y
                        ),
                        IssueLocation::WorldPos {
                            x: s.position.x,
                            y: s.position.y,
                        },
                    );
                }
            } else if check_dup_ids {
                seen_ids
                    .entry(id)
                    .or_default()
                    .push(format!("Structure \"{}\"", s.name));
            }
        }

        if check_modules
            && s.modules > 0
            && let Some(stats) = stats
            && let Some(info) = stats.structure_info(&s.name)
            && let Some(ref stype) = info.structure_type
            && !accepts_modules(stype)
        {
            push_warning(
                results,
                WarningRule::InvalidModules,
                cat,
                format!(
                    "Structure \"{}\" (type {}) at ({}, {}) has {} modules but this type does not accept modules.",
                    s.name, stype, s.position.x, s.position.y, s.modules
                ),
                IssueLocation::WorldPos {
                    x: s.position.x,
                    y: s.position.y,
                },
            );
        }
    }

    for d in &map.droids {
        if check_names
            && let Some(stats) = stats
            && stats.template_info(&d.name).is_none()
        {
            push_warning(
                results,
                WarningRule::UnknownObjectName,
                cat,
                format!(
                    "Droid \"{}\" at ({}, {}) not found in stats database.",
                    d.name, d.position.x, d.position.y
                ),
                IssueLocation::WorldPos {
                    x: d.position.x,
                    y: d.position.y,
                },
            );
        }

        if d.player < PLAYER_SCAVENGERS || d.player >= MAX_PLAYERS as i8 {
            push_problem(
                results,
                cat,
                format!(
                    "Droid \"{}\" at ({}, {}) has invalid player {}.",
                    d.name, d.position.x, d.position.y, d.player
                ),
                IssueLocation::WorldPos {
                    x: d.position.x,
                    y: d.position.y,
                },
            );
        }

        if let Some(id) = d.id {
            if id == 0 {
                if check_zero_id {
                    push_warning(
                        results,
                        WarningRule::ZeroObjectId,
                        cat,
                        format!(
                            "Droid \"{}\" at ({}, {}) has invalid ID 0.",
                            d.name, d.position.x, d.position.y
                        ),
                        IssueLocation::WorldPos {
                            x: d.position.x,
                            y: d.position.y,
                        },
                    );
                }
            } else if check_dup_ids {
                seen_ids
                    .entry(id)
                    .or_default()
                    .push(format!("Droid \"{}\"", d.name));
            }
        }
    }

    for f in &map.features {
        if check_names
            && let Some(stats) = stats
            && stats.feature_info(&f.name).is_none()
        {
            push_warning(
                results,
                WarningRule::UnknownObjectName,
                cat,
                format!(
                    "Feature \"{}\" at ({}, {}) not found in stats database.",
                    f.name, f.position.x, f.position.y
                ),
                IssueLocation::WorldPos {
                    x: f.position.x,
                    y: f.position.y,
                },
            );
        }

        if let Some(player) = f.player
            && (player < PLAYER_SCAVENGERS || player >= MAX_PLAYERS as i8)
        {
            push_problem(
                results,
                cat,
                format!(
                    "Feature \"{}\" at ({}, {}) has invalid player {}.",
                    f.name, f.position.x, f.position.y, player
                ),
                IssueLocation::WorldPos {
                    x: f.position.x,
                    y: f.position.y,
                },
            );
        }

        if let Some(id) = f.id {
            if id == 0 {
                if check_zero_id {
                    push_warning(
                        results,
                        WarningRule::ZeroObjectId,
                        cat,
                        format!(
                            "Feature \"{}\" at ({}, {}) has invalid ID 0.",
                            f.name, f.position.x, f.position.y
                        ),
                        IssueLocation::WorldPos {
                            x: f.position.x,
                            y: f.position.y,
                        },
                    );
                }
            } else if check_dup_ids {
                seen_ids
                    .entry(id)
                    .or_default()
                    .push(format!("Feature \"{}\"", f.name));
            }
        }
    }

    if check_dup_ids {
        for (id, owners) in &seen_ids {
            if owners.len() > 1 {
                push_warning(
                    results,
                    WarningRule::DuplicateObjectIds,
                    cat,
                    format!("Duplicate object ID {id}: {}.", owners.join(", ")),
                    IssueLocation::None,
                );
            }
        }
    }

    if check_struct_count {
        let mut type_counts: HashMap<(i8, &str), usize> = HashMap::new();
        for s in &map.structures {
            *type_counts.entry((s.player, &s.name)).or_insert(0) += 1;
        }
        for (&(player, name), &count) in &type_counts {
            if count > MAX_STRUCTURES_PER_TYPE {
                push_warning(
                    results,
                    WarningRule::StructureCountExceeded,
                    cat,
                    format!(
                        "Player {player} has {count} of structure \"{name}\". The limit is {MAX_STRUCTURES_PER_TYPE}.",
                    ),
                    IssueLocation::None,
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constants::TILE_UNITS;
    use crate::validate::test_support::*;
    use crate::validate::types::Severity;

    #[test]
    fn structure_name_in_stats_ok() {
        let mut map = valid_map(64, 64);
        let center = 32 * TILE_UNITS;
        map.structures
            .push(make_structure("A0PowerGenerator", center, center, 0));
        let stats = MockStats::new().with_structure("A0PowerGenerator", "POWER GENERATOR", 1, 1);
        let mut results = ValidationResults::default();
        validate_object_data(
            &map,
            Some(&stats),
            &ValidationConfig::default(),
            &mut results,
        );
        assert!(
            !results
                .issues
                .iter()
                .any(|i| i.message.contains("not found")),
            "issues: {:?}",
            results.issues
        );
    }

    #[test]
    fn structure_name_not_in_stats_warns() {
        let mut map = valid_map(64, 64);
        let center = 32 * TILE_UNITS;
        map.structures
            .push(make_structure("FakeBuilding", center, center, 0));
        let stats = MockStats::new();
        let mut results = ValidationResults::default();
        validate_object_data(
            &map,
            Some(&stats),
            &ValidationConfig::default(),
            &mut results,
        );
        assert!(
            results
                .issues
                .iter()
                .any(|i| i.severity == Severity::Warning
                    && i.message.contains("FakeBuilding")
                    && i.message.contains("not found"))
        );
    }

    #[test]
    fn feature_name_not_in_stats_warns() {
        let mut map = valid_map(64, 64);
        let center = 32 * TILE_UNITS;
        map.features
            .push(make_feature("FakeFeature", center, center));
        let stats = MockStats::new();
        let mut results = ValidationResults::default();
        validate_object_data(
            &map,
            Some(&stats),
            &ValidationConfig::default(),
            &mut results,
        );
        assert!(
            results
                .issues
                .iter()
                .any(|i| i.severity == Severity::Warning
                    && i.message.contains("FakeFeature")
                    && i.message.contains("not found"))
        );
    }

    #[test]
    fn droid_name_not_in_stats_warns() {
        let mut map = valid_map(64, 64);
        let center = 32 * TILE_UNITS;
        map.droids
            .push(make_droid("FakeTemplate", center, center, 0));
        let stats = MockStats::new();
        let mut results = ValidationResults::default();
        validate_object_data(
            &map,
            Some(&stats),
            &ValidationConfig::default(),
            &mut results,
        );
        assert!(
            results
                .issues
                .iter()
                .any(|i| i.severity == Severity::Warning
                    && i.message.contains("FakeTemplate")
                    && i.message.contains("not found"))
        );
    }

    #[test]
    fn no_stats_skips_name_checks() {
        let mut map = valid_map(64, 64);
        let center = 32 * TILE_UNITS;
        map.structures
            .push(make_structure("FakeBuilding", center, center, 0));
        map.features
            .push(make_feature("FakeFeature", center, center));
        map.droids
            .push(make_droid("FakeTemplate", center, center, 0));
        let mut results = ValidationResults::default();
        validate_object_data(&map, None, &ValidationConfig::default(), &mut results);
        assert!(
            !results
                .issues
                .iter()
                .any(|i| i.message.contains("not found"))
        );
    }

    #[test]
    fn player_negative_one_scavenger_ok() {
        let mut map = valid_map(64, 64);
        let center = 32 * TILE_UNITS;
        map.structures
            .push(make_structure("Test", center, center, -1));
        let mut results = ValidationResults::default();
        validate_object_data(&map, None, &ValidationConfig::default(), &mut results);
        assert!(
            !results
                .issues
                .iter()
                .any(|i| i.message.contains("invalid player")),
            "issues: {:?}",
            results.issues
        );
    }

    #[test]
    fn player_zero_ok() {
        let mut map = valid_map(64, 64);
        let center = 32 * TILE_UNITS;
        map.structures
            .push(make_structure("Test", center, center, 0));
        let mut results = ValidationResults::default();
        validate_object_data(&map, None, &ValidationConfig::default(), &mut results);
        assert!(
            !results
                .issues
                .iter()
                .any(|i| i.message.contains("invalid player"))
        );
    }

    #[test]
    fn player_ten_ok() {
        let mut map = valid_map(64, 64);
        let center = 32 * TILE_UNITS;
        map.structures
            .push(make_structure("Test", center, center, 10));
        let mut results = ValidationResults::default();
        validate_object_data(&map, None, &ValidationConfig::default(), &mut results);
        assert!(
            !results
                .issues
                .iter()
                .any(|i| i.message.contains("invalid player"))
        );
    }

    #[test]
    fn player_eleven_invalid() {
        let mut map = valid_map(64, 64);
        let center = 32 * TILE_UNITS;
        map.structures
            .push(make_structure("Test", center, center, 11));
        let mut results = ValidationResults::default();
        validate_object_data(&map, None, &ValidationConfig::default(), &mut results);
        assert!(results
            .issues
            .iter()
            .any(|i| i.severity == Severity::Problem && i.message.contains("invalid player 11")));
    }

    #[test]
    fn player_negative_two_invalid() {
        let mut map = valid_map(64, 64);
        let center = 32 * TILE_UNITS;
        map.structures
            .push(make_structure("Test", center, center, -2));
        let mut results = ValidationResults::default();
        validate_object_data(&map, None, &ValidationConfig::default(), &mut results);
        assert!(results
            .issues
            .iter()
            .any(|i| i.severity == Severity::Problem && i.message.contains("invalid player -2")));
    }

    #[test]
    fn feature_player_none_ok() {
        let mut map = valid_map(64, 64);
        let center = 32 * TILE_UNITS;
        map.features.push(make_feature("Test", center, center));
        let mut results = ValidationResults::default();
        validate_object_data(&map, None, &ValidationConfig::default(), &mut results);
        assert!(
            !results
                .issues
                .iter()
                .any(|i| i.message.contains("invalid player"))
        );
    }

    #[test]
    fn feature_player_invalid() {
        let mut map = valid_map(64, 64);
        let center = 32 * TILE_UNITS;
        map.features
            .push(make_feature_with_player("Test", center, center, 15));
        let mut results = ValidationResults::default();
        validate_object_data(&map, None, &ValidationConfig::default(), &mut results);
        assert!(results
            .issues
            .iter()
            .any(|i| i.severity == Severity::Problem && i.message.contains("invalid player 15")));
    }

    #[test]
    fn duplicate_ids_two_structures() {
        let mut map = valid_map(64, 64);
        let center = 32 * TILE_UNITS;
        map.structures
            .push(make_structure_with_id("A", center, center, 0, 5));
        map.structures.push(make_structure_with_id(
            "B",
            center + TILE_UNITS,
            center,
            0,
            5,
        ));
        let mut results = ValidationResults::default();
        validate_object_data(&map, None, &ValidationConfig::default(), &mut results);
        assert!(
            results
                .issues
                .iter()
                .any(|i| i.message.contains("Duplicate object ID 5"))
        );
    }

    #[test]
    fn duplicate_ids_across_types() {
        let mut map = valid_map(64, 64);
        let center = 32 * TILE_UNITS;
        map.structures
            .push(make_structure_with_id("S", center, center, 0, 5));
        map.droids
            .push(make_droid_with_id("D", center, center, 0, 5));
        let mut results = ValidationResults::default();
        validate_object_data(&map, None, &ValidationConfig::default(), &mut results);
        assert!(
            results
                .issues
                .iter()
                .any(|i| i.message.contains("Duplicate object ID 5"))
        );
    }

    #[test]
    fn duplicate_ids_three_way() {
        let mut map = valid_map(64, 64);
        let center = 32 * TILE_UNITS;
        map.structures
            .push(make_structure_with_id("S", center, center, 0, 5));
        map.droids
            .push(make_droid_with_id("D", center, center, 0, 5));
        map.features
            .push(make_feature_with_id("F", center, center, 5));
        let mut results = ValidationResults::default();
        validate_object_data(&map, None, &ValidationConfig::default(), &mut results);
        assert!(
            results
                .issues
                .iter()
                .any(|i| i.message.contains("Duplicate object ID 5"))
        );
    }

    #[test]
    fn no_duplicate_ids_different_values() {
        let mut map = valid_map(64, 64);
        let center = 32 * TILE_UNITS;
        map.structures
            .push(make_structure_with_id("A", center, center, 0, 5));
        map.structures.push(make_structure_with_id(
            "B",
            center + TILE_UNITS,
            center,
            0,
            6,
        ));
        let mut results = ValidationResults::default();
        validate_object_data(&map, None, &ValidationConfig::default(), &mut results);
        assert!(
            !results
                .issues
                .iter()
                .any(|i| i.message.contains("Duplicate"))
        );
    }

    #[test]
    fn none_ids_not_duplicates() {
        let mut map = valid_map(64, 64);
        let center = 32 * TILE_UNITS;
        map.structures.push(make_structure("A", center, center, 0));
        map.structures
            .push(make_structure("B", center + TILE_UNITS, center, 0));
        let mut results = ValidationResults::default();
        validate_object_data(&map, None, &ValidationConfig::default(), &mut results);
        assert!(
            !results
                .issues
                .iter()
                .any(|i| i.message.contains("Duplicate"))
        );
    }

    #[test]
    fn zero_id_structure_warns() {
        let mut map = valid_map(64, 64);
        let center = 32 * TILE_UNITS;
        map.structures
            .push(make_structure_with_id("Test", center, center, 0, 0));
        let mut results = ValidationResults::default();
        validate_object_data(&map, None, &ValidationConfig::default(), &mut results);
        assert!(
            results
                .issues
                .iter()
                .any(|i| i.severity == Severity::Warning && i.message.contains("invalid ID 0"))
        );
    }

    #[test]
    fn zero_id_droid_warns() {
        let mut map = valid_map(64, 64);
        let center = 32 * TILE_UNITS;
        map.droids
            .push(make_droid_with_id("Test", center, center, 0, 0));
        let mut results = ValidationResults::default();
        validate_object_data(&map, None, &ValidationConfig::default(), &mut results);
        assert!(
            results
                .issues
                .iter()
                .any(|i| i.severity == Severity::Warning
                    && i.message.contains("Droid")
                    && i.message.contains("invalid ID 0"))
        );
    }

    #[test]
    fn zero_id_feature_warns() {
        let mut map = valid_map(64, 64);
        let center = 32 * TILE_UNITS;
        map.features
            .push(make_feature_with_id("Test", center, center, 0));
        let mut results = ValidationResults::default();
        validate_object_data(&map, None, &ValidationConfig::default(), &mut results);
        assert!(
            results
                .issues
                .iter()
                .any(|i| i.severity == Severity::Warning
                    && i.message.contains("Feature")
                    && i.message.contains("invalid ID 0"))
        );
    }

    #[test]
    fn nonzero_id_ok() {
        let mut map = valid_map(64, 64);
        let center = 32 * TILE_UNITS;
        map.structures
            .push(make_structure_with_id("Test", center, center, 0, 1));
        let mut results = ValidationResults::default();
        validate_object_data(&map, None, &ValidationConfig::default(), &mut results);
        assert!(
            !results
                .issues
                .iter()
                .any(|i| i.message.contains("invalid ID"))
        );
    }

    #[test]
    fn modules_on_factory_ok() {
        let mut map = valid_map(64, 64);
        let center = 32 * TILE_UNITS;
        let mut s = make_structure("A0LightFactory", center, center, 0);
        s.modules = 2;
        map.structures.push(s);
        let stats = MockStats::new().with_structure("A0LightFactory", "FACTORY", 3, 3);
        let mut results = ValidationResults::default();
        validate_object_data(
            &map,
            Some(&stats),
            &ValidationConfig::default(),
            &mut results,
        );
        assert!(
            !results
                .issues
                .iter()
                .any(|i| i.message.contains("does not accept modules")),
            "issues: {:?}",
            results.issues
        );
    }

    #[test]
    fn modules_on_research_ok() {
        let mut map = valid_map(64, 64);
        let center = 32 * TILE_UNITS;
        let mut s = make_structure("A0ResearchFacility", center, center, 0);
        s.modules = 1;
        map.structures.push(s);
        let stats = MockStats::new().with_structure("A0ResearchFacility", "RESEARCH", 1, 1);
        let mut results = ValidationResults::default();
        validate_object_data(
            &map,
            Some(&stats),
            &ValidationConfig::default(),
            &mut results,
        );
        assert!(
            !results
                .issues
                .iter()
                .any(|i| i.message.contains("does not accept modules"))
        );
    }

    #[test]
    fn modules_on_power_generator_ok() {
        let mut map = valid_map(64, 64);
        let center = 32 * TILE_UNITS;
        let mut s = make_structure("A0PowerGenerator", center, center, 0);
        s.modules = 1;
        map.structures.push(s);
        let stats = MockStats::new().with_structure("A0PowerGenerator", "POWER GENERATOR", 1, 1);
        let mut results = ValidationResults::default();
        validate_object_data(
            &map,
            Some(&stats),
            &ValidationConfig::default(),
            &mut results,
        );
        assert!(
            !results
                .issues
                .iter()
                .any(|i| i.message.contains("does not accept modules"))
        );
    }

    #[test]
    fn modules_on_wall_warns() {
        let mut map = valid_map(64, 64);
        let center = 32 * TILE_UNITS;
        let mut s = make_structure("A0HardcreteMk1Wall", center, center, 0);
        s.modules = 1;
        map.structures.push(s);
        let stats = MockStats::new().with_structure("A0HardcreteMk1Wall", "WALL", 1, 1);
        let mut results = ValidationResults::default();
        validate_object_data(
            &map,
            Some(&stats),
            &ValidationConfig::default(),
            &mut results,
        );
        assert!(
            results
                .issues
                .iter()
                .any(|i| i.message.contains("does not accept modules"))
        );
    }

    #[test]
    fn modules_on_defense_warns() {
        let mut map = valid_map(64, 64);
        let center = 32 * TILE_UNITS;
        let mut s = make_structure("A0BaBaBunker", center, center, 0);
        s.modules = 1;
        map.structures.push(s);
        let stats = MockStats::new().with_structure("A0BaBaBunker", "DEFENSE", 1, 1);
        let mut results = ValidationResults::default();
        validate_object_data(
            &map,
            Some(&stats),
            &ValidationConfig::default(),
            &mut results,
        );
        assert!(
            results
                .issues
                .iter()
                .any(|i| i.message.contains("does not accept modules"))
        );
    }

    #[test]
    fn modules_zero_on_any_type_ok() {
        let mut map = valid_map(64, 64);
        let center = 32 * TILE_UNITS;
        map.structures
            .push(make_structure("A0HardcreteMk1Wall", center, center, 0));
        let stats = MockStats::new().with_structure("A0HardcreteMk1Wall", "WALL", 1, 1);
        let mut results = ValidationResults::default();
        validate_object_data(
            &map,
            Some(&stats),
            &ValidationConfig::default(),
            &mut results,
        );
        assert!(
            !results
                .issues
                .iter()
                .any(|i| i.message.contains("does not accept modules"))
        );
    }

    #[test]
    fn modules_no_stats_skips_check() {
        let mut map = valid_map(64, 64);
        let center = 32 * TILE_UNITS;
        let mut s = make_structure("WallThing", center, center, 0);
        s.modules = 5;
        map.structures.push(s);
        let mut results = ValidationResults::default();
        validate_object_data(&map, None, &ValidationConfig::default(), &mut results);
        assert!(
            !results
                .issues
                .iter()
                .any(|i| i.message.contains("does not accept modules"))
        );
    }

    #[test]
    fn structure_count_at_limit_ok() {
        let mut map = valid_map(64, 64);
        let center = 32 * TILE_UNITS;
        for _ in 0..MAX_STRUCTURES_PER_TYPE {
            map.structures
                .push(make_structure("A0HardcreteMk1Wall", center, center, 0));
        }
        let mut results = ValidationResults::default();
        validate_object_data(&map, None, &ValidationConfig::default(), &mut results);
        assert!(
            !results
                .issues
                .iter()
                .any(|i| i.message.contains("limit is"))
        );
    }

    #[test]
    fn structure_count_exceeds_limit() {
        let mut map = valid_map(64, 64);
        let center = 32 * TILE_UNITS;
        for _ in 0..=MAX_STRUCTURES_PER_TYPE {
            map.structures
                .push(make_structure("A0HardcreteMk1Wall", center, center, 0));
        }
        let mut results = ValidationResults::default();
        validate_object_data(&map, None, &ValidationConfig::default(), &mut results);
        assert!(
            results
                .issues
                .iter()
                .any(|i| i.message.contains("limit is 255"))
        );
    }

    #[test]
    fn structure_count_per_player_independent() {
        let mut map = valid_map(64, 64);
        let center = 32 * TILE_UNITS;
        for _ in 0..MAX_STRUCTURES_PER_TYPE {
            map.structures
                .push(make_structure("Wall", center, center, 0));
        }
        for _ in 0..MAX_STRUCTURES_PER_TYPE {
            map.structures
                .push(make_structure("Wall", center, center, 1));
        }
        let mut results = ValidationResults::default();
        validate_object_data(&map, None, &ValidationConfig::default(), &mut results);
        assert!(
            !results.issues.iter().any(|i| i.message.contains("limit")),
            "issues: {:?}",
            results.issues
        );
    }

    #[test]
    fn structure_count_different_types_ok() {
        let mut map = valid_map(64, 64);
        let center = 32 * TILE_UNITS;
        for _ in 0..MAX_STRUCTURES_PER_TYPE {
            map.structures
                .push(make_structure("WallA", center, center, 0));
        }
        for _ in 0..MAX_STRUCTURES_PER_TYPE {
            map.structures
                .push(make_structure("WallB", center, center, 0));
        }
        let mut results = ValidationResults::default();
        validate_object_data(&map, None, &ValidationConfig::default(), &mut results);
        assert!(!results.issues.iter().any(|i| i.message.contains("limit")));
    }

    #[test]
    fn disabled_unknown_name_suppresses_warning() {
        let mut config = ValidationConfig::default();
        config.disabled.insert(WarningRule::UnknownObjectName);

        let mut map = valid_map(20, 20);
        map.structures
            .push(make_structure("FakeBuilding", 1000, 1000, 0));

        let stats = MockStats::new();
        let r = crate::validate::validate_map(&map, Some(&stats), &ValidationConfig::default());
        assert!(
            r.issues
                .iter()
                .any(|i| i.message.contains("not found in stats"))
        );

        let r = crate::validate::validate_map(&map, Some(&stats), &config);
        assert!(
            !r.issues
                .iter()
                .any(|i| i.message.contains("not found in stats"))
        );
    }
}
