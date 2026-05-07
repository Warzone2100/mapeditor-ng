//! Core data types for parsed PIE models.

use glam::{Vec2, Vec3};

/// A complete PIE model with one or more LOD levels.
#[derive(Debug, Clone)]
pub struct PieModel {
    pub version: u32,
    pub model_type: u32,
    pub texture_page: String,
    pub texture_width: u32,
    pub texture_height: u32,
    /// Tileset-specific texture pages: index 0 = Arizona, 1 = Urban, 2 = Rockies.
    pub texture_pages: Vec<String>,
    /// `TCMask` texture pages (team color mask) per tileset index.
    pub tcmask_pages: Vec<String>,
    pub normal_page: Option<String>,
    pub specular_page: Option<String>,
    pub event_page: Option<String>,
    pub levels: Vec<PieLevel>,
}

/// A single LOD level within a PIE model.
#[derive(Debug, Clone)]
pub struct PieLevel {
    pub vertices: Vec<Vec3>,
    pub polygons: Vec<PiePolygon>,
    pub connectors: Vec<Vec3>,
}

/// A polygon (triangle) in a PIE model.
#[derive(Debug, Clone)]
pub struct PiePolygon {
    pub flags: u32,
    pub indices: Vec<u16>,
    pub tex_coords: Vec<Vec2>,
    pub anim_frames: Option<u32>,
    pub anim_rate: Option<u32>,
    pub anim_width: Option<f32>,
    pub anim_height: Option<f32>,
}

impl PiePolygon {
    pub fn has_texture(&self) -> bool {
        self.flags & crate::constants::PIE_TEX != 0
    }

    pub fn has_tex_anim(&self) -> bool {
        self.flags & crate::constants::PIE_TEXANIM != 0
    }
}

impl PieModel {
    pub fn has_tcmask(&self) -> bool {
        self.model_type & crate::constants::PIE_TCMASK != 0
    }
}
