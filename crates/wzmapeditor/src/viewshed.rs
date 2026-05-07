//! Range-ring overlay for tower placement.
//!
//! For each selected weaponized structure, produce a ring at its weapon's
//! `long_range`. The renderer draws the rings; nothing is rasterized per tile.

use glam::Vec3;
use wz_maplib::map_data::MapData;
use wz_maplib::objects::Structure;
use wz_stats::StatsDatabase;

use crate::viewport::picking::sample_terrain_height_pub;

/// World-unit offset of the ring above the structure's ground tile, so the
/// outline floats at roughly tower-top height instead of clipping into the model.
pub const EYE_OFFSET: f32 = 32.0;

/// One source structure's ring: position and radius.
#[derive(Debug, Clone, Copy)]
pub struct ViewshedRing {
    pub center: Vec3,
    pub max_range: f32,
}

/// Output of one compute pass.
#[derive(Debug, Clone, Default)]
pub struct ViewshedFrame {
    pub rings: Vec<ViewshedRing>,
}

/// Stable hash of a source list, used to detect selection changes without
/// rebuilding ring vertices every frame.
pub fn selection_sig(sources: &[usize]) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = rustc_hash::FxHasher::default();
    sources.len().hash(&mut h);
    for s in sources {
        s.hash(&mut h);
    }
    h.finish()
}

/// Pull the weaponized structure indices out of the current selection.
///
/// Non-structure selections (droids, features, labels, gateways) are
/// silently ignored. Selected structures with no weapon stat are also
/// dropped so the compute pass does no useless work.
pub fn collect_sources(
    structures: &[Structure],
    selection: &crate::app::Selection,
    stats: &StatsDatabase,
) -> Vec<usize> {
    selection
        .objects
        .iter()
        .filter_map(|obj| match obj {
            crate::app::SelectedObject::Structure(i) => Some(*i),
            _ => None,
        })
        .filter(|i| {
            structures
                .get(*i)
                .and_then(|s| stats.weapon_for_structure(&s.name))
                .is_some()
        })
        .collect()
}

/// Build a [`ViewshedFrame`] for the given source structures.
///
/// Structures without a weapon, or whose weapon has no `long_range`, are
/// silently skipped. The frame's `rings` is empty when nothing matched.
pub fn compute_viewshed(
    map: &MapData,
    structures: &[Structure],
    sources: &[usize],
    stats: &StatsDatabase,
) -> ViewshedFrame {
    let mut rings = Vec::with_capacity(sources.len());
    for &idx in sources {
        let Some(structure) = structures.get(idx) else {
            continue;
        };
        let Some(weapon) = stats.weapon_for_structure(&structure.name) else {
            continue;
        };
        let Some(max_range) = weapon.long_range.filter(|r| *r > 0) else {
            continue;
        };
        let sx = structure.position.x as f32;
        let sz = structure.position.y as f32;
        let ground = sample_terrain_height_pub(map, sx, sz);
        rings.push(ViewshedRing {
            center: Vec3::new(sx, ground + EYE_OFFSET, sz),
            max_range: max_range as f32,
        });
    }
    ViewshedFrame { rings }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_selection_produces_no_rings() {
        let map = MapData::new(8, 8);
        let stats = StatsDatabase::default();
        let frame = compute_viewshed(&map, &[], &[], &stats);
        assert!(frame.rings.is_empty());
    }

    #[test]
    fn selection_sig_changes_with_membership() {
        assert_ne!(selection_sig(&[1, 2]), selection_sig(&[1, 2, 3]));
        assert_ne!(selection_sig(&[1, 2]), selection_sig(&[2, 1]));
        assert_eq!(selection_sig(&[]), selection_sig(&[]));
    }
}
