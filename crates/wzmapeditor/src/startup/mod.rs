//! Startup system: splash screen, background loading pipeline, progress.
//!
//! Coordinates all background work that must complete before the editor
//! is usable: base.wz extraction, tileset/stats loading, ground texture
//! caching, model preparation, and thumbnail generation.

pub mod loading_ui;
pub mod pipeline;
pub mod splash_ui;
pub mod task;
pub mod workers;

use std::collections::HashMap;
use std::sync::mpsc;

/// All in-flight background loading state for startup and mid-session reloads.
pub struct RuntimeTasks {
    /// Extraction progress in thousandths (0-1000) while base.wz extracts.
    pub extraction_progress: Option<std::sync::Arc<std::sync::atomic::AtomicU32>>,
    /// Delivers `Ok(data_dir)` or `Err(message)` when extraction finishes.
    pub extraction_rx: Option<task::TaskHandle<Result<std::path::PathBuf, String>>>,
    pub ground_texture_load: Option<GroundTextureLoadState>,
    pub ground_precache_rx: Option<mpsc::Receiver<GroundPrecacheResult>>,
    /// Pre-cache progress in thousandths (0-1000).
    pub ground_precache_progress: Option<std::sync::Arc<std::sync::atomic::AtomicU32>>,
    /// Latches once attempted, so we don't retry every frame.
    pub ground_precache_attempted: bool,
    /// Pre-cached ground data keyed by tileset name.
    pub precached_ground_data: HashMap<String, crate::viewport::ground_types::GroundData>,
    pub connector_precache_rx: Option<mpsc::Receiver<HashMap<String, Vec<glam::Vec3>>>>,
    pub map_model_load: Option<MapModelLoadState>,
    /// Latches once attempted, so we don't retry every frame on failure.
    pub stats_load_attempted: bool,
    /// Latches once attempted, so we don't retry every frame.
    pub tileset_load_attempted: bool,
}

impl RuntimeTasks {
    pub fn new() -> Self {
        Self {
            extraction_progress: None,
            extraction_rx: None,
            ground_texture_load: None,
            ground_precache_rx: None,
            ground_precache_progress: None,
            ground_precache_attempted: false,
            precached_ground_data: HashMap::new(),
            connector_precache_rx: None,
            map_model_load: None,
            stats_load_attempted: false,
            tileset_load_attempted: false,
        }
    }

    /// Extraction progress as a fraction in `[0.0, 1.0]`, or `None`.
    pub fn extraction_fraction(&self) -> Option<f32> {
        self.extraction_progress
            .as_ref()
            .map(|p| p.load(std::sync::atomic::Ordering::Relaxed) as f32 / 1000.0)
    }

    pub fn ground_precache_fraction(&self) -> Option<f32> {
        self.ground_precache_progress
            .as_ref()
            .map(|p| p.load(std::sync::atomic::Ordering::Relaxed) as f32 / 1000.0)
    }

    pub fn connectors_done(&self) -> bool {
        self.connector_precache_rx.is_none()
    }

    pub fn models_done(&self) -> bool {
        self.map_model_load.is_none()
    }

    pub fn model_fraction(&self) -> Option<f32> {
        self.map_model_load
            .as_ref()
            .map(|s| s.uploaded as f32 / s.total.max(1) as f32)
    }
}

/// Pre-decoded tileset images and atlas built off the main thread.
pub struct TilesetPayload {
    /// Pre-decoded tiles ready for `ctx.load_texture()`.
    pub tile_images: Vec<(u16, egui::ColorImage)>,
    pub source_dir: std::path::PathBuf,
    /// Flat RGBA atlas data ready for GPU upload.
    pub atlas: Option<crate::viewport::atlas::TileAtlas>,
}

/// Metadata that travels with a loaded map from the background thread.
pub struct LoadMapMeta {
    /// Source path (persisted for auto-reload on next launch).
    pub source_path: Option<std::path::PathBuf>,
    /// Writable location for Ctrl+S quick-save.
    pub save_path: Option<std::path::PathBuf>,
    /// Archive prefix for multi-map .wz files.
    pub archive_prefix: Option<String>,
}

/// Payload from the background ground texture loader. Each buffer is
/// flat RGBA; per-layer count matches across diffuse/normal/specular.
pub struct GroundTexturePayload {
    pub diffuse: Vec<u8>,
    /// Empty if normal maps are unavailable.
    pub normals: Vec<u8>,
    /// Empty if specular maps are unavailable.
    pub specular: Vec<u8>,
    /// One layer per tile index.
    pub decal_diffuse: Vec<u8>,
    /// Empty if decal normal maps are unavailable.
    pub decal_normal: Vec<u8>,
    /// Empty if decal specular maps are unavailable.
    pub decal_specular: Vec<u8>,
    pub num_decal_tiles: u32,
}

pub(crate) struct GroundPrecacheResult {
    pub message: String,
    /// Parsed ground data per tileset, keyed by name (e.g. "arizona").
    pub ground_data: HashMap<String, crate::viewport::ground_types::GroundData>,
}

/// Background ground texture loading + chunked GPU upload state.
///
/// Once the worker delivers `GroundTexturePayload`, GPU uploads happen
/// one chunk per frame (7 steps) so the UI stays responsive.
pub struct GroundTextureLoadState {
    pub receiver: mpsc::Receiver<GroundTexturePayload>,
    /// Needed for GPU upload and terrain mesh.
    pub ground_data: crate::viewport::ground_types::GroundData,
    /// Worker progress (0..1000). During upload this is repurposed to
    /// 1001..2000, mapped back to 0.0..1.0 in the UI.
    pub progress: std::sync::Arc<std::sync::atomic::AtomicU32>,
    /// Payload waiting for GPU upload, populated on first `try_recv`.
    pub payload: Option<GroundTexturePayload>,
    /// Current upload step (0..=6). `None` means still awaiting payload.
    pub upload_step: Option<u32>,
    pub upload_views: GroundUploadViews,
}

/// Intermediate wgpu `TextureView`s accumulated during chunked upload.
/// Each field is populated by one upload step and consumed when the
/// final bind group is assembled in step 6.
#[derive(Default)]
pub struct GroundUploadViews {
    pub high_diffuse: Option<wgpu::TextureView>,
    pub high_normal: Option<wgpu::TextureView>,
    pub high_specular: Option<wgpu::TextureView>,
    pub decal_diffuse: Option<wgpu::TextureView>,
    pub decal_normal: Option<wgpu::TextureView>,
    pub decal_specular: Option<wgpu::TextureView>,
}

impl std::fmt::Debug for GroundTextureLoadState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GroundTextureLoadState")
            .finish_non_exhaustive()
    }
}

/// Incremental model loading state for map objects.
///
/// Models are prepared (disk I/O, parse, mesh build) on background
/// threads and delivered via channel. The main thread polls each frame
/// and does the cheap GPU upload.
pub struct MapModelLoadState {
    pub receiver: mpsc::Receiver<crate::viewport::model_loader::PreparedModel>,
    pub total: usize,
    pub uploaded: usize,
}

impl std::fmt::Debug for MapModelLoadState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MapModelLoadState")
            .field("total", &self.total)
            .field("uploaded", &self.uploaded)
            .finish_non_exhaustive()
    }
}
