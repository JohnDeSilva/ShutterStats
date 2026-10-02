//! Photo filtering: the predicate, the option lists, and the dropdown widget.
//!
//! All filters are "empty means all", which lets the UI treat an untouched
//! filter as a no-op without special cases.
//!
//! [`PhotoFilters`] groups the five per-photo-table filters so call sites pass
//! one argument instead of five, which is what made the original call sites
//! error-prone.

use std::collections::{BTreeSet, HashSet};

use camera_stats::PhotoMeta;
use eframe::egui;

use crate::table::TableState;

/// The active set of filters, cloned from [`TableState`] when a view needs it.
///
/// Cloning sidesteps the borrow checker: a view needs `&mut self.table_state`
/// to render the widgets and `&self.photos` to evaluate them, which cannot both
/// be borrowed from `self` inside the same `egui` closure.
#[derive(Debug, Default, Clone)]
pub struct PhotoFilters {
    pub cameras: HashSet<String>,
    pub lenses: HashSet<String>,
    pub apertures: HashSet<String>,
    pub focals: HashSet<String>,
    pub types: HashSet<String>,
}

impl PhotoFilters {
    /// Snapshot the filters held in `state`.
    pub fn from_state(state: &TableState) -> Self {
        Self {
            cameras: state.photo_camera_filter.clone(),
            lenses: state.photo_lens_filter.clone(),
            apertures: state.photo_aperture_filter.clone(),
            focals: state.photo_focal_filter.clone(),
            types: state.photo_type_filter.clone(),
        }
    }

    /// True when no filter is restricting anything.
    pub fn is_empty(&self) -> bool {
        self.cameras.is_empty()
            && self.lenses.is_empty()
            && self.apertures.is_empty()
            && self.focals.is_empty()
            && self.types.is_empty()
    }

    /// A short, human-readable summary for logging, e.g. `cameras=1 lens=0`.
    ///
    /// Counts rather than values keep log lines short and avoid dumping dozens
    /// of lens names into the log file.
    pub fn summary(&self) -> String {
        format!(
            "cameras={} lenses={} apertures={} focals={} types={}",
            self.cameras.len(),
            self.lenses.len(),
            self.apertures.len(),
            self.focals.len(),
            self.types.len()
        )
    }
}

/// Apply all filters, returning the photos that match every active criterion.
///
/// Aperture and focal length are compared as formatted strings because that is
/// how the dropdown options are built, so the two stay consistent by
/// construction. Type matching is case-insensitive because the value comes from
/// file extensions.
pub fn apply<'a>(photos: &'a [PhotoMeta], filters: &PhotoFilters) -> Vec<&'a PhotoMeta> {
    photos
        .iter()
        .filter(|p| {
            let camera_ok = filters.cameras.is_empty()
                || p.camera_model
                    .as_deref()
                    .is_some_and(|c| filters.cameras.contains(c));
            let lens_ok = filters.lenses.is_empty()
                || p.lens_model
                    .as_deref()
                    .is_some_and(|l| filters.lenses.contains(l));
            let aperture_ok = filters.apertures.is_empty()
                || p.aperture
                    .is_some_and(|v| filters.apertures.contains(&format!("{v:.1}")));
            let focal_ok = filters.focals.is_empty()
                || p.focal_length
                    .is_some_and(|v| filters.focals.contains(&format!("{v:.0}")));
            let type_ok = filters.types.is_empty()
                || filters
                    .types
                    .iter()
                    .any(|t| t.eq_ignore_ascii_case(&p.photo_type));

            camera_ok && lens_ok && aperture_ok && focal_ok && type_ok
        })
        .collect()
}

/// Distinct camera models, sorted and de-duplicated.
pub fn unique_cameras(photos: &[PhotoMeta]) -> Vec<String> {
    sorted_unique(photos.iter().filter_map(|p| p.camera_model.clone()))
}

/// Distinct lens models, sorted and de-duplicated.
pub fn unique_lenses(photos: &[PhotoMeta]) -> Vec<String> {
    sorted_unique(photos.iter().filter_map(|p| p.lens_model.clone()))
}

/// Distinct apertures, formatted to one decimal to match [`apply`].
pub fn unique_apertures(photos: &[PhotoMeta]) -> Vec<String> {
    sorted_numeric(
        photos
            .iter()
            .filter_map(|p| p.aperture)
            .map(|v| (v, format!("{v:.1}"))),
    )
}

/// Distinct focal lengths, formatted to whole numbers to match [`apply`].
pub fn unique_focals(photos: &[PhotoMeta]) -> Vec<String> {
    sorted_numeric(
        photos
            .iter()
            .filter_map(|p| p.focal_length)
            .map(|v| (v, format!("{v:.0}"))),
    )
}

/// Distinct photo types, derived from the data so the dropdown can never offer
/// a type that is not actually present.
pub fn unique_types(photos: &[PhotoMeta]) -> Vec<String> {
    sorted_unique(photos.iter().map(|p| p.photo_type.clone()))
}

fn sorted_unique(items: impl Iterator<Item = String>) -> Vec<String> {
    let set: BTreeSet<String> = items.collect();
    set.into_iter().collect()
}

/// Sort by the numeric value so 2.8 precedes 10, then emit the display string.
fn sorted_numeric(items: impl Iterator<Item = (f32, String)>) -> Vec<String> {
    let mut items: Vec<(f32, String)> = items.collect();
    items.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    items.dedup_by(|a, b| a.1 == b.1);
    items.into_iter().map(|(_, s)| s).collect()
}

/// Multi-select dropdown used by every filter row.
///
/// `selected` is mutated in place. Clicks are collected during the inner loop
/// and applied afterwards because `selected` is borrowed by the closure that
/// renders the options.
pub fn dropdown_filter(
    ui: &mut egui::Ui,
    label: &str,
    options: &[String],
    selected: &mut HashSet<String>,
) {
    let sorted_options = sort_options_naturally(options);
    let count = selected.len();

    egui::ComboBox::from_id_salt(format!("filter_{label}"))
        .selected_text(if count == 0 {
            "All".to_string()
        } else {
            format!("{count} selected")
        })
        .show_ui(ui, |ui| {
            if ui.selectable_label(count == 0, "All").clicked() {
                selected.clear();
            }
            ui.separator();

            let mut toggles = Vec::new();
            for opt in &sorted_options {
                if ui.selectable_label(selected.contains(opt), opt).clicked() {
                    toggles.push(opt.clone());
                }
            }
            for opt in toggles {
                if !selected.remove(&opt) {
                    selected.insert(opt);
                }
            }
        });
}

/// Sort options the way a human expects.
///
/// The dropdown options are plain numbers (`"2.8"`, `"10"`, `"135"`) produced by
/// the `unique_*` functions, and a purely lexicographic sort would put `"10"`
/// before `"2.8"`. Comparing the parsed number first fixes that, while
/// non-numeric options (camera and lens names) still sort alphabetically.
fn sort_options_naturally(options: &[String]) -> Vec<String> {
    let mut sorted = options.to_vec();
    sorted.sort_by(|a, b| match (leading_number(a), leading_number(b)) {
        (Some(an), Some(bn)) => an.partial_cmp(&bn).unwrap_or(a.cmp(b)),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => a.cmp(b),
    });
    sorted
}

/// Parse a leading numeric prefix, tolerating a decimal point (`2.8`, `135`).
fn leading_number(s: &str) -> Option<f64> {
    let digits: String = s
        .chars()
        .take_while(|c| c.is_ascii_digit() || *c == '.')
        .collect();
    digits.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn photo(name: &str, photo_type: &str, camera: &str, lens: Option<&str>) -> PhotoMeta {
        PhotoMeta {
            path: PathBuf::from(name),
            iso: Some(400),
            aperture: Some(2.8),
            focal_length: Some(50.0),
            camera_model: Some(camera.into()),
            lens_model: lens.map(str::to_string),
            date_time: None,
            width: 6000,
            height: 4000,
            photo_type: photo_type.into(),
        }
    }

    fn fixture() -> Vec<PhotoMeta> {
        vec![
            photo("a.jpg", "Jpeg", "Sony A7", Some("Prime")),
            photo("b.ARW", "Raw", "Sony A7", Some("Prime")),
            photo("c.jpg", "Jpeg", "Canon R5", Some("Zoom")),
            photo("d.ARW", "Raw", "Canon R5", None),
        ]
    }

    /// Regression: the type filter used `format!("{:?}", photo_type)`, which
    /// yields `"Raw"` *with literal quote characters* and never matched the
    /// `Raw` stored in the filter set, so selecting a type showed zero rows.
    #[test]
    fn type_filter_matches_raw() {
        let photos = fixture();

        let mut f = PhotoFilters::default();
        f.types.insert("Raw".into());
        let rows = apply(&photos, &f);
        assert_eq!(rows.len(), 2, "expected the 2 RAW rows");
        assert!(rows.iter().all(|p| p.photo_type == "Raw"));

        let mut f = PhotoFilters::default();
        f.types.insert("Jpeg".into());
        assert_eq!(apply(&photos, &f).len(), 2);

        // Case-insensitive, since the value derives from a file extension.
        let mut f = PhotoFilters::default();
        f.types.insert("raw".into());
        assert_eq!(apply(&photos, &f).len(), 2);

        // Empty means all.
        assert_eq!(apply(&photos, &PhotoFilters::default()).len(), 4);
    }

    #[test]
    fn filters_are_anded_together() {
        let photos = fixture();

        let mut f = PhotoFilters::default();
        f.types.insert("Raw".into());
        f.cameras.insert("Canon R5".into());
        let rows = apply(&photos, &f);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].path.file_name().unwrap(), "d.ARW");
    }

    /// A photo missing the filtered field must be excluded, not silently kept.
    #[test]
    fn missing_field_excludes_row() {
        let photos = fixture();

        let mut f = PhotoFilters::default();
        f.lenses.insert("Zoom".into());
        let rows = apply(&photos, &f);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].path.file_name().unwrap(), "c.jpg");
    }

    #[test]
    fn options_are_derived_and_deduped() {
        let photos = fixture();

        let types = unique_types(&photos);
        assert_eq!(types, vec!["Jpeg".to_string(), "Raw".to_string()]);

        let cameras = unique_cameras(&photos);
        assert_eq!(cameras, vec!["Canon R5".to_string(), "Sony A7".to_string()]);

        // A photo with no lens must not contribute an empty option.
        let lenses = unique_lenses(&photos);
        assert_eq!(lenses, vec!["Prime".to_string(), "Zoom".to_string()]);
        assert!(!lenses.contains(&String::new()));
    }

    #[test]
    fn apertures_sort_numerically() {
        let mut photos = fixture();
        photos[0].aperture = Some(10.0);
        photos[1].aperture = Some(2.8);

        let apertures = unique_apertures(&photos);
        assert_eq!(apertures, vec!["2.8".to_string(), "10.0".to_string()]);
    }

    /// The dropdown options are plain numbers, so the sort must compare them
    /// numerically: lexicographically "10" would come before "2.8".
    #[test]
    fn natural_sort_orders_numbers_numerically() {
        let opts: Vec<String> = ["10.0", "2.8", "135", "22.0"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        assert_eq!(
            sort_options_naturally(&opts),
            vec!["2.8", "10.0", "22.0", "135"]
        );
    }

    /// Non-numeric options (camera and lens names) fall back to alphabetical
    /// order, and numbers sort ahead of them.
    #[test]
    fn natural_sort_falls_back_to_alphabetical() {
        let opts: Vec<String> = ["Sony A7", "2.8", "Canon R5"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        assert_eq!(
            sort_options_naturally(&opts),
            vec!["2.8", "Canon R5", "Sony A7"]
        );
    }

    #[test]
    fn filter_summary_reports_counts() {
        let mut f = PhotoFilters::default();
        assert!(f.is_empty());
        f.types.insert("Raw".into());
        assert!(!f.is_empty());
        assert_eq!(
            f.summary(),
            "cameras=0 lenses=0 apertures=0 focals=0 types=1"
        );
    }
}
