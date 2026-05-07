//! Shared test utilities for validation tests.

use std::collections::HashMap;

use crate::io_wz::WzMap;
use crate::objects::{Droid, Feature, Structure, WorldPos};
use crate::terrain_types::{TerrainType, TerrainTypeData};

use super::types::{FeatureInfo, StatsLookup, StructureInfo, TemplateInfo};

pub(super) struct MockStats {
    structures: HashMap<String, StructureInfo>,
    features: HashMap<String, FeatureInfo>,
    templates: HashMap<String, TemplateInfo>,
}

impl MockStats {
    pub(super) fn new() -> Self {
        Self {
            structures: HashMap::new(),
            features: HashMap::new(),
            templates: HashMap::new(),
        }
    }

    pub(super) fn with_structure(mut self, name: &str, stype: &str, w: u32, b: u32) -> Self {
        self.structures.insert(
            name.to_string(),
            StructureInfo {
                structure_type: Some(stype.to_string()),
                width: w,
                breadth: b,
            },
        );
        self
    }

    pub(super) fn with_feature(mut self, name: &str, ftype: &str) -> Self {
        self.features.insert(
            name.to_string(),
            FeatureInfo {
                feature_type: Some(ftype.to_string()),
            },
        );
        self
    }

    pub(super) fn with_template(mut self, name: &str, dtype: &str, has_construct: bool) -> Self {
        self.templates.insert(
            name.to_string(),
            TemplateInfo {
                droid_type: Some(dtype.to_string()),
                has_construct,
            },
        );
        self
    }
}

impl StatsLookup for MockStats {
    fn structure_info(&self, name: &str) -> Option<StructureInfo> {
        self.structures.get(name).cloned()
    }
    fn feature_info(&self, name: &str) -> Option<FeatureInfo> {
        self.features.get(name).cloned()
    }
    fn template_info(&self, name: &str) -> Option<TemplateInfo> {
        self.templates.get(name).cloned()
    }
}

pub(super) fn make_structure(name: &str, x: u32, y: u32, player: i8) -> Structure {
    Structure {
        name: name.to_string(),
        position: WorldPos { x, y },
        direction: 0,
        player,
        modules: 0,
        id: None,
    }
}

pub(super) fn make_structure_with_id(name: &str, x: u32, y: u32, player: i8, id: u32) -> Structure {
    Structure {
        name: name.to_string(),
        position: WorldPos { x, y },
        direction: 0,
        player,
        modules: 0,
        id: Some(id),
    }
}

pub(super) fn make_droid(name: &str, x: u32, y: u32, player: i8) -> Droid {
    Droid {
        name: name.to_string(),
        position: WorldPos { x, y },
        direction: 0,
        player,
        id: None,
    }
}

pub(super) fn make_droid_with_id(name: &str, x: u32, y: u32, player: i8, id: u32) -> Droid {
    Droid {
        name: name.to_string(),
        position: WorldPos { x, y },
        direction: 0,
        player,
        id: Some(id),
    }
}

pub(super) fn make_feature(name: &str, x: u32, y: u32) -> Feature {
    Feature {
        name: name.to_string(),
        position: WorldPos { x, y },
        direction: 0,
        id: None,
        player: None,
    }
}

pub(super) fn make_feature_with_id(name: &str, x: u32, y: u32, id: u32) -> Feature {
    Feature {
        name: name.to_string(),
        position: WorldPos { x, y },
        direction: 0,
        id: Some(id),
        player: None,
    }
}

pub(super) fn make_feature_with_player(name: &str, x: u32, y: u32, player: i8) -> Feature {
    Feature {
        name: name.to_string(),
        position: WorldPos { x, y },
        direction: 0,
        id: None,
        player: Some(player),
    }
}

/// Create a basic valid map with TTP data.
pub(super) fn valid_map(width: u32, height: u32) -> WzMap {
    let mut map = WzMap::new("TestMap", width, height);
    // Add minimal TTP data so the "no terrain types" check passes
    map.terrain_types = Some(TerrainTypeData {
        terrain_types: vec![TerrainType::Sand; 78],
    });
    map
}

/// Create a map configured for multiplayer validation (with stats).
pub(super) fn multiplayer_map(players: u8) -> WzMap {
    let mut map = valid_map(64, 64);
    map.players = players;
    map
}
