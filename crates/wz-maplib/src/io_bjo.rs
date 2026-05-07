//! Readers for the legacy binary object init files (`.bjo`).
//!
//! Campaign maps (and older skirmish maps) store structures, droids, and
//! features in `struct.bjo`, `dinit.bjo`, `feat.bjo` rather than the newer
//! JSON siblings. Mirrors the C++ loaders in `lib/wzmaplib/src/map.cpp`
//! (`loadBJOStructureInit`, `loadBJODroidInit`, `loadBJOFeatureInit`).

use std::io::{Cursor, Read};

use byteorder::{LittleEndian, ReadBytesExt};

use crate::MapError;
use crate::constants::{PLAYER_SCAVENGERS, TILE_MASK, TILE_UNITS};
use crate::objects::{Droid, Feature, Structure, WorldPos};

/// Name buffer length depends on the file-format version.
fn name_length(version: u32) -> usize {
    if version <= 19 { 40 } else { 60 }
}

/// Scavenger player slot used by BJO: `max(mapMaxPlayers, 7)`.
fn scavenger_slot(map_max_players: u32) -> u32 {
    map_max_players.max(7)
}

/// Translate a BJO player index to the editor's player space.
///
/// The scavenger slot maps to `PLAYER_SCAVENGERS` (-1); other values pass
/// through. Cast to `i8` mirrors the C++ loader, which stores player as
/// `int8_t`.
fn convert_player(bjo_player: u32, map_max_players: u32) -> i8 {
    if bjo_player == scavenger_slot(map_max_players) {
        PLAYER_SCAVENGERS
    } else {
        bjo_player as i8
    }
}

/// Convert a raw BJO direction (degrees, 0-359) to the game's 0-65535 range.
///
/// C++ macro: `DEG(d) = d * 8192 / 45`. Wrapping cast drops the top bits so
/// a value of 360° (65536) becomes 0, matching the game's u16 rotation.
fn degrees_to_internal(deg: u32) -> u16 {
    ((deg.wrapping_mul(8192) / 45) & 0xFFFF) as u16
}

fn bjo_io_err(ctx: &str, e: std::io::Error) -> MapError {
    MapError::Io {
        context: ctx.into(),
        source: e,
    }
}

fn short_read(ctx: &str) -> MapError {
    MapError::Io {
        context: ctx.into(),
        source: std::io::Error::new(std::io::ErrorKind::UnexpectedEof, "short read"),
    }
}

/// Read a fixed-length, null-padded name field.
fn read_name(cursor: &mut Cursor<&[u8]>, len: usize, ctx: &str) -> Result<String, MapError> {
    let mut buf = vec![0u8; len];
    cursor
        .read_exact(&mut buf)
        .map_err(|e| bjo_io_err(ctx, e))?;
    let end = buf.iter().position(|&b| b == 0).unwrap_or(buf.len());
    Ok(String::from_utf8_lossy(&buf[..end]).into_owned())
}

fn read_header(
    cursor: &mut Cursor<&[u8]>,
    magic: [u8; 4],
    filename: &str,
) -> Result<(u32, u32), MapError> {
    let mut tag = [0u8; 4];
    cursor
        .read_exact(&mut tag)
        .map_err(|_| short_read(&format!("{filename}: reading header")))?;
    if tag != magic {
        return Err(MapError::JsonFormat(format!(
            "{filename}: bad magic {tag:?}, expected {magic:?}"
        )));
    }
    let version = cursor
        .read_u32::<LittleEndian>()
        .map_err(|e| bjo_io_err(&format!("{filename}: reading version"), e))?;
    let quantity = cursor
        .read_u32::<LittleEndian>()
        .map_err(|e| bjo_io_err(&format!("{filename}: reading quantity"), e))?;
    Ok((version, quantity))
}

/// Parse `struct.bjo`.
pub fn read_structures(bytes: &[u8], map_max_players: u32) -> Result<Vec<Structure>, MapError> {
    let mut cursor = Cursor::new(bytes);
    let (version, quantity) = read_header(&mut cursor, *b"stru", "struct.bjo")?;

    if !(7..=8).contains(&version) {
        return Err(MapError::JsonFormat(format!(
            "struct.bjo: unsupported version {version} (expected 7..=8)"
        )));
    }

    let name_len = name_length(version);
    let mut out = Vec::with_capacity(quantity as usize);

    for i in 0..quantity {
        let name = read_name(
            &mut cursor,
            name_len,
            &format!("struct.bjo: record {i} name"),
        )?;
        let id = cursor
            .read_u32::<LittleEndian>()
            .map_err(|e| bjo_io_err("struct.bjo: id", e))?;
        let x = cursor
            .read_u32::<LittleEndian>()
            .map_err(|e| bjo_io_err("struct.bjo: x", e))?;
        let y = cursor
            .read_u32::<LittleEndian>()
            .map_err(|e| bjo_io_err("struct.bjo: y", e))?;
        let _z = cursor
            .read_u32::<LittleEndian>()
            .map_err(|e| bjo_io_err("struct.bjo: z", e))?;
        let direction = cursor
            .read_u32::<LittleEndian>()
            .map_err(|e| bjo_io_err("struct.bjo: direction", e))?;
        let player = cursor
            .read_u32::<LittleEndian>()
            .map_err(|e| bjo_io_err("struct.bjo: player", e))?;
        let _in_fire = cursor
            .read_i32::<LittleEndian>()
            .map_err(|e| bjo_io_err("struct.bjo: inFire", e))?;
        let _burn_start = cursor
            .read_u32::<LittleEndian>()
            .map_err(|e| bjo_io_err("struct.bjo: burnStart", e))?;
        let _burn_damage = cursor
            .read_u32::<LittleEndian>()
            .map_err(|e| bjo_io_err("struct.bjo: burnDamage", e))?;
        // status (u8) + 3 padding bytes.
        let mut status_pad = [0u8; 4];
        cursor
            .read_exact(&mut status_pad)
            .map_err(|e| bjo_io_err("struct.bjo: status", e))?;
        // Ten trailing u32s the editor doesn't use: currentBuildPts, body,
        // armour, resistance, dummy1, subjectInc, timeStarted, output,
        // capacity, quantity.
        for _ in 0..10 {
            cursor
                .read_u32::<LittleEndian>()
                .map_err(|e| bjo_io_err("struct.bjo: tail", e))?;
        }

        out.push(Structure {
            name,
            position: WorldPos { x, y },
            direction: degrees_to_internal(direction),
            player: convert_player(player, map_max_players),
            modules: 0,
            id: (id > 0).then_some(id),
        });
    }

    Ok(out)
}

/// Parse `dinit.bjo`.
pub fn read_droids(bytes: &[u8], map_max_players: u32) -> Result<Vec<Droid>, MapError> {
    let mut cursor = Cursor::new(bytes);
    let (version, quantity) = read_header(&mut cursor, *b"dint", "dinit.bjo")?;

    let name_len = name_length(version);
    let mut out = Vec::with_capacity(quantity as usize);

    for i in 0..quantity {
        let name = read_name(
            &mut cursor,
            name_len,
            &format!("dinit.bjo: record {i} name"),
        )?;
        let id = cursor
            .read_u32::<LittleEndian>()
            .map_err(|e| bjo_io_err("dinit.bjo: id", e))?;
        let x = cursor
            .read_u32::<LittleEndian>()
            .map_err(|e| bjo_io_err("dinit.bjo: x", e))?;
        let y = cursor
            .read_u32::<LittleEndian>()
            .map_err(|e| bjo_io_err("dinit.bjo: y", e))?;
        let _z = cursor
            .read_u32::<LittleEndian>()
            .map_err(|e| bjo_io_err("dinit.bjo: z", e))?;
        let direction = cursor
            .read_u32::<LittleEndian>()
            .map_err(|e| bjo_io_err("dinit.bjo: direction", e))?;
        let player = cursor
            .read_u32::<LittleEndian>()
            .map_err(|e| bjo_io_err("dinit.bjo: player", e))?;
        let _in_fire = cursor
            .read_i32::<LittleEndian>()
            .map_err(|e| bjo_io_err("dinit.bjo: inFire", e))?;
        let _burn_start = cursor
            .read_u32::<LittleEndian>()
            .map_err(|e| bjo_io_err("dinit.bjo: burnStart", e))?;
        let _burn_damage = cursor
            .read_u32::<LittleEndian>()
            .map_err(|e| bjo_io_err("dinit.bjo: burnDamage", e))?;

        // Droid positions snap to the tile centre: strip sub-tile bits, add half a tile.
        let snap = |v: u32| (v & !TILE_MASK).saturating_add(TILE_UNITS / 2);

        out.push(Droid {
            name,
            position: WorldPos {
                x: snap(x),
                y: snap(y),
            },
            direction: degrees_to_internal(direction),
            player: convert_player(player, map_max_players),
            id: (id > 0).then_some(id),
        });
    }

    Ok(out)
}

/// Parse `feat.bjo`.
pub fn read_features(bytes: &[u8], _map_max_players: u32) -> Result<Vec<Feature>, MapError> {
    let mut cursor = Cursor::new(bytes);
    let (version, quantity) = read_header(&mut cursor, *b"feat", "feat.bjo")?;

    if !(7..=19).contains(&version) {
        return Err(MapError::JsonFormat(format!(
            "feat.bjo: unsupported version {version} (expected 7..=19)"
        )));
    }

    let name_len = name_length(version);
    let mut out = Vec::with_capacity(quantity as usize);

    for i in 0..quantity {
        let name = read_name(&mut cursor, name_len, &format!("feat.bjo: record {i} name"))?;
        let id = cursor
            .read_u32::<LittleEndian>()
            .map_err(|e| bjo_io_err("feat.bjo: id", e))?;
        let x = cursor
            .read_u32::<LittleEndian>()
            .map_err(|e| bjo_io_err("feat.bjo: x", e))?;
        let y = cursor
            .read_u32::<LittleEndian>()
            .map_err(|e| bjo_io_err("feat.bjo: y", e))?;
        let _z = cursor
            .read_u32::<LittleEndian>()
            .map_err(|e| bjo_io_err("feat.bjo: z", e))?;
        let direction = cursor
            .read_u32::<LittleEndian>()
            .map_err(|e| bjo_io_err("feat.bjo: direction", e))?;
        let _player = cursor
            .read_u32::<LittleEndian>()
            .map_err(|e| bjo_io_err("feat.bjo: player", e))?;
        let _in_fire = cursor
            .read_i32::<LittleEndian>()
            .map_err(|e| bjo_io_err("feat.bjo: inFire", e))?;
        let _burn_start = cursor
            .read_u32::<LittleEndian>()
            .map_err(|e| bjo_io_err("feat.bjo: burnStart", e))?;
        let _burn_damage = cursor
            .read_u32::<LittleEndian>()
            .map_err(|e| bjo_io_err("feat.bjo: burnDamage", e))?;

        if version >= 14 {
            let mut vis = [0u8; 8];
            cursor
                .read_exact(&mut vis)
                .map_err(|e| bjo_io_err("feat.bjo: visibility", e))?;
        }

        out.push(Feature {
            name,
            position: WorldPos { x, y },
            direction: degrees_to_internal(direction),
            id: (id > 0).then_some(id),
            // Upstream ignores feature player.
            player: None,
        });
    }

    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use byteorder::WriteBytesExt;
    use std::io::Write;

    fn write_name(buf: &mut Vec<u8>, name: &str, len: usize) {
        let bytes = name.as_bytes();
        let take = bytes.len().min(len);
        buf.extend_from_slice(&bytes[..take]);
        buf.extend(std::iter::repeat_n(0u8, len - take));
    }

    fn build_struct_bjo(version: u32, records: &[(u32, &str, u32, u32, u32, u32)]) -> Vec<u8> {
        let mut b = Vec::new();
        b.extend_from_slice(b"stru");
        b.write_u32::<LittleEndian>(version).unwrap();
        b.write_u32::<LittleEndian>(records.len() as u32).unwrap();
        let name_len = name_length(version);
        for (id, name, x, y, dir, player) in records {
            write_name(&mut b, name, name_len);
            b.write_u32::<LittleEndian>(*id).unwrap();
            b.write_u32::<LittleEndian>(*x).unwrap();
            b.write_u32::<LittleEndian>(*y).unwrap();
            b.write_u32::<LittleEndian>(0).unwrap(); // z
            b.write_u32::<LittleEndian>(*dir).unwrap();
            b.write_u32::<LittleEndian>(*player).unwrap();
            b.write_i32::<LittleEndian>(0).unwrap(); // inFire
            b.write_u32::<LittleEndian>(0).unwrap(); // burnStart
            b.write_u32::<LittleEndian>(0).unwrap(); // burnDamage
            b.write_all(&[1u8, 0, 0, 0]).unwrap(); // status=SS_BUILT + 3 padding
            for _ in 0..10 {
                b.write_u32::<LittleEndian>(0).unwrap();
            }
        }
        b
    }

    fn build_dinit_bjo(version: u32, records: &[(u32, &str, u32, u32, u32, u32)]) -> Vec<u8> {
        let mut b = Vec::new();
        b.extend_from_slice(b"dint");
        b.write_u32::<LittleEndian>(version).unwrap();
        b.write_u32::<LittleEndian>(records.len() as u32).unwrap();
        let name_len = name_length(version);
        for (id, name, x, y, dir, player) in records {
            write_name(&mut b, name, name_len);
            b.write_u32::<LittleEndian>(*id).unwrap();
            b.write_u32::<LittleEndian>(*x).unwrap();
            b.write_u32::<LittleEndian>(*y).unwrap();
            b.write_u32::<LittleEndian>(0).unwrap();
            b.write_u32::<LittleEndian>(*dir).unwrap();
            b.write_u32::<LittleEndian>(*player).unwrap();
            b.write_i32::<LittleEndian>(0).unwrap();
            b.write_u32::<LittleEndian>(0).unwrap();
            b.write_u32::<LittleEndian>(0).unwrap();
        }
        b
    }

    fn build_feat_bjo(version: u32, records: &[(u32, &str, u32, u32, u32)]) -> Vec<u8> {
        let mut b = Vec::new();
        b.extend_from_slice(b"feat");
        b.write_u32::<LittleEndian>(version).unwrap();
        b.write_u32::<LittleEndian>(records.len() as u32).unwrap();
        let name_len = name_length(version);
        for (id, name, x, y, dir) in records {
            write_name(&mut b, name, name_len);
            b.write_u32::<LittleEndian>(*id).unwrap();
            b.write_u32::<LittleEndian>(*x).unwrap();
            b.write_u32::<LittleEndian>(*y).unwrap();
            b.write_u32::<LittleEndian>(0).unwrap();
            b.write_u32::<LittleEndian>(*dir).unwrap();
            b.write_u32::<LittleEndian>(0).unwrap(); // player
            b.write_i32::<LittleEndian>(0).unwrap();
            b.write_u32::<LittleEndian>(0).unwrap();
            b.write_u32::<LittleEndian>(0).unwrap();
            if version >= 14 {
                b.write_all(&[0u8; 8]).unwrap();
            }
        }
        b
    }

    #[test]
    fn struct_bjo_v7_reads_records() {
        let bytes = build_struct_bjo(7, &[(5, "A0PowerGenerator", 1280, 2560, 90, 0)]);
        let s = read_structures(&bytes, 2).expect("parse");
        assert_eq!(s.len(), 1);
        assert_eq!(s[0].name, "A0PowerGenerator");
        assert_eq!(s[0].position.x, 1280);
        assert_eq!(s[0].position.y, 2560);
        assert_eq!(s[0].player, 0);
        assert_eq!(s[0].id, Some(5));
        // 90° * 8192 / 45 = 16384 (quarter turn in 0-65535 space).
        assert_eq!(s[0].direction, 16384);
    }

    #[test]
    fn dinit_bjo_snaps_droid_to_tile_center() {
        // X=200 is 72 units into tile 1; should snap to 128 + 64 = 192.
        let bytes = build_dinit_bjo(8, &[(1, "ViperMG", 200, 500, 180, 1)]);
        let d = read_droids(&bytes, 2).expect("parse");
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].position.x, 192);
        // Y=500: floor to 384, +64 = 448.
        assert_eq!(d[0].position.y, 448);
        assert_eq!(d[0].direction, 32768); // 180°
        assert_eq!(d[0].player, 1);
    }

    #[test]
    fn feat_bjo_v14_reads_visibility_padding() {
        let bytes = build_feat_bjo(14, &[(42, "OilResource", 1024, 1024, 0)]);
        let f = read_features(&bytes, 2).expect("parse");
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].name, "OilResource");
        assert_eq!(f[0].position.x, 1024);
        assert_eq!(f[0].id, Some(42));
    }

    #[test]
    fn scavenger_slot_maps_to_minus_one() {
        // For a 2-player map, scavenger sits in slot max(2, 7) = 7.
        let bytes = build_dinit_bjo(8, &[(1, "Scav", 0, 0, 0, 7)]);
        let d = read_droids(&bytes, 2).expect("parse");
        assert_eq!(d[0].player, PLAYER_SCAVENGERS);
    }

    #[test]
    fn bad_magic_errors() {
        let mut bytes = build_dinit_bjo(8, &[]);
        bytes[0] = b'x';
        assert!(read_droids(&bytes, 2).is_err());
    }
}
