//! Binary game.map format reader and writer.

use std::io::{Cursor, Read, Write};

use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};

use crate::MapError;
use crate::constants::*;
use crate::map_data::{Gateway, MapData, MapTile};

/// Output format version for writing game.map files.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    /// FlaME-compatible / old binary format (version 10, byte heights).
    Ver1BinaryOld,
    /// JSON-era format (version 39, byte heights).
    Ver2,
    /// Latest format (version 40, full-range u16 heights).
    Ver3,
}

impl OutputFormat {
    fn map_version(self) -> u32 {
        match self {
            OutputFormat::Ver1BinaryOld => 10,
            OutputFormat::Ver2 => MAP_VERSION_OLD_MAX,
            OutputFormat::Ver3 => MAP_VERSION_FULL_HEIGHT,
        }
    }
}

/// Read tile data from a cursor for the given format version.
fn read_tiles(
    cursor: &mut Cursor<&[u8]>,
    version: u32,
    num_tiles: usize,
) -> Result<Vec<MapTile>, MapError> {
    let mut tiles = Vec::with_capacity(num_tiles);

    if version >= MAP_VERSION_FULL_HEIGHT {
        // v40+: texture (u16 LE) + height (u16 LE).
        for _ in 0..num_tiles {
            let texture = cursor
                .read_u16::<LittleEndian>()
                .map_err(|e| MapError::Io {
                    context: "reading tile".into(),
                    source: e,
                })?;
            let height = cursor
                .read_u16::<LittleEndian>()
                .map_err(|e| MapError::Io {
                    context: "reading tile height".into(),
                    source: e,
                })?;
            if height > TILE_MAX_HEIGHT {
                return Err(MapError::TileHeightOverflow {
                    height,
                    max: TILE_MAX_HEIGHT,
                });
            }
            tiles.push(MapTile { height, texture });
        }
    } else {
        // v10-39: texture (u16 LE) + height (u8, scaled by ELEVATION_SCALE).
        for _ in 0..num_tiles {
            let texture = cursor
                .read_u16::<LittleEndian>()
                .map_err(|e| MapError::Io {
                    context: "reading tile".into(),
                    source: e,
                })?;
            let raw_height = cursor.read_u8().map_err(|e| MapError::Io {
                context: "reading tile height".into(),
                source: e,
            })?;
            let height = raw_height as u16 * ELEVATION_SCALE;
            tiles.push(MapTile { height, texture });
        }
    }

    Ok(tiles)
}

/// Read gateway data from a cursor.
fn read_gateways(cursor: &mut Cursor<&[u8]>) -> Result<Vec<Gateway>, MapError> {
    let gw_version = cursor
        .read_u32::<LittleEndian>()
        .map_err(|e| MapError::Io {
            context: "reading gateway version".into(),
            source: e,
        })?;
    let num_gateways = cursor
        .read_u32::<LittleEndian>()
        .map_err(|e| MapError::Io {
            context: "reading gateway count".into(),
            source: e,
        })?;
    if gw_version != GATEWAY_VERSION {
        return Err(MapError::BadGatewayVersion {
            got: gw_version,
            expected: GATEWAY_VERSION,
        });
    }

    let mut gateways = Vec::with_capacity(num_gateways as usize);
    for _ in 0..num_gateways {
        let x1 = cursor.read_u8().map_err(|e| MapError::Io {
            context: "reading gateway".into(),
            source: e,
        })?;
        let y1 = cursor.read_u8().map_err(|e| MapError::Io {
            context: "reading gateway".into(),
            source: e,
        })?;
        let x2 = cursor.read_u8().map_err(|e| MapError::Io {
            context: "reading gateway".into(),
            source: e,
        })?;
        let y2 = cursor.read_u8().map_err(|e| MapError::Io {
            context: "reading gateway".into(),
            source: e,
        })?;
        gateways.push(Gateway { x1, y1, x2, y2 });
    }

    Ok(gateways)
}

/// Read a game.map file from bytes.
pub fn read_game_map(data: &[u8]) -> Result<MapData, MapError> {
    let mut cursor = Cursor::new(data);

    let mut magic = [0u8; 4];
    cursor.read_exact(&mut magic).map_err(|e| MapError::Io {
        context: "reading map header".into(),
        source: e,
    })?;
    if &magic != b"map " {
        return Err(MapError::InvalidHeader(magic));
    }

    let version = cursor
        .read_u32::<LittleEndian>()
        .map_err(|e| MapError::Io {
            context: "reading map version".into(),
            source: e,
        })?;
    if !(MAP_VERSION_MIN_SUPPORTED..=MAP_VERSION_CURRENT).contains(&version) {
        return Err(MapError::UnsupportedVersion {
            version,
            min: MAP_VERSION_MIN_SUPPORTED,
            max: MAP_VERSION_CURRENT,
        });
    }

    let width = cursor
        .read_u32::<LittleEndian>()
        .map_err(|e| MapError::Io {
            context: "reading map width".into(),
            source: e,
        })?;
    let height = cursor
        .read_u32::<LittleEndian>()
        .map_err(|e| MapError::Io {
            context: "reading map height".into(),
            source: e,
        })?;

    if (width as u64) * (height as u64) > MAP_MAX_AREA || width <= 1 || height <= 1 {
        return Err(MapError::InvalidDimensions { width, height });
    }

    let num_tiles = (width * height) as usize;
    let tiles = read_tiles(&mut cursor, version, num_tiles)?;
    let gateways = read_gateways(&mut cursor)?;

    Ok(MapData {
        width,
        height,
        tiles,
        gateways,
    })
}

/// Write a game.map file to bytes.
pub fn write_game_map(map: &MapData, format: OutputFormat) -> Result<Vec<u8>, MapError> {
    let version = format.map_version();
    let num_tiles = (map.width * map.height) as usize;

    if num_tiles != map.tiles.len() {
        return Err(MapError::TileMismatch {
            width: map.width,
            height: map.height,
            count: map.tiles.len(),
        });
    }
    if (map.width as u64) * (map.height as u64) > MAP_MAX_AREA || map.width <= 1 || map.height <= 1
    {
        return Err(MapError::InvalidDimensions {
            width: map.width,
            height: map.height,
        });
    }

    // header(16) + tiles(num * 3 or 4) + gw_header(8) + gws(num * 4).
    let tile_size = if version >= MAP_VERSION_FULL_HEIGHT {
        4
    } else {
        3
    };
    let capacity = 16 + num_tiles * tile_size + 8 + map.gateways.len() * 4;
    let mut buf = Vec::with_capacity(capacity);

    let io_err = |e: std::io::Error| MapError::Io {
        context: "writing map".into(),
        source: e,
    };

    buf.write_all(b"map ").map_err(&io_err)?;
    buf.write_u32::<LittleEndian>(version).map_err(&io_err)?;
    buf.write_u32::<LittleEndian>(map.width).map_err(&io_err)?;
    buf.write_u32::<LittleEndian>(map.height).map_err(&io_err)?;

    if version >= MAP_VERSION_FULL_HEIGHT {
        for tile in &map.tiles {
            if tile.height > TILE_MAX_HEIGHT {
                return Err(MapError::TileHeightOverflow {
                    height: tile.height,
                    max: TILE_MAX_HEIGHT,
                });
            }
            buf.write_u16::<LittleEndian>(tile.texture)
                .map_err(&io_err)?;
            buf.write_u16::<LittleEndian>(tile.height)
                .map_err(&io_err)?;
        }
    } else {
        for tile in &map.tiles {
            let max_old = 255u16 * ELEVATION_SCALE;
            if tile.height > max_old {
                return Err(MapError::TileHeightOverflow {
                    height: tile.height,
                    max: max_old,
                });
            }
            let byte_height = (tile.height / ELEVATION_SCALE) as u8;
            buf.write_u16::<LittleEndian>(tile.texture)
                .map_err(&io_err)?;
            buf.write_u8(byte_height).map_err(&io_err)?;
        }
    }

    buf.write_u32::<LittleEndian>(GATEWAY_VERSION)
        .map_err(&io_err)?;
    buf.write_u32::<LittleEndian>(map.gateways.len() as u32)
        .map_err(&io_err)?;

    for gw in &map.gateways {
        buf.write_u8(gw.x1).map_err(&io_err)?;
        buf.write_u8(gw.y1).map_err(&io_err)?;
        buf.write_u8(gw.x2).map_err(&io_err)?;
        buf.write_u8(gw.y2).map_err(&io_err)?;
    }

    Ok(buf)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_roundtrip_ver3() {
        let mut map = MapData::new(4, 4);
        map.tiles[0].height = 100;
        map.tiles[0].texture = 42;
        map.tiles[5].height = 200;
        map.gateways.push(Gateway {
            x1: 0,
            y1: 0,
            x2: 3,
            y2: 0,
        });

        let bytes = write_game_map(&map, OutputFormat::Ver3).unwrap();
        let loaded = read_game_map(&bytes).unwrap();

        assert_eq!(loaded.width, 4);
        assert_eq!(loaded.height, 4);
        assert_eq!(loaded.tiles[0].height, 100);
        assert_eq!(loaded.tiles[0].texture, 42);
        assert_eq!(loaded.tiles[5].height, 200);
        assert_eq!(loaded.gateways.len(), 1);
        assert_eq!(loaded.gateways[0].x1, 0);
        assert_eq!(loaded.gateways[0].x2, 3);
    }

    #[test]
    fn test_roundtrip_old_format() {
        let mut map = MapData::new(4, 4);
        map.tiles[0].height = 100; // 100 / 2 = 50 as byte, * 2 = 100
        map.tiles[3].height = 510; // 510 / 2 = 255 as byte, * 2 = 510

        let bytes = write_game_map(&map, OutputFormat::Ver2).unwrap();
        let loaded = read_game_map(&bytes).unwrap();

        assert_eq!(loaded.tiles[0].height, 100);
        assert_eq!(loaded.tiles[3].height, 510);
    }

    #[test]
    fn test_invalid_header() {
        let data = b"bad data here";
        assert!(read_game_map(data).is_err());
    }

    #[test]
    fn test_invalid_version_too_low() {
        let mut buf = Vec::new();
        buf.write_all(b"map ").unwrap();
        buf.write_u32::<LittleEndian>(5).unwrap(); // version below MIN_SUPPORTED=10
        buf.write_u32::<LittleEndian>(4).unwrap(); // width
        buf.write_u32::<LittleEndian>(4).unwrap(); // height
        let err = read_game_map(&buf).unwrap_err();
        assert!(matches!(
            err,
            MapError::UnsupportedVersion { version: 5, .. }
        ));
    }

    #[test]
    fn test_invalid_version_too_high() {
        let mut buf = Vec::new();
        buf.write_all(b"map ").unwrap();
        buf.write_u32::<LittleEndian>(99).unwrap();
        buf.write_u32::<LittleEndian>(4).unwrap();
        buf.write_u32::<LittleEndian>(4).unwrap();
        let err = read_game_map(&buf).unwrap_err();
        assert!(matches!(
            err,
            MapError::UnsupportedVersion { version: 99, .. }
        ));
    }

    #[test]
    fn test_invalid_dimensions_zero() {
        let mut buf = Vec::new();
        buf.write_all(b"map ").unwrap();
        buf.write_u32::<LittleEndian>(MAP_VERSION_CURRENT).unwrap();
        buf.write_u32::<LittleEndian>(0).unwrap(); // width=0
        buf.write_u32::<LittleEndian>(4).unwrap();
        let err = read_game_map(&buf).unwrap_err();
        assert!(matches!(err, MapError::InvalidDimensions { .. }));
    }

    #[test]
    fn test_invalid_dimensions_one() {
        let mut buf = Vec::new();
        buf.write_all(b"map ").unwrap();
        buf.write_u32::<LittleEndian>(MAP_VERSION_CURRENT).unwrap();
        buf.write_u32::<LittleEndian>(1).unwrap();
        buf.write_u32::<LittleEndian>(1).unwrap();
        let err = read_game_map(&buf).unwrap_err();
        assert!(matches!(err, MapError::InvalidDimensions { .. }));
    }

    #[test]
    fn test_invalid_dimensions_too_large() {
        let mut buf = Vec::new();
        buf.write_all(b"map ").unwrap();
        buf.write_u32::<LittleEndian>(MAP_VERSION_CURRENT).unwrap();
        buf.write_u32::<LittleEndian>(300).unwrap(); // > 256
        buf.write_u32::<LittleEndian>(300).unwrap();
        let err = read_game_map(&buf).unwrap_err();
        assert!(matches!(err, MapError::InvalidDimensions { .. }));
    }

    #[test]
    fn test_write_tile_mismatch() {
        let mut map = MapData::new(4, 4);
        map.tiles.pop();
        let err = write_game_map(&map, OutputFormat::Ver3).unwrap_err();
        assert!(matches!(err, MapError::TileMismatch { .. }));
    }

    #[test]
    fn test_write_height_overflow_ver3() {
        let mut map = MapData::new(2, 2);
        map.tiles[0].height = TILE_MAX_HEIGHT + 1;
        let err = write_game_map(&map, OutputFormat::Ver3).unwrap_err();
        assert!(matches!(err, MapError::TileHeightOverflow { .. }));
    }

    #[test]
    fn test_write_height_overflow_old_format() {
        let mut map = MapData::new(2, 2);
        // Old format max is 255 * ELEVATION_SCALE = 510.
        map.tiles[0].height = 511;
        let err = write_game_map(&map, OutputFormat::Ver2).unwrap_err();
        assert!(matches!(err, MapError::TileHeightOverflow { .. }));
    }

    #[test]
    fn test_gateway_roundtrip() {
        let mut map = MapData::new(4, 4);
        map.gateways.push(Gateway {
            x1: 0,
            y1: 0,
            x2: 3,
            y2: 0,
        });
        map.gateways.push(Gateway {
            x1: 1,
            y1: 1,
            x2: 2,
            y2: 3,
        });

        let bytes = write_game_map(&map, OutputFormat::Ver3).unwrap();
        let loaded = read_game_map(&bytes).unwrap();

        assert_eq!(loaded.gateways.len(), 2);
        assert_eq!(loaded.gateways[1].x1, 1);
        assert_eq!(loaded.gateways[1].y2, 3);
    }

    #[test]
    fn test_empty_gateways() {
        let map = MapData::new(2, 2);
        let bytes = write_game_map(&map, OutputFormat::Ver3).unwrap();
        let loaded = read_game_map(&bytes).unwrap();
        assert!(loaded.gateways.is_empty());
    }

    #[test]
    fn test_bad_gateway_version() {
        let map = MapData::new(2, 2);
        let mut bytes = write_game_map(&map, OutputFormat::Ver3).unwrap();
        // Gateway version sits at offset header(16) + tiles(2*2*4=16) = 32.
        bytes[32] = 2;
        let err = read_game_map(&bytes).unwrap_err();
        assert!(matches!(err, MapError::BadGatewayVersion { got: 2, .. }));
    }

    #[test]
    fn test_texture_flags_preserved() {
        let mut map = MapData::new(2, 2);
        map.tiles[0].texture = MapTile::make_texture(42, true, false, 2, true);
        map.tiles[1].texture = MapTile::make_texture(77, false, true, 1, false);

        let bytes = write_game_map(&map, OutputFormat::Ver3).unwrap();
        let loaded = read_game_map(&bytes).unwrap();

        assert_eq!(loaded.tiles[0].texture_id(), 42);
        assert!(loaded.tiles[0].x_flip());
        assert!(!loaded.tiles[0].y_flip());
        assert_eq!(loaded.tiles[0].rotation(), 2);
        assert!(loaded.tiles[0].tri_flip());

        assert_eq!(loaded.tiles[1].texture_id(), 77);
        assert!(!loaded.tiles[1].x_flip());
        assert!(loaded.tiles[1].y_flip());
        assert_eq!(loaded.tiles[1].rotation(), 1);
        assert!(!loaded.tiles[1].tri_flip());
    }

    #[test]
    fn test_ver1_format_roundtrip() {
        let mut map = MapData::new(2, 2);
        map.tiles[0].height = 128; // 128 / 2 = 64 as byte
        let bytes = write_game_map(&map, OutputFormat::Ver1BinaryOld).unwrap();
        let loaded = read_game_map(&bytes).unwrap();
        assert_eq!(loaded.tiles[0].height, 128);
    }

    #[test]
    fn test_all_formats_produce_loadable_output() {
        let mut map = MapData::new(2, 2);
        map.tiles[0].height = 100;
        map.tiles[0].texture = 42;

        for format in [
            OutputFormat::Ver1BinaryOld,
            OutputFormat::Ver2,
            OutputFormat::Ver3,
        ] {
            let bytes = write_game_map(&map, format).unwrap();
            let loaded = read_game_map(&bytes).unwrap();
            assert_eq!(loaded.tiles[0].height, 100, "height failed for {format:?}");
            assert_eq!(loaded.tiles[0].texture, 42, "texture failed for {format:?}");
        }
    }
}
