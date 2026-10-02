//! Photo-table sorting state.
//!
//! Sorting is expressed as an optional `(column, direction)` pair. Keeping it as
//! data rather than a closure means the active sort is one comparable value
//! instead of an opaque `Box<dyn Fn>`, which keeps [`TableState`] clonable and
//! `Debug`-printable.

use camera_stats::PhotoMeta;

/// Column the photo table is sorted by.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum SortColumn {
    /// File name. Also drives the Date and Type headers, which have no
    /// dedicated sort of their own.
    Name,
    /// Not currently reachable from the UI. It exists because the camera and
    /// lens summary tables were planned to sort by shot count; those tables are
    /// not implemented yet (their `show_camera_table` / `show_lens_table`
    /// toggles currently do nothing), so this variant sorts by file name.
    Count,
    /// Aperture value.
    Aperture,
    /// Focal length.
    FocalLength,
    /// Camera model.
    Camera,
    /// Lens model.
    Lens,
}

/// Sort direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum SortOrder {
    Ascending,
    Descending,
}

/// Mutable, serializable UI state for the photo table.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct TableState {
    /// Active sort, or `None` for "unsorted" (scan order).
    pub photo_sort: Option<(SortColumn, SortOrder)>,
    /// Cameras selected in the Camera filter. Empty means "all".
    pub photo_camera_filter: std::collections::HashSet<String>,
    /// Lenses selected in the Lens filter. Empty means "all".
    pub photo_lens_filter: std::collections::HashSet<String>,
    /// Apertures selected in the Aperture filter, formatted to one decimal.
    pub photo_aperture_filter: std::collections::HashSet<String>,
    /// Focal lengths selected in the Focal filter, formatted to whole numbers.
    pub photo_focal_filter: std::collections::HashSet<String>,
    /// Photo types selected in the Type filter. Empty means "all".
    pub photo_type_filter: std::collections::HashSet<String>,
}

/// Advance a sort to the next state when its header is clicked.
///
/// Clicking a new column starts ascending; clicking the active column toggles
/// the direction. Returning `None` clears sorting.
pub fn next_sort_state(
    current: &Option<(SortColumn, SortOrder)>,
    column: SortColumn,
) -> Option<(SortColumn, SortOrder)> {
    match current {
        Some((col, SortOrder::Ascending)) if *col == column => {
            Some((column, SortOrder::Descending))
        }
        Some(_) => Some((column, SortOrder::Ascending)),
        None => Some((column, SortOrder::Ascending)),
    }
}

/// Sort `photos` in place according to `sort`.
///
/// Fields that are `None` (a photo with no recorded lens, say) sort together as
/// empty strings rather than panicking on `partial_cmp`.
pub fn sort_photos(photos: &mut [&PhotoMeta], sort: Option<(SortColumn, SortOrder)>) {
    let Some((column, order)) = sort else {
        return;
    };
    photos.sort_by(|a, b| {
        let cmp = match column {
            SortColumn::Name | SortColumn::Count => file_name(a).cmp(&file_name(b)),
            SortColumn::Aperture => a
                .aperture
                .partial_cmp(&b.aperture)
                .unwrap_or(std::cmp::Ordering::Equal),
            SortColumn::FocalLength => a
                .focal_length
                .partial_cmp(&b.focal_length)
                .unwrap_or(std::cmp::Ordering::Equal),
            SortColumn::Camera => a
                .camera_model
                .as_deref()
                .unwrap_or("")
                .cmp(b.camera_model.as_deref().unwrap_or("")),
            SortColumn::Lens => a
                .lens_model
                .as_deref()
                .unwrap_or("")
                .cmp(b.lens_model.as_deref().unwrap_or("")),
        };
        match order {
            SortOrder::Ascending => cmp,
            SortOrder::Descending => cmp.reverse(),
        }
    });
}

/// File name of a photo as an owned `String`, or `""` if the path has none.
///
/// Owned rather than borrowed because `to_string_lossy` borrows from a
/// temporary `Cow`; sorting needs a value that outlives the match arm.
fn file_name(photo: &PhotoMeta) -> String {
    photo
        .path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn photo(name: &str, lens: Option<&str>, focal: Option<f32>) -> PhotoMeta {
        PhotoMeta {
            path: PathBuf::from(name),
            iso: Some(400),
            aperture: Some(2.8),
            focal_length: focal,
            camera_model: Some("Cam".into()),
            lens_model: lens.map(str::to_string),
            date_time: None,
            width: 6000,
            height: 4000,
            photo_type: "Jpeg".into(),
        }
    }

    #[test]
    fn cycling_sort_state_toggles_then_switches() {
        let mut state = None;
        state = next_sort_state(&state, SortColumn::Name);
        assert_eq!(state, Some((SortColumn::Name, SortOrder::Ascending)));

        state = next_sort_state(&state, SortColumn::Name);
        assert_eq!(state, Some((SortColumn::Name, SortOrder::Descending)));

        // Switching columns restarts at ascending.
        state = next_sort_state(&state, SortColumn::Lens);
        assert_eq!(state, Some((SortColumn::Lens, SortOrder::Ascending)));
    }

    #[test]
    fn sort_tolerates_missing_fields() {
        let photos = [
            photo("b.jpg", Some("Zoom"), Some(100.0)),
            photo("a.jpg", None, None),
            photo("c.jpg", Some("Prime"), Some(50.0)),
        ];
        let mut refs: Vec<&PhotoMeta> = photos.iter().collect();

        // Must not panic even though one photo lacks a lens and a focal length.
        // A missing lens sorts as the empty string, so it leads when ascending.
        sort_photos(&mut refs, Some((SortColumn::Lens, SortOrder::Ascending)));
        let names: Vec<String> = refs.iter().map(|p| file_name(p)).collect();
        assert_eq!(names, vec!["a.jpg", "c.jpg", "b.jpg"]);

        // Reversing puts the photo with data first.
        sort_photos(&mut refs, Some((SortColumn::Lens, SortOrder::Descending)));
        let names: Vec<String> = refs.iter().map(|p| file_name(p)).collect();
        assert_eq!(names, vec!["b.jpg", "c.jpg", "a.jpg"]);

        // Focal length uses partial_cmp, where `None` is unordered; it must
        // still produce a total order rather than panicking.
        sort_photos(
            &mut refs,
            Some((SortColumn::FocalLength, SortOrder::Descending)),
        );
        assert_eq!(refs.len(), 3, "no photos lost while sorting");
    }

    #[test]
    fn unsorted_is_a_no_op() {
        let photos = [photo("b.jpg", None, None), photo("a.jpg", None, None)];
        let mut refs: Vec<&PhotoMeta> = photos.iter().collect();
        sort_photos(&mut refs, None);
        assert_eq!(refs[0].path.file_name().unwrap(), "b.jpg");
    }
}
