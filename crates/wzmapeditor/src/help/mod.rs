//! In-app interactive help & onboarding.
//!
//! Markdown topics authored in `help/*.md` are embedded at compile time and
//! rendered into a searchable, cross-linked browser ([`render`]), augmented
//! with F1 contextual routing ([`context`]). One slug (`&'static str`) is the
//! canonical identity of a topic everywhere.

pub mod content;
pub mod context;
pub mod render;
pub mod search;

use egui_commonmark::CommonMarkCache;

use crate::app::EditorApp;
use content::{DEFAULT_TOPIC, topic_by_id};

/// All help/onboarding state, owned by [`EditorApp`] as a single field.
pub struct HelpState {
    pub open: bool,
    /// Slug of the currently shown topic.
    pub current: String,
    pub search_query: String,
    /// Must persist across frames so commonmark image handles and link hooks
    /// survive between renders.
    pub cache: CommonMarkCache,
    /// Built lazily on the first non-empty search query.
    search_index: Option<search::SearchIndex>,
}

impl Default for HelpState {
    fn default() -> Self {
        Self {
            open: false,
            current: DEFAULT_TOPIC.to_string(),
            search_query: String::new(),
            cache: CommonMarkCache::default(),
            search_index: None,
        }
    }
}

impl std::fmt::Debug for HelpState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HelpState")
            .field("open", &self.open)
            .field("current", &self.current)
            .finish_non_exhaustive()
    }
}

impl HelpState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the starting topic, e.g. restored from config.
    pub fn set_initial_topic(&mut self, slug: &str) {
        if topic_by_id(slug).is_some() {
            self.current = slug.to_string();
        }
    }

    /// Show a topic. Unknown slugs fall back to the default topic.
    pub fn navigate(&mut self, slug: &str) {
        let target = if topic_by_id(slug).is_some() {
            slug
        } else {
            DEFAULT_TOPIC
        };
        self.current = target.to_string();
    }

    /// Ranked search results for the current query, building the index lazily.
    /// Returns an empty list for a blank query.
    pub fn search_results(&mut self, web: bool) -> Vec<search::SearchHit> {
        if self.search_query.trim().is_empty() {
            return Vec::new();
        }
        let index = self
            .search_index
            .get_or_insert_with(search::SearchIndex::build);
        index.query(&self.search_query, web)
    }
}

/// Open the help browser at a specific topic (used by F1 and `?` buttons).
pub fn open_browser_at(app: &mut EditorApp, slug: &str) {
    app.help.open = true;
    app.help.navigate(slug);
}

/// Open the help browser at the last-viewed topic (used by the Help menu).
pub fn open_browser_home(app: &mut EditorApp) {
    app.help.open = true;
}
