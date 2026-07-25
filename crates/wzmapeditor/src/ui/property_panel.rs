//! Right sidebar editor for the current selection. Tool settings live in
//! the Terrain tool palette.

use egui::Ui;

use wz_maplib::WzMap;
use wz_maplib::constants::{MAX_PLAYERS, PLAYER_SCAVENGERS, TILE_SHIFT};
use wz_maplib::labels::ScriptLabel;
use wz_maplib::map_data::Gateway;
use wz_maplib::objects::{Droid, Feature, Structure};

use crate::app::{EditorApp, SelectedObject};
use crate::map::history::{CompoundCommand, EditCommand};
use crate::tools::gateway_tool::ReplaceGatewayCommand;
use crate::tools::label_tool::MoveLabelCommand;
use crate::tools::object_edit::ReplaceObjectCommand;

/// 90 degrees in WZ2100 internal units (0-65535 maps to 0-360).
const DIRECTION_QUARTER: u16 = 16384;

/// A saved copy of whichever object the panel is editing.
#[derive(Debug, Clone)]
enum PropertyValue {
    Structure(Structure),
    Droid(Droid),
    Feature(Feature),
    Label(ScriptLabel),
    Gateway(Gateway),
}

/// A panel edit that has not reached the undo history yet.
///
/// A `DragValue` reports a change on every frame it is dragged, so pushing an
/// undo entry per change would fill the stack with one entry per pixel of drag.
/// The pre-edit copy is taken once when the gesture starts and becomes a single
/// entry once the pointer is released and keyboard focus has gone.
#[derive(Debug)]
pub struct PropertyEdit {
    target: SelectedObject,
    before: PropertyValue,
}

/// Collects, across all of the panel's widgets, whether anything changed this
/// frame and whether the user is still mid-gesture.
#[derive(Default)]
struct EditActivity {
    changed: bool,
    active: bool,
}

impl EditActivity {
    /// Fold a widget response in, returning whether that widget changed.
    fn note(&mut self, response: &egui::Response) -> bool {
        self.active |= response.dragged() || response.has_focus();
        self.changed |= response.changed();
        response.changed()
    }

    /// Fold in a widget that reports a change without exposing a `Response`.
    fn note_changed(&mut self, changed: bool) {
        self.changed |= changed;
    }
}

/// Labelled drag for a world coordinate, clamped at zero.
fn coord_drag(ui: &mut Ui, act: &mut EditActivity, label: &str, value: &mut u32) {
    ui.label(label);
    let mut v = *value as i32;
    let response = ui.add(egui::DragValue::new(&mut v).speed(16));
    if act.note(&response) {
        *value = v.max(0) as u32;
    }
}

/// Labelled drag for a gateway endpoint, clamped to the map bounds.
fn gateway_drag(ui: &mut Ui, act: &mut EditActivity, label: &str, value: &mut u8, max: i32) {
    ui.label(label);
    let mut v = i32::from(*value);
    let response = ui.add(egui::DragValue::new(&mut v).range(0..=max));
    if act.note(&response) {
        *value = v.clamp(0, max) as u8;
    }
}

/// Copy the currently selected object out of the map.
fn snapshot(map: &WzMap, target: SelectedObject) -> Option<PropertyValue> {
    match target {
        SelectedObject::Structure(i) => {
            map.structures.get(i).cloned().map(PropertyValue::Structure)
        }
        SelectedObject::Droid(i) => map.droids.get(i).cloned().map(PropertyValue::Droid),
        SelectedObject::Feature(i) => map.features.get(i).cloned().map(PropertyValue::Feature),
        SelectedObject::Label(i) => map
            .labels
            .get(i)
            .map(|(_, label)| PropertyValue::Label(label.clone())),
        SelectedObject::Gateway(i) => map
            .map_data
            .gateways
            .get(i)
            .copied()
            .map(PropertyValue::Gateway),
    }
}

/// Build the undo command for a completed edit. Returns `None` if the two
/// snapshots are of different kinds, which cannot happen for a single target.
fn replace_command(
    target: SelectedObject,
    before: PropertyValue,
    after: PropertyValue,
) -> Option<Box<dyn EditCommand>> {
    let index = match target {
        SelectedObject::Structure(i)
        | SelectedObject::Droid(i)
        | SelectedObject::Feature(i)
        | SelectedObject::Label(i)
        | SelectedObject::Gateway(i) => i,
    };
    match (before, after) {
        (PropertyValue::Structure(b), PropertyValue::Structure(a)) => {
            Some(Box::new(ReplaceObjectCommand::structure(index, b, a)))
        }
        (PropertyValue::Droid(b), PropertyValue::Droid(a)) => {
            Some(Box::new(ReplaceObjectCommand::droid(index, b, a)))
        }
        (PropertyValue::Feature(b), PropertyValue::Feature(a)) => {
            Some(Box::new(ReplaceObjectCommand::feature(index, b, a)))
        }
        (PropertyValue::Label(b), PropertyValue::Label(a)) => Some(Box::new(MoveLabelCommand {
            index,
            old_label: b,
            new_label: a,
        })),
        (PropertyValue::Gateway(b), PropertyValue::Gateway(a)) => {
            Some(Box::new(ReplaceGatewayCommand {
                index,
                before: b,
                after: a,
            }))
        }
        _ => None,
    }
}

/// Snap a raw direction value to the nearest 90 degree step (0..=3).
fn direction_to_step(dir: u16) -> u8 {
    (((u32::from(dir) + u32::from(DIRECTION_QUARTER) / 2) / u32::from(DIRECTION_QUARTER)) % 4) as u8
}

/// Render the 4 rotation buttons (0/90/180/270). Pass `None` for the
/// multi-select "varies" case.
fn rotation_buttons(ui: &mut Ui, current: Option<u8>) -> Option<u8> {
    let mut clicked = None;
    for s in 0u8..4 {
        let label = format!("{}°", u32::from(s) * 90);
        let btn = egui::Button::new(label)
            .small()
            .selected(current == Some(s));
        if ui.add(btn).clicked() {
            clicked = Some(s);
        }
    }
    clicked
}

fn rotation_widget(ui: &mut Ui, dir: &mut u16) -> bool {
    let cur = direction_to_step(*dir);
    match rotation_buttons(ui, Some(cur)) {
        Some(s) if s != cur => {
            *dir = u16::from(s) * DIRECTION_QUARTER;
            true
        }
        _ => false,
    }
}

fn player_label(p: i8) -> String {
    if p == PLAYER_SCAVENGERS {
        "Scavenger".to_string()
    } else {
        format!("Player {p}")
    }
}

/// Player picker shared by the property panel and the Asset Browser so both
/// present the same "Scavenger / Player N" combo. Returns true when changed.
pub(crate) fn player_widget(
    ui: &mut Ui,
    p: &mut i8,
    salt: impl std::hash::Hash + std::fmt::Debug,
) -> bool {
    let original = *p;
    egui::ComboBox::from_id_salt(salt)
        .selected_text(player_label(*p))
        .show_ui(ui, |ui| {
            ui.selectable_value(p, PLAYER_SCAVENGERS, "Scavenger");
            for n in 0..MAX_PLAYERS as i8 {
                ui.selectable_value(p, n, format!("Player {n}"));
            }
        });
    *p != original
}

fn show_tile_coords(ui: &mut Ui, world_x: u32, world_y: u32) {
    let tx = world_x >> TILE_SHIFT;
    let ty = world_y >> TILE_SHIFT;
    ui.label(
        egui::RichText::new(format!("Tile: ({tx}, {ty})"))
            .small()
            .weak(),
    );
}

fn show_id_field(ui: &mut Ui, id: Option<u32>) {
    let text = match id {
        Some(v) => format!("ID: {v}"),
        None => "ID: (unassigned)".to_string(),
    };
    ui.label(egui::RichText::new(text).small().weak());
}

pub fn show_property_panel(ui: &mut Ui, app: &mut EditorApp) {
    ui.heading("Selection");
    ui.separator();

    if app.document.is_none() {
        ui.label("No map loaded.");
        ui.label("Use File > New Map or Open to get started.");
        return;
    }

    show_selected_object_props(ui, app);

    if matches!(
        app.tool_state.active_tool,
        crate::tools::ToolId::Gateway | crate::tools::ToolId::ScriptLabel
    ) {
        ui.separator();
        match app.tool_state.active_tool {
            crate::tools::ToolId::Gateway => show_gateway_list(ui, app),
            crate::tools::ToolId::ScriptLabel => show_label_list(ui, app),
            _ => {}
        }
    }
}

fn show_selected_object_props(ui: &mut Ui, app: &mut EditorApp) {
    if app.selection.len() > 1 {
        show_multi_selection_props(ui, app);
        return;
    }
    let Some(sel) = app.selection.single() else {
        ui.label("Click objects in the viewport to select them.");
        return;
    };

    let Some(doc) = app.document.as_mut() else {
        return;
    };

    let before = snapshot(&doc.map, sel);
    let mut act = EditActivity::default();

    match sel {
        SelectedObject::Structure(i) => {
            if let Some(s) = doc.map.structures.get_mut(i) {
                ui.label(format!("Structure: {}", s.name));
                show_id_field(ui, s.id);

                ui.horizontal(|ui| {
                    coord_drag(ui, &mut act, "X:", &mut s.position.x);
                    coord_drag(ui, &mut act, "Y:", &mut s.position.y);
                });
                show_tile_coords(ui, s.position.x, s.position.y);

                ui.horizontal(|ui| {
                    ui.label("Rotation:");
                    act.note_changed(rotation_widget(ui, &mut s.direction));
                });

                ui.horizontal(|ui| {
                    ui.label("Player:");
                    act.note_changed(player_widget(ui, &mut s.player, ("struct_player", i)));
                });

                ui.horizontal(|ui| {
                    ui.label("Modules:");
                    let mut m = s.modules as i32;
                    let response = ui.add(egui::DragValue::new(&mut m).range(0..=4));
                    if act.note(&response) {
                        s.modules = m as u8;
                    }
                });
            }
        }
        SelectedObject::Droid(i) => {
            if let Some(d) = doc.map.droids.get_mut(i) {
                let droid_name = d.name.clone();
                ui.label(format!("Droid: {droid_name}"));
                show_id_field(ui, d.id);

                ui.horizontal(|ui| {
                    coord_drag(ui, &mut act, "X:", &mut d.position.x);
                    coord_drag(ui, &mut act, "Y:", &mut d.position.y);
                });
                show_tile_coords(ui, d.position.x, d.position.y);

                ui.horizontal(|ui| {
                    ui.label("Rotation:");
                    act.note_changed(rotation_widget(ui, &mut d.direction));
                });

                ui.horizontal(|ui| {
                    ui.label("Player:");
                    act.note_changed(player_widget(ui, &mut d.player, ("droid_player", i)));
                });
            }
        }
        SelectedObject::Feature(i) => {
            if let Some(f) = doc.map.features.get_mut(i) {
                ui.label(format!("Feature: {}", f.name));
                show_id_field(ui, f.id);

                ui.horizontal(|ui| {
                    coord_drag(ui, &mut act, "X:", &mut f.position.x);
                    coord_drag(ui, &mut act, "Y:", &mut f.position.y);
                });
                show_tile_coords(ui, f.position.x, f.position.y);

                ui.horizontal(|ui| {
                    ui.label("Rotation:");
                    act.note_changed(rotation_widget(ui, &mut f.direction));
                });
            }
        }
        SelectedObject::Label(i) => {
            if let Some((key, label)) = doc.map.labels.get_mut(i) {
                let type_name = match label {
                    ScriptLabel::Position { .. } => "Position",
                    ScriptLabel::Area { .. } => "Area",
                };
                ui.label(format!("Label: {} ({type_name})", label.label()));
                ui.label(format!("Key: {key}"));

                match label {
                    ScriptLabel::Position { pos, label: name } => {
                        ui.horizontal(|ui| {
                            ui.label("Name:");
                            let response = ui.text_edit_singleline(name);
                            act.note(&response);
                        });
                        ui.horizontal(|ui| {
                            coord_drag(ui, &mut act, "X:", &mut pos[0]);
                            coord_drag(ui, &mut act, "Y:", &mut pos[1]);
                        });
                    }
                    ScriptLabel::Area {
                        pos1,
                        pos2,
                        label: name,
                    } => {
                        ui.horizontal(|ui| {
                            ui.label("Name:");
                            let response = ui.text_edit_singleline(name);
                            act.note(&response);
                        });
                        ui.horizontal(|ui| {
                            coord_drag(ui, &mut act, "X1:", &mut pos1[0]);
                            coord_drag(ui, &mut act, "Y1:", &mut pos1[1]);
                        });
                        ui.horizontal(|ui| {
                            coord_drag(ui, &mut act, "X2:", &mut pos2[0]);
                            coord_drag(ui, &mut act, "Y2:", &mut pos2[1]);
                        });
                    }
                }
            }
        }
        SelectedObject::Gateway(i) => {
            let max_x = doc.map.map_data.width.saturating_sub(1) as i32;
            let max_y = doc.map.map_data.height.saturating_sub(1) as i32;
            if let Some(gw) = doc.map.map_data.gateways.get_mut(i) {
                ui.label(format!("Gateway #{i}"));

                ui.horizontal(|ui| {
                    gateway_drag(ui, &mut act, "X1:", &mut gw.x1, max_x);
                    gateway_drag(ui, &mut act, "Y1:", &mut gw.y1, max_y);
                });
                ui.horizontal(|ui| {
                    gateway_drag(ui, &mut act, "X2:", &mut gw.x2, max_x);
                    gateway_drag(ui, &mut act, "Y2:", &mut gw.y2, max_y);
                });
            }
        }
    }

    if act.changed {
        doc.dirty = true;
        match sel {
            SelectedObject::Structure(_)
            | SelectedObject::Droid(_)
            | SelectedObject::Feature(_) => {
                app.objects_dirty = true;
            }
            SelectedObject::Label(_) | SelectedObject::Gateway(_) => {
                app.validation_dirty = true;
            }
        }
        if app.property_edit.is_none()
            && let Some(before) = before
        {
            app.property_edit = Some(PropertyEdit {
                target: sel,
                before,
            });
        }
    }

    // Record the gesture as one undo entry, once the drag is released and any
    // keyboard focus has moved on.
    if !act.active
        && let Some(edit) = app.property_edit.take()
        && let Some(after) = snapshot(&doc.map, edit.target)
        && let Some(cmd) = replace_command(edit.target, edit.before, after)
    {
        doc.history.push_already_applied(cmd);
    }
}

/// Merge a per-object value into a running "common across selection" tracker.
/// Once values disagree, `varies` latches true and `common` clears.
fn merge_common<T: Eq + Copy>(common: &mut Option<T>, varies: &mut bool, value: T) {
    if *varies {
        return;
    }
    match *common {
        Some(v) if v != value => {
            *varies = true;
            *common = None;
        }
        Some(_) => {}
        None => *common = Some(value),
    }
}

/// Property editor shown when more than one object is selected.
fn show_multi_selection_props(ui: &mut Ui, app: &mut EditorApp) {
    let Some(doc) = app.document.as_mut() else {
        return;
    };

    let mut struct_count = 0usize;
    let mut droid_count = 0usize;
    let mut feat_count = 0usize;
    let mut player_targets = 0usize;
    let mut rot_targets = 0usize;
    let mut common_player: Option<i8> = None;
    let mut player_varies = false;
    let mut common_rot: Option<u8> = None;
    let mut rot_varies = false;

    for obj in &app.selection.objects {
        match obj {
            SelectedObject::Structure(i) => {
                if let Some(s) = doc.map.structures.get(*i) {
                    struct_count += 1;
                    player_targets += 1;
                    rot_targets += 1;
                    merge_common(&mut common_player, &mut player_varies, s.player);
                    merge_common(
                        &mut common_rot,
                        &mut rot_varies,
                        direction_to_step(s.direction),
                    );
                }
            }
            SelectedObject::Droid(i) => {
                if let Some(d) = doc.map.droids.get(*i) {
                    droid_count += 1;
                    player_targets += 1;
                    rot_targets += 1;
                    merge_common(&mut common_player, &mut player_varies, d.player);
                    merge_common(
                        &mut common_rot,
                        &mut rot_varies,
                        direction_to_step(d.direction),
                    );
                }
            }
            SelectedObject::Feature(i) => {
                if let Some(f) = doc.map.features.get(*i) {
                    feat_count += 1;
                    rot_targets += 1;
                    merge_common(
                        &mut common_rot,
                        &mut rot_varies,
                        direction_to_step(f.direction),
                    );
                    if let Some(p) = f.player {
                        player_targets += 1;
                        merge_common(&mut common_player, &mut player_varies, p);
                    }
                }
            }
            SelectedObject::Label(_) | SelectedObject::Gateway(_) => {}
        }
    }

    let total = struct_count + droid_count + feat_count;
    ui.label(format!("{total} objects selected"));
    ui.label(
        egui::RichText::new(format!(
            "({struct_count} structs, {droid_count} droids, {feat_count} feats)"
        ))
        .small()
        .weak(),
    );

    if total == 0 {
        return;
    }

    let mut new_rot_step: Option<u8> = None;
    let mut new_player: Option<i8> = None;

    if rot_targets > 0 {
        ui.horizontal(|ui| {
            ui.label("Rotation:");
            if rot_varies {
                ui.label(egui::RichText::new("(varies)").weak());
            }
        });
        ui.horizontal(|ui| {
            ui.add_space(20.0);
            let highlight = if rot_varies { None } else { common_rot };
            if let Some(s) = rotation_buttons(ui, highlight) {
                new_rot_step = Some(s);
            }
        });
    }

    if player_targets > 0 {
        ui.horizontal(|ui| {
            ui.label("Player:");
            let display = if player_varies {
                "(varies)".to_string()
            } else {
                common_player.map_or_else(String::new, player_label)
            };
            egui::ComboBox::from_id_salt("multi_player")
                .selected_text(display)
                .show_ui(ui, |ui| {
                    if ui.selectable_label(false, "Scavenger").clicked() {
                        new_player = Some(PLAYER_SCAVENGERS);
                    }
                    for n in 0..MAX_PLAYERS as i8 {
                        if ui.selectable_label(false, format!("Player {n}")).clicked() {
                            new_player = Some(n);
                        }
                    }
                });
        });
    }

    if new_rot_step.is_none() && new_player.is_none() {
        return;
    }

    // Applying to the whole selection is one click, so it is recorded as a
    // single compound entry rather than one per object.
    let mut commands: Vec<Box<dyn EditCommand>> = Vec::new();
    for obj in app.selection.objects.clone() {
        match obj {
            SelectedObject::Structure(i) => {
                if let Some(s) = doc.map.structures.get_mut(i) {
                    let before = s.clone();
                    let mut touched = false;
                    if let Some(step) = new_rot_step {
                        s.direction = u16::from(step) * DIRECTION_QUARTER;
                        touched = true;
                    }
                    if let Some(p) = new_player {
                        s.player = p;
                        touched = true;
                    }
                    if touched {
                        commands.push(Box::new(ReplaceObjectCommand::structure(
                            i,
                            before,
                            s.clone(),
                        )));
                    }
                }
            }
            SelectedObject::Droid(i) => {
                if let Some(d) = doc.map.droids.get_mut(i) {
                    let before = d.clone();
                    let mut touched = false;
                    if let Some(step) = new_rot_step {
                        d.direction = u16::from(step) * DIRECTION_QUARTER;
                        touched = true;
                    }
                    if let Some(p) = new_player {
                        d.player = p;
                        touched = true;
                    }
                    if touched {
                        commands.push(Box::new(ReplaceObjectCommand::droid(i, before, d.clone())));
                    }
                }
            }
            SelectedObject::Feature(i) => {
                if let Some(f) = doc.map.features.get_mut(i) {
                    let before = f.clone();
                    let mut touched = false;
                    if let Some(step) = new_rot_step {
                        f.direction = u16::from(step) * DIRECTION_QUARTER;
                        touched = true;
                    }
                    if let Some(p) = new_player
                        && f.player.is_some()
                    {
                        f.player = Some(p);
                        touched = true;
                    }
                    if touched {
                        commands.push(Box::new(ReplaceObjectCommand::feature(
                            i,
                            before,
                            f.clone(),
                        )));
                    }
                }
            }
            SelectedObject::Label(_) | SelectedObject::Gateway(_) => {}
        }
    }

    if !commands.is_empty() {
        doc.dirty = true;
        doc.history
            .push_already_applied(Box::new(CompoundCommand::new(commands)));
        app.objects_dirty = true;
    }
}

fn show_gateway_list(ui: &mut Ui, app: &mut EditorApp) {
    let Some(doc) = app.document.as_ref() else {
        return;
    };

    if doc.map.map_data.gateways.is_empty() {
        ui.label("No gateways.");
        return;
    }

    ui.label(format!("{} gateways:", doc.map.map_data.gateways.len()));

    let mut delete_idx = None;

    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            for (i, gw) in doc.map.map_data.gateways.iter().enumerate() {
                ui.horizontal(|ui| {
                    let label = format!("#{}: ({},{}) - ({},{})", i, gw.x1, gw.y1, gw.x2, gw.y2);
                    ui.label(&label);
                    if ui.small_button("X").clicked() {
                        delete_idx = Some(i);
                    }
                });
            }
        });

    if let Some(idx) = delete_idx {
        let Some(doc) = app.document.as_mut() else {
            return;
        };
        let gw = doc.map.map_data.gateways[idx];
        let cmd = crate::tools::gateway_tool::DeleteGatewayCommand {
            index: idx,
            saved: gw,
        };
        cmd.execute(&mut doc.map);
        doc.history.push_already_applied(Box::new(cmd));
        doc.dirty = true;
        app.validation_dirty = true;
    }
}

fn show_label_list(ui: &mut Ui, app: &mut EditorApp) {
    let Some(doc) = app.document.as_ref() else {
        return;
    };

    if doc.map.labels.is_empty() {
        ui.label("No labels.");
        return;
    }

    ui.label(format!("{} labels:", doc.map.labels.len()));

    let mut delete_idx = None;

    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            for (i, (key, label)) in doc.map.labels.iter().enumerate() {
                ui.horizontal(|ui| {
                    let info = match label {
                        wz_maplib::labels::ScriptLabel::Position { label: name, pos } => {
                            format!("{key}: \"{name}\" ({}, {})", pos[0], pos[1])
                        }
                        wz_maplib::labels::ScriptLabel::Area {
                            label: name,
                            pos1,
                            pos2,
                        } => {
                            format!(
                                "{key}: \"{name}\" ({},{})..({},{})",
                                pos1[0], pos1[1], pos2[0], pos2[1]
                            )
                        }
                    };
                    ui.label(egui::RichText::new(&info).small());
                    if ui.small_button("X").clicked() {
                        delete_idx = Some(i);
                    }
                });
            }
        });

    if let Some(idx) = delete_idx {
        let Some(doc) = app.document.as_mut() else {
            return;
        };
        let (saved_key, saved_label) = doc.map.labels[idx].clone();
        let cmd = crate::tools::label_tool::DeleteLabelCommand {
            index: idx,
            saved_key,
            saved_label,
        };
        cmd.execute(&mut doc.map);
        doc.history.push_already_applied(Box::new(cmd));
        doc.dirty = true;
        app.validation_dirty = true;
    }
}
