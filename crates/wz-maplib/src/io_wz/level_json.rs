//! Build and parse the `level.json` and `gam.json` sidecars that WZ2100
//! expects alongside `game.map` inside a `.wz` archive.

use crate::map_data::MapData;

use super::common::read_zip_file;

/// Build the `level.json` content that WZ2100 requires to recognize a map.
pub(super) fn build_level_json(name: &str, players: u8, tileset: &str) -> String {
    serde_json::to_string_pretty(&serde_json::json!({
        "name": name,
        "type": "skirmish",
        "players": players,
        "tileset": tileset,
        "author": { "name": "wzmapeditor" },
        "generator": "wzmapeditor"
    }))
    .expect("level.json serialization cannot fail")
}

/// Metadata extracted from `level.json` inside a `.wz` archive.
pub(super) struct LevelMeta {
    pub name: String,
    pub players: u8,
    pub tileset: String,
}

/// Try to read `level.json` from a zip archive and extract metadata.
pub(super) fn read_level_json<R: std::io::Read + std::io::Seek>(
    archive: &mut zip::ZipArchive<R>,
    prefix: &str,
) -> Option<LevelMeta> {
    let bytes = read_zip_file(archive, &format!("{prefix}level.json")).ok()?;
    let text = String::from_utf8_lossy(&bytes);
    let json: serde_json::Value = serde_json::from_str(&text).ok()?;
    let name = json.get("name")?.as_str()?.to_string();
    let players = json.get("players")?.as_u64()? as u8;
    let tileset = json.get("tileset")?.as_str()?.to_string();
    Some(LevelMeta {
        name,
        players,
        tileset,
    })
}

/// Build a `gam.json` file with scroll/viewport bounds.
pub(super) fn build_gam_json(map_data: &MapData) -> String {
    serde_json::to_string_pretty(&serde_json::json!({
        "version": 7,
        "gameTime": 0,
        "GameType": 0,
        "ScrollMinX": 0,
        "ScrollMinY": 0,
        "ScrollMaxX": map_data.width,
        "ScrollMaxY": map_data.height,
        "levelName": ""
    }))
    .expect("gam.json serialization cannot fail")
}
