//! Verifies chart plot behaviour that cannot be checked with a pure function:
//! that bounds pinned with `set_plot_bounds` survive into the rendered plot, and
//! that the plot is not interactive.
//!
//! `egui_plot` computes axis bounds internally, so the only way to confirm the
//! app's pinned range really takes effect is to render a plot and read back the
//! bounds it settled on.
//!
//! The range arithmetic itself lives in the binary crate (`ui::charts`), which an
//! integration test cannot import, so it is unit tested there against the real
//! `TOP_BUFFER` constant. This file deliberately does *not* copy that
//! arithmetic: a copied formula would keep passing even if the app's constant
//! changed. Instead it pins a distinctive sentinel range and checks the rendered
//! axis matches it exactly, which is what proves the app's own values are used.

use std::cell::Cell;

use eframe::egui;
use egui_plot::{Bar, BarChart, Plot, PlotBounds};

/// Render a histogram and return the final `(y_min, y_max)`.
///
/// `pinned` is `Some` to force bounds the way the app does, or `None` to leave
/// them to `egui_plot`'s auto-fit.
fn render_with(counts: &[(f64, f64)], pinned: Option<PlotBounds>) -> (f64, f64) {
    // `Plot::show` returns a `PlotResponse` whose `transform` holds the bounds
    // as they were finally computed. Reading `plot_ui.plot_bounds()` inside the
    // closure would return stale values, because `prepare()` runs afterwards.
    let result = Cell::new((f64::NAN, f64::NAN));

    egui::__run_test_ui(|ui| {
        let bars: Vec<Bar> = counts
            .iter()
            .map(|(x, y)| Bar::new(*x, *y).width(0.8))
            .collect();

        let response = Plot::new("test_histogram")
            .height(200.0)
            .allow_zoom(false)
            .allow_drag(false)
            .allow_scroll(false)
            .allow_boxed_zoom(false)
            .show(ui, |plot_ui| {
                plot_ui.bar_chart(BarChart::new(bars).width(0.8));
                if let Some(bounds) = pinned {
                    plot_ui.set_plot_bounds(bounds);
                }
            });

        let bounds = response.transform.bounds();
        result.set((bounds.min()[1], bounds.max()[1]));
    });

    result.get()
}

/// A distinctive y range the app would never auto-fit to.
///
/// Used as a probe: if the rendered axis comes back exactly `[0.0, SENTINEL]`
/// then `set_plot_bounds` is genuinely honoured. Asserting a specific number
/// rather than a copied formula keeps this file independent of the app's
/// arithmetic — the buffer maths itself is unit tested in `src/ui/charts.rs`,
/// where the real constant lives.
const SENTINEL_MAX: f64 = 4_242.5;

fn sentinel_bounds() -> PlotBounds {
    PlotBounds::from_min_max([-50.0, 0.0], [450.0, SENTINEL_MAX])
}

/// `set_plot_bounds` must be honoured exactly, or the app's computed top buffer
/// and zero baseline would be silently discarded by auto-fit.
#[test]
fn pinned_bounds_are_honoured_exactly() {
    let counts = [(100.0, 5.0), (200.0, 12.0), (400.0, 3.0)];
    let (min, max) = render_with(&counts, Some(sentinel_bounds()));

    assert_eq!(min, 0.0, "the count axis must start at zero, got {min}");
    assert_eq!(
        max, SENTINEL_MAX,
        "the axis must reach the pinned maximum, leaving the app's top buffer \
         above the tallest bar"
    );
    assert!(
        max > 12.0,
        "the tallest bar must not touch the top of the frame"
    );
}

/// A pinned minimum at 0 must hold even when the data goes negative, so a
/// malformed EXIF value cannot drag the axis below the baseline.
#[test]
fn a_pinned_zero_minimum_survives_negative_data() {
    let counts = [(1.0, -50.0), (2.0, -3.0), (3.0, 4.0)];
    let (min, max) = render_with(&counts, Some(sentinel_bounds()));

    assert_eq!(min, 0.0, "a negative bar pulled the axis to {min}");
    assert_eq!(max, SENTINEL_MAX);
}

/// A single bar has zero x span, which must not collapse the transform or
/// produce an inverted range.
#[test]
fn a_single_bar_still_renders() {
    let counts = [(135.0, 9.0)];
    let (min, max) = render_with(&counts, Some(sentinel_bounds()));

    assert_eq!(min, 0.0);
    assert_eq!(max, SENTINEL_MAX);
}

/// Records why the bounds are pinned rather than auto-fitted.
///
/// `egui_plot` pads auto-bounds by 5% on every side, so a histogram whose
/// baseline is 0 gets a y minimum of about `-0.05 * max`: negative tick labels
/// under a plot with no negative data. Its y maximum also floats with the data,
/// so any headroom above the tallest bar would be an accident rather than a
/// fixed gap.
///
/// If a future `egui_plot` fixes this upstream, this test fails and the explicit
/// bounds in `ui::charts` can be reconsidered.
#[test]
fn auto_fitted_bounds_are_why_they_are_pinned() {
    let counts = [(100.0, 5.0), (200.0, 12.0), (400.0, 3.0)];

    let (auto_min, _) = render_with(&counts, None);
    let (pinned_min, pinned_max) = render_with(&counts, Some(sentinel_bounds()));

    assert!(
        auto_min < 0.0,
        "expected auto-fit to dip below zero, got {auto_min}; \
         egui_plot may have fixed this, so the pinned bounds in ui/charts.rs \
         could be revisited"
    );
    assert_eq!(
        pinned_min, 0.0,
        "pinned bounds should sit exactly on the baseline, got {pinned_min}"
    );
    assert_eq!(pinned_max, SENTINEL_MAX);
}

/// Run 4 frames in one persistent [`egui::Context`], injecting scroll and drag
/// events on the given frames, and record the plot bounds from each.
///
/// The persistent context is the whole point: `egui_plot` stores pan/zoom in
/// context memory keyed by plot id, so only a shared context across frames can
/// reveal state that a locked plot must not accumulate. Pass `usize::MAX` for a
/// gesture that should not fire.
fn bounds_across_frames(locked: bool, drag_frame: usize, scroll_frame: usize) -> Vec<(f64, f64)> {
    use eframe::egui::{Event, MouseWheelUnit, PointerButton, Pos2, RawInput, Vec2 as EVec2};

    let ctx = egui::Context::default();
    let counts = [(100.0, 5.0), (200.0, 12.0), (400.0, 3.0)];
    let recorded = std::cell::RefCell::new(Vec::new());

    // The plot's screen rect is only known after the first frame, and egui_plot
    // gates both scroll and drag on the pointer being inside it, so the rect is
    // captured on frame 0 and used to aim the later gestures.
    let plot_rect = std::cell::RefCell::new(egui::Rect::from_min_size(
        Pos2::ZERO,
        EVec2::new(800.0, 200.0),
    ));
    let centre = plot_rect.borrow().center();

    for frame in 0..4 {
        let mut events = vec![Event::PointerMoved(centre)];

        if frame == scroll_frame {
            // egui_plot zooms on scroll, but only once the pointer is known to
            // be inside the plot rect.
            events.push(Event::MouseWheel {
                unit: MouseWheelUnit::Point,
                delta: EVec2::new(0.0, -50.0),
                modifiers: Default::default(),
            });
        }
        if frame == drag_frame {
            // Press inside the plot and drag: this is the pan gesture.
            events.push(Event::PointerButton {
                pos: centre,
                button: PointerButton::Primary,
                pressed: true,
                modifiers: Default::default(),
            });
        }
        // `checked_add` because `usize::MAX` is the "never fire" sentinel.
        if drag_frame.checked_add(1) == Some(frame) {
            // Move while held. This is the frame where the drag takes effect,
            // since `dragged_by` needs a previous and a current position.
            let moved = centre + EVec2::new(60.0, 40.0);
            events.push(Event::PointerMoved(moved));
            events.push(Event::PointerButton {
                pos: moved,
                button: PointerButton::Primary,
                pressed: false,
                modifiers: Default::default(),
            });
        }

        let input = RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                Pos2::ZERO,
                EVec2::new(800.0, 600.0),
            )),
            events,
            ..Default::default()
        };

        let _ = ctx.run(input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let bars: Vec<Bar> = counts.iter().map(|(x, y)| Bar::new(*x, *y)).collect();
                let mut plot = Plot::new("persistent").height(200.0);
                if locked {
                    plot = plot
                        .allow_zoom(false)
                        .allow_drag(false)
                        .allow_scroll(false)
                        .allow_boxed_zoom(false)
                        .allow_double_click_reset(false);
                }
                let response = plot.show(ui, |plot_ui| {
                    plot_ui.bar_chart(BarChart::new(bars).width(0.8));
                    plot_ui.set_plot_bounds(PlotBounds::from_min_max([80.0, 0.0], [420.0, 17.0]));
                });
                let b = response.transform.bounds();
                *plot_rect.borrow_mut() = response.response.rect;
                recorded.borrow_mut().push((b.min()[1], b.max()[1]));
            });
        });
    }

    recorded.into_inner()
}

/// The harness is only meaningful if it can see a change. This drives the
/// *unlocked* default plot with a scroll gesture and asserts the bounds really
/// do move — otherwise the locked-plot test below could pass vacuously.
#[test]
fn unlocked_plot_does_zoom_so_the_harness_is_sensitive() {
    let bounds = bounds_across_frames(false, usize::MAX, 1);
    let first = bounds[0];
    let later = bounds[bounds.len() - 1];

    assert_ne!(
        first, later,
        "an unlocked plot must change bounds on scroll, otherwise the \
         locked-plot assertions prove nothing (saw {bounds:?})"
    );
}

/// A locked plot must ignore every gesture that would pan or zoom, so its bounds
/// stay a pure function of the data no matter what the user does.
#[test]
fn locked_plot_ignores_scroll_and_drag() {
    for (drag_frame, scroll_frame, label) in [
        (usize::MAX, 1, "scroll"),
        (2, usize::MAX, "drag"),
        (2, 1, "both"),
    ] {
        let bounds = bounds_across_frames(true, drag_frame, scroll_frame);

        let first = bounds[0];
        for (frame, b) in bounds.iter().enumerate() {
            assert_eq!(
                first, *b,
                "{label} changed the bounds of a locked plot at frame {frame}: {bounds:?}"
            );
        }
    }
}

/// Locking must not have broken the pinned range: the y axis must still sit at
/// 0 with the same maximum on every frame, however the user interacts.
#[test]
fn locked_plot_keeps_its_range_on_every_frame() {
    for bounds in bounds_across_frames(true, 2, 1) {
        assert_eq!(
            bounds.0, 0.0,
            "locked plot y axis should stay at 0, got {bounds:?}"
        );
        assert_eq!(
            bounds.1, 17.0,
            "locked plot should keep its pinned range, got {bounds:?}"
        );
    }
}
