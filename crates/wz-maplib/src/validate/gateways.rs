//! Gateway validation: bounds, axis-alignment.

use crate::io_wz::WzMap;

use super::types::{
    IssueLocation, ValidationCategory, ValidationConfig, ValidationResults, WarningRule,
};
use super::{push_problem, push_warning};

pub(super) fn validate_gateways(
    map: &WzMap,
    config: &ValidationConfig,
    results: &mut ValidationResults,
) {
    let cat = ValidationCategory::Gateways;
    let w = map.map_data.width;
    let h = map.map_data.height;
    let check_axis_aligned = config.is_enabled(WarningRule::GatewayNotAxisAligned);

    for (idx, gw) in map.map_data.gateways.iter().enumerate() {
        let x1 = gw.x1 as u32;
        let y1 = gw.y1 as u32;
        let x2 = gw.x2 as u32;
        let y2 = gw.y2 as u32;

        if x1 >= w || y1 >= h || x2 >= w || y2 >= h {
            push_problem(
                results,
                cat,
                format!(
                    "Gateway {idx} coordinates ({x1},{y1})-({x2},{y2}) are out of map bounds ({w}x{h}).",
                ),
                IssueLocation::TilePos {
                    x: x1.min(w.saturating_sub(1)),
                    y: y1.min(h.saturating_sub(1)),
                },
            );
        }

        if check_axis_aligned && x1 != x2 && y1 != y2 {
            push_warning(
                results,
                WarningRule::GatewayNotAxisAligned,
                cat,
                format!(
                    "Gateway {idx} ({x1},{y1})-({x2},{y2}) is not axis-aligned (must be horizontal or vertical).",
                ),
                IssueLocation::TilePos {
                    x: x1.min(w.saturating_sub(1)),
                    y: y1.min(h.saturating_sub(1)),
                },
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map_data::Gateway;
    use crate::validate::test_support::valid_map;
    use crate::validate::types::Severity;

    #[test]
    fn gateway_within_bounds_ok() {
        let mut map = valid_map(64, 64);
        map.map_data.gateways.push(Gateway {
            x1: 10,
            y1: 10,
            x2: 10,
            y2: 20,
        });
        let mut results = ValidationResults::default();
        validate_gateways(&map, &ValidationConfig::default(), &mut results);
        assert!(results.issues.is_empty(), "issues: {:?}", results.issues);
    }

    #[test]
    fn gateway_x1_out_of_bounds() {
        let mut map = valid_map(32, 32);
        map.map_data.gateways.push(Gateway {
            x1: 32,
            y1: 10,
            x2: 10,
            y2: 10,
        });
        let mut results = ValidationResults::default();
        validate_gateways(&map, &ValidationConfig::default(), &mut results);
        assert!(results
            .issues
            .iter()
            .any(|i| i.severity == Severity::Problem && i.message.contains("out of map bounds")));
    }

    #[test]
    fn gateway_y1_out_of_bounds() {
        let mut map = valid_map(32, 32);
        map.map_data.gateways.push(Gateway {
            x1: 10,
            y1: 32,
            x2: 10,
            y2: 10,
        });
        let mut results = ValidationResults::default();
        validate_gateways(&map, &ValidationConfig::default(), &mut results);
        assert!(results
            .issues
            .iter()
            .any(|i| i.severity == Severity::Problem && i.message.contains("out of map bounds")));
    }

    #[test]
    fn gateway_x2_out_of_bounds() {
        let mut map = valid_map(32, 32);
        map.map_data.gateways.push(Gateway {
            x1: 10,
            y1: 10,
            x2: 32,
            y2: 10,
        });
        let mut results = ValidationResults::default();
        validate_gateways(&map, &ValidationConfig::default(), &mut results);
        assert!(results
            .issues
            .iter()
            .any(|i| i.severity == Severity::Problem && i.message.contains("out of map bounds")));
    }

    #[test]
    fn gateway_y2_out_of_bounds() {
        let mut map = valid_map(32, 32);
        map.map_data.gateways.push(Gateway {
            x1: 10,
            y1: 10,
            x2: 10,
            y2: 32,
        });
        let mut results = ValidationResults::default();
        validate_gateways(&map, &ValidationConfig::default(), &mut results);
        assert!(results
            .issues
            .iter()
            .any(|i| i.severity == Severity::Problem && i.message.contains("out of map bounds")));
    }

    #[test]
    fn gateway_both_endpoints_out_of_bounds() {
        let mut map = valid_map(32, 32);
        map.map_data.gateways.push(Gateway {
            x1: 40,
            y1: 40,
            x2: 50,
            y2: 50,
        });
        let mut results = ValidationResults::default();
        validate_gateways(&map, &ValidationConfig::default(), &mut results);
        // Should produce exactly one bounds problem (not two)
        let bounds_issues: Vec<_> = results
            .issues
            .iter()
            .filter(|i| i.message.contains("out of map bounds"))
            .collect();
        assert_eq!(bounds_issues.len(), 1);
    }

    #[test]
    fn gateway_horizontal_ok() {
        let mut map = valid_map(64, 64);
        map.map_data.gateways.push(Gateway {
            x1: 10,
            y1: 15,
            x2: 20,
            y2: 15, // same y -> horizontal
        });
        let mut results = ValidationResults::default();
        validate_gateways(&map, &ValidationConfig::default(), &mut results);
        assert!(
            !results
                .issues
                .iter()
                .any(|i| i.message.contains("axis-aligned")),
            "issues: {:?}",
            results.issues
        );
    }

    #[test]
    fn gateway_vertical_ok() {
        let mut map = valid_map(64, 64);
        map.map_data.gateways.push(Gateway {
            x1: 15,
            y1: 10,
            x2: 15, // same x -> vertical
            y2: 20,
        });
        let mut results = ValidationResults::default();
        validate_gateways(&map, &ValidationConfig::default(), &mut results);
        assert!(
            !results
                .issues
                .iter()
                .any(|i| i.message.contains("axis-aligned"))
        );
    }

    #[test]
    fn gateway_diagonal_warns() {
        let mut map = valid_map(64, 64);
        map.map_data.gateways.push(Gateway {
            x1: 10,
            y1: 10,
            x2: 20,
            y2: 20, // x1!=x2 AND y1!=y2 -> diagonal
        });
        let mut results = ValidationResults::default();
        validate_gateways(&map, &ValidationConfig::default(), &mut results);
        assert!(
            results
                .issues
                .iter()
                .any(|i| i.message.contains("axis-aligned"))
        );
    }

    #[test]
    fn gateway_single_tile_ok() {
        let mut map = valid_map(64, 64);
        map.map_data.gateways.push(Gateway {
            x1: 10,
            y1: 10,
            x2: 10,
            y2: 10,
        });
        let mut results = ValidationResults::default();
        validate_gateways(&map, &ValidationConfig::default(), &mut results);
        assert!(results.issues.is_empty(), "issues: {:?}", results.issues);
    }

    #[test]
    fn multiple_gateways_each_validated() {
        let mut map = valid_map(32, 32);
        // Gateway 0: valid
        map.map_data.gateways.push(Gateway {
            x1: 5,
            y1: 5,
            x2: 5,
            y2: 10,
        });
        // Gateway 1: out of bounds
        map.map_data.gateways.push(Gateway {
            x1: 40,
            y1: 5,
            x2: 40,
            y2: 10,
        });
        // Gateway 2: diagonal
        map.map_data.gateways.push(Gateway {
            x1: 5,
            y1: 5,
            x2: 10,
            y2: 10,
        });
        let mut results = ValidationResults::default();
        validate_gateways(&map, &ValidationConfig::default(), &mut results);
        assert_eq!(results.issues.len(), 2); // one bounds + one diagonal
    }
}
