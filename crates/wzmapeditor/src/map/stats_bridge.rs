//! Adapter implementing [`StatsLookup`] for [`StatsDatabase`].

use wz_maplib::validate::{FeatureInfo, StatsLookup, StructureInfo, TemplateInfo};
use wz_stats::StatsDatabase;

/// Adapts `StatsDatabase` to the validation trait.
pub struct StatsBridge<'a>(pub &'a StatsDatabase);

impl StatsLookup for StatsBridge<'_> {
    fn structure_info(&self, name: &str) -> Option<StructureInfo> {
        self.0.structures.get(name).map(|s| StructureInfo {
            structure_type: s.structure_type.clone(),
            width: s.width.unwrap_or(1),
            breadth: s.breadth.unwrap_or(1),
        })
    }

    fn feature_info(&self, name: &str) -> Option<FeatureInfo> {
        self.0.features.get(name).map(|f| FeatureInfo {
            feature_type: f.feature_type.clone(),
        })
    }

    fn template_info(&self, name: &str) -> Option<TemplateInfo> {
        self.0.templates.get(name).map(|t| TemplateInfo {
            droid_type: t.droid_type.clone(),
            has_construct: t.construct.is_some(),
        })
    }
}
