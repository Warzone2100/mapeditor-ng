//! Core map data structures: tiles, gateways, and terrain grid.

use serde::{Deserialize, Serialize};

use crate::constants::*;

/// Core map data: terrain tiles and gateways.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MapData {
    pub width: u32,
    pub height: u32,
    pub tiles: Vec<MapTile>,
    pub gateways: Vec<Gateway>,
}

impl MapData {
    /// Create a new empty map with the given dimensions.
    pub fn new(width: u32, height: u32) -> Self {
        let num_tiles = (width * height) as usize;
        Self {
            width,
            height,
            tiles: vec![MapTile::default(); num_tiles],
            gateways: Vec::new(),
        }
    }

    /// Get a tile at the given coordinates, or None if out of bounds.
    pub fn tile(&self, x: u32, y: u32) -> Option<&MapTile> {
        if x < self.width && y < self.height {
            Some(&self.tiles[(y * self.width + x) as usize])
        } else {
            None
        }
    }

    /// Get a mutable tile at the given coordinates, or None if out of bounds.
    pub fn tile_mut(&mut self, x: u32, y: u32) -> Option<&mut MapTile> {
        if x < self.width && y < self.height {
            Some(&mut self.tiles[(y * self.width + x) as usize])
        } else {
            None
        }
    }

    /// Build a copy of this map at `(new_width, new_height)`, with content offset
    /// by `(offset_x, offset_y)` source-tile units.
    ///
    /// `offset` is the source coordinate that lands at the destination's `(0, 0)`.
    /// Positive values crop from the source's top-left; negative values pad the
    /// destination's top-left with default tiles. Tiles outside the overlap are
    /// left as `MapTile::default()`. Gateways shift by `-offset` and are dropped
    /// when any endpoint falls outside `[0, new_width) x [0, new_height)` or
    /// exceeds the gateway u8 coordinate range.
    #[must_use]
    pub fn resized(&self, new_width: u32, new_height: u32, offset_x: i32, offset_y: i32) -> Self {
        let mut out = Self::new(new_width, new_height);

        let nw = i64::from(new_width);
        let nh = i64::from(new_height);
        let ow = i64::from(self.width);
        let oh = i64::from(self.height);
        let ox = i64::from(offset_x);
        let oy = i64::from(offset_y);

        // Destination overlap range. The .max(start) guard prevents a negative
        // range when the shift moves the source entirely off the destination.
        let start_dx = (-ox).max(0);
        let end_dx = (ow - ox).min(nw).max(start_dx);
        let start_dy = (-oy).max(0);
        let end_dy = (oh - oy).min(nh).max(start_dy);

        for dy in start_dy..end_dy {
            for dx in start_dx..end_dx {
                let sx = (dx + ox) as u32;
                let sy = (dy + oy) as u32;
                let src = self.tiles[(sy * self.width + sx) as usize];
                let dst_idx = (dy as u32 * new_width + dx as u32) as usize;
                out.tiles[dst_idx] = src;
            }
        }

        let max_u8 = i64::from(u8::MAX);
        for gw in &self.gateways {
            let nx1 = i64::from(gw.x1) - ox;
            let ny1 = i64::from(gw.y1) - oy;
            let nx2 = i64::from(gw.x2) - ox;
            let ny2 = i64::from(gw.y2) - oy;
            let in_bounds =
                |x: i64, y: i64| x >= 0 && y >= 0 && x < nw && y < nh && x <= max_u8 && y <= max_u8;
            if in_bounds(nx1, ny1) && in_bounds(nx2, ny2) {
                out.gateways.push(Gateway {
                    x1: nx1 as u8,
                    y1: ny1 as u8,
                    x2: nx2 as u8,
                    y2: ny2 as u8,
                });
            }
        }

        out
    }
}

/// A single map tile.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct MapTile {
    /// Height at the top-left vertex of this tile.
    pub height: u16,
    /// Texture ID + flip/rotation flags packed into a u16.
    pub texture: u16,
}

impl MapTile {
    /// Extract the texture number (0-511).
    pub fn texture_id(&self) -> u16 {
        self.texture & TILE_NUMMASK
    }

    /// Whether the texture is flipped on the X axis.
    pub fn x_flip(&self) -> bool {
        self.texture & TILE_XFLIP != 0
    }

    /// Whether the texture is flipped on the Y axis.
    pub fn y_flip(&self) -> bool {
        self.texture & TILE_YFLIP != 0
    }

    /// Rotation value (0-3, representing 0/90/180/270 degrees).
    pub fn rotation(&self) -> u8 {
        ((self.texture & TILE_ROTMASK) >> TILE_ROTSHIFT) as u8
    }

    /// Whether the triangle diagonal is flipped.
    pub fn tri_flip(&self) -> bool {
        self.texture & TILE_TRIFLIP != 0
    }

    /// Build a texture value from components.
    pub fn make_texture(id: u16, x_flip: bool, y_flip: bool, rotation: u8, tri_flip: bool) -> u16 {
        let mut val = id & TILE_NUMMASK;
        if x_flip {
            val |= TILE_XFLIP;
        }
        if y_flip {
            val |= TILE_YFLIP;
        }
        val |= ((rotation as u16) << TILE_ROTSHIFT) & TILE_ROTMASK;
        if tri_flip {
            val |= TILE_TRIFLIP;
        }
        val
    }
}

/// A gateway between two tile positions.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Gateway {
    pub x1: u8,
    pub y1: u8,
    pub x2: u8,
    pub y2: u8,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_creates_correct_size() {
        let map = MapData::new(8, 6);
        assert_eq!(map.width, 8);
        assert_eq!(map.height, 6);
        assert_eq!(map.tiles.len(), 48);
        assert!(map.gateways.is_empty());
    }

    #[test]
    fn tile_in_bounds() {
        let map = MapData::new(4, 4);
        assert!(map.tile(0, 0).is_some());
        assert!(map.tile(3, 3).is_some());
    }

    #[test]
    fn tile_out_of_bounds() {
        let map = MapData::new(4, 4);
        assert!(map.tile(4, 0).is_none());
        assert!(map.tile(0, 4).is_none());
        assert!(map.tile(100, 100).is_none());
    }

    #[test]
    fn tile_mut_modifies() {
        let mut map = MapData::new(4, 4);
        map.tile_mut(2, 3).unwrap().height = 500;
        assert_eq!(map.tile(2, 3).unwrap().height, 500);
    }

    #[test]
    fn tile_mut_out_of_bounds() {
        let mut map = MapData::new(4, 4);
        assert!(map.tile_mut(4, 0).is_none());
    }

    #[test]
    fn texture_id_extracts_lower_bits() {
        let tile = MapTile {
            height: 0,
            texture: 0x89AB,
        };
        // 0x89AB & TILE_NUMMASK (0x01FF) = 0x01AB.
        assert_eq!(tile.texture_id(), 0x01AB);
    }

    #[test]
    fn x_flip_bit() {
        let tile = MapTile {
            height: 0,
            texture: TILE_XFLIP,
        };
        assert!(tile.x_flip());
        assert!(!tile.y_flip());
    }

    #[test]
    fn y_flip_bit() {
        let tile = MapTile {
            height: 0,
            texture: TILE_YFLIP,
        };
        assert!(!tile.x_flip());
        assert!(tile.y_flip());
    }

    #[test]
    fn rotation_values() {
        for rot in 0..4u8 {
            let tex = (rot as u16) << TILE_ROTSHIFT;
            let tile = MapTile {
                height: 0,
                texture: tex,
            };
            assert_eq!(tile.rotation(), rot, "rotation {rot} failed");
        }
    }

    #[test]
    fn tri_flip_bit() {
        let tile = MapTile {
            height: 0,
            texture: TILE_TRIFLIP,
        };
        assert!(tile.tri_flip());
        let tile2 = MapTile {
            height: 0,
            texture: 0,
        };
        assert!(!tile2.tri_flip());
    }

    #[test]
    fn make_texture_roundtrip_all_combinations() {
        for id in [0u16, 1, 42, 77, 511] {
            for x_flip in [false, true] {
                for y_flip in [false, true] {
                    for rot in 0..4u8 {
                        for tri_flip in [false, true] {
                            let tex = MapTile::make_texture(id, x_flip, y_flip, rot, tri_flip);
                            let tile = MapTile {
                                height: 0,
                                texture: tex,
                            };
                            assert_eq!(
                                tile.texture_id(),
                                id,
                                "id mismatch for {id},{x_flip},{y_flip},{rot},{tri_flip}"
                            );
                            assert_eq!(tile.x_flip(), x_flip, "x_flip mismatch");
                            assert_eq!(tile.y_flip(), y_flip, "y_flip mismatch");
                            assert_eq!(tile.rotation(), rot, "rotation mismatch");
                            assert_eq!(tile.tri_flip(), tri_flip, "tri_flip mismatch");
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn make_texture_masks_id_to_9_bits() {
        // 0xFFFF must mask down to 0x01FF.
        let tex = MapTile::make_texture(0xFFFF, false, false, 0, false);
        let tile = MapTile {
            height: 0,
            texture: tex,
        };
        assert_eq!(tile.texture_id(), 511);
    }

    #[test]
    fn new_map_tiles_are_zeroed() {
        // The editor's new-map flow relies on Default producing zeroed tiles.
        let map = MapData::new(2, 2);
        for tile in &map.tiles {
            assert_eq!(tile.height, 0);
            assert_eq!(tile.texture, 0);
        }
    }

    fn fill_tiles(map: &mut MapData) {
        let w = map.width;
        let h = map.height;
        for y in 0..h {
            for x in 0..w {
                let v = (y * w + x + 1) as u16;
                let tile = map.tile_mut(x, y).unwrap();
                tile.height = v;
                tile.texture = v;
            }
        }
    }

    #[test]
    fn resized_identity_zero_offset_returns_clone() {
        let mut src = MapData::new(8, 6);
        fill_tiles(&mut src);
        src.gateways.push(Gateway {
            x1: 1,
            y1: 2,
            x2: 1,
            y2: 5,
        });

        let out = src.resized(8, 6, 0, 0);
        assert_eq!(out.width, 8);
        assert_eq!(out.height, 6);
        assert_eq!(out.tiles.len(), src.tiles.len());
        for (a, b) in out.tiles.iter().zip(src.tiles.iter()) {
            assert_eq!(a.height, b.height);
            assert_eq!(a.texture, b.texture);
        }
        assert_eq!(out.gateways.len(), 1);
        assert_eq!(out.gateways[0].x1, 1);
        assert_eq!(out.gateways[0].y2, 5);
    }

    #[test]
    fn resized_shrink_topleft_keeps_corner() {
        let mut src = MapData::new(8, 8);
        fill_tiles(&mut src);

        let out = src.resized(4, 4, 0, 0);
        assert_eq!(out.width, 4);
        assert_eq!(out.height, 4);
        assert_eq!(
            out.tile(0, 0).unwrap().height,
            src.tile(0, 0).unwrap().height
        );
        assert_eq!(
            out.tile(3, 3).unwrap().height,
            src.tile(3, 3).unwrap().height
        );
        assert!(out.tile(5, 5).is_none());
    }

    #[test]
    fn resized_grow_anchor_center_pads_with_default() {
        let mut src = MapData::new(4, 4);
        fill_tiles(&mut src);

        // Grow 4x4 -> 8x8 with center anchor: offset = (-2, -2).
        let out = src.resized(8, 8, -2, -2);
        assert_eq!(out.width, 8);
        assert_eq!(out.height, 8);

        for sy in 0..4 {
            for sx in 0..4 {
                let s = src.tile(sx, sy).unwrap();
                let d = out.tile(sx + 2, sy + 2).unwrap();
                assert_eq!(s.height, d.height, "tile ({sx},{sy}) lost in grow");
                assert_eq!(s.texture, d.texture);
            }
        }
        for x in 0..8 {
            assert_eq!(out.tile(x, 0).unwrap().height, 0);
            assert_eq!(out.tile(x, 7).unwrap().height, 0);
        }
        for y in 0..8 {
            assert_eq!(out.tile(0, y).unwrap().height, 0);
            assert_eq!(out.tile(7, y).unwrap().height, 0);
        }
    }

    #[test]
    fn resized_full_shift_no_overlap_yields_blank() {
        let mut src = MapData::new(8, 8);
        fill_tiles(&mut src);

        // Offset of (100, 100) into an 8x8 result has no overlap.
        let out = src.resized(8, 8, 100, 100);
        assert_eq!(out.tiles.len(), 64);
        for tile in &out.tiles {
            assert_eq!(tile.height, 0);
            assert_eq!(tile.texture, 0);
        }
    }

    #[test]
    fn resized_gateway_inside_kept_shifted() {
        let mut src = MapData::new(16, 16);
        src.gateways.push(Gateway {
            x1: 5,
            y1: 5,
            x2: 5,
            y2: 10,
        });

        // Crop 4 from top/left: gateway at (5,5)..(5,10) -> (1,1)..(1,6).
        let out = src.resized(8, 8, 4, 4);
        assert_eq!(out.gateways.len(), 1);
        assert_eq!(out.gateways[0].x1, 1);
        assert_eq!(out.gateways[0].y1, 1);
        assert_eq!(out.gateways[0].x2, 1);
        assert_eq!(out.gateways[0].y2, 6);
    }

    #[test]
    fn resized_gateway_partial_dropped() {
        let mut src = MapData::new(16, 16);
        // Straddles the crop boundary; should be dropped.
        src.gateways.push(Gateway {
            x1: 2,
            y1: 2,
            x2: 12,
            y2: 2,
        });
        // Fully inside; should survive.
        src.gateways.push(Gateway {
            x1: 5,
            y1: 5,
            x2: 7,
            y2: 5,
        });

        let out = src.resized(8, 8, 0, 0);
        assert_eq!(out.gateways.len(), 1);
        assert_eq!(out.gateways[0].x1, 5);
        assert_eq!(out.gateways[0].x2, 7);
    }
}
