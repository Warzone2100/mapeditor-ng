//! Terrain type palette (ttypes.ttp) format reader and writer.

use std::io::{Cursor, Read, Write};

use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};

use crate::MapError;
use crate::terrain_types::{TerrainType, TerrainTypeData};

const TTP_MAGIC: &[u8; 4] = b"ttyp";

/// Minimum TTP version accepted by WZ2100 (VER1/VER2 maps write version 8).
const TTP_VERSION_MIN: u32 = 7;
/// Maximum TTP version accepted by WZ2100 (VER3 maps write version 39).
const TTP_VERSION_MAX: u32 = 39;
/// Default TTP version written by this library.
const TTP_VERSION_WRITE: u32 = 8;

/// Read terrain type data from a ttypes.ttp file.
pub fn read_ttp(data: &[u8]) -> Result<TerrainTypeData, MapError> {
    let mut cursor = Cursor::new(data);

    let mut magic = [0u8; 4];
    cursor.read_exact(&mut magic).map_err(|e| MapError::Io {
        context: "reading TTP header".into(),
        source: e,
    })?;
    if &magic != TTP_MAGIC {
        return Err(MapError::InvalidTtp(format!(
            "expected 'ttyp', got {magic:?}"
        )));
    }

    let version = cursor
        .read_u32::<LittleEndian>()
        .map_err(|e| MapError::Io {
            context: "reading TTP version".into(),
            source: e,
        })?;
    if !(TTP_VERSION_MIN..=TTP_VERSION_MAX).contains(&version) {
        return Err(MapError::InvalidTtp(format!(
            "unsupported version {version} (expected {TTP_VERSION_MIN}-{TTP_VERSION_MAX})"
        )));
    }

    let count = cursor
        .read_u32::<LittleEndian>()
        .map_err(|e| MapError::Io {
            context: "reading terrain type count".into(),
            source: e,
        })?;

    let mut terrain_types = Vec::with_capacity(count as usize);
    for _ in 0..count {
        let tt = cursor
            .read_u16::<LittleEndian>()
            .map_err(|e| MapError::Io {
                context: "reading terrain type".into(),
                source: e,
            })?;
        terrain_types.push(TerrainType::from(tt));
    }

    Ok(TerrainTypeData { terrain_types })
}

/// Write terrain type data to TTP format bytes.
pub fn write_ttp(data: &TerrainTypeData) -> Result<Vec<u8>, MapError> {
    let capacity = 4 + 4 + 4 + data.terrain_types.len() * 2;
    let mut buf = Vec::with_capacity(capacity);

    let io_err = |e: std::io::Error| MapError::Io {
        context: "writing TTP".into(),
        source: e,
    };

    buf.write_all(TTP_MAGIC).map_err(&io_err)?;
    buf.write_u32::<LittleEndian>(TTP_VERSION_WRITE)
        .map_err(&io_err)?;
    buf.write_u32::<LittleEndian>(data.terrain_types.len() as u32)
        .map_err(&io_err)?;

    for tt in &data.terrain_types {
        buf.write_u16::<LittleEndian>(*tt as u16).map_err(&io_err)?;
    }

    Ok(buf)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ttp_roundtrip() {
        let data = TerrainTypeData {
            terrain_types: vec![
                TerrainType::Sand,
                TerrainType::Water,
                TerrainType::Cliffface,
                TerrainType::Road,
            ],
        };

        let bytes = write_ttp(&data).unwrap();
        let loaded = read_ttp(&bytes).unwrap();

        assert_eq!(loaded.terrain_types.len(), 4);
        assert_eq!(loaded.terrain_types[0], TerrainType::Sand);
        assert_eq!(loaded.terrain_types[1], TerrainType::Water);
        assert_eq!(loaded.terrain_types[2], TerrainType::Cliffface);
        assert_eq!(loaded.terrain_types[3], TerrainType::Road);
    }

    #[test]
    fn test_ttp_invalid_magic() {
        let mut buf = Vec::new();
        buf.write_all(b"nope").unwrap();
        buf.write_u32::<LittleEndian>(TTP_VERSION_WRITE).unwrap();
        buf.write_u32::<LittleEndian>(0).unwrap();
        let err = read_ttp(&buf).unwrap_err();
        assert!(matches!(err, MapError::InvalidTtp(_)));
    }

    #[test]
    fn test_ttp_wrong_version() {
        let mut buf = Vec::new();
        buf.write_all(TTP_MAGIC).unwrap();
        buf.write_u32::<LittleEndian>(99).unwrap(); // above max
        buf.write_u32::<LittleEndian>(0).unwrap();
        let err = read_ttp(&buf).unwrap_err();
        assert!(matches!(err, MapError::InvalidTtp(_)));
    }

    #[test]
    fn test_ttp_version_range_accepted() {
        // VER3 maps write TTP version 39, which must be accepted.
        for version in [7, 8, 20, 39] {
            let mut buf = Vec::new();
            buf.write_all(TTP_MAGIC).unwrap();
            buf.write_u32::<LittleEndian>(version).unwrap();
            buf.write_u32::<LittleEndian>(0).unwrap(); // 0 entries
            assert!(
                read_ttp(&buf).is_ok(),
                "version {version} should be accepted"
            );
        }
    }

    #[test]
    fn test_ttp_version_below_min_rejected() {
        let mut buf = Vec::new();
        buf.write_all(TTP_MAGIC).unwrap();
        buf.write_u32::<LittleEndian>(6).unwrap(); // below min
        buf.write_u32::<LittleEndian>(0).unwrap();
        assert!(matches!(
            read_ttp(&buf).unwrap_err(),
            MapError::InvalidTtp(_)
        ));
    }

    #[test]
    fn test_ttp_empty_terrain_types() {
        let data = TerrainTypeData {
            terrain_types: vec![],
        };
        let bytes = write_ttp(&data).unwrap();
        let loaded = read_ttp(&bytes).unwrap();
        assert!(loaded.terrain_types.is_empty());
    }

    #[test]
    fn test_ttp_all_terrain_types() {
        let data = TerrainTypeData {
            terrain_types: (0..12).map(TerrainType::from).collect(),
        };
        let bytes = write_ttp(&data).unwrap();
        let loaded = read_ttp(&bytes).unwrap();
        assert_eq!(loaded.terrain_types.len(), 12);
        assert_eq!(loaded.terrain_types[0], TerrainType::Sand);
        assert_eq!(loaded.terrain_types[7], TerrainType::Water);
        assert_eq!(loaded.terrain_types[8], TerrainType::Cliffface);
        assert_eq!(loaded.terrain_types[11], TerrainType::Slush);
    }

    #[test]
    fn test_ttp_truncated_data() {
        let mut buf = Vec::new();
        buf.write_all(TTP_MAGIC).unwrap();
        buf.write_u32::<LittleEndian>(TTP_VERSION_WRITE).unwrap();
        buf.write_u32::<LittleEndian>(5).unwrap(); // claims 5, supplies 2
        buf.write_u16::<LittleEndian>(0).unwrap();
        buf.write_u16::<LittleEndian>(1).unwrap();
        let err = read_ttp(&buf).unwrap_err();
        assert!(matches!(err, MapError::Io { .. }));
    }
}
