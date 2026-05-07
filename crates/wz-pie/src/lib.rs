//! PIE 3D model format parser for Warzone 2100 (v2/v3/v4).

pub mod constants;
pub mod parser;
pub mod types;

pub use parser::parse_pie;
pub use types::{PieLevel, PieModel, PiePolygon};

/// Errors returned when parsing PIE model files.
#[derive(Debug, thiserror::Error)]
pub enum PieError {
    #[error("invalid PIE header: {0}")]
    InvalidHeader(String),
    #[error("unsupported PIE version {version} (supported {min}-{max})")]
    UnsupportedVersion { version: u32, min: u32, max: u32 },
    #[error("unexpected end of file in {section}")]
    UnexpectedEof { section: String },
    #[error("invalid vertex data: {0}")]
    InvalidVertex(String),
    #[error("invalid polygon data: {0}")]
    InvalidPolygon(String),
    #[error("parse error: {0}")]
    Parse(String),
}
