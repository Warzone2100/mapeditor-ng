//! Script label placement and editing commands.

use wz_maplib::constants::TILE_UNITS;
use wz_maplib::io_wz::WzMap;
use wz_maplib::labels::ScriptLabel;
use wz_maplib::objects::WorldPos;

use crate::map::history::EditCommand;
use crate::tools::trait_def::{Tool, ToolCtx};

pub struct AddLabelCommand {
    pub key: String,
    pub label: ScriptLabel,
}

impl EditCommand for AddLabelCommand {
    fn execute(&self, map: &mut WzMap) {
        map.labels.push((self.key.clone(), self.label.clone()));
    }

    fn undo(&self, map: &mut WzMap) {
        if let Some(idx) = map.labels.iter().position(|(k, _)| k == &self.key) {
            map.labels.remove(idx);
        }
    }
}

pub struct DeleteLabelCommand {
    pub index: usize,
    pub saved_key: String,
    pub saved_label: ScriptLabel,
}

impl EditCommand for DeleteLabelCommand {
    fn execute(&self, map: &mut WzMap) {
        if self.index < map.labels.len() {
            map.labels.remove(self.index);
        }
    }

    fn undo(&self, map: &mut WzMap) {
        let idx = self.index.min(map.labels.len());
        map.labels
            .insert(idx, (self.saved_key.clone(), self.saved_label.clone()));
    }
}

pub struct MoveLabelCommand {
    pub index: usize,
    pub old_label: ScriptLabel,
    pub new_label: ScriptLabel,
}

impl EditCommand for MoveLabelCommand {
    fn execute(&self, map: &mut WzMap) {
        if let Some((_, label)) = map.labels.get_mut(self.index) {
            *label = self.new_label.clone();
        }
    }

    fn undo(&self, map: &mut WzMap) {
        if let Some((_, label)) = map.labels.get_mut(self.index) {
            *label = self.old_label.clone();
        }
    }
}

/// Generate the next available key for a label type.
pub fn next_label_key(labels: &[(String, ScriptLabel)], prefix: &str) -> String {
    let mut max_idx = -1_i32;
    for (key, _) in labels {
        if let Some(rest) = key.strip_prefix(prefix)
            && let Ok(n) = rest.parse::<i32>()
        {
            max_idx = max_idx.max(n);
        }
    }
    format!("{}{}", prefix, max_idx + 1)
}

/// Click for position labels, drag for area labels.
#[derive(Debug, Default)]
pub(crate) struct ScriptLabelTool {
    /// True = drag-rectangle area labels; false = single-click position labels.
    pub(crate) place_area: bool,
    /// User-typed label name. Empty falls back to `pos{N}` / `area{N}`.
    pub(crate) name: String,
    drag_start: Option<(u32, u32)>,
}

impl ScriptLabelTool {
    /// In-flight area-label drag start. Read by the overlay to preview the rectangle.
    pub(crate) fn drag_start(&self) -> Option<(u32, u32)> {
        self.drag_start
    }
}

fn world_pos_to_tile(pos: WorldPos) -> (u32, u32) {
    (pos.x / TILE_UNITS, pos.y / TILE_UNITS)
}

impl Tool for ScriptLabelTool {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn on_mouse_press(&mut self, _ctx: &mut ToolCtx<'_>, pos: WorldPos) {
        if self.place_area {
            self.drag_start = Some(world_pos_to_tile(pos));
        }
    }

    fn on_mouse_drag(&mut self, _ctx: &mut ToolCtx<'_>, _pos: WorldPos) {
        // Rectangle finalises on release.
    }

    fn on_mouse_release(
        &mut self,
        ctx: &mut ToolCtx<'_>,
        pos: Option<WorldPos>,
    ) -> Option<Box<dyn EditCommand>> {
        if self.place_area {
            let start = self.drag_start.take();
            let (Some((sx, sy)), Some(end_pos)) = (start, pos) else {
                return None;
            };
            let (tx, ty) = world_pos_to_tile(end_pos);
            let world_x1 = sx * TILE_UNITS;
            let world_y1 = sy * TILE_UNITS;
            // +1 makes the rectangle inclusive of the end tile.
            let world_x2 = (tx + 1) * TILE_UNITS;
            let world_y2 = (ty + 1) * TILE_UNITS;
            let name = if self.name.is_empty() {
                format!("area{}", ctx.map.labels.len())
            } else {
                std::mem::take(&mut self.name)
            };
            let key = next_label_key(&ctx.map.labels, "area_");
            let label = ScriptLabel::new_area(name, world_x1, world_y1, world_x2, world_y2);
            let cmd = AddLabelCommand {
                key: key.clone(),
                label,
            };
            cmd.execute(ctx.map);
            ctx.mark_objects_dirty();
            ctx.log(format!("Added area label: {key}"));
            Some(Box::new(cmd))
        } else {
            let end_pos = pos?;
            let (tx, ty) = world_pos_to_tile(end_pos);
            let world_x = tx * TILE_UNITS + TILE_UNITS / 2;
            let world_y = ty * TILE_UNITS + TILE_UNITS / 2;
            let name = if self.name.is_empty() {
                format!("pos{}", ctx.map.labels.len())
            } else {
                std::mem::take(&mut self.name)
            };
            let key = next_label_key(&ctx.map.labels, "position_");
            let label = ScriptLabel::new_position(name, world_x, world_y);
            let cmd = AddLabelCommand {
                key: key.clone(),
                label,
            };
            cmd.execute(ctx.map);
            ctx.mark_objects_dirty();
            ctx.log(format!("Added position label: {key}"));
            Some(Box::new(cmd))
        }
    }

    fn on_deactivated(&mut self, _ctx: &mut ToolCtx<'_>) -> Option<Box<dyn EditCommand>> {
        self.drag_start = None;
        None
    }

    fn properties_ui(&mut self, ui: &mut egui::Ui, _ctx: &mut ToolCtx<'_>) {
        ui.heading("Script Label");
        ui.horizontal(|ui| {
            ui.radio_value(&mut self.place_area, false, "Position");
            ui.radio_value(&mut self.place_area, true, "Area");
        });
        ui.horizontal(|ui| {
            ui.label("Name:");
            ui.text_edit_singleline(&mut self.name);
        });
        if self.place_area {
            ui.label("Drag to create area label.");
        } else {
            ui.label("Click to place position label.");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn next_label_key_empty_labels() {
        let labels: Vec<(String, ScriptLabel)> = vec![];
        assert_eq!(next_label_key(&labels, "pos"), "pos0");
    }

    #[test]
    fn next_label_key_increments_from_existing() {
        let labels = vec![
            (
                "pos0".to_string(),
                ScriptLabel::new_position("start".into(), 100, 200),
            ),
            (
                "pos1".to_string(),
                ScriptLabel::new_position("start".into(), 300, 400),
            ),
        ];
        assert_eq!(next_label_key(&labels, "pos"), "pos2");
    }

    #[test]
    fn next_label_key_skips_different_prefix() {
        let labels = vec![
            (
                "area0".to_string(),
                ScriptLabel::new_area("zone".into(), 0, 0, 100, 100),
            ),
            (
                "pos5".to_string(),
                ScriptLabel::new_position("start".into(), 100, 200),
            ),
        ];
        assert_eq!(next_label_key(&labels, "pos"), "pos6");
        assert_eq!(next_label_key(&labels, "area"), "area1");
    }

    #[test]
    fn next_label_key_handles_gaps() {
        let labels = vec![
            (
                "pos0".to_string(),
                ScriptLabel::new_position("a".into(), 0, 0),
            ),
            (
                "pos5".to_string(),
                ScriptLabel::new_position("b".into(), 0, 0),
            ),
        ];
        assert_eq!(next_label_key(&labels, "pos"), "pos6");
    }

    #[test]
    fn add_and_delete_label_commands() {
        let mut map = WzMap::new("test", 4, 4);

        let add_cmd = AddLabelCommand {
            key: "pos0".to_string(),
            label: ScriptLabel::new_position("test".into(), 100, 200),
        };

        add_cmd.execute(&mut map);
        assert_eq!(map.labels.len(), 1);
        assert_eq!(map.labels[0].0, "pos0");

        add_cmd.undo(&mut map);
        assert!(map.labels.is_empty());
    }

    #[test]
    fn delete_label_command_roundtrip() {
        let mut map = WzMap::new("test", 4, 4);
        let label = ScriptLabel::new_position("test".into(), 100, 200);
        map.labels.push(("pos0".to_string(), label.clone()));

        let del_cmd = DeleteLabelCommand {
            index: 0,
            saved_key: "pos0".to_string(),
            saved_label: label,
        };

        del_cmd.execute(&mut map);
        assert!(map.labels.is_empty());

        del_cmd.undo(&mut map);
        assert_eq!(map.labels.len(), 1);
        assert_eq!(map.labels[0].0, "pos0");
    }

    #[test]
    fn move_label_command_roundtrip() {
        let mut map = WzMap::new("test", 4, 4);
        let old_label = ScriptLabel::new_position("test".into(), 100, 200);
        let new_label = ScriptLabel::new_position("test".into(), 300, 400);
        map.labels.push(("pos0".to_string(), old_label.clone()));

        let move_cmd = MoveLabelCommand {
            index: 0,
            old_label: old_label.clone(),
            new_label: new_label.clone(),
        };

        move_cmd.execute(&mut map);
        assert_eq!(map.labels[0].1.center().x, 300);
        assert_eq!(map.labels[0].1.center().y, 400);

        move_cmd.undo(&mut map);
        assert_eq!(map.labels[0].1.center().x, 100);
        assert_eq!(map.labels[0].1.center().y, 200);
    }

    use crate::map::history::EditHistory;
    use crate::tools::MirrorMode;
    use crate::tools::trait_def::DirtyFlags;

    fn world_pos_for(tile: (u32, u32)) -> WorldPos {
        WorldPos {
            x: tile.0 * TILE_UNITS + TILE_UNITS / 2,
            y: tile.1 * TILE_UNITS + TILE_UNITS / 2,
        }
    }

    #[test]
    fn script_label_tool_position_click_adds_position_label() {
        let mut map = WzMap::new("test", 8, 8);
        let mut history = EditHistory::new();
        let mut dirty = DirtyFlags::default();
        let mut tool = ScriptLabelTool::default();

        let mut hovered_tile: Option<(u32, u32)> = None;
        let mut log_sink = |_msg: String| {};
        let mut dirty_tiles = rustc_hash::FxHashSet::default();
        let mut stroke_active = false;
        let mut ctx = ToolCtx {
            map: &mut map,
            history: &mut history,
            dirty: &mut dirty,
            stats: None,
            placement_player: 0,
            mirror_mode: MirrorMode::None,
            terrain_dirty_tiles: &mut dirty_tiles,
            stroke_active: &mut stroke_active,
            tile_pools: &[],
            log_sink: &mut log_sink,
            hovered_tile: &mut hovered_tile,
        };
        let cmd = tool.on_mouse_release(&mut ctx, Some(world_pos_for((3, 4))));
        assert!(cmd.is_some(), "release should return an AddLabelCommand");
        assert_eq!(ctx.map.labels.len(), 1);
        let (key, label) = &ctx.map.labels[0];
        assert!(key.starts_with("position_"), "got key {key}");
        match label {
            ScriptLabel::Position { pos, .. } => {
                assert_eq!(pos[0], 3 * TILE_UNITS + TILE_UNITS / 2);
                assert_eq!(pos[1], 4 * TILE_UNITS + TILE_UNITS / 2);
            }
            ScriptLabel::Area { .. } => panic!("expected Position label"),
        }
        assert!(dirty.objects, "objects dirty flag should be set");
    }

    #[test]
    fn script_label_tool_area_drag_adds_area_label() {
        let mut map = WzMap::new("test", 8, 8);
        let mut history = EditHistory::new();
        let mut dirty = DirtyFlags::default();
        let mut tool = ScriptLabelTool {
            place_area: true,
            ..Default::default()
        };

        let mut hovered_tile: Option<(u32, u32)> = None;
        let mut log_sink = |_msg: String| {};
        let mut dirty_tiles = rustc_hash::FxHashSet::default();
        let mut stroke_active = false;
        let mut ctx = ToolCtx {
            map: &mut map,
            history: &mut history,
            dirty: &mut dirty,
            stats: None,
            placement_player: 0,
            mirror_mode: MirrorMode::None,
            terrain_dirty_tiles: &mut dirty_tiles,
            stroke_active: &mut stroke_active,
            tile_pools: &[],
            log_sink: &mut log_sink,
            hovered_tile: &mut hovered_tile,
        };
        tool.on_mouse_press(&mut ctx, world_pos_for((1, 2)));
        assert_eq!(tool.drag_start(), Some((1, 2)));
        let cmd = tool.on_mouse_release(&mut ctx, Some(world_pos_for((4, 5))));
        assert!(cmd.is_some(), "release should return an AddLabelCommand");
        assert!(tool.drag_start().is_none(), "drag start should be cleared");
        assert_eq!(ctx.map.labels.len(), 1);
        let (key, label) = &ctx.map.labels[0];
        assert!(key.starts_with("area_"), "got key {key}");
        match label {
            ScriptLabel::Area { pos1, pos2, .. } => {
                assert_eq!(pos1[0], TILE_UNITS);
                assert_eq!(pos1[1], 2 * TILE_UNITS);
                assert_eq!(pos2[0], 5 * TILE_UNITS);
                assert_eq!(pos2[1], 6 * TILE_UNITS);
            }
            ScriptLabel::Position { .. } => panic!("expected Area label"),
        }
        assert!(dirty.objects, "objects dirty flag should be set");
    }

    #[test]
    fn script_label_tool_release_with_no_press_in_area_mode_returns_none() {
        let mut map = WzMap::new("test", 4, 4);
        let mut history = EditHistory::new();
        let mut dirty = DirtyFlags::default();
        let mut tool = ScriptLabelTool {
            place_area: true,
            ..Default::default()
        };
        let mut hovered_tile: Option<(u32, u32)> = None;
        let mut log_sink = |_msg: String| {};
        let mut dirty_tiles = rustc_hash::FxHashSet::default();
        let mut stroke_active = false;
        let mut ctx = ToolCtx {
            map: &mut map,
            history: &mut history,
            dirty: &mut dirty,
            stats: None,
            placement_player: 0,
            mirror_mode: MirrorMode::None,
            terrain_dirty_tiles: &mut dirty_tiles,
            stroke_active: &mut stroke_active,
            tile_pools: &[],
            log_sink: &mut log_sink,
            hovered_tile: &mut hovered_tile,
        };
        assert!(
            tool.on_mouse_release(&mut ctx, Some(world_pos_for((1, 1))))
                .is_none()
        );
        assert!(ctx.map.labels.is_empty());
    }
}
