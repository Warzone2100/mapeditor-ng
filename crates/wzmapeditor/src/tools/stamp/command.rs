//! Reversible commands for stamp and scatter applications.

use wz_maplib::WzMap;
use wz_maplib::objects::{Droid, Feature, Structure, WorldPos};

use super::pattern::StampObject;
use super::placement::{PushedObject, push_object};
use crate::map::history::EditCommand;

/// Reversible command for a single stamp or scatter application.
#[derive(Debug)]
pub struct StampCommand {
    /// Tile changes: (`x`, `y`, `old_texture`, `old_height`, `new_texture`, `new_height`).
    pub tile_changes: Vec<(u32, u32, u16, u16, u16, u16)>,
    /// Structures added by this stamp (for redo).
    pub structures: Vec<Structure>,
    /// Droids added by this stamp (for redo).
    pub droids: Vec<Droid>,
    /// Features added by this stamp (for redo).
    pub features: Vec<Feature>,
    /// Indices of added structures at the time of application (for undo removal).
    pub(super) structure_indices: Vec<usize>,
    /// Indices of added droids at the time of application (for undo removal).
    pub(super) droid_indices: Vec<usize>,
    /// Indices of added features at the time of application (for undo removal).
    pub(super) feature_indices: Vec<usize>,
}

impl EditCommand for StampCommand {
    fn execute(&self, map: &mut WzMap) {
        for &(x, y, _, _, new_tex, new_h) in &self.tile_changes {
            if let Some(tile) = map.map_data.tile_mut(x, y) {
                tile.texture = new_tex;
                tile.height = new_h;
            }
        }
        for s in &self.structures {
            map.structures.push(s.clone());
        }
        for d in &self.droids {
            map.droids.push(d.clone());
        }
        for f in &self.features {
            map.features.push(f.clone());
        }
    }

    fn undo(&self, map: &mut WzMap) {
        for &(x, y, old_tex, old_h, _, _) in &self.tile_changes {
            if let Some(tile) = map.map_data.tile_mut(x, y) {
                tile.texture = old_tex;
                tile.height = old_h;
            }
        }
        // Reverse order keeps earlier indices valid as we remove later ones.
        for &i in self.feature_indices.iter().rev() {
            if i < map.features.len() {
                map.features.remove(i);
            }
        }
        for &i in self.droid_indices.iter().rev() {
            if i < map.droids.len() {
                map.droids.remove(i);
            }
        }
        for &i in self.structure_indices.iter().rev() {
            if i < map.structures.len() {
                map.structures.remove(i);
            }
        }
    }
}

/// Accumulates objects placed during a stamp or scatter into a `StampCommand`.
#[derive(Default)]
pub(super) struct ObjectAccum {
    structures: Vec<Structure>,
    droids: Vec<Droid>,
    features: Vec<Feature>,
    structure_indices: Vec<usize>,
    droid_indices: Vec<usize>,
    feature_indices: Vec<usize>,
}

impl ObjectAccum {
    pub(super) fn place(
        &mut self,
        map: &mut WzMap,
        template: &StampObject,
        position: WorldPos,
        direction: u16,
    ) {
        match push_object(map, template, position, direction) {
            PushedObject::Structure(idx, s) => {
                self.structure_indices.push(idx);
                self.structures.push(s);
            }
            PushedObject::Droid(idx, d) => {
                self.droid_indices.push(idx);
                self.droids.push(d);
            }
            PushedObject::Feature(idx, f) => {
                self.feature_indices.push(idx);
                self.features.push(f);
            }
        }
    }

    pub(super) fn into_command(
        self,
        tile_changes: Vec<(u32, u32, u16, u16, u16, u16)>,
    ) -> StampCommand {
        StampCommand {
            tile_changes,
            structures: self.structures,
            droids: self.droids,
            features: self.features,
            structure_indices: self.structure_indices,
            droid_indices: self.droid_indices,
            feature_indices: self.feature_indices,
        }
    }
}
