//! Filesystem search for PIE files and texture pages.
//!
//! PIE files live across many subdirectories under `base/` and `mp/`.
//! The sync loader walks a known-dirs list and falls back to a recursive
//! search; the background loader prebuilds a case-insensitive index for
//! O(1) lookups per parse.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Subdirectories under `data_dir` searched by the synchronous PIE loader.
pub(crate) const PIE_SEARCH_DIRS: &[&str] = &[
    "base/components/prop",
    "base/components/bodies",
    "base/components/weapons",
    "base/components/turrets",
    "base/structs",
    "base/features",
    "base/misc",
    "base/effects",
    "base/components",
    "mp/components/bodies",
    "mp/components/weapons",
    "mp/components/turrets",
    "mp/structs",
    "mp/effects",
    "mp/components",
];

/// Find a PIE file by name under `data_dir`. Walks the known-dirs list
/// first, then falls back to a depth-limited recursive search rooted at
/// `base/` and `mp/`.
pub(crate) fn find_pie_file(data_dir: &Path, imd_name: &str) -> Option<PathBuf> {
    for dir in PIE_SEARCH_DIRS {
        let path = data_dir.join(dir).join(imd_name);
        if path.exists() {
            return Some(path);
        }
    }

    for subdir in ["base", "mp"] {
        let search_root = data_dir.join(subdir);
        if let Some(found) = find_file_recursive(&search_root, imd_name) {
            return Some(found);
        }
    }

    None
}

/// Recursive file search rooted at `dir`. First match wins; depth is
/// capped to avoid scanning the entire data tree.
fn find_file_recursive(dir: &Path, filename: &str) -> Option<PathBuf> {
    find_file_recursive_depth(dir, filename, 5)
}

fn find_file_recursive_depth(dir: &Path, filename: &str, max_depth: u32) -> Option<PathBuf> {
    if max_depth == 0 {
        return None;
    }

    let Ok(entries) = std::fs::read_dir(dir) else {
        return None;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_file()
            && path
                .file_name()
                .is_some_and(|n| n.to_string_lossy().eq_ignore_ascii_case(filename))
        {
            return Some(path);
        }
        if path.is_dir()
            && let Some(found) = find_file_recursive_depth(&path, filename, max_depth - 1)
        {
            return Some(found);
        }
    }

    None
}

/// Case-insensitive filename to path index for all `.pie`, `.png`, and
/// `.ktx2` files under `data_dir/base/` and `data_dir/mp/`. Scanning
/// once avoids per-model recursive searches, which otherwise dominate
/// load time on large maps.
pub(crate) fn build_pie_file_index(data_dir: &Path) -> HashMap<String, PathBuf> {
    let mut index = HashMap::new();
    let base = data_dir.join("base");
    if base.exists() {
        index_directory_recursive(&base, &mut index, 6);
    }
    let mp = data_dir.join("mp");
    if mp.exists() {
        index_directory_recursive(&mp, &mut index, 6);
    }
    let tcmask_count = index.keys().filter(|k| k.contains("tcmask")).count();
    log::info!(
        "File index: {} entries ({} tcmask files)",
        index.len(),
        tcmask_count
    );
    index
}

fn index_directory_recursive(dir: &Path, index: &mut HashMap<String, PathBuf>, depth: u32) {
    if depth == 0 {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            index_directory_recursive(&path, index, depth - 1);
        } else if let Some(name) = path.file_name() {
            let name_str = name.to_string_lossy();
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
            if ext.eq_ignore_ascii_case("pie")
                || ext.eq_ignore_ascii_case("png")
                || ext.eq_ignore_ascii_case("ktx2")
            {
                index
                    .entry(name_str.to_string())
                    .or_insert_with(|| path.clone());
                let lower = name_str.to_lowercase();
                if lower != name_str.as_ref() {
                    index.entry(lower).or_insert(path);
                }
            }
        }
    }
}

/// Look up a PIE file in the prebuilt index. Tries exact case first,
/// then lowercase.
pub(crate) fn lookup_in_index<'a>(
    file_index: &'a HashMap<String, PathBuf>,
    imd_name: &str,
) -> Option<&'a PathBuf> {
    file_index
        .get(imd_name)
        .or_else(|| file_index.get(&imd_name.to_lowercase()))
}
