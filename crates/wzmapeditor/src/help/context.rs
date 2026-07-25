//! Contextual help routing: map where the user currently is to a help topic.
//!
//! Drives F1 and the `?` affordances. The mappings are pure functions so they
//! can be unit-tested without egui.

use egui::{Align, Layout, Ui};

use super::content::DEFAULT_TOPIC;
use crate::app::{DockTab, EditorApp};
use crate::tools::ToolId;
use crate::ui::settings_window::SettingsPage;

/// Topic slug most relevant to the current UI focus.
///
/// Settings (when open) wins, then the active tool if the viewport holds focus,
/// then the focused panel, falling back to the getting-started topic.
pub fn topic_for_context(app: &mut EditorApp) -> &'static str {
    if app.settings_open {
        return topic_for_settings_page(app.settings_page);
    }
    match app.dock_focused_tab() {
        Some(DockTab::Viewport) | None => topic_for_tool(app.tool_state.active_tool),
        Some(tab) => topic_for_tab(&tab),
    }
}

/// Topic for an active tool.
pub fn topic_for_tool(tool: ToolId) -> &'static str {
    match tool {
        ToolId::HeightBrush => "terrain-height-brush",
        ToolId::TexturePaint => "terrain-texture-paint",
        ToolId::GroundTypePaint => "terrain-ground-types",
        ToolId::VertexSculpt => "terrain-vertex-sculpt",
        ToolId::Stamp => "terrain-stamp",
        ToolId::WallPlacement => "terrain-walls",
        ToolId::ObjectSelect | ToolId::ObjectPlace | ToolId::Gateway | ToolId::ScriptLabel => {
            "objects"
        }
    }
}

/// Topic for a focused dock panel.
pub fn topic_for_tab(tab: &DockTab) -> &'static str {
    match tab {
        DockTab::Viewport => "interface-overview",
        DockTab::Terrain => "terrain",
        DockTab::TilesetBrowser => "terrain-ground-types",
        DockTab::AssetBrowser | DockTab::Hierarchy | DockTab::Properties => "objects",
        DockTab::Minimap => "minimap",
        DockTab::Validation => "validation",
        DockTab::Balance => "balance",
        DockTab::OutputLog | DockTab::Unknown => DEFAULT_TOPIC,
    }
}

/// Topic for an open settings page.
pub fn topic_for_settings_page(page: SettingsPage) -> &'static str {
    match page {
        SettingsPage::Keybindings => "mouse-gestures",
        SettingsPage::Viewport | SettingsPage::Rendering => "settings-graphics",
        SettingsPage::Problems => "validation",
        SettingsPage::Maps => "maps-properties",
        SettingsPage::Game => "test-map",
        SettingsPage::AutoSave | SettingsPage::About => DEFAULT_TOPIC,
    }
}

/// Lay out `body` with a small `?` pinned to the panel's top-right corner,
/// opening contextual help for `slug`.
///
/// The button shares the body's first row instead of preceding it: on its own
/// row it cost a full row of height in every panel.
pub fn with_panel_help<R>(
    ui: &mut Ui,
    app: &mut EditorApp,
    slug: &'static str,
    body: impl FnOnce(&mut Ui, &mut EditorApp) -> R,
) -> R {
    ui.with_layout(Layout::right_to_left(Align::Min), |ui| {
        if ui
            .small_button("?")
            .on_hover_text("Open help for this panel (F1)")
            .clicked()
        {
            super::open_browser_at(app, slug);
        }
        // Everything left of the button, laid out normally.
        ui.vertical(|ui| body(ui, app)).inner
    })
    .inner
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::help::content::topic_by_id;

    #[test]
    fn every_tool_maps_to_a_real_topic() {
        let tools = [
            ToolId::HeightBrush,
            ToolId::TexturePaint,
            ToolId::GroundTypePaint,
            ToolId::VertexSculpt,
            ToolId::Stamp,
            ToolId::WallPlacement,
            ToolId::ObjectSelect,
            ToolId::ObjectPlace,
            ToolId::Gateway,
            ToolId::ScriptLabel,
        ];
        for tool in tools {
            let slug = topic_for_tool(tool);
            assert!(
                topic_by_id(slug).is_some(),
                "tool {tool:?} -> missing {slug}"
            );
        }
    }

    #[test]
    fn every_tab_maps_to_a_real_topic() {
        let tabs = [
            DockTab::Viewport,
            DockTab::Terrain,
            DockTab::TilesetBrowser,
            DockTab::AssetBrowser,
            DockTab::Hierarchy,
            DockTab::Properties,
            DockTab::Minimap,
            DockTab::Validation,
            DockTab::Balance,
            DockTab::OutputLog,
            DockTab::Unknown,
        ];
        for tab in &tabs {
            let slug = topic_for_tab(tab);
            assert!(topic_by_id(slug).is_some(), "tab {tab:?} -> missing {slug}");
        }
    }

    #[test]
    fn every_settings_page_maps_to_a_real_topic() {
        for page in SettingsPage::ALL {
            let slug = topic_for_settings_page(page);
            assert!(
                topic_by_id(slug).is_some(),
                "page {page:?} -> missing {slug}"
            );
        }
    }
}
