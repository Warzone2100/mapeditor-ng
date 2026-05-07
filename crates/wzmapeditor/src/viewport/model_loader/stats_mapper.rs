//! Stat-name to PIE-file resolution.
//!
//! Builds an object-name to `imd_name` table from a `StatsDatabase` and
//! resolves droid templates and structures into their visible component
//! models (body, propulsion, weapons, mounts, sensors).

use std::collections::HashMap;

use wz_stats::StatsDatabase;

/// Resolved component PIE model names for a droid template.
#[derive(Debug)]
pub struct DroidComponents {
    pub body: Option<String>,
    pub propulsion: Option<String>,
    pub weapons: Vec<String>,
    /// Turret mount models corresponding 1:1 to `weapons`.
    pub mounts: Vec<Option<String>>,
}

/// Resolved weapon/sensor turret models for a structure.
#[derive(Debug)]
pub struct StructureWeapons {
    pub weapons: Vec<String>,
    /// Turret mount models corresponding 1:1 to `weapons`.
    pub mounts: Vec<Option<String>>,
}

/// Build object-name to `imd_name` from structure, feature, and template
/// stats. Templates use the body model as their primary IMD; the
/// composite (body+prop+weapon) is the caller's responsibility.
pub(crate) fn build_imd_map(stats: &StatsDatabase) -> HashMap<String, String> {
    let mut imd_map = HashMap::new();

    for (id, ss) in &stats.structures {
        if let Some(imd) = ss.pie_model() {
            imd_map.insert(id.clone(), imd.to_string());
        }
    }

    for (id, fs) in &stats.features {
        if let Some(imd) = fs.pie_model() {
            imd_map.insert(id.clone(), imd.to_string());
        }
    }

    for (id, tmpl) in &stats.templates {
        if let Some(body_stats) = stats.bodies.get(&tmpl.body)
            && let Some(imd) = body_stats.pie_model()
        {
            imd_map.insert(id.clone(), imd.to_string());
        }
    }

    log::info!(
        "ModelLoader initialized: {} imd mappings from stats ({} templates, {} bodies, {} weapons)",
        imd_map.len(),
        stats.templates.len(),
        stats.bodies.len(),
        stats.weapons.len(),
    );

    imd_map
}

/// Resolve a droid template name into its component PIE model names.
pub(crate) fn resolve_droid_components(
    droid_name: &str,
    stats: &StatsDatabase,
) -> Option<DroidComponents> {
    let tmpl = stats.templates.get(droid_name)?;

    let body_stats = stats.bodies.get(&tmpl.body);
    let body_imd = body_stats
        .and_then(|b| b.pie_model())
        .map(ToString::to_string);

    // Body-specific propulsion (propulsionExtraModels) wins over the
    // generic propulsion.json model.
    let prop_imd = body_stats
        .and_then(|b| b.propulsion_model(&tmpl.propulsion))
        .or_else(|| {
            stats
                .propulsion
                .get(&tmpl.propulsion)
                .and_then(|p| p.pie_model())
        })
        .map(ToString::to_string);

    let mut weapon_imds = Vec::new();
    let mut mount_imds = Vec::new();

    for w in &tmpl.weapons {
        if let Some(ws) = stats.weapons.get(w)
            && let Some(model) = ws.pie_model()
        {
            weapon_imds.push(model.to_string());
            mount_imds.push(ws.mount_model.clone());
        }
    }

    push_turret(
        tmpl.construct.as_ref(),
        &stats.construct,
        &mut weapon_imds,
        &mut mount_imds,
    );
    push_turret(
        tmpl.sensor.as_ref(),
        &stats.sensor,
        &mut weapon_imds,
        &mut mount_imds,
    );
    push_turret(
        tmpl.repair.as_ref(),
        &stats.repair,
        &mut weapon_imds,
        &mut mount_imds,
    );

    // DROID_COMMAND lists the brain's turret weapon in both `weapons`
    // and `brain.turret`; skip the duplicate.
    if let Some(ref brain_name) = tmpl.brain
        && let Some(bs) = stats.brain.get(brain_name)
        && let Some(ref turret_weapon) = bs.turret
        && !tmpl.weapons.contains(turret_weapon)
        && let Some(ws) = stats.weapons.get(turret_weapon)
        && let Some(model) = ws.pie_model()
    {
        weapon_imds.push(model.to_string());
        mount_imds.push(ws.mount_model.clone());
    }

    Some(DroidComponents {
        body: body_imd,
        propulsion: prop_imd,
        weapons: weapon_imds,
        mounts: mount_imds,
    })
}

/// Resolve a structure name into its weapon/sensor/ECM turret models.
pub(crate) fn resolve_structure_weapons(
    structure_name: &str,
    stats: &StatsDatabase,
) -> Option<StructureWeapons> {
    let struct_stats = stats.structures.get(structure_name)?;

    let mut weapon_imds = Vec::new();
    let mut mount_imds = Vec::new();

    for w in &struct_stats.weapons {
        if let Some(ws) = stats.weapons.get(w)
            && let Some(model) = ws.pie_model()
        {
            weapon_imds.push(model.to_string());
            mount_imds.push(ws.mount_model.clone());
        }
    }

    // Walls, corner walls and gates never mount a visible turret even
    // though their stats reference ZNULLSENSOR/BaBaSensor with a non-null
    // model. WZ2100's renderer skips the turret pass for these types.
    let suppress_turret_fallback = matches!(
        struct_stats.structure_type.as_deref(),
        Some("WALL" | "CORNER WALL" | "GATE")
    );

    if weapon_imds.is_empty()
        && !suppress_turret_fallback
        && let Some(ref ecm_id) = struct_stats.ecm_id
        && let Some(es) = stats.ecm.get(ecm_id)
        && let Some(ref model) = es.sensor_model
    {
        weapon_imds.push(model.clone());
        mount_imds.push(es.mount_model.clone());
    }

    if weapon_imds.is_empty()
        && !suppress_turret_fallback
        && let Some(ref sensor_id) = struct_stats.sensor_id
        && let Some(ss) = stats.sensor.get(sensor_id)
        && let Some(ref model) = ss.sensor_model
    {
        weapon_imds.push(model.clone());
        mount_imds.push(ss.mount_model.clone());
    }

    if weapon_imds.is_empty() {
        return None;
    }

    Some(StructureWeapons {
        weapons: weapon_imds,
        mounts: mount_imds,
    })
}

/// Turret stats with a PIE model and mount model.
trait TurretStats {
    fn turret_model(&self) -> Option<&str>;
    fn mount_model(&self) -> Option<&String>;
}

impl TurretStats for wz_stats::turrets::ConstructStats {
    fn turret_model(&self) -> Option<&str> {
        self.sensor_model.as_deref()
    }
    fn mount_model(&self) -> Option<&String> {
        self.mount_model.as_ref()
    }
}

impl TurretStats for wz_stats::turrets::SensorStats {
    fn turret_model(&self) -> Option<&str> {
        self.sensor_model.as_deref()
    }
    fn mount_model(&self) -> Option<&String> {
        self.mount_model.as_ref()
    }
}

impl TurretStats for wz_stats::turrets::RepairStats {
    fn turret_model(&self) -> Option<&str> {
        self.model.as_deref()
    }
    fn mount_model(&self) -> Option<&String> {
        self.mount_model.as_ref()
    }
}

fn push_turret<T: TurretStats>(
    name: Option<&String>,
    stats_map: &HashMap<String, T>,
    weapon_imds: &mut Vec<String>,
    mount_imds: &mut Vec<Option<String>>,
) {
    let Some(turret_name) = name else { return };
    let Some(ts) = stats_map.get(turret_name) else {
        return;
    };
    let Some(model) = ts.turret_model() else {
        return;
    };
    weapon_imds.push(model.to_string());
    mount_imds.push(ts.mount_model().cloned());
}
