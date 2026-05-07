//! Validation results panel. Displays map validation issues in a navigable tree.

use egui::{Color32, RichText, Ui};
use wz_maplib::constants::TILE_UNITS_F32;
use wz_maplib::validate::{IssueLocation, Severity, WarningRule};

use crate::app::{EditorApp, SelectedObject};

pub fn show_validation_panel(ui: &mut Ui, app: &mut EditorApp) {
    ui.horizontal(|ui| {
        if let Some(ref results) = app.validation_results {
            let p = results.problem_count();
            let w = results.warning_count();
            if p > 0 {
                ui.colored_label(Color32::from_rgb(255, 80, 80), format!("{p} problems"));
            }
            if w > 0 {
                ui.colored_label(Color32::from_rgb(255, 200, 60), format!("{w} warnings"));
            }
            if p == 0 && w == 0 {
                ui.colored_label(Color32::from_rgb(80, 220, 80), "No issues found");
            }
        }
    });

    ui.separator();

    let Some(ref results) = app.validation_results else {
        ui.label("Open a map to see validation results.");
        return;
    };

    if results.issues.is_empty() {
        ui.label("Map passed all validation checks.");
        return;
    }

    let mut focus_pos: Option<(f32, f32)> = None;
    let mut select_at: Option<(u32, u32)> = None;
    let mut disable_rule: Option<WarningRule> = None;

    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            for (category, issues) in results.by_category() {
                let header = format!("{} ({})", category.label(), issues.len());
                egui::CollapsingHeader::new(RichText::new(header).strong())
                    .default_open(true)
                    .show(ui, |ui| {
                        for issue in issues {
                            let color = match issue.severity {
                                Severity::Problem => Color32::from_rgb(255, 80, 80),
                                Severity::Warning => Color32::from_rgb(255, 200, 60),
                            };
                            let prefix = match issue.severity {
                                Severity::Problem => "ERROR",
                                Severity::Warning => "WARN",
                            };
                            let text =
                                RichText::new(format!("[{prefix}] {}", issue.message)).color(color);
                            let resp = ui.selectable_label(false, text);

                            if resp.double_clicked() {
                                match &issue.location {
                                    IssueLocation::WorldPos { x, y } => {
                                        focus_pos = Some((*x as f32, *y as f32));
                                        select_at = Some((*x, *y));
                                    }
                                    IssueLocation::TilePos { x, y } => {
                                        let wx = (*x as f32 + 0.5) * TILE_UNITS_F32;
                                        let wy = (*y as f32 + 0.5) * TILE_UNITS_F32;
                                        focus_pos = Some((wx, wy));
                                    }
                                    IssueLocation::None => {}
                                }
                            }

                            if let Some(rule) = issue.rule {
                                resp.context_menu(|ui| {
                                    if ui.button(format!("Disable \"{}\"", rule.label())).clicked()
                                    {
                                        disable_rule = Some(rule);
                                        ui.close();
                                    }
                                });
                            }

                            if !matches!(issue.location, IssueLocation::None) {
                                resp.on_hover_text("Double-click to navigate");
                            }
                        }
                    });
            }
        });

    if let Some(rule) = disable_rule {
        app.config.validation_config.disabled.insert(rule);
        app.config.save();
        app.validation_dirty = true;
    }

    if let Some((x, z)) = focus_pos {
        app.focus_request = Some((x, z));
    }

    if let Some((wx, wy)) = select_at
        && let Some(sel) = find_object_at(app, wx, wy)
    {
        app.selection.set_single(sel);
        app.objects_dirty = true;
    }
}

fn find_object_at(app: &EditorApp, x: u32, y: u32) -> Option<SelectedObject> {
    let doc = app.document.as_ref()?;

    for (i, s) in doc.map.structures.iter().enumerate() {
        if s.position.x == x && s.position.y == y {
            return Some(SelectedObject::Structure(i));
        }
    }
    for (i, d) in doc.map.droids.iter().enumerate() {
        if d.position.x == x && d.position.y == y {
            return Some(SelectedObject::Droid(i));
        }
    }
    for (i, f) in doc.map.features.iter().enumerate() {
        if f.position.x == x && f.position.y == y {
            return Some(SelectedObject::Feature(i));
        }
    }
    None
}
