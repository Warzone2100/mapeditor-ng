//! Map object types: structures, droids, and features.

use serde::{Deserialize, Serialize};

/// A world position in WZ2100 coordinate space.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct WorldPos {
    pub x: u32,
    pub y: u32,
}

/// A placed structure on the map.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Structure {
    pub name: String,
    pub position: WorldPos,
    #[serde(default)]
    pub direction: u16,
    #[serde(default)]
    pub player: i8,
    #[serde(default)]
    pub modules: u8,
    #[serde(default)]
    pub id: Option<u32>,
}

/// A placed droid on the map.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Droid {
    pub name: String,
    pub position: WorldPos,
    #[serde(default)]
    pub direction: u16,
    #[serde(default)]
    pub player: i8,
    #[serde(default)]
    pub id: Option<u32>,
}

/// A placed feature on the map.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Feature {
    pub name: String,
    pub position: WorldPos,
    #[serde(default)]
    pub direction: u16,
    #[serde(default)]
    pub id: Option<u32>,
    #[serde(default)]
    pub player: Option<i8>,
}
