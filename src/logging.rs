use std::path::Path;
use tracing::{debug, error, info, warn};
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

pub fn init_logging(log_dir: Option<&Path>) -> Result<Option<WorkerGuard>, anyhow::Error> {
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,camera_stats=debug"));

    let fmt_layer = fmt::layer()
        .with_target(true)
        .with_thread_ids(true)
        .with_thread_names(true)
        .with_file(true)
        .with_line_number(true);

    if let Some(dir) = log_dir {
        std::fs::create_dir_all(dir)?;
        let file_appender = tracing_appender::rolling::daily(dir, "camera_stats.log");
        let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);
        let file_layer = fmt_layer.with_writer(non_blocking).with_ansi(false);

        tracing_subscriber::registry()
            .with(env_filter)
            .with(file_layer)
            .init();

        info!("Logging initialized to file: {:?}", dir);
        Ok(Some(guard))
    } else {
        let stdout_layer = fmt_layer.with_writer(std::io::stdout).with_ansi(true);

        tracing_subscriber::registry()
            .with(env_filter)
            .with(stdout_layer)
            .init();

        info!("Logging initialized to stdout");
        Ok(None)
    }
}

pub fn log_scan_start(dir: &Path, file_count: usize) {
    info!(target: "scan", "Starting scan of {:?} ({} files)", dir, file_count);
}

pub fn log_scan_complete(dir: &Path, found: usize, duration_ms: u128) {
    info!(target: "scan", "Completed scan of {:?}: found {} photos in {}ms", dir, found, duration_ms);
}

pub fn log_scan_error(dir: &Path, err: &anyhow::Error) {
    error!(target: "scan", "Scan failed for {:?}: {}", dir, err);
}

pub fn log_exif_extract(path: &Path, success: bool, err: Option<&anyhow::Error>) {
    if success {
        debug!(target: "exif", "Extracted EXIF from {:?}", path);
    } else if let Some(e) = err {
        warn!(target: "exif", "Failed to extract EXIF from {:?}: {}", path, e);
    }
}

pub fn log_ui_event(event: &str) {
    info!(target: "ui", "{}", event);
}

/// Log a display setting the user changed.
///
/// Recorded at `debug` so that turning on `RUST_LOG=debug` records a replayable
/// trace of every setting change, without spamming the default log.
pub fn log_setting_changed(setting: &str, value: impl std::fmt::Display) {
    debug!(target: "ui", "Setting changed: {setting} = {value}");
}

/// Log how many photos survived filtering, and how many were dropped.
///
/// This is the first thing to look at when a table "looks empty": it separates
/// "the filter excluded everything" from "no photos were ever loaded".
pub fn log_filter_result(shown: usize, total: usize, filters: &str) {
    info!(target: "ui", "Filter {filters} -> {shown} of {total} photos shown");
}

/// Log the aggregated statistics for a completed scan.
///
/// The distinct-value counts here are the quickest way to confirm that EXIF
/// extraction actually worked, without opening a single file.
pub fn log_stats_summary(
    iso_values: usize,
    aperture_values: usize,
    focal_values: usize,
    cameras: usize,
    lenses: usize,
) {
    info!(
        target: "scan",
        "Stats: {iso_values} ISO values, {aperture_values} apertures, {focal_values} focal lengths, {cameras} cameras, {lenses} lenses"
    );
}
