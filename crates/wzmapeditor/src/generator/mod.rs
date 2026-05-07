//! Procedural map generator for WZ2100 multiplayer maps. Combines a passage
//! node network for gameplay-driven height levels, fractal-noise terrain with
//! erosion, mirror-symmetric player placement, and coherent-noise auto-texturing.

pub mod dialog;
pub mod heightmap;
pub mod nodes;
pub mod pipeline;
pub mod placement;
pub mod terrain;
pub mod texturing;

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

use crate::config::Tileset;
use crate::tools::MirrorMode;

/// All parameters for procedural map generation.
#[derive(Debug, Clone)]
pub struct GeneratorConfig {
    // -- Layout --
    /// Map name (without the Nc- prefix).
    pub map_name: String,
    /// Map width in tiles (48..=250).
    pub width: u32,
    /// Map height in tiles (48..=250).
    pub height: u32,
    /// Tileset: Arizona, Urban, or Rockies.
    pub tileset: Tileset,
    /// Number of players (2..=10).
    pub players: u8,
    /// Symmetry mode for terrain and object placement.
    pub symmetry: MirrorMode,

    // -- Terrain --
    /// Number of discrete height levels (3..=5).
    pub height_levels: u8,
    /// Base height level for player starts (-1 = random, 0..=(levels-1)).
    pub base_level: i8,
    /// Probability of height level changing between adjacent nodes (0.0..=1.0).
    pub level_frequency: f32,
    /// Diamond-square noise amplitude (0.0..=1.0).
    pub height_variation: f32,
    /// Post-process flattening factor (0.0..=1.0).
    pub flatness: f32,
    /// Number of water bodies to spawn (0..=5).
    pub water_spawns: u8,

    // -- Resources --
    /// Oil derricks per player base (0..=16).
    pub base_oil: u8,
    /// Extra scattered oil derricks (0..=99).
    pub extra_oil: u8,
    /// Minimum derricks per oil cluster.
    pub oil_cluster_min: u8,
    /// Maximum derricks per oil cluster.
    pub oil_cluster_max: u8,
    /// Constructor droids per player (0..=15).
    pub trucks_per_player: u8,

    // -- Features --
    /// Whether to scatter decorative features (rocks, boulders, trees, wrecks, ruins).
    pub scatter_features: bool,
    /// Density of scattered features (0.0..=1.0).
    pub feature_density: f32,
    /// Number of scattered oil drums (pickup power) (0..=30).
    pub oil_drums: u8,

    // -- Scavengers --
    /// Whether to place scavenger bases scattered across the map.
    pub scavengers: bool,
    /// Number of scavenger base clusters to place (0..=8).
    pub scavenger_bases: u8,

    // -- RNG --
    /// Random seed (0 = pick a random seed).
    pub seed: u64,
}

impl Default for GeneratorConfig {
    fn default() -> Self {
        Self {
            map_name: "GeneratedMap".to_string(),
            width: 128,
            height: 128,
            tileset: Tileset::Arizona,
            players: 2,
            symmetry: MirrorMode::Vertical,
            height_levels: 4,
            base_level: -1,
            level_frequency: 0.6,
            height_variation: 0.7,
            flatness: 0.3,
            water_spawns: 0,
            base_oil: 4,
            extra_oil: 6,
            oil_cluster_min: 1,
            oil_cluster_max: 3,
            trucks_per_player: 4,
            scatter_features: true,
            feature_density: 0.3,
            oil_drums: 6,
            scavengers: true,
            scavenger_bases: 3,
            seed: 0,
        }
    }
}

/// Result type for the generator pipeline. Currently infallible, but uses
/// `anyhow::Result` so future failure paths can attach context without
/// inventing a dedicated error enum.
pub type GeneratorResult = anyhow::Result<wz_maplib::io_wz::WzMap>;

/// Thread-safe progress reporter for the background generation thread.
pub struct ProgressReporter {
    /// Progress as thousandths (0-1000).
    progress: Arc<AtomicU32>,
    /// Current step description.
    label: Arc<Mutex<String>>,
}

impl ProgressReporter {
    pub fn new(progress: Arc<AtomicU32>, label: Arc<Mutex<String>>) -> Self {
        Self { progress, label }
    }

    /// Update progress (0.0..=1.0) and step label.
    pub fn set(&self, step: &str, fraction: f32) {
        if let Ok(mut l) = self.label.lock() {
            *l = step.to_string();
        }
        self.progress
            .store((fraction * 1000.0) as u32, Ordering::Relaxed);
    }
}
