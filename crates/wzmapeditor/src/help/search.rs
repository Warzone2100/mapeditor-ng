//! Lazy in-memory full-text search over help topics.
//!
//! The index is built once on the first non-empty query and ranks a title
//! match above a body match, and an earlier match above a later one. All
//! logic is independent of egui so it can be unit-tested directly.

use super::content::TOPICS;

/// One ranked search result.
#[derive(Debug, Clone)]
pub struct SearchHit {
    pub id: &'static str,
    pub title: &'static str,
    pub snippet: String,
}

/// Pre-lowercased haystacks for ranking, built once and reused.
#[derive(Debug)]
pub struct SearchIndex {
    entries: Vec<Entry>,
}

#[derive(Debug)]
struct Entry {
    id: &'static str,
    title: &'static str,
    native_only: bool,
    title_lc: String,
    body_lc: String,
    body_plain: String,
}

/// Characters around a body match included in the result snippet.
const SNIPPET_RADIUS: usize = 70;

impl SearchIndex {
    /// Build the index over every topic. Native-only topics are kept here and
    /// filtered at query time so one index serves both builds.
    pub fn build() -> Self {
        let entries = TOPICS
            .iter()
            .map(|t| {
                let body_plain = strip_markdown(t.body);
                Entry {
                    id: t.id,
                    title: t.title,
                    native_only: t.native_only,
                    title_lc: t.title.to_lowercase(),
                    body_lc: body_plain.to_lowercase(),
                    body_plain,
                }
            })
            .collect();
        Self { entries }
    }

    /// Rank topics against `query`. Returns an empty list for a blank query.
    /// When `web` is true, native-only topics are excluded.
    pub fn query(&self, query: &str, web: bool) -> Vec<SearchHit> {
        let needle = query.trim().to_lowercase();
        if needle.is_empty() {
            return Vec::new();
        }

        // Rank key: (bucket, position). Bucket 0 = title match, 1 = body match;
        // lower is better, so title hits sort ahead of body hits and earlier
        // matches ahead of later ones.
        let mut ranked: Vec<(u8, usize, &Entry)> = Vec::new();
        for entry in &self.entries {
            if web && entry.native_only {
                continue;
            }
            if let Some(pos) = entry.title_lc.find(&needle) {
                ranked.push((0, pos, entry));
            } else if let Some(pos) = entry.body_lc.find(&needle) {
                ranked.push((1, pos, entry));
            }
        }
        ranked.sort_by_key(|(bucket, pos, _)| (*bucket, *pos));

        ranked
            .into_iter()
            .map(|(bucket, pos, entry)| SearchHit {
                id: entry.id,
                title: entry.title,
                snippet: if bucket == 0 {
                    snippet(&entry.body_plain, 0, false)
                } else {
                    snippet(&entry.body_plain, pos, true)
                },
            })
            .collect()
    }
}

/// Build a short snippet of `plain` centred on `pos`. With `centered`, the
/// window is taken around the match and prefixed with an ellipsis when it does
/// not start at the beginning; otherwise the leading text is used.
fn snippet(plain: &str, pos: usize, centered: bool) -> String {
    if plain.is_empty() {
        return String::new();
    }
    let start = if centered {
        floor_boundary(plain, pos.saturating_sub(SNIPPET_RADIUS))
    } else {
        0
    };
    let end = ceil_boundary(plain, (start + SNIPPET_RADIUS * 2).min(plain.len()));
    let mut out = String::new();
    if start > 0 {
        out.push('\u{2026}');
    }
    out.push_str(plain[start..end].trim());
    if end < plain.len() {
        out.push('\u{2026}');
    }
    out
}

fn floor_boundary(s: &str, mut i: usize) -> usize {
    while i > 0 && !s.is_char_boundary(i) {
        i -= 1;
    }
    i
}

fn ceil_boundary(s: &str, mut i: usize) -> usize {
    let len = s.len();
    while i < len && !s.is_char_boundary(i) {
        i += 1;
    }
    i
}

/// Strip the common markdown syntax so the haystack and snippets read as plain
/// prose. Link text is kept and the trailing `(url)` dropped; headings, emphasis,
/// code ticks, blockquotes and table pipes are removed; whitespace is collapsed.
pub fn strip_markdown(md: &str) -> String {
    let mut out = String::with_capacity(md.len());
    let mut chars = md.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '#' | '*' | '_' | '`' | '>' | '|' | '[' => {}
            ']' => {
                if chars.peek() == Some(&'(') {
                    chars.next();
                    for d in chars.by_ref() {
                        if d == ')' {
                            break;
                        }
                    }
                }
            }
            '\n' | '\r' | '\t' => out.push(' '),
            other => out.push(other),
        }
    }

    let mut collapsed = String::with_capacity(out.len());
    let mut prev_space = false;
    for c in out.chars() {
        if c == ' ' {
            if !prev_space {
                collapsed.push(' ');
            }
            prev_space = true;
        } else {
            collapsed.push(c);
            prev_space = false;
        }
    }
    collapsed.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_query_returns_no_hits() {
        let index = SearchIndex::build();
        assert!(index.query("", false).is_empty());
        assert!(index.query("   ", false).is_empty());
    }

    #[test]
    fn title_hit_ranks_above_body_hit() {
        // "height" appears in the "Height Brush" title and in several bodies.
        // The title match must come first.
        let index = SearchIndex::build();
        let hits = index.query("height", false);
        assert!(!hits.is_empty(), "expected matches for 'height'");
        assert_eq!(
            hits[0].id, "terrain-height-brush",
            "title match should outrank body-only matches"
        );
    }

    #[test]
    fn query_matches_known_topic() {
        let index = SearchIndex::build();
        let hits = index.query("generator", false);
        assert!(hits.iter().any(|h| h.id == "generator"));
    }

    #[test]
    fn web_excludes_native_only_topics() {
        let index = SearchIndex::build();
        // "test-map" is native-only; searching its title must not surface it on web.
        let native = index.query("testing your map", false);
        assert!(native.iter().any(|h| h.id == "test-map"));
        let web = index.query("testing your map", true);
        assert!(!web.iter().any(|h| h.id == "test-map"));
    }

    #[test]
    fn strip_markdown_drops_syntax_and_keeps_link_text() {
        let s = strip_markdown("# Title\n\nSee [the brush](terrain.md) for **more**.");
        assert!(!s.contains('#'));
        assert!(!s.contains('*'));
        assert!(
            !s.contains("terrain.md"),
            "the link target should not pollute the search haystack"
        );
        assert!(s.contains("the brush"));
        assert!(s.contains("Title"));
    }

    #[test]
    fn snippet_never_panics_on_unicode() {
        // Exercise boundary snapping with a multi-byte character near the cut.
        let plain = "café ".repeat(40);
        let _ = snippet(&plain, 75, true);
    }
}
