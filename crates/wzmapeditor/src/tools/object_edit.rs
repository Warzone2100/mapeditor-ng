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

/// A saved copy of a single object, holding the before/after state of a
/// [`ReplaceObjectCommand`].
#[derive(Debug, Clone)]
enum ObjectValue {
    Structure(Structure),
    Droid(Droid),
    Feature(Feature),
}

/// Command: overwrite an object with a saved copy of itself.
///
/// The property panel edits position, rotation, player, and module count with
/// separate widgets, so a single gesture can touch more than one field. Saving
/// the whole object collapses that into one undo step without needing a command
/// per field.
#[derive(Debug)]
pub struct ReplaceObjectCommand {
    index: usize,
    before: ObjectValue,
    after: ObjectValue,
}

impl ReplaceObjectCommand {
    pub fn structure(index: usize, before: Structure, after: Structure) -> Self {
        Self {
            index,
            before: ObjectValue::Structure(before),
            after: ObjectValue::Structure(after),
        }
    }

    pub fn droid(index: usize, before: Droid, after: Droid) -> Self {
        Self {
            index,
            before: ObjectValue::Droid(before),
            after: ObjectValue::Droid(after),
        }
    }

    pub fn feature(index: usize, before: Feature, after: Feature) -> Self {
        Self {
            index,
            before: ObjectValue::Feature(before),
            after: ObjectValue::Feature(after),
        }
    }
}

impl EditCommand for ReplaceObjectCommand {
    fn execute(&self, map: &mut WzMap) {
        write_object(map, self.index, &self.after);
    }

    fn undo(&self, map: &mut WzMap) {
        write_object(map, self.index, &self.before);
    }
}

fn write_object(map: &mut WzMap, index: usize, value: &ObjectValue) {
    match value {
        ObjectValue::Structure(v) => {
            if let Some(s) = map.structures.get_mut(index) {
                *s = v.clone();
            }
        }
        ObjectValue::Droid(v) => {
            if let Some(d) = map.droids.get_mut(index) {
                *d = v.clone();
            }
        }
        ObjectValue::Feature(v) => {
            if let Some(f) = map.features.get_mut(index) {
                *f = v.clone();
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

#[cfg(test)]
mod tests {
    use super::*;

    fn structure() -> Structure {
        Structure {
            name: "A0CommandCentre".into(),
            position: WorldPos { x: 256, y: 256 },
            direction: 0,
            player: 0,
            modules: 0,
            id: Some(1),
        }
    }

    #[test]
    fn replace_object_restores_every_edited_field() {
        let mut map = WzMap::new("test", 8, 8);
        let before = structure();
        map.structures.push(before.clone());

        let mut after = before.clone();
        after.position = WorldPos { x: 512, y: 768 };
        after.direction = 16384;
        after.player = 3;
        after.modules = 2;

        let cmd = ReplaceObjectCommand::structure(0, before, after);
        cmd.execute(&mut map);

        let edited = &map.structures[0];
        assert_eq!(edited.position.x, 512);
        assert_eq!(edited.direction, 16384);
        assert_eq!(edited.player, 3);
        assert_eq!(edited.modules, 2);

        cmd.undo(&mut map);

        let restored = &map.structures[0];
        assert_eq!(restored.position.x, 256);
        assert_eq!(restored.direction, 0);
        assert_eq!(restored.player, 0);
        assert_eq!(restored.modules, 0);
    }

    fn droid() -> Droid {
        Droid {
            name: "ConstructionDroid".into(),
            position: WorldPos { x: 256, y: 256 },
            direction: 0,
            player: 0,
            id: Some(2),
        }
    }

    fn feature() -> Feature {
        Feature {
            name: "OilResource".into(),
            position: WorldPos { x: 256, y: 256 },
            direction: 0,
            id: Some(3),
            player: Some(0),
        }
    }

    /// Undo/redo only refresh instance buffers when the replayed command says
    /// so, so any command that adds, removes, moves, rotates or re-owns an
    /// object has to report `true` or the viewport keeps drawing the state the
    /// user just undid.
    #[test]
    fn every_object_command_dirties_object_buffers() {
        let commands: Vec<(&str, Box<dyn EditCommand>)> = vec![
            (
                "PlaceStructureCommand",
                Box::new(PlaceStructureCommand {
                    structure: structure(),
                }),
            ),
            (
                "PlaceDroidCommand",
                Box::new(PlaceDroidCommand { droid: droid() }),
            ),
            (
                "PlaceFeatureCommand",
                Box::new(PlaceFeatureCommand { feature: feature() }),
            ),
            (
                "DeleteObjectCommand",
                Box::new(DeleteObjectCommand::structure(0, structure())),
            ),
            (
                "MoveObjectCommand",
                Box::new(MoveObjectCommand {
                    kind: ObjectKind::Structure,
                    index: 0,
                    old_pos: WorldPos { x: 256, y: 256 },
                    new_pos: WorldPos { x: 512, y: 512 },
                }),
            ),
            (
                "RotateObjectCommand",
                Box::new(RotateObjectCommand {
                    kind: ObjectKind::Structure,
                    index: 0,
                    old_direction: 0,
                    new_direction: 16384,
                }),
            ),
            (
                "ReplaceObjectCommand",
                Box::new(ReplaceObjectCommand::structure(0, structure(), structure())),
            ),
        ];

        for (name, cmd) in commands {
            assert!(
                cmd.dirties_objects(),
                "{name} changes object geometry, so undo must rebuild instance buffers"
            );
        }
    }

    #[test]
    fn replace_object_ignores_a_stale_index() {
        let mut map = WzMap::new("test", 8, 8);
        let cmd = ReplaceObjectCommand::structure(7, structure(), structure());
        cmd.execute(&mut map);
        cmd.undo(&mut map);
        assert!(map.structures.is_empty());
    }
}
