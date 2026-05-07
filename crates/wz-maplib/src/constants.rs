//! Game constants and coordinate conversion utilities.

/// The shift on a world coordinate to get the tile coordinate.
pub const TILE_SHIFT: u32 = 7;

/// The number of units across a tile.
pub const TILE_UNITS: u32 = 1 << TILE_SHIFT;

/// Tile spacing in world units as f32, for rendering calculations.
pub const TILE_UNITS_F32: f32 = TILE_UNITS as f32;

/// The mask to get internal tile coords from a full coordinate.
pub const TILE_MASK: u32 = 0x7f;

/// Multiplier for tile height in pre-v40 formats (byte heights are scaled by this).
pub const ELEVATION_SCALE: u16 = 2;

/// Maximum map width/height.
pub const MAP_MAX_WIDTH: u32 = 256;
pub const MAP_MAX_HEIGHT: u32 = 256;
pub const MAP_MAX_AREA: u64 = 256 * 256;

/// Maximum tile height.
pub const TILE_MAX_HEIGHT: u16 = 255 * ELEVATION_SCALE;

// Texture flag layout from `map_types.h`. Bits 9-10 are unused.
pub const TILE_XFLIP: u16 = 0x8000;
pub const TILE_YFLIP: u16 = 0x4000;
/// Two-bit rotation index (0-3, each step = 90 degrees CW), bits 13-12.
pub const TILE_ROTMASK: u16 = 0x3000;
pub const TILE_ROTSHIFT: u16 = 12;
/// Triangle diagonal flip (swaps quad split direction).
pub const TILE_TRIFLIP: u16 = 0x0800;
/// Nine-bit texture index (0-511).
pub const TILE_NUMMASK: u16 = 0x01ff;

/// game.map versions.
pub const MAP_VERSION_OLD_MAX: u32 = 39;
pub const MAP_VERSION_FULL_HEIGHT: u32 = 40;
pub const MAP_VERSION_CURRENT: u32 = MAP_VERSION_FULL_HEIGHT;
pub const MAP_VERSION_MIN_SUPPORTED: u32 = 10;

/// Gateway version (always 1).
pub const GATEWAY_VERSION: u32 = 1;

/// The player number for scavengers in object files.
pub const PLAYER_SCAVENGERS: i8 = -1;

/// Maximum number of players in the game (matches C++ `MAX_PLAYERS` in map.cpp).
pub const MAX_PLAYERS: u8 = 11;

/// Minimum distance from map edge in tiles for valid object placement (WZ2100: `TOO_NEAR_EDGE`).
pub const TOO_NEAR_EDGE: u32 = 3;

/// Maximum terrain slope (height difference) for building placement (WZ2100: `MAX_INCLINE`).
pub const MAX_INCLINE: u16 = 50;

/// Maximum map dimension for `.wz` export (WZ2100 editor convention; absolute max is 256).
pub const MAP_MAX_WZ_EXPORT: u32 = 250;

/// Maximum structures of any one type per player.
pub const MAX_STRUCTURES_PER_TYPE: usize = 255;

/// Maximum map name length for `.wz` format.
pub const MAP_NAME_MAX_LEN: usize = 16;

/// Maximum direction value (WZ2100 uses 0-65535 for 0-360 degrees).
pub const DIRECTION_MAX: u16 = 65535;

/// Convert a WZ2100 direction (0-65535) to radians.
pub fn direction_to_radians(direction: u16) -> f32 {
    (direction as f32 / (DIRECTION_MAX as f32 + 1.0)) * std::f32::consts::TAU
}

/// Convert a WZ2100 direction (0-65535) to degrees.
pub fn direction_to_degrees(direction: u16) -> f32 {
    (direction as f32 / (DIRECTION_MAX as f32 + 1.0)) * 360.0
}

/// Convert a tile coordinate to a world coordinate.
pub fn world_coord(map_coord: i32) -> i32 {
    map_coord.wrapping_shl(TILE_SHIFT)
}

/// Convert a world coordinate to a tile coordinate.
pub fn map_coord(world_coord: i32) -> i32 {
    world_coord >> TILE_SHIFT
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn world_coord_conversion() {
        assert_eq!(world_coord(0), 0);
        assert_eq!(world_coord(1), TILE_UNITS as i32); // 128
        assert_eq!(world_coord(5), 5 * TILE_UNITS as i32);
    }

    #[test]
    fn map_coord_conversion() {
        assert_eq!(map_coord(0), 0);
        assert_eq!(map_coord(128), 1);
        assert_eq!(map_coord(640), 5); // 640 >> 7 = 5
        assert_eq!(map_coord(127), 0); // sub-tile
    }

    #[test]
    fn world_map_coord_roundtrip() {
        for tile in 0..10 {
            let world = world_coord(tile);
            assert_eq!(map_coord(world), tile);
        }
    }

    #[test]
    fn direction_to_degrees_cardinal() {
        assert!((direction_to_degrees(0) - 0.0).abs() < 0.1);
        // 90° = 65536/4 = 16384
        assert!((direction_to_degrees(16384) - 90.0).abs() < 0.1);
        // 180° = 32768
        assert!((direction_to_degrees(32768) - 180.0).abs() < 0.1);
        // 270° = 49152
        assert!((direction_to_degrees(49152) - 270.0).abs() < 0.1);
    }

    #[test]
    fn direction_to_radians_cardinal() {
        assert!((direction_to_radians(0) - 0.0).abs() < 0.01);
        assert!((direction_to_radians(16384) - std::f32::consts::FRAC_PI_2).abs() < 0.01);
        assert!((direction_to_radians(32768) - std::f32::consts::PI).abs() < 0.01);
    }

    #[test]
    #[expect(
        clippy::erasing_op,
        reason = "0 & TILE_MASK is the test's edge case, not a bug"
    )]
    fn tile_mask_isolates_sub_tile_bits() {
        assert_eq!(0u32 & TILE_MASK, 0);
        assert_eq!(127u32 & TILE_MASK, 127);
        assert_eq!(128u32 & TILE_MASK, 0); // exactly one tile
        assert_eq!(0xC8u32 & TILE_MASK, 72); // 200 - 128 = 72
    }

    #[test]
    fn tile_flag_bits_dont_overlap() {
        assert_eq!(TILE_XFLIP & TILE_YFLIP, 0);
        assert_eq!(TILE_XFLIP & TILE_ROTMASK, 0);
        assert_eq!(TILE_XFLIP & TILE_TRIFLIP, 0);
        assert_eq!(TILE_XFLIP & TILE_NUMMASK, 0);
        assert_eq!(TILE_YFLIP & TILE_ROTMASK, 0);
        assert_eq!(TILE_YFLIP & TILE_TRIFLIP, 0);
        assert_eq!(TILE_YFLIP & TILE_NUMMASK, 0);
        assert_eq!(TILE_ROTMASK & TILE_TRIFLIP, 0);
        assert_eq!(TILE_ROTMASK & TILE_NUMMASK, 0);
        assert_eq!(TILE_TRIFLIP & TILE_NUMMASK, 0);
    }

    #[test]
    fn all_bits_accounted_for() {
        // Bits 9-10 (0x0600) are unused by WZ2100, leaving 0xF9FF.
        let all = TILE_XFLIP | TILE_YFLIP | TILE_ROTMASK | TILE_TRIFLIP | TILE_NUMMASK;
        assert_eq!(all, 0xF9FF);
    }
}
