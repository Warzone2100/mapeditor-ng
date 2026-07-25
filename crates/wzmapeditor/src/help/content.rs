//! Compile-checked table of help topics and their authored markdown bodies.
//!
//! There is no manifest file and no runtime parse: the topic set is small and
//! fixed, so it lives in a `static` table whose markdown is embedded from
//! `help/*.md` with `include_str!`, matching the `SettingsPage::ALL` /
//! `Action::ALL` convention. Those files double as the project's user
//! documentation, so the README links to them for reading on GitHub.
//! The canonical identity of a topic is its `&'static str` slug, reused by the
//! sidebar, search, cross-links, F1 routing, and persistence.

/// Sidebar grouping for help topics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HelpCategory {
    GettingStarted,
    Maps,
    Terrain,
    Objects,
    Generator,
    Rendering,
    Analysis,
    Settings,
    Testing,
}

impl HelpCategory {
    /// Categories in sidebar display order.
    pub const ALL: &'static [Self] = &[
        Self::GettingStarted,
        Self::Maps,
        Self::Terrain,
        Self::Objects,
        Self::Generator,
        Self::Rendering,
        Self::Analysis,
        Self::Settings,
        Self::Testing,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::GettingStarted => "Getting Started",
            Self::Maps => "Maps",
            Self::Terrain => "Terrain",
            Self::Objects => "Objects",
            Self::Generator => "Generator",
            Self::Rendering => "Rendering",
            Self::Analysis => "Analysis",
            Self::Settings => "Settings",
            Self::Testing => "Testing",
        }
    }
}

/// A single help topic: metadata plus its embedded markdown body.
#[derive(Debug)]
pub struct HelpTopic {
    /// Stable slug, e.g. `"terrain-height-brush"`. Unique across [`TOPICS`].
    pub id: &'static str,
    pub title: &'static str,
    pub category: HelpCategory,
    /// Authored markdown, embedded at compile time.
    pub body: &'static str,
    /// Hidden in the web build (features that only exist natively).
    pub native_only: bool,
}

/// The slug shown when the browser opens with no prior selection.
pub const DEFAULT_TOPIC: &str = "getting-started";

pub static TOPICS: &[HelpTopic] = &[
    HelpTopic {
        id: "getting-started",
        title: "Getting Started",
        category: HelpCategory::GettingStarted,
        body: include_str!("../../help/getting-started.md"),
        native_only: false,
    },
    HelpTopic {
        id: "interface-overview",
        title: "Interface Overview",
        category: HelpCategory::GettingStarted,
        body: include_str!("../../help/interface-overview.md"),
        native_only: false,
    },
    HelpTopic {
        id: "mouse-gestures",
        title: "Mouse & Gestures",
        category: HelpCategory::GettingStarted,
        body: include_str!("../../help/mouse-gestures.md"),
        native_only: false,
    },
    HelpTopic {
        id: "maps-create",
        title: "Creating & Opening Maps",
        category: HelpCategory::Maps,
        body: include_str!("../../help/maps-create.md"),
        native_only: false,
    },
    HelpTopic {
        id: "maps-resize",
        title: "Resizing a Map",
        category: HelpCategory::Maps,
        body: include_str!("../../help/maps-resize.md"),
        native_only: false,
    },
    HelpTopic {
        id: "maps-properties",
        title: "Map Properties",
        category: HelpCategory::Maps,
        body: include_str!("../../help/maps-properties.md"),
        native_only: false,
    },
    HelpTopic {
        id: "maps-publish",
        title: "Publishing Maps",
        category: HelpCategory::Maps,
        body: include_str!("../../help/maps-publish.md"),
        native_only: true,
    },
    HelpTopic {
        id: "terrain",
        title: "Terrain Tools",
        category: HelpCategory::Terrain,
        body: include_str!("../../help/terrain.md"),
        native_only: false,
    },
    HelpTopic {
        id: "terrain-height-brush",
        title: "Height Brush",
        category: HelpCategory::Terrain,
        body: include_str!("../../help/terrain-height-brush.md"),
        native_only: false,
    },
    HelpTopic {
        id: "terrain-texture-paint",
        title: "Texture Painting",
        category: HelpCategory::Terrain,
        body: include_str!("../../help/terrain-texture-paint.md"),
        native_only: false,
    },
    HelpTopic {
        id: "terrain-ground-types",
        title: "Ground Types",
        category: HelpCategory::Terrain,
        body: include_str!("../../help/terrain-ground-types.md"),
        native_only: false,
    },
    HelpTopic {
        id: "terrain-vertex-sculpt",
        title: "Vertex Sculpt",
        category: HelpCategory::Terrain,
        body: include_str!("../../help/terrain-vertex-sculpt.md"),
        native_only: false,
    },
    HelpTopic {
        id: "terrain-stamp",
        title: "Stamp Tool",
        category: HelpCategory::Terrain,
        body: include_str!("../../help/terrain-stamp.md"),
        native_only: false,
    },
    HelpTopic {
        id: "terrain-walls",
        title: "Walls",
        category: HelpCategory::Terrain,
        body: include_str!("../../help/terrain-walls.md"),
        native_only: false,
    },
    HelpTopic {
        id: "terrain-mirror",
        title: "Mirroring Edits",
        category: HelpCategory::Terrain,
        body: include_str!("../../help/terrain-mirror.md"),
        native_only: false,
    },
    HelpTopic {
        id: "objects",
        title: "Placing Objects",
        category: HelpCategory::Objects,
        body: include_str!("../../help/objects.md"),
        native_only: false,
    },
    HelpTopic {
        id: "generator",
        title: "Map Generator",
        category: HelpCategory::Generator,
        body: include_str!("../../help/generator.md"),
        native_only: false,
    },
    HelpTopic {
        id: "rendering",
        title: "Rendering & Overlays",
        category: HelpCategory::Rendering,
        body: include_str!("../../help/rendering.md"),
        native_only: false,
    },
    HelpTopic {
        id: "minimap",
        title: "Minimap",
        category: HelpCategory::Rendering,
        body: include_str!("../../help/minimap.md"),
        native_only: false,
    },
    HelpTopic {
        id: "validation",
        title: "Validation & Problems",
        category: HelpCategory::Analysis,
        body: include_str!("../../help/validation.md"),
        native_only: false,
    },
    HelpTopic {
        id: "balance",
        title: "Balance Analysis",
        category: HelpCategory::Analysis,
        body: include_str!("../../help/balance.md"),
        native_only: false,
    },
    HelpTopic {
        id: "settings-graphics",
        title: "Graphics & Theme",
        category: HelpCategory::Settings,
        body: include_str!("../../help/settings-graphics.md"),
        native_only: false,
    },
    HelpTopic {
        id: "test-map",
        title: "Testing Your Map",
        category: HelpCategory::Testing,
        body: include_str!("../../help/test-map.md"),
        native_only: true,
    },
];

/// Look up a topic by its slug.
pub fn topic_by_id(id: &str) -> Option<&'static HelpTopic> {
    TOPICS.iter().find(|t| t.id == id)
}

/// How topics cross-reference one another: the target's own file name.
///
/// Every slug equals its file's stem, so a plain relative path serves both
/// readers — GitHub resolves it to the neighbouring file, and the browser
/// claims it with a link hook before it can escape as a URL.
pub fn topic_link(id: &str) -> String {
    format!("{id}.md")
}

/// Whether a topic should be offered in the current build. `web` hides
/// `native_only` topics.
pub fn topic_visible(topic: &HelpTopic, web: bool) -> bool {
    !(web && topic.native_only)
}

/// Topics in a category, filtered for the current build.
pub fn topics_in(cat: HelpCategory, web: bool) -> impl Iterator<Item = &'static HelpTopic> {
    TOPICS
        .iter()
        .filter(move |t| t.category == cat && topic_visible(t, web))
}

/// Whether the editor is running as the web (wasm32) build.
pub const fn is_web() -> bool {
    cfg!(target_arch = "wasm32")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    /// The topic a cross-link destination refers to, if it names one.
    fn topic_for_link(destination: &str) -> Option<&'static HelpTopic> {
        topic_by_id(destination.strip_suffix(".md")?)
    }

    /// Pull every inline link destination out of a markdown body, i.e. the text
    /// between `](` and `)`.
    fn link_destinations(body: &str) -> Vec<&str> {
        let mut found = Vec::new();
        let mut rest = body;
        while let Some(pos) = rest.find("](") {
            let tail = &rest[pos + 2..];
            match tail.find(')') {
                Some(end) => {
                    found.push(&tail[..end]);
                    rest = &tail[end..];
                }
                None => break,
            }
        }
        found
    }

    #[test]
    fn topic_ids_are_unique() {
        let mut seen = HashSet::new();
        for topic in TOPICS {
            assert!(seen.insert(topic.id), "duplicate topic id: {}", topic.id);
        }
    }

    #[test]
    fn every_category_is_in_all() {
        for topic in TOPICS {
            assert!(
                HelpCategory::ALL.contains(&topic.category),
                "topic {} has a category missing from HelpCategory::ALL",
                topic.id
            );
        }
    }

    #[test]
    fn bodies_are_non_empty() {
        for topic in TOPICS {
            assert!(
                !topic.body.trim().is_empty(),
                "topic {} has an empty body",
                topic.id
            );
        }
    }

    /// Any internal link that does not resolve to a topic gets no link hook, so
    /// clicking it in the browser escapes as a URL instead of navigating.
    #[test]
    fn cross_links_resolve_to_topics() {
        for topic in TOPICS {
            for dest in link_destinations(topic.body) {
                if dest.starts_with("http") || dest.starts_with("mailto:") {
                    continue;
                }
                assert!(
                    topic_for_link(dest).is_some(),
                    "topic {} links to `{dest}`, which is not a topic",
                    topic.id
                );
            }
        }
    }

    /// The cross-link form is the topic's own file name, which is what makes the
    /// bodies readable on GitHub as well as in the browser.
    #[test]
    fn topic_links_round_trip_through_file_names() {
        for topic in TOPICS {
            let link = topic_link(topic.id);
            assert_eq!(link, format!("{}.md", topic.id));
            assert_eq!(
                topic_for_link(&link).map(|t| t.id),
                Some(topic.id),
                "{link} should resolve back to its own topic"
            );
        }
    }

    /// A custom `wzhelp:` scheme renders as a dead link on GitHub, so topics
    /// must reference each other by file name.
    #[test]
    fn no_topic_uses_a_custom_link_scheme() {
        for topic in TOPICS {
            assert!(
                !topic.body.contains("wzhelp:"),
                "topic {} still uses a wzhelp: link",
                topic.id
            );
        }
    }

    #[test]
    fn default_topic_exists() {
        assert!(topic_by_id(DEFAULT_TOPIC).is_some());
    }
}
