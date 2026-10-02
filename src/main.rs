//! Camera Stats — EXIF statistics for a photo library.
//!
//! This file is only the entry point. The code lives in focused modules:
//!
//! | Module | Responsibility |
//! |---|---|
//! | [`app`] | Application state and the `eframe` update loop |
//! | [`config`] | User display preferences (charts, tables, EXIF columns) |
//! | [`table`] | Photo-table sorting |
//! | [`filter`] | Filtering predicate, filter options, filter dropdown |
//! | [`analytics`] | Aggregation of photos into chart data |
//! | [`ui`] | `egui` drawing code, one module per panel |
//!
//! The EXIF decoding itself lives in the `camera_stats` library crate
//! ([`camera_stats`]), which is separate so it can be unit tested without
//! starting a window.

mod analytics;
mod app;
mod config;
mod filter;
mod table;
mod ui;

fn main() -> anyhow::Result<()> {
    app::run()
}
