//! Pure Rust I/O library for Warzone 2100 map formats.

pub mod constants;
pub mod io_binary;
pub mod io_bjo;
pub mod io_json;
pub mod io_lev;
pub mod io_ttp;
pub mod io_wz;
pub mod labels;
pub mod map_data;
pub mod objects;
pub mod terrain_types;
pub mod validate;

pub use io_binary::OutputFormat;
pub use io_wz::{MapPreview, ResizeReport, Weather, WzMap};
pub use labels::ScriptLabel;
pub use map_data::{Gateway, MapData, MapTile};
pub use objects::{Droid, Feature, Structure, WorldPos};
pub use terrain_types::{TerrainType, TerrainTypeData};
pub use validate::{ValidationConfig, WarningRule};

/// Errors returned by map I/O operations.
#[derive(Debug, thiserror::Error)]
pub enum MapError {
    /// The binary map header was invalid.
    #[error("invalid map header: expected 'map ', got {0:?}")]
    InvalidHeader([u8; 4]),
    /// The map version is not supported.
    #[error("unsupported map version {version} (supported {min}..={max})")]
    UnsupportedVersion { version: u32, min: u32, max: u32 },
    /// The map dimensions exceed limits.
    #[error("map dimensions out of range: {width}x{height}")]
    InvalidDimensions { width: u32, height: u32 },
    /// A tile height exceeded the maximum.
    #[error("tile height {height} exceeds maximum {max}")]
    TileHeightOverflow { height: u16, max: u16 },
    /// Tile count does not match map dimensions.
    #[error("map dimensions {width}x{height} don't match tile count {count}")]
    TileMismatch {
        width: u32,
        height: u32,
        count: usize,
    },
    /// The gateway version was unexpected.
    #[error("bad gateway version: {got} (expected {expected})")]
    BadGatewayVersion { got: u32, expected: u32 },
    /// The TTP header or version was invalid.
    #[error("invalid TTP data: {0}")]
    InvalidTtp(String),
    /// A JSON parsing error occurred.
    #[error("JSON error in {file}: {source}")]
    Json {
        file: String,
        #[source]
        source: serde_json::Error,
    },
    /// A JSON structure error (missing field, bad format).
    #[error("{0}")]
    JsonFormat(String),
    /// An I/O error occurred.
    #[error("{context}: {source}")]
    Io {
        context: String,
        #[source]
        source: std::io::Error,
    },
    /// A zip archive error occurred.
    #[error("archive error: {0}")]
    Zip(#[from] zip::result::ZipError),
    /// No game.map found in a .wz archive.
    #[error("no game.map found in .wz archive")]
    NoGameMap,
}
