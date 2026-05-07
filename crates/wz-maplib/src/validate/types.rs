//! Warning rules, configuration, result types, and stats lookup trait.

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

/// Individual warning rules that can be enabled/disabled by the user.
///
/// Each variant corresponds to a specific warning check in the validation engine.
/// Problems (errors) cannot be disabled - only warnings are suppressible.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum WarningRule {
    /// Map dimensions exceed the .wz export limit (250x250).
    WzExportDimensionLimit,
    /// Map name is empty.
    MapNameEmpty,
    /// Map name exceeds 16 characters.
    MapNameTooLong,
    /// Map base name does not start with a letter.
    MapNameInvalidStart,
    /// Map name contains invalid characters.
    MapNameInvalidChars,
    /// Tile height exceeds maximum (510).
    TileHeightOverflow,
    /// Tile texture index exceeds terrain type count.
    TileTextureOutOfRange,
    /// Water tile has incorrect triangle direction.
    WaterTriangleDirection,
    /// Object is within 3 tiles of the map edge.
    ObjectNearEdge,
    /// Two structures occupy the same tile.
    OverlappingStructures,
    /// Object name not found in the stats database.
    UnknownObjectName,
    /// Object has ID 0 (invalid in WZ2100).
    ZeroObjectId,
    /// Multiple objects share the same ID.
    DuplicateObjectIds,
    /// Structure has modules but its type does not accept them.
    InvalidModules,
    /// More than 255 structures of one type per player.
    StructureCountExceeded,
    /// Player count exceeds 10.
    PlayerCountHigh,
    /// Multiplayer player has no constructor units.
    MissingConstructors,
    /// Objects assigned to a player slot beyond the map's player count.
    UnusedPlayerObjects,
    /// Gateway is not axis-aligned (diagonal).
    GatewayNotAxisAligned,
    /// Label position is outside map bounds.
    LabelOutOfBounds,
    /// Multiple labels share the same name.
    DuplicateLabelNames,
}

impl WarningRule {
    /// All warning rules in display order.
    pub const ALL: [Self; 21] = [
        Self::WzExportDimensionLimit,
        Self::MapNameEmpty,
        Self::MapNameTooLong,
        Self::MapNameInvalidStart,
        Self::MapNameInvalidChars,
        Self::TileHeightOverflow,
        Self::TileTextureOutOfRange,
        Self::WaterTriangleDirection,
        Self::ObjectNearEdge,
        Self::OverlappingStructures,
        Self::UnknownObjectName,
        Self::ZeroObjectId,
        Self::DuplicateObjectIds,
        Self::InvalidModules,
        Self::StructureCountExceeded,
        Self::PlayerCountHigh,
        Self::MissingConstructors,
        Self::UnusedPlayerObjects,
        Self::GatewayNotAxisAligned,
        Self::LabelOutOfBounds,
        Self::DuplicateLabelNames,
    ];

    /// Human-readable label for this warning rule.
    pub fn label(self) -> &'static str {
        match self {
            Self::WzExportDimensionLimit => "WZ export dimension limit",
            Self::MapNameEmpty => "Empty map name",
            Self::MapNameTooLong => "Map name too long",
            Self::MapNameInvalidStart => "Map name invalid start character",
            Self::MapNameInvalidChars => "Map name invalid characters",
            Self::TileHeightOverflow => "Tile height overflow",
            Self::TileTextureOutOfRange => "Tile texture out of range",
            Self::WaterTriangleDirection => "Water triangle direction",
            Self::ObjectNearEdge => "Object near map edge",
            Self::OverlappingStructures => "Overlapping structures",
            Self::UnknownObjectName => "Unknown object name",
            Self::ZeroObjectId => "Zero object ID",
            Self::DuplicateObjectIds => "Duplicate object IDs",
            Self::InvalidModules => "Invalid structure modules",
            Self::StructureCountExceeded => "Structure count exceeded",
            Self::PlayerCountHigh => "Player count exceeds 10",
            Self::MissingConstructors => "Missing constructor units",
            Self::UnusedPlayerObjects => "Objects on unused player",
            Self::GatewayNotAxisAligned => "Non-axis-aligned gateway",
            Self::LabelOutOfBounds => "Label out of bounds",
            Self::DuplicateLabelNames => "Duplicate label names",
        }
    }

    /// The validation category this rule belongs to.
    pub fn category(self) -> ValidationCategory {
        match self {
            Self::WzExportDimensionLimit
            | Self::MapNameEmpty
            | Self::MapNameTooLong
            | Self::MapNameInvalidStart
            | Self::MapNameInvalidChars => ValidationCategory::Map,
            Self::TileHeightOverflow
            | Self::TileTextureOutOfRange
            | Self::WaterTriangleDirection => ValidationCategory::Terrain,
            Self::ObjectNearEdge | Self::OverlappingStructures => {
                ValidationCategory::ObjectPositions
            }
            Self::UnknownObjectName
            | Self::ZeroObjectId
            | Self::DuplicateObjectIds
            | Self::InvalidModules
            | Self::StructureCountExceeded => ValidationCategory::ObjectData,
            Self::PlayerCountHigh | Self::MissingConstructors | Self::UnusedPlayerObjects => {
                ValidationCategory::Multiplayer
            }
            Self::GatewayNotAxisAligned => ValidationCategory::Gateways,
            Self::LabelOutOfBounds | Self::DuplicateLabelNames => ValidationCategory::Labels,
        }
    }
}

/// Configuration for which validation warnings are enabled.
///
/// By default all warnings are enabled (empty disabled set).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ValidationConfig {
    /// Warning rules that have been disabled by the user.
    #[serde(default)]
    pub disabled: HashSet<WarningRule>,
}

impl ValidationConfig {
    /// Returns true if the given warning rule is enabled (not disabled).
    pub fn is_enabled(&self, rule: WarningRule) -> bool {
        !self.disabled.contains(&rule)
    }
}

/// Severity of a validation issue.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Severity {
    /// Blocks correct gameplay - the map will not work correctly in-game.
    Problem,
    /// Suspicious but not necessarily fatal - may indicate a mistake.
    Warning,
}

/// Category grouping for validation issues.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ValidationCategory {
    Map,
    Terrain,
    ObjectPositions,
    ObjectData,
    Multiplayer,
    Gateways,
    Labels,
}

impl ValidationCategory {
    pub fn label(self) -> &'static str {
        match self {
            Self::Map => "Map",
            Self::Terrain => "Terrain",
            Self::ObjectPositions => "Object Positions",
            Self::ObjectData => "Object Data",
            Self::Multiplayer => "Multiplayer",
            Self::Gateways => "Gateways",
            Self::Labels => "Labels",
        }
    }
}

/// Location of a validation issue, used for viewport navigation.
#[derive(Debug, Clone)]
pub enum IssueLocation {
    /// No specific location (map-level issue).
    None,
    /// A world position (for objects).
    WorldPos { x: u32, y: u32 },
    /// A tile coordinate (for terrain issues).
    TilePos { x: u32, y: u32 },
}

/// A single validation issue.
#[derive(Debug, Clone)]
pub struct ValidationIssue {
    pub severity: Severity,
    pub category: ValidationCategory,
    pub message: String,
    pub location: IssueLocation,
    /// The warning rule that produced this issue (`None` for problems).
    pub rule: Option<WarningRule>,
}

/// Results of a full map validation pass.
#[derive(Debug, Clone, Default)]
pub struct ValidationResults {
    pub issues: Vec<ValidationIssue>,
}

impl ValidationResults {
    pub fn problem_count(&self) -> usize {
        self.issues
            .iter()
            .filter(|i| i.severity == Severity::Problem)
            .count()
    }

    pub fn warning_count(&self) -> usize {
        self.issues
            .iter()
            .filter(|i| i.severity == Severity::Warning)
            .count()
    }

    pub fn has_problems(&self) -> bool {
        self.issues.iter().any(|i| i.severity == Severity::Problem)
    }

    /// Group issues by category for tree-view display.
    /// Returns only categories that have at least one issue, sorted by category order.
    pub fn by_category(&self) -> Vec<(ValidationCategory, Vec<&ValidationIssue>)> {
        let mut map: HashMap<ValidationCategory, Vec<&ValidationIssue>> = HashMap::new();
        for issue in &self.issues {
            map.entry(issue.category).or_default().push(issue);
        }
        let mut groups: Vec<_> = map.into_iter().collect();
        groups.sort_by_key(|(cat, _)| *cat);
        groups
    }
}

/// Information about a structure type from the stats database.
#[derive(Debug, Clone)]
pub struct StructureInfo {
    pub structure_type: Option<String>,
    pub width: u32,
    pub breadth: u32,
}

/// Information about a feature type from the stats database.
#[derive(Debug, Clone)]
pub struct FeatureInfo {
    pub feature_type: Option<String>,
}

/// Information about a droid template from the stats database.
#[derive(Debug, Clone)]
pub struct TemplateInfo {
    pub droid_type: Option<String>,
    pub has_construct: bool,
}

/// Trait for looking up object stats during validation.
///
/// Allows `wz-maplib` to remain decoupled from `wz-stats`. The editor implements
/// this trait against `StatsDatabase`.
pub trait StatsLookup {
    fn structure_info(&self, name: &str) -> Option<StructureInfo>;
    fn feature_info(&self, name: &str) -> Option<FeatureInfo>;
    fn template_info(&self, name: &str) -> Option<TemplateInfo>;
}

#[cfg(test)]
mod tests {
    use super::*;

    // -- ValidationResults tests -------------------------------------------

    #[test]
    fn results_default_is_empty() {
        let r = ValidationResults::default();
        assert_eq!(r.issues.len(), 0);
        assert_eq!(r.problem_count(), 0);
        assert_eq!(r.warning_count(), 0);
        assert!(!r.has_problems());
    }

    #[test]
    fn results_problem_count() {
        let mut r = ValidationResults::default();
        r.issues.push(ValidationIssue {
            severity: Severity::Problem,
            category: ValidationCategory::Map,
            message: "a".into(),
            location: IssueLocation::None,
            rule: None,
        });
        r.issues.push(ValidationIssue {
            severity: Severity::Problem,
            category: ValidationCategory::Map,
            message: "b".into(),
            location: IssueLocation::None,
            rule: None,
        });
        r.issues.push(ValidationIssue {
            severity: Severity::Warning,
            category: ValidationCategory::Map,
            message: "c".into(),
            location: IssueLocation::None,
            rule: None,
        });
        assert_eq!(r.problem_count(), 2);
        assert_eq!(r.warning_count(), 1);
    }

    #[test]
    fn results_has_problems_false_when_only_warnings() {
        let mut r = ValidationResults::default();
        r.issues.push(ValidationIssue {
            severity: Severity::Warning,
            category: ValidationCategory::Map,
            message: "w".into(),
            location: IssueLocation::None,
            rule: None,
        });
        assert!(!r.has_problems());
    }

    #[test]
    fn results_has_problems_true_when_mixed() {
        let mut r = ValidationResults::default();
        r.issues.push(ValidationIssue {
            severity: Severity::Warning,
            category: ValidationCategory::Map,
            message: "w".into(),
            location: IssueLocation::None,
            rule: None,
        });
        r.issues.push(ValidationIssue {
            severity: Severity::Problem,
            category: ValidationCategory::Terrain,
            message: "p".into(),
            location: IssueLocation::None,
            rule: None,
        });
        assert!(r.has_problems());
    }

    #[test]
    fn results_by_category_groups_correctly() {
        let mut r = ValidationResults::default();
        r.issues.push(ValidationIssue {
            severity: Severity::Warning,
            category: ValidationCategory::Map,
            message: "m1".into(),
            location: IssueLocation::None,
            rule: None,
        });
        r.issues.push(ValidationIssue {
            severity: Severity::Problem,
            category: ValidationCategory::Terrain,
            message: "t1".into(),
            location: IssueLocation::None,
            rule: None,
        });
        r.issues.push(ValidationIssue {
            severity: Severity::Warning,
            category: ValidationCategory::Map,
            message: "m2".into(),
            location: IssueLocation::None,
            rule: None,
        });
        let groups = r.by_category();
        assert_eq!(groups.len(), 2);
        // Map comes before Terrain in enum order
        assert_eq!(groups[0].0, ValidationCategory::Map);
        assert_eq!(groups[0].1.len(), 2);
        assert_eq!(groups[1].0, ValidationCategory::Terrain);
        assert_eq!(groups[1].1.len(), 1);
    }

    #[test]
    fn results_by_category_empty_categories_omitted() {
        let r = ValidationResults::default();
        let groups = r.by_category();
        assert!(groups.is_empty());
    }

    // -- ValidationConfig tests -----------------------------------------------

    #[test]
    fn config_default_enables_all_warnings() {
        let config = ValidationConfig::default();
        for rule in WarningRule::ALL {
            assert!(config.is_enabled(rule), "{rule:?} should be enabled");
        }
    }

    #[test]
    fn config_disabled_rule_is_not_enabled() {
        let mut config = ValidationConfig::default();
        config.disabled.insert(WarningRule::ObjectNearEdge);
        assert!(!config.is_enabled(WarningRule::ObjectNearEdge));
        assert!(config.is_enabled(WarningRule::OverlappingStructures));
    }

    #[test]
    fn all_warning_rules_have_labels_and_categories() {
        for rule in WarningRule::ALL {
            assert!(!rule.label().is_empty(), "{rule:?} has empty label");
            // Just verify category() doesn't panic
            let _ = rule.category();
        }
    }

    #[test]
    fn warning_rule_all_covers_every_variant() {
        // Ensure WarningRule::ALL is updated when new variants are added.
        // Each variant must appear exactly once in ALL.
        let all_set: HashSet<WarningRule> = WarningRule::ALL.iter().copied().collect();
        assert_eq!(
            all_set.len(),
            WarningRule::ALL.len(),
            "WarningRule::ALL has duplicate entries"
        );
        // If a new variant is added but not in ALL, this will fail to compile
        // in the match below (non-exhaustive match).
        for rule in &WarningRule::ALL {
            match rule {
                WarningRule::WzExportDimensionLimit
                | WarningRule::MapNameEmpty
                | WarningRule::MapNameTooLong
                | WarningRule::MapNameInvalidStart
                | WarningRule::MapNameInvalidChars
                | WarningRule::TileHeightOverflow
                | WarningRule::TileTextureOutOfRange
                | WarningRule::WaterTriangleDirection
                | WarningRule::ObjectNearEdge
                | WarningRule::OverlappingStructures
                | WarningRule::UnknownObjectName
                | WarningRule::ZeroObjectId
                | WarningRule::DuplicateObjectIds
                | WarningRule::InvalidModules
                | WarningRule::StructureCountExceeded
                | WarningRule::PlayerCountHigh
                | WarningRule::MissingConstructors
                | WarningRule::UnusedPlayerObjects
                | WarningRule::GatewayNotAxisAligned
                | WarningRule::LabelOutOfBounds
                | WarningRule::DuplicateLabelNames => {}
            }
        }
    }
}
