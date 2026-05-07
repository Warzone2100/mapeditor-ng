//! Heightmap generation: multi-octave Perlin with domain warping, ridged
//! multi-fractal mountains, and particle-based hydraulic/thermal erosion.
//! Node levels from the passage network act as soft control points so the
//! resulting terrain still respects gameplay layout.

use noise::{Fbm, MultiFractal, NoiseFn, Perlin, RidgedMulti};

use super::GeneratorConfig;
use super::nodes::NodeNetwork;
use super::terrain::LevelAssignment;

/// XOR-fold a u64 seed to u32 so upper bits still influence the output. The
/// `noise` crate only takes u32, and a plain cast would collide every pair of
/// seeds sharing the low 32 bits.
#[inline]
fn fold_seed_u32(seed: u64) -> u32 {
    ((seed ^ (seed >> 32)) & 0xFFFF_FFFF) as u32
}

/// 2D heightmap, one f32 per tile, matching `MapTile.height` layout.
#[derive(Debug, Clone)]
pub struct Heightmap {
    pub width: u32,
    pub height: u32,
    pub data: Vec<f32>,
}

impl Heightmap {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            data: vec![0.0; (width * height) as usize],
        }
    }

    /// Height at (x, y), with coordinates clamped to bounds.
    pub fn get(&self, x: u32, y: u32) -> f32 {
        let x = x.min(self.width.saturating_sub(1));
        let y = y.min(self.height.saturating_sub(1));
        self.data[(y * self.width + x) as usize]
    }

    pub fn set(&mut self, x: u32, y: u32, val: f32) {
        if x < self.width && y < self.height {
            self.data[(y * self.width + x) as usize] = val;
        }
    }
}

/// Generate a heightmap by combining node-driven plateaus, fractal noise, and
/// light erosion. Real WZ2100 maps use the full 0-510 range with distinct
/// plateau/cliff/basin tiers, so the pipeline is:
///
/// 1. Voronoi-snap each tile to its nearest node level (sharp tier boundaries).
/// 2. Add large-amplitude fractal noise for variation within each tier.
/// 3. Quantize back toward tier centers (flatness controls how strongly).
/// 4. Light hydraulic erosion for drainage channels.
/// 5. Conservative thermal erosion that keeps cliffs intact.
pub(crate) fn generate(
    config: &GeneratorConfig,
    network: &NodeNetwork,
    levels: &LevelAssignment,
    rng: &mut fastrand::Rng,
) -> Heightmap {
    let max_height = wz_maplib::constants::TILE_MAX_HEIGHT as f32;
    let level_height = max_height / (config.height_levels as f32 - 1.0).max(1.0);
    let seed = if config.seed == 0 {
        rng.u64(..)
    } else {
        config.seed
    };

    let mut heightmap = Heightmap::new(config.width, config.height);

    build_plateau_field(&mut heightmap, network, levels, level_height);
    add_noise_variation(&mut heightmap, config, seed);
    apply_level_quantization(&mut heightmap, config, level_height);

    // Apply water before erosion so drainage channels respect it.
    apply_water(&mut heightmap, network, levels, config);

    let drop_count = erosion_drop_count(config);
    if drop_count > 0 {
        hydraulic_erosion(&mut heightmap, drop_count, fold_seed_u32(seed));
    }

    // Talus 120 (of TILE_MAX_HEIGHT=510) leaves cliffs up to ~25% of max height
    // intact. Lower values (30-40) flatten the plateau-and-cliff look.
    thermal_erosion(&mut heightmap, 1, 120.0);

    // Re-apply so erosion sediment doesn't refill water zones.
    apply_water(&mut heightmap, network, levels, config);

    for h in &mut heightmap.data {
        *h = h.clamp(0.0, max_height);
    }

    heightmap
}

/// Erosion droplet count, deliberately lower than heavy-erosion presets so
/// cliffs and plateaus survive. Scales with map area and with `flatness` so
/// the user can ask for extra smoothing.
fn erosion_drop_count(config: &GeneratorConfig) -> u32 {
    let area_ratio = (config.width as f32 * config.height as f32) / (128.0 * 128.0);
    let base = 1500.0 + config.flatness * 5000.0;
    (base * area_ratio.sqrt()) as u32
}

/// Build the plateau field from node levels via Voronoi assignment, blending
/// only inside a narrow band at cell edges where two different-level nodes are
/// nearly equidistant. Sharp boundaries are the visual hallmark of WZ2100 maps.
fn build_plateau_field(
    heightmap: &mut Heightmap,
    network: &NodeNetwork,
    levels: &LevelAssignment,
    level_height: f32,
) {
    // Narrow band gives cliff-like boundaries; erosion softens them later.
    let transition = 0.08_f64;

    for y in 0..heightmap.height {
        for x in 0..heightmap.width {
            let mut best = (f64::MAX, 0.0_f64);
            let mut second = (f64::MAX, 0.0_f64);

            for (i, node) in network.nodes.iter().enumerate() {
                let dx = f64::from(x) - f64::from(node.tile_x);
                let dy = f64::from(y) - f64::from(node.tile_y);
                let d = dx * dx + dy * dy;

                let target = if levels.water[i] {
                    0.0
                } else {
                    f64::from(levels.levels[i]) * f64::from(level_height)
                };

                if d < best.0 {
                    second = best;
                    best = (d, target);
                } else if d < second.0 {
                    second = (d, target);
                }
            }

            let best_d = best.0.sqrt();
            let second_d = second.0.sqrt();

            // 1 = well inside best node's cell, 0 = at the Voronoi boundary.
            let margin = if second_d > 0.0 {
                ((second_d - best_d) / second_d).clamp(0.0, 1.0)
            } else {
                1.0
            };

            let h = if margin >= transition || (best.1 - second.1).abs() < 1.0 {
                best.1
            } else {
                // Cubic Hermite blend across the transition band.
                let t = (margin / transition).clamp(0.0, 1.0);
                let smooth = t * t * (3.0 - 2.0 * t);
                second.1 * (1.0 - smooth) + best.1 * smooth
            };

            heightmap.set(x, y, h as f32);
        }
    }
}

/// Pull heights toward the nearest discrete level tier. Higher `flatness`
/// gives more distinct plateaus; this is what produces the multi-modal height
/// distribution seen in real WZ2100 maps.
fn apply_level_quantization(
    heightmap: &mut Heightmap,
    config: &GeneratorConfig,
    level_height: f32,
) {
    if config.flatness <= 0.0 || level_height <= 0.0 {
        return;
    }
    let max_height = wz_maplib::constants::TILE_MAX_HEIGHT as f32;
    // Cap pull at 0.7 so even flatness=1 keeps some noise variation.
    let pull = config.flatness * 0.7;
    let max_levels = (config.height_levels as f32 - 1.0).max(1.0);

    for h in &mut heightmap.data {
        let level_f = (*h / level_height).clamp(0.0, max_levels);
        let target_level = level_f.round();
        let target = (target_level * level_height).clamp(0.0, max_height);
        *h = *h * (1.0 - pull) + target * pull;
    }
}

/// Domain-warped fBm continents + ridged multi-fractal mountains + fine detail.
fn add_noise_variation(heightmap: &mut Heightmap, config: &GeneratorConfig, seed: u64) {
    // Constants tuned by matching generator stats against real WZ2100 maps.
    // Continental fBm carries the silhouette, ridges drive mountain crests at
    // higher elevations, detail adds small-scale variation.
    const NOISE_AMPLITUDE_FRAC: f32 = 0.55;
    const WARP_STRENGTH: f64 = 0.35;
    const RIDGE_HEIGHT_WEIGHT: f32 = 0.35;
    const RIDGE_SCALE: f64 = 0.8;
    const DETAIL_SCALE: f64 = 1.5;
    const WEIGHT_CONTINENT: f32 = 0.55;
    const WEIGHT_DETAIL: f32 = 0.10;

    let max_height = wz_maplib::constants::TILE_MAX_HEIGHT as f32;
    // Amplitude must let noise cross plateau boundaries for natural-looking
    // features, since real maps use the full height range.
    let amplitude = max_height * NOISE_AMPLITUDE_FRAC * config.height_variation;
    if amplitude < 1.0 {
        return;
    }

    let s = fold_seed_u32(seed);

    let continent: Fbm<Perlin> = Fbm::new(s)
        .set_octaves(6)
        .set_frequency(1.0)
        .set_lacunarity(2.2)
        .set_persistence(0.5);

    // Domain-warp noise breaks up grid alignment.
    let warp_x: Fbm<Perlin> = Fbm::new(s.wrapping_add(100))
        .set_octaves(3)
        .set_frequency(1.5);
    let warp_y: Fbm<Perlin> = Fbm::new(s.wrapping_add(200))
        .set_octaves(3)
        .set_frequency(1.5);

    let ridges: RidgedMulti<Perlin> = RidgedMulti::new(s.wrapping_add(300))
        .set_octaves(4)
        .set_frequency(2.0)
        .set_lacunarity(2.1);

    let detail: Fbm<Perlin> = Fbm::new(s.wrapping_add(400))
        .set_octaves(4)
        .set_frequency(5.0)
        .set_persistence(0.4);

    // Scale tile coords so feature size grows with the map.
    let map_extent = f64::from(config.width.max(config.height));
    let scale = 4.0 / map_extent;

    for y in 0..heightmap.height {
        for x in 0..heightmap.width {
            let nx = f64::from(x) * scale;
            let ny = f64::from(y) * scale;

            let wx = nx + warp_x.get([nx, ny]) * WARP_STRENGTH;
            let wy = ny + warp_y.get([nx, ny]) * WARP_STRENGTH;

            let c = continent.get([wx, wy]);

            // Ridges contribute more at higher elevations to form mountain crests.
            let current = heightmap.get(x, y);
            let height_frac = (current / max_height).clamp(0.0, 1.0);
            let ridge_weight = height_frac * RIDGE_HEIGHT_WEIGHT;
            let r = ridges.get([wx * RIDGE_SCALE, wy * RIDGE_SCALE]);

            let d = detail.get([nx * DETAIL_SCALE, ny * DETAIL_SCALE]);

            let noise =
                c as f32 * WEIGHT_CONTINENT + r as f32 * ridge_weight + d as f32 * WEIGHT_DETAIL;

            heightmap.set(x, y, current + noise * amplitude);
        }
    }
}

/// Particle-based hydraulic erosion (Hans Theobald Beyer's algorithm). Simulates
/// droplets that erode and deposit sediment to create drainage channels and
/// alluvial fans.
fn hydraulic_erosion(heightmap: &mut Heightmap, num_drops: u32, seed: u32) {
    const INERTIA: f32 = 0.05;
    const CAPACITY: f32 = 4.0;
    const DEPOSITION: f32 = 0.3;
    const EROSION: f32 = 0.3;
    const EVAPORATION: f32 = 0.01;
    const MIN_SLOPE: f32 = 0.01;
    const GRAVITY: f32 = 4.0;
    const MAX_LIFE: u32 = 64;
    const BRUSH_RADIUS: f32 = 3.0;

    let w = heightmap.width;
    let h = heightmap.height;
    let mut rng = fastrand::Rng::with_seed(u64::from(seed) ^ 0xE205_1024);

    for _ in 0..num_drops {
        let mut px = rng.f32() * (w as f32 - 4.0) + 2.0;
        let mut py = rng.f32() * (h as f32 - 4.0) + 2.0;
        let mut dx = 0.0_f32;
        let mut dy = 0.0_f32;
        let mut speed = 1.0_f32;
        let mut water = 1.0_f32;
        let mut sediment = 0.0_f32;

        for _ in 0..MAX_LIFE {
            if px < 1.0 || py < 1.0 || px >= (w - 2) as f32 || py >= (h - 2) as f32 {
                break;
            }

            let (gx, gy) = gradient_at(heightmap, px, py);

            // Inertia preserves heading; gradient pulls downhill.
            dx = dx * INERTIA - gx * (1.0 - INERTIA);
            dy = dy * INERTIA - gy * (1.0 - INERTIA);

            let len = (dx * dx + dy * dy).sqrt();
            if len < 1e-4 {
                // Pick a random heading on flat terrain to avoid getting stuck.
                let a = rng.f32() * std::f32::consts::TAU;
                dx = a.cos();
                dy = a.sin();
            } else {
                dx /= len;
                dy /= len;
            }

            let npx = px + dx;
            let npy = py + dy;

            if npx < 1.0 || npy < 1.0 || npx >= (w - 2) as f32 || npy >= (h - 2) as f32 {
                break;
            }

            let old_h = interpolated_height(heightmap, px, py);
            let new_h = interpolated_height(heightmap, npx, npy);
            let h_diff = new_h - old_h;

            // Carrying capacity grows with slope, speed, and water volume.
            let slope = (-h_diff).max(MIN_SLOPE);
            let cap = slope * speed * water * CAPACITY;

            if sediment > cap || h_diff > 0.0 {
                let amount = if h_diff > 0.0 {
                    sediment.min(h_diff)
                } else {
                    (sediment - cap) * DEPOSITION
                };
                deposit_sediment(heightmap, px, py, amount);
                sediment -= amount;
            } else {
                let amount = ((cap - sediment) * EROSION).min(-h_diff);
                erode_terrain(heightmap, px, py, amount, BRUSH_RADIUS);
                sediment += amount;
            }

            speed = (speed * speed + h_diff * GRAVITY).max(0.01).sqrt();
            water *= 1.0 - EVAPORATION;

            px = npx;
            py = npy;
        }
    }
}

fn interpolated_height(hm: &Heightmap, x: f32, y: f32) -> f32 {
    let ix = x.floor() as u32;
    let iy = y.floor() as u32;
    let fx = x - ix as f32;
    let fy = y - iy as f32;

    let h00 = hm.get(ix, iy);
    let h10 = hm.get(ix + 1, iy);
    let h01 = hm.get(ix, iy + 1);
    let h11 = hm.get(ix + 1, iy + 1);

    h00 * (1.0 - fx) * (1.0 - fy) + h10 * fx * (1.0 - fy) + h01 * (1.0 - fx) * fy + h11 * fx * fy
}

fn gradient_at(hm: &Heightmap, x: f32, y: f32) -> (f32, f32) {
    let ix = x.floor() as u32;
    let iy = y.floor() as u32;
    let fx = x - ix as f32;
    let fy = y - iy as f32;

    let h00 = hm.get(ix, iy);
    let h10 = hm.get(ix + 1, iy);
    let h01 = hm.get(ix, iy + 1);
    let h11 = hm.get(ix + 1, iy + 1);

    let gx = (h10 - h00) * (1.0 - fy) + (h11 - h01) * fy;
    let gy = (h01 - h00) * (1.0 - fx) + (h11 - h10) * fx;

    (gx, gy)
}

fn deposit_sediment(hm: &mut Heightmap, x: f32, y: f32, amount: f32) {
    let ix = x.floor() as u32;
    let iy = y.floor() as u32;
    let fx = x - ix as f32;
    let fy = y - iy as f32;

    add_height(hm, ix, iy, amount * (1.0 - fx) * (1.0 - fy));
    add_height(hm, ix + 1, iy, amount * fx * (1.0 - fy));
    add_height(hm, ix, iy + 1, amount * (1.0 - fx) * fy);
    add_height(hm, ix + 1, iy + 1, amount * fx * fy);
}

fn erode_terrain(hm: &mut Heightmap, x: f32, y: f32, amount: f32, radius: f32) {
    let cx = x.round() as i32;
    let cy = y.round() as i32;
    let r = radius.ceil() as i32;

    let mut weights: Vec<(u32, u32, f32)> = Vec::new();
    let mut total = 0.0_f32;

    for bdy in -r..=r {
        for bdx in -r..=r {
            let dist = ((bdx * bdx + bdy * bdy) as f32).sqrt();
            if dist <= radius {
                let px = cx + bdx;
                let py = cy + bdy;
                if px >= 0 && py >= 0 && (px as u32) < hm.width && (py as u32) < hm.height {
                    let w = (1.0 - dist / radius).powi(2);
                    weights.push((px as u32, py as u32, w));
                    total += w;
                }
            }
        }
    }

    if total > 0.0 {
        for &(px, py, w) in &weights {
            add_height(hm, px, py, -amount * w / total);
        }
    }
}

fn add_height(hm: &mut Heightmap, x: u32, y: u32, delta: f32) {
    if x < hm.width && y < hm.height {
        let idx = (y * hm.width + x) as usize;
        hm.data[idx] += delta;
    }
}

/// Thermal erosion: material above the `talus` per-tile drop transfers to
/// lower neighbors, softening overhangs while keeping cliffs intact at high
/// talus values.
fn thermal_erosion(heightmap: &mut Heightmap, iterations: u32, talus: f32) {
    let transfer = 0.4_f32;
    let w = heightmap.width;
    let h = heightmap.height;

    for _ in 0..iterations {
        let snap = heightmap.data.clone();

        for y in 1..h - 1 {
            for x in 1..w - 1 {
                let idx = (y * w + x) as usize;
                let center = snap[idx];

                let neighbors = [
                    (y * w + x - 1) as usize,
                    (y * w + x + 1) as usize,
                    ((y - 1) * w + x) as usize,
                    ((y + 1) * w + x) as usize,
                ];

                let mut max_diff = 0.0_f32;
                let mut total_excess = 0.0_f32;

                for &ni in &neighbors {
                    let diff = center - snap[ni];
                    if diff > talus {
                        total_excess += diff - talus;
                        max_diff = max_diff.max(diff);
                    }
                }

                if total_excess <= 0.0 {
                    continue;
                }

                for &ni in &neighbors {
                    let diff = center - snap[ni];
                    if diff > talus {
                        let frac = (diff - talus) / total_excess;
                        let amount = (max_diff - talus) * transfer * frac * 0.5;
                        heightmap.data[idx] -= amount;
                        heightmap.data[ni] += amount;
                    }
                }
            }
        }
    }
}

/// Drop water regions to near-zero height with cubic-faded shoreline.
fn apply_water(
    heightmap: &mut Heightmap,
    network: &NodeNetwork,
    levels: &LevelAssignment,
    config: &GeneratorConfig,
) {
    let water_radius = network.node_spacing * 3 / 4;

    for (i, node) in network.nodes.iter().enumerate() {
        if !levels.water[i] {
            continue;
        }

        let r = water_radius as i32;
        for wdy in -r..=r {
            for wdx in -r..=r {
                let tx = node.tile_x as i32 + wdx;
                let ty = node.tile_y as i32 + wdy;
                if tx < 0 || ty < 0 {
                    continue;
                }
                let tx = tx as u32;
                let ty = ty as u32;
                if tx >= config.width || ty >= config.height {
                    continue;
                }

                let dist = ((wdx * wdx + wdy * wdy) as f32).sqrt();
                let t = (dist / r as f32).min(1.0);
                let fade = t * t * (3.0 - 2.0 * t);
                let current = heightmap.get(tx, ty);
                heightmap.set(tx, ty, current * fade);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generator::nodes::build_node_network;
    use crate::generator::terrain::assign_height_levels;

    fn default_config() -> GeneratorConfig {
        GeneratorConfig::default()
    }

    #[test]
    fn test_heightmap_dimensions() {
        let config = GeneratorConfig {
            width: 64,
            height: 96,
            ..default_config()
        };
        let mut rng = fastrand::Rng::with_seed(42);
        let net = build_node_network(&config, &mut rng);
        let la = assign_height_levels(&net, &config, &mut rng);
        let hm = generate(&config, &net, &la, &mut rng);

        assert_eq!(hm.width, 64);
        assert_eq!(hm.height, 96);
        assert_eq!(hm.data.len(), 64 * 96);
    }

    #[test]
    fn test_heights_within_range() {
        let config = default_config();
        let mut rng = fastrand::Rng::with_seed(42);
        let net = build_node_network(&config, &mut rng);
        let la = assign_height_levels(&net, &config, &mut rng);
        let hm = generate(&config, &net, &la, &mut rng);

        let max = wz_maplib::constants::TILE_MAX_HEIGHT as f32;
        for (i, &h) in hm.data.iter().enumerate() {
            assert!(
                (0.0..=max).contains(&h),
                "Height at index {i} is {h}, expected 0..={max}"
            );
        }
    }

    #[test]
    fn test_deterministic_generation() {
        let config = default_config();

        let mut rng1 = fastrand::Rng::with_seed(77);
        let net1 = build_node_network(&config, &mut rng1);
        let la1 = assign_height_levels(&net1, &config, &mut rng1);
        let hm1 = generate(&config, &net1, &la1, &mut rng1);

        let mut rng2 = fastrand::Rng::with_seed(77);
        let net2 = build_node_network(&config, &mut rng2);
        let la2 = assign_height_levels(&net2, &config, &mut rng2);
        let hm2 = generate(&config, &net2, &la2, &mut rng2);

        assert_eq!(hm1.data.len(), hm2.data.len());
        for i in 0..hm1.data.len() {
            assert!(
                (hm1.data[i] - hm2.data[i]).abs() < 0.001,
                "Mismatch at index {i}: {} vs {}",
                hm1.data[i],
                hm2.data[i]
            );
        }
    }

    #[test]
    fn test_water_regions_near_zero() {
        let config = GeneratorConfig {
            water_spawns: 3,
            ..default_config()
        };
        let mut rng = fastrand::Rng::with_seed(42);
        let net = build_node_network(&config, &mut rng);
        let la = assign_height_levels(&net, &config, &mut rng);
        let hm = generate(&config, &net, &la, &mut rng);

        for (i, node) in net.nodes.iter().enumerate() {
            if la.water[i] {
                let h = hm.get(node.tile_x, node.tile_y);
                assert!(
                    h < 10.0,
                    "Water node ({},{}) has height {h}, expected near 0",
                    node.tile_x,
                    node.tile_y
                );
            }
        }
    }

    #[test]
    fn test_flat_map_with_no_variation() {
        let config = GeneratorConfig {
            height_variation: 0.0,
            flatness: 1.0,
            height_levels: 3,
            level_frequency: 0.0,
            ..default_config()
        };
        let mut rng = fastrand::Rng::with_seed(42);
        let net = build_node_network(&config, &mut rng);
        let la = assign_height_levels(&net, &config, &mut rng);
        let hm = generate(&config, &net, &la, &mut rng);

        let mean: f32 = hm.data.iter().sum::<f32>() / hm.data.len() as f32;
        let variance: f32 =
            hm.data.iter().map(|h| (h - mean).powi(2)).sum::<f32>() / hm.data.len() as f32;
        let std_dev = variance.sqrt();

        assert!(
            std_dev < 50.0,
            "Expected low variation, got std_dev={std_dev}"
        );
    }

    #[test]
    fn test_fold_seed_u32_preserves_upper_entropy() {
        // Seeds sharing low 32 bits but differing high bits must fold differently.
        let a = fold_seed_u32(0x0000_0000_DEAD_BEEF);
        let b = fold_seed_u32(0x1234_5678_DEAD_BEEF);
        assert_ne!(a, b, "Upper 32 bits of seed should influence fold");
        assert_eq!(fold_seed_u32(42), fold_seed_u32(42));
    }

    #[test]
    fn test_heightmap_get_set() {
        let mut hm = Heightmap::new(10, 10);
        hm.set(5, 3, 42.5);
        assert!((hm.get(5, 3) - 42.5).abs() < 0.001);

        let _h = hm.get(100, 100);
    }

    #[test]
    fn test_erosion_creates_variation() {
        let config = GeneratorConfig {
            height_variation: 0.8,
            flatness: 0.5,
            ..default_config()
        };
        let mut rng = fastrand::Rng::with_seed(42);
        let net = build_node_network(&config, &mut rng);
        let la = assign_height_levels(&net, &config, &mut rng);
        let hm = generate(&config, &net, &la, &mut rng);

        let min_h = hm.data.iter().copied().fold(f32::MAX, f32::min);
        let max_h = hm.data.iter().copied().fold(f32::MIN, f32::max);
        assert!(
            max_h - min_h > 50.0,
            "Expected meaningful height range, got {min_h}..{max_h}"
        );
    }

    #[test]
    fn test_node_field_respects_levels() {
        let config = GeneratorConfig {
            height_variation: 0.0,
            flatness: 0.0,
            height_levels: 3,
            level_frequency: 1.0,
            ..default_config()
        };
        let mut rng = fastrand::Rng::with_seed(42);
        let net = build_node_network(&config, &mut rng);
        let la = assign_height_levels(&net, &config, &mut rng);
        let hm = generate(&config, &net, &la, &mut rng);

        let max_height = wz_maplib::constants::TILE_MAX_HEIGHT as f32;
        let level_height = max_height / 2.0;

        for (i, node) in net.nodes.iter().enumerate() {
            let expected = if la.water[i] {
                0.0
            } else {
                la.levels[i] as f32 * level_height
            };
            let actual = hm.get(node.tile_x, node.tile_y);
            let diff = (actual - expected).abs();
            // Voronoi center sits close to the node's own level, with slight
            // pull from adjacent cells.
            assert!(
                diff < level_height * 0.6,
                "Node ({},{}) expected ~{expected}, got {actual} (diff={diff})",
                node.tile_x,
                node.tile_y
            );
        }
    }
}
