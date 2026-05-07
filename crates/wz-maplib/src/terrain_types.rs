//! Terrain type classification for tile textures.

use serde::{Deserialize, Serialize};

pub const TERRAIN_TYPE_COUNT: usize = 12;

/// Terrain type classification for tiles.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u16)]
pub enum TerrainType {
    Sand = 0,
    SandYellow = 1,
    Bakedearth = 2,
    GreenMud = 3,
    RedBrush = 4,
    PinkRock = 5,
    Road = 6,
    Water = 7,
    Cliffface = 8,
    Rubble = 9,
    SheetIce = 10,
    Slush = 11,
}

impl From<u16> for TerrainType {
    fn from(val: u16) -> Self {
        match val {
            0 => Self::Sand,
            1 => Self::SandYellow,
            2 => Self::Bakedearth,
            3 => Self::GreenMud,
            4 => Self::RedBrush,
            5 => Self::PinkRock,
            6 => Self::Road,
            7 => Self::Water,
            8 => Self::Cliffface,
            9 => Self::Rubble,
            10 => Self::SheetIce,
            11 => Self::Slush,
            other => {
                log::warn!("Unknown terrain type {other}, defaulting to Sand");
                Self::Sand
            }
        }
    }
}

/// Terrain type data: maps each texture tile to a terrain type.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerrainTypeData {
    pub terrain_types: Vec<TerrainType>,
}

impl TerrainTypeData {
    pub fn new() -> Self {
        Self {
            terrain_types: Vec::new(),
        }
    }
}

impl Default for TerrainTypeData {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terrain_type_from_valid_values() {
        assert_eq!(TerrainType::from(0), TerrainType::Sand);
        assert_eq!(TerrainType::from(6), TerrainType::Road);
        assert_eq!(TerrainType::from(7), TerrainType::Water);
        assert_eq!(TerrainType::from(8), TerrainType::Cliffface);
        assert_eq!(TerrainType::from(11), TerrainType::Slush);
    }

    #[test]
    fn terrain_type_from_unknown_defaults_to_sand() {
        assert_eq!(TerrainType::from(12), TerrainType::Sand);
        assert_eq!(TerrainType::from(255), TerrainType::Sand);
        assert_eq!(TerrainType::from(u16::MAX), TerrainType::Sand);
    }

    #[test]
    fn terrain_type_roundtrip_as_u16() {
        for i in 0..12u16 {
            let tt = TerrainType::from(i);
            assert_eq!(tt as u16, i);
        }
    }
}
