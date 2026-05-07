//! Height level assignment, water designation, and ramp carving.

use std::collections::VecDeque;

use super::GeneratorConfig;
use super::nodes::NodeNetwork;

#[derive(Debug, Clone)]
pub struct LevelAssignment {
    /// Height level per node (0..levels-1).
    pub levels: Vec<i8>,
    pub water: Vec<bool>,
}

/// Assign height levels to all nodes via BFS from player bases. `level_frequency`
/// controls how often adjacent nodes change level; mirrored nodes share a level
/// to preserve map symmetry.
pub(crate) fn assign_height_levels(
    network: &NodeNetwork,
    config: &GeneratorConfig,
    rng: &mut fastrand::Rng,
) -> LevelAssignment {
    let n = network.nodes.len();
    let max_level = (config.height_levels as i8) - 1;

    let mut levels = vec![-1i8; n];
    let mut water = vec![false; n];
    let mut visited = vec![false; n];

    let base_level = if config.base_level < 0 {
        rng.i8(0..=max_level)
    } else {
        config.base_level.min(max_level)
    };

    let mut queue = VecDeque::new();
    for &pi in &network.player_nodes {
        if pi < n {
            levels[pi] = base_level;
            visited[pi] = true;
            queue.push_back(pi);
        }
    }

    if queue.is_empty() {
        let center = network
            .node_at(network.grid_w / 2, network.grid_h / 2)
            .unwrap_or(0);
        levels[center] = base_level;
        visited[center] = true;
        queue.push_back(center);
    }

    // Index into neighbors inside the loop to avoid cloning; the mutable borrows
    // (levels, visited, queue) don't overlap with `network`.
    while let Some(current) = queue.pop_front() {
        let current_level = levels[current];
        let neighbor_count = network.nodes[current].neighbors.len();

        for idx in 0..neighbor_count {
            let ni = network.nodes[current].neighbors[idx];
            if visited[ni] {
                continue;
            }

            let new_level = if rng.f32() < config.level_frequency {
                let delta = if rng.bool() { 1 } else { -1 };
                (current_level + delta).clamp(0, max_level)
            } else {
                current_level
            };

            levels[ni] = new_level;
            visited[ni] = true;
            queue.push_back(ni);

            assign_symmetric(
                ni,
                new_level,
                network,
                config,
                &mut levels,
                &mut visited,
                &mut queue,
            );
        }
    }

    for level in &mut levels {
        if *level < 0 {
            *level = rng.i8(0..=max_level);
        }
    }

    assign_water(network, config, &levels, &mut water, rng);

    LevelAssignment { levels, water }
}

fn assign_symmetric(
    node_idx: usize,
    level: i8,
    network: &NodeNetwork,
    config: &GeneratorConfig,
    levels: &mut [i8],
    visited: &mut [bool],
    queue: &mut VecDeque<usize>,
) {
    let node = &network.nodes[node_idx];
    let mirror_pts = crate::tools::mirror::mirror_points(
        node.gx,
        node.gy,
        network.grid_w,
        network.grid_h,
        config.symmetry,
    );

    for &(mx, my) in &mirror_pts {
        if let Some(mi) = network.node_at(mx, my)
            && !visited[mi]
        {
            levels[mi] = level;
            visited[mi] = true;
            queue.push_back(mi);
        }
    }
}

fn assign_water(
    network: &NodeNetwork,
    config: &GeneratorConfig,
    levels: &[i8],
    water: &mut [bool],
    rng: &mut fastrand::Rng,
) {
    if config.water_spawns == 0 {
        return;
    }

    let candidates: Vec<usize> = (0..network.nodes.len())
        .filter(|&i| levels[i] == 0 && !network.nodes[i].is_border && network.nodes[i].player < 0)
        .collect();

    if candidates.is_empty() {
        return;
    }

    let mut seeds = Vec::new();
    let mut used = vec![false; network.nodes.len()];

    for _ in 0..config.water_spawns {
        let available: Vec<usize> = candidates.iter().copied().filter(|&i| !used[i]).collect();
        if available.is_empty() {
            break;
        }
        let seed = available[rng.usize(..available.len())];
        water[seed] = true;
        used[seed] = true;
        seeds.push(seed);

        let node = &network.nodes[seed];
        let mirror_pts = crate::tools::mirror::mirror_points(
            node.gx,
            node.gy,
            network.grid_w,
            network.grid_h,
            config.symmetry,
        );
        for &(mx, my) in &mirror_pts {
            if let Some(mi) = network.node_at(mx, my) {
                water[mi] = true;
                used[mi] = true;
            }
        }
    }

    let mut expand_queue: VecDeque<usize> = seeds.into();
    // Cap water at 25% of nodes so it never overruns the playable area.
    let max_water = (network.nodes.len() / 4).max(1);
    let mut water_count = expand_queue.len();

    while let Some(current) = expand_queue.pop_front() {
        if water_count >= max_water {
            break;
        }
        for &ni in &network.nodes[current].neighbors {
            if water[ni] || levels[ni] != 0 || network.nodes[ni].player >= 0 {
                continue;
            }
            if rng.bool() {
                water[ni] = true;
                water_count += 1;
                expand_queue.push_back(ni);

                let node = &network.nodes[ni];
                let mirror_pts = crate::tools::mirror::mirror_points(
                    node.gx,
                    node.gy,
                    network.grid_w,
                    network.grid_h,
                    config.symmetry,
                );
                for &(mx, my) in &mirror_pts {
                    if let Some(mi) = network.node_at(mx, my)
                        && !water[mi]
                    {
                        water[mi] = true;
                        water_count += 1;
                    }
                }
            }
        }
    }
}

/// Carve smooth ramps in-place between adjacent nodes that differ by exactly one level.
pub(crate) fn carve_ramps(
    heights: &mut [f32],
    map_width: u32,
    map_height: u32,
    network: &NodeNetwork,
    levels: &LevelAssignment,
    config: &GeneratorConfig,
) {
    let level_height =
        wz_maplib::constants::TILE_MAX_HEIGHT as f32 / (config.height_levels as f32 - 1.0).max(1.0);

    for (i, node) in network.nodes.iter().enumerate() {
        for &ni in &node.neighbors {
            if ni <= i {
                continue;
            }
            let level_diff = (levels.levels[i] - levels.levels[ni]).abs();
            if level_diff != 1 {
                continue;
            }

            let neighbor = &network.nodes[ni];
            carve_ramp_between(
                heights,
                map_width,
                map_height,
                node.tile_x,
                node.tile_y,
                neighbor.tile_x,
                neighbor.tile_y,
                levels.levels[i] as f32 * level_height,
                levels.levels[ni] as f32 * level_height,
            );
        }
    }
}

fn carve_ramp_between(
    heights: &mut [f32],
    map_width: u32,
    map_height: u32,
    x0: u32,
    y0: u32,
    x1: u32,
    y1: u32,
    h0: f32,
    h1: f32,
) {
    let dx = x1 as f32 - x0 as f32;
    let dy = y1 as f32 - y0 as f32;
    let dist = (dx * dx + dy * dy).sqrt();
    if dist < 1.0 {
        return;
    }

    let steps = dist.ceil() as u32;
    // Half-width of the ramp corridor in tiles.
    let ramp_width = 3u32;

    for step in 0..=steps {
        let t = step as f32 / steps as f32;
        let cx = x0 as f32 + dx * t;
        let cy = y0 as f32 + dy * t;
        let target_h = h0 + (h1 - h0) * t;

        for oy in -(ramp_width as i32)..=(ramp_width as i32) {
            for ox in -(ramp_width as i32)..=(ramp_width as i32) {
                let tx = (cx as i32 + ox).clamp(0, map_width as i32 - 1) as u32;
                let ty = (cy as i32 + oy).clamp(0, map_height as i32 - 1) as u32;

                let idx = (ty * map_width + tx) as usize;
                if idx < heights.len() {
                    let d = ((ox * ox + oy * oy) as f32).sqrt();
                    let blend = 1.0 - (d / (ramp_width as f32 + 1.0)).min(1.0);
                    let blend = blend * blend;
                    heights[idx] = heights[idx] * (1.0 - blend) + target_h * blend;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generator::nodes::build_node_network;
    use crate::tools::MirrorMode;

    fn default_config() -> GeneratorConfig {
        GeneratorConfig::default()
    }

    #[test]
    fn test_level_assignment_all_valid() {
        let config = GeneratorConfig {
            height_levels: 4,
            ..default_config()
        };
        let mut rng = fastrand::Rng::with_seed(42);
        let net = build_node_network(&config, &mut rng);
        let la = assign_height_levels(&net, &config, &mut rng);

        let max_level = config.height_levels as i8 - 1;
        for (i, &level) in la.levels.iter().enumerate() {
            assert!(
                level >= 0 && level <= max_level,
                "Node {i} has level {level}, expected 0..={max_level}"
            );
        }
    }

    #[test]
    fn test_level_assignment_count_matches_nodes() {
        let config = default_config();
        let mut rng = fastrand::Rng::with_seed(42);
        let net = build_node_network(&config, &mut rng);
        let la = assign_height_levels(&net, &config, &mut rng);

        assert_eq!(la.levels.len(), net.nodes.len());
        assert_eq!(la.water.len(), net.nodes.len());
    }

    #[test]
    fn test_player_bases_at_base_level() {
        let config = GeneratorConfig {
            base_level: 2,
            height_levels: 5,
            ..default_config()
        };
        let mut rng = fastrand::Rng::with_seed(42);
        let net = build_node_network(&config, &mut rng);
        let la = assign_height_levels(&net, &config, &mut rng);

        for &pi in &net.player_nodes {
            assert_eq!(la.levels[pi], 2, "Player node {pi} not at base level 2");
        }
    }

    #[test]
    fn test_water_not_at_player_bases() {
        let config = GeneratorConfig {
            water_spawns: 3,
            ..default_config()
        };
        let mut rng = fastrand::Rng::with_seed(42);
        let net = build_node_network(&config, &mut rng);
        let la = assign_height_levels(&net, &config, &mut rng);

        for &pi in &net.player_nodes {
            assert!(!la.water[pi], "Player node {pi} should not be water");
        }
    }

    #[test]
    fn test_no_water_when_spawns_zero() {
        let config = GeneratorConfig {
            water_spawns: 0,
            ..default_config()
        };
        let mut rng = fastrand::Rng::with_seed(42);
        let net = build_node_network(&config, &mut rng);
        let la = assign_height_levels(&net, &config, &mut rng);

        assert!(la.water.iter().all(|&w| !w), "No water expected");
    }

    #[test]
    fn test_symmetric_levels_vertical() {
        let config = GeneratorConfig {
            width: 128,
            height: 128,
            symmetry: MirrorMode::Vertical,
            ..default_config()
        };
        let mut rng = fastrand::Rng::with_seed(42);
        let net = build_node_network(&config, &mut rng);
        let la = assign_height_levels(&net, &config, &mut rng);

        for node in &net.nodes {
            let mirror_x = net.grid_w.saturating_sub(1).saturating_sub(node.gx);
            if let Some(mi) = net.node_at(mirror_x, node.gy) {
                let my_idx = net.node_at(node.gx, node.gy).unwrap();
                assert_eq!(
                    la.levels[my_idx], la.levels[mi],
                    "Node ({},{}) level {} != mirror ({},{}) level {}",
                    node.gx, node.gy, la.levels[my_idx], mirror_x, node.gy, la.levels[mi]
                );
            }
        }
    }

    #[test]
    fn test_ramp_carving_changes_heights() {
        let config = GeneratorConfig {
            height_levels: 3,
            level_frequency: 1.0,
            ..default_config()
        };
        let mut rng = fastrand::Rng::with_seed(42);
        let net = build_node_network(&config, &mut rng);
        let la = assign_height_levels(&net, &config, &mut rng);

        let size = (config.width * config.height) as usize;
        let mut heights = vec![0.0f32; size];

        let level_h = 510.0 / 2.0;
        for node in &net.nodes {
            let idx = (node.tile_y * config.width + node.tile_x) as usize;
            if idx < size {
                let node_idx = net.node_at(node.gx, node.gy).unwrap();
                heights[idx] = la.levels[node_idx] as f32 * level_h;
            }
        }

        let before = heights.clone();
        carve_ramps(
            &mut heights,
            config.width,
            config.height,
            &net,
            &la,
            &config,
        );

        let changed = heights
            .iter()
            .zip(before.iter())
            .filter(|(a, b)| (*a - *b).abs() > 0.01)
            .count();
        assert!(changed > 0, "Ramp carving should have changed some heights");
    }

    #[test]
    fn test_level_assignment_deterministic() {
        let config = default_config();

        let mut rng1 = fastrand::Rng::with_seed(99);
        let net1 = build_node_network(&config, &mut rng1);
        let la1 = assign_height_levels(&net1, &config, &mut rng1);

        let mut rng2 = fastrand::Rng::with_seed(99);
        let net2 = build_node_network(&config, &mut rng2);
        let la2 = assign_height_levels(&net2, &config, &mut rng2);

        assert_eq!(la1.levels, la2.levels);
        assert_eq!(la1.water, la2.water);
    }
}
