//! Undo/redo commands for object placement, deletion, and movement.

use wz_maplib::WzMap;
use wz_maplib::objects::{Droid, Feature, Structure, WorldPos};

use crate::map::history::EditCommand;

/// Which type of object list to operate on.
#[derive(Debug, Clone, Copy)]
pub enum ObjectKind {
    Structure,
    Droid,
    Feature,
}

/// Command: place a new structure on the map.
#[derive(Debug)]
pub struct PlaceStructureCommand {
    pub structure: Structure,
}

impl EditCommand for PlaceStructureCommand {
    fn execute(&self, map: &mut WzMap) {
        map.structures.push(self.structure.clone());
    }

    fn undo(&self, map: &mut WzMap) {
        map.structures.pop();
    }
}

/// Command: place a new droid on the map.
#[derive(Debug)]
pub struct PlaceDroidCommand {
    pub droid: Droid,
}

impl EditCommand for PlaceDroidCommand {
    fn execute(&self, map: &mut WzMap) {
        map.droids.push(self.droid.clone());
    }

    fn undo(&self, map: &mut WzMap) {
        map.droids.pop();
    }
}

/// Command: place a new feature on the map.
#[derive(Debug)]
pub struct PlaceFeatureCommand {
    pub feature: Feature,
}

impl EditCommand for PlaceFeatureCommand {
    fn execute(&self, map: &mut WzMap) {
        map.features.push(self.feature.clone());
    }

    fn undo(&self, map: &mut WzMap) {
        map.features.pop();
    }
}

/// Command: delete an object by kind and index.
#[derive(Debug)]
pub struct DeleteObjectCommand {
    pub kind: ObjectKind,
    pub index: usize,
    /// Stored copy for undo.
    saved_structure: Option<Structure>,
    saved_droid: Option<Droid>,
    saved_feature: Option<Feature>,
}

impl DeleteObjectCommand {
    pub fn structure(index: usize, obj: Structure) -> Self {
        Self {
            kind: ObjectKind::Structure,
            index,
            saved_structure: Some(obj),
            saved_droid: None,
            saved_feature: None,
        }
    }

    pub fn droid(index: usize, obj: Droid) -> Self {
        Self {
            kind: ObjectKind::Droid,
            index,
            saved_structure: None,
            saved_droid: Some(obj),
            saved_feature: None,
        }
    }

    pub fn feature(index: usize, obj: Feature) -> Self {
        Self {
            kind: ObjectKind::Feature,
            index,
            saved_structure: None,
            saved_droid: None,
            saved_feature: Some(obj),
        }
    }
}

impl EditCommand for DeleteObjectCommand {
    fn execute(&self, map: &mut WzMap) {
        match self.kind {
            ObjectKind::Structure => {
                if self.index < map.structures.len() {
                    map.structures.remove(self.index);
                }
            }
            ObjectKind::Droid => {
                if self.index < map.droids.len() {
                    map.droids.remove(self.index);
                }
            }
            ObjectKind::Feature => {
                if self.index < map.features.len() {
                    map.features.remove(self.index);
                }
            }
        }
    }

    fn undo(&self, map: &mut WzMap) {
        match self.kind {
            ObjectKind::Structure => {
                if let Some(ref obj) = self.saved_structure {
                    let idx = self.index.min(map.structures.len());
                    map.structures.insert(idx, obj.clone());
                }
            }
            ObjectKind::Droid => {
                if let Some(ref obj) = self.saved_droid {
                    let idx = self.index.min(map.droids.len());
                    map.droids.insert(idx, obj.clone());
                }
            }
            ObjectKind::Feature => {
                if let Some(ref obj) = self.saved_feature {
                    let idx = self.index.min(map.features.len());
                    map.features.insert(idx, obj.clone());
                }
            }
        }
    }
}

/// Command: move an object to a new position.
#[derive(Debug)]
pub struct MoveObjectCommand {
    pub kind: ObjectKind,
    pub index: usize,
    pub old_pos: WorldPos,
    pub new_pos: WorldPos,
}

impl EditCommand for MoveObjectCommand {
    fn execute(&self, map: &mut WzMap) {
        set_object_pos(map, self.kind, self.index, self.new_pos);
    }

    fn undo(&self, map: &mut WzMap) {
        set_object_pos(map, self.kind, self.index, self.old_pos);
    }
}

/// Command: rotate an object to a new direction.
#[derive(Debug)]
pub struct RotateObjectCommand {
    pub kind: ObjectKind,
    pub index: usize,
    pub old_direction: u16,
    pub new_direction: u16,
}

impl EditCommand for RotateObjectCommand {
    fn execute(&self, map: &mut WzMap) {
        set_object_direction(map, self.kind, self.index, self.new_direction);
    }

    fn undo(&self, map: &mut WzMap) {
        set_object_direction(map, self.kind, self.index, self.old_direction);
    }
}

fn set_object_pos(map: &mut WzMap, kind: ObjectKind, index: usize, pos: WorldPos) {
    match kind {
        ObjectKind::Structure => {
            if let Some(s) = map.structures.get_mut(index) {
                s.position = pos;
            }
        }
        ObjectKind::Droid => {
            if let Some(d) = map.droids.get_mut(index) {
                d.position = pos;
            }
        }
        ObjectKind::Feature => {
            if let Some(f) = map.features.get_mut(index) {
                f.position = pos;
            }
        }
    }
}

fn set_object_direction(map: &mut WzMap, kind: ObjectKind, index: usize, direction: u16) {
    match kind {
        ObjectKind::Structure => {
            if let Some(s) = map.structures.get_mut(index) {
                s.direction = direction;
            }
        }
        ObjectKind::Droid => {
            if let Some(d) = map.droids.get_mut(index) {
                d.direction = direction;
            }
        }
        ObjectKind::Feature => {
            if let Some(f) = map.features.get_mut(index) {
                f.direction = direction;
            }
        }
    }
}
