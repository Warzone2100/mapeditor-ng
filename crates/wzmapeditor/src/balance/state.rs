//! Editor-side cache for the per-player balance summary. The analysis is
//! pure data and microseconds-fast, so we run it synchronously on demand
//! rather than spawning a worker.

use std::collections::{HashMap, HashSet};

use egui::Color32;
use wz_maplib::WzMap;

use super::analysis::{BalanceReport, run_balance_analysis};

#[derive(Debug, Default)]
pub struct BalanceState {
    pub report: Option<BalanceReport>,
    /// Players the user has ticked in the panel; the viewport overlay
    /// rings their structures, droids, and Voronoi-assigned oil tiles.
    pub highlighted_players: HashSet<i8>,
    /// Draw the per-tile nearest-player partition as overlay lines so the
    /// user can see the boundary the oil count actually uses.
    pub show_voronoi: bool,
    /// Tint each Voronoi cell faintly with its owning player's slot color.
    /// Independent of `show_voronoi` so the user can pick lines, fill, both,
    /// or neither.
    pub show_voronoi_tint: bool,
    /// Cycle index per `(player, structure_name)` so repeat-clicks on a
    /// breakdown row step through every copy instead of always landing
    /// on the first one.
    pub breakdown_cycle: HashMap<(i8, String), usize>,
    /// When true, the structure breakdown hides any name where every
    /// player has the same count. Anything that's left is what's making
    /// the layout uneven.
    pub breakdown_diff_only: bool,
}

impl BalanceState {
    pub fn clear(&mut self) {
        self.report = None;
        self.highlighted_players.clear();
        self.breakdown_cycle.clear();
    }

    pub fn refresh(&mut self, map: &WzMap) {
        self.report = Some(run_balance_analysis(map));
    }

    pub fn ensure(&mut self, map: &WzMap) {
        if self.report.is_none() {
            self.refresh(map);
        }
    }
}

/// Player slot color, matching the palette used for actual structure
/// rendering (`viewport::pie_mesh::team_color`) so the balance overlay
/// agrees with what the user sees on the map: P0 orange, P1 green, P2
/// grey, P3 black, P4 red, P5 blue, P6 pink, P7 cyan, plus the 8 extras.
#[must_use]
pub fn player_color(player: i8) -> Color32 {
    if player < 0 {
        return Color32::from_rgb(170, 170, 170);
    }
    let c = crate::viewport::pie_mesh::team_color(player);
    Color32::from_rgb(
        (c[0] * 255.0) as u8,
        (c[1] * 255.0) as u8,
        (c[2] * 255.0) as u8,
    )
}
