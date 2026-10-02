//! The top toolbar: folder selection, export, settings, and status text.

use camera_stats::{export_csv, scan_directory};
use chrono::Local;
use eframe::egui;
use rfd::FileDialog;
use std::path::PathBuf;
use std::sync::mpsc;
use std::thread;
use std::time::Instant;
use tracing::error;

use crate::app::CameraStatsApp;
use crate::ui;

/// Actions the toolbar can request from the app, returned so the caller can
/// perform them after the `egui` closure has released its borrow.
pub enum Action {
    /// Begin scanning `dir` on a background thread.
    Scan(PathBuf),
    /// The user dismissed the export dialog without choosing a path.
    ExportCancelled,
    /// Write the CSV to `path`.
    Export(PathBuf),
}

/// Render the toolbar.
///
/// Scanning is deliberately not done inline: `scan_directory` walks a directory
/// and decodes every file, which would freeze the UI for seconds on a large
/// library. A worker thread plus a channel keeps frames responsive, and the app
/// polls the channel in its update loop.
pub fn render(app: &mut CameraStatsApp, ctx: &egui::Context) -> Option<Action> {
    let mut action = None;

    egui::TopBottomPanel::top("toolbar").show(ctx, |ui| {
        ui.horizontal(|ui| {
            if ui.button("📁 Select Folder").clicked() && !app.scanning {
                if let Some(dir) = FileDialog::new().pick_folder() {
                    app.start_scan(&dir);
                    action = Some(Action::Scan(dir));
                }
            }

            ui.separator();

            if let Some(dir) = &app.selected_dir {
                ui.label(format!("📂 {}", dir.display()));
            }

            ui.separator();

            if ui.button("📤 Export CSV").clicked() && !app.photos.is_empty() {
                app.show_export_dialog = true;
            }

            ui.separator();

            if ui.button("⚙️ Settings").clicked() {
                app.show_settings = true;
            }

            // The status text is the app's only feedback channel, so it is
            // colour-coded: a green "Found N photos" reads as success at a
            // glance and a red "Export failed" as something to act on.
            let (status, scanning) = (app.status.clone(), app.scanning);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.colored_label(status_color(&status, scanning), &status);
            });
        });
    });

    action
}

/// Show the save dialog and export the current photo list to CSV.
///
/// Returns `true` when a write was attempted, so the caller can tell a cancel
/// from a completed export without inspecting the file system.
pub fn run_export_dialog(app: &mut CameraStatsApp) -> Option<Action> {
    // Default the file name to the current timestamp so repeated exports do not
    // overwrite each other silently.
    let suggested = format!("camera_stats_{}.csv", Local::now().format("%Y%m%d_%H%M%S"));

    let Some(path) = FileDialog::new()
        .add_filter("CSV", &["csv"])
        .set_file_name(suggested)
        .save_file()
    else {
        // Cancelled: no status change, so the user is not told about a write
        // that never happened.
        app.show_export_dialog = false;
        app.log_ui("export cancelled");
        return Some(Action::ExportCancelled);
    };

    // The export can fail for reasons the user can act on (bad path, full
    // disk), so report the error in the status bar as well as the log.
    match export_csv(&app.photos, &path) {
        Ok(_) => {
            app.status = format!("Exported to {}", path.display());
            app.log_ui(&format!(
                "exported {} photos to {:?}",
                app.photos.len(),
                path
            ));
        }
        Err(e) => {
            app.status = format!("Export failed: {e}");
            error!(target: "ui", "CSV export to {:?} failed: {e}", path);
        }
    }
    app.show_export_dialog = false;

    Some(Action::Export(path))
}

/// Spawn the background scan for `dir`.
///
/// Kept next to the toolbar so the "click a button, get a thread" wiring is in
/// one place rather than split between the view and the app.
pub fn spawn_scan(dir: PathBuf) -> mpsc::Receiver<Vec<camera_stats::PhotoMeta>> {
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let started = Instant::now();
        scan_directory(dir, tx);
        tracing::debug!(target: "scan", "worker finished in {} ms", started.elapsed().as_millis());
    });
    rx
}

/// Colourise the status bar: green when a scan finished, amber while running.
pub fn status_color(status: &str, scanning: bool) -> egui::Color32 {
    if scanning {
        egui::Color32::from_rgb(0xE0, 0x9F, 0x3E) // amber
    } else if status.starts_with("Exported") {
        egui::Color32::from_rgb(0x97, 0xC8, 0x54) // green
    } else if status.contains("failed") {
        egui::Color32::from_rgb(0xE6, 0x6A, 0x6A) // red
    } else {
        ui::TEXT_COLOR
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::TEXT_COLOR;

    #[test]
    fn status_colour_reflects_state() {
        assert_ne!(status_color("Scanning...", true), TEXT_COLOR);
        assert_ne!(status_color("Exported to x.csv", false), TEXT_COLOR);
        assert_ne!(status_color("Export failed: no space", false), TEXT_COLOR);
        assert_eq!(status_color("Found 22 photos", false), TEXT_COLOR);
    }

    #[test]
    fn export_failures_are_flagged_red() {
        let failing = status_color("Export failed: permission denied", false);
        let ok = status_color("Found 22 photos", false);
        assert_ne!(failing, ok);
    }
}
