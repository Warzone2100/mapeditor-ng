//! Multiplayer validation: player count, HQ, constructors, unused players.

use crate::constants::PLAYER_SCAVENGERS;
use crate::io_wz::WzMap;

use super::types::{
    IssueLocation, StatsLookup, ValidationCategory, ValidationConfig, ValidationResults,
    WarningRule,
};
use super::{push_problem, push_warning};

pub(super) fn validate_multiplayer(
    map: &WzMap,
    stats: Option<&dyn StatsLookup>,
    config: &ValidationConfig,
    results: &mut ValidationResults,
) {
    let cat = ValidationCategory::Multiplayer;
    let players = map.players;

    if players < 2 {
        return;
    }

    if config.is_enabled(WarningRule::PlayerCountHigh) && players > 10 {
        push_warning(
            results,
            WarningRule::PlayerCountHigh,
            cat,
            format!("Player count {players} exceeds maximum of 10."),
            IssueLocation::None,
        );
    }

    if let Some(stats) = stats {
        let check_constructors = config.is_enabled(WarningRule::MissingConstructors);
        for p in 0..players {
            let player = p as i8;

            // HQ is a Problem (always checked, not suppressible).
            let has_hq = map.structures.iter().any(|s| {
                s.player == player
                    && stats
                        .structure_info(&s.name)
                        .and_then(|info| info.structure_type)
                        .as_deref()
                        == Some("HQ")
            });
            if !has_hq {
                push_problem(
                    results,
                    cat,
                    format!("Player {player} has no Command Centre (HQ)."),
                    IssueLocation::None,
                );
            }

            if check_constructors {
                let has_constructor = map.droids.iter().any(|d| {
                    d.player == player
                        && stats
                            .template_info(&d.name)
                            .is_some_and(|t| t.has_construct)
                });
                if !has_constructor {
                    push_warning(
                        results,
                        WarningRule::MissingConstructors,
                        cat,
                        format!("Player {player} has no constructor units."),
                        IssueLocation::None,
                    );
                }
            }
        }
    }

    if config.is_enabled(WarningRule::UnusedPlayerObjects) {
        for s in &map.structures {
            if s.player >= 0 && s.player != PLAYER_SCAVENGERS && (s.player as u8) >= players {
                push_warning(
                    results,
                    WarningRule::UnusedPlayerObjects,
                    cat,
                    format!(
                        "Structure \"{}\" at ({}, {}) is assigned to unused player {}.",
                        s.name, s.position.x, s.position.y, s.player
                    ),
                    IssueLocation::WorldPos {
                        x: s.position.x,
                        y: s.position.y,
                    },
                );
            }
        }
        for d in &map.droids {
            if d.player >= 0 && d.player != PLAYER_SCAVENGERS && (d.player as u8) >= players {
                push_warning(
                    results,
                    WarningRule::UnusedPlayerObjects,
                    cat,
                    format!(
                        "Droid \"{}\" at ({}, {}) is assigned to unused player {}.",
                        d.name, d.position.x, d.position.y, d.player
                    ),
                    IssueLocation::WorldPos {
                        x: d.position.x,
                        y: d.position.y,
                    },
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
    fn multiplayer_valid_setup() {
        let mut map = multiplayer_map(4);
        let center = 32 * TILE_UNITS;
        let stats = MockStats::new()
            .with_structure("A0CommandCentre", "HQ", 2, 2)
            .with_template("ConstructorDroid", "DROID", true);
        for p in 0..4i8 {
            map.structures.push(make_structure(
                "A0CommandCentre",
                center,
                center + (p as u32) * TILE_UNITS * 4,
                p,
            ));
            map.droids.push(make_droid(
                "ConstructorDroid",
                center + TILE_UNITS,
                center + (p as u32) * TILE_UNITS * 4,
                p,
            ));
        }
        let mut results = ValidationResults::default();
        validate_multiplayer(
            &map,
            Some(&stats),
            &ValidationConfig::default(),
            &mut results,
        );
        assert!(results.issues.is_empty(), "issues: {:?}", results.issues);
    }

    #[test]
    fn singleplayer_map_skips_multiplayer_checks() {
        let mut map = valid_map(64, 64);
        map.players = 1;
        let mut results = ValidationResults::default();
        validate_multiplayer(&map, None, &ValidationConfig::default(), &mut results);
        assert!(results.issues.is_empty());
    }

    #[test]
    fn player_count_above_ten_warns() {
        let map = multiplayer_map(12);
        let mut results = ValidationResults::default();
        validate_multiplayer(&map, None, &ValidationConfig::default(), &mut results);
        assert!(
            results
                .issues
                .iter()
                .any(|i| i.severity == Severity::Warning && i.message.contains("exceeds maximum"))
        );
    }

    #[test]
    fn player_missing_hq() {
        let map = multiplayer_map(2);
        let stats = MockStats::new()
            .with_structure("A0CommandCentre", "HQ", 2, 2)
            .with_template("ConstructorDroid", "DROID", true);
        let mut results = ValidationResults::default();
        validate_multiplayer(
            &map,
            Some(&stats),
            &ValidationConfig::default(),
            &mut results,
        );
        let hq_problems: Vec<_> = results
            .issues
            .iter()
            .filter(|i| i.severity == Severity::Problem && i.message.contains("Command Centre"))
            .collect();
        assert_eq!(hq_problems.len(), 2);
    }

    #[test]
    fn player_has_hq_ok() {
        let mut map = multiplayer_map(2);
        let center = 32 * TILE_UNITS;
        let stats = MockStats::new()
            .with_structure("A0CommandCentre", "HQ", 2, 2)
            .with_template("ConstructorDroid", "DROID", true);
        for p in 0..2i8 {
            map.structures.push(make_structure(
                "A0CommandCentre",
                center,
                center + (p as u32) * TILE_UNITS * 4,
                p,
            ));
            map.droids.push(make_droid(
                "ConstructorDroid",
                center + TILE_UNITS,
                center + (p as u32) * TILE_UNITS * 4,
                p,
            ));
        }
        let mut results = ValidationResults::default();
        validate_multiplayer(
            &map,
            Some(&stats),
            &ValidationConfig::default(),
            &mut results,
        );
        assert!(
            !results
                .issues
                .iter()
                .any(|i| i.message.contains("Command Centre")),
            "issues: {:?}",
            results.issues
        );
    }

    #[test]
    fn some_players_missing_hq() {
        let mut map = multiplayer_map(2);
        let center = 32 * TILE_UNITS;
        let stats = MockStats::new()
            .with_structure("A0CommandCentre", "HQ", 2, 2)
            .with_template("ConstructorDroid", "DROID", true);
        // Only player 0 gets an HQ.
        map.structures
            .push(make_structure("A0CommandCentre", center, center, 0));
        map.droids.push(make_droid(
            "ConstructorDroid",
            center + TILE_UNITS,
            center,
            0,
        ));
        map.droids.push(make_droid(
            "ConstructorDroid",
            center + TILE_UNITS,
            center + 4 * TILE_UNITS,
            1,
        ));
        let mut results = ValidationResults::default();
        validate_multiplayer(
            &map,
            Some(&stats),
            &ValidationConfig::default(),
            &mut results,
        );
        let hq_problems: Vec<_> = results
            .issues
            .iter()
            .filter(|i| i.severity == Severity::Problem && i.message.contains("Command Centre"))
            .collect();
        assert_eq!(hq_problems.len(), 1);
    }

    #[test]
    fn player_missing_constructor() {
        let mut map = multiplayer_map(2);
        let center = 32 * TILE_UNITS;
        let stats = MockStats::new()
            .with_structure("A0CommandCentre", "HQ", 2, 2)
            .with_template("ConstructorDroid", "DROID", true)
            .with_template("ViperMk1", "DROID", false);
        for p in 0..2i8 {
            map.structures.push(make_structure(
                "A0CommandCentre",
                center,
                center + (p as u32) * TILE_UNITS * 4,
                p,
            ));
        }
        // Player 0 has a constructor; player 1 has only a combat droid.
        map.droids.push(make_droid(
            "ConstructorDroid",
            center + TILE_UNITS,
            center,
            0,
        ));
        map.droids.push(make_droid(
            "ViperMk1",
            center + TILE_UNITS,
            center + 4 * TILE_UNITS,
            1,
        ));
        let mut results = ValidationResults::default();
        validate_multiplayer(
            &map,
            Some(&stats),
            &ValidationConfig::default(),
            &mut results,
        );
        let constructor_warnings: Vec<_> = results
            .issues
            .iter()
            .filter(|i| i.message.contains("constructor"))
            .collect();
        assert_eq!(constructor_warnings.len(), 1);
    }

    #[test]
    fn player_has_constructor_ok() {
        let mut map = multiplayer_map(2);
        let center = 32 * TILE_UNITS;
        let stats = MockStats::new()
            .with_structure("A0CommandCentre", "HQ", 2, 2)
            .with_template("ConstructorDroid", "DROID", true);
        for p in 0..2i8 {
            map.structures.push(make_structure(
                "A0CommandCentre",
                center,
                center + (p as u32) * TILE_UNITS * 4,
                p,
            ));
            map.droids.push(make_droid(
                "ConstructorDroid",
                center + TILE_UNITS,
                center + (p as u32) * TILE_UNITS * 4,
                p,
            ));
        }
        let mut results = ValidationResults::default();
        validate_multiplayer(
            &map,
            Some(&stats),
            &ValidationConfig::default(),
            &mut results,
        );
        assert!(
            !results
                .issues
                .iter()
                .any(|i| i.message.contains("constructor"))
        );
    }

    #[test]
    fn unused_player_has_units() {
        let mut map = multiplayer_map(4);
        let center = 32 * TILE_UNITS;
        map.structures
            .push(make_structure("Test", center, center, 5));
        let mut results = ValidationResults::default();
        validate_multiplayer(&map, None, &ValidationConfig::default(), &mut results);
        assert!(
            results
                .issues
                .iter()
                .any(|i| i.severity == Severity::Warning && i.message.contains("unused player 5"))
        );
    }

    #[test]
    fn scavenger_units_not_flagged_as_unused() {
        let mut map = multiplayer_map(4);
        let center = 32 * TILE_UNITS;
        map.structures
            .push(make_structure("ScavThing", center, center, -1));
        let mut results = ValidationResults::default();
        validate_multiplayer(&map, None, &ValidationConfig::default(), &mut results);
        assert!(
            !results
                .issues
                .iter()
                .any(|i| i.message.contains("unused player"))
        );
    }

    #[test]
    fn all_players_accounted_for() {
        let mut map = multiplayer_map(4);
        let center = 32 * TILE_UNITS;
        for p in 0..4i8 {
            map.structures.push(make_structure(
                "Test",
                center,
                center + (p as u32) * TILE_UNITS * 4,
                p,
            ));
        }
        let mut results = ValidationResults::default();
        validate_multiplayer(&map, None, &ValidationConfig::default(), &mut results);
        assert!(
            !results
                .issues
                .iter()
                .any(|i| i.message.contains("unused player"))
        );
    }
}
