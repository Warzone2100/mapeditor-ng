//! PIE model flags and version constants.

// Model flags from upstream WZ2100 imd.h.

/// Premultiplied implies additive.
pub const PIE_NO_ADDITIVE: u32 = 0x0000_0001;
pub const PIE_ADDITIVE: u32 = 0x0000_0002;
pub const PIE_PREMULTIPLIED: u32 = 0x0000_0004;

/// Pitch to camera implies roll to camera.
pub const PIE_ROLL_TO_CAMERA: u32 = 0x0000_0010;
pub const PIE_PITCH_TO_CAMERA: u32 = 0x0000_0020;

pub const PIE_NOSTRETCH: u32 = 0x0000_1000;
pub const PIE_TCMASK: u32 = 0x0001_0000;

pub const PIE_TEX: u32 = 0x0000_0200;
pub const PIE_TEXANIM: u32 = 0x0000_4000;

pub const PIE_VER_2: u32 = 2;
pub const PIE_VER_3: u32 = 3;
pub const PIE_VER_4: u32 = 4;
pub const PIE_MIN_VER: u32 = PIE_VER_2;
pub const PIE_MAX_VER: u32 = PIE_VER_4;
