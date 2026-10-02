//! Application state and the `eframe` entry point.
//!
//! All state lives on [`CameraStatsApp`]. It is built once at startup and then
//! only mutated from the UI, except for [`CameraStatsApp::photos`], which is
//! written once by the background scan worker and handed over through a
//! channel.
//!
//! The `eframe::App::update` method is kept short on purpose: it decides *what*
//! to draw this frame and delegates the drawing to [`crate::ui`].

use std::path::PathBuf;
use std::sync::mpsc;
use std::time::Instant;

use anyhow::Result;
use camera_stats::logging::{init_logging, log_filter_result, log_scan_complete, log_ui_event};
use camera_stats::{PhotoMeta, Stats};
use eframe::egui;
use tracing::info;

use crate::config::DisplayConfig;
use crate::table::TableState;
use crate::ui;

/// Polling interval while a scan is running.
///
/// Long enough not to spin the CPU, short enough that the result appears to
/// arrive immediately after the last file is decoded.
const SCAN_POLL_INTERVAL: std::time::Duration = std::time::Duration::from_millis(100);

/// Top-level application state.
pub struct CameraStatsApp {
    /// Photos loaded from the selected folder, populated by the scan worker.
    pub photos: Vec<PhotoMeta>,
    /// Aggregated statistics, recomputed whenever `photos` changes.
    pub stats: Stats,

    /// Folder the user picked, if any. Kept for display and for the log.
    pub selected_dir: Option<PathBuf>,
    /// One-line status shown at the right of the toolbar.
    pub status: String,
    /// True while the background scan is running.
    pub scanning: bool,
    /// Receiver for the scan worker's result; `None` when idle.
    pub scan_rx: Option<mpsc::Receiver<Vec<PhotoMeta>>>,
    /// When the current scan started, for the elapsed-time log line.
    pub scan_start: Option<Instant>,

    /// Kept alive for the process lifetime: dropping it flushes and closes the
    /// log file, so losing this field would truncate the final log lines.
    #[allow(dead_code)]
    log_guard: Option<tracing_appender::non_blocking::WorkerGuard>,

    /// Whether the export file dialog should be shown.
    pub show_export_dialog: bool,
    /// Whether the settings window is open.
    pub show_settings: bool,

    /// User display preferences.
    pub display_config: DisplayConfig,
    /// Sort and filter state for the photo table.
    pub table_state: TableState,
}

impl Default for CameraStatsApp {
    fn default() -> Self {
        // Logs go to the platform data directory, falling back to stdout if it
        // cannot be created. A failure to set up logging must not stop the app.
        let log_dir = dirs::data_local_dir().map(|d| d.join("camera_stats").join("logs"));
        let log_guard = init_logging(log_dir.as_deref())
            .ok()
            .flatten()
            .inspect(|_| {
                log_ui_event(&format!(
                    "Logging to {:?}",
                    log_dir.as_deref().unwrap_or(&PathBuf::from("stdout"))
                ));
            });

        info!(target: "ui", "Camera Stats starting");

        Self {
            photos: Vec::new(),
            stats: Stats::default(),
            selected_dir: None,
            status: "Select a folder to scan".into(),
            scanning: false,
            scan_rx: None,
            scan_start: None,
            log_guard,
            show_export_dialog: false,
            show_settings: false,
            display_config: DisplayConfig::default(),
            table_state: TableState::default(),
        }
    }
}

impl CameraStatsApp {
    /// Record a user interaction.
    ///
    /// Funnelled through one method so every interaction lands in the log with
    /// the same `ui` target and level.
    pub fn log_ui(&self, message: &str) {
        log_ui_event(message);
    }

    /// Begin a scan of `dir`, replacing any scan already in progress.
    ///
    /// `dir` is stored for display before the worker starts so the toolbar
    /// updates on the very next frame rather than after the first file.
    pub fn start_scan(&mut self, dir: &PathBuf) {
        self.selected_dir = Some(dir.clone());
        self.status = format!("Scanning {}...", dir.display());
        self.scanning = true;
        self.scan_start = Some(Instant::now());
        self.scan_rx = Some(ui::toolbar::spawn_scan(dir.clone()));
        self.log_ui(&format!("scan started for {:?}", dir));
    }

    /// Poll the scan worker, folding a finished result into the app state.
    ///
    /// Returns `true` when photos were just loaded. Polling is non-blocking:
    /// `try_recv` yields `Err` while the worker is still running, which is not
    /// an error condition.
    fn poll_scan(&mut self, ctx: &egui::Context) -> bool {
        let Some(rx) = &self.scan_rx else {
            return false;
        };

        let Ok(photos) = rx.try_recv() else {
            // Still working: ask for another frame soon so the result is picked
            // up promptly instead of waiting for unrelated user input.
            ctx.request_repaint_after(SCAN_POLL_INTERVAL);
            return false;
        };

        let duration = self
            .scan_start
            .map(|s| s.elapsed().as_millis())
            .unwrap_or(0);
        if let Some(dir) = &self.selected_dir {
            log_scan_complete(dir, photos.len(), duration);
        }

        let kept = photos.len();
        self.photos = photos;
        self.stats = Stats::from_photos(&self.photos);
        self.status = format!("Found {kept} photos in {duration} ms");
        self.scanning = false;
        self.scan_rx = None;
        self.scan_start = None;

        // Log the distinct-value counts: if these are all zero, EXIF extraction
        // silently produced empty records and the charts will look broken.
        camera_stats::logging::log_stats_summary(
            self.stats.iso_counts.len(),
            self.stats.aperture_counts.len(),
            self.stats.focal_length_counts.len(),
            self.stats.camera_counts.len(),
            self.stats.lens_counts.len(),
        );
        log_filter_result(kept, kept, "none (unfiltered)");

        true
    }

    /// The empty-state panel shown before a folder has been scanned.
    fn render_welcome(&self, ui: &mut egui::Ui) {
        ui.centered_and_justified(|ui| {
            ui.heading("📸 Camera Stats");
            ui.add_space(10.0);
            ui.label("Select a folder containing photos to analyze EXIF data");
        });
    }

    /// The main content area once photos are loaded.
    fn render_content(&mut self, ui: &mut egui::Ui) {
        // Cloned up front because the views need `&mut self.table_state` for
        // their widgets while reading `&self.photos` for their data, which the
        // borrow checker will not allow from the same `self`.
        let filters = crate::filter::PhotoFilters::from_state(&self.table_state);
        let columns = self.display_config.photo_table_columns.clone();
        let max_rows = self.display_config.max_table_rows;

        ui.heading("📊 Statistics");
        ui.add_space(10.0);

        egui::ScrollArea::vertical().show(ui, |ui| {
            ui::charts::render_section(ui, &self.photos, &filters, &self.display_config);
            ui::photo_table::render(ui, &self.photos, &mut self.table_state, &columns, max_rows);
        });
    }
}

impl eframe::App for CameraStatsApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_scan(ctx);

        // The toolbar may ask for work that needs `&self.photos` (export) or
        // spawns a thread (scan); both are handled after the panel closes so the
        // `egui` borrow is released first.
        if let Some(action) = ui::toolbar::render(self, ctx) {
            match action {
                ui::toolbar::Action::Scan(dir) => self.start_scan(&dir),
                ui::toolbar::Action::Export(path) => self.log_ui(&format!("exported to {path:?}")),
                ui::toolbar::Action::ExportCancelled => {}
            }
        }

        if self.show_export_dialog {
            ui::toolbar::run_export_dialog(self);
        }
        if self.show_settings {
            ui::settings::render(self, ctx);
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            if self.photos.is_empty() {
                self.render_welcome(ui);
            } else {
                self.render_content(ui);
            }
        });
    }
}

/// Launch the native window.
///
/// Errors are returned rather than panicking so a failed window creation prints
/// a readable message instead of a backtrace.
pub fn run() -> Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1000.0, 800.0])
            .with_min_inner_size([800.0, 600.0]),
        ..Default::default()
    };

    eframe::run_native(
        "Camera Stats",
        options,
        Box::new(|_cc| Ok(Box::new(CameraStatsApp::default()))),
    )
    .map_err(|e| anyhow::anyhow!("{e:?}"))
}
