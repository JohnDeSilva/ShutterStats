//! Verifies chart plot behaviour that cannot be checked with a pure function:
//! that a rendered histogram's y axis starts at zero, and that the plot is not
//! interactive.
//!
//! `egui_plot` computes axis bounds internally, so the only way to confirm the
//! axis really cannot go negative is to render a plot and read back the bounds
//! it settled on.

use std::cell::Cell;

use eframe::egui::{self, Vec2};
use egui_plot::{Bar, BarChart, Plot};

/// Margin mirroring `charts::MARGIN_FRACTION`: x padded, y not.
const MARGIN_FRACTION: Vec2 = Vec2::new(0.05, 0.0);

/// Render a histogram and return the y range of the final plot bounds.
fn render_histogram(counts: &[(f64, f64)], margin: Vec2) -> (f64, f64) {
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
            .set_margin_fraction(margin)
            .show(ui, |plot_ui| {
                plot_ui.bar_chart(BarChart::new(bars).width(0.8));
            });

        let bounds = response.transform.bounds();
        result.set((bounds.min()[1], bounds.max()[1]));
    });

    result.get()
}

/// The regression this protects against: `egui_plot` pads auto-bounds by a
/// fraction on every side. For a histogram whose baseline is 0, that padding
/// puts the axis minimum below zero, so the plot grows negative tick labels
/// under data that has no negative values. A zero y margin keeps the baseline
/// exactly on 0.
#[test]
fn y_axis_never_goes_below_zero() {
    let (min, max) = render_histogram(
        &[(100.0, 5.0), (200.0, 12.0), (400.0, 3.0)],
        MARGIN_FRACTION,
    );

    assert!(
        min >= 0.0,
        "y axis started at {min} (max {max}); it must not go below zero"
    );
    // Sanity: the data is in range, so the assertion above is not passing just
    // because the bounds collapsed.
    assert!(max >= 12.0, "y max {max} should cover the tallest bar (12)");
}

/// A count is never negative, so no realistic distribution should produce a
/// negative axis — including a single bar and a tall narrow spike.
#[test]
fn y_axis_stays_non_negative_across_shapes() {
    let cases: Vec<Vec<(f64, f64)>> = vec![
        vec![(800.0, 1.0)],
        vec![(100.0, 1.0), (1600.0, 999.0)],
        vec![(1.0, 7.0), (2.0, 7.0), (3.0, 7.0)],
        vec![(50.0, 0.0), (60.0, 4.0)],
    ];

    for counts in cases {
        let (min, max) = render_histogram(&counts, MARGIN_FRACTION);
        assert!(
            min >= 0.0,
            "y axis went negative: {min} with data {counts:?}"
        );
        assert!(max > min, "y axis collapsed for data {counts:?}");
    }
}

/// Documents the failure being fixed with actual numbers, so the workaround is
/// justified by evidence rather than by assumption.
///
/// If a future `egui_plot` stops padding below zero, this test fails and the
/// zero-margin workaround in `charts.rs` can be removed.
#[test]
fn default_margin_is_what_caused_negative_ticks() {
    let counts = [(100.0, 5.0), (200.0, 12.0), (400.0, 3.0)];

    // egui_plot's default margin: 5% on both axes.
    let (default_min, _) = render_histogram(&counts, Vec2::splat(0.05));
    let (fixed_min, _) = render_histogram(&counts, MARGIN_FRACTION);

    assert!(
        default_min < 0.0,
        "expected the default margin to dip below zero, got {default_min}; \
         the zero-margin workaround in charts.rs may no longer be needed"
    );
    assert!(
        fixed_min >= 0.0,
        "the zero-margin plot should sit at 0 or above, got {fixed_min}"
    );
}

/// Run `frames` frames in one persistent [`egui::Context`], injecting scroll and
/// drag events on the given frames, and record the plot bounds from each.
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
    let plot_rect = std::cell::Cell::new(egui::Rect::from_min_size(
        Pos2::ZERO,
        EVec2::new(800.0, 200.0),
    ));
    let centre = plot_rect.get().center();

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
            events.push(Event::PointerMoved(centre + EVec2::new(60.0, 40.0)));
            events.push(Event::PointerButton {
                pos: centre + EVec2::new(60.0, 40.0),
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
                plot = plot.set_margin_fraction(MARGIN_FRACTION);
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
                });
                let b = response.transform.bounds();
                plot_rect.set(response.response.rect);
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

/// Locking must not have broken the zero baseline: the interaction tests above
/// all use `MARGIN_FRACTION`, so the axis must still start at 0 on every frame.
#[test]
fn locked_plot_still_starts_at_zero_on_every_frame() {
    for bounds in bounds_across_frames(true, 2, 1) {
        assert!(
            bounds.0 >= 0.0,
            "locked plot y axis dipped below zero: {bounds:?}"
        );
    }
}

/// The x axis still gets its margin, so the outermost bars are not flush
/// against the frame. Clamping the y axis must not have cost the x padding.
#[test]
fn x_axis_keeps_its_margin() {
    let counts = [(100.0, 5.0), (200.0, 12.0)];

    let x_min = Cell::new(f64::NAN);
    egui::__run_test_ui(|ui| {
        let bars: Vec<Bar> = counts.iter().map(|(x, y)| Bar::new(*x, *y)).collect();
        let response = Plot::new("x_margin")
            .height(200.0)
            .set_margin_fraction(MARGIN_FRACTION)
            .show(ui, |plot_ui| plot_ui.bar_chart(BarChart::new(bars)));
        x_min.set(response.transform.bounds().min()[0]);
    });

    let first_x = counts[0].0;
    assert!(
        x_min.get() < first_x,
        "x min {} should sit below the first bar at {first_x}",
        x_min.get()
    );
}
