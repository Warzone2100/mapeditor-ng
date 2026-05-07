//! Map-level resize as an undoable edit command.

use std::sync::OnceLock;

use wz_maplib::{ResizeReport, WzMap};

use crate::map::history::EditCommand;

/// Reversible whole-map resize.
///
/// Stores the pre-resize map for undo, and lazy-caches the post-resize map
/// on first execute so redo is a single clone.
pub struct ResizeMapCommand {
    before: WzMap,
    new_width: u32,
    new_height: u32,
    offset_x: i32,
    offset_y: i32,
    after: OnceLock<WzMap>,
}

impl std::fmt::Debug for ResizeMapCommand {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ResizeMapCommand")
            .field("new_width", &self.new_width)
            .field("new_height", &self.new_height)
            .field("offset_x", &self.offset_x)
            .field("offset_y", &self.offset_y)
            .finish_non_exhaustive()
    }
}

impl ResizeMapCommand {
    /// Apply the resize and return the command for
    /// `EditHistory::push_already_applied`, plus a report of what was dropped.
    pub fn apply(
        map: &mut WzMap,
        new_width: u32,
        new_height: u32,
        offset_x: i32,
        offset_y: i32,
    ) -> (Box<Self>, ResizeReport) {
        let before = map.clone();
        let (after, report) = before.resized(new_width, new_height, offset_x, offset_y);
        *map = after.clone();
        let after_cell = OnceLock::new();
        let _ = after_cell.set(after);
        let cmd = Box::new(Self {
            before,
            new_width,
            new_height,
            offset_x,
            offset_y,
            after: after_cell,
        });
        (cmd, report)
    }
}

impl EditCommand for ResizeMapCommand {
    fn execute(&self, map: &mut WzMap) {
        let after = self.after.get_or_init(|| {
            self.before
                .resized(
                    self.new_width,
                    self.new_height,
                    self.offset_x,
                    self.offset_y,
                )
                .0
        });
        *map = after.clone();
    }

    fn undo(&self, map: &mut WzMap) {
        *map = self.before.clone();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wz_maplib::constants::world_coord;
    use wz_maplib::objects::WorldPos;

    fn map_with_one_struct() -> WzMap {
        let mut m = WzMap::new("test", 16, 16);
        // Tile (3, 3) survives a shrink to 8x8.
        m.structures.push(wz_maplib::Structure {
            name: "A0PowMod1".to_string(),
            position: WorldPos {
                x: world_coord(3) as u32,
                y: world_coord(3) as u32,
            },
            direction: 0,
            player: 0,
            modules: 0,
            id: None,
        });
        m
    }

    #[test]
    fn apply_then_undo_roundtrips() {
        let mut map = map_with_one_struct();
        let original = map.clone();
        let (cmd, _report) = ResizeMapCommand::apply(&mut map, 8, 8, 0, 0);
        assert_eq!(map.map_data.width, 8);
        assert_eq!(map.structures.len(), 1);

        cmd.undo(&mut map);
        assert_eq!(map.map_data.width, 16);
        assert_eq!(map.structures.len(), original.structures.len());
    }

    #[test]
    fn redo_uses_cached_after_snapshot() {
        let mut map = map_with_one_struct();
        let (cmd, _) = ResizeMapCommand::apply(&mut map, 8, 8, 0, 0);
        let after_first = map.clone();

        cmd.undo(&mut map);
        cmd.execute(&mut map);

        assert_eq!(map.map_data.width, after_first.map_data.width);
        assert_eq!(map.structures.len(), after_first.structures.len());
    }
}
