//! Droid Designer: custom template authoring, validation, and UI.
//!
//! Lets users assemble custom droid templates (body + propulsion + up to
//! three weapon turrets, plus optional sensor / ECM / repair / construct /
//! brain) without editing JSON. Templates live in [`CustomTemplateStore`]
//! and round-trip through the map archive as `templates.json`.

pub mod custom_templates;
pub mod state;
pub mod tabs;
pub mod ui;
pub mod validation;

pub use custom_templates::CustomTemplateStore;
pub use state::Designer;
