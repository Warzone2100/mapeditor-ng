//! Per-player oil, structure, and droid counts for the Balance panel. Pure
//! data crunching over a [`WzMap`], microseconds even on the largest workshop
//! maps, so the editor calls it synchronously instead of spawning a worker.

use std::collections::{BTreeMap, HashMap};

use wz_maplib::constants::map_coord;
use wz_maplib::io_wz::WzMap;

/// Structure name prefix that identifies a player HQ. `A0CommandCentre`,
/// `A0CommandCentreNP`, etc. all start with this.
const HQ_NAME_PREFIX: &str = "A0CommandCentre";

/// One player's slice of [`BalanceReport`]. Players with no structures
/// don't appear; the report only enumerates colors that own at least one
/// placed object.
#[derive(Debug, Clone)]
pub struct PlayerBalance {
    pub player: i8,
    /// Tile position used for Voronoi oil assignment. The HQ if one exists,
    /// otherwise the first-listed structure for that player.
    pub start_tile: (u32, u32),
    pub has_hq: bool,
    pub oil_count: u32,
    pub structure_count: u32,
    pub droid_count: u32,
    /// Structure-name -> count, sorted alphabetically so two players with
    /// the same lineup compare equal.
    pub structures: BTreeMap<String, u32>,
}

/// Output of [`run_balance_analysis`]. The `*_balanced` flags answer the
/// "are all players equipped the same?" question per-category so the panel
/// can color each row independently.
#[derive(Debug, Clone, Default)]
pub struct BalanceReport {
    pub players: Vec<PlayerBalance>,
    pub total_oil: u32,
    /// Oils that ended up assigned to no player (only happens when the
    /// report has zero players, but tracked so the panel can warn).
    pub neutral_oil: u32,
    /// Voronoi assignment used for `oil_count`: oil tile -> nearest player
    /// id. The panel uses this to highlight a player's oil resources in
    /// the viewport.
    pub oil_assignment: Vec<((u32, u32), i8)>,
    pub oil_balanced: bool,
    pub structures_balanced: bool,
    pub droids_balanced: bool,
}

impl BalanceReport {
    #[must_use]
    pub fn fully_balanced(&self) -> bool {
        self.oil_balanced && self.structures_balanced && self.droids_balanced
    }
}

/// Build a [`BalanceReport`] from a loaded map.
#[must_use]
pub fn run_balance_analysis(map: &WzMap) -> BalanceReport {
    let starts = collect_player_starts(map);
    if starts.is_empty() {
        return BalanceReport {
            total_oil: count_oil(map),
            oil_balanced: true,
            structures_balanced: true,
            droids_balanced: true,
            ..BalanceReport::default()
        };
    }

    let mut player_ids: Vec<i8> = starts.keys().copied().collect();
    player_ids.sort_unstable();

    let mut per_player: HashMap<i8, PlayerBalance> = player_ids
        .iter()
        .map(|&id| {
            let info = &starts[&id];
            (
                id,
                PlayerBalance {
                    player: id,
                    start_tile: info.tile,
                    has_hq: info.has_hq,
                    oil_count: 0,
                    structure_count: 0,
                    droid_count: 0,
                    structures: BTreeMap::new(),
                },
            )
        })
        .collect();

    for s in &map.structures {
        if let Some(p) = per_player.get_mut(&s.player) {
            p.structure_count += 1;
            *p.structures.entry(s.name.clone()).or_insert(0) += 1;
        }
    }
    for d in &map.droids {
        if let Some(p) = per_player.get_mut(&d.player) {
            p.droid_count += 1;
        }
    }

    let oils = collect_oil_tiles(map);
    let total_oil = oils.len() as u32;
    let mut neutral_oil = 0u32;
    let mut oil_assignment: Vec<((u32, u32), i8)> = Vec::with_capacity(oils.len());
    for &(ox, oy) in &oils {
        let mut best: Option<(f32, i8)> = None;
        for &id in &player_ids {
            let (tx, ty) = starts[&id].tile;
            let dx = ox as f32 - tx as f32;
            let dy = oy as f32 - ty as f32;
            let d = dx.mul_add(dx, dy * dy);
            best = Some(match best {
                Some((bd, _)) if bd <= d => best.unwrap(),
                _ => (d, id),
            });
        }
        match best {
            Some((_, id)) => {
                if let Some(p) = per_player.get_mut(&id) {
                    p.oil_count += 1;
                }
                oil_assignment.push(((ox, oy), id));
            }
            None => neutral_oil += 1,
        }
    }

    let players: Vec<PlayerBalance> = player_ids
        .iter()
        .map(|id| {
            per_player
                .remove(id)
                .expect("player_ids matches per_player")
        })
        .collect();

    let oil_balanced = all_equal(players.iter().map(|p| p.oil_count));
    let droids_balanced = all_equal(players.iter().map(|p| p.droid_count));
    let structures_balanced = players
        .first()
        .is_none_or(|first| players.iter().all(|p| p.structures == first.structures));

    BalanceReport {
        players,
        total_oil,
        neutral_oil,
        oil_assignment,
        oil_balanced,
        structures_balanced,
        droids_balanced,
    }
}

fn all_equal<I: IntoIterator<Item = u32>>(iter: I) -> bool {
    let mut iter = iter.into_iter();
    let Some(first) = iter.next() else {
        return true;
    };
    iter.all(|v| v == first)
}

struct StartInfo {
    tile: (u32, u32),
    has_hq: bool,
}

fn collect_player_starts(map: &WzMap) -> HashMap<i8, StartInfo> {
    let mut by_player: HashMap<i8, StartInfo> = HashMap::new();
    for s in &map.structures {
        if s.player < 0 {
            continue;
        }
        let tile = (
            map_coord(s.position.x as i32).max(0) as u32,
            map_coord(s.position.y as i32).max(0) as u32,
        );
        let is_hq = s.name.starts_with(HQ_NAME_PREFIX);
        match by_player.get_mut(&s.player) {
            Some(info) if !info.has_hq && is_hq => {
                info.tile = tile;
                info.has_hq = true;
            }
            Some(_) => {}
            None => {
                by_player.insert(
                    s.player,
                    StartInfo {
                        tile,
                        has_hq: is_hq,
                    },
                );
            }
        }
    }
    by_player
}

fn count_oil(map: &WzMap) -> u32 {
    map.features
        .iter()
        .filter(|f| f.name.eq_ignore_ascii_case("OilResource"))
        .count() as u32
}

fn collect_oil_tiles(map: &WzMap) -> Vec<(u32, u32)> {
    map.features
        .iter()
        .filter(|f| f.name.eq_ignore_ascii_case("OilResource"))
        .map(|f| {
            (
                map_coord(f.position.x as i32).max(0) as u32,
                map_coord(f.position.y as i32).max(0) as u32,
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use wz_maplib::constants::TILE_UNITS;
    use wz_maplib::objects::{Droid, Feature, Structure, WorldPos};

    fn place_structure(map: &mut WzMap, name: &str, player: i8, tx: u32, ty: u32) {
        map.structures.push(Structure {
            name: name.into(),
            position: WorldPos {
                x: tx * TILE_UNITS + TILE_UNITS / 2,
                y: ty * TILE_UNITS + TILE_UNITS / 2,
            },
            direction: 0,
            player,
            modules: 0,
            id: None,
        });
    }

    fn place_oil(map: &mut WzMap, tx: u32, ty: u32) {
        map.features.push(Feature {
            name: "OilResource".into(),
            position: WorldPos {
                x: tx * TILE_UNITS + TILE_UNITS / 2,
                y: ty * TILE_UNITS + TILE_UNITS / 2,
            },
            direction: 0,
            id: None,
            player: None,
        });
    }

    fn place_droid(map: &mut WzMap, name: &str, player: i8, tx: u32, ty: u32) {
        map.droids.push(Droid {
            name: name.into(),
            position: WorldPos {
                x: tx * TILE_UNITS + TILE_UNITS / 2,
                y: ty * TILE_UNITS + TILE_UNITS / 2,
            },
            direction: 0,
            player,
            id: None,
        });
    }

    #[test]
    fn empty_map_is_trivially_balanced() {
        let map = WzMap::new("t", 16, 16);
        let report = run_balance_analysis(&map);
        assert!(report.players.is_empty());
        assert!(report.fully_balanced());
        assert_eq!(report.total_oil, 0);
    }

    #[test]
    fn voronoi_assigns_oil_to_nearest_player() {
        let mut map = WzMap::new("t", 32, 32);
        place_structure(&mut map, "A0CommandCentre", 0, 4, 4);
        place_structure(&mut map, "A0CommandCentre", 1, 28, 28);
        place_oil(&mut map, 5, 5);
        place_oil(&mut map, 6, 6);
        place_oil(&mut map, 27, 27);

        let report = run_balance_analysis(&map);
        let p0 = report.players.iter().find(|p| p.player == 0).unwrap();
        let p1 = report.players.iter().find(|p| p.player == 1).unwrap();
        assert_eq!(p0.oil_count, 2);
        assert_eq!(p1.oil_count, 1);
        assert_eq!(report.total_oil, 3);
        assert!(!report.oil_balanced);
        assert_eq!(report.oil_assignment.len(), 3);
        assert_eq!(report.oil_assignment[0], ((5, 5), 0));
        assert_eq!(report.oil_assignment[2], ((27, 27), 1));
    }

    #[test]
    fn equal_layout_is_balanced() {
        let mut map = WzMap::new("t", 32, 32);
        place_structure(&mut map, "A0CommandCentre", 0, 4, 4);
        place_structure(&mut map, "A0PowerGenerator", 0, 5, 4);
        place_structure(&mut map, "A0CommandCentre", 1, 28, 28);
        place_structure(&mut map, "A0PowerGenerator", 1, 27, 28);
        place_oil(&mut map, 5, 5);
        place_oil(&mut map, 27, 27);

        let report = run_balance_analysis(&map);
        assert!(report.fully_balanced());
        assert!(report.oil_balanced);
        assert!(report.structures_balanced);
        assert!(report.droids_balanced);
    }

    #[test]
    fn structure_mismatch_breaks_structure_balance() {
        let mut map = WzMap::new("t", 32, 32);
        place_structure(&mut map, "A0CommandCentre", 0, 4, 4);
        place_structure(&mut map, "A0PowerGenerator", 0, 5, 4);
        place_structure(&mut map, "A0CommandCentre", 1, 28, 28);

        let report = run_balance_analysis(&map);
        assert!(!report.structures_balanced);
        assert!(report.oil_balanced);
    }

    #[test]
    fn droid_count_is_per_player() {
        let mut map = WzMap::new("t", 32, 32);
        place_structure(&mut map, "A0CommandCentre", 0, 4, 4);
        place_structure(&mut map, "A0CommandCentre", 1, 28, 28);
        place_droid(&mut map, "ConstructionDroid", 0, 4, 5);
        place_droid(&mut map, "ConstructionDroid", 0, 4, 6);
        place_droid(&mut map, "ConstructionDroid", 1, 28, 27);

        let report = run_balance_analysis(&map);
        let p0 = report.players.iter().find(|p| p.player == 0).unwrap();
        let p1 = report.players.iter().find(|p| p.player == 1).unwrap();
        assert_eq!(p0.droid_count, 2);
        assert_eq!(p1.droid_count, 1);
        assert!(!report.droids_balanced);
    }

    #[test]
    fn hq_is_preferred_start_tile() {
        let mut map = WzMap::new("t", 32, 32);
        place_structure(&mut map, "Factory", 0, 5, 5);
        place_structure(&mut map, "A0CommandCentre", 0, 9, 9);
        let report = run_balance_analysis(&map);
        let p0 = &report.players[0];
        assert_eq!(p0.start_tile, (9, 9));
        assert!(p0.has_hq);
    }
}
