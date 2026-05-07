//! State container types for the droid designer.

use eframe::egui_wgpu;
use wz_stats::StatsDatabase;
use wz_stats::templates::TemplateStats;

use crate::designer::custom_templates::CustomTemplateStore;
use crate::designer::tabs::DesignerTabs;
use crate::designer::validation::Issue;
use crate::thumbnails::ThumbnailCache;
use crate::viewport::model_loader::ModelLoader;

/// Which slot the user is currently editing in the grid.
///
/// WZ2100 droids have four physical mount points: chassis, legs, weapon
/// turret(s), and a secondary-system turret (sensor, ECM, repair,
/// construct, or commander brain). The five system-turret kinds are
/// mutually exclusive, so the designer collapses them into a single
/// "Systems" tab where picking one replaces whatever else was equipped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SlotTab {
    Body,
    Propulsion,
    Weapon(u8),
    /// Unified sensor / ECM / repair / construct / brain slot.
    Turret,
}

/// Sub-category within the unified `Turret` slot. Tells the UI which
/// `TemplateStats` field to read or write when the user picks an option.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TurretKind {
    Sensor,
    Ecm,
    Repair,
    Construct,
    Brain,
}

impl SlotTab {
    /// Unicode glyph fallback when the WZ2100 sprite can't be loaded
    /// (e.g. data dir not configured yet).
    pub fn icon(self) -> &'static str {
        match self {
            Self::Body => "\u{1F9CD}",      // bust in silhouette
            Self::Propulsion => "\u{2699}", // gear
            Self::Weapon(_) => "\u{2694}",  // crossed swords
            Self::Turret => "\u{1F4E1}",    // satellite antenna
        }
    }

    /// Filename of the WZ2100 design-screen sprite for this slot.
    /// Resolved against `data/base/images/intfac/<filename>`.
    pub fn sprite_filename(self) -> &'static str {
        match self {
            Self::Body => "image_des_body.png",
            Self::Propulsion => "image_des_propulsion.png",
            Self::Weapon(_) => "image_des_weapons.png",
            // The in-game design screen groups sensor/ECM/repair/construct
            // under "Systems"; we mirror that.
            Self::Turret => "image_des_systems.png",
        }
    }

    pub fn label(self) -> String {
        match self {
            Self::Body => "Body".into(),
            Self::Propulsion => "Propulsion".into(),
            Self::Weapon(i) => format!("W{}", i + 1),
            Self::Turret => "Systems".into(),
        }
    }

    pub fn tooltip(self) -> &'static str {
        match self {
            Self::Body => "Chassis: hitpoints, armour, weapon slots",
            Self::Propulsion => "Locomotion: wheels, tracks, hover, VTOL",
            Self::Weapon(_) => "Weapon turret",
            Self::Turret => "Sensor, ECM, repair, construct, or commander",
        }
    }
}

/// Outcome of a designer frame, so the caller can react (e.g. re-point
/// a placed droid after a Save-as-new).
#[derive(Debug, Default)]
pub struct DesignerOutcome {
    /// Id of the template just saved (inserted or overwritten).
    pub saved_template_id: Option<String>,
    pub cancelled: bool,
}

/// Shared dependencies the designer borrows from `EditorApp`.
pub struct DesignerCtx<'a> {
    pub db: &'a mut StatsDatabase,
    pub store: &'a mut CustomTemplateStore,
    pub thumbnails: &'a mut ThumbnailCache,
    pub model_loader: &'a mut Option<ModelLoader>,
    pub render_state: Option<&'a egui_wgpu::RenderState>,
    /// Used to load WZ2100 UI sprites from `data/base/images/intfac/`.
    pub data_dir: Option<&'a std::path::Path>,
}

/// Transient, per-window state for the designer.
pub struct Designer {
    pub open: bool,
    /// The template currently being edited.
    pub buffer: TemplateStats,
    pub tabs: DesignerTabs,
    /// Existing template's id when editing in place. `None` means Save
    /// allocates a fresh id.
    pub editing_id: Option<String>,
    /// Set when opened from a placed droid's property panel. Save then
    /// re-points the droid at the newly-created template.
    pub retarget_droid_index: Option<usize>,
    /// Issues from the most recent validation pass.
    pub issues: Vec<Issue>,
    pub name_buf: String,
    /// Most-recent Save error, shown inline.
    pub last_save_error: Option<String>,
    /// Live preview's y-axis rotation in radians, advanced each frame.
    pub preview_rotation: f32,
    /// WZ2100 sprites keyed by filename, loaded lazily on first draw.
    pub icon_cache: std::collections::HashMap<&'static str, egui::TextureHandle>,
}

impl Default for Designer {
    fn default() -> Self {
        Self {
            open: false,
            buffer: empty_template(),
            tabs: DesignerTabs::default(),
            editing_id: None,
            retarget_droid_index: None,
            issues: Vec::new(),
            name_buf: String::new(),
            last_save_error: None,
            preview_rotation: 0.0,
            icon_cache: std::collections::HashMap::new(),
        }
    }
}

impl Designer {
    /// Look up the WZ2100 sprite for a slot icon, loading on first call.
    /// Returns `None` when the data dir isn't configured or the PNG can't
    /// be decoded; the slot button then falls back to its glyph.
    pub fn icon_for(
        &mut self,
        slot: SlotTab,
        ctx: &egui::Context,
        data_dir: Option<&std::path::Path>,
    ) -> Option<egui::TextureHandle> {
        let filename = slot.sprite_filename();
        if let Some(tex) = self.icon_cache.get(filename) {
            return Some(tex.clone());
        }
        let dir = data_dir?;
        let path = dir
            .join("base")
            .join("images")
            .join("intfac")
            .join(filename);
        let img = image::open(&path).ok()?.into_rgba8();
        let size = [img.width() as usize, img.height() as usize];
        let pixels = img.into_raw();
        let color_image = egui::ColorImage::from_rgba_unmultiplied(size, &pixels);
        let tex = ctx.load_texture(
            format!("designer_icon_{filename}"),
            color_image,
            egui::TextureOptions::LINEAR,
        );
        self.icon_cache.insert(filename, tex.clone());
        Some(tex)
    }

    /// Open the designer with a blank buffer (menu entry point).
    #[expect(dead_code, reason = "Droid Designer is temporarily disabled")]
    pub fn open_fresh(&mut self, db: &StatsDatabase) {
        use crate::designer::validation::{self, DroidFamily};
        self.buffer = empty_template();
        // Defaults: first designable Standard body and matching propulsion.
        if let Some((body_id, _)) = db
            .bodies
            .iter()
            .find(|(_, b)| validation::body_selectable(b, DroidFamily::Standard))
        {
            self.buffer.body.clone_from(body_id);
        }
        if let Some((prop_id, _)) = db
            .propulsion
            .iter()
            .find(|(_, p)| validation::propulsion_allowed(p, DroidFamily::Standard))
        {
            self.buffer.propulsion.clone_from(prop_id);
        }
        if let Some((w_id, _)) = db
            .weapons
            .iter()
            .find(|(_, w)| validation::weapon_allowed(w, DroidFamily::Standard))
        {
            self.buffer.weapons = vec![w_id.clone()];
        }
        self.name_buf = "New Droid".to_string();
        self.buffer.name = Some(self.name_buf.clone());
        self.tabs.active_slot = SlotTab::Body;
        self.editing_id = None;
        self.retarget_droid_index = None;
        self.last_save_error = None;
        self.open = true;
    }

    /// Open the designer pre-filled from an existing template. Saving
    /// creates a NEW template (copy-on-edit); the source is untouched.
    #[expect(dead_code, reason = "Droid Designer is temporarily disabled")]
    pub fn open_with_template(&mut self, src: &TemplateStats) {
        self.buffer = src.clone();
        self.buffer.id = String::new();
        self.name_buf = src
            .name
            .clone()
            .unwrap_or_else(|| src.display_name().to_string());
        self.buffer.name = Some(self.name_buf.clone());
        self.tabs.active_slot = SlotTab::Body;
        self.editing_id = None;
        self.retarget_droid_index = None;
        self.last_save_error = None;
        self.open = true;
    }
}

pub(crate) fn empty_template() -> TemplateStats {
    TemplateStats {
        id: String::new(),
        body: String::new(),
        propulsion: String::new(),
        weapons: Vec::new(),
        name: None,
        droid_type: Some("WEAPON".into()),
        construct: None,
        sensor: None,
        repair: None,
        ecm: None,
        brain: None,
    }
}
