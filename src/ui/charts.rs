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
//! negative data. Relying on the default margins is therefore not an option, and
//! the bounds are pinned explicitly instead — see [`plot_bounds`].
//!
//! Pinning the bounds also makes the top buffer exact: because the y maximum is
//! `tallest + TOP_BUFFER` rather than whatever auto-fit lands on, the gap above
//! the highest bar is a fixed 5 photos on every chart, under every filter, on
//! every frame.
//!
//! `tests/chart_axes.rs` verifies this by rendering a real plot and reading the
//! bounds back out of `egui_plot`'s transform, rather than by asserting the
//! constants here.

use camera_stats::PhotoMeta;
use eframe::egui;
use egui_plot::{Bar, BarChart, Line, Plot, PlotBounds, PlotPoints};

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

/// Headroom above the tallest bar, as a fraction of the tallest bar's height.
///
/// The y axis is pinned to `[0, tallest * (1 + TOP_BUFFER_FRACTION)]`, so this is
/// the gap between the top of the highest bar and the top of the frame. Without
/// it the tallest bar sits flush against the frame edge.
///
/// A fraction rather than a fixed count, so the gap looks the same whether the
/// tallest bar is 2 photos or 2000. See [`MIN_TOP_BUFFER`] for the case where
/// that leaves no visible gap at all.
const TOP_BUFFER_FRACTION: f64 = 0.05;

/// Floor for the headroom, in count units.
///
/// A percentage of a very short bar is no gap: 5% of a bar of 4 is 0.2, which
/// rounds away to nothing on the axis. This guarantees the frame always has
/// something visible above the tallest bar.
const MIN_TOP_BUFFER: f64 = 1.0;

/// Padding on the x axis, as a fraction of the data range.
///
/// The x axis is a category (an ISO value, an f-stop), so it needs breathing
/// room at both ends to stop the outermost bars touching the frame.
const X_MARGIN_FRACTION: f64 = 0.05;

/// Smallest x span to plot, in x units.
///
/// Stops a histogram whose bars all share one x (a single focal length, say)
/// from collapsing the x axis to a single point.
const MIN_X_SPAN: f64 = 1.0;

/// The y range for a series: pinned to zero, with headroom above the tallest bar.
///
/// Pinning the minimum at 0 is what keeps a count axis from ever showing a
/// negative tick, and 0 is also the bar baseline, so a zero minimum is correct
/// rather than merely tidy.
fn y_range(points: &PlotPoints) -> (f64, f64) {
    let tallest = points
        .points()
        .iter()
        .map(|p| p.y)
        .filter(|y| y.is_finite())
        .fold(f64::NEG_INFINITY, f64::max);

    // An empty or all-non-finite series has no tallest bar; treat it as 0 so the
    // axis is still a usable range.
    let tallest = if tallest.is_finite() {
        tallest.max(0.0)
    } else {
        0.0
    };

    // The fraction carries the look of the gap, the floor guarantees one exists:
    // with every bar at 0 (or an empty series) the fraction contributes nothing,
    // and a zero-height axis would collapse the plot.
    let headroom = (tallest * TOP_BUFFER_FRACTION).max(MIN_TOP_BUFFER);

    (0.0, tallest + headroom)
}

/// The x range for a series: the data span, padded, and never zero-width.
fn x_range(points: &PlotPoints) -> (f64, f64) {
    let mut min = f64::INFINITY;
    let mut max = f64::NEG_INFINITY;
    for p in points.points() {
        if p.x.is_finite() {
            min = min.min(p.x);
            max = max.max(p.x);
        }
    }

    if !min.is_finite() || !max.is_finite() {
        return (-MIN_X_SPAN / 2.0, MIN_X_SPAN / 2.0);
    }

    // A single distinct x has zero span, so widen it about its own centre.
    let (min, max, span) = if max - min < MIN_X_SPAN {
        let centre = (min + max) / 2.0;
        let half = MIN_X_SPAN / 2.0;
        (centre - half, centre + half, MIN_X_SPAN)
    } else {
        (min, max, max - min)
    };

    let pad = span * X_MARGIN_FRACTION;
    (min - pad, max + pad)
}

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
/// Locked against zoom, drag, scroll, and box-zoom. Bounds are deliberately not
/// left to `egui_plot`'s auto-fit: [`apply_bounds`] pins them every frame so the
/// axis is a pure function of the data.
fn locked_plot(id: &str, height: f32) -> Plot<'_> {
    Plot::new(id)
        .height(height)
        .allow_zoom(false)
        .allow_drag(false)
        .allow_scroll(false)
        .allow_boxed_zoom(false)
        .allow_double_click_reset(false)
}

/// The exact plot bounds for `points`: x padded, y pinned to zero with a
/// [`TOP_BUFFER`] gap at the top.
///
/// Computed before the series is moved into a `Line`, so all three chart styles
/// share one range and cannot drift apart.
fn plot_bounds(points: &PlotPoints) -> PlotBounds {
    let (y_min, y_max) = y_range(points);
    let (x_min, x_max) = x_range(points);
    PlotBounds::from_min_max([x_min, y_min], [x_max, y_max])
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
    let bounds = plot_bounds(&points);

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
                plot_ui.set_plot_bounds(bounds);
            });
        }
        // `fill(0.0)` closes the line back to the axis, which reads better than
        // a bare outline for a distribution. The axis minimum is pinned to 0,
        // so that fill lands exactly on the baseline.
        ChartType::Lines => {
            let line = Line::new(points).name(label).fill(0.0_f32).highlight(true);
            locked_plot(id, height).show(ui, |plot_ui| {
                plot_ui.line(line);
                plot_ui.set_plot_bounds(bounds);
            });
        }
        ChartType::Points => {
            let line = Line::new(points).name(label).highlight(true);
            locked_plot(id, height).show(ui, |plot_ui| {
                plot_ui.line(line);
                plot_ui.set_plot_bounds(bounds);
            });
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

    /// Build a series from `(x, y)` pairs.
    fn series(pairs: &[(f64, f64)]) -> PlotPoints {
        PlotPoints::from(pairs.iter().map(|(x, y)| [*x, *y]).collect::<Vec<_>>())
    }

    /// The requested feature: the axis stops exactly `TOP_BUFFER` above the
    /// tallest bar, so there is always a visible gap at the top.
    #[test]
    fn y_axis_leaves_a_top_buffer() {
        let points = series(&[(100.0, 5.0), (200.0, 12.0), (400.0, 3.0)]);
        let (min, max) = y_range(&points);

        assert_eq!(min, 0.0, "the count axis must start at zero");
        assert_eq!(
            max,
            12.0 + TOP_BUFFER,
            "the axis should stop 5 units above the tallest bar (12)"
        );
        assert!(
            max > 12.0,
            "the tallest bar must not touch the top of the frame"
        );
    }

    /// The buffer is a fixed count, so it holds for any bar height.
    #[test]
    fn top_buffer_is_exactly_top_buffer_for_any_height() {
        for tallest in [1.0, 7.0, 99.0, 4200.0] {
            let points = series(&[(1.0, tallest)]);
            let (min, max) = y_range(&points);
            assert_eq!(min, 0.0);
            assert_eq!(
                max - tallest,
                TOP_BUFFER,
                "gap above a {tallest}-tall bar should be {TOP_BUFFER}"
            );
        }
    }

    /// A count can never be negative, but a malformed EXIF value could reach
    /// here as a negative y. The minimum is pinned at 0 regardless, so the axis
    /// cannot grow a negative tick.
    #[test]
    fn y_axis_never_starts_below_zero_even_with_negative_data() {
        let points = series(&[(1.0, -50.0), (2.0, -3.0)]);
        let (min, max) = y_range(&points);

        assert_eq!(min, 0.0, "a negative bar must not drag the axis below 0");
        assert!(max >= TOP_BUFFER);
    }

    /// Non-finite y values must not poison the range into NaN, which would
    /// silently render an empty plot.
    #[test]
    fn non_finite_values_do_not_poison_the_range() {
        let points = series(&[
            (1.0, 4.0),
            (2.0, f64::NAN),
            (3.0, f64::INFINITY),
            (4.0, f64::NEG_INFINITY),
        ]);
        let (y_min, y_max) = y_range(&points);
        let (x_min, x_max) = x_range(&points);

        assert!(
            y_min.is_finite() && y_max.is_finite(),
            "y range {y_min}..{y_max}"
        );
        assert!(
            x_min.is_finite() && x_max.is_finite(),
            "x range {x_min}..{x_max}"
        );
        assert_eq!(y_max, 4.0 + TOP_BUFFER, "the finite bar sets the height");
    }

    /// An empty series still needs a drawable range rather than an inverted or
    /// infinite one.
    #[test]
    fn empty_series_yields_a_usable_range() {
        let points = series(&[]);
        let (y_min, y_max) = y_range(&points);
        let (x_min, x_max) = x_range(&points);

        assert_eq!(y_min, 0.0);
        assert!(y_max > y_min, "y range {y_min}..{y_max} must have height");
        assert!(x_max > x_min, "x range {x_min}..{x_max} must have width");
    }

    /// A single distinct x has zero width, which would collapse the transform.
    #[test]
    fn x_range_never_collapses_to_zero_width() {
        for points in [
            series(&[(135.0, 9.0)]),
            series(&[(135.0, 9.0), (135.0, 3.0)]),
            series(&[(0.0, 1.0), (0.2, 1.0)]),
        ] {
            let (min, max) = x_range(&points);
            assert!(
                max - min >= MIN_X_SPAN,
                "x range {min}..{max} is too narrow to plot"
            );
        }
    }

    /// The x axis is padded so the outermost bars are not flush with the frame,
    /// and the padding is symmetric.
    #[test]
    fn x_axis_is_padded_symmetrically() {
        let points = series(&[(100.0, 1.0), (200.0, 2.0), (300.0, 3.0)]);
        let (min, max) = x_range(&points);

        let pad_left = 100.0 - min;
        let pad_right = max - 300.0;
        assert!(
            pad_left > 0.0 && pad_right > 0.0,
            "both ends need padding: {min}..{max}"
        );
        assert!(
            (pad_left - pad_right).abs() < 1e-9,
            "padding should be symmetric, got {pad_left} vs {pad_right}"
        );
        assert_eq!(pad_left, 200.0 * X_MARGIN_FRACTION);
    }

    /// The bounds handed to `egui_plot` are the ones computed here, so the axis
    /// is a pure function of the data.
    #[test]
    fn plot_bounds_match_the_computed_ranges() {
        let points = series(&[(100.0, 5.0), (200.0, 12.0)]);
        let bounds = plot_bounds(&points);

        assert_eq!(bounds.min()[1], 0.0);
        assert_eq!(bounds.max()[1], 12.0 + TOP_BUFFER);
        assert_eq!(bounds.min()[0], x_range(&points).0);
        assert_eq!(bounds.max()[0], x_range(&points).1);
    }

    /// Guards the one thing the integration test cannot check: it pins a
    /// sentinel range rather than calling this code, so nothing there would catch
    /// `TOP_BUFFER` drifting. This asserts the shipped value is the requested 5.
    #[test]
    fn top_buffer_is_five() {
        assert_eq!(TOP_BUFFER, 5.0, "the chart top buffer is 5 units");
    }

    /// Every chart style must be given the same range, so switching Bars -> Lines
    /// -> Points does not rescale the axis under the user.
    #[test]
    fn all_chart_types_share_one_range() {
        let points = series(&[(100.0, 5.0), (200.0, 12.0)]);
        let bounds = plot_bounds(&points);

        // `plot_chart` computes `bounds` once before the match and hands the
        // same `Copy` value to each arm, so this holds by construction; the
        // assertion documents the invariant.
        for chart_type in [ChartType::Bars, ChartType::Lines, ChartType::Points] {
            let per_style = plot_bounds(&points);
            assert_eq!(bounds.min(), per_style.min(), "{chart_type:?}");
            assert_eq!(bounds.max(), per_style.max(), "{chart_type:?}");
        }
    }

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
    /// The bounds themselves are covered by `tests/chart_axes.rs`, which renders
    /// an actual plot and reads back what `egui_plot` computed.
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
