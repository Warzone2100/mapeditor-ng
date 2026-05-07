//! Modal UI for the droid designer.
//!
//! Layout mirrors the in-game WZ2100 Design screen: slot-selector sidebar
//! on the right, component choice grid on the left, live 3D preview in the
//! centre. Per-sub-panel rendering lives in `DesignerTabs` in `tabs.rs`.

use egui::{Color32, RichText, Ui, Vec2};

use wz_stats::StatsDatabase;
use wz_stats::templates::TemplateStats;

use crate::designer::state::{Designer, DesignerCtx};
use crate::designer::tabs::{PREVIEW_ID, property_row};
use crate::designer::validation::{self, DroidFamily};

/// Derive the WZ2100 `droid_type` string from the current loadout.
///
/// Droid type is an emergent property of the body + turret combination,
/// not user-selectable. Mirrors WZ2100's after-the-fact classification.
fn derive_droid_type(buf: &TemplateStats, db: &StatsDatabase) -> &'static str {
    let body = db.bodies.get(&buf.body);
    let is_cyborg = body.is_some_and(wz_stats::bodies::BodyStats::is_cyborg);
    let is_super_cyborg = body.is_some_and(wz_stats::bodies::BodyStats::is_super_cyborg);
    let is_transporter = body.is_some_and(wz_stats::bodies::BodyStats::is_transporter);

    if is_transporter {
        return "TRANSPORTER";
    }
    if is_super_cyborg {
        return "CYBORG_SUPER";
    }
    if is_cyborg {
        // Support-turret variants have their own droid types.
        if buf.construct.is_some() {
            return "CYBORG_CONSTRUCT";
        }
        if buf.repair.is_some() {
            return "CYBORG_REPAIR";
        }
        return "CYBORG";
    }
    if buf.brain.is_some() {
        return "DROID_COMMAND";
    }
    if buf.construct.is_some() {
        return "CONSTRUCT";
    }
    if buf.repair.is_some() {
        return "REPAIR";
    }
    if buf.sensor.is_some() {
        return "SENSOR";
    }
    if buf.ecm.is_some() {
        return "ECM";
    }
    "WEAPON"
}

/// Swap in defaults that match the new droid family when the user changes
/// type, so they don't have to reset every slot manually.
fn retarget_for_family(buf: &mut TemplateStats, db: &StatsDatabase) {
    let family = validation::droid_family(buf.droid_type.as_deref());

    let need_new_body = match db.bodies.get(&buf.body) {
        Some(b) => !validation::body_selectable(b, family),
        None => true,
    };
    if need_new_body {
        if let Some((id, _)) = db
            .bodies
            .iter()
            .find(|(_, b)| validation::body_selectable(b, family))
        {
            buf.body.clone_from(id);
        } else {
            buf.body.clear();
        }
    }

    let need_new_prop = match db.propulsion.get(&buf.propulsion) {
        Some(p) => !validation::propulsion_allowed(p, family),
        None => true,
    };
    if need_new_prop {
        if let Some((id, _)) = db
            .propulsion
            .iter()
            .find(|(_, p)| validation::propulsion_allowed(p, family))
        {
            buf.propulsion.clone_from(id);
        } else {
            buf.propulsion.clear();
        }
    }

    let medium = db.propulsion.get(&buf.propulsion).map_or(
        validation::PropulsionMedium::Unknown,
        validation::propulsion_medium,
    );
    buf.weapons.retain(|w_id| {
        if w_id.is_empty() {
            return true;
        }
        db.weapons.get(w_id).is_some_and(|w| {
            validation::weapon_allowed(w, family) && validation::weapon_fits_propulsion(w, medium)
        })
    });

    if let Some(ref s) = buf.sensor.clone() {
        let want_cyborg = matches!(family, DroidFamily::Cyborg | DroidFamily::SuperCyborg);
        let ok = db
            .sensor
            .get(s)
            .is_some_and(|st| (st.usage_class.as_deref() == Some("Cyborg")) == want_cyborg);
        if !ok {
            buf.sensor = None;
        }
    }
    if let Some(ref s) = buf.ecm.clone() {
        let want_cyborg = matches!(family, DroidFamily::Cyborg | DroidFamily::SuperCyborg);
        let ok = db
            .ecm
            .get(s)
            .is_some_and(|st| (st.usage_class.as_deref() == Some("Cyborg")) == want_cyborg);
        if !ok {
            buf.ecm = None;
        }
    }
    if let Some(ref s) = buf.repair.clone() {
        let want_cyborg = matches!(family, DroidFamily::Cyborg | DroidFamily::SuperCyborg);
        let ok = db
            .repair
            .get(s)
            .is_some_and(|st| (st.usage_class.as_deref() == Some("Cyborg")) == want_cyborg);
        if !ok {
            buf.repair = None;
        }
    }
    if let Some(ref s) = buf.construct.clone() {
        let want_cyborg = matches!(family, DroidFamily::Cyborg | DroidFamily::SuperCyborg);
        let ok = db
            .construct
            .get(s)
            .is_some_and(|st| (st.usage_class.as_deref() == Some("Cyborg")) == want_cyborg);
        if !ok {
            buf.construct = None;
        }
    }
}

/// Draw the designer modal. No-op when `designer.open` is false.
pub fn show_designer(
    ctx: &egui::Context,
    designer: &mut Designer,
    mut dctx: DesignerCtx<'_>,
) -> crate::designer::state::DesignerOutcome {
    let mut outcome = crate::designer::state::DesignerOutcome::default();
    if !designer.open {
        return outcome;
    }

    // Auto-derive droid_type, then retarget incompatible parts when the
    // family changes (e.g. cyborg body picked).
    let derived_type = derive_droid_type(&designer.buffer, dctx.db).to_string();
    let prev_family = validation::droid_family(designer.buffer.droid_type.as_deref());
    let new_family = validation::droid_family(Some(derived_type.as_str()));
    designer.buffer.droid_type = Some(derived_type);
    if prev_family != new_family {
        retarget_for_family(&mut designer.buffer, dctx.db);
    }

    // Register the preview template so thumbnail generation and validation
    // operate on the live buffer. Preview texture updates in place every
    // frame in the tab's preview sub-panel.
    let mut preview_stats = designer.buffer.clone();
    preview_stats.id = PREVIEW_ID.into();
    preview_stats.name = Some(designer.name_buf.clone());
    dctx.db
        .templates
        .insert(PREVIEW_ID.to_string(), preview_stats);

    designer.issues = validation::validate(&designer.buffer, dctx.db);

    let mut close_requested = false;
    let mut save_requested = false;
    let mut save_as_new_requested = false;
    let mut window_open = true;

    egui::Window::new("Droid Designer")
        .open(&mut window_open)
        .collapsible(false)
        .resizable(true)
        .default_size([1100.0, 720.0])
        .min_size([900.0, 560.0])
        .show(ctx, |ui| {
            top_bar(ui, designer, dctx.db);
            ui.separator();

            // Move `tabs` out for the body grid and put it back to sidestep
            // the split-borrow conflict between `designer.tabs` and the rest
            // of `designer`.
            let mut tabs = std::mem::take(&mut designer.tabs);
            egui::Grid::new("designer_body_grid")
                .num_columns(3)
                .spacing([8.0, 0.0])
                .show(ui, |ui| {
                    ui.allocate_ui_with_layout(
                        Vec2::new(340.0, 520.0),
                        egui::Layout::top_down(egui::Align::Min),
                        |ui| tabs.component_grid(ui, designer, &mut dctx),
                    );

                    ui.allocate_ui_with_layout(
                        Vec2::new(460.0, 520.0),
                        egui::Layout::top_down(egui::Align::Center),
                        |ui| tabs.preview(ui, designer, &mut dctx),
                    );

                    ui.allocate_ui_with_layout(
                        Vec2::new(130.0, 520.0),
                        egui::Layout::top_down(egui::Align::Center),
                        |ui| tabs.slot_selector(ui, designer, &dctx),
                    );
                    ui.end_row();
                });
            designer.tabs = tabs;

            ui.separator();
            designer.tabs.validation_strip(ui, &designer.issues);
            ui.separator();

            ui.horizontal(|ui| {
                if let Some(ref err) = designer.last_save_error {
                    ui.colored_label(Color32::from_rgb(255, 100, 100), err);
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let save_ready = validation::is_save_ready(&designer.issues);
                    let save_btn = egui::Button::new(RichText::new("Save").strong());
                    if ui.add_enabled(save_ready, save_btn).clicked() {
                        save_requested = true;
                    }
                    if ui
                        .add_enabled(save_ready, egui::Button::new("Save as new"))
                        .clicked()
                    {
                        save_as_new_requested = true;
                    }
                    if ui.button("Cancel").clicked() {
                        close_requested = true;
                        outcome.cancelled = true;
                    }
                });
            });
        });

    // Save runs outside the window closure so its borrow on `designer` is released.
    if save_requested || save_as_new_requested {
        match commit_save(designer, &mut dctx, save_as_new_requested) {
            Ok(id) => {
                outcome.saved_template_id = Some(id);
                close_requested = true;
            }
            Err(e) => {
                designer.last_save_error = Some(e);
            }
        }
    }

    if !window_open {
        close_requested = true;
        outcome.cancelled = true;
    }

    if close_requested {
        // Keep the preview `TextureId` for the lifetime of `ThumbnailCache`.
        // Freeing here races the current frame's draw pass and trips the
        // `egui_texid_Managed(N) label has been destroyed` panic on shutdown
        // or reopen; the next open re-renders in place.
        dctx.db.templates.remove(PREVIEW_ID);
        designer.open = false;
    }

    outcome
}

fn commit_save(
    designer: &mut Designer,
    dctx: &mut DesignerCtx<'_>,
    force_new: bool,
) -> Result<String, String> {
    let id = if force_new || designer.editing_id.is_none() {
        dctx.store.fresh_id(dctx.db)
    } else {
        designer.editing_id.clone().unwrap()
    };

    let mut final_tpl = designer.buffer.clone();
    final_tpl.id.clone_from(&id);
    final_tpl.name = Some(designer.name_buf.clone());

    // Validate against the real id rather than the preview id.
    let issues = validation::validate(&final_tpl, dctx.db);
    if !validation::is_save_ready(&issues) {
        return Err("Template has errors. Fix them before saving".into());
    }

    dctx.store.insert(final_tpl, dctx.db);
    log::info!("Saved custom droid template '{id}'");
    Ok(id)
}

fn top_bar(ui: &mut Ui, designer: &mut Designer, db: &StatsDatabase) {
    ui.horizontal(|ui| {
        egui::Grid::new("designer_top_bar")
            .num_columns(2)
            .spacing([8.0, 0.0])
            .show(ui, |ui| {
                let buffer = &mut designer.buffer;
                let name_buf = &mut designer.name_buf;
                let changed = property_row(ui, "Name:", |ui| {
                    ui.add(egui::TextEdit::singleline(name_buf).desired_width(220.0))
                });
                if changed {
                    buffer.name = Some(name_buf.clone());
                }
            });

        ui.add_space(16.0);
        // Read-only: derived from the loadout (see `derive_droid_type`).
        let derived = designer.buffer.droid_type.as_deref().unwrap_or("WEAPON");
        ui.label(RichText::new("Type:").weak());
        ui.label(RichText::new(derived).monospace().weak());

        ui.add_space(16.0);
        let power = estimate_power_cost(&designer.buffer, db);
        ui.label(RichText::new(format!("Power: {power}")).strong());
    });
}

pub(crate) fn estimate_power_cost(t: &TemplateStats, db: &StatsDatabase) -> u32 {
    // Propulsion cost isn't parsed; weight / 2 is the same ballpark as
    // WZ2100's `WEIGHT_TO_POWER_RATIO`.
    const PROPULSION_WEIGHT_DIVISOR: u32 = 2;
    // Weapons / turrets don't have build-power fields parsed; flat 10
    // per weapon scales roughly with loadout size.
    const POWER_PER_WEAPON: u32 = 10;

    let mut cost = db
        .bodies
        .get(&t.body)
        .and_then(|b| b.build_power)
        .unwrap_or(0);
    if let Some(p) = db.propulsion.get(&t.propulsion) {
        cost += p.weight.unwrap_or(0) / PROPULSION_WEIGHT_DIVISOR;
    }
    cost += POWER_PER_WEAPON * t.weapons.iter().filter(|w| !w.is_empty()).count() as u32;
    cost
}
