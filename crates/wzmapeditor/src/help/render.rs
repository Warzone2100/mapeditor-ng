//! The help browser window: sidebar navigation + commonmark content pane.
//!
//! Mirrors [`crate::ui::settings_window`]'s layout (resizable `Window`, left
//! nav, separator, right content). Markdown is rendered with `egui_commonmark`;
//! cross-links between topics are intercepted as in-app navigation via the
//! cache's link hooks, and external links are forced to open in a new tab so
//! the web SPA isn't unloaded.

use egui::{RichText, TextEdit, Ui};
use egui_commonmark::CommonMarkViewer;

use super::content::{self, HelpCategory, topic_by_id, topic_link};
use crate::app::EditorApp;

/// Fixed width of the topic sidebar, in points.
const SIDEBAR_WIDTH: f32 = 220.0;

pub fn show_help_window(ctx: &egui::Context, app: &mut EditorApp) {
    let mut open = app.help.open;

    let screen = ctx.content_rect();
    let default_pos = egui::pos2(
        (screen.width() - 760.0) * 0.5,
        (screen.height() - 560.0) * 0.5,
    );

    egui::Window::new("Help")
        .open(&mut open)
        .resizable(true)
        .collapsible(false)
        .default_size([760.0, 560.0])
        .min_size([480.0, 360.0])
        .default_pos(default_pos)
        .show(ctx, |ui| {
            show_help_contents(ui, app);
        });

    // Persist the last-viewed topic when the window is dismissed, rather than
    // on every navigation, to avoid config churn.
    if app.help.open && !open {
        app.config.help_last_topic = Some(app.help.current.clone());
        app.config.save();
    }
    app.help.open = open;
}

fn show_help_contents(ui: &mut Ui, app: &mut EditorApp) {
    let web = content::is_web();
    let mut pending_nav: Option<String> = None;

    // Both scroll areas need an explicit `id_salt`: `Ui::new_child` falls back
    // to a fixed salt, so sibling `vertical()`s share an id and their default
    // ScrollArea ids collide, which scrolls both panes together.
    ui.horizontal_top(|ui| {
        ui.vertical(|ui| {
            ui.set_min_width(SIDEBAR_WIDTH);
            ui.set_max_width(SIDEBAR_WIDTH);
            ui.add(
                TextEdit::singleline(&mut app.help.search_query)
                    .hint_text("Search help")
                    .desired_width(f32::INFINITY),
            );
            ui.add_space(4.0);
            ui.separator();
            egui::ScrollArea::vertical()
                .id_salt("help_nav_scroll")
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    if app.help.search_query.trim().is_empty() {
                        show_topic_tree(ui, app, web, &mut pending_nav);
                    } else {
                        show_search_results(ui, app, web, &mut pending_nav);
                    }
                });
        });

        ui.separator();

        ui.vertical(|ui| {
            egui::ScrollArea::vertical()
                .id_salt("help_content_scroll")
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    show_content_pane(ui, app, &mut pending_nav);
                });
        });
    });

    if let Some(slug) = pending_nav {
        app.help.navigate(&slug);
    }
}

fn show_topic_tree(ui: &mut Ui, app: &EditorApp, web: bool, pending_nav: &mut Option<String>) {
    for &cat in HelpCategory::ALL {
        let mut topics = content::topics_in(cat, web).peekable();
        if topics.peek().is_none() {
            continue;
        }
        ui.add_space(4.0);
        ui.label(RichText::new(cat.label()).strong().weak());
        ui.indent(cat.label(), |ui| {
            ui.visuals_mut().indent_has_left_vline = false;
            for topic in topics {
                let selected = app.help.current == topic.id;
                if ui.selectable_label(selected, topic.title).clicked() {
                    *pending_nav = Some(topic.id.to_string());
                }
            }
        });
    }
}

fn show_search_results(
    ui: &mut Ui,
    app: &mut EditorApp,
    web: bool,
    pending_nav: &mut Option<String>,
) {
    let hits = app.help.search_results(web);
    if hits.is_empty() {
        ui.add_space(4.0);
        ui.label(RichText::new("No matches.").weak());
        return;
    }
    for hit in hits {
        if ui.selectable_label(false, hit.title).clicked() {
            *pending_nav = Some(hit.id.to_string());
        }
        if !hit.snippet.is_empty() {
            ui.label(RichText::new(hit.snippet).weak().small());
        }
        ui.add_space(2.0);
    }
}

fn show_content_pane(ui: &mut Ui, app: &mut EditorApp, pending_nav: &mut Option<String>) {
    let current = app.help.current.clone();

    let Some(topic) = topic_by_id(&current) else {
        ui.label("This help topic is unavailable.");
        return;
    };

    // Topics cross-link each other as plain relative paths (`terrain.md`) so the
    // same files read correctly on GitHub. A hook per topic claims those
    // destinations for in-app navigation instead of letting them escape as URLs;
    // read which fired after show(), then clear so the set is rebuilt next frame.
    for t in content::TOPICS {
        app.help.cache.add_link_hook(topic_link(t.id));
    }

    // Inline `code` spans are mostly key names, and egui's default code
    // background is a solid mid-grey that reads as a bright block in running
    // text. A translucent tint keeps them legible against either theme.
    {
        let visuals = ui.visuals_mut();
        visuals.code_bg_color = if visuals.dark_mode {
            egui::Color32::from_white_alpha(24)
        } else {
            egui::Color32::from_black_alpha(20)
        };
    }

    // No `default_width`: it only raises the wrap width (the viewer takes the
    // max of it and the available width), which would let the content push the
    // window wider every frame.
    CommonMarkViewer::new().show(ui, &mut app.help.cache, topic.body);

    for t in content::TOPICS {
        if app.help.cache.get_link_hook(&topic_link(t.id)) == Some(true) {
            *pending_nav = Some(t.id.to_string());
        }
    }
    app.help.cache.link_hooks_clear();

    // Force any external link the body emitted to open in a new browser tab so
    // the web SPA is not navigated away from (and unsaved state lost).
    ui.ctx().output_mut(|o| {
        for cmd in &mut o.commands {
            if let egui::OutputCommand::OpenUrl(open) = cmd {
                open.new_tab = true;
            }
        }
    });
}
