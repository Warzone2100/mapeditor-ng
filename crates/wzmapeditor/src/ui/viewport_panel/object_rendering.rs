//! GPU instance building for map objects - structures, droids, features, ghosts.

use std::sync::Arc;

use glam::Mat4;
use rustc_hash::FxHashMap;

use crate::app::EditorApp;
use crate::tools::{self, ToolId};
use crate::viewport::pie_mesh::{self, ModelInstance};
use crate::viewport::renderer::EditorRenderer;

use super::SELECTION_ALPHA;

/// Collect model names referenced by the map that aren't yet uploaded to GPU.
pub(crate) fn collect_unloaded_models(app: &EditorApp) -> Vec<String> {
    let Some(doc) = app.document.as_ref() else {
        return Vec::new();
    };
    let Some(loader) = app.model_loader.as_ref() else {
        return Vec::new();
    };
    let stats = app.stats.as_ref();
    let map = &doc.map;

    let mut needed: std::collections::HashSet<String> = std::collections::HashSet::new();

    for s in &map.structures {
        if let Some(st) = stats.and_then(|db| db.structures.get(&s.name)) {
            // Preload every structureModel variant so the per-frame wall_type
            // lookup never stalls on a load. CORNER WALL stats (older saves
            // stored corners under CWall) also pull in the paired base stat's
            // full variant array.
            let ty = st.structure_type.as_deref();
            let is_wall = matches!(ty, Some("WALL"));
            let is_corner_wall = matches!(ty, Some("CORNER WALL"));
            if is_wall || is_corner_wall {
                if let Some(ref models) = st.structure_model {
                    for m in models {
                        needed.insert(m.clone());
                    }
                }
                let base_name = if is_corner_wall {
                    tools::wall_tool::base_wall_for_corner(&s.name)
                } else {
                    Some(s.name.as_str())
                };
                if let Some(base_name) = base_name {
                    if let Some(base_stat) = stats.and_then(|db| db.structures.get(base_name))
                        && let Some(ref models) = base_stat.structure_model
                    {
                        for m in models {
                            needed.insert(m.clone());
                        }
                    }
                    if let Some((corner, _)) = tools::wall_tool::corner_wall_for(base_name)
                        && let Some(corner_stat) = stats.and_then(|db| db.structures.get(corner))
                        && let Some(m) = corner_stat.pie_model()
                    {
                        needed.insert(m.to_string());
                    }
                }
            } else if let Some(imd) = st.pie_model_for_modules(s.modules) {
                needed.insert(imd.to_string());
            }
            if let Some(base) = st.base_model.as_deref().or(st.base_imd.as_deref()) {
                needed.insert(base.to_string());
            }
            if let Some(sw) = loader.resolve_structure_weapons(&s.name, stats.unwrap()) {
                for w in &sw.weapons {
                    needed.insert(w.clone());
                }
                for m in sw.mounts.iter().flatten() {
                    needed.insert(m.clone());
                }
            }
        } else if let Some(imd) = loader.imd_for_object(&s.name) {
            needed.insert(imd.to_string());
        }
    }

    for f in &map.features {
        if let Some(imd) = loader.imd_for_object(&f.name) {
            needed.insert(imd.to_string());
        }
    }

    for d in &map.droids {
        let mut has_components = false;
        if let Some(db) = stats
            && let Some(components) = loader.resolve_droid_components(&d.name, db)
        {
            if let Some(ref body) = components.body {
                needed.insert(body.clone());
                has_components = true;
            }
            if let Some(ref prop) = components.propulsion {
                needed.insert(prop.clone());
            }
            for w in &components.weapons {
                needed.insert(w.clone());
            }
            for m in components.mounts.iter().flatten() {
                needed.insert(m.clone());
            }
        }
        // Only fall back to the imd_map when component resolution didn't
        // produce a body, so we don't load a model that may not exist.
        if !has_components && let Some(imd) = loader.imd_for_object(&d.name) {
            needed.insert(imd.to_string());
        }
    }

    needed
        .into_iter()
        .filter(|name| !loader.is_uploaded(name))
        .collect()
}

pub(super) fn prepare_object_rendering(
    app: &mut EditorApp,
    renderer: &mut EditorRenderer,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) {
    let Some(doc) = app.document.as_ref() else {
        return;
    };
    let Some(loader) = app.model_loader.as_mut() else {
        return;
    };

    loader.set_tileset(app.current_tileset.texture_index());

    let map = &doc.map;
    let map_data = &map.map_data;

    // Keys are `Arc<str>` handles from the `ModelLoader` intern pool so
    // per-frame inserts are a ref-bump rather than a `String` allocation.
    // On a 200-object map with weapon turrets this saves ~800 allocations
    // per rebuild.
    let mut instances_by_model: FxHashMap<Arc<str>, Vec<ModelInstance>> = FxHashMap::default();

    let stats_ref_structs = app.stats.as_ref();

    // Tile -> wall stat id index keeps per-structure wall variant resolution
    // O(1) per neighbour instead of O(structures).
    let wall_tiles = build_wall_tile_index(map, stats_ref_structs);
    let selection = &app.selection;
    for (si, s) in map.structures.iter().enumerate() {
        let is_selected = selection.contains(&crate::app::SelectedObject::Structure(si));

        // Main structure model. Walls pick `structureModel[wallType()]` from
        // their 4-neighbor mask (see `src/structure.cpp:1377,1786`). Single-
        // model base stats fall back to the paired `CORNER WALL` stat at
        // any non-straight junction so BaBa / Tank Trap / Collective / NEXUS
        // get a real corner piece. Tiles stored under the CWall stat (older
        // saves) resolve via the paired base stat so their variant still
        // matches the neighbor mask. Non-walls use the module-aware stride.
        //
        // `override_dir` carries an absolute render direction for walls
        // (derived from the neighbor-mask shape + any per-family offset).
        // When it's set, the stored `s.direction` is ignored: wall rotation
        // is a function of the neighbor layout, not of the saved value.
        // This makes older maps that baked a CWall offset into the stored
        // direction render correctly without double-rotating.
        let (struct_imd, override_dir): (Option<Arc<str>>, Option<u16>) = {
            let stat = stats_ref_structs.and_then(|st| st.structures.get(&s.name));
            let (name_opt, dir_override) = if let Some(ss) = stat {
                let ty = ss.structure_type.as_deref();
                let is_wall = matches!(ty, Some("WALL"));
                let is_corner_wall = matches!(ty, Some("CORNER WALL"));
                if is_wall || is_corner_wall {
                    if is_corner_wall && tools::wall_tool::corner_stat_is_cross_pie(&s.name) {
                        // User-intended cross corner: render the CWall PIE as
                        // authored at the saved direction. Skip mask-derived
                        // shape resolution so neighbours don't flip it.
                        (ss.pie_model_for_modules(0).map(ToOwned::to_owned), None)
                    } else {
                        let (base_name, base_stat) = if is_corner_wall {
                            let bn = tools::wall_tool::base_wall_for_corner(&s.name);
                            let bs = bn.and_then(|n| {
                                stats_ref_structs.and_then(|db| db.structures.get(n))
                            });
                            (bn.unwrap_or(s.name.as_str()), bs.unwrap_or(ss))
                        } else {
                            (s.name.as_str(), ss)
                        };
                        let tile = (s.position.x >> 7, s.position.y >> 7);
                        let shape = wall_shape_from_index(&wall_tiles, base_name, tile);
                        match wall_model_for(
                            base_stat,
                            base_name,
                            shape.wall_type,
                            stats_ref_structs,
                        ) {
                            Some((m, o)) => (Some(m), Some(shape.direction.wrapping_add(o))),
                            None => (None, None),
                        }
                    }
                } else {
                    (
                        ss.pie_model_for_modules(s.modules).map(ToOwned::to_owned),
                        None,
                    )
                }
            } else {
                (None, None)
            };
            let name_opt = name_opt.or_else(|| loader.imd_for_object(&s.name).map(str::to_owned));
            (name_opt.map(|n| loader.intern(&n)), dir_override)
        };

        let render_direction = override_dir.unwrap_or(s.direction);
        let mut instance = make_instance(
            s.position.x,
            s.position.y,
            render_direction,
            s.player,
            map_data,
        );
        if is_selected {
            instance.team_color[3] = SELECTION_ALPHA;
        }

        if let Some(ref imd) = struct_imd {
            loader.ensure_model(imd, renderer, device, queue);
            instances_by_model
                .entry(Arc::clone(imd))
                .or_default()
                .push(instance);
        }

        // Sample once per structure, not per weapon-mount iteration.
        let struct_terrain_h = crate::viewport::picking::sample_terrain_height_pub(
            map_data,
            s.position.x as f32,
            s.position.y as f32,
        );

        // Base/foundation pad. Flat decal at Y=0 lifted slightly above
        // terrain to avoid z-fighting.
        if let Some(stats) = stats_ref_structs {
            if let Some(struct_stats) = stats.structures.get(&s.name) {
                let base_imd = struct_stats
                    .base_model
                    .as_deref()
                    .or(struct_stats.base_imd.as_deref());
                if let Some(base_name) = base_imd {
                    loader.ensure_model(base_name, renderer, device, queue);
                    let mut base_instance = make_instance_with_y_offset(
                        s.position.x,
                        s.position.y,
                        s.direction,
                        s.player,
                        map_data,
                        2.0,
                    );
                    if is_selected {
                        base_instance.team_color[3] = SELECTION_ALPHA;
                    }
                    let base_key = loader.intern(base_name);
                    instances_by_model
                        .entry(base_key)
                        .or_default()
                        .push(base_instance);
                }
            }

            let mut turret_color = pie_mesh::team_color(s.player);
            if is_selected {
                turret_color[3] = SELECTION_ALPHA;
            }
            render_structure_turrets(
                loader,
                renderer,
                device,
                queue,
                &mut instances_by_model,
                &s.name,
                struct_imd.as_deref(),
                stats,
                glam::Vec3::new(s.position.x as f32, struct_terrain_h, s.position.y as f32),
                s.direction,
                turret_color,
            );
        }
    }

    for (fi, f) in map.features.iter().enumerate() {
        if let Some(imd_name) = loader.imd_for_object(&f.name) {
            let imd_owned = imd_name.to_owned();
            loader.ensure_model(&imd_owned, renderer, device, queue);
            let player = f.player.unwrap_or(0);
            let mut instance =
                make_instance(f.position.x, f.position.y, f.direction, player, map_data);
            if selection.contains(&crate::app::SelectedObject::Feature(fi)) {
                instance.team_color[3] = SELECTION_ALPHA;
            }
            let feature_key = loader.intern(&imd_owned);
            instances_by_model
                .entry(feature_key)
                .or_default()
                .push(instance);
        }
    }

    let stats_ref = app.stats.as_ref();
    for (di, d) in map.droids.iter().enumerate() {
        let mut base_instance =
            make_instance(d.position.x, d.position.y, d.direction, d.player, map_data);
        let is_droid_selected = selection.contains(&crate::app::SelectedObject::Droid(di));
        if is_droid_selected {
            base_instance.team_color[3] = SELECTION_ALPHA;
        }

        // Sample once per droid, shared across all weapon mounts.
        let droid_terrain_h = crate::viewport::picking::sample_terrain_height_pub(
            map_data,
            d.position.x as f32,
            d.position.y as f32,
        );

        let mut rendered = false;
        if let Some(stats) = stats_ref
            && let Some(components) = loader.resolve_droid_components(&d.name, stats)
            && let Some(ref body_imd) = components.body
        {
            loader.ensure_model(body_imd, renderer, device, queue);
            let body_key = loader.intern(body_imd);
            instances_by_model
                .entry(Arc::clone(&body_key))
                .or_default()
                .push(base_instance);
            rendered = true;

            // Cyborgs reuse the body model as propulsion; skip the duplicate.
            if let Some(ref prop_imd) = components.propulsion
                && components.body.as_deref() != Some(prop_imd.as_str())
            {
                loader.ensure_model(prop_imd, renderer, device, queue);
                let prop_key = loader.intern(prop_imd);
                instances_by_model
                    .entry(prop_key)
                    .or_default()
                    .push(base_instance);
            }

            if !components.weapons.is_empty() {
                let connectors = loader.get_connectors(body_imd);
                let angle_rad = wz_maplib::constants::direction_to_radians(d.direction);
                let rotation = glam::Quat::from_rotation_y(angle_rad);
                let translation =
                    glam::Vec3::new(d.position.x as f32, droid_terrain_h, d.position.y as f32);
                for (wi, weapon_imd) in components.weapons.iter().enumerate() {
                    let body_connector = connectors.get(wi).copied().unwrap_or(glam::Vec3::ZERO);

                    let mount_pos = translation + rotation * body_connector;
                    let mount_matrix = model_matrix_from_pos_dir(mount_pos, angle_rad);
                    let mut mount_instance = ModelInstance {
                        model_matrix: mount_matrix.to_cols_array_2d(),
                        team_color: pie_mesh::team_color(d.player),
                    };
                    if is_droid_selected {
                        mount_instance.team_color[3] = SELECTION_ALPHA;
                    }

                    let mut mount_connector = glam::Vec3::ZERO;
                    if let Some(Some(mount_imd)) = components.mounts.get(wi) {
                        loader.ensure_model(mount_imd, renderer, device, queue);
                        mount_connector = loader
                            .get_connectors(mount_imd)
                            .first()
                            .copied()
                            .unwrap_or(glam::Vec3::ZERO);
                        let mount_key = loader.intern(mount_imd);
                        instances_by_model
                            .entry(mount_key)
                            .or_default()
                            .push(mount_instance);
                    }

                    let weapon_pos = mount_pos + rotation * mount_connector;
                    let weapon_matrix = model_matrix_from_pos_dir(weapon_pos, angle_rad);
                    let mut weapon_instance = ModelInstance {
                        model_matrix: weapon_matrix.to_cols_array_2d(),
                        team_color: pie_mesh::team_color(d.player),
                    };
                    if is_droid_selected {
                        weapon_instance.team_color[3] = SELECTION_ALPHA;
                    }

                    loader.ensure_model(weapon_imd, renderer, device, queue);
                    let weapon_key = loader.intern(weapon_imd);
                    instances_by_model
                        .entry(weapon_key)
                        .or_default()
                        .push(weapon_instance);
                }
            }
        }

        if !rendered && let Some(imd_name) = loader.imd_for_object(&d.name) {
            let imd = imd_name.to_owned();
            loader.ensure_model(&imd, renderer, device, queue);
            let imd_key = loader.intern(&imd);
            instances_by_model
                .entry(imd_key)
                .or_default()
                .push(base_instance);
        }
    }

    let place_ghost = (app.tool_state.active_tool == ToolId::ObjectPlace)
        .then(|| app.tool_state.object_place())
        .flatten()
        .and_then(|t| {
            let name = t.placement_object.as_deref()?;
            let pos = t.preview_pos?;
            Some((name.to_owned(), pos, t.placement_direction, t.preview_valid))
        });
    if let Some((obj_name, (gx, gz), ghost_dir, valid)) = place_ghost {
        let obj_name = obj_name.as_str();
        let ghost_color = if valid {
            [0.0, 0.8, 0.0, -0.45] // green ghost = valid
        } else {
            [0.8, 0.0, 0.0, -0.45] // red ghost = invalid
        };

        let ghost_instance = |wx: u32, wz: u32, dir: u16| -> ModelInstance {
            let wxf = wx as f32;
            let wzf = wz as f32;
            let terrain_h = crate::viewport::picking::sample_terrain_height_pub(map_data, wxf, wzf);
            let angle_rad = wz_maplib::constants::direction_to_radians(dir);
            let translation = glam::Vec3::new(wxf, terrain_h, wzf);
            let model_matrix = model_matrix_from_pos_dir(translation, angle_rad);
            ModelInstance {
                model_matrix: model_matrix.to_cols_array_2d(),
                team_color: ghost_color,
            }
        };

        let imd_name: Option<String> = if let Some(stats) = app.stats.as_ref() {
            if let Some(ss) = stats.structures.get(obj_name) {
                ss.pie_model_for_modules(0)
                    .map(str::to_owned)
                    .or_else(|| loader.imd_for_object(obj_name).map(str::to_owned))
            } else {
                loader.imd_for_object(obj_name).map(str::to_owned)
            }
        } else {
            loader.imd_for_object(obj_name).map(str::to_owned)
        };

        if let Some(ref imd) = imd_name {
            loader.ensure_model(imd, renderer, device, queue);
            let imd_key = loader.intern(imd);

            let mirror_mode = app.tool_state.mirror_mode;
            let ghost_pts = tools::mirror::mirror_world_points(
                gx,
                gz,
                map_data.width,
                map_data.height,
                mirror_mode,
            );

            for (pi, &(mpx, mpz)) in ghost_pts.iter().enumerate() {
                let m_dir = tools::mirror::mirror_direction(ghost_dir, mirror_mode, pi);
                instances_by_model
                    .entry(Arc::clone(&imd_key))
                    .or_default()
                    .push(ghost_instance(mpx, mpz, m_dir));

                let wxf = mpx as f32;
                let wzf = mpz as f32;
                let terrain_h =
                    crate::viewport::picking::sample_terrain_height_pub(map_data, wxf, wzf);

                if let Some(stats) = app.stats.as_ref()
                    && let Some(ss) = stats.structures.get(obj_name)
                {
                    let base_imd = ss.base_model.as_deref().or(ss.base_imd.as_deref());
                    if let Some(base_name) = base_imd {
                        loader.ensure_model(base_name, renderer, device, queue);
                        let angle_rad = wz_maplib::constants::direction_to_radians(m_dir);
                        let translation = glam::Vec3::new(wxf, terrain_h + 2.0, wzf);
                        let model_matrix = model_matrix_from_pos_dir(translation, angle_rad);
                        let base_ghost = ModelInstance {
                            model_matrix: model_matrix.to_cols_array_2d(),
                            team_color: ghost_color,
                        };
                        let base_key = loader.intern(base_name);
                        instances_by_model
                            .entry(base_key)
                            .or_default()
                            .push(base_ghost);
                    }
                }

                // Ghost turrets (weapons / sensor / ECM). Resolves to nothing
                // for non-structure placements.
                if let Some(stats) = app.stats.as_ref() {
                    render_structure_turrets(
                        loader,
                        renderer,
                        device,
                        queue,
                        &mut instances_by_model,
                        obj_name,
                        Some(imd.as_str()),
                        stats,
                        glam::Vec3::new(wxf, terrain_h, wzf),
                        m_dir,
                        ghost_color,
                    );

                    // Droid composite: propulsion + per-weapon mount + weapon
                    // at the body's connector points. Mirrors the placed-droid
                    // assembly so the ghost previews the full vehicle.
                    render_droid_ghost(
                        loader,
                        renderer,
                        device,
                        queue,
                        &mut instances_by_model,
                        obj_name,
                        imd.as_str(),
                        stats,
                        glam::Vec3::new(wxf, terrain_h, wzf),
                        m_dir,
                        ghost_color,
                    );
                }
            }
        }
    }

    // Ghost preview for the wall placer: resolve the variant + rotation the
    // renderer would emit if a wall were placed under the cursor, so the user
    // sees the actual junction snap before clicking.
    if app.tool_state.active_tool == ToolId::WallPlacement
        && let Some(wt) = app.tool_state.wall_tool()
        && let (Some(family), Some((tx, ty))) = (wt.family(), wt.hover_tile())
        && tx < map_data.width
        && ty < map_data.height
        && let Some(stats) = stats_ref_structs
    {
        let shape = wall_shape_from_index(&wall_tiles, &family.base, (tx, ty));
        // Cross-corners L picks the family's CWall PIE directly so the ghost
        // matches what gets saved. Other shapes use the live render's
        // resolver.
        let resolved: Option<(String, u16)> = if wt.cross_corners()
            && shape.wall_type == tools::wall_tool::WALL_TYPE_L_CORNER
            && tools::wall_tool::family_has_cross_corner(&family.base)
            && let Some((corner_name, _)) = tools::wall_tool::corner_wall_for(&family.base)
            && let Some(corner_stat) = stats.structures.get(corner_name)
            && let Some(m) = corner_stat.pie_model()
        {
            Some((m.to_owned(), 0))
        } else if let Some(base_stat) = stats.structures.get(&family.base) {
            wall_model_for(base_stat, &family.base, shape.wall_type, stats_ref_structs)
                .map(|(m, off)| (m, shape.direction.wrapping_add(off)))
        } else {
            None
        };

        if let Some((imd, dir)) = resolved {
            // Skip the ghost when the cursor sits on a tile that already
            // has a wall from this family; the placed wall is the preview.
            let already_has_family_wall = map.structures.iter().any(|s| {
                let stile = (s.position.x >> 7, s.position.y >> 7);
                if stile != (tx, ty) {
                    return false;
                }
                if s.name == family.base {
                    return true;
                }
                tools::wall_tool::base_wall_for_corner(&s.name) == Some(family.base.as_str())
            });
            if !already_has_family_wall {
                loader.ensure_model(&imd, renderer, device, queue);
                let imd_key = loader.intern(&imd);
                let wx =
                    tx * wz_maplib::constants::TILE_UNITS + wz_maplib::constants::TILE_UNITS / 2;
                let wz =
                    ty * wz_maplib::constants::TILE_UNITS + wz_maplib::constants::TILE_UNITS / 2;
                let terrain_h = crate::viewport::picking::sample_terrain_height_pub(
                    map_data, wx as f32, wz as f32,
                );
                let angle_rad = wz_maplib::constants::direction_to_radians(dir);
                let translation = glam::Vec3::new(wx as f32, terrain_h, wz as f32);
                let model_matrix = model_matrix_from_pos_dir(translation, angle_rad);
                let ghost = ModelInstance {
                    model_matrix: model_matrix.to_cols_array_2d(),
                    team_color: [0.0_f32, 0.8, 0.0, -0.45],
                };
                instances_by_model.entry(imd_key).or_default().push(ghost);
            }
        }
    }

    // Stamp tool ghosts. Only Single mode shows PIE ghosts; Scatter relies on
    // its circle outline overlay instead.
    let stamp_ghost = (app.tool_state.active_tool == ToolId::Stamp)
        .then(|| app.tool_state.stamp())
        .flatten()
        .filter(|s| !s.capture_mode && s.mode == tools::StampMode::Single && s.stamp_objects)
        .and_then(|s| s.pattern.as_ref().zip(s.preview_pos));
    if let Some((pattern, (ox, oy))) = stamp_ghost {
        // Only show ghosts when the footprint is fully on-map (green box).
        let end_x = ox + pattern.width;
        let end_y = oy + pattern.height;
        if end_x <= map_data.width && end_y <= map_data.height && !pattern.objects.is_empty() {
            let ghost_color = [0.0_f32, 0.8, 0.0, -0.45];
            let origin_wx = ox * wz_maplib::constants::TILE_UNITS;
            let origin_wy = oy * wz_maplib::constants::TILE_UNITS;

            for obj in &pattern.objects {
                let (obj_name, off_x, off_y, direction) = obj.name_offset_dir();
                let wx = (origin_wx as i32 + off_x) as f32;
                let wz = (origin_wy as i32 + off_y) as f32;
                let terrain_h =
                    crate::viewport::picking::sample_terrain_height_pub(map_data, wx, wz);
                let angle_rad = wz_maplib::constants::direction_to_radians(direction);
                let translation = glam::Vec3::new(wx, terrain_h, wz);
                let model_matrix = model_matrix_from_pos_dir(translation, angle_rad);
                let ghost = ModelInstance {
                    model_matrix: model_matrix.to_cols_array_2d(),
                    team_color: ghost_color,
                };

                let imd_name: Option<String> = if let Some(stats) = app.stats.as_ref() {
                    if let Some(ss) = stats.structures.get(obj_name) {
                        ss.pie_model_for_modules(0)
                            .map(str::to_owned)
                            .or_else(|| loader.imd_for_object(obj_name).map(str::to_owned))
                    } else {
                        loader.imd_for_object(obj_name).map(str::to_owned)
                    }
                } else {
                    loader.imd_for_object(obj_name).map(str::to_owned)
                };

                if let Some(ref imd) = imd_name {
                    loader.ensure_model(imd, renderer, device, queue);
                    let imd_key = loader.intern(imd);
                    instances_by_model.entry(imd_key).or_default().push(ghost);

                    if let tools::stamp::StampObject::Structure { .. } = obj
                        && let Some(stats) = app.stats.as_ref()
                    {
                        if let Some(ss) = stats.structures.get(obj_name) {
                            let base_imd = ss.base_model.as_deref().or(ss.base_imd.as_deref());
                            if let Some(base_name) = base_imd {
                                loader.ensure_model(base_name, renderer, device, queue);
                                let base_translation = glam::Vec3::new(wx, terrain_h + 2.0, wz);
                                let base_matrix =
                                    model_matrix_from_pos_dir(base_translation, angle_rad);
                                let base_ghost = ModelInstance {
                                    model_matrix: base_matrix.to_cols_array_2d(),
                                    team_color: ghost_color,
                                };
                                let base_key = loader.intern(base_name);
                                instances_by_model
                                    .entry(base_key)
                                    .or_default()
                                    .push(base_ghost);
                            }
                        }
                        render_structure_turrets(
                            loader,
                            renderer,
                            device,
                            queue,
                            &mut instances_by_model,
                            obj_name,
                            Some(imd.as_str()),
                            stats,
                            glam::Vec3::new(wx, terrain_h, wz),
                            direction,
                            ghost_color,
                        );
                    }
                }
            }
        }
    }

    renderer.prepare_object_draw_calls(device, queue, &instances_by_model);
}

/// PIE mesh vertices are Z-flipped at build time (see `pie_mesh.rs`), which
/// reverses the rotation handedness. WZ2100 uses `UNDEG(-direction)` on
/// un-flipped vertices; after the Z-flip the sign inverts to `+angle`.
fn model_matrix_from_pos_dir(translation: glam::Vec3, angle_rad: f32) -> Mat4 {
    Mat4::from_rotation_translation(glam::Quat::from_rotation_y(angle_rad), translation)
}

fn make_instance(
    world_x: u32,
    world_y: u32,
    direction: u16,
    player: i8,
    map_data: &wz_maplib::MapData,
) -> ModelInstance {
    // WZ2100 2D y maps to 3D z; y is up.
    let wx = world_x as f32;
    let wz = world_y as f32;

    let terrain_h = crate::viewport::picking::sample_terrain_height_pub(map_data, wx, wz);
    let angle_rad = wz_maplib::constants::direction_to_radians(direction);

    let translation = glam::Vec3::new(wx, terrain_h, wz);
    let model_matrix = model_matrix_from_pos_dir(translation, angle_rad);

    ModelInstance {
        model_matrix: model_matrix.to_cols_array_2d(),
        team_color: pie_mesh::team_color(player),
    }
}

/// Like [`make_instance`] but adds a vertical offset to prevent z-fighting.
fn make_instance_with_y_offset(
    world_x: u32,
    world_y: u32,
    direction: u16,
    player: i8,
    map_data: &wz_maplib::MapData,
    y_offset: f32,
) -> ModelInstance {
    let wx = world_x as f32;
    let wz = world_y as f32;
    let terrain_h = crate::viewport::picking::sample_terrain_height_pub(map_data, wx, wz);
    let angle_rad = wz_maplib::constants::direction_to_radians(direction);
    let translation = glam::Vec3::new(wx, terrain_h + y_offset, wz);
    let model_matrix = model_matrix_from_pos_dir(translation, angle_rad);

    ModelInstance {
        model_matrix: model_matrix.to_cols_array_2d(),
        team_color: pie_mesh::team_color(player),
    }
}

/// Append turret instances (mount + weapon/sensor/ECM head) for a structure.
///
/// Mirrors WZ2100's `renderStructureTurrets` priority: weapons, then ECM,
/// then sensor. `resolve_structure_weapons` already encodes that fallback
/// and skips walls/gates.
#[expect(
    clippy::too_many_arguments,
    reason = "renders one structure's full turret stack; needs every GPU + cache handle"
)]
fn render_structure_turrets(
    loader: &mut crate::viewport::model_loader::ModelLoader,
    renderer: &mut EditorRenderer,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    instances_by_model: &mut FxHashMap<Arc<str>, Vec<ModelInstance>>,
    struct_name: &str,
    struct_imd: Option<&str>,
    stats: &wz_stats::StatsDatabase,
    world_pos: glam::Vec3,
    direction: u16,
    team_color: [f32; 4],
) {
    let Some(sw) = loader.resolve_structure_weapons(struct_name, stats) else {
        return;
    };
    let connectors = struct_imd
        .map(|imd| loader.get_connectors(imd))
        .unwrap_or_default();
    let angle_rad = wz_maplib::constants::direction_to_radians(direction);
    let rotation = glam::Quat::from_rotation_y(angle_rad);

    for (wi, weapon_imd) in sw.weapons.iter().enumerate() {
        let connector = connectors.get(wi).copied().unwrap_or(glam::Vec3::ZERO);
        let mount_pos = world_pos + rotation * connector;
        let mount_matrix = model_matrix_from_pos_dir(mount_pos, angle_rad);
        let mount_instance = ModelInstance {
            model_matrix: mount_matrix.to_cols_array_2d(),
            team_color,
        };

        let mut mount_connector = glam::Vec3::ZERO;
        if let Some(Some(mount_imd)) = sw.mounts.get(wi) {
            loader.ensure_model(mount_imd, renderer, device, queue);
            mount_connector = loader
                .get_connectors(mount_imd)
                .first()
                .copied()
                .unwrap_or(glam::Vec3::ZERO);
            let mount_key = loader.intern(mount_imd);
            instances_by_model
                .entry(mount_key)
                .or_default()
                .push(mount_instance);
        }

        let weapon_pos = mount_pos + rotation * mount_connector;
        let weapon_matrix = model_matrix_from_pos_dir(weapon_pos, angle_rad);
        let weapon_instance = ModelInstance {
            model_matrix: weapon_matrix.to_cols_array_2d(),
            team_color,
        };
        loader.ensure_model(weapon_imd, renderer, device, queue);
        let weapon_key = loader.intern(weapon_imd);
        instances_by_model
            .entry(weapon_key)
            .or_default()
            .push(weapon_instance);
    }
}

/// Build ghost instances for a droid template's propulsion, mounts, and
/// weapons at the body's connector points. No-op when `obj_name` isn't a
/// known droid template.
fn render_droid_ghost(
    loader: &mut crate::viewport::model_loader::ModelLoader,
    renderer: &mut EditorRenderer,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    instances_by_model: &mut FxHashMap<Arc<str>, Vec<ModelInstance>>,
    obj_name: &str,
    body_imd: &str,
    stats: &wz_stats::StatsDatabase,
    world_pos: glam::Vec3,
    direction: u16,
    team_color: [f32; 4],
) {
    let Some(components) = loader.resolve_droid_components(obj_name, stats) else {
        return;
    };
    let angle_rad = wz_maplib::constants::direction_to_radians(direction);
    let rotation = glam::Quat::from_rotation_y(angle_rad);
    let body_matrix = model_matrix_from_pos_dir(world_pos, angle_rad);
    let body_instance = ModelInstance {
        model_matrix: body_matrix.to_cols_array_2d(),
        team_color,
    };

    // Cyborgs reuse the body model as propulsion; skip the duplicate.
    if let Some(ref prop_imd) = components.propulsion
        && components.body.as_deref() != Some(prop_imd.as_str())
    {
        loader.ensure_model(prop_imd, renderer, device, queue);
        let prop_key = loader.intern(prop_imd);
        instances_by_model
            .entry(prop_key)
            .or_default()
            .push(body_instance);
    }

    if components.weapons.is_empty() {
        return;
    }
    let connectors = loader.get_connectors(body_imd);

    for (wi, weapon_imd) in components.weapons.iter().enumerate() {
        let body_connector = connectors.get(wi).copied().unwrap_or(glam::Vec3::ZERO);
        let mount_pos = world_pos + rotation * body_connector;
        let mount_matrix = model_matrix_from_pos_dir(mount_pos, angle_rad);
        let mount_instance = ModelInstance {
            model_matrix: mount_matrix.to_cols_array_2d(),
            team_color,
        };

        let mut mount_connector = glam::Vec3::ZERO;
        if let Some(Some(mount_imd)) = components.mounts.get(wi) {
            loader.ensure_model(mount_imd, renderer, device, queue);
            mount_connector = loader
                .get_connectors(mount_imd)
                .first()
                .copied()
                .unwrap_or(glam::Vec3::ZERO);
            let mount_key = loader.intern(mount_imd);
            instances_by_model
                .entry(mount_key)
                .or_default()
                .push(mount_instance);
        }

        let weapon_pos = mount_pos + rotation * mount_connector;
        let weapon_matrix = model_matrix_from_pos_dir(weapon_pos, angle_rad);
        let weapon_instance = ModelInstance {
            model_matrix: weapon_matrix.to_cols_array_2d(),
            team_color,
        };
        loader.ensure_model(weapon_imd, renderer, device, queue);
        let weapon_key = loader.intern(weapon_imd);
        instances_by_model
            .entry(weapon_key)
            .or_default()
            .push(weapon_instance);
    }
}

/// Marker recorded per occupied wall-family tile: which base stat sits there,
/// and whether the occupant combines with walls (so towers, hardpoints, and
/// gates count as connectors just like WZ2100 `isWallCombiningStructureType`).
struct WallTileEntry {
    stat_name: String,
    combines_only: bool,
}

/// Map each tile to its wall-family connector. Rebuilt once per frame; the
/// per-structure render loop reads this to compute the 4-neighbor mask
/// without rescanning `map.structures`.
///
/// `stat_name` is normalised to the family's base `WALL` stat so tiles saved
/// under either the base stat or the paired `CORNER WALL` stat match as the
/// same family. This keeps older maps (corners stored as `CWall`) rendering
/// correctly alongside freshly painted tiles saved as the base stat.
fn build_wall_tile_index(
    map: &wz_maplib::WzMap,
    stats: Option<&wz_stats::StatsDatabase>,
) -> std::collections::HashMap<(u32, u32), WallTileEntry> {
    let mut out = std::collections::HashMap::new();
    let Some(db) = stats else {
        return out;
    };
    for s in &map.structures {
        let Some(st) = db.structures.get(&s.name) else {
            continue;
        };
        let ty = st.structure_type.as_deref();
        let is_wall = matches!(ty, Some("WALL"));
        let is_corner_wall = matches!(ty, Some("CORNER WALL"));
        if !is_wall && !is_corner_wall && !st.combines_with_wall {
            continue;
        }
        let normalized = if is_corner_wall {
            tools::wall_tool::base_wall_for_corner(&s.name)
                .map_or_else(|| s.name.clone(), str::to_owned)
        } else {
            s.name.clone()
        };
        let tile = (s.position.x >> 7, s.position.y >> 7);
        out.insert(
            tile,
            WallTileEntry {
                stat_name: normalized,
                combines_only: !is_wall && !is_corner_wall,
            },
        );
    }
    out
}

/// Resolve the wall shape at `tile` from the per-frame tile index. Neighbors
/// count as "present" when they are either the same base stat or a
/// `combinesWithWall` structure (wall tower / hardpoint / gate), matching
/// `isWallCombiningStructureType` in WZ2100.
fn wall_shape_from_index(
    index: &std::collections::HashMap<(u32, u32), WallTileEntry>,
    stat_name: &str,
    tile: (u32, u32),
) -> tools::wall_tool::WallShape {
    let mut mask = 0u8;
    let candidates = [
        (-1i32, 0i32, tools::wall_tool::MASK_LEFT),
        (1, 0, tools::wall_tool::MASK_RIGHT),
        (0, -1, tools::wall_tool::MASK_UP),
        (0, 1, tools::wall_tool::MASK_DOWN),
    ];
    for (dx, dy, bit) in candidates {
        let Some(nx) = tile.0.checked_add_signed(dx) else {
            continue;
        };
        let Some(ny) = tile.1.checked_add_signed(dy) else {
            continue;
        };
        let Some(entry) = index.get(&(nx, ny)) else {
            continue;
        };
        if entry.combines_only || entry.stat_name == stat_name {
            mask |= bit;
        }
    }
    tools::wall_tool::wall_shape_for_mask(mask)
}

/// Resolve which PIE to render for a wall at a given `wall_type`, plus an
/// extra direction offset needed to align the chosen PIE's authored
/// orientation with the `wallDir` encoding.
///
/// Hardcrete's `structureModel` has 4 entries covering straight / cross /
/// T / L, so we index into it directly with no offset. Single-model
/// families (`BaBa`, Tank Trap, Collective, NEXUS) fall back to their paired
/// `CORNER WALL` stat for any non-straight junction so bends, tees, and
/// crosses show a real corner piece.
fn wall_model_for(
    base_stat: &wz_stats::structures::StructureStats,
    base_name: &str,
    wall_type: u8,
    stats: Option<&wz_stats::StatsDatabase>,
) -> Option<(String, u16)> {
    let base_models_len = base_stat.structure_model.as_ref().map_or(0, Vec::len);
    if wall_type != tools::wall_tool::WALL_TYPE_STRAIGHT
        && base_models_len < 4
        && let Some((corner, dir_offset)) = tools::wall_tool::corner_wall_for(base_name)
        && let Some(corner_stat) = stats.and_then(|db| db.structures.get(corner))
        && let Some(m) = corner_stat.pie_model()
    {
        return Some((m.to_string(), dir_offset));
    }
    base_stat
        .pie_model_for_wall_type(wall_type)
        .map(|m| (m.to_owned(), 0))
}
