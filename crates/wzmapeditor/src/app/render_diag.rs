//! On-demand report of terrain and ground-texture state for the Output panel.
//!
//! Web builds have no disk log: `eframe::WebLogger` sends `log` records to the
//! browser console, and the Output panel only shows what editor code pushes into
//! it. This assembles the facts a native user would otherwise read out of
//! `wzmapeditor.log` -- which texture occupies which array slot, and which
//! ground type the splatting vote actually chose -- so a browser session can
//! diagnose a mis-textured map without opening developer tools.

use crate::app::EditorApp;
use crate::app::output_log::LogSource;
use crate::viewport::ground_types::GroundData;

/// Ground indices and tile textures listed before the tail is summarized.
///
/// A tileset has at most 16 ground types, so this only elides a long tail of
/// near-unused entries; the summary line still accounts for them.
const MAX_ROWS: usize = 8;

/// Bumped with every shading change. A cached wasm build is otherwise
/// indistinguishable from the current one in a pasted report.
const SHADING_REVISION: u32 = 2;

/// One report line and whether it describes a problem.
struct Line {
    warn: bool,
    text: String,
}

fn info(text: String) -> Line {
    Line { warn: false, text }
}

fn warn(text: String) -> Line {
    Line { warn: true, text }
}

/// Log the current renderer, tileset and ground-splatting state, and reveal the
/// Renderer source so the report -- and anything logged while terrain loaded --
/// is visible.
pub fn report(app: &mut EditorApp) {
    app.output_log.show_source(LogSource::Render);
    for line in collect(app) {
        if line.warn {
            app.log_render_warn(line.text);
        } else {
            app.log_render(line.text);
        }
    }
}

fn collect(app: &EditorApp) -> Vec<Line> {
    let mut lines = vec![
        info(format!(
            "--- Renderer diagnostics (build {}, shading rev {SHADING_REVISION}) ---",
            env!("CARGO_PKG_VERSION"),
        )),
        info(format!(
            "Tileset {} - terrain quality {}",
            app.current_tileset.as_str(),
            app.render_settings.terrain_quality.label(),
        )),
    ];
    collect_map(app, &mut lines);
    collect_ground_data(app, &mut lines);
    lines
}

fn collect_map(app: &EditorApp, lines: &mut Vec<Line>) {
    let Some(doc) = app.document.as_ref() else {
        lines.push(info("Map: none open".to_owned()));
        return;
    };

    let map = &doc.map.map_data;
    let rows = tile_texture_histogram(map);
    let summary = histogram_summary(&rows, map.tiles.len(), |id| format!("tile {id}"));
    lines.push(info(format!(
        "Map {}x{}, {} distinct tile textures: {summary}",
        map.width,
        map.height,
        rows.len()
    )));

    // The vote weights read terrain types, and a map's own .ttp takes precedence
    // over the built-in table, so a mismatch changes which ground type wins.
    let builtin = app.current_tileset.full_terrain_types();
    match doc.map.terrain_types.as_ref() {
        None => lines.push(info(format!(
            "Terrain types: map has no .ttp; using the built-in {}-entry table",
            builtin.len()
        ))),
        Some(ttp) => {
            let differing = ttp
                .terrain_types
                .iter()
                .zip(builtin.iter())
                .filter(|(a, b)| a != b)
                .count();
            lines.push(info(format!(
                "Terrain types: map .ttp has {} entries vs built-in {}, {differing} differ",
                ttp.terrain_types.len(),
                builtin.len()
            )));
        }
    }
}

fn collect_ground_data(app: &EditorApp, lines: &mut Vec<Line>) {
    let Some(gd) = app.ground_data.as_ref() else {
        lines.push(info(
            "Ground data: not loaded (Classic terrain draws the tile atlas only, \
             so splatting is inactive)"
                .to_owned(),
        ));
        return;
    };

    lines.push(info(format!(
        "Ground data: {} ground types, {} tile entries, {} decals, {} terrain types",
        gd.ground_types.len(),
        gd.tile_grounds.len(),
        gd.decal_tiles.iter().filter(|&&d| d).count(),
        gd.terrain_types.len(),
    )));

    lines.push(info("Ground textures by array slot:".to_owned()));
    for (i, gt) in gd.ground_types.iter().enumerate() {
        let nm = if gt.normal_filename.is_some() {
            "nm"
        } else {
            "--"
        };
        let sm = if gt.specular_filename.is_some() {
            "sm"
        } else {
            "--"
        };
        lines.push(info(format!(
            "  [{i}] {} - {} scale {} {nm} {sm}",
            gt.name, gt.filename, gt.scale
        )));
    }

    if let Some(doc) = app.document.as_ref() {
        collect_vote(gd, &doc.map.map_data, lines);
    }
}

/// Report which ground types the per-vertex vote selected across the whole map.
///
/// This separates a texture-array problem from a vote problem: one index
/// dominating means the vote chose it, whatever the array happens to hold.
fn collect_vote(gd: &GroundData, map: &wz_maplib::MapData, lines: &mut Vec<Line>) {
    let grid = gd.build_ground_grid(map);
    let total = grid.len();
    let mut counts = vec![0u32; gd.ground_types.len()];
    let mut out_of_range = 0u32;
    for g in grid {
        match counts.get_mut(g as usize) {
            Some(slot) => *slot += 1,
            None => out_of_range += 1,
        }
    }

    let rows = ranked_rows(counts.into_iter().enumerate());
    let summary = histogram_summary(&rows, total, |i| {
        let name = gd.ground_types.get(i).map_or("?", |gt| gt.name.as_str());
        format!("[{i}] {name}")
    });
    lines.push(info(format!(
        "Ground-type vote over {total} vertices: {summary}"
    )));

    if out_of_range > 0 {
        lines.push(warn(format!(
            "{out_of_range} vertices voted for a ground type outside the {} loaded slots",
            gd.ground_types.len()
        )));
    }
}

fn tile_texture_histogram(map: &wz_maplib::MapData) -> Vec<(usize, u32)> {
    let mut counts = std::collections::HashMap::new();
    for tile in &map.tiles {
        *counts.entry(tile.texture_id() as usize).or_insert(0u32) += 1;
    }
    ranked_rows(counts)
}

/// Drop empty entries and order by descending count, then by index so equal
/// counts render in a stable order.
fn ranked_rows(counts: impl IntoIterator<Item = (usize, u32)>) -> Vec<(usize, u32)> {
    let mut rows: Vec<(usize, u32)> = counts.into_iter().filter(|&(_, n)| n > 0).collect();
    rows.sort_unstable_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    rows
}

/// Render up to [`MAX_ROWS`] rows as `label count (pct%)`, noting any remainder.
fn histogram_summary(
    rows: &[(usize, u32)],
    total: usize,
    label: impl Fn(usize) -> String,
) -> String {
    if rows.is_empty() {
        return "none".to_owned();
    }
    let total = total as u64;
    let pct = |n: u32| (u64::from(n) * 100).checked_div(total).unwrap_or(0);
    let mut parts: Vec<String> = rows
        .iter()
        .take(MAX_ROWS)
        .map(|&(i, n)| format!("{} {n} ({}%)", label(i), pct(n)))
        .collect();
    if rows.len() > MAX_ROWS {
        let rest: u32 = rows.iter().skip(MAX_ROWS).map(|&(_, n)| n).sum();
        parts.push(format!(
            "{} more {rest} ({}%)",
            rows.len() - MAX_ROWS,
            pct(rest)
        ));
    }
    parts.join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ranked_rows_orders_by_count_then_index() {
        let rows = ranked_rows(vec![(0, 5), (1, 0), (2, 9), (3, 5)]);
        assert_eq!(rows, vec![(2, 9), (0, 5), (3, 5)]);
    }

    #[test]
    fn histogram_summary_folds_the_tail_into_one_entry() {
        let rows: Vec<(usize, u32)> = (0..MAX_ROWS + 3).map(|i| (i, 10)).collect();
        let summary = histogram_summary(&rows, 10 * rows.len(), |i| format!("g{i}"));
        assert!(summary.starts_with("g0 10 (9%)"), "{summary}");
        assert!(summary.ends_with("3 more 30 (27%)"), "{summary}");
        assert!(!summary.contains("g8 "), "{summary}");
    }

    #[test]
    fn histogram_summary_handles_an_empty_map() {
        assert_eq!(histogram_summary(&[], 0, |i| format!("g{i}")), "none");
    }

    #[test]
    fn tile_texture_histogram_counts_each_texture() {
        let mut map = wz_maplib::MapData::new(2, 2);
        map.tile_mut(0, 0).expect("tile in range").texture = 7;
        map.tile_mut(1, 0).expect("tile in range").texture = 7;
        map.tile_mut(0, 1).expect("tile in range").texture = 3;
        assert_eq!(tile_texture_histogram(&map), vec![(7, 2), (0, 1), (3, 1)]);
    }
}
