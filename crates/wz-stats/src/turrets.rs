//! Non-weapon turret stats: construction, sensor, repair, brain.
//!
//! Each has PIE model references used to render the turret on droids.

use std::collections::HashMap;

use serde::Deserialize;

/// Construction turret stats (from construction.json).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConstructStats {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub name: Option<String>,
    /// Turret PIE model. Named `sensorModel` in WZ2100 data despite not being a sensor.
    #[serde(default, rename = "sensorModel")]
    pub sensor_model: Option<String>,
    #[serde(default, rename = "mountModel")]
    pub mount_model: Option<String>,
    #[serde(default, deserialize_with = "crate::bodies::deserialize_bool_int")]
    pub designable: bool,
    #[serde(default, rename = "usageClass")]
    pub usage_class: Option<String>,
}

/// Sensor turret stats (from sensor.json).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SensorStats {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default, rename = "sensorModel")]
    pub sensor_model: Option<String>,
    #[serde(default, rename = "mountModel")]
    pub mount_model: Option<String>,
    #[serde(default, deserialize_with = "crate::bodies::deserialize_bool_int")]
    pub designable: bool,
    #[serde(default, rename = "usageClass")]
    pub usage_class: Option<String>,
    /// `TURRET` for droid-mountable sensors, `DEFAULT`/`WALL` for
    /// structure-only sensors.
    #[serde(default)]
    pub location: Option<String>,
}

/// Repair turret stats (from repair.json).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RepairStats {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default, rename = "mountModel")]
    pub mount_model: Option<String>,
    #[serde(default, deserialize_with = "crate::bodies::deserialize_bool_int")]
    pub designable: bool,
    #[serde(default, rename = "usageClass")]
    pub usage_class: Option<String>,
    #[serde(default)]
    pub location: Option<String>,
}

/// ECM turret stats. Used by repair facilities and similar structures
/// that mount a turret via `ecmID` rather than a weapon or sensor.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EcmStats {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub name: Option<String>,
    /// Turret PIE model. Named `sensorModel` in WZ2100 data despite not being a sensor.
    #[serde(default, rename = "sensorModel")]
    pub sensor_model: Option<String>,
    #[serde(default, rename = "mountModel")]
    pub mount_model: Option<String>,
    #[serde(default, deserialize_with = "crate::bodies::deserialize_bool_int")]
    pub designable: bool,
    #[serde(default, rename = "usageClass")]
    pub usage_class: Option<String>,
    #[serde(default)]
    pub location: Option<String>,
}

/// Brain/commander stats (from brain.json).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrainStats {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub name: Option<String>,
    /// Weapon stat name used as the command turret.
    #[serde(default)]
    pub turret: Option<String>,
    #[serde(default, deserialize_with = "crate::bodies::deserialize_bool_int")]
    pub designable: bool,
    #[serde(default, rename = "usageClass")]
    pub usage_class: Option<String>,
}

pub fn load_construct(
    json_str: &str,
) -> Result<HashMap<String, ConstructStats>, crate::StatsError> {
    crate::loaders::load_stat_map(json_str, "construction.json", |_, _| {})
}

pub fn load_sensor(json_str: &str) -> Result<HashMap<String, SensorStats>, crate::StatsError> {
    crate::loaders::load_stat_map(json_str, "sensor.json", |_, _| {})
}

pub fn load_repair(json_str: &str) -> Result<HashMap<String, RepairStats>, crate::StatsError> {
    crate::loaders::load_stat_map(json_str, "repair.json", |_, _| {})
}

pub fn load_brain(json_str: &str) -> Result<HashMap<String, BrainStats>, crate::StatsError> {
    crate::loaders::load_stat_map(json_str, "brain.json", |_, _| {})
}

pub fn load_ecm(json_str: &str) -> Result<HashMap<String, EcmStats>, crate::StatsError> {
    crate::loaders::load_stat_map(json_str, "ecm.json", |_, _| {})
}
