//! Droid designer modal integration.

use super::EditorApp;

/// Drive the droid designer modal. Handles retargeting placed droids
/// when the user clicks "Save" from the property-panel entry point.
pub(super) fn update_designer(app: &mut EditorApp, ctx: &egui::Context) {
    if !app.designer.open {
        return;
    }
    let Some(ref mut stats) = app.stats else {
        // Designer needs the stats DB; close if unavailable.
        app.designer.open = false;
        return;
    };

    let dctx = crate::designer::state::DesignerCtx {
        db: stats,
        store: &mut app.custom_templates,
        thumbnails: &mut app.model_thumbnails,
        model_loader: &mut app.model_loader,
        render_state: app.wgpu_render_state.as_ref(),
        data_dir: app.config.data_dir.as_deref(),
    };
    let outcome = crate::designer::ui::show_designer(ctx, &mut app.designer, dctx);

    if let Some(new_id) = outcome.saved_template_id {
        // If opened from a placed droid, re-point it at the new template.
        if let (Some(idx), Some(ref mut doc)) =
            (app.designer.retarget_droid_index, app.document.as_mut())
            && let Some(d) = doc.map.droids.get_mut(idx)
        {
            log::info!("Re-pointing droid #{idx} from '{}' to '{new_id}'", d.name);
            d.name = new_id;
            doc.dirty = true;
            app.objects_dirty = true;
        }
        app.designer.retarget_droid_index = None;
        // Push templates.json payload into the document so save_to_wz writes it.
        refresh_custom_templates_json(app);
    }
}

/// Rebuild `document.map.custom_templates_json` from the store so
/// `save_to_wz_archive` writes the latest payload.
pub(super) fn refresh_custom_templates_json(app: &mut EditorApp) {
    let Some(ref mut doc) = app.document else {
        return;
    };
    doc.map.custom_templates_json = app.custom_templates.to_json();
    doc.dirty = true;
}
