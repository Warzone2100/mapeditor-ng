//! Lightweight map metadata for thumbnail generation and map browsing.
//!
//! [`MapPreview`] holds just the heightmap, texture indices, and terrain
//! types needed to draw a minimap. The `peek_*` and `scan_*` functions
//! avoid the cost of a full [`super::WzMap`] load (object JSON, labels,
//! templates) so the map browser can list hundreds of maps quickly.

use std::path::Path;
use std::path::PathBuf;

use crate::MapError;
use crate::io_binary;
use crate::io_ttp;

use super::common::{find_map_prefix, parse_player_count, read_zip_file};

/// Lightweight map metadata extracted from a `.wz` archive.
///
/// Holds the heightmap, texture indices, and terrain types needed for
/// minimap thumbnails without loading full object data.
#[derive(Debug, Clone)]
pub struct MapPreview {
    /// Display name derived from the archive filename or internal folder.
    pub name: String,
    pub width: u32,
    pub height: u32,
    /// Player count parsed from the "Nc-" filename prefix; 0 if absent.
    pub players: u8,
    /// Flat tile heightmap, length = width * height.
    pub heights: Vec<u16>,
    /// Flat tile texture indices (masked), length = width * height.
    pub textures: Vec<u16>,
    /// Terrain type per tile texture index, indexed by `texture_id`.
    pub terrain_types: Vec<u16>,
    pub path: PathBuf,
    /// Prefix inside the archive for multi-map archives (e.g. `mp.wz`).
    /// Empty for single-map archives.
    pub archive_prefix: String,
}

/// Read only the map dimensions and heightmap from a `.wz` archive.
///
/// Much faster than `load_from_wz_archive` because it skips JSON
/// object files and terrain type data.
pub fn peek_map_preview(wz_path: &Path) -> Result<MapPreview, MapError> {
    let file = std::fs::File::open(wz_path).map_err(|e| MapError::Io {
        context: format!("opening {}", wz_path.display()),
        source: e,
    })?;
    let mut archive = zip::ZipArchive::new(file)?;

    let map_prefix = find_map_prefix(&archive)?;

    let map_bytes = read_zip_file(&mut archive, &format!("{map_prefix}game.map"))?;
    let map_data = io_binary::read_game_map(&map_bytes)?;

    let name = wz_path.file_stem().map_or_else(
        || "unnamed".to_string(),
        |s| s.to_string_lossy().to_string(),
    );

    let players = parse_player_count(&name);

    let heights: Vec<u16> = map_data.tiles.iter().map(|t| t.height).collect();
    let textures: Vec<u16> = map_data
        .tiles
        .iter()
        .map(|t| t.texture & crate::constants::TILE_NUMMASK)
        .collect();

    let terrain_types = match read_zip_file(&mut archive, &format!("{map_prefix}ttypes.ttp")) {
        Ok(bytes) => match io_ttp::read_ttp(&bytes) {
            Ok(ttp) => ttp.terrain_types.iter().map(|t| *t as u16).collect(),
            Err(_) => Vec::new(),
        },
        Err(_) => Vec::new(),
    };

    Ok(MapPreview {
        name,
        width: map_data.width,
        height: map_data.height,
        players,
        heights,
        textures,
        terrain_types,
        path: wz_path.to_path_buf(),
        archive_prefix: String::new(),
    })
}

/// Scan a directory for `.wz` map archives and return previews.
///
/// Errors on individual files are logged and skipped.
pub fn scan_map_directory(dir: &Path) -> Vec<MapPreview> {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(e) => {
            log::warn!("Cannot read map directory {}: {}", dir.display(), e);
            return Vec::new();
        }
    };

    let mut previews = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().is_some_and(|ext| ext == "wz") {
            match peek_map_preview(&path) {
                Ok(preview) => previews.push(preview),
                Err(e) => {
                    log::warn!("Skipping {}: {}", path.display(), e);
                }
            }
        }
    }

    previews.sort_by(|a, b| a.players.cmp(&b.players).then_with(|| a.name.cmp(&b.name)));
    previews
}

/// Scan a multi-map `.wz` archive (e.g. `mp.wz`) for all maps inside it.
///
/// Finds every `game.map` entry, reads its heightmap, and returns a
/// `MapPreview` for each. The `archive_prefix` field on each preview
/// records the internal path prefix needed to load that specific map.
pub fn scan_wz_archive_maps(wz_path: &Path) -> Vec<MapPreview> {
    let file = match std::fs::File::open(wz_path) {
        Ok(f) => f,
        Err(e) => {
            log::warn!("Cannot open {}: {}", wz_path.display(), e);
            return Vec::new();
        }
    };
    let mut archive = match zip::ZipArchive::new(file) {
        Ok(a) => a,
        Err(e) => {
            log::warn!("Cannot read archive {}: {}", wz_path.display(), e);
            return Vec::new();
        }
    };

    let prefixes: Vec<String> = (0..archive.len())
        .filter_map(|i| {
            let name = archive.name_for_index(i)?;
            name.strip_suffix("game.map").map(ToString::to_string)
        })
        .collect();

    let mut previews = Vec::new();
    for prefix in prefixes {
        let map_path = format!("{prefix}game.map");
        let map_bytes = match read_zip_file(&mut archive, &map_path) {
            Ok(b) => b,
            Err(e) => {
                log::warn!("Skipping {map_path}: {e}");
                continue;
            }
        };
        let map_data = match io_binary::read_game_map(&map_bytes) {
            Ok(d) => d,
            Err(e) => {
                log::warn!("Skipping {map_path}: {e}");
                continue;
            }
        };

        let name = prefix
            .trim_end_matches('/')
            .rsplit('/')
            .next()
            .unwrap_or("unnamed")
            .to_string();

        let players = parse_player_count(&name);
        let heights: Vec<u16> = map_data.tiles.iter().map(|t| t.height).collect();
        let textures: Vec<u16> = map_data
            .tiles
            .iter()
            .map(|t| t.texture & crate::constants::TILE_NUMMASK)
            .collect();

        let terrain_types = match read_zip_file(&mut archive, &format!("{prefix}ttypes.ttp")) {
            Ok(bytes) => match io_ttp::read_ttp(&bytes) {
                Ok(ttp) => ttp.terrain_types.iter().map(|t| *t as u16).collect(),
                Err(_) => Vec::new(),
            },
            Err(_) => Vec::new(),
        };

        previews.push(MapPreview {
            name,
            width: map_data.width,
            height: map_data.height,
            players,
            heights,
            textures,
            terrain_types,
            path: wz_path.to_path_buf(),
            archive_prefix: prefix,
        });
    }

    previews.sort_by(|a, b| a.players.cmp(&b.players).then_with(|| a.name.cmp(&b.name)));
    previews
}
