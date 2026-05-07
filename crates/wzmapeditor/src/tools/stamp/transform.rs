//! Rotation, mirror, and scale transforms for stamp patterns.

use wz_maplib::constants::TILE_UNITS;
use wz_maplib::map_data::MapTile;

use super::pattern::{StampObject, StampPattern, StampTile};
use crate::tools::mirror::{rot_to_sa, sa_to_rot};

/// 90 degrees in WZ2100 direction units.
const DIR_QUARTER: u32 = 0x4000;
/// 180 degrees in WZ2100 direction units.
const DIR_HALF: u32 = 0x8000;
/// Full circle (360 degrees) in WZ2100 direction units.
const DIR_FULL: u32 = 0x1_0000;
/// Bitmask for the valid direction range (16-bit).
const DIR_MASK: u32 = 0xFFFF;

/// Rotate an object direction (0-65535) by a number of 90-degree CW steps.
fn rotate_direction(direction: u16, rot_steps: u8) -> u16 {
    let offset = (rot_steps as u32) * DIR_QUARTER;
    ((direction as u32).wrapping_add(offset) & DIR_MASK) as u16
}

/// Flip an object direction across the X axis (left/right mirror).
fn flip_direction_x(direction: u16) -> u16 {
    (DIR_FULL.wrapping_sub(direction as u32) & DIR_MASK) as u16
}

/// Flip an object direction across the Y axis (top/bottom mirror).
fn flip_direction_y(direction: u16) -> u16 {
    (DIR_HALF.wrapping_sub(direction as u32) & DIR_MASK) as u16
}

/// Transform a tile's packed texture orientation by composing a rotation and flip.
///
/// Decomposes the texture into SA-space, applies the transform, and reconstructs.
fn transform_tile_texture(texture: u16, rot_steps: u8, flip_x: bool, flip_y: bool) -> u16 {
    let tile = MapTile { height: 0, texture };
    let orig_rot = tile.rotation();
    let orig_xf = tile.x_flip();
    let orig_yf = tile.y_flip();
    let tri_flip = tile.tri_flip();
    let tex_id = tile.texture_id();

    let (sa, mut sxf, mut syf) = rot_to_sa(orig_rot, orig_xf, orig_yf);

    if flip_x {
        if sa {
            syf = !syf;
        } else {
            sxf = !sxf;
        }
    }
    if flip_y {
        if sa {
            sxf = !sxf;
        } else {
            syf = !syf;
        }
    }

    let (mut new_rot, new_xf, new_yf) = sa_to_rot(sa, sxf, syf);
    new_rot = (new_rot + rot_steps) % 4;

    MapTile::make_texture(tex_id, new_xf, new_yf, new_rot, tri_flip)
}

/// Rotate a tile position within the pattern grid after flips have been applied.
///
/// `(dx, dy)` are the already-flipped coordinates. `w` and `h` are the
/// original pattern dimensions (before rotation).
fn rotate_tile_pos(dx: u32, dy: u32, w: u32, h: u32, rot_steps: u8) -> (u32, u32) {
    match rot_steps % 4 {
        0 => (dx, dy),
        1 => (h - 1 - dy, dx),
        2 => (w - 1 - dx, h - 1 - dy),
        3 => (dy, w - 1 - dx),
        _ => unreachable!(),
    }
}

/// Transform an object's world-unit offset (relative to pattern origin).
///
/// Flips first, then rotates CW.
fn transform_object_offset(
    ox: i32,
    oy: i32,
    extent_x: i32,
    extent_y: i32,
    rot_steps: u8,
    flip_x: bool,
    flip_y: bool,
) -> (i32, i32) {
    let mut x = ox;
    let mut y = oy;

    if flip_x {
        x = extent_x - x;
    }
    if flip_y {
        y = extent_y - y;
    }

    match rot_steps % 4 {
        0 => (x, y),
        1 => (extent_y - y, x),
        2 => (extent_x - x, extent_y - y),
        3 => (y, extent_x - x),
        _ => unreachable!(),
    }
}

/// Transform an object direction: flips first, then rotation.
pub fn transform_object_direction(dir: u16, rot_steps: u8, flip_x: bool, flip_y: bool) -> u16 {
    let mut d = dir;
    if flip_x {
        d = flip_direction_x(d);
    }
    if flip_y {
        d = flip_direction_y(d);
    }
    rotate_direction(d, rot_steps)
}

/// Apply rotation and flip transforms to produce a new `StampPattern`.
///
/// Order: flips are applied first, then rotation. This matches the convention
/// used by the mirror system.
pub(super) fn transform_pattern(
    pattern: &StampPattern,
    rot_steps: u8,
    flip_x: bool,
    flip_y: bool,
) -> StampPattern {
    let w = pattern.width;
    let h = pattern.height;

    let extent_x = (w * TILE_UNITS) as i32;
    let extent_y = (h * TILE_UNITS) as i32;

    let mut new_tiles = Vec::with_capacity(pattern.tiles.len());
    for tile in &pattern.tiles {
        let mut dx = tile.dx;
        let mut dy = tile.dy;

        if flip_x {
            dx = w - 1 - dx;
        }
        if flip_y {
            dy = h - 1 - dy;
        }

        let (new_dx, new_dy) = rotate_tile_pos(dx, dy, w, h, rot_steps);
        let new_texture = transform_tile_texture(tile.texture, rot_steps, flip_x, flip_y);

        new_tiles.push(StampTile {
            dx: new_dx,
            dy: new_dy,
            texture: new_texture,
            height: tile.height,
        });
    }

    let mut new_objects = Vec::with_capacity(pattern.objects.len());
    for obj in &pattern.objects {
        let (ox, oy, dir) = obj.offset_dir();

        let (new_ox, new_oy) =
            transform_object_offset(ox, oy, extent_x, extent_y, rot_steps, flip_x, flip_y);
        let new_dir = transform_object_direction(dir, rot_steps, flip_x, flip_y);

        let new_obj = match obj {
            StampObject::Structure {
                name,
                player,
                modules,
                ..
            } => StampObject::Structure {
                name: name.clone(),
                offset_x: new_ox,
                offset_y: new_oy,
                direction: new_dir,
                player: *player,
                modules: *modules,
            },
            StampObject::Droid { name, player, .. } => StampObject::Droid {
                name: name.clone(),
                offset_x: new_ox,
                offset_y: new_oy,
                direction: new_dir,
                player: *player,
            },
            StampObject::Feature { name, player, .. } => StampObject::Feature {
                name: name.clone(),
                offset_x: new_ox,
                offset_y: new_oy,
                direction: new_dir,
                player: *player,
            },
        };
        new_objects.push(new_obj);
    }

    let (new_w, new_h) = if rot_steps % 2 == 1 { (h, w) } else { (w, h) };

    StampPattern {
        width: new_w,
        height: new_h,
        tiles: new_tiles,
        objects: new_objects,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transform_identity() {
        let pat = StampPattern {
            width: 2,
            height: 3,
            tiles: vec![
                StampTile {
                    dx: 0,
                    dy: 0,
                    texture: 10,
                    height: 50,
                },
                StampTile {
                    dx: 1,
                    dy: 2,
                    texture: 20,
                    height: 60,
                },
            ],
            objects: Vec::new(),
        };
        let result = transform_pattern(&pat, 0, false, false);
        assert_eq!(result.width, 2);
        assert_eq!(result.height, 3);
        assert_eq!(result.tiles[0].dx, 0);
        assert_eq!(result.tiles[0].dy, 0);
        assert_eq!(result.tiles[1].dx, 1);
        assert_eq!(result.tiles[1].dy, 2);
    }

    #[test]
    fn transform_90cw_swaps_dimensions() {
        let pat = StampPattern {
            width: 2,
            height: 3,
            tiles: vec![StampTile {
                dx: 0,
                dy: 0,
                texture: 10,
                height: 50,
            }],
            objects: Vec::new(),
        };
        let result = transform_pattern(&pat, 1, false, false);
        assert_eq!(result.width, 3);
        assert_eq!(result.height, 2);
        // (0, 0) rotated 90 CW in 2x3 -> (h-1-0, 0) = (2, 0)
        assert_eq!(result.tiles[0].dx, 2);
        assert_eq!(result.tiles[0].dy, 0);
    }

    #[test]
    fn transform_180_preserves_dimensions() {
        let pat = StampPattern {
            width: 3,
            height: 3,
            tiles: vec![StampTile {
                dx: 0,
                dy: 0,
                texture: 5,
                height: 10,
            }],
            objects: Vec::new(),
        };
        let result = transform_pattern(&pat, 2, false, false);
        assert_eq!(result.width, 3);
        assert_eq!(result.height, 3);
        assert_eq!(result.tiles[0].dx, 2);
        assert_eq!(result.tiles[0].dy, 2);
    }

    #[test]
    fn transform_flip_x() {
        let pat = StampPattern {
            width: 3,
            height: 2,
            tiles: vec![StampTile {
                dx: 0,
                dy: 0,
                texture: 5,
                height: 10,
            }],
            objects: Vec::new(),
        };
        let result = transform_pattern(&pat, 0, true, false);
        assert_eq!(result.width, 3);
        assert_eq!(result.tiles[0].dx, 2);
        assert_eq!(result.tiles[0].dy, 0);
    }

    #[test]
    fn transform_flip_y() {
        let pat = StampPattern {
            width: 3,
            height: 2,
            tiles: vec![StampTile {
                dx: 0,
                dy: 0,
                texture: 5,
                height: 10,
            }],
            objects: Vec::new(),
        };
        let result = transform_pattern(&pat, 0, false, true);
        assert_eq!(result.tiles[0].dx, 0);
        assert_eq!(result.tiles[0].dy, 1);
    }

    #[test]
    fn transform_270cw() {
        let pat = StampPattern {
            width: 2,
            height: 3,
            tiles: vec![StampTile {
                dx: 1,
                dy: 0,
                texture: 10,
                height: 50,
            }],
            objects: Vec::new(),
        };
        let result = transform_pattern(&pat, 3, false, false);
        assert_eq!(result.width, 3);
        assert_eq!(result.height, 2);
        assert_eq!(result.tiles[0].dx, 0);
        assert_eq!(result.tiles[0].dy, 0);
    }

    #[test]
    fn transform_flip_x_then_90cw() {
        let pat = StampPattern {
            width: 3,
            height: 2,
            tiles: vec![StampTile {
                dx: 0,
                dy: 0,
                texture: 5,
                height: 10,
            }],
            objects: Vec::new(),
        };
        let result = transform_pattern(&pat, 1, true, false);
        // flip_x first: (0,0) -> (2,0); then 90 CW in 3x2: (h-1-dy, dx) = (1, 2)
        assert_eq!(result.width, 2);
        assert_eq!(result.height, 3);
        assert_eq!(result.tiles[0].dx, 1);
        assert_eq!(result.tiles[0].dy, 2);
    }

    #[test]
    fn transform_object_positions() {
        let pat = StampPattern {
            width: 2,
            height: 2,
            tiles: Vec::new(),
            objects: vec![StampObject::Feature {
                name: "Tree1".into(),
                offset_x: 0,
                offset_y: 0,
                direction: 0,
                player: None,
            }],
        };

        let result = transform_pattern(&pat, 1, false, false);
        let StampObject::Feature {
            offset_x,
            offset_y,
            direction,
            ..
        } = &result.objects[0]
        else {
            unreachable!("transform_pattern preserves variant; input is Feature");
        };
        assert_eq!(*offset_x, 256);
        assert_eq!(*offset_y, 0);
        assert_eq!(*direction, 0x4000);
    }

    #[test]
    fn transform_tile_texture_rotation_composes() {
        let tex = MapTile::make_texture(42, false, false, 1, false);
        let result = transform_tile_texture(tex, 1, false, false);
        let tile = MapTile {
            height: 0,
            texture: result,
        };
        assert_eq!(tile.texture_id(), 42);
        assert_eq!(tile.rotation(), 2);
        assert!(!tile.x_flip());
        assert!(!tile.y_flip());
    }

    #[test]
    fn transform_tile_texture_flip_x_mirrors() {
        let tex = MapTile::make_texture(10, false, false, 0, false);
        let result = transform_tile_texture(tex, 0, true, false);
        let tile = MapTile {
            height: 0,
            texture: result,
        };
        assert_eq!(tile.texture_id(), 10);
        assert!(tile.x_flip());
    }

    #[test]
    fn transform_tile_texture_preserves_tri_flip() {
        let tex = MapTile::make_texture(7, false, false, 0, true);
        let result = transform_tile_texture(tex, 2, true, true);
        let tile = MapTile {
            height: 0,
            texture: result,
        };
        assert_eq!(tile.texture_id(), 7);
        assert!(tile.tri_flip());
    }

    #[test]
    fn transform_tile_texture_roundtrip_all_rotations() {
        for rot in 0..4u8 {
            for &xf in &[false, true] {
                let tex = MapTile::make_texture(10, xf, false, rot, false);
                let mut t = tex;
                for _ in 0..4 {
                    t = transform_tile_texture(t, 1, false, false);
                }
                assert_eq!(t, tex, "4x90 CW roundtrip failed for rot={rot}, xf={xf}");
            }
        }
    }

    #[test]
    fn transform_tile_texture_flip_x_then_90cw() {
        let tex = MapTile::make_texture(10, false, false, 0, false);
        let result = transform_tile_texture(tex, 1, true, false);
        let tile = MapTile {
            height: 0,
            texture: result,
        };
        assert_eq!(tile.texture_id(), 10);
        let mut t = tex;
        for _ in 0..4 {
            t = transform_tile_texture(t, 1, true, false);
        }
        assert_eq!(t, tex, "4x (flip_x + 90 CW) should return to original");
        assert_ne!(result, tex);
    }

    #[test]
    fn rotate_direction_90cw() {
        assert_eq!(rotate_direction(0, 1), 0x4000);
        assert_eq!(rotate_direction(0x4000, 1), 0x8000);
        assert_eq!(rotate_direction(0x8000, 1), 0xC000);
        assert_eq!(rotate_direction(0xC000, 1), 0);
    }

    #[test]
    fn flip_direction_x_cardinals() {
        assert_eq!(flip_direction_x(0), 0);
        assert_eq!(flip_direction_x(0x4000), 0xC000);
        assert_eq!(flip_direction_x(0x8000), 0x8000);
        assert_eq!(flip_direction_x(0xC000), 0x4000);
    }

    #[test]
    fn flip_direction_y_cardinals() {
        assert_eq!(flip_direction_y(0), 0x8000);
        assert_eq!(flip_direction_y(0x8000), 0);
        assert_eq!(flip_direction_y(0x4000), 0x4000);
        assert_eq!(flip_direction_y(0xC000), 0xC000);
    }

    #[test]
    fn transform_object_direction_flip_and_rotate() {
        assert_eq!(transform_object_direction(0, 1, true, false), 0x4000);
        assert_eq!(transform_object_direction(0x4000, 2, false, true), 0xC000);
        assert_eq!(transform_object_direction(0x8000, 3, true, true), 0xC000);
    }
}
