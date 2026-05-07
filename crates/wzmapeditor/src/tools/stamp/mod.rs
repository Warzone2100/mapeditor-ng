//! Stamp tool: capture a rectangular tile/object pattern and stamp it repeatedly.
//!
//! The implementation is split into focused submodules:
//! - [`pattern`] holds the captured pattern data and rectangular capture.
//! - [`transform`] handles rotation, mirror, and direction transforms.
//! - [`placement`] applies a pattern to the map (single-stamp + `push_object`).
//! - [`scatter`] randomises object placement inside a circular brush.
//! - [`command`] defines the reversible `StampCommand` used by undo/redo.

pub mod command;
pub mod pattern;
pub mod placement;
pub mod scatter;
pub mod transform;

pub use command::StampCommand;
pub use pattern::{StampObject, StampPattern, capture_pattern};

use placement::apply_stamp;
use scatter::apply_scatter;
use transform::transform_pattern;

use wz_maplib::objects::WorldPos;

use crate::map::history::{CompoundCommand, EditCommand};
use crate::tools::StampMode;
use crate::tools::mirror;
use crate::tools::trait_def::{Tool, ToolCtx};

/// Stateful Stamp tool: capture-then-place, with Single and Scatter sub-modes.
#[derive(Debug)]
pub struct StampTool {
    /// Captured pattern, `None` until the user drags a capture rectangle.
    pub pattern: Option<StampPattern>,
    /// `true` while waiting for the user to drag a capture rectangle. Flips
    /// to `false` once a pattern is captured and the tool moves into placement.
    pub capture_mode: bool,
    /// Start tile of the in-progress capture drag. Read by the overlay layer
    /// to draw the live capture rectangle.
    pub capture_start: Option<(u32, u32)>,
    /// Whether to write the pattern's tile texture and orientation.
    pub stamp_tiles: bool,
    /// Whether to write the pattern's tile heights.
    pub stamp_terrain: bool,
    /// Whether to place the pattern's structures, droids, and features.
    pub stamp_objects: bool,
    /// Pick a random 0/90/180/270 rotation per stamp click.
    pub random_rotation: bool,
    /// Pick a random X/Y flip combo per stamp click.
    pub random_flip: bool,
    /// Hover preview origin for the ghost overlay.
    pub preview_pos: Option<(u32, u32)>,
    /// Single vs Scatter sub-mode.
    pub mode: StampMode,
    /// Scatter brush radius in tiles.
    pub scatter_radius: u32,
    /// Scatter density: objects per tile squared.
    pub scatter_density: f32,
    /// Minimum cursor travel between scatter bursts during a drag.
    pub scatter_stroke_spacing: u32,
    /// Minimum world-unit spacing between objects in a single burst.
    pub scatter_min_spacing: u32,
    /// In-flight scatter commands accumulated during a drag stroke.
    scatter_stroke: Vec<StampCommand>,
    /// Last scatter burst tile, used to rate-limit drag-paint.
    scatter_last_tile: Option<(u32, u32)>,
}

impl Default for StampTool {
    fn default() -> Self {
        Self {
            pattern: None,
            capture_mode: true,
            capture_start: None,
            stamp_tiles: true,
            stamp_terrain: true,
            stamp_objects: true,
            random_rotation: false,
            random_flip: false,
            preview_pos: None,
            mode: StampMode::Single,
            // Radius 3 covers ~28 tile squares, density 0.05 ~= 1 object per burst.
            scatter_radius: 3,
            scatter_density: 0.05,
            // Two-tile cursor travel between bursts keeps drag-paint sparse.
            scatter_stroke_spacing: 2,
            scatter_min_spacing: 0,
            scatter_stroke: Vec::new(),
            scatter_last_tile: None,
        }
    }
}

fn world_pos_to_tile(pos: WorldPos) -> (u32, u32) {
    use wz_maplib::constants::TILE_UNITS;
    (pos.x / TILE_UNITS, pos.y / TILE_UNITS)
}

fn stamp_command_non_empty(cmd: &StampCommand) -> bool {
    !cmd.tile_changes.is_empty()
        || !cmd.structures.is_empty()
        || !cmd.droids.is_empty()
        || !cmd.features.is_empty()
}

fn pack_commands_as_one(commands: Vec<Box<dyn EditCommand>>) -> Option<Box<dyn EditCommand>> {
    match commands.len() {
        0 => None,
        1 => Some(commands.into_iter().next().expect("checked len == 1")),
        _ => Some(Box::new(CompoundCommand::new(commands))),
    }
}

impl StampTool {
    fn update_preview(&mut self, ctx: &mut ToolCtx<'_>, new_pos: Option<(u32, u32)>) {
        if self.preview_pos != new_pos {
            self.preview_pos = new_pos;
            ctx.mark_objects_dirty();
        }
    }

    /// Map a hovered tile to the stamp's preview origin. Single centres the
    /// pattern footprint on the cursor; Scatter uses the cursor tile directly
    /// as the burst centre.
    fn preview_origin_for(&self, tile: (u32, u32)) -> (u32, u32) {
        match self.mode {
            StampMode::Single => {
                let (pw, ph) = self
                    .pattern
                    .as_ref()
                    .map_or((0, 0), |p| (p.width, p.height));
                (tile.0.saturating_sub(pw / 2), tile.1.saturating_sub(ph / 2))
            }
            StampMode::Scatter => tile,
        }
    }

    // Live capture flows through ToolSwitchRequest::StampWithPattern from object_tools.rs.
    fn capture_complete(&mut self, ctx: &mut ToolCtx<'_>, sx: u32, sy: u32, tx: u32, ty: u32) {
        let pattern = capture_pattern(ctx.map, sx, sy, tx, ty);
        let tile_count = pattern.tiles.len();
        let obj_count = pattern.objects.len();
        self.pattern = Some(pattern);
        self.capture_mode = false;
        ctx.log(format!(
            "Captured stamp pattern: {tile_count} tiles, {obj_count} objects"
        ));
    }

    fn apply_single(
        &mut self,
        ctx: &mut ToolCtx<'_>,
        tx: u32,
        ty: u32,
    ) -> Option<Box<dyn EditCommand>> {
        let base_pattern = self.pattern.as_ref()?;
        let mut rng = fastrand::Rng::new();
        let rot_steps = if self.random_rotation { rng.u8(..4) } else { 0 };
        let (flip_x, flip_y) = if self.random_flip {
            (rng.bool(), rng.bool())
        } else {
            (false, false)
        };
        let pattern = if rot_steps != 0 || flip_x || flip_y {
            transform_pattern(base_pattern, rot_steps, flip_x, flip_y)
        } else {
            base_pattern.clone()
        };
        let map_w = ctx.map.map_data.width;
        let map_h = ctx.map.map_data.height;
        let mirror_pts = mirror::mirror_points(tx, ty, map_w, map_h, ctx.mirror_mode);

        let stamp_tiles = self.stamp_tiles;
        let stamp_terrain = self.stamp_terrain;
        let stamp_objects = self.stamp_objects;
        let mut commands: Vec<Box<dyn EditCommand>> = Vec::new();
        for &(mx, my) in &mirror_pts {
            let cmd = apply_stamp(
                ctx.map,
                &pattern,
                mx,
                my,
                stamp_tiles,
                stamp_terrain,
                stamp_objects,
            );
            if stamp_command_non_empty(&cmd) {
                commands.push(Box::new(cmd));
            }
        }
        if stamp_tiles || stamp_terrain {
            ctx.mark_terrain_dirty();
            ctx.mark_minimap_dirty();
        }
        if stamp_objects {
            ctx.mark_objects_dirty();
        }
        ctx.log("Stamped pattern".to_string());
        pack_commands_as_one(commands)
    }

    fn scatter_burst(&mut self, ctx: &mut ToolCtx<'_>, tx: u32, ty: u32) {
        let Some(base_pattern) = self.pattern.as_ref() else {
            return;
        };
        if base_pattern.objects.is_empty() {
            return;
        }
        let mut rng = fastrand::Rng::new();
        let radius = self.scatter_radius;
        let density = self.scatter_density;
        let min_spacing = self.scatter_min_spacing;
        let rand_rot = self.random_rotation;
        let rand_flip = self.random_flip;
        let map_w = ctx.map.map_data.width;
        let map_h = ctx.map.map_data.height;
        let mirror_pts = mirror::mirror_points(tx, ty, map_w, map_h, ctx.mirror_mode);
        for &(mx, my) in &mirror_pts {
            let cmd = apply_scatter(
                ctx.map,
                base_pattern,
                mx,
                my,
                radius,
                density,
                min_spacing,
                rand_rot,
                rand_flip,
                &mut rng,
            );
            if stamp_command_non_empty(&cmd) {
                self.scatter_stroke.push(cmd);
            }
        }
        ctx.mark_objects_dirty();
    }

    fn finalize_scatter(&mut self, ctx: &mut ToolCtx<'_>) -> Option<Box<dyn EditCommand>> {
        let stroke = std::mem::take(&mut self.scatter_stroke);
        self.scatter_last_tile = None;
        if stroke.is_empty() {
            return None;
        }
        let count = stroke.len();
        ctx.log(format!("Scattered ({count} bursts)"));
        let commands: Vec<Box<dyn EditCommand>> =
            stroke.into_iter().map(|c| Box::new(c) as _).collect();
        pack_commands_as_one(commands)
    }
}

impl Tool for StampTool {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn on_mouse_press(&mut self, ctx: &mut ToolCtx<'_>, pos: WorldPos) {
        let (tx, ty) = world_pos_to_tile(pos);
        if self.capture_mode {
            self.capture_start = Some((tx, ty));
            return;
        }
        if self.pattern.is_none() {
            return;
        }
        let (origin_x, origin_y) = self.preview_origin_for((tx, ty));
        self.update_preview(ctx, Some((origin_x, origin_y)));
        match self.mode {
            StampMode::Single => {
                if let Some(cmd) = self.apply_single(ctx, origin_x, origin_y) {
                    ctx.history.push_already_applied(cmd);
                }
            }
            StampMode::Scatter => {
                self.scatter_stroke.clear();
                self.scatter_last_tile = None;
                self.scatter_burst(ctx, tx, ty);
                self.scatter_last_tile = Some((tx, ty));
            }
        }
    }

    fn on_mouse_hover(&mut self, ctx: &mut ToolCtx<'_>, pos: Option<WorldPos>) {
        if self.capture_mode || self.pattern.is_none() {
            return;
        }
        let new_pos = pos.map(|p| self.preview_origin_for(world_pos_to_tile(p)));
        self.update_preview(ctx, new_pos);
    }

    fn on_mouse_drag(&mut self, ctx: &mut ToolCtx<'_>, pos: WorldPos) {
        let (tx, ty) = world_pos_to_tile(pos);
        if self.capture_mode {
            // Capture is start + release; the overlay reads capture_start directly.
            return;
        }
        if self.pattern.is_none() {
            return;
        }
        let origin = self.preview_origin_for((tx, ty));
        self.update_preview(ctx, Some(origin));
        match self.mode {
            StampMode::Single => {}
            StampMode::Scatter => {
                let should_fire = match self.scatter_last_tile {
                    None => true,
                    Some((lx, ly)) => {
                        let spacing = self.scatter_stroke_spacing.max(1);
                        let dx = tx.abs_diff(lx);
                        let dy = ty.abs_diff(ly);
                        dx.max(dy) >= spacing
                    }
                };
                if should_fire {
                    self.scatter_burst(ctx, tx, ty);
                    self.scatter_last_tile = Some((tx, ty));
                }
            }
        }
    }

    fn on_mouse_release(
        &mut self,
        ctx: &mut ToolCtx<'_>,
        pos: Option<WorldPos>,
    ) -> Option<Box<dyn EditCommand>> {
        if self.capture_mode {
            if let (Some((sx, sy)), Some(end)) = (self.capture_start.take(), pos) {
                let (tx, ty) = world_pos_to_tile(end);
                self.capture_complete(ctx, sx, sy, tx, ty);
            }
            return None;
        }
        match self.mode {
            StampMode::Single => None,
            StampMode::Scatter => self.finalize_scatter(ctx),
        }
    }

    fn on_deactivated(&mut self, ctx: &mut ToolCtx<'_>) -> Option<Box<dyn EditCommand>> {
        let cmd = self.finalize_scatter(ctx);
        self.preview_pos = None;
        self.capture_start = None;
        cmd
    }

    fn on_secondary_click(&mut self, ctx: &mut ToolCtx<'_>) -> bool {
        if self.capture_start.is_some() {
            self.capture_start = None;
            ctx.log("Stamp capture cancelled".to_string());
            return true;
        }
        if !self.capture_mode && self.pattern.is_some() {
            self.capture_mode = true;
            self.pattern = None;
            self.preview_pos = None;
            ctx.mark_objects_dirty();
            ctx.log("Stamp cancelled, switched to capture mode".to_string());
            return true;
        }
        false
    }

    fn properties_ui(&mut self, ui: &mut egui::Ui, _ctx: &mut ToolCtx<'_>) {
        use egui::RichText;
        ui.heading("Stamp Tool");
        if self.capture_mode {
            ui.label("CAPTURE MODE");
            ui.label("Drag a rectangle on the map to capture a pattern.");
            if self.pattern.is_some() {
                ui.separator();
                if ui.button("Switch to Stamp Mode").clicked() {
                    self.capture_mode = false;
                }
            }
        } else if let Some(ref pattern) = self.pattern {
            ui.label("STAMP MODE");
            ui.label(format!(
                "Pattern: {}x{} tiles",
                pattern.width, pattern.height
            ));
            if !pattern.objects.is_empty() {
                ui.label(format!("{} objects captured", pattern.objects.len()));
            }
            ui.separator();
            if ui.button("Re-capture").clicked() {
                self.capture_mode = true;
                self.pattern = None;
            }
        } else {
            ui.label("No pattern captured.");
            if ui.button("Capture Pattern").clicked() {
                self.capture_mode = true;
            }
        }

        ui.separator();
        ui.label("Mode:");
        let prev_mode = self.mode;
        egui::ComboBox::from_id_salt("stamp_mode_combo")
            .selected_text(match self.mode {
                StampMode::Single => "Single",
                StampMode::Scatter => "Scatter",
            })
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut self.mode, StampMode::Single, "Single");
                ui.selectable_value(&mut self.mode, StampMode::Scatter, "Scatter");
            });
        if prev_mode != self.mode {
            self.pattern = None;
            self.capture_mode = true;
            self.preview_pos = None;
            self.capture_start = None;
            self.scatter_stroke.clear();
            self.scatter_last_tile = None;
        }

        ui.separator();
        match self.mode {
            StampMode::Single => {
                ui.checkbox(&mut self.stamp_tiles, "Stamp Tiles")
                    .on_hover_text("Write the pattern's tile textures and orientation");
                ui.checkbox(&mut self.stamp_terrain, "Stamp Terrain")
                    .on_hover_text("Write the pattern's tile heights (terrain elevation)");
                ui.checkbox(&mut self.stamp_objects, "Stamp Objects");
                ui.separator();
                ui.checkbox(&mut self.random_rotation, "Random Rotation");
                ui.checkbox(&mut self.random_flip, "Random Flip");
            }
            StampMode::Scatter => {
                ui.label("Click or drag to scatter objects within a circular brush.");
                ui.label("(Tiles are not stamped in Scatter mode.)");
                ui.add(egui::Slider::new(&mut self.scatter_radius, 1..=20).text("Radius (tiles)"));
                ui.add(
                    egui::Slider::new(&mut self.scatter_density, 0.01_f32..=1.0_f32)
                        .text("Density (per tile²)"),
                );
                let radius = self.scatter_radius as f32;
                let density = self.scatter_density;
                let burst_area = std::f32::consts::PI * radius * radius;
                let burst_count = (burst_area * density).round().max(1.0) as u32;
                ui.label(format!("≈ {burst_count} object(s) per burst"));

                ui.separator();
                ui.add(
                    egui::Slider::new(&mut self.scatter_stroke_spacing, 1..=10)
                        .text("Stroke spacing (tiles)"),
                );
                ui.add(
                    egui::Slider::new(&mut self.scatter_min_spacing, 0..=256)
                        .text("Min object spacing"),
                );

                ui.separator();
                ui.checkbox(&mut self.random_rotation, "Random Rotation");
                ui.checkbox(&mut self.random_flip, "Random Flip");
            }
        }
        let _ = RichText::new("");
    }
}
