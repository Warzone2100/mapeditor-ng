//! Gateway creation, editing, and deletion with undo/redo.

use wz_maplib::WzMap;
use wz_maplib::constants::TILE_UNITS;
use wz_maplib::map_data::Gateway;
use wz_maplib::objects::WorldPos;

use crate::map::history::EditCommand;
use crate::tools::trait_def::{Tool, ToolCtx};

#[derive(Debug)]
pub struct AddGatewayCommand {
    pub gateway: Gateway,
}

impl EditCommand for AddGatewayCommand {
    fn execute(&self, map: &mut WzMap) {
        map.map_data.gateways.push(self.gateway);
    }

    fn undo(&self, map: &mut WzMap) {
        map.map_data.gateways.pop();
    }
}

#[derive(Debug)]
pub struct DeleteGatewayCommand {
    pub index: usize,
    pub saved: Gateway,
}

impl EditCommand for DeleteGatewayCommand {
    fn execute(&self, map: &mut WzMap) {
        if self.index < map.map_data.gateways.len() {
            map.map_data.gateways.remove(self.index);
        }
    }

    fn undo(&self, map: &mut WzMap) {
        let idx = self.index.min(map.map_data.gateways.len());
        map.map_data.gateways.insert(idx, self.saved);
    }
}

/// Drag-rectangle gateway placement. Owns the in-flight start tile.
#[derive(Debug, Default)]
pub(crate) struct GatewayTool {
    drag_start: Option<(u32, u32)>,
}

#[cfg(test)]
impl GatewayTool {
    fn drag_start(&self) -> Option<(u32, u32)> {
        self.drag_start
    }
}

fn world_pos_to_tile(pos: WorldPos) -> (u32, u32) {
    (pos.x / TILE_UNITS, pos.y / TILE_UNITS)
}

impl Tool for GatewayTool {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn on_mouse_press(&mut self, _ctx: &mut ToolCtx<'_>, pos: WorldPos) {
        self.drag_start = Some(world_pos_to_tile(pos));
    }

    fn on_mouse_drag(&mut self, _ctx: &mut ToolCtx<'_>, _pos: WorldPos) {
        // Rectangle finalises on release; no per-frame work.
    }

    fn on_mouse_release(
        &mut self,
        ctx: &mut ToolCtx<'_>,
        pos: Option<WorldPos>,
    ) -> Option<Box<dyn EditCommand>> {
        let start = self.drag_start.take();
        let (Some((sx, sy)), Some(end_pos)) = (start, pos) else {
            return None;
        };
        let (tx, ty) = world_pos_to_tile(end_pos);
        // Gateway endpoints are stored as u8, so 255 is the largest tile
        // index any side of the rectangle can reference.
        let min_x = sx.min(tx).min(255);
        let min_y = sy.min(ty).min(255);
        let max_x = sx.max(tx).min(255);
        let max_y = sy.max(ty).min(255);
        let gw = Gateway {
            x1: min_x as u8,
            y1: min_y as u8,
            x2: max_x as u8,
            y2: max_y as u8,
        };
        let cmd = AddGatewayCommand { gateway: gw };
        cmd.execute(ctx.map);
        ctx.mark_objects_dirty();
        ctx.log(format!(
            "Added gateway: ({},{}) - ({},{})",
            gw.x1, gw.y1, gw.x2, gw.y2
        ));
        Some(Box::new(cmd))
    }

    fn on_deactivated(&mut self, _ctx: &mut ToolCtx<'_>) -> Option<Box<dyn EditCommand>> {
        self.drag_start = None;
        None
    }

    fn properties_ui(&mut self, ui: &mut egui::Ui, _ctx: &mut ToolCtx<'_>) {
        ui.heading("Gateway");
        ui.label("Click and drag to create gateways.");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
    fn gateway_tool_press_release_places_gateway_and_returns_command() {
        let mut map = WzMap::new("test", 8, 8);
        let mut history = EditHistory::new();
        let mut dirty = DirtyFlags::default();
        let mut tool = GatewayTool::default();

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
        assert!(cmd.is_some(), "release should return an AddGatewayCommand");
        assert!(tool.drag_start().is_none(), "drag start should be cleared");
        assert_eq!(ctx.map.map_data.gateways.len(), 1);
        let gw = ctx.map.map_data.gateways[0];
        assert_eq!((gw.x1, gw.y1, gw.x2, gw.y2), (1, 2, 4, 5));
        assert!(dirty.objects, "objects dirty flag should be set");
    }

    #[test]
    fn gateway_tool_release_with_no_press_returns_none() {
        let mut map = WzMap::new("test", 4, 4);
        let mut history = EditHistory::new();
        let mut dirty = DirtyFlags::default();
        let mut tool = GatewayTool::default();
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
        assert!(tool.on_deactivated(&mut ctx).is_none());
        assert!(ctx.map.map_data.gateways.is_empty());
    }

    #[test]
    fn gateway_tool_release_off_terrain_returns_none_and_clears_start() {
        let mut map = WzMap::new("test", 4, 4);
        let mut history = EditHistory::new();
        let mut dirty = DirtyFlags::default();
        let mut tool = GatewayTool::default();
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
        tool.on_mouse_press(&mut ctx, world_pos_for((2, 2)));
        assert!(tool.on_mouse_release(&mut ctx, None).is_none());
        assert!(tool.drag_start().is_none());
        assert!(ctx.map.map_data.gateways.is_empty());
    }
}
