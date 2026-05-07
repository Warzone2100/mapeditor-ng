//! WZ2100 component-compatibility rules for the droid designer.
//!
//! Mirrors `design.cpp::intValidTemplate` so templates saved here load
//! cleanly in the game. Pure functions, no UI, no I/O.

use wz_stats::StatsDatabase;
use wz_stats::bodies::BodyStats;
use wz_stats::propulsion::PropulsionStats;
use wz_stats::templates::TemplateStats;
use wz_stats::weapons::WeaponStats;

/// Broad family a droid template falls into, derived from `droidType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DroidFamily {
    /// Tank-like droid (any non-cyborg, non-transporter type).
    Standard,
    /// Light, medium or heavy cyborg.
    Cyborg,
    /// Super cyborg (NEXUS).
    SuperCyborg,
    /// Transporter / `SuperTransporter`.
    Transporter,
}

pub fn droid_family(droid_type: Option<&str>) -> DroidFamily {
    match droid_type.map(str::to_ascii_uppercase).as_deref() {
        Some("CYBORG_SUPER") => DroidFamily::SuperCyborg,
        Some("CYBORG" | "CYBORG_CONSTRUCT" | "CYBORG_REPAIR") => DroidFamily::Cyborg,
        Some("TRANSPORTER" | "SUPERTRANSPORTER") => DroidFamily::Transporter,
        _ => DroidFamily::Standard,
    }
}

fn usage_matches(family: DroidFamily, usage: Option<&str>) -> bool {
    match family {
        DroidFamily::Cyborg => usage == Some("Cyborg"),
        DroidFamily::SuperCyborg => usage == Some("SuperCyborg") || usage == Some("Cyborg"),
        DroidFamily::Standard | DroidFamily::Transporter => usage.is_none(),
    }
}

pub fn body_allowed(body: &BodyStats, family: DroidFamily) -> bool {
    match family {
        DroidFamily::Cyborg | DroidFamily::SuperCyborg => body.is_cyborg(),
        DroidFamily::Transporter => body.is_transporter(),
        DroidFamily::Standard => {
            !body.is_cyborg() && !body.is_transporter() && !body.is_scavenger()
        }
    }
}

/// True when the body should appear in the designer for this family.
/// Hides scavenger and AI-only chassis via the `designable` flag.
pub fn body_selectable(body: &BodyStats, family: DroidFamily) -> bool {
    if !body.designable {
        return false;
    }
    body_allowed(body, family)
}

pub fn propulsion_allowed(prop: &PropulsionStats, family: DroidFamily) -> bool {
    if !prop.designable {
        return false;
    }
    match family {
        DroidFamily::Cyborg | DroidFamily::SuperCyborg => {
            is_legged(prop) && usage_matches(family, prop.usage_class.as_deref())
        }
        DroidFamily::Transporter => propulsion_medium(prop) == PropulsionMedium::Air,
        DroidFamily::Standard => !is_legged(prop),
    }
}

pub fn weapon_allowed(weapon: &WeaponStats, family: DroidFamily) -> bool {
    if !weapon.designable {
        return false;
    }
    usage_matches(family, weapon.usage_class.as_deref())
}

/// VTOL weapons (`numAttackRuns > 0`) require LIFT propulsion; ground
/// weapons can't mount on VTOL propulsion.
pub fn weapon_fits_propulsion(weapon: &WeaponStats, medium: PropulsionMedium) -> bool {
    matches!(
        (weapon.is_vtol(), medium),
        (true, PropulsionMedium::Air)
            | (false, PropulsionMedium::Ground)
            | (_, PropulsionMedium::Unknown)
    )
}

/// Broad propulsion medium derived from `PropulsionStats.propulsion_type`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PropulsionMedium {
    /// Wheeled / tracked / half-tracked / hover / legged.
    Ground,
    /// VTOL / transporter (`LIFT`).
    Air,
    /// Unrecognised `propulsion_type` string.
    Unknown,
}

pub fn propulsion_medium(stats: &PropulsionStats) -> PropulsionMedium {
    match stats
        .propulsion_type
        .as_deref()
        .map(str::to_ascii_uppercase)
    {
        Some(ref s) if s == "LIFT" => PropulsionMedium::Air,
        Some(ref s)
            if matches!(
                s.as_str(),
                "WHEELED" | "TRACKED" | "HALF-TRACKED" | "HOVER" | "LEGGED"
            ) =>
        {
            PropulsionMedium::Ground
        }
        _ => PropulsionMedium::Unknown,
    }
}

/// True for the legged/cyborg propulsion type that cyborg bodies require.
pub fn is_legged(stats: &PropulsionStats) -> bool {
    stats
        .propulsion_type
        .as_deref()
        .is_some_and(|s| s.eq_ignore_ascii_case("LEGGED"))
}

/// Errors block saving; warnings let the user proceed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
}

/// Which slot the issue is about. Drives picker highlighting in the UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Slot {
    Body,
    Propulsion,
    Weapon(u8),
    Brain,
    Sensor,
    Ecm,
    Repair,
    Construct,
    /// Concerns the whole template (e.g. name/type).
    General,
}

#[derive(Debug, Clone)]
pub struct Issue {
    pub severity: Severity,
    pub slot: Slot,
    pub message: String,
}

impl Issue {
    fn error(slot: Slot, msg: impl Into<String>) -> Self {
        Self {
            severity: Severity::Error,
            slot,
            message: msg.into(),
        }
    }
    fn warn(slot: Slot, msg: impl Into<String>) -> Self {
        Self {
            severity: Severity::Warning,
            slot,
            message: msg.into(),
        }
    }
}

/// Max weapon slots this body allows.
pub fn max_weapon_slots(body: &BodyStats) -> u8 {
    body.weapon_slot_count()
}

pub fn propulsion_compatible(body: &BodyStats, prop: &PropulsionStats) -> Result<(), String> {
    if body.is_cyborg() && !is_legged(prop) {
        return Err("Cyborg bodies require legged propulsion".into());
    }
    if !body.is_cyborg() && is_legged(prop) {
        return Err("Legged propulsion is only for cyborg bodies".into());
    }
    Ok(())
}

/// Returns `Some(slot)` when the droid type name implies a required
/// non-weapon turret fitting.
pub fn required_turret_for_type(droid_type: Option<&str>) -> Option<Slot> {
    let t = droid_type?.to_ascii_uppercase();
    match t.as_str() {
        "CONSTRUCT" => Some(Slot::Construct),
        "SENSOR" => Some(Slot::Sensor),
        "ECM" => Some(Slot::Ecm),
        "REPAIR" => Some(Slot::Repair),
        "DROID_COMMAND" | "COMMAND" => Some(Slot::Brain),
        _ => None,
    }
}

/// Validate a template against the stats database. Returns every issue
/// found so the UI can display them all. Empty vec = save-ready.
pub fn validate(template: &TemplateStats, db: &StatsDatabase) -> Vec<Issue> {
    let mut issues = Vec::new();

    let Some(body) = db.bodies.get(&template.body) else {
        issues.push(Issue::error(
            Slot::Body,
            if template.body.is_empty() {
                "No body selected".to_string()
            } else {
                format!("Unknown body '{}'", template.body)
            },
        ));
        return issues;
    };

    let Some(prop) = db.propulsion.get(&template.propulsion) else {
        issues.push(Issue::error(
            Slot::Propulsion,
            if template.propulsion.is_empty() {
                "No propulsion selected".to_string()
            } else {
                format!("Unknown propulsion '{}'", template.propulsion)
            },
        ));
        return issues;
    };

    if let Err(msg) = propulsion_compatible(body, prop) {
        issues.push(Issue::error(Slot::Propulsion, msg));
    }

    let family = droid_family(template.droid_type.as_deref());
    if !body_allowed(body, family) {
        issues.push(Issue::error(
            Slot::Body,
            format!(
                "Body '{}' (class {}) is not allowed for droid type '{}'",
                template.body,
                body.body_class.as_deref().unwrap_or("Droids"),
                template.droid_type.as_deref().unwrap_or("?"),
            ),
        ));
    }
    if body.is_scavenger() || !body.designable {
        issues.push(Issue::error(
            Slot::Body,
            format!("Body '{}' is not player-buildable", template.body),
        ));
    }
    if !propulsion_allowed(prop, family) {
        issues.push(Issue::error(
            Slot::Propulsion,
            format!(
                "Propulsion '{}' is not allowed for droid type '{}'",
                template.propulsion,
                template.droid_type.as_deref().unwrap_or("?"),
            ),
        ));
    }
    let medium = propulsion_medium(prop);
    for (i, w_id) in template.weapons.iter().enumerate() {
        if w_id.is_empty() {
            continue;
        }
        if let Some(w) = db.weapons.get(w_id) {
            if !weapon_allowed(w, family) {
                issues.push(Issue::error(
                    Slot::Weapon(i as u8),
                    format!(
                        "Weapon '{w_id}' is not selectable for droid type '{}'",
                        template.droid_type.as_deref().unwrap_or("?"),
                    ),
                ));
            }
            if !weapon_fits_propulsion(w, medium) {
                let hint = if w.is_vtol() {
                    "VTOL weapon requires VTOL propulsion"
                } else {
                    "Ground weapon cannot be used on VTOL propulsion"
                };
                issues.push(Issue::error(Slot::Weapon(i as u8), hint.to_string()));
            }
        }
    }

    let slots = max_weapon_slots(body);
    if template.weapons.len() > slots as usize {
        issues.push(Issue::error(
            Slot::Weapon(slots),
            format!(
                "Body '{}' has {} weapon slot(s); template has {}",
                template.body,
                slots,
                template.weapons.len()
            ),
        ));
    }
    for (i, w) in template.weapons.iter().enumerate() {
        if !w.is_empty() && !db.weapons.contains_key(w) {
            issues.push(Issue::error(
                Slot::Weapon(i as u8),
                format!("Unknown weapon '{w}'"),
            ));
        }
    }

    if let Some(ref s) = template.sensor
        && !db.sensor.contains_key(s)
    {
        issues.push(Issue::error(Slot::Sensor, format!("Unknown sensor '{s}'")));
    }
    if let Some(ref s) = template.ecm
        && !db.ecm.contains_key(s)
    {
        issues.push(Issue::error(Slot::Ecm, format!("Unknown ECM '{s}'")));
    }
    if let Some(ref s) = template.repair
        && !db.repair.contains_key(s)
    {
        issues.push(Issue::error(
            Slot::Repair,
            format!("Unknown repair turret '{s}'"),
        ));
    }
    if let Some(ref s) = template.construct
        && !db.construct.contains_key(s)
    {
        issues.push(Issue::error(
            Slot::Construct,
            format!("Unknown constructor '{s}'"),
        ));
    }
    if let Some(ref s) = template.brain
        && !db.brain.contains_key(s)
    {
        issues.push(Issue::error(Slot::Brain, format!("Unknown brain '{s}'")));
    }

    if let Some(required) = required_turret_for_type(template.droid_type.as_deref()) {
        let (present, slot_name) = match required {
            Slot::Construct => (template.construct.is_some(), "construction turret"),
            Slot::Sensor => (template.sensor.is_some(), "sensor turret"),
            Slot::Ecm => (template.ecm.is_some(), "ECM turret"),
            Slot::Repair => (template.repair.is_some(), "repair turret"),
            Slot::Brain => (template.brain.is_some(), "commander brain"),
            _ => (true, ""),
        };
        if !present {
            issues.push(Issue::error(
                required,
                format!(
                    "Droid type '{}' requires a {slot_name}",
                    template.droid_type.as_deref().unwrap_or("?")
                ),
            ));
        }
    }

    if template.brain.is_some() && template.weapons.is_empty() {
        issues.push(Issue::error(
            Slot::Weapon(0),
            "Commander droids need at least one weapon",
        ));
    }

    if matches!(
        template.droid_type.as_deref(),
        Some("TRANSPORTER" | "SUPERTRANSPORTER")
    ) && propulsion_medium(prop) != PropulsionMedium::Air
    {
        issues.push(Issue::error(
            Slot::Propulsion,
            "Transporters require VTOL (LIFT) propulsion",
        ));
    }

    let has_non_brain_system = template.sensor.is_some()
        || template.ecm.is_some()
        || template.repair.is_some()
        || template.construct.is_some();
    if medium == PropulsionMedium::Air && has_non_brain_system {
        issues.push(Issue::error(
            Slot::General,
            "VTOL droids cannot mount system turrets".to_string(),
        ));
    }

    let has_weapons = template.weapons.iter().any(|w| !w.is_empty());
    if has_weapons && has_non_brain_system {
        issues.push(Issue::error(
            Slot::General,
            "Cannot have both weapons and a system turret (sensor/ECM/repair/construct)"
                .to_string(),
        ));
    }

    let has_any_turret = has_weapons || has_non_brain_system || template.brain.is_some();
    if !has_any_turret {
        issues.push(Issue::warn(
            Slot::General,
            "Droid has no turrets, will spawn unarmed",
        ));
    }

    issues
}

/// True when no issue has Error severity.
pub fn is_save_ready(issues: &[Issue]) -> bool {
    !issues.iter().any(|i| i.severity == Severity::Error)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn db_with(body: BodyStats, prop: PropulsionStats, weapons: Vec<WeaponStats>) -> StatsDatabase {
        let mut db = StatsDatabase::default();
        db.bodies.insert(body.id.clone(), body);
        db.propulsion.insert(prop.id.clone(), prop);
        for w in weapons {
            db.weapons.insert(w.id.clone(), w);
        }
        db
    }

    fn body(id: &str, slots: u8, class: Option<&str>) -> BodyStats {
        BodyStats {
            id: id.into(),
            name: None,
            model: None,
            hitpoints: Some(100),
            weight: Some(100),
            size: None,
            build_power: Some(100),
            armour_kinetic: None,
            armour_heat: None,
            weapon_slots: Some(slots),
            body_class: class.map(str::to_string),
            designable: true,
            usage_class: None,
            propulsion_extra_models: HashMap::default(),
        }
    }

    fn prop(id: &str, kind: &str) -> PropulsionStats {
        PropulsionStats {
            id: id.into(),
            name: None,
            model: None,
            speed: Some(10),
            weight: Some(50),
            propulsion_type: Some(kind.into()),
            designable: true,
            usage_class: None,
        }
    }

    fn weapon(id: &str) -> WeaponStats {
        WeaponStats {
            id: id.into(),
            name: None,
            model: None,
            mount_model: None,
            short_range: Some(100),
            long_range: Some(200),
            damage: Some(10),
            designable: true,
            usage_class: None,
            num_attack_runs: None,
        }
    }

    fn tmpl(body: &str, prop: &str, weapons: &[&str]) -> TemplateStats {
        TemplateStats {
            id: "T".into(),
            body: body.into(),
            propulsion: prop.into(),
            weapons: weapons.iter().map(|s| (*s).to_string()).collect(),
            name: Some("T".into()),
            droid_type: Some("WEAPON".into()),
            construct: None,
            sensor: None,
            repair: None,
            ecm: None,
            brain: None,
        }
    }

    #[test]
    fn valid_basic_template_has_no_errors() {
        let db = db_with(body("B", 2, None), prop("P", "WHEELED"), vec![weapon("W")]);
        let t = tmpl("B", "P", &["W"]);
        let issues = validate(&t, &db);
        assert!(is_save_ready(&issues), "got: {issues:?}");
    }

    #[test]
    fn weapon_slot_overflow_errors() {
        let db = db_with(
            body("B", 1, None),
            prop("P", "WHEELED"),
            vec![weapon("W1"), weapon("W2")],
        );
        let t = tmpl("B", "P", &["W1", "W2"]);
        let issues = validate(&t, &db);
        assert!(!is_save_ready(&issues));
    }

    #[test]
    fn cyborg_requires_legged_propulsion() {
        let db = db_with(
            body("CyBody", 1, Some("Cyborgs")),
            prop("Wheels", "WHEELED"),
            vec![weapon("W")],
        );
        let t = tmpl("CyBody", "Wheels", &["W"]);
        let issues = validate(&t, &db);
        assert!(!is_save_ready(&issues));
    }

    #[test]
    fn legged_rejected_for_non_cyborg() {
        let db = db_with(
            body("Tank", 1, None),
            prop("Legs", "LEGGED"),
            vec![weapon("W")],
        );
        let t = tmpl("Tank", "Legs", &["W"]);
        let issues = validate(&t, &db);
        assert!(!is_save_ready(&issues));
    }

    #[test]
    fn construct_droid_needs_constructor() {
        let db = db_with(body("B", 1, None), prop("P", "WHEELED"), vec![]);
        let mut t = tmpl("B", "P", &[]);
        t.droid_type = Some("CONSTRUCT".into());
        let issues = validate(&t, &db);
        assert!(!is_save_ready(&issues));
    }

    #[test]
    fn unknown_body_errors() {
        let db = db_with(body("B", 1, None), prop("P", "WHEELED"), vec![weapon("W")]);
        let t = tmpl("Missing", "P", &["W"]);
        let issues = validate(&t, &db);
        assert!(!is_save_ready(&issues));
    }

    #[test]
    fn empty_droid_produces_warning_not_error() {
        let db = db_with(body("B", 1, None), prop("P", "WHEELED"), vec![]);
        let t = tmpl("B", "P", &[]);
        let issues = validate(&t, &db);
        assert!(is_save_ready(&issues));
        assert!(issues.iter().any(|i| i.severity == Severity::Warning));
    }
}
