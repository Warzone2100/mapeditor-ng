//! Parser for WZ2100 level manifests (`gamedesc.lev`, `addon.lev`).
//!
//! The manifest lists every playable level as a block keyed by a kind
//! keyword (`camstart`, `expand`, `miss_keep`, `between`, ...) followed by
//! zero or more `dataset` / `game` / `data` parameters. Blocks are
//! separated by whitespace; comments use C syntax (`/* ... */` and `//`).
//!
//! Mirrors the grammar in WZ2100's `src/levels.cpp` lexer, simplified to
//! the subset the editor needs to surface overlay missions.

use crate::MapError;

/// Classification of a level-manifest entry.
///
/// Variants the editor acts on are listed explicitly; everything else
/// (multiplayer skirmish kinds, `campaign` dataset definitions,
/// `camchange` transitions, unknown tokens) falls into `Other`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LevelKind {
    /// Full mission, loads its own terrain from `game.map`.
    CamStart,
    /// Continuation mission that reuses the prior mission's terrain and
    /// overlays only its own object files.
    Expand,
    /// Side mission (engineering, alpha escape, etc.) with its own terrain.
    MissKeep,
    /// Non-playable cutscene / transition. No terrain of its own.
    Between,
    /// Any other kind (dataset definition, multiplayer, camchange, unknown).
    Other,
}

/// One block from a `.lev` manifest.
#[derive(Debug, Clone)]
pub struct LevelEntry {
    pub kind: LevelKind,
    pub name: String,
    pub dataset: Option<String>,
    pub game_path: Option<String>,
    pub data_paths: Vec<String>,
}

/// Parse a `.lev` manifest source into a flat list of entries, preserving
/// file order. Unrecognized block-starter keywords become `LevelKind::Other`
/// so the parser tolerates future engine additions.
pub fn parse_gamedesc(src: &str) -> Result<Vec<LevelEntry>, MapError> {
    let stripped = strip_comments(src);
    let tokens = tokenize(&stripped)?;

    let mut entries: Vec<LevelEntry> = Vec::new();
    let mut current: Option<LevelEntry> = None;
    let mut i = 0;

    while i < tokens.len() {
        let tok = &tokens[i];
        match tok {
            Token::Ident(word) => match word.as_str() {
                "dataset" => {
                    let entry = current
                        .as_mut()
                        .ok_or_else(|| lev_err("`dataset` before any level block"))?;
                    let value = expect_ident(&tokens, i + 1)?;
                    entry.dataset = Some(value.to_string());
                    i += 2;
                }
                "game" => {
                    let entry = current
                        .as_mut()
                        .ok_or_else(|| lev_err("`game` before any level block"))?;
                    let value = expect_string(&tokens, i + 1)?;
                    entry.game_path = Some(value.to_string());
                    i += 2;
                }
                "data" => {
                    let entry = current
                        .as_mut()
                        .ok_or_else(|| lev_err("`data` before any level block"))?;
                    let value = expect_string(&tokens, i + 1)?;
                    entry.data_paths.push(value.to_string());
                    i += 2;
                }
                other => {
                    // Block starter: close any previous block, open a new one.
                    if let Some(entry) = current.take() {
                        entries.push(entry);
                    }
                    let name = expect_ident(&tokens, i + 1)?;
                    current = Some(LevelEntry {
                        kind: classify_starter(other),
                        name: name.to_string(),
                        dataset: None,
                        game_path: None,
                        data_paths: Vec::new(),
                    });
                    i += 2;
                }
            },
            Token::Quoted(_) => {
                return Err(lev_err("unexpected quoted string outside a parameter"));
            }
        }
    }

    if let Some(entry) = current.take() {
        entries.push(entry);
    }
    Ok(entries)
}

/// Map a starter keyword to its editor-facing kind.
fn classify_starter(keyword: &str) -> LevelKind {
    match keyword {
        "camstart" => LevelKind::CamStart,
        "expand" | "expand_limbo" => LevelKind::Expand,
        "miss_keep" | "miss_keep_limbo" => LevelKind::MissKeep,
        "between" => LevelKind::Between,
        _ => LevelKind::Other,
    }
}

/// Strip `/* ... */` block comments and `// ...` line comments.
fn strip_comments(src: &str) -> String {
    let bytes = src.as_bytes();
    let mut out = String::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if i + 1 < bytes.len() && bytes[i] == b'/' && bytes[i + 1] == b'*' {
            i += 2;
            while i + 1 < bytes.len() && !(bytes[i] == b'*' && bytes[i + 1] == b'/') {
                i += 1;
            }
            i += 2;
            out.push(' ');
            continue;
        }
        if i + 1 < bytes.len() && bytes[i] == b'/' && bytes[i + 1] == b'/' {
            while i < bytes.len() && bytes[i] != b'\n' {
                i += 1;
            }
            continue;
        }
        out.push(bytes[i] as char);
        i += 1;
    }
    out
}

#[derive(Debug, Clone)]
enum Token {
    Ident(String),
    Quoted(String),
}

fn tokenize(src: &str) -> Result<Vec<Token>, MapError> {
    let bytes = src.as_bytes();
    let mut tokens = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        if b.is_ascii_whitespace() {
            i += 1;
            continue;
        }
        if b == b'"' {
            i += 1;
            let start = i;
            while i < bytes.len() && bytes[i] != b'"' {
                i += 1;
            }
            if i >= bytes.len() {
                return Err(lev_err("unterminated quoted string"));
            }
            let s = std::str::from_utf8(&bytes[start..i])
                .map_err(|_| lev_err("non-UTF-8 in quoted string"))?;
            tokens.push(Token::Quoted(s.to_string()));
            i += 1;
            continue;
        }
        let start = i;
        while i < bytes.len() && !bytes[i].is_ascii_whitespace() && bytes[i] != b'"' {
            i += 1;
        }
        let s = std::str::from_utf8(&bytes[start..i])
            .map_err(|_| lev_err("non-UTF-8 in identifier"))?;
        tokens.push(Token::Ident(s.to_string()));
    }
    Ok(tokens)
}

fn expect_ident(tokens: &[Token], idx: usize) -> Result<&str, MapError> {
    match tokens.get(idx) {
        Some(Token::Ident(s)) => Ok(s.as_str()),
        Some(Token::Quoted(_)) => Err(lev_err("expected identifier, found quoted string")),
        None => Err(lev_err("unexpected end of file while expecting identifier")),
    }
}

fn expect_string(tokens: &[Token], idx: usize) -> Result<&str, MapError> {
    match tokens.get(idx) {
        Some(Token::Quoted(s)) => Ok(s.as_str()),
        Some(Token::Ident(_)) => Err(lev_err("expected quoted string, found identifier")),
        None => Err(lev_err("unexpected end of file while expecting string")),
    }
}

fn lev_err(msg: &str) -> MapError {
    MapError::JsonFormat(format!("gamedesc.lev: {msg}"))
}

/// A level resolved against its campaign context.
///
/// For `camstart` and `miss_keep`, `folder` points at the archive prefix
/// holding `game.map`. For `expand`, `folder` holds only the overlay
/// object files and `base_folder` points at the preceding full mission's
/// terrain.
#[derive(Debug, Clone)]
pub struct ResolvedLevel {
    pub name: String,
    pub dataset: String,
    pub kind: LevelKind,
    pub folder: String,
    pub base_folder: Option<String>,
}

/// Resolved campaign index keyed by the order levels appear in the manifest.
#[derive(Debug, Clone)]
pub struct CampaignIndex {
    pub levels: Vec<ResolvedLevel>,
}

impl CampaignIndex {
    pub fn find(&self, name: &str) -> Option<&ResolvedLevel> {
        self.levels.iter().find(|l| l.name == name)
    }
}

/// Walk the entries in order, resolving each `expand` against the most
/// recent `camstart` / `miss_keep` in the same dataset.
///
/// Entries without a `game_path` or `dataset` are skipped, as are `Between`
/// cutscenes and `Other` (dataset definitions, multiplayer, camchange).
pub fn build_index(entries: &[LevelEntry]) -> CampaignIndex {
    use std::collections::HashMap;
    let mut current_base: HashMap<String, String> = HashMap::new();
    let mut levels = Vec::new();

    for entry in entries {
        if !matches!(
            entry.kind,
            LevelKind::CamStart | LevelKind::Expand | LevelKind::MissKeep
        ) {
            continue;
        }
        let Some(game_path) = entry.game_path.as_deref() else {
            continue;
        };
        let Some(dataset) = entry.dataset.as_deref() else {
            continue;
        };
        let Some(folder) = folder_from_game_path(game_path) else {
            continue;
        };

        let base_folder = match entry.kind {
            LevelKind::CamStart | LevelKind::MissKeep => {
                current_base.insert(dataset.to_string(), folder.clone());
                None
            }
            LevelKind::Expand => current_base.get(dataset).cloned(),
            _ => unreachable!(),
        };

        levels.push(ResolvedLevel {
            name: entry.name.clone(),
            dataset: dataset.to_string(),
            kind: entry.kind,
            folder,
            base_folder,
        });
    }

    CampaignIndex { levels }
}

/// `"wrf/cam1/cam1a.gam"` -> `Some("wrf/cam1/cam1a/")`. Returns `None` if
/// the path does not end in `.gam`.
fn folder_from_game_path(game_path: &str) -> Option<String> {
    let stem = game_path.strip_suffix(".gam")?;
    Some(format!("{stem}/"))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"
/* header comment */
campaign    CAM_1
data        "wrf/cam1.wrf"

camstart    CAM_1A
dataset     CAM_1
game        "wrf/cam1/cam1a.gam"
data        "wrf/cam1/cam1a.wrf"

expand      CAM_1B
dataset     CAM_1
game        "wrf/cam1/cam1b.gam"  // inline comment
data        "wrf/cam1/cam1b.wrf"

between     SUB_1_1S
dataset     CAM_1
data        "wrf/cam1/sub1-1s.wrf"

miss_keep   SUB_1_1
dataset     CAM_1
game        "wrf/cam1/sub1-1.gam"
data        "wrf/cam1/sub1-1.wrf"
"#;

    #[test]
    fn parses_basic_entries_in_order() {
        let entries = parse_gamedesc(SAMPLE).expect("parse");
        let names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(
            names,
            vec!["CAM_1", "CAM_1A", "CAM_1B", "SUB_1_1S", "SUB_1_1"]
        );
    }

    #[test]
    fn classifies_kinds() {
        let entries = parse_gamedesc(SAMPLE).expect("parse");
        assert_eq!(entries[0].kind, LevelKind::Other); // campaign
        assert_eq!(entries[1].kind, LevelKind::CamStart);
        assert_eq!(entries[2].kind, LevelKind::Expand);
        assert_eq!(entries[3].kind, LevelKind::Between);
        assert_eq!(entries[4].kind, LevelKind::MissKeep);
    }

    #[test]
    fn captures_fields() {
        let entries = parse_gamedesc(SAMPLE).expect("parse");
        let cam_1a = &entries[1];
        assert_eq!(cam_1a.dataset.as_deref(), Some("CAM_1"));
        assert_eq!(cam_1a.game_path.as_deref(), Some("wrf/cam1/cam1a.gam"));
        assert_eq!(cam_1a.data_paths, vec!["wrf/cam1/cam1a.wrf"]);
    }

    #[test]
    fn limbo_variants_map_to_plain_kinds() {
        let src = r#"
miss_keep_limbo  SUB_3_1
dataset CAM_3
game    "wrf/cam3/sub3-1.gam"
data    "wrf/cam3/sub3-1.wrf"

expand_limbo  CAM3C
dataset CAM_3
game    "wrf/cam3/cam3c.gam"
data    "wrf/cam3/cam3c.wrf"
"#;
        let entries = parse_gamedesc(src).expect("parse");
        assert_eq!(entries[0].kind, LevelKind::MissKeep);
        assert_eq!(entries[1].kind, LevelKind::Expand);
    }

    #[test]
    fn empty_and_comment_only_inputs_produce_no_entries() {
        assert!(parse_gamedesc("").unwrap().is_empty());
        assert!(parse_gamedesc("   \n\t  \n").unwrap().is_empty());
        assert!(
            parse_gamedesc("/* only */ // comments\n")
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn unterminated_quote_is_an_error() {
        let err = parse_gamedesc("camstart CAM game \"missing_end").unwrap_err();
        assert!(matches!(err, MapError::JsonFormat(ref msg) if msg.contains("unterminated")));
    }

    #[test]
    fn orphan_parameter_keyword_is_an_error() {
        let err = parse_gamedesc("dataset CAM_1").unwrap_err();
        assert!(
            matches!(err, MapError::JsonFormat(ref msg) if msg.contains("before any level block"))
        );
    }

    #[test]
    fn tolerates_crlf_and_mixed_whitespace() {
        let src = "camstart\tCAM_TEST\r\ndataset  CAM_X\r\ngame\t\t\"a/b.gam\"\r\n";
        let entries = parse_gamedesc(src).expect("parse");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].name, "CAM_TEST");
        assert_eq!(entries[0].dataset.as_deref(), Some("CAM_X"));
        assert_eq!(entries[0].game_path.as_deref(), Some("a/b.gam"));
    }

    #[test]
    fn real_gamedesc_fixture_matches_expected_counts() {
        let src = include_str!("../tests/fixtures/gamedesc.lev");
        let entries = parse_gamedesc(src).expect("parse");
        let count = |k: LevelKind| entries.iter().filter(|e| e.kind == k).count();
        assert_eq!(count(LevelKind::CamStart), 5);
        assert_eq!(count(LevelKind::Expand), 12);
        assert_eq!(count(LevelKind::MissKeep), 18);
        assert_eq!(count(LevelKind::Between), 20);
        // `campaign` dataset definitions and `camchange` transitions are Other.
        assert_eq!(count(LevelKind::Other), 6);
    }

    #[test]
    fn campaign_index_resolves_expand_to_prior_full_mission() {
        let entries = parse_gamedesc(SAMPLE).expect("parse");
        let idx = build_index(&entries);
        let names: Vec<&str> = idx.levels.iter().map(|l| l.name.as_str()).collect();
        // Only playable missions remain; `campaign` and `between` are dropped.
        assert_eq!(names, vec!["CAM_1A", "CAM_1B", "SUB_1_1"]);

        let cam_1a = idx.find("CAM_1A").unwrap();
        assert_eq!(cam_1a.kind, LevelKind::CamStart);
        assert_eq!(cam_1a.folder, "wrf/cam1/cam1a/");
        assert!(cam_1a.base_folder.is_none());

        let cam_1b = idx.find("CAM_1B").unwrap();
        assert_eq!(cam_1b.kind, LevelKind::Expand);
        assert_eq!(cam_1b.folder, "wrf/cam1/cam1b/");
        assert_eq!(cam_1b.base_folder.as_deref(), Some("wrf/cam1/cam1a/"));

        let sub = idx.find("SUB_1_1").unwrap();
        assert_eq!(sub.kind, LevelKind::MissKeep);
        assert!(sub.base_folder.is_none());
    }

    #[test]
    fn miss_keep_resets_base_for_following_expand() {
        // miss_keep resets the base terrain for any following `expand` in
        // the same dataset; CAM_1C resolves against SUB_1_3, not CAM_1A.
        let src = include_str!("../tests/fixtures/gamedesc.lev");
        let entries = parse_gamedesc(src).expect("parse");
        let idx = build_index(&entries);
        let cam_1c = idx.find("CAM_1C").expect("CAM_1C present");
        assert_eq!(cam_1c.kind, LevelKind::Expand);
        assert_eq!(cam_1c.base_folder.as_deref(), Some("wrf/cam1/sub1-3/"));
    }

    #[test]
    fn expand_base_is_scoped_per_dataset() {
        // Each dataset tracks its own current base independently.
        let src = r#"
camstart  CAM_1A
dataset   CAM_1
game      "wrf/cam1/cam1a.gam"

camstart  CAM_2A
dataset   CAM_2
game      "wrf/cam2/cam2a.gam"

expand    CAM_1B
dataset   CAM_1
game      "wrf/cam1/cam1b.gam"

expand    CAM_2B
dataset   CAM_2
game      "wrf/cam2/cam2b.gam"
"#;
        let entries = parse_gamedesc(src).expect("parse");
        let idx = build_index(&entries);
        assert_eq!(
            idx.find("CAM_1B").unwrap().base_folder.as_deref(),
            Some("wrf/cam1/cam1a/")
        );
        assert_eq!(
            idx.find("CAM_2B").unwrap().base_folder.as_deref(),
            Some("wrf/cam2/cam2a/")
        );
    }

    #[test]
    fn real_gamedesc_spot_checks_cam_1b() {
        let src = include_str!("../tests/fixtures/gamedesc.lev");
        let entries = parse_gamedesc(src).expect("parse");
        let cam_1b = entries
            .iter()
            .find(|e| e.name == "CAM_1B")
            .expect("CAM_1B present");
        assert_eq!(cam_1b.kind, LevelKind::Expand);
        assert_eq!(cam_1b.dataset.as_deref(), Some("CAM_1"));
        assert_eq!(cam_1b.game_path.as_deref(), Some("wrf/cam1/cam1b.gam"));
    }
}
