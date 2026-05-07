//! Game stats loader for Warzone 2100.

pub mod bodies;
pub mod database;
pub mod features;
mod loaders;
pub mod propulsion;
pub mod structures;
pub mod templates;
pub mod terrain_table;
pub mod turrets;
pub mod weapons;

pub use database::StatsDatabase;

/// Errors returned when loading game stats.
#[derive(Debug, thiserror::Error)]
pub enum StatsError {
    #[error("failed to read {path}: {source}")]
    Io {
        path: std::path::PathBuf,
        source: std::io::Error,
    },
    #[error("failed to parse {file}: {source}")]
    Parse {
        file: String,
        source: serde_json::Error,
    },
}
