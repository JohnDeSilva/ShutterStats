//! Chart rendering.
//!
//! [`plot_chart`] draws a `PlotPoints` series in whichever style the user picked;
//! [`render_section`] draws the three distribution charts for the current filter
//! selection.
//!
//! # Why the plots are locked
//! These are distribution histograms: the y value is always a photo count, so a
//! user can only ever "damage" the view by scrolling or dragging it away from the
//! data. Every interaction that would pan or zoom is therefore disabled, and the
//! axes are always auto-fitted to the data instead. That also removes the need
//! to carry zoom state between frames.
//!
//! # Why the y axis starts at zero
//! [`egui_plot`] pads auto-computed bounds by 5% on every side by default. For a
//! histogram whose baseline is 0, that padding produces a y minimum of roughly
//! `-5%`, so the axis grows negative tick labels below a plot that has no
//! negative data. [`MARGIN_FRACTION`] drops the y padding to zero while leaving
//! the x margin, which is still needed to keep the outermost bars inside the
//! frame.
//!
//! `tests/chart_axes.rs` verifies this by rendering a real plot and reading the
//! bounds back out of `egui_plot`'s transform, rather than by asserting the
//! constant here.

use camera_stats::PhotoMeta;
use eframe::egui::{self, Vec2};
use egui_plot::{Bar, BarChart, Line, Plot, PlotPoints};

use crate::analytics;
use crate::config::{ApertureScale, ChartType, DisplayConfig};
use crate::filter::PhotoFilters;

/// Chart height as a fraction of the window's height.
///
/// Scaling with the window rather than a fixed pixel height keeps the three
/// charts readable on a small laptop and useful on a large display. The value is
/// a fraction of the viewport, clamped to sane pixel bounds by
/// [`chart_height`].
const CHART_HEIGHT_FRACTION: f32 = 0.22;

/// Lower clamp for chart height, in points.
///
/// Guards against a very short window collapsing the plot to nothing.
const MIN_CHART_HEIGHT: f32 = 140.0;

/// Upper clamp for chart height, in points.
///
/// Guards against a tall window stretching the plot to an absurd size.
const MAX_CHART_HEIGHT: f32 = 420.0;

/// Padding added around the auto-fitted data, as a fraction of the data range.
///
/// The x axis keeps a margin so the first and last bars are not flush against
/// the frame. The y axis gets none, so the baseline sits exactly on 0 and the
/// axis never shows a negative tick.
const MARGIN_FRACTION: Vec2 = Vec2::new(0.05, 0.0);

/// Chart height in points for the current window size.
///
/// `available` is the height left in the scroll area this frame and `viewport`
/// is the window height, so the chart grows with the window but never claims
/// more space than actually exists.
fn chart_height(available: f32, viewport: f32) -> f32 {
    let from_viewport = viewport * CHART_HEIGHT_FRACTION;
    from_viewport
        .min(available)
        .clamp(MIN_CHART_HEIGHT, MAX_CHART_HEIGHT)
}

/// A plot configured for a read-only histogram.
///
/// Locked against zoom, drag, scroll, and box-zoom, with a zero y margin so the
/// count axis starts at exactly 0.
fn locked_plot(id: &str, height: f32) -> Plot<'_> {
    Plot::new(id)
        .height(height)
        .allow_zoom(false)
        .allow_drag(false)
        .allow_scroll(false)
        .allow_boxed_zoom(false)
        .allow_double_click_reset(false)
        .set_margin_fraction(MARGIN_FRACTION)
}

/// Draw one series, styled per `chart_type`.
///
/// `id` must be unique per chart so `egui` keeps their layouts independent.
pub fn plot_chart(
    ui: &mut egui::Ui,
    id: &str,
    label: &str,
    points: PlotPoints,
    chart_type: ChartType,
) {
    let viewport = ui.ctx().screen_rect().height();
    let height = chart_height(ui.available_height(), viewport);

    match chart_type {
        ChartType::Bars => {
            let bars: Vec<Bar> = points
                .points()
                .iter()
                .map(|p| Bar::new(p.x, p.y).width(0.8))
                .collect();
            locked_plot(id, height).show(ui, |plot_ui| {
                plot_ui.bar_chart(
                    BarChart::new(bars)
                        .name(label)
                        .width(0.8)
                        .element_formatter(Box::new(move |bar, _| {
                            format!("{}: {:.0}", bar.argument, bar.value)
                        })),
                );
            });
        }
        // `fill(0.0)` closes the line back to the axis, which reads better than
        // a bare outline for a distribution.
        ChartType::Lines => {
            let line = Line::new(points).name(label).fill(0.0_f32).highlight(true);
            locked_plot(id, height).show(ui, |plot_ui| plot_ui.line(line));
        }
        ChartType::Points => {
            let line = Line::new(points).name(label).highlight(true);
            locked_plot(id, height).show(ui, |plot_ui| plot_ui.line(line));
        }
    }
}

/// Draw the ISO, aperture, and focal-length distribution charts.
///
/// Charts reflect the *filtered* selection, not the whole library, so the
/// distributions stay consistent with the photo table below them.
pub fn render_section(
    ui: &mut egui::Ui,
    photos: &[PhotoMeta],
    filters: &PhotoFilters,
    config: &DisplayConfig,
) {
    let filtered = crate::filter::apply(photos, filters);
    let refs: Vec<&PhotoMeta> = filtered.to_vec();

    if config.show_iso_chart && !analytics::iso_counts(&refs).is_empty() {
        ui.group(|ui| {
            ui.label("ISO Distribution");
            let points = analytics::iso_points(&refs);
            if !points.points().is_empty() {
                plot_chart(ui, "iso_plot", "ISO", points, config.chart_type);
            }
        });
    }

    if config.show_aperture_chart && !analytics::aperture_counts(&refs).is_empty() {
        ui.group(|ui| {
            let label = aperture_label(config.aperture_scale);
            ui.label(label);
            let points = analytics::aperture_points(&refs, config.aperture_scale);
            if !points.points().is_empty() {
                plot_chart(ui, "aperture_plot", label, points, config.chart_type);
            }
        });
    }

    if config.show_focal_chart && !analytics::focal_counts(&refs).is_empty() {
        ui.group(|ui| {
            ui.label("Focal Length Distribution (mm)");
            let points = analytics::focal_points(&refs);
            if !points.points().is_empty() {
                plot_chart(ui, "focal_plot", "Focal Length", points, config.chart_type);
            }
        });
    }
}

/// Axis caption for the aperture chart, which varies with the scale.
fn aperture_label(scale: ApertureScale) -> &'static str {
    match scale {
        ApertureScale::Linear => "Aperture (f-stop)",
        ApertureScale::Logarithmic => "Aperture (log scale)",
        ApertureScale::FStops => "Aperture (standard f-stops)",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aperture_label_matches_scale() {
        assert_eq!(aperture_label(ApertureScale::Linear), "Aperture (f-stop)");
        assert_eq!(
            aperture_label(ApertureScale::Logarithmic),
            "Aperture (log scale)"
        );
        assert_eq!(
            aperture_label(ApertureScale::FStops),
            "Aperture (standard f-stops)"
        );
    }

    /// The height must stay usable across window sizes and never collapse or
    /// blow up.
    ///
    /// The margin that keeps the y axis at zero is not asserted here because
    /// checking a constant against itself proves nothing; `tests/chart_axes.rs`
    /// renders an actual plot and reads back the bounds it computes.
    #[test]
    fn chart_height_scales_with_the_window() {
        // Twice the window gives twice the chart.
        assert_eq!(chart_height(4_000.0, 800.0), 176.0);
        assert_eq!(chart_height(4_000.0, 1_600.0), 352.0);
    }

    /// The chart never exceeds the space available, and never collapses or
    /// blows up regardless of window size.
    #[test]
    fn chart_height_always_within_limits() {
        for (available, viewport) in [
            (0.0, 600.0),
            (50.0, 600.0),
            (200.0, 800.0),
            (800.0, 900.0),
            (4_000.0, 1_600.0),
            (100_000.0, 100_000.0),
        ] {
            let h = chart_height(available, viewport);
            assert!(
                (MIN_CHART_HEIGHT..=MAX_CHART_HEIGHT).contains(&h),
                "height {h} out of range for available {available}, viewport {viewport}"
            );
            assert!(
                h <= available.max(MIN_CHART_HEIGHT),
                "height {h} exceeds the {available} points available"
            );
        }
    }
}
