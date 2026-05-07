//! Per-player starting-economy summary, modeled on the breakdown
//! maps.wz2100.net shows for every uploaded multiplayer map.

use egui::{Color32, RichText, Ui};
use wz_maplib::constants::TILE_UNITS_F32;

use crate::app::EditorApp;
use crate::balance::{BalanceReport, PlayerBalance, player_color};

const BALANCED: Color32 = Color32::from_rgb(80, 220, 80);
const IMBALANCED: Color32 = Color32::from_rgb(255, 180, 70);
const MUTED: Color32 = Color32::from_rgb(170, 170, 170);

pub fn show_balance_panel(ui: &mut Ui, app: &mut EditorApp) {
    if app.document.is_none() {
        ui.label("Open a map to see the balance summary.");
        return;
    }

    if let Some(ref doc) = app.document {
        app.balance.ensure(&doc.map);
    }

    ui.horizontal(|ui| {
        if ui.button(RichText::new("Re-run").strong()).clicked()
            && let Some(ref doc) = app.document
        {
            app.balance.refresh(&doc.map);
        }
        ui.checkbox(&mut app.balance.show_voronoi, "Zone lines")
            .on_hover_text(
                "Draw the nearest-player partition as outline. Each oil tile is credited to \
                 whichever player's start it falls inside.",
            );
        ui.checkbox(&mut app.balance.show_voronoi_tint, "Zone fill")
            .on_hover_text(
                "Tint each cell faintly with its owning player's slot color so areas of \
                 ownership are visible at a glance.",
            );
    });
    ui.separator();

    let Some(report) = app.balance.report.clone() else {
        return;
    };

    if report.players.is_empty() {
        ui.label(
            "No player structures placed. Add an A0CommandCentre per player to see a summary.",
        );
        return;
    }

    let verdict_color = if report.fully_balanced() {
        BALANCED
    } else {
        IMBALANCED
    };
    let verdict_text = if report.fully_balanced() {
        "Balanced across players."
    } else {
        "Imbalanced."
    };
    ui.colored_label(verdict_color, verdict_text);

    show_stats_block(ui, &report);
    ui.add_space(4.0);

    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            show_summary_table(ui, app, &report);
            ui.add_space(8.0);
            show_structure_breakdown(ui, app, &report.players);
        });
}

fn show_stats_block(ui: &mut Ui, report: &BalanceReport) {
    ui.horizontal_wrapped(|ui| {
        ui.label(format!("Players: {}", report.players.len()));
        ui.label("|");
        ui.label(format!("Total oil: {}", report.total_oil));
        if report.neutral_oil > 0 {
            ui.label(RichText::new(format!("({} unassigned)", report.neutral_oil)).color(MUTED));
        }
    });
    ui.horizontal_wrapped(|ui| {
        category_pill(ui, "Oil", report.oil_balanced);
        category_pill(ui, "Structures", report.structures_balanced);
        category_pill(ui, "Droids", report.droids_balanced);
    });
}

fn category_pill(ui: &mut Ui, label: &str, ok: bool) {
    let (color, suffix) = if ok {
        (BALANCED, "OK")
    } else {
        (IMBALANCED, "uneven")
    };
    ui.colored_label(color, format!("{label}: {suffix}"));
}

fn show_summary_table(ui: &mut Ui, app: &mut EditorApp, report: &BalanceReport) {
    egui::Grid::new("balance_summary")
        .num_columns(6)
        .striped(true)
        .show(ui, |ui| {
            for header in ["Show", "Player", "Start", "Oil", "Struct", "Droid"] {
                ui.label(RichText::new(header).strong());
            }
            ui.end_row();

            let oil_med = median(report.players.iter().map(|p| p.oil_count));
            let struct_med = median(report.players.iter().map(|p| p.structure_count));
            let droid_med = median(report.players.iter().map(|p| p.droid_count));

            for p in &report.players {
                let mut shown = app.balance.highlighted_players.contains(&p.player);
                if ui.checkbox(&mut shown, "").changed() {
                    if shown {
                        app.balance.highlighted_players.insert(p.player);
                    } else {
                        app.balance.highlighted_players.remove(&p.player);
                    }
                }

                let player_label = if p.has_hq {
                    RichText::new(format!("P{}", p.player)).color(player_color(p.player))
                } else {
                    RichText::new(format!("P{}*", p.player)).color(IMBALANCED)
                };
                let resp = ui
                    .selectable_label(false, player_label)
                    .on_hover_text(if p.has_hq {
                        "Click to focus camera on player start."
                    } else {
                        "No HQ; start tile is the first listed structure. Click to focus."
                    });
                if resp.clicked() {
                    let (tx, ty) = p.start_tile;
                    app.focus_request = Some((
                        (tx as f32 + 0.5) * TILE_UNITS_F32,
                        (ty as f32 + 0.5) * TILE_UNITS_F32,
                    ));
                }

                ui.label(format!("({}, {})", p.start_tile.0, p.start_tile.1));
                ui.colored_label(
                    cell_color(p.oil_count, oil_med, report.oil_balanced),
                    p.oil_count.to_string(),
                );
                ui.colored_label(
                    cell_color(p.structure_count, struct_med, report.structures_balanced),
                    p.structure_count.to_string(),
                );
                ui.colored_label(
                    cell_color(p.droid_count, droid_med, report.droids_balanced),
                    p.droid_count.to_string(),
                );
                ui.end_row();
            }
        });
}

fn show_structure_breakdown(ui: &mut Ui, app: &mut EditorApp, players: &[PlayerBalance]) {
    if players.is_empty() {
        return;
    }
    ui.collapsing("Structure breakdown", |ui| {
        ui.checkbox(
            &mut app.balance.breakdown_diff_only,
            "Show only differences",
        )
        .on_hover_text(
            "Hide structures every player has the same count of. What's left \
                 is what's making the layout uneven.",
        );

        // Names with identical counts across all players are "common" and
        // hidden when diff-only is on. Absence counts as zero, so names
        // only some players have stay visible as differences.
        let diff_only = app.balance.breakdown_diff_only;
        let common_names: std::collections::HashSet<String> = if diff_only && !players.is_empty() {
            let all_names: std::collections::BTreeSet<&String> =
                players.iter().flat_map(|p| p.structures.keys()).collect();
            all_names
                .into_iter()
                .filter(|name| {
                    let first = players[0].structures.get(*name).copied().unwrap_or(0);
                    players
                        .iter()
                        .all(|p| p.structures.get(*name).copied().unwrap_or(0) == first)
                })
                .cloned()
                .collect()
        } else {
            std::collections::HashSet::new()
        };

        for p in players {
            ui.label(
                RichText::new(format!("Player {}", p.player))
                    .strong()
                    .color(player_color(p.player)),
            );
            // Snapshot so the borrow on `p.structures` doesn't pin `app`
            // when we mutate `app.focus_request` / `breakdown_cycle`.
            let entries: Vec<(String, u32)> = p
                .structures
                .iter()
                .filter(|(name, _)| !diff_only || !common_names.contains(*name))
                .map(|(name, count)| (name.clone(), *count))
                .collect();
            let pid = p.player;
            if entries.is_empty() {
                ui.indent(("balance_breakdown_indent", pid), |ui| {
                    if diff_only {
                        ui.colored_label(MUTED, "matches all other players");
                    } else {
                        ui.colored_label(MUTED, "no structures placed");
                    }
                });
                continue;
            }
            ui.indent(("balance_breakdown_indent", pid), |ui| {
                for (name, count) in entries {
                    let resp = ui
                        .selectable_label(false, format!("{name} x {count}"))
                        .on_hover_text(
                            "Click to focus camera. Repeat clicks cycle through copies.",
                        );
                    if resp.clicked() {
                        focus_next_structure(app, pid, &name);
                    }
                }
            });
        }
    });
}

fn focus_next_structure(app: &mut EditorApp, player: i8, name: &str) {
    let Some(ref doc) = app.document else {
        return;
    };
    let positions: Vec<(f32, f32)> = doc
        .map
        .structures
        .iter()
        .filter(|s| s.player == player && s.name == name)
        .map(|s| (s.position.x as f32, s.position.y as f32))
        .collect();
    if positions.is_empty() {
        return;
    }
    let key = (player, name.to_string());
    let idx = app.balance.breakdown_cycle.entry(key).or_insert(0);
    let pos = positions[*idx % positions.len()];
    *idx = (*idx + 1) % positions.len();
    app.focus_request = Some(pos);
}

fn cell_color(value: u32, median: Option<u32>, category_balanced: bool) -> Color32 {
    if category_balanced {
        return Color32::PLACEHOLDER;
    }
    match median {
        Some(m) if value == m => MUTED,
        Some(_) => IMBALANCED,
        None => MUTED,
    }
}

fn median<I: Iterator<Item = u32>>(iter: I) -> Option<u32> {
    let mut v: Vec<u32> = iter.collect();
    if v.is_empty() {
        return None;
    }
    v.sort_unstable();
    Some(v[v.len() / 2])
}
