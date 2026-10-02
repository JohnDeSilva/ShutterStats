//! Aggregation that turns a set of photos into chart data.
//!
//! These functions are pure, which keeps the plotting code in [`crate::ui`] free
//! of data-shaping logic and makes the numeric behaviour directly testable.

use std::collections::BTreeMap;

use camera_stats::PhotoMeta;
use egui_plot::PlotPoints;

use crate::config::ApertureScale;

/// Standard full-stop f-stops, used by [`aperture_points`] in `FStops` mode.
const F_STOPS: [f32; 11] = [1.0, 1.4, 2.0, 2.8, 4.0, 5.6, 8.0, 11.0, 16.0, 22.0, 32.0];

/// Count photos per ISO value. Photos without a recorded ISO are skipped.
pub fn iso_counts(photos: &[&PhotoMeta]) -> BTreeMap<u32, usize> {
    let mut counts = BTreeMap::new();
    for p in photos {
        if let Some(iso) = p.iso {
            *counts.entry(iso).or_insert(0) += 1;
        }
    }
    counts
}

/// Count photos per aperture, bucketed to one decimal place.
///
/// Apertures are reported by EXIF as rationals, so the same f-stop can arrive
/// as 2.8, 2.7999, or 28/10. Bucketing merges them into one bar.
pub fn aperture_counts(photos: &[&PhotoMeta]) -> BTreeMap<i32, usize> {
    bucketed_apertures(photos)
}

/// Count photos per focal length, rounded to whole millimetres.
pub fn focal_counts(photos: &[&PhotoMeta]) -> BTreeMap<i32, usize> {
    let mut counts = BTreeMap::new();
    for p in photos {
        if let Some(fl) = p.focal_length {
            *counts.entry(fl.round() as i32).or_insert(0) += 1;
        }
    }
    counts
}

/// Build `x -> count` plot points for ISO.
pub fn iso_points(photos: &[&PhotoMeta]) -> PlotPoints {
    iso_counts(photos)
        .iter()
        .map(|(k, v)| [*k as f64, *v as f64])
        .collect()
}

/// Build `x -> count` plot points for focal length.
pub fn focal_points(photos: &[&PhotoMeta]) -> PlotPoints {
    focal_counts(photos)
        .iter()
        .map(|(k, v)| [*k as f64, *v as f64])
        .collect()
}

/// Build `x -> count` plot points for aperture, honouring the chosen scale.
///
/// Apertures are bucketed to one decimal place first, because EXIF reports them
/// as rationals and the same f-stop can arrive as 2.8 or 2.7999.
///
/// * [`ApertureScale::Linear`] plots the f-stop directly.
/// * [`ApertureScale::Logarithmic`] plots `log10(f)`, so equal ratios are
///   equally spaced, which is how photographers read a stop series.
/// * [`ApertureScale::FStops`] snaps each photo to the nearest standard stop so
///   the buckets land on recognisable values.
pub fn aperture_points(photos: &[&PhotoMeta], scale: ApertureScale) -> PlotPoints {
    let mut points: Vec<[f64; 2]> = match scale {
        ApertureScale::Linear => bucketed_apertures(photos)
            .into_iter()
            .map(|(key, count)| [key as f64 / 10.0, count as f64])
            .collect(),

        ApertureScale::Logarithmic => bucketed_apertures(photos)
            .into_iter()
            .filter_map(|(key, count)| {
                let f = key as f64 / 10.0;
                // log10(0) is -inf, which would break the plot bounds.
                (f > 0.0).then(|| [f.log10(), count as f64])
            })
            .collect(),

        ApertureScale::FStops => {
            let mut snapped: BTreeMap<usize, usize> = BTreeMap::new();
            for p in photos.iter().filter_map(|p| p.aperture) {
                let idx = nearest_fstop_index(p);
                *snapped.entry(idx).or_insert(0) += 1;
            }
            snapped
                .into_iter()
                .map(|(idx, count)| [F_STOPS[idx] as f64, count as f64])
                .collect()
        }
    };

    // Charts read left to right, so always emit ascending x.
    points.sort_by(|a, b| a[0].partial_cmp(&b[0]).unwrap_or(std::cmp::Ordering::Equal));
    PlotPoints::from(points)
}

/// Bucket apertures by `round(f * 10)`, i.e. to one decimal place.
fn bucketed_apertures(photos: &[&PhotoMeta]) -> BTreeMap<i32, usize> {
    let mut counts = BTreeMap::new();
    for p in photos.iter().filter_map(|p| p.aperture) {
        *counts.entry((p * 10.0).round() as i32).or_insert(0) += 1;
    }
    counts
}

/// Index into [`F_STOPS`] of the stop closest to `aperture`.
fn nearest_fstop_index(aperture: f32) -> usize {
    F_STOPS
        .iter()
        .enumerate()
        .min_by(|(_, a), (_, b)| {
            (*a - aperture)
                .abs()
                .partial_cmp(&(*b - aperture).abs())
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|(i, _)| i)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn photo(iso: Option<u32>, aperture: Option<f32>, focal: Option<f32>) -> PhotoMeta {
        PhotoMeta {
            path: PathBuf::from("x.jpg"),
            iso,
            aperture,
            focal_length: focal,
            camera_model: None,
            lens_model: None,
            date_time: None,
            width: 6000,
            height: 4000,
            photo_type: "Jpeg".into(),
        }
    }

    fn refs(photos: &[PhotoMeta]) -> Vec<&PhotoMeta> {
        photos.iter().collect()
    }

    /// The `(x, y)` pairs of a `PlotPoints`, for readable assertions.
    ///
    /// `egui_plot` returns `PlotPoint` structs rather than `[f64; 2]`, so tests
    /// unwrap them here once instead of indexing `.0`/`.1` at every call site.
    fn pairs(points: &PlotPoints) -> Vec<(f64, f64)> {
        points.points().iter().map(|p| (p.x, p.y)).collect()
    }

    #[test]
    fn iso_and_focal_skip_missing_values() {
        let photos = vec![
            photo(Some(100), Some(2.8), Some(35.0)),
            photo(None, Some(2.8), Some(50.0)),
            photo(Some(400), None, None),
        ];
        let r = refs(&photos);

        assert_eq!(iso_counts(&r).get(&100), Some(&1));
        assert_eq!(iso_counts(&r).get(&400), Some(&1));
        assert_eq!(iso_counts(&r).len(), 2, "None ISO must be skipped");

        assert_eq!(focal_counts(&r).get(&35), Some(&1));
        assert_eq!(focal_counts(&r).len(), 2, "None focal must be skipped");
    }

    #[test]
    fn aperture_buckets_to_one_decimal() {
        let photos = vec![
            photo(None, Some(2.8), None),
            photo(None, Some(2.82), None), // same bucket as 2.8
            photo(None, Some(4.0), None),
        ];
        let r = refs(&photos);
        let points = pairs(&aperture_points(&r, ApertureScale::Linear));

        assert_eq!(points.len(), 2);
        assert!((points[0].0 - 2.8).abs() < 1e-4, "x was {}", points[0].0);
        assert_eq!(points[0].1, 2.0, "2.8 and 2.82 share one bucket");
        assert_eq!(points[1], (4.0, 1.0));
    }

    #[test]
    fn logarithmic_scale_is_ascending_and_dropped_for_zero() {
        let photos = vec![photo(None, Some(1.0), None), photo(None, Some(10.0), None)];
        let r = refs(&photos);
        let points = aperture_points(&r, ApertureScale::Logarithmic);
        let xs: Vec<f64> = pairs(&points).into_iter().map(|(x, _)| x).collect();
        assert!(xs.windows(2).all(|w| w[0] < w[1]), "log scale must ascend");
    }

    #[test]
    fn fstop_mode_snaps_to_standard_stops() {
        let photos = vec![
            photo(None, Some(2.75), None), // nearest stop is 2.8
            photo(None, Some(2.85), None), // also 2.8
            photo(None, Some(9.0), None),  // nearest stop is 8.0
        ];
        let r = refs(&photos);
        let points = pairs(&aperture_points(&r, ApertureScale::FStops));

        // Compare with a tolerance: the stop table holds f32 literals, so 2.8 is
        // 2.7999999... in binary floating point.
        assert_eq!(points.len(), 2);
        assert!((points[0].0 - 2.8).abs() < 1e-4, "x was {}", points[0].0);
        assert_eq!(points[0].1, 2.0);
        assert_eq!(points[1], (8.0, 1.0));
    }

    #[test]
    fn empty_input_produces_no_points() {
        let r = refs(&[]);
        assert!(iso_points(&r).points().is_empty());
        assert!(focal_points(&r).points().is_empty());
        for scale in [
            ApertureScale::Linear,
            ApertureScale::Logarithmic,
            ApertureScale::FStops,
        ] {
            assert!(aperture_points(&r, scale).points().is_empty());
        }
    }
}
