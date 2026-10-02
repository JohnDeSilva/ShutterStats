//! All `egui` view code.
//!
//! The app struct in [`crate::app`] owns state and the update loop; everything
//! here is a free function that draws one part of the window. That split means
//! a view never has to hold a borrow across a mutable app field, which is the
//! usual source of borrow-checker friction in immediate-mode UI.
//!
//! - [`toolbar`] — folder selection, export, settings, status
//! - [`charts`] — ISO / aperture / focal-length distributions
//! - [`photo_table`] — sortable, filterable photo rows
//! - [`settings`] — display preferences and EXIF column visibility

pub mod charts;
pub mod photo_table;
pub mod settings;
pub mod toolbar;

/// Default text colour, used where a module needs a neutral foreground without
/// depending on the app's theme state.
pub const TEXT_COLOR: egui::Color32 = egui::Color32::from_rgb(0xD0, 0xD0, 0xD0);
