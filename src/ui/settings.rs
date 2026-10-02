//! The Settings window: charts, tables, chart style, and EXIF column visibility.
//!
//! The column pickers used to be ~800 lines of hand-written `ui.checkbox` calls.
//! They are now declared once as data in the [`column_group!`] tables below,
//! where each entry names the struct field *as a literal identifier*, so the
//! label and the field it drives cannot drift apart and a typo is a compile
//! error rather than a checkbox that silently does nothing.

use eframe::egui;

use crate::config::{ApertureScale, ChartType, DisplayConfig, PhotoTableColumns};

/// Render the Settings window if the user has opened it.
pub fn render(app: &mut crate::app::CameraStatsApp, ctx: &egui::Context) {
    // The window's open/close flag and the body both need `app`, so take the
    // flag first and leave the struct fully borrowable for the closure.
    let mut open = app.show_settings;

    egui::Window::new("⚙️ Display Settings")
        .open(&mut open)
        .default_width(400.0)
        .show(ctx, |ui| {
            visibility_section(ui, &mut app.display_config);
            ui.separator();
            style_section(ui, &mut app.display_config);
            ui.separator();
            columns_section(ui, &mut app.display_config.photo_table_columns);
            ui.separator();

            if ui.button("Reset to Defaults").clicked() {
                app.display_config = DisplayConfig::default();
                app.log_ui("settings reset to defaults");
            }
        });

    // Written back after the closure so the close button takes effect.
    app.show_settings = open;
}

/// Checkboxes for which charts and tables are visible.
fn visibility_section(ui: &mut egui::Ui, config: &mut DisplayConfig) {
    ui.heading("Charts");
    ui.checkbox(&mut config.show_iso_chart, "ISO Distribution");
    ui.checkbox(&mut config.show_aperture_chart, "Aperture Distribution");
    ui.checkbox(&mut config.show_focal_chart, "Focal Length Distribution");

    ui.separator();
    ui.heading("Tables");
    ui.checkbox(&mut config.show_camera_table, "Camera Table");
    ui.checkbox(&mut config.show_lens_table, "Lens Table");
    ui.checkbox(&mut config.show_photo_table, "Photo Details Table");
}

/// Chart rendering style, aperture axis scaling, and table row limits.
fn style_section(ui: &mut egui::Ui, config: &mut DisplayConfig) {
    ui.heading("Chart Style");
    egui::ComboBox::from_label("Chart Type")
        .selected_text(format!("{:?}", config.chart_type))
        .show_ui(ui, |ui| {
            ui.selectable_value(&mut config.chart_type, ChartType::Bars, "Bars");
            ui.selectable_value(&mut config.chart_type, ChartType::Lines, "Lines");
            ui.selectable_value(&mut config.chart_type, ChartType::Points, "Points");
        });

    ui.separator();
    ui.heading("Aperture Scale");
    egui::ComboBox::from_label("Scale")
        .selected_text(format!("{:?}", config.aperture_scale))
        .show_ui(ui, |ui| {
            ui.selectable_value(
                &mut config.aperture_scale,
                ApertureScale::Linear,
                "Linear (f/1.0, f/2.0...)",
            );
            ui.selectable_value(
                &mut config.aperture_scale,
                ApertureScale::Logarithmic,
                "Logarithmic",
            );
            ui.selectable_value(
                &mut config.aperture_scale,
                ApertureScale::FStops,
                "Standard F-Stops",
            );
        });

    ui.separator();
    ui.heading("Table Limits");
    ui.add(
        egui::DragValue::new(&mut config.max_table_rows)
            .range(5..=100)
            .prefix("Max rows: "),
    );
}

/// Scrollable picker for every optional EXIF column, grouped by availability.
fn columns_section(ui: &mut egui::Ui, columns: &mut PhotoTableColumns) {
    ui.heading("Photo Table Columns");
    egui::ScrollArea::vertical()
        .max_height(300.0)
        .show(ui, |ui| {
            for (title, group) in ALL_GROUPS {
                ui.strong(*title);
                render_group(ui, columns, group);
                ui.separator();
            }
        });
}

/// One optional column in the Settings picker.
pub struct ColumnDef {
    /// Name of the backing `PhotoTableColumns` field, for tests and error
    /// messages. Field access goes through `get`/`set`, not through this string.
    pub field: &'static str,
    /// Text shown next to the checkbox.
    pub label: &'static str,
    /// Reads the current value of the backing field.
    pub get: fn(&PhotoTableColumns) -> bool,
    /// Assigns the checkbox value to the backing field.
    ///
    /// Getters and setters are function pointers rather than string keys,
    /// because Rust cannot turn a runtime string into a field access.
    pub set: fn(&mut PhotoTableColumns, bool),
}

/// Build one [`ColumnDef`] from a literal field name.
///
/// Non-capturing closures coerce to `fn` pointers, so `$field` stays a real
/// identifier here: a misspelling fails to compile instead of producing a
/// checkbox that does nothing.
macro_rules! column_def {
    ($field:ident, $label:expr) => {
        ColumnDef {
            field: stringify!($field),
            label: $label,
            get: |c: &PhotoTableColumns| c.$field,
            set: |c: &mut PhotoTableColumns, v: bool| c.$field = v,
        }
    };
}

/// Build a named group of [`ColumnDef`]s from `(field, label)` pairs.
macro_rules! column_group {
    ($name:ident, $(($field:ident, $label:expr)),* $(,)?) => {
        #[doc = concat!("Columns declared by the `", stringify!($name), "` group.")]
        pub const $name: &[ColumnDef] = &[$(column_def!($field, $label)),*];
    };
}

column_group!(
    DEFAULT,
    (show_file, "File"),
    (show_date, "Date"),
    (show_aperture, "Aperture"),
    (show_focal_length, "Focal Length"),
    (show_camera, "Camera"),
    (show_lens, "Lens"),
    (show_type, "Type"),
);

column_group!(
    JPEG,
    (show_make, "Make"),
    (show_software, "Software"),
    (show_artist, "Artist"),
    (show_copyright, "Copyright"),
    (show_lens_make, "Lens Make"),
    (show_lens_specification, "Lens Specification"),
    (show_exposure_time, "Exposure Time"),
    (show_shutter_speed_value, "Shutter Speed Value"),
    (show_exposure_program, "Exposure Program"),
    (show_exposure_mode, "Exposure Mode"),
    (show_exposure_bias_value, "Exposure Bias"),
    (show_metering_mode, "Metering Mode"),
    (show_flash, "Flash"),
    (show_white_balance, "White Balance"),
    (show_color_space, "Color Space"),
    (show_scene_capture_type, "Scene Capture Type"),
    (show_gain_control, "Gain Control"),
    (show_contrast, "Contrast"),
    (show_saturation, "Saturation"),
    (show_sharpness, "Sharpness"),
    (show_body_serial_number, "Body Serial Number"),
    (show_camera_owner_name, "Camera Owner Name"),
    (show_gps_latitude, "GPS Latitude"),
    (show_gps_longitude, "GPS Longitude"),
    (show_gps_altitude, "GPS Altitude"),
    (show_orientation, "Orientation"),
    (show_x_resolution, "X Resolution"),
    (show_y_resolution, "Y Resolution"),
    (show_temperature, "Temperature"),
    (show_humidity, "Humidity"),
    (show_pressure, "Pressure"),
    (show_exif_version, "Exif Version"),
    (show_flashpix_version, "Flashpix Version"),
    (show_sensing_method, "Sensing Method"),
);

column_group!(
    RAW,
    (show_raw_width, "Raw Width"),
    (show_raw_height, "Raw Height"),
    (show_raw_cpp, "Raw Components/Pixel"),
    (show_raw_bps, "Raw Bits/Sample"),
    (show_black_level, "Black Level"),
    (show_white_level, "White Level"),
    (show_active_area, "Active Area"),
    (show_crop_area, "Crop Area"),
    (show_raw_cfa_pattern, "CFA Pattern"),
    (show_color_matrix_1, "Color Matrix 1"),
    (show_color_matrix_2, "Color Matrix 2"),
    (show_wb_coeffs, "WB Coefficients"),
    (show_xyz_to_cam, "XYZ to Camera Matrix"),
    (show_clean_make, "Clean Make"),
    (show_clean_model, "Clean Model"),
    (show_default_scale, "Default Scale"),
    (show_best_quality_scale, "Best Quality Scale"),
);

column_group!(
    BOTH,
    (show_photographic_sensitivity, "Photographic Sensitivity"),
    (show_iso_speed, "ISO Speed"),
    (
        show_recommended_exposure_index,
        "Recommended Exposure Index"
    ),
    (
        show_standard_output_sensitivity,
        "Standard Output Sensitivity"
    ),
    (show_sensitivity_type, "Sensitivity Type"),
    (show_aperture_value, "Aperture Value (EV)"),
    (show_max_aperture_value, "Max Aperture Value (EV)"),
    (show_focal_length_in_35mm_film, "Focal Length (35mm equiv)"),
    (show_focal_plane_x_resolution, "Focal Plane X Resolution"),
    (show_focal_plane_y_resolution, "Focal Plane Y Resolution"),
    (
        show_focal_plane_resolution_unit,
        "Focal Plane Resolution Unit"
    ),
    (show_flash_energy, "Flash Energy"),
    (show_subject_distance, "Subject Distance"),
    (show_subject_distance_range, "Subject Distance Range"),
    (show_custom_rendered, "Custom Rendered"),
    (show_gamma, "Gamma"),
    (show_image_description, "Image Description"),
    (show_user_comment, "User Comment"),
    (show_maker_note, "Maker Note"),
    (show_image_unique_id, "Image Unique ID"),
    (show_lens_serial_number, "Lens Serial Number"),
    (show_date_time_original, "Date Time Original"),
    (show_date_time_digitized, "Date Time Digitized"),
    (show_sub_sec_time, "Sub-second Time"),
    (show_sub_sec_time_original, "Sub-second Time Original"),
    (show_sub_sec_time_digitized, "Sub-second Time Digitized"),
    (show_offset_time, "Offset Time"),
    (show_offset_time_original, "Offset Time Original"),
    (show_offset_time_digitized, "Offset Time Digitized"),
    (show_gps_latitude_ref, "GPS Latitude Ref"),
    (show_gps_longitude_ref, "GPS Longitude Ref"),
    (show_gps_altitude_ref, "GPS Altitude Ref"),
    (show_gps_timestamp, "GPS Timestamp"),
    (show_gps_satellites, "GPS Satellites"),
    (show_gps_status, "GPS Status"),
    (show_gps_measure_mode, "GPS Measure Mode"),
    (show_gps_dop, "GPS DOP"),
    (show_gps_speed, "GPS Speed"),
    (show_gps_speed_ref, "GPS Speed Ref"),
    (show_gps_track, "GPS Track"),
    (show_gps_track_ref, "GPS Track Ref"),
    (show_gps_img_direction, "GPS Img Direction"),
    (show_gps_img_direction_ref, "GPS Img Direction Ref"),
    (show_gps_map_datum, "GPS Map Datum"),
    (show_gps_date_stamp, "GPS Date Stamp"),
    (show_gps_processing_method, "GPS Processing Method"),
    (show_gps_area_information, "GPS Area Information"),
    (show_gps_differential, "GPS Differential"),
    (show_gps_h_positioning_error, "GPS H Positioning Error"),
    (show_resolution_unit, "Resolution Unit"),
    (
        show_photometric_interpretation,
        "Photometric Interpretation"
    ),
    (show_compression, "Compression"),
    (show_bits_per_sample, "Bits Per Sample"),
    (show_samples_per_pixel, "Samples Per Pixel"),
    (show_planar_configuration, "Planar Configuration"),
    (show_ycbcr_sub_sampling, "YCbCr Sub Sampling"),
    (show_ycbcr_positioning, "YCbCr Positioning"),
    (show_reference_black_white, "Reference Black/White"),
    (show_white_point, "White Point"),
    (show_primary_chromaticities, "Primary Chromaticities"),
    (show_ycbcr_coefficients, "YCbCr Coefficients"),
    (show_water_depth, "Water Depth"),
    (show_acceleration, "Acceleration"),
    (show_camera_elevation_angle, "Camera Elevation Angle"),
    (show_components_configuration, "Components Configuration"),
    (show_compressed_bits_per_pixel, "Compressed Bits/Pixel"),
    (show_file_source, "File Source"),
    (show_scene_type, "Scene Type"),
    (show_cfa_pattern, "CFA Pattern"),
    (
        show_device_setting_description,
        "Device Setting Description"
    ),
    (show_subject_area, "Subject Area"),
    (show_related_sound_file, "Related Sound File"),
    (
        show_spatial_frequency_response,
        "Spatial Frequency Response"
    ),
    (show_spectral_sensitivity, "Spectral Sensitivity"),
    (show_oecf, "OECF"),
    (show_width, "Width"),
    (show_height, "Height"),
    (show_raw_orientation, "Raw Orientation"),
    (show_fuji_rotation_width, "Fuji Rotation Width"),
    (show_camera_mode, "Camera Mode"),
    (show_remark, "Remark"),
);

/// Every group paired with its heading, in display order.
///
/// Rendering iterates this rather than a hand-written list, so the picker and
/// the tests always agree on which groups exist and in what order.
pub const ALL_GROUPS: &[(&str, &[ColumnDef])] = &[
    ("Default Columns (Always Visible)", DEFAULT),
    ("JPEG Only", JPEG),
    ("RAW Only", RAW),
    ("Available in Both", BOTH),
];

/// Render one group of column checkboxes.
///
/// The value is read into a local, passed to the checkbox, and written back
/// only on a change. Writing through `set` on every frame would be correct but
/// would mark the struct dirty for no reason.
fn render_group(ui: &mut egui::Ui, columns: &mut PhotoTableColumns, group: &[ColumnDef]) {
    for column in group {
        let mut value = (column.get)(columns);
        let response = ui.add(egui::Checkbox::new(&mut value, column.label));
        if response.changed() {
            (column.set)(columns, value);
        }
        // The tooltip names the backing field, which makes it obvious which EXIF
        // tag a label corresponds to when reporting a mislabelled column.
        // `on_hover_text` rather than `Checkbox::tooltip`, which this egui
        // version does not provide.
        response.on_hover_text(format!("PhotoTableColumns::{}", column.field));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    /// The field names of a default `PhotoTableColumns`, taken from its
    /// serialized form so the test tracks the struct rather than a copied list.
    ///
    /// Returns owned `String`s because the names are owned by the parsed
    /// `serde_json::Value`; a `&'static str` is not available here.
    fn declared_fields() -> HashSet<String> {
        let value: serde_json::Value = serde_json::to_value(PhotoTableColumns::default()).unwrap();
        value
            .as_object()
            .expect("PhotoTableColumns serializes as a struct")
            .keys()
            .cloned()
            .collect()
    }

    /// Every column across every group, in display order.
    fn all_columns() -> impl Iterator<Item = &'static ColumnDef> {
        ALL_GROUPS.iter().flat_map(|(_, group)| group.iter())
    }

    #[test]
    fn groups_cover_every_field_exactly_once() {
        let declared = declared_fields();
        let listed: Vec<String> = all_columns().map(|c| c.field.to_string()).collect();

        let unique: HashSet<&String> = listed.iter().collect();
        assert_eq!(
            unique.len(),
            listed.len(),
            "a column appears in more than one group"
        );

        let listed_set: HashSet<String> = listed.into_iter().collect();
        assert_eq!(
            declared, listed_set,
            "Settings columns and PhotoTableColumns fields disagree"
        );
    }

    #[test]
    fn labels_are_not_blank() {
        for column in all_columns() {
            assert!(!column.label.trim().is_empty(), "{} is blank", column.field);
        }
    }

    /// A setter must write the field its getter reads; if the two ever drift the
    /// checkbox would appear to work while changing nothing.
    #[test]
    fn setters_write_the_field_getters_read() {
        let mut columns = PhotoTableColumns::default();
        for column in all_columns() {
            let before = (column.get)(&columns);
            (column.set)(&mut columns, !before);
            assert_eq!(
                (column.get)(&columns),
                !before,
                "setter for {} did not change what its getter reads",
                column.field
            );
            // Restore so later iterations start from the documented default.
            (column.set)(&mut columns, before);
        }
    }
}
