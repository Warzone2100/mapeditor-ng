//! Cache state for `ModelLoader`: stat-name to IMD mapping, parsed PIE
//! cache, uploaded set, not-found set, connector positions, an interning
//! pool, and the active tileset index.
//!
//! Invariant: an IMD name is never simultaneously in any two of
//! `parsed_cache`, `uploaded`, and `not_found_cache`. `insert_parsed`
//! rejects names already uploaded, `promote_to_uploaded` atomically
//! moves a parsed model into the uploaded set, and `mark_not_found`
//! evicts stale entries first.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use wz_stats::StatsDatabase;

use super::pie_loader::ParsedModel;
use super::stats_mapper::build_imd_map;

pub(super) struct LoaderCache {
    imd_map: HashMap<String, String>,
    uploaded: HashSet<String>,
    parsed_cache: HashMap<String, ParsedModel>,
    not_found_cache: HashSet<String>,
    connector_cache: HashMap<String, Vec<glam::Vec3>>,
    interned_names: rustc_hash::FxHashMap<Box<str>, Arc<str>>,
    tileset_index: usize,
}

impl LoaderCache {
    pub(super) fn new(stats: &StatsDatabase) -> Self {
        Self {
            imd_map: build_imd_map(stats),
            uploaded: HashSet::new(),
            parsed_cache: HashMap::new(),
            not_found_cache: HashSet::new(),
            connector_cache: HashMap::new(),
            interned_names: rustc_hash::FxHashMap::default(),
            tileset_index: 0,
        }
    }

    pub(super) fn imd_for_object(&self, name: &str) -> Option<&str> {
        self.imd_map.get(name).map(String::as_str)
    }

    pub(super) fn tileset_index(&self) -> usize {
        self.tileset_index
    }

    /// Switch the active tileset index. Clears `uploaded` since textures
    /// depend on tileset; PIE geometry and connectors are tileset-agnostic
    /// so `parsed_cache` and `connector_cache` stay. Returns true if the
    /// index changed.
    pub(super) fn set_tileset(&mut self, index: usize) -> bool {
        if self.tileset_index == index {
            return false;
        }
        self.tileset_index = index;
        self.uploaded.clear();
        true
    }

    pub(super) fn is_uploaded(&self, name: &str) -> bool {
        self.uploaded.contains(name)
    }

    /// Mark an IMD as uploaded without going through `parsed_cache`.
    ///
    /// Used by the background path (parsed model arrived via channel and
    /// never lived in `parsed_cache`) and the failure path (mark a missing
    /// or broken model "uploaded" to short-circuit retry loops). Any
    /// stale `parsed_cache` entry is evicted to preserve the invariant.
    pub(super) fn mark_uploaded(&mut self, name: &str) {
        self.parsed_cache.remove(name);
        self.uploaded.insert(name.to_string());
    }

    pub(super) fn has_parsed(&self, name: &str) -> bool {
        self.parsed_cache.contains_key(name)
    }

    pub(super) fn get_parsed(&self, name: &str) -> Option<&ParsedModel> {
        self.parsed_cache.get(name)
    }

    /// Insert a parsed PIE. Rejected if the model is already uploaded
    /// (would break single-residency). Returns true on store.
    pub(super) fn insert_parsed(&mut self, name: &str, parsed: ParsedModel) -> bool {
        if self.uploaded.contains(name) {
            return false;
        }
        self.parsed_cache.insert(name.to_string(), parsed);
        true
    }

    /// Atomically remove the parsed model and mark the name uploaded.
    /// Returns `None` when no parsed entry exists; treat that as "nothing
    /// to upload" rather than an error.
    pub(super) fn promote_to_uploaded(&mut self, name: &str) -> Option<ParsedModel> {
        let parsed = self.parsed_cache.remove(name)?;
        self.uploaded.insert(name.to_string());
        Some(parsed)
    }

    pub(super) fn is_not_found(&self, name: &str) -> bool {
        self.not_found_cache.contains(name)
    }

    /// Mark an IMD as missing so future lookups short-circuit. Stale
    /// parsed/uploaded entries are evicted so the negative cache cannot
    /// coexist with a positive one.
    pub(super) fn mark_not_found(&mut self, name: &str) {
        self.parsed_cache.remove(name);
        self.uploaded.remove(name);
        self.not_found_cache.insert(name.to_string());
    }

    pub(super) fn get_connectors(&self, name: &str) -> Option<&[glam::Vec3]> {
        self.connector_cache.get(name).map(Vec::as_slice)
    }

    pub(super) fn has_connectors(&self, name: &str) -> bool {
        self.connector_cache.contains_key(name)
    }

    pub(super) fn insert_connectors(&mut self, name: String, connectors: Vec<glam::Vec3>) {
        self.connector_cache.insert(name, connectors);
    }

    /// Insert connectors only when absent. Used when merging
    /// background-precache results so foreground results (potentially more
    /// up to date) are not overwritten.
    pub(super) fn insert_connectors_if_absent(
        &mut self,
        name: String,
        connectors: Vec<glam::Vec3>,
    ) {
        self.connector_cache.entry(name).or_insert(connectors);
    }

    /// Owned snapshot of connector cache keys, for handing to a background
    /// thread that needs to filter against it.
    pub(super) fn connector_key_snapshot(&self) -> HashSet<String> {
        self.connector_cache.keys().cloned().collect()
    }

    pub(super) fn intern(&mut self, name: &str) -> Arc<str> {
        if let Some(existing) = self.interned_names.get(name) {
            return Arc::clone(existing);
        }
        let arc: Arc<str> = Arc::from(name);
        self.interned_names
            .insert(Box::from(name), Arc::clone(&arc));
        arc
    }

    pub(super) fn imd_map_len(&self) -> usize {
        self.imd_map.len()
    }

    pub(super) fn uploaded_len(&self) -> usize {
        self.uploaded.len()
    }

    pub(super) fn parsed_cache_len(&self) -> usize {
        self.parsed_cache.len()
    }

    pub(super) fn not_found_cache_len(&self) -> usize {
        self.not_found_cache.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wz_pie::PieModel;

    fn make_parsed() -> ParsedModel {
        ParsedModel {
            pie: PieModel {
                version: 3,
                model_type: 0x200,
                texture_page: String::new(),
                texture_width: 256,
                texture_height: 256,
                texture_pages: Vec::new(),
                tcmask_pages: Vec::new(),
                normal_page: None,
                specular_page: None,
                event_page: None,
                levels: Vec::new(),
            },
            texture_data: None,
            tcmask_data: None,
            normal_data: None,
            specular_data: None,
        }
    }

    #[test]
    fn promote_evicts_parsed_and_marks_uploaded() {
        let stats = StatsDatabase::default();
        let mut cache = LoaderCache::new(&stats);

        assert!(cache.insert_parsed("foo.pie", make_parsed()));
        assert!(cache.has_parsed("foo.pie"));
        assert!(!cache.is_uploaded("foo.pie"));

        let promoted = cache.promote_to_uploaded("foo.pie");
        assert!(promoted.is_some(), "promote should return the parsed model");
        assert!(
            !cache.has_parsed("foo.pie"),
            "parsed lookup must return None after promotion",
        );
        assert!(cache.is_uploaded("foo.pie"));
    }

    #[test]
    fn insert_parsed_rejected_when_already_uploaded() {
        let stats = StatsDatabase::default();
        let mut cache = LoaderCache::new(&stats);

        cache.mark_uploaded("foo.pie");
        let accepted = cache.insert_parsed("foo.pie", make_parsed());
        assert!(!accepted, "must not store parsed for an uploaded name");
        assert!(!cache.has_parsed("foo.pie"));
    }

    #[test]
    fn mark_uploaded_evicts_stale_parsed() {
        let stats = StatsDatabase::default();
        let mut cache = LoaderCache::new(&stats);

        assert!(cache.insert_parsed("foo.pie", make_parsed()));
        cache.mark_uploaded("foo.pie");
        assert!(!cache.has_parsed("foo.pie"));
        assert!(cache.is_uploaded("foo.pie"));
    }

    #[test]
    fn promote_missing_returns_none() {
        let stats = StatsDatabase::default();
        let mut cache = LoaderCache::new(&stats);
        assert!(cache.promote_to_uploaded("missing.pie").is_none());
        assert!(!cache.is_uploaded("missing.pie"));
    }

    #[test]
    fn set_tileset_clears_uploaded_only() {
        let stats = StatsDatabase::default();
        let mut cache = LoaderCache::new(&stats);

        cache.mark_uploaded("foo.pie");
        cache.insert_connectors("foo.pie".to_string(), vec![glam::Vec3::ZERO]);
        assert!(cache.is_uploaded("foo.pie"));
        assert!(cache.has_connectors("foo.pie"));

        let changed = cache.set_tileset(1);
        assert!(changed);
        assert!(!cache.is_uploaded("foo.pie"));
        assert!(
            cache.has_connectors("foo.pie"),
            "connectors must survive a tileset switch",
        );
    }

    #[test]
    fn mark_not_found_evicts_parsed_and_uploaded() {
        let stats = StatsDatabase::default();
        let mut cache = LoaderCache::new(&stats);

        assert!(cache.insert_parsed("foo.pie", make_parsed()));
        cache.mark_not_found("foo.pie");
        assert!(!cache.has_parsed("foo.pie"));
        assert!(!cache.is_uploaded("foo.pie"));
        assert!(cache.is_not_found("foo.pie"));

        cache.mark_uploaded("bar.pie");
        cache.mark_not_found("bar.pie");
        assert!(!cache.is_uploaded("bar.pie"));
        assert!(!cache.has_parsed("bar.pie"));
        assert!(cache.is_not_found("bar.pie"));
    }

    #[test]
    fn intern_returns_same_arc_for_repeated_name() {
        let stats = StatsDatabase::default();
        let mut cache = LoaderCache::new(&stats);

        let a = cache.intern("body1.pie");
        let b = cache.intern("body1.pie");
        assert!(Arc::ptr_eq(&a, &b));

        let c = cache.intern("body2.pie");
        assert!(!Arc::ptr_eq(&a, &c));
    }
}
