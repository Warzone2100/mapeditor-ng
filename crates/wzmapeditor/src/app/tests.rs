//! Unit tests for the application module.

use super::testing::{TEST_AI_SCRIPT, build_skirmish_config};
use super::types::*;

#[test]
fn skirmish_config_has_correct_map_name() {
    let json = build_skirmish_config("my_test_map", 2);
    let parsed: serde_json::Value = serde_json::from_str(&json).expect("valid JSON");
    assert_eq!(parsed["challenge"]["map"], "my_test_map");
}

#[test]
fn skirmish_config_clamps_to_min_players() {
    let json = build_skirmish_config("test", 1);
    let parsed: serde_json::Value = serde_json::from_str(&json).expect("valid JSON");
    assert_eq!(parsed["challenge"]["maxPlayers"], 2);
    assert!(parsed["player_0"].is_object());
    assert!(parsed["player_1"].is_object());
    assert_eq!(parsed["player_1"]["ai"], TEST_AI_SCRIPT);
}

#[test]
fn skirmish_config_fills_ai_slots() {
    let json = build_skirmish_config("test", 4);
    let parsed: serde_json::Value = serde_json::from_str(&json).expect("valid JSON");
    assert_eq!(parsed["challenge"]["maxPlayers"], 4);
    assert!(parsed["player_0"]["ai"].is_null());
    for i in 1..4u8 {
        let key = format!("player_{i}");
        assert_eq!(parsed[&key]["ai"], TEST_AI_SCRIPT, "missing AI for {key}");
        assert_eq!(parsed[&key]["team"], i, "wrong team for {key}");
    }
}

#[test]
fn skirmish_config_player_0_is_team_0() {
    let json = build_skirmish_config("test", 2);
    let parsed: serde_json::Value = serde_json::from_str(&json).expect("valid JSON");
    assert_eq!(parsed["player_0"]["team"], 0);
}

#[test]
fn campaign_prefix_detected() {
    let prefixes = [
        "wrf/cam1/cam1a/",
        "wrf/cam2/cam2-1s/",
        "wrf/cam3/cam3a/",
        "wrf/tutorial/tut1/",
        "wrf/fastplay/fp1/",
    ];
    for prefix in &prefixes {
        assert!(
            prefix.starts_with("wrf/"),
            "expected campaign prefix: {prefix}"
        );
    }
}

#[test]
fn skirmish_prefix_not_campaign() {
    let prefixes = ["", "multiplay/maps/2c-Startup/", "multiplay/maps/4c-rush/"];
    for prefix in &prefixes {
        assert!(
            !prefix.starts_with("wrf/"),
            "should not be campaign: {prefix}"
        );
    }
}

#[test]
fn selection_set_single() {
    let mut sel = Selection::default();
    assert!(sel.is_empty());

    sel.set_single(SelectedObject::Structure(0));
    assert_eq!(sel.len(), 1);
    assert_eq!(sel.single(), Some(SelectedObject::Structure(0)));
}

#[test]
fn selection_toggle() {
    let mut sel = Selection::default();
    sel.set_single(SelectedObject::Structure(0));

    sel.toggle(SelectedObject::Droid(1));
    assert_eq!(sel.len(), 2);
    assert!(sel.contains(&SelectedObject::Structure(0)));
    assert!(sel.contains(&SelectedObject::Droid(1)));
    assert!(sel.single().is_none());

    sel.toggle(SelectedObject::Structure(0));
    assert_eq!(sel.len(), 1);
    assert!(!sel.contains(&SelectedObject::Structure(0)));
    assert_eq!(sel.single(), Some(SelectedObject::Droid(1)));
}

#[test]
fn selection_add_no_duplicates() {
    let mut sel = Selection::default();
    sel.add(SelectedObject::Feature(5));
    sel.add(SelectedObject::Feature(5));
    assert_eq!(sel.len(), 1);
}

#[test]
fn selection_clear() {
    let mut sel = Selection::default();
    sel.set_single(SelectedObject::Structure(0));
    sel.add(SelectedObject::Droid(1));
    sel.clear();
    assert!(sel.is_empty());
    assert_eq!(sel.single(), None);
}

#[test]
fn selection_single_returns_none_for_multi() {
    let mut sel = Selection::default();
    sel.add(SelectedObject::Structure(0));
    sel.add(SelectedObject::Structure(1));
    assert_eq!(sel.single(), None);
    assert_eq!(sel.len(), 2);
}

#[test]
fn enforce_group_keeps_majority() {
    let mut sel = Selection::default();
    sel.add(SelectedObject::Structure(0));
    sel.add(SelectedObject::Structure(1));
    sel.add(SelectedObject::Feature(0));
    sel.enforce_group();
    assert_eq!(sel.len(), 2);
    assert!(sel.contains(&SelectedObject::Structure(0)));
    assert!(!sel.contains(&SelectedObject::Feature(0)));
}

#[test]
fn enforce_group_structures_and_droids_coexist() {
    let mut sel = Selection::default();
    sel.add(SelectedObject::Structure(0));
    sel.add(SelectedObject::Droid(0));
    sel.enforce_group();
    assert_eq!(sel.len(), 2);
}

#[test]
fn enforce_group_tie_uses_last_added() {
    let mut sel = Selection::default();
    sel.add(SelectedObject::Structure(0));
    sel.add(SelectedObject::Feature(0));
    sel.enforce_group();
    assert_eq!(sel.len(), 1);
    assert!(sel.contains(&SelectedObject::Feature(0)));
}
