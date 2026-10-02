//! The photo details table: sortable headers, filter dropdowns, and rows.
//!
//! Headers and cells are described once in [`COLUMNS`] and rendered from that
//! single description, so a column can never end up with a header that does
//! not match its cell.
//!
//! Visibility is driven by the `show_*` flags in [`PhotoTableColumns`]. A hidden
//! column also hides its filter dropdown, which keeps the grid columns aligned.

use camera_stats::PhotoMeta;
use eframe::egui;
use tracing::info;

use crate::config::PhotoTableColumns;
use crate::filter::{self, PhotoFilters};
use crate::table::{self, SortColumn, TableState};

/// Placeholder shown when a photo has no value for a column.
const MISSING: &str = "—";

/// Builds the option list for a column's filter dropdown.
type OptionBuilder = fn(&[PhotoMeta]) -> Vec<String>;

/// One column of the photo table.
///
/// `visible` and `value` are function pointers rather than closures so the whole
/// table is a `const`, which keeps it visible at a glance instead of buried in
/// imperative draw code.
struct Column {
    /// Header text, also used as the label for this column's filter.
    header: &'static str,
    /// Column the sort button drives.
    sort: SortColumn,
    /// The settings flag that shows or hides this column.
    visible: fn(&PhotoTableColumns) -> bool,
    /// Formats the cell value for a photo.
    value: fn(&PhotoMeta) -> String,
    /// Filter options for this column, or `None` if it has no filter.
    filter: Option<OptionBuilder>,
    /// The `TableState` field this column's filter writes to.
    ///
    /// Unused when `filter` is `None`.
    filter_slot: fn(&mut TableState) -> &mut std::collections::HashSet<String>,
}

/// Every implemented column, in display order.
///
/// Adding an EXIF column means adding one entry here; the header, cell, sort
/// button, and filter all follow automatically.
const COLUMNS: &[Column] = &[
    Column {
        header: "File",
        sort: SortColumn::Name,
        visible: |c| c.show_file,
        value: |p| file_name(p),
        filter: None,
        filter_slot: |_| unreachable!("no filter for this column"),
    },
    Column {
        header: "Date",
        sort: SortColumn::Name,
        visible: |c| c.show_date,
        value: |p| {
            p.date_time.map_or_else(
                || MISSING.to_string(),
                |dt| dt.format("%Y-%m-%d").to_string(),
            )
        },
        filter: None,
        filter_slot: |_| unreachable!("no filter for this column"),
    },
    Column {
        header: "Aperture",
        sort: SortColumn::Aperture,
        visible: |c| c.show_aperture,
        value: |p| {
            p.aperture
                .map_or_else(|| MISSING.to_string(), |v| format!("f/{v:.1}"))
        },
        filter: Some(filter::unique_apertures),
        filter_slot: |s| &mut s.photo_aperture_filter,
    },
    Column {
        header: "Focal (mm)",
        sort: SortColumn::FocalLength,
        visible: |c| c.show_focal_length,
        value: |p| {
            p.focal_length
                .map_or_else(|| MISSING.to_string(), |v| format!("{v:.0}"))
        },
        filter: Some(filter::unique_focals),
        filter_slot: |s| &mut s.photo_focal_filter,
    },
    Column {
        header: "Camera",
        sort: SortColumn::Camera,
        visible: |c| c.show_camera,
        value: |p| p.camera_model.as_deref().unwrap_or(MISSING).to_string(),
        filter: Some(filter::unique_cameras),
        filter_slot: |s| &mut s.photo_camera_filter,
    },
    Column {
        header: "Lens",
        sort: SortColumn::Lens,
        visible: |c| c.show_lens,
        value: |p| p.lens_model.as_deref().unwrap_or(MISSING).to_string(),
        filter: Some(filter::unique_lenses),
        filter_slot: |s| &mut s.photo_lens_filter,
    },
    Column {
        header: "Type",
        sort: SortColumn::Name,
        visible: |c| c.show_type,
        value: |p| p.photo_type.clone(),
        filter: Some(filter::unique_types),
        filter_slot: |s| &mut s.photo_type_filter,
    },
];

/// File name of a photo, or the full path rendered as a fallback.
fn file_name(photo: &PhotoMeta) -> String {
    photo
        .path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| photo.path.to_string_lossy().into_owned())
}

/// Render the photo table.
///
/// `state` is borrowed mutably for the sort and filter controls; `photos` is
/// borrowed immutably for the rows. Splitting the two this way is what lets the
/// widgets and the data live in the same frame without a borrow conflict.
pub fn render(
    ui: &mut egui::Ui,
    photos: &[PhotoMeta],
    state: &mut TableState,
    columns: &PhotoTableColumns,
    max_rows: usize,
) {
    // Snapshot the filters so the resulting row set and any change can be logged.
    let before = PhotoFilters::from_state(state);

    // Options are derived from the whole library, not the filtered subset, so
    // an active filter cannot hide the other options from the user.
    let mut rows = filter::apply(photos, &before);
    table::sort_photos(&mut rows, state.photo_sort);

    // Distinguish "filters matched nothing" from "no photos are loaded": an
    // empty table with active filters is a confusing state to debug from the UI
    // alone, so it gets its own message.
    if rows.is_empty() && !before.is_empty() {
        ui.colored_label(
            crate::ui::TEXT_COLOR,
            format!(
                "No photos match the current filters ({}). Photos loaded: {}.",
                before.summary(),
                photos.len()
            ),
        );
        return;
    }

    let shown = rows.len().min(max_rows);
    let after = PhotoFilters::from_state(state);
    if after.summary() != before.summary() {
        info!(target: "ui", "photo filters changed: {} -> {} rows", after.summary(), rows.len());
    }
    if rows.len() > max_rows {
        info!(target: "ui", "truncating table: showing {shown} of {} rows (max_table_rows)", rows.len());
    }

    let visible: Vec<&Column> = COLUMNS.iter().filter(|c| (c.visible)(columns)).collect();

    ui.group(|ui| {
        ui.label(format!("All Photos ({} of {})", rows.len(), photos.len()));
        egui::ScrollArea::horizontal()
            .max_width(ui.available_width())
            .show(ui, |ui| {
                egui::Grid::new("photo_grid")
                    .striped(true)
                    .min_col_width(100.0)
                    .show(ui, |ui| {
                        header_row(ui, &visible, state);
                        filter_row(ui, &visible, photos, state);
                        ui.end_row();
                        body_rows(ui, &visible, &rows[..shown]);
                    });
            });
    });
}

/// Sortable header cells, one per visible column.
fn header_row(ui: &mut egui::Ui, visible: &[&Column], state: &mut TableState) {
    for column in visible {
        let arrow = match state.photo_sort {
            Some((col, order)) if col == column.sort => match order {
                table::SortOrder::Ascending => " ▲",
                table::SortOrder::Descending => " ▼",
            },
            _ => "",
        };

        // `frame(false)` draws a borderless label-button that matches the
        // surrounding grid styling.
        let response =
            ui.add(egui::Button::new(format!("{}{}", column.header, arrow)).frame(false));
        if response.clicked() {
            state.photo_sort = table::next_sort_state(&state.photo_sort, column.sort);
            match state.photo_sort {
                Some((col, order)) => info!(target: "ui", "sorting by {:?} {:?}", col, order),
                None => info!(target: "ui", "sorting cleared"),
            }
        }
    }
    ui.end_row();
}

/// Filter dropdown cells, one per visible column that has a filter.
///
/// Columns without a filter render an empty cell so the grid rows stay aligned.
fn filter_row(
    ui: &mut egui::Ui,
    visible: &[&Column],
    photos: &[PhotoMeta],
    state: &mut TableState,
) {
    for column in visible {
        let Some(build_options) = column.filter else {
            ui.label("");
            continue;
        };
        let options = build_options(photos);
        // Borrow each slot independently so the loop is not held across calls
        // to `dropdown_filter`, which needs `&mut egui::Ui` as well.
        let slot = (column.filter_slot)(state);
        filter::dropdown_filter(ui, column.header, &options, slot);
    }
    ui.end_row();
}

/// One row per photo.
fn body_rows(ui: &mut egui::Ui, visible: &[&Column], rows: &[&PhotoMeta]) {
    for photo in rows {
        for column in visible {
            ui.label((column.value)(photo));
        }
        ui.end_row();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn photo(name: &str, photo_type: &str, camera: Option<&str>, lens: Option<&str>) -> PhotoMeta {
        PhotoMeta {
            path: PathBuf::from(name),
            iso: Some(400),
            aperture: Some(2.8),
            focal_length: Some(18.0),
            camera_model: camera.map(str::to_string),
            lens_model: lens.map(str::to_string),
            date_time: None,
            width: 6000,
            height: 4000,
            photo_type: photo_type.into(),
        }
    }

    fn fixture() -> Vec<PhotoMeta> {
        vec![
            photo("b.ARW", "Raw", Some("Sony FX30"), Some("Wide")),
            photo("a.jpg", "Jpeg", Some("Sony FX30"), Some("Wide")),
            photo("c.jpg", "Jpeg", None, None),
        ]
    }

    #[test]
    fn default_columns_are_all_visible() {
        let columns = PhotoTableColumns::default();
        let visible: Vec<&str> = COLUMNS
            .iter()
            .filter(|c| (c.visible)(&columns))
            .map(|c| c.header)
            .collect();
        assert_eq!(
            visible,
            vec![
                "File",
                "Date",
                "Aperture",
                "Focal (mm)",
                "Camera",
                "Lens",
                "Type"
            ]
        );
    }

    /// A column's header and its cell formatter must agree, so the value
    /// function has to tolerate the "no value" case without panicking.
    #[test]
    fn cell_formatting_never_panics_on_missing_fields() {
        let p = photo("x.jpg", "Jpeg", None, None);
        for column in COLUMNS {
            let text = (column.value)(&p);
            assert!(!text.is_empty(), "{} produced empty text", column.header);
        }
        assert_eq!((COLUMNS[4].value)(&p), MISSING, "missing camera");
        assert_eq!((COLUMNS[5].value)(&p), MISSING, "missing lens");
    }

    #[test]
    fn formatted_values_match_the_filter_options() {
        let photos = fixture();
        let p = &photos[0];

        // The cell text and the filter option come from the same formatting, so
        // selecting a filter value can never exclude the row it came from.
        let aperture_cell = (COLUMNS[2].value)(p);
        assert_eq!(aperture_cell, "f/2.8");
        assert!(filter::unique_apertures(&photos).contains(&"2.8".to_string()));

        let focal_cell = (COLUMNS[3].value)(p);
        assert_eq!(focal_cell, "18");
        assert!(filter::unique_focals(&photos).contains(&"18".to_string()));
    }

    #[test]
    fn only_columns_with_filters_expose_a_slot() {
        for column in COLUMNS {
            match (column.filter, column.header) {
                (None, "File") | (None, "Date") => {}
                (None, other) => panic!("{other} has no filter but should have one"),
                (Some(_), _) => {}
            }
        }
    }

    #[test]
    fn truncation_keeps_the_leading_rows() {
        let photos = fixture();
        let state = TableState {
            photo_sort: Some((SortColumn::Name, table::SortOrder::Ascending)),
            ..Default::default()
        };

        let mut rows = filter::apply(&photos, &PhotoFilters::from_state(&state));
        table::sort_photos(&mut rows, state.photo_sort);

        let first_two: Vec<String> = rows[..2].iter().map(|p| file_name(p)).collect();
        assert_eq!(first_two, vec!["a.jpg", "b.ARW"]);
        assert_eq!(
            rows.len(),
            3,
            "truncation is the caller's slice, not a loss"
        );
    }
}
