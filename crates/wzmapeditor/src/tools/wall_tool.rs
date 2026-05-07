//! Wall-family aware drag placement.
//!
//! Every painted tile is normally saved under the family's base `WALL`
//! stat. The shape variant (straight / cross / T / L) is recomputed at
//! render time from the tile's 4-neighbor mask; the base stat's
//! `structureModel` array carries the four PIE variants (see
//! `viewport_panel/object_rendering.rs::wall_model_for`). Rotation follows
//! the `wallDir` table at `~/warzone2100/src/structure.cpp:1226`.
//!
//! Single-model families (`BaBa`, Tank Trap) have only one entry in
//! `structureModel`, so at non-straight positions the renderer falls back
//! to the paired `CORNER WALL` stat's PIE, with a per-family rotation
//! offset to align authored legs with the `wallDir` table.
//!
//! The "cross corners" option saves new L-corner tiles under the family's
//! `CWall` stat instead of the base stat, but only for families whose
//! `CWall` PIE is a cross (Hardcrete, Collective, NEXUS). These tiles
//! bypass mask-derived shape resolution at render time and survive
//! save/reload (no auto-migration for cross-PIE `CWalls`).

use std::collections::HashSet;

use egui::RichText;

use wz_maplib::constants::TILE_UNITS;
use wz_maplib::objects::{Structure, WorldPos};

use crate::map::history::{CompoundCommand, EditCommand};
use crate::tools::object_edit::{DeleteObjectCommand, PlaceStructureCommand};
use crate::tools::trait_def::{Tool, ToolCtx};

/// A selectable wall family, identified by its WALL-type stat name.
///
/// The game resolves corner / T / cross visuals by indexing the stat's
/// `structureModel` array at placement time, so there is no separate
/// "corner stat" to track in the UI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct WallFamily {
    pub label: String,
    pub base: String,
}

/// Hardcoded friendly labels for base-game wall stats. Anything else with
/// `type: "WALL"` falls through to [`enumerate_families`]'s generic pass.
const DEFAULT_FAMILIES: &[(&str, &str)] = &[
    ("Hardcrete Mk1", "A0HardcreteMk1Wall"),
    ("Collective", "CollectiveWall"),
    ("NEXUS", "NEXUSWall"),
    ("BaBa", "A0BaBaHorizontalWall"),
    ("Tank Trap", "A0TankTrap"),
];

/// Maps a base wall stat to its separate `CORNER WALL` stat plus an extra
/// Y-rotation (in WZ2100 direction units) that aligns the corner PIE's
/// authored orientation with the `wallDir` table.
///
/// Every base-game wall has a paired corner stat. Hardcrete's base
/// `structureModel` carries four variants (straight/cross/T/L) but the
/// game only uses `pIMD[0]` on map load, so even Hardcrete has to route
/// through its `CWall` stat to render a corner in game. Single-model
/// families (`BaBa`, Tank Trap, Collective, NEXUS) need this for the
/// same reason.
///
/// The `wallDir` table is calibrated against a native ┘ corner (legs at
/// world -X and world -Z, i.e. LEFT+UP = mask 0x5). Hardcrete, Collective,
/// and NEXUS `CWalls` are cross-shaped PIEs symmetric under 90° rotations,
/// so offset 0 is fine. `A0BabaCornerWall` (BLBRBCR1.PIE) is the only
/// asymmetric L-corner in the base game, and its authored legs are at PIE
/// -X and PIE -Z. After `pie_mesh.rs`'s PIE→world Z flip (+Z in PIE = north,
/// +Z in world = south) the legs end up at world -X and world +Z, which is
/// mask 0x9 ┐, not ┘. Pre-rotating 270° (0xC000) makes a fresh `BaBa`
/// corner at direction 0 align to ┘ first so wallDir applies cleanly.
///
/// `cross_pie = true` means the `CWall` stat's authored model is a cross (+)
/// shape suitable as an intentional corner. For `BaBa` and Tank Trap the
/// `CWall` is just an L-corner asset, so the "cross corners" option is
/// meaningless and disabled.
const CORNER_WALL_PAIRS: &[(&str, &str, u16, bool)] = &[
    ("A0HardcreteMk1Wall", "A0HardcreteMk1CWall", 0, true),
    ("CollectiveWall", "CollectiveCWall", 0, true),
    ("NEXUSWall", "NEXUSCWall", 0, true),
    ("A0BaBaHorizontalWall", "A0BabaCornerWall", 0xC000, false),
    ("A0TankTrap", "TankTrapC", 0, false),
];

/// Look up the `CORNER WALL` stat paired with `base`, if any, along with the
/// extra direction offset needed to align its authored PIE orientation.
pub(crate) fn corner_wall_for(base: &str) -> Option<(&'static str, u16)> {
    CORNER_WALL_PAIRS
        .iter()
        .find_map(|&(b, c, off, _)| (b == base).then_some((c, off)))
}

/// Reverse lookup: find the base `WALL` stat that pairs with a given
/// `CORNER WALL` stat id. Used by the renderer to reshape walls from maps
/// that still store corners under the `CWall` stat (older saves).
pub(crate) fn base_wall_for_corner(corner: &str) -> Option<&'static str> {
    CORNER_WALL_PAIRS
        .iter()
        .find_map(|&(b, c, _, _)| (c == corner).then_some(b))
}

/// True when the family's `CWall` stat carries a cross-shaped PIE that's a
/// sensible intentional corner. Drives both the placement-time "cross
/// corners" toggle and the renderer's direct-render path.
pub(crate) fn family_has_cross_corner(base: &str) -> bool {
    CORNER_WALL_PAIRS
        .iter()
        .any(|&(b, _, _, has)| b == base && has)
}

/// True when `corner` is a `CWall` stat whose PIE is a cross. The renderer
/// uses this to short-circuit mask-derived shape resolution for
/// user-intended cross corners.
pub(crate) fn corner_stat_is_cross_pie(corner: &str) -> bool {
    CORNER_WALL_PAIRS
        .iter()
        .any(|&(_, c, _, has)| c == corner && has)
}

/// Rewrite legacy `CORNER WALL` stat names in-place to their paired base
/// `WALL` stat, except for cross-PIE `CWalls` which represent user-intended
/// cross corners and must survive load. Applied on load so legacy
/// L-corner `CWalls` from older editors get reshaped dynamically by the
/// in-game `structChooseWallType` scan; cross-PIE `CWalls` are left alone.
///
/// Returns the number of structures rewritten.
pub(crate) fn migrate_corner_walls_to_base(structures: &mut [Structure]) -> usize {
    let mut n = 0;
    for s in structures.iter_mut() {
        if let Some(base) = base_wall_for_corner(&s.name) {
            if family_has_cross_corner(base) {
                continue;
            }
            s.name = base.to_string();
            n += 1;
        }
    }
    n
}

/// Build the list of wall families from loaded stats. Missing default
/// entries are skipped silently; any additional WALL-type stats get
/// appended under their stat id so modded walls just show up.
pub(crate) fn enumerate_families(stats: &wz_stats::StatsDatabase) -> Vec<WallFamily> {
    let mut out = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();

    for &(label, base) in DEFAULT_FAMILIES {
        if stats.structures.contains_key(base) {
            out.push(WallFamily {
                label: label.to_string(),
                base: base.to_string(),
            });
            seen.insert(base.to_string());
        }
    }

    // Sort the modded tail by stat id so dropdown order stays stable
    // across runs (HashMap iteration is unordered).
    let mut extras: Vec<&String> = stats
        .structures
        .iter()
        .filter(|(name, s)| !seen.contains(*name) && s.structure_type.as_deref() == Some("WALL"))
        .map(|(name, _)| name)
        .collect();
    extras.sort();
    for name in extras {
        out.push(WallFamily {
            label: name.clone(),
            base: name.clone(),
        });
    }

    out
}

/// Neighbor-mask bits used by [`wall_shape_for_mask`].
pub(crate) const MASK_LEFT: u8 = 1;
pub(crate) const MASK_RIGHT: u8 = 2;
pub(crate) const MASK_UP: u8 = 4;
pub(crate) const MASK_DOWN: u8 = 8;

/// WZ2100 wall variant indices (`wallType` in `structure.cpp:1234`).
/// These index into a base wall stat's `structureModel` array.
pub(crate) const WALL_TYPE_STRAIGHT: u8 = 0;
#[cfg_attr(
    not(test),
    allow(dead_code, reason = "documents the wallType encoding; used by tests")
)]
const WALL_TYPE_CROSS: u8 = 1;
#[cfg_attr(
    not(test),
    allow(dead_code, reason = "documents the wallType encoding; used by tests")
)]
const WALL_TYPE_T: u8 = 2;
pub(crate) const WALL_TYPE_L_CORNER: u8 = 3;

/// Shape chosen for a wall tile: which variant model, and what rotation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct WallShape {
    /// Variant (0=straight, 1=cross, 2=T, 3=L). Fed to
    /// `StructureStats::pie_model_for_wall_type` at render time.
    pub wall_type: u8,
    /// Model rotation in WZ2100 direction units (0 / 0x4000 / 0x8000 / 0xC000).
    pub direction: u16,
}

/// Map a 4-neighbor mask to the correct variant + rotation.
///
/// Direct port of `wallDir[]` and `wallType[]` from WZ2100
/// `src/structure.cpp:1226-1239`.
pub(crate) fn wall_shape_for_mask(mask: u8) -> WallShape {
    const DIRS_DEG: [u16; 16] = [
        0, 0, 180, 0, 270, 0, 270, 0, 90, 90, 180, 180, 270, 90, 270, 0,
    ];
    const TYPES: [u8; 16] = [0, 0, 0, 0, 0, 3, 3, 2, 0, 3, 3, 2, 0, 2, 2, 1];
    let idx = (mask & 0x0F) as usize;
    WallShape {
        wall_type: TYPES[idx],
        direction: deg_to_direction(DIRS_DEG[idx]),
    }
}

fn deg_to_direction(deg: u16) -> u16 {
    match deg {
        90 => 0x4000,
        180 => 0x8000,
        270 => 0xC000,
        _ => 0,
    }
}

/// Stroke-scoped state. Lives in `ToolState` between drag frames.
#[derive(Default)]
pub(crate) struct WallStrokeState {
    pub last_tile: Option<(u32, u32)>,
    pub touched: HashSet<(u32, u32)>,
    pub commands: Vec<Box<dyn EditCommand>>,
}

// `Box<dyn EditCommand>` has no Debug impl; derive on `ToolState` needs one here.
impl std::fmt::Debug for WallStrokeState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WallStrokeState")
            .field("last_tile", &self.last_tile)
            .field("touched_tiles", &self.touched.len())
            .field("pending_commands", &self.commands.len())
            .finish()
    }
}

impl WallStrokeState {
    pub fn reset(&mut self) {
        self.last_tile = None;
        self.touched.clear();
        self.commands.clear();
    }

    pub fn take_commands(&mut self) -> Vec<Box<dyn EditCommand>> {
        self.last_tile = None;
        self.touched.clear();
        std::mem::take(&mut self.commands)
    }
}

/// Extend a stroke from its previous cursor position to `next`, painting
/// walls on every tile crossed (4-connected Bresenham to avoid diagonal
/// skips). Mutates `map` and appends the applied commands to `state`.
pub(crate) fn extend_stroke(
    state: &mut WallStrokeState,
    family: &WallFamily,
    next: (u32, u32),
    stats: &wz_stats::StatsDatabase,
    map: &mut wz_maplib::WzMap,
    player: i8,
    cross_corners: bool,
) {
    let path = match state.last_tile {
        Some(prev) => bresenham_4_connected(prev, next),
        None => vec![next],
    };
    for tile in path {
        place_wall_tile(state, family, tile, stats, map, player, cross_corners);
    }
    state.last_tile = Some(next);
}

fn place_wall_tile(
    state: &mut WallStrokeState,
    family: &WallFamily,
    tile: (u32, u32),
    stats: &wz_stats::StatsDatabase,
    map: &mut wz_maplib::WzMap,
    player: i8,
    cross_corners: bool,
) {
    if !tile_in_bounds(map, tile) {
        return;
    }
    if state.touched.contains(&tile) {
        return;
    }
    state.touched.insert(tile);

    let mask = compute_neighbor_mask(map, stats, family, tile);
    let shape = wall_shape_for_mask(mask);
    let (new_name, new_dir) = pick_stat_and_dir(family, shape, cross_corners);

    if let Some(idx) = find_family_structure_at(map, family, tile) {
        let cur = &map.structures[idx];
        if cur.name != new_name || cur.direction != new_dir {
            swap_family_wall(state, map, idx, new_name, new_dir);
        }
    } else {
        if tile_has_blocker(map, stats, tile) {
            return;
        }
        let structure = Structure {
            name: new_name,
            position: tile_center_world(tile),
            direction: new_dir,
            player,
            modules: 0,
            id: None,
        };
        let cmd = PlaceStructureCommand { structure };
        cmd.execute(map);
        state.commands.push(Box::new(cmd));
    }

    refresh_neighbors(state, family, tile, stats, map, cross_corners);
}

/// Pick the stat id + rotation to save for a freshly-computed wall shape.
///
/// Tiles store under the family's base `WALL` stat by default; the shape
/// variant is resolved at render time from the neighbor mask. With
/// `cross_corners` set and an L-corner mask, the family's `CWall` stat is
/// used directly so the renderer picks up the cross PIE without going
/// through mask-derived shape resolution. Only families whose `CWall` is
/// a cross PIE qualify.
fn pick_stat_and_dir(family: &WallFamily, shape: WallShape, cross_corners: bool) -> (String, u16) {
    if cross_corners
        && shape.wall_type == WALL_TYPE_L_CORNER
        && family_has_cross_corner(&family.base)
        && let Some((corner_stat, _)) = corner_wall_for(&family.base)
    {
        return (corner_stat.to_string(), 0);
    }
    (family.base.clone(), shape.direction)
}

fn swap_family_wall(
    state: &mut WallStrokeState,
    map: &mut wz_maplib::WzMap,
    idx: usize,
    new_name: String,
    new_dir: u16,
) {
    let saved = map.structures[idx].clone();
    let replacement = Structure {
        name: new_name,
        position: saved.position,
        direction: new_dir,
        player: saved.player,
        modules: 0,
        id: saved.id,
    };
    let del = DeleteObjectCommand::structure(idx, saved);
    del.execute(map);
    state.commands.push(Box::new(del));
    let place = PlaceStructureCommand {
        structure: replacement,
    };
    place.execute(map);
    state.commands.push(Box::new(place));
}

fn refresh_neighbors(
    state: &mut WallStrokeState,
    family: &WallFamily,
    center: (u32, u32),
    stats: &wz_stats::StatsDatabase,
    map: &mut wz_maplib::WzMap,
    cross_corners: bool,
) {
    for (dx, dy) in [(-1i32, 0i32), (1, 0), (0, -1), (0, 1)] {
        let Some(nt) = offset_tile(center, dx, dy) else {
            continue;
        };
        if !tile_in_bounds(map, nt) {
            continue;
        }
        let Some(idx) = find_family_structure_at(map, family, nt) else {
            continue;
        };
        let mask = compute_neighbor_mask(map, stats, family, nt);
        let shape = wall_shape_for_mask(mask);
        let (new_name, new_dir) = pick_stat_and_dir(family, shape, cross_corners);

        let cur = &map.structures[idx];
        if cur.name == new_name && cur.direction == new_dir {
            continue;
        }

        swap_family_wall(state, map, idx, new_name, new_dir);
        // No recursion: WZ2100's `structChooseWallType` only touches the
        // immediate 4 neighbors of the newly placed tile.
    }
}

/// Compute the 4-neighbor mask for `tile`, treating any of these as "present":
/// - a wall from the same family (base or corner), or
/// - any stat with `combines_with_wall` (wall towers, hardpoints, gates).
fn compute_neighbor_mask(
    map: &wz_maplib::WzMap,
    stats: &wz_stats::StatsDatabase,
    family: &WallFamily,
    tile: (u32, u32),
) -> u8 {
    let mut mask = 0u8;
    for (dx, dy, bit) in [
        (-1i32, 0i32, MASK_LEFT),
        (1, 0, MASK_RIGHT),
        (0, -1, MASK_UP),
        (0, 1, MASK_DOWN),
    ] {
        let Some(nt) = offset_tile(tile, dx, dy) else {
            continue;
        };
        if tile_has_wall_connector(map, stats, family, nt) {
            mask |= bit;
        }
    }
    mask
}

fn tile_has_wall_connector(
    map: &wz_maplib::WzMap,
    stats: &wz_stats::StatsDatabase,
    family: &WallFamily,
    tile: (u32, u32),
) -> bool {
    for s in &map.structures {
        if structure_covers_tile(s, stats, tile) && is_wall_connector(&s.name, stats, family) {
            return true;
        }
    }
    false
}

fn is_wall_connector(name: &str, stats: &wz_stats::StatsDatabase, family: &WallFamily) -> bool {
    if name == family.base {
        return true;
    }
    if let Some((corner, _)) = corner_wall_for(&family.base)
        && name == corner
    {
        return true;
    }
    if let Some(st) = stats.structures.get(name)
        && st.combines_with_wall
    {
        return true;
    }
    false
}

/// Is there any structure already on this tile that would reject a new wall?
///
/// Matches WZ2100's `FromSave=true` load path at `src/structure.cpp:1614`:
/// any existing structure on the tile rejects a second wall. Only defense
/// and gate types can replace walls (handled via `build_placement_with_wall_replace`
/// in the generic placement path, not here). Walls from the same family
/// are short-circuited by [`find_family_structure_at`] before we reach this
/// check so retraces are still idempotent.
fn tile_has_blocker(
    map: &wz_maplib::WzMap,
    stats: &wz_stats::StatsDatabase,
    tile: (u32, u32),
) -> bool {
    map.structures
        .iter()
        .any(|s| structure_covers_tile(s, stats, tile))
}

fn find_family_structure_at(
    map: &wz_maplib::WzMap,
    family: &WallFamily,
    tile: (u32, u32),
) -> Option<usize> {
    let corner_name = corner_wall_for(&family.base).map(|(c, _)| c);
    map.structures.iter().position(|s| {
        if structure_center_tile(s) != tile {
            return false;
        }
        s.name == family.base || corner_name.is_some_and(|c| c == s.name)
    })
}

fn structure_center_tile(s: &Structure) -> (u32, u32) {
    (s.position.x >> 7, s.position.y >> 7)
}

/// Does the structure's footprint (from its stat's width/breadth) include `tile`?
/// Falls back to a 1x1 footprint for stats we don't know about.
fn structure_covers_tile(s: &Structure, stats: &wz_stats::StatsDatabase, tile: (u32, u32)) -> bool {
    let (w, b) = stats.structures.get(&s.name).map_or((1, 1), |st| {
        (st.width.unwrap_or(1), st.breadth.unwrap_or(1))
    });
    let snap_dir = s.direction.wrapping_add(0x2000) & 0xC000;
    let (sx, sz) = if snap_dir == 0x4000 || snap_dir == 0xC000 {
        (b, w)
    } else {
        (w, b)
    };
    let cx = s.position.x >> 7;
    let cz = s.position.y >> 7;
    let ox = cx.saturating_sub(sx / 2);
    let oz = cz.saturating_sub(sz / 2);
    tile.0 >= ox && tile.0 < ox + sx && tile.1 >= oz && tile.1 < oz + sz
}

fn tile_center_world(tile: (u32, u32)) -> WorldPos {
    WorldPos {
        x: tile.0 * TILE_UNITS + TILE_UNITS / 2,
        y: tile.1 * TILE_UNITS + TILE_UNITS / 2,
    }
}

fn tile_in_bounds(map: &wz_maplib::WzMap, tile: (u32, u32)) -> bool {
    tile.0 < map.map_data.width && tile.1 < map.map_data.height
}

fn offset_tile(tile: (u32, u32), dx: i32, dy: i32) -> Option<(u32, u32)> {
    let nx = (tile.0 as i64) + dx as i64;
    let ny = (tile.1 as i64) + dy as i64;
    if nx < 0 || ny < 0 {
        return None;
    }
    Some((nx as u32, ny as u32))
}

/// 4-connected Bresenham: always steps one axis at a time, so diagonal
/// cursor motion still paints a continuous wall run.
fn bresenham_4_connected(a: (u32, u32), b: (u32, u32)) -> Vec<(u32, u32)> {
    let mut out = Vec::new();
    let mut x = a.0 as i64;
    let mut y = a.1 as i64;
    let x1 = b.0 as i64;
    let y1 = b.1 as i64;
    let dx = (x1 - x).abs();
    let dy = (y1 - y).abs();
    let sx = if x < x1 { 1 } else { -1 };
    let sy = if y < y1 { 1 } else { -1 };
    let mut err = dx - dy;
    out.push((x as u32, y as u32));
    while x != x1 || y != y1 {
        let e2 = err * 2;
        if e2 > -dy {
            err -= dy;
            x += sx;
        } else if e2 < dx {
            err += dx;
            y += sy;
        }
        out.push((x as u32, y as u32));
    }
    out
}

/// Stateful wall-placement tool. Owns the active family, the
/// cross-corner toggle and the in-flight stroke buffer.
#[derive(Debug, Default)]
pub(crate) struct WallTool {
    family: Option<WallFamily>,
    cross_corners: bool,
    stroke: WallStrokeState,
    /// Tile under the cursor. Drives the 3D ghost the renderer draws so
    /// the user can see which wall variant + rotation will land before
    /// they click. `None` when the cursor is off-map or the tool just
    /// activated.
    hover_tile: Option<(u32, u32)>,
}

impl WallTool {
    /// Read-only access for the renderer's ghost path.
    pub(crate) fn family(&self) -> Option<&WallFamily> {
        self.family.as_ref()
    }

    /// Whether the "save L corners under the cross-PIE `CWall` stat"
    /// toggle is on. The renderer needs this to pick the right ghost PIE.
    pub(crate) fn cross_corners(&self) -> bool {
        self.cross_corners
    }

    /// Tile under the cursor for the ghost preview.
    pub(crate) fn hover_tile(&self) -> Option<(u32, u32)> {
        self.hover_tile
    }

    fn ensure_family(&mut self, stats: &wz_stats::StatsDatabase) {
        if self.family.is_none() {
            self.family = enumerate_families(stats).into_iter().next();
        }
    }

    fn set_hover(&mut self, ctx: &mut ToolCtx<'_>, tile: Option<(u32, u32)>) {
        if self.hover_tile != tile {
            self.hover_tile = tile;
            ctx.mark_objects_dirty();
        }
    }

    fn paint_at(&mut self, ctx: &mut ToolCtx<'_>, tile: (u32, u32)) {
        let Some(stats) = ctx.stats else {
            return;
        };
        self.ensure_family(stats);
        let Some(family) = self.family.clone() else {
            return;
        };
        extend_stroke(
            &mut self.stroke,
            &family,
            tile,
            stats,
            ctx.map,
            ctx.placement_player,
            self.cross_corners,
        );
        ctx.mark_objects_dirty();
    }

    fn flush(&mut self) -> Option<Box<dyn EditCommand>> {
        let commands = self.stroke.take_commands();
        if commands.is_empty() {
            None
        } else {
            Some(Box::new(CompoundCommand::new(commands)))
        }
    }
}

fn world_pos_to_tile(pos: WorldPos) -> (u32, u32) {
    (pos.x / TILE_UNITS, pos.y / TILE_UNITS)
}

impl Tool for WallTool {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn on_mouse_press(&mut self, ctx: &mut ToolCtx<'_>, pos: WorldPos) {
        self.stroke.reset();
        self.paint_at(ctx, world_pos_to_tile(pos));
    }

    fn on_mouse_drag(&mut self, ctx: &mut ToolCtx<'_>, pos: WorldPos) {
        let tile = world_pos_to_tile(pos);
        self.set_hover(ctx, Some(tile));
        self.paint_at(ctx, tile);
    }

    fn on_mouse_hover(&mut self, ctx: &mut ToolCtx<'_>, pos: Option<WorldPos>) {
        self.set_hover(ctx, pos.map(world_pos_to_tile));
    }

    fn on_mouse_release(
        &mut self,
        _ctx: &mut ToolCtx<'_>,
        _pos: Option<WorldPos>,
    ) -> Option<Box<dyn EditCommand>> {
        self.flush()
    }

    fn on_deactivated(&mut self, ctx: &mut ToolCtx<'_>) -> Option<Box<dyn EditCommand>> {
        self.set_hover(ctx, None);
        self.flush()
    }

    fn properties_ui(&mut self, ui: &mut egui::Ui, ctx: &mut ToolCtx<'_>) {
        ui.heading("Wall Placement");

        let Some(stats) = ctx.stats else {
            ui.label("Loading stats...");
            return;
        };

        let families = enumerate_families(stats);
        if families.is_empty() {
            ui.label("No wall families found in stats.");
            return;
        }

        if self.family.is_none() {
            self.family = families.first().cloned();
        }
        let current_label = self
            .family
            .as_ref()
            .map_or("(none)".to_string(), |f| f.label.clone());
        egui::ComboBox::from_id_salt("wall_family_combo")
            .selected_text(current_label)
            .show_ui(ui, |ui| {
                for family in &families {
                    let selected = self.family.as_ref().is_some_and(|f| f == family);
                    if ui.selectable_label(selected, &family.label).clicked() {
                        self.family = Some(family.clone());
                    }
                }
            });
        if let Some(ref fam) = self.family {
            ui.label(RichText::new(format!("Stat: {}", fam.base)).small().weak());
        }

        ui.add_space(4.0);
        let family_supports_cross = self
            .family
            .as_ref()
            .is_some_and(|f| family_has_cross_corner(&f.base));
        if !family_supports_cross {
            self.cross_corners = false;
        }
        ui.add_enabled_ui(family_supports_cross, |ui| {
            let resp = ui
                .checkbox(&mut self.cross_corners, "Cross-shape corners")
                .on_hover_text(
                    "New L-corner placements save under the family's + (cross) wall stat. \
                     Existing walls are not changed.",
                );
            if !family_supports_cross {
                resp.on_disabled_hover_text("This family has no + corner variant.");
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wall_type_table_matches_wz2100() {
        let expected = [0u8, 0, 0, 0, 0, 3, 3, 2, 0, 3, 3, 2, 0, 2, 2, 1];
        for (mask, &want) in expected.iter().enumerate() {
            let got = wall_shape_for_mask(mask as u8).wall_type;
            assert_eq!(got, want, "mask {mask:#x}");
        }
    }

    #[test]
    fn direction_table_matches_wz2100() {
        let expected_deg = [
            0u16, 0, 180, 0, 270, 0, 270, 0, 90, 90, 180, 180, 270, 90, 270, 0,
        ];
        for (mask, &deg) in expected_deg.iter().enumerate() {
            let dir = wall_shape_for_mask(mask as u8).direction;
            let got_deg = (u32::from(dir) * 360 / 65536) as u16;
            assert_eq!(got_deg, deg, "mask {mask:#x}: expected {deg} deg");
        }
    }

    #[test]
    fn isolated_wall_is_straight() {
        let shape = wall_shape_for_mask(0);
        assert_eq!(shape.wall_type, WALL_TYPE_STRAIGHT);
        assert_eq!(shape.direction, 0);
    }

    #[test]
    fn horizontal_run_is_straight() {
        let shape = wall_shape_for_mask(MASK_LEFT | MASK_RIGHT);
        assert_eq!(shape.wall_type, WALL_TYPE_STRAIGHT);
    }

    #[test]
    fn vertical_run_straight_rotated() {
        let shape = wall_shape_for_mask(MASK_UP | MASK_DOWN);
        assert_eq!(shape.wall_type, WALL_TYPE_STRAIGHT);
        assert_eq!(shape.direction, 0xC000);
    }

    #[test]
    fn l_shape_uses_corner_variant() {
        for mask in [
            MASK_LEFT | MASK_UP,
            MASK_LEFT | MASK_DOWN,
            MASK_RIGHT | MASK_UP,
            MASK_RIGHT | MASK_DOWN,
        ] {
            assert_eq!(
                wall_shape_for_mask(mask).wall_type,
                WALL_TYPE_L_CORNER,
                "mask {mask:#x}"
            );
        }
    }

    #[test]
    fn t_junction_uses_t_variant() {
        let shape = wall_shape_for_mask(MASK_LEFT | MASK_RIGHT | MASK_DOWN);
        assert_eq!(shape.wall_type, WALL_TYPE_T);
    }

    #[test]
    fn cross_uses_cross_variant() {
        let shape = wall_shape_for_mask(MASK_LEFT | MASK_RIGHT | MASK_UP | MASK_DOWN);
        assert_eq!(shape.wall_type, WALL_TYPE_CROSS);
    }

    #[test]
    fn cross_corner_option_routes_l_to_cwall_for_cross_pie_families() {
        let shape = wall_shape_for_mask(MASK_LEFT | MASK_UP);
        for (base, expected_corner) in [
            ("A0HardcreteMk1Wall", "A0HardcreteMk1CWall"),
            ("CollectiveWall", "CollectiveCWall"),
            ("NEXUSWall", "NEXUSCWall"),
        ] {
            let (name, dir) = pick_stat_and_dir(&family(base), shape, true);
            assert_eq!(name, expected_corner, "family {base}");
            assert_eq!(dir, 0, "family {base}");
        }
    }

    #[test]
    fn cross_corner_option_leaves_non_cross_pie_families_alone() {
        let shape = wall_shape_for_mask(MASK_LEFT | MASK_UP);
        for base in ["A0BaBaHorizontalWall", "A0TankTrap"] {
            let (name, _) = pick_stat_and_dir(&family(base), shape, true);
            assert_eq!(name, base, "family {base}");
        }
    }

    #[test]
    fn cross_corner_option_off_uses_base_stat_at_l() {
        let shape = wall_shape_for_mask(MASK_LEFT | MASK_UP);
        let (name, dir) = pick_stat_and_dir(&family("A0HardcreteMk1Wall"), shape, false);
        assert_eq!(name, "A0HardcreteMk1Wall");
        assert_eq!(dir, shape.direction);
    }

    #[test]
    fn cross_corner_option_unaffected_at_non_l_masks() {
        for mask in 0u8..16 {
            let shape = wall_shape_for_mask(mask);
            if shape.wall_type == WALL_TYPE_L_CORNER {
                continue;
            }
            let (name, dir) = pick_stat_and_dir(&family("A0HardcreteMk1Wall"), shape, true);
            assert_eq!(name, "A0HardcreteMk1Wall", "mask {mask:#x}");
            assert_eq!(dir, shape.direction, "mask {mask:#x}");
        }
    }

    #[test]
    fn bresenham_horizontal() {
        let pts = bresenham_4_connected((2, 5), (6, 5));
        assert_eq!(pts, vec![(2, 5), (3, 5), (4, 5), (5, 5), (6, 5)]);
    }

    #[test]
    fn bresenham_vertical() {
        let pts = bresenham_4_connected((3, 2), (3, 5));
        assert_eq!(pts, vec![(3, 2), (3, 3), (3, 4), (3, 5)]);
    }

    fn family(base: &str) -> WallFamily {
        WallFamily {
            label: base.to_string(),
            base: base.to_string(),
        }
    }

    #[test]
    fn straight_tile_saves_under_base_stat() {
        let shape = wall_shape_for_mask(MASK_LEFT | MASK_RIGHT);
        let (name, dir) = pick_stat_and_dir(&family("A0HardcreteMk1Wall"), shape, false);
        assert_eq!(name, "A0HardcreteMk1Wall");
        assert_eq!(dir, shape.direction);
    }

    #[test]
    fn l_corner_tile_saves_under_base_wall_stat() {
        let shape = wall_shape_for_mask(MASK_LEFT | MASK_UP);
        let (name, dir) = pick_stat_and_dir(&family("A0HardcreteMk1Wall"), shape, false);
        assert_eq!(name, "A0HardcreteMk1Wall");
        assert_eq!(dir, shape.direction);
    }

    #[test]
    fn t_and_cross_tiles_save_under_base_wall_stat() {
        for mask in [
            MASK_LEFT | MASK_RIGHT | MASK_DOWN,
            MASK_LEFT | MASK_RIGHT | MASK_UP | MASK_DOWN,
        ] {
            let shape = wall_shape_for_mask(mask);
            let (name, _) = pick_stat_and_dir(&family("CollectiveWall"), shape, false);
            assert_eq!(name, "CollectiveWall", "mask {mask:#x}");
        }
    }

    #[test]
    fn base_wall_for_corner_round_trips() {
        assert_eq!(
            base_wall_for_corner("A0HardcreteMk1CWall"),
            Some("A0HardcreteMk1Wall")
        );
        assert_eq!(
            base_wall_for_corner("A0BabaCornerWall"),
            Some("A0BaBaHorizontalWall")
        );
        assert_eq!(base_wall_for_corner("NotAWall"), None);
    }

    #[test]
    fn all_default_families_pair_with_a_cwall() {
        for &(_, base) in DEFAULT_FAMILIES {
            assert!(
                corner_wall_for(base).is_some(),
                "default family {base} has no paired CWall"
            );
        }
    }

    #[test]
    fn migrate_rewrites_only_non_cross_pie_cwalls() {
        let pos = WorldPos { x: 0, y: 0 };
        let mut structures = vec![
            Structure {
                name: "A0HardcreteMk1CWall".into(),
                position: pos,
                direction: 0,
                player: 0,
                modules: 0,
                id: None,
            },
            Structure {
                name: "A0HardcreteMk1Wall".into(),
                position: pos,
                direction: 0,
                player: 0,
                modules: 0,
                id: None,
            },
            Structure {
                name: "A0BabaCornerWall".into(),
                position: pos,
                direction: 0x4000,
                player: 0,
                modules: 0,
                id: None,
            },
            Structure {
                name: "A0CommandCentre".into(),
                position: pos,
                direction: 0,
                player: 0,
                modules: 0,
                id: None,
            },
        ];
        let n = migrate_corner_walls_to_base(&mut structures);
        assert_eq!(n, 1);
        assert_eq!(structures[0].name, "A0HardcreteMk1CWall");
        assert_eq!(structures[1].name, "A0HardcreteMk1Wall");
        assert_eq!(structures[2].name, "A0BaBaHorizontalWall");
        assert_eq!(structures[2].direction, 0x4000);
        assert_eq!(structures[3].name, "A0CommandCentre");
    }

    #[test]
    fn wall_tool_press_release_places_structure_and_returns_command() {
        use crate::map::history::EditHistory;
        use crate::tools::trait_def::{DirtyFlags, Tool, ToolCtx};
        use wz_stats::StatsDatabase;

        let mut stats = StatsDatabase::default();
        let stat = wz_stats::structures::StructureStats {
            id: "A0HardcreteMk1Wall".into(),
            name: "Hardcrete Wall".into(),
            structure_type: Some("WALL".into()),
            width: Some(1),
            breadth: Some(1),
            ..Default::default()
        };
        stats.structures.insert("A0HardcreteMk1Wall".into(), stat);

        let mut map = wz_maplib::WzMap::new("test", 8, 8);
        let mut history = EditHistory::new();
        let mut dirty = DirtyFlags::default();
        let mut tool = WallTool::default();

        let pos = WorldPos {
            x: 4 * TILE_UNITS + TILE_UNITS / 2,
            y: 4 * TILE_UNITS + TILE_UNITS / 2,
        };
        let mut hovered_tile: Option<(u32, u32)> = None;
        let mut log_sink = |_msg: String| {};
        let mut dirty_tiles = rustc_hash::FxHashSet::default();
        let mut stroke_active = false;
        let mut ctx = ToolCtx {
            map: &mut map,
            history: &mut history,
            dirty: &mut dirty,
            stats: Some(&stats),
            placement_player: 0,
            mirror_mode: crate::tools::MirrorMode::None,
            terrain_dirty_tiles: &mut dirty_tiles,
            stroke_active: &mut stroke_active,
            tile_pools: &[],
            log_sink: &mut log_sink,
            hovered_tile: &mut hovered_tile,
        };
        tool.on_mouse_press(&mut ctx, pos);
        assert_eq!(ctx.map.structures.len(), 1, "press should place a wall");
        let cmd = tool.on_mouse_release(&mut ctx, None);
        assert!(cmd.is_some(), "release should return a CompoundCommand");
        assert!(dirty.objects, "objects dirty flag should be set");
    }

    #[test]
    fn wall_tool_release_with_no_press_returns_none() {
        use crate::map::history::EditHistory;
        use crate::tools::trait_def::{DirtyFlags, Tool, ToolCtx};

        let mut map = wz_maplib::WzMap::new("test", 4, 4);
        let mut history = EditHistory::new();
        let mut dirty = DirtyFlags::default();
        let mut tool = WallTool::default();
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
            mirror_mode: crate::tools::MirrorMode::None,
            terrain_dirty_tiles: &mut dirty_tiles,
            stroke_active: &mut stroke_active,
            tile_pools: &[],
            log_sink: &mut log_sink,
            hovered_tile: &mut hovered_tile,
        };
        assert!(tool.on_mouse_release(&mut ctx, None).is_none());
        assert!(tool.on_deactivated(&mut ctx).is_none());
    }

    #[test]
    fn bresenham_never_skips_tiles_diagonally() {
        let pts = bresenham_4_connected((0, 0), (3, 3));
        for win in pts.windows(2) {
            let dx = (win[0].0 as i32 - win[1].0 as i32).abs();
            let dy = (win[0].1 as i32 - win[1].1 as i32).abs();
            assert_eq!(dx + dy, 1, "steps must be 4-connected: {win:?}");
        }
        assert_eq!(pts.first(), Some(&(0, 0)));
        assert_eq!(pts.last(), Some(&(3, 3)));
    }
}
