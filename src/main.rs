use std::path::PathBuf;
use std::sync::mpsc;
use std::thread;
use std::time::Instant;

use anyhow::Result;
use chrono::Local;
use eframe::egui;
use egui_plot::{Bar, BarChart, Line, Plot, PlotPoints};
use rfd::FileDialog;
use tracing::{error, info};

use camera_stats::{export_csv, PhotoMeta, Stats, scan_directory, logging::*};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct DisplayConfig {
    show_iso_chart: bool,
    show_aperture_chart: bool,
    show_focal_chart: bool,
    show_camera_table: bool,
    show_lens_table: bool,
    show_photo_table: bool,
    chart_type: ChartType,
    aperture_scale: ApertureScale,
    max_table_rows: usize,
    photo_table_columns: PhotoTableColumns,
}

impl Default for DisplayConfig {
    fn default() -> Self {
        Self {
            show_iso_chart: true,
            show_aperture_chart: true,
            show_focal_chart: true,
            show_camera_table: true,
            show_lens_table: true,
            show_photo_table: true,
            chart_type: ChartType::Bars,
            aperture_scale: ApertureScale::Linear,
            max_table_rows: 10,
            photo_table_columns: PhotoTableColumns::default(),
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct PhotoTableColumns {
    // Always shown (defaults)
    show_file: bool,
    show_date: bool,
    show_aperture: bool,
    show_focal_length: bool,
    show_camera: bool,
    show_lens: bool,
    show_type: bool,
    
    // JPEG only
    show_make: bool,
    show_software: bool,
    show_artist: bool,
    show_copyright: bool,
    show_lens_make: bool,
    show_lens_specification: bool,
    show_exposure_time: bool,
    show_shutter_speed_value: bool,
    show_exposure_program: bool,
    show_exposure_mode: bool,
    show_exposure_bias_value: bool,
    show_metering_mode: bool,
    show_flash: bool,
    show_white_balance: bool,
    show_color_space: bool,
    show_scene_capture_type: bool,
    show_gain_control: bool,
    show_contrast: bool,
    show_saturation: bool,
    show_sharpness: bool,
    show_body_serial_number: bool,
    show_camera_owner_name: bool,
    show_gps_latitude: bool,
    show_gps_longitude: bool,
    show_gps_altitude: bool,
    show_orientation: bool,
    show_x_resolution: bool,
    show_y_resolution: bool,
    show_temperature: bool,
    show_humidity: bool,
    show_pressure: bool,
    show_exif_version: bool,
    show_flashpix_version: bool,
    show_sensing_method: bool,
    
    // RAW only
    show_raw_width: bool,
    show_raw_height: bool,
    show_raw_cpp: bool,
    show_raw_bps: bool,
    show_black_level: bool,
    show_white_level: bool,
    show_active_area: bool,
    show_crop_area: bool,
    show_raw_cfa_pattern: bool,
    show_color_matrix_1: bool,
    show_color_matrix_2: bool,
    show_wb_coeffs: bool,
    show_xyz_to_cam: bool,
    show_clean_make: bool,
    show_clean_model: bool,
    show_default_scale: bool,
    show_best_quality_scale: bool,
    
    // Both (available in both but not defaults)
    show_photographic_sensitivity: bool,
    show_iso_speed: bool,
    show_recommended_exposure_index: bool,
    show_standard_output_sensitivity: bool,
    show_sensitivity_type: bool,
    show_aperture_value: bool,
    show_max_aperture_value: bool,
    show_focal_length_in_35mm_film: bool,
    show_focal_plane_x_resolution: bool,
    show_focal_plane_y_resolution: bool,
    show_focal_plane_resolution_unit: bool,
    show_flash_energy: bool,
    show_subject_distance: bool,
    show_subject_distance_range: bool,
    show_custom_rendered: bool,
    show_gamma: bool,
    show_image_description: bool,
    show_user_comment: bool,
    show_maker_note: bool,
    show_image_unique_id: bool,
    show_lens_serial_number: bool,
    show_date_time_original: bool,
    show_date_time_digitized: bool,
    show_sub_sec_time: bool,
    show_sub_sec_time_original: bool,
    show_sub_sec_time_digitized: bool,
    show_offset_time: bool,
    show_offset_time_original: bool,
    show_offset_time_digitized: bool,
    show_gps_latitude_ref: bool,
    show_gps_longitude_ref: bool,
    show_gps_altitude_ref: bool,
    show_gps_timestamp: bool,
    show_gps_satellites: bool,
    show_gps_status: bool,
    show_gps_measure_mode: bool,
    show_gps_dop: bool,
    show_gps_speed: bool,
    show_gps_speed_ref: bool,
    show_gps_track: bool,
    show_gps_track_ref: bool,
    show_gps_img_direction: bool,
    show_gps_img_direction_ref: bool,
    show_gps_map_datum: bool,
    show_gps_date_stamp: bool,
    show_gps_processing_method: bool,
    show_gps_area_information: bool,
    show_gps_differential: bool,
    show_gps_h_positioning_error: bool,
    show_resolution_unit: bool,
    show_photometric_interpretation: bool,
    show_compression: bool,
    show_bits_per_sample: bool,
    show_samples_per_pixel: bool,
    show_planar_configuration: bool,
    show_ycbcr_sub_sampling: bool,
    show_ycbcr_positioning: bool,
    show_reference_black_white: bool,
    show_white_point: bool,
    show_primary_chromaticities: bool,
    show_ycbcr_coefficients: bool,
    show_water_depth: bool,
    show_acceleration: bool,
    show_camera_elevation_angle: bool,
    show_components_configuration: bool,
    show_compressed_bits_per_pixel: bool,
    show_file_source: bool,
    show_scene_type: bool,
    show_cfa_pattern: bool,
    show_device_setting_description: bool,
    show_subject_area: bool,
    show_related_sound_file: bool,
    show_spatial_frequency_response: bool,
    show_spectral_sensitivity: bool,
    show_oecf: bool,
    show_width: bool,
    show_height: bool,
    show_raw_orientation: bool,
    show_fuji_rotation_width: bool,
    show_camera_mode: bool,
    show_remark: bool,
}
impl Default for PhotoTableColumns {
    fn default() -> Self {
        Self {
            // Defaults - always shown
            show_file: true,
            show_date: true,
            show_aperture: true,
            show_focal_length: true,
            show_camera: true,
            show_lens: true,
            show_type: true,
            
            // JPEG only - hidden by default
            show_make: false,
            show_software: false,
            show_artist: false,
            show_copyright: false,
            show_lens_make: false,
            show_lens_specification: false,
            show_exposure_time: false,
            show_shutter_speed_value: false,
            show_exposure_program: false,
            show_exposure_mode: false,
            show_exposure_bias_value: false,
            show_metering_mode: false,
            show_flash: false,
            show_white_balance: false,
            show_color_space: false,
            show_scene_capture_type: false,
            show_gain_control: false,
            show_contrast: false,
            show_saturation: false,
            show_sharpness: false,
            show_body_serial_number: false,
            show_camera_owner_name: false,
            show_gps_latitude: false,
            show_gps_longitude: false,
            show_gps_altitude: false,
            show_orientation: false,
            show_x_resolution: false,
            show_y_resolution: false,
            show_temperature: false,
            show_humidity: false,
            show_pressure: false,
            show_exif_version: false,
            show_flashpix_version: false,
            show_sensing_method: false,
            
            // RAW only - hidden by default
            show_raw_width: false,
            show_raw_height: false,
            show_raw_cpp: false,
            show_raw_bps: false,
            show_black_level: false,
            show_white_level: false,
            show_active_area: false,
            show_crop_area: false,
            show_raw_cfa_pattern: false,
            show_color_matrix_1: false,
            show_color_matrix_2: false,
            show_wb_coeffs: false,
            show_xyz_to_cam: false,
            show_clean_make: false,
            show_clean_model: false,
            show_default_scale: false,
            show_best_quality_scale: false,
            
            // Both - hidden by default
            show_photographic_sensitivity: false,
            show_iso_speed: false,
            show_recommended_exposure_index: false,
            show_standard_output_sensitivity: false,
            show_sensitivity_type: false,
            show_aperture_value: false,
            show_max_aperture_value: false,
            show_focal_length_in_35mm_film: false,
            show_focal_plane_x_resolution: false,
            show_focal_plane_y_resolution: false,
            show_focal_plane_resolution_unit: false,
            show_flash_energy: false,
            show_subject_distance: false,
            show_subject_distance_range: false,
            show_custom_rendered: false,
            show_gamma: false,
            show_image_description: false,
            show_user_comment: false,
            show_maker_note: false,
            show_image_unique_id: false,
            show_lens_serial_number: false,
            show_date_time_original: false,
            show_date_time_digitized: false,
            show_sub_sec_time: false,
            show_sub_sec_time_original: false,
            show_sub_sec_time_digitized: false,
            show_offset_time: false,
            show_offset_time_original: false,
            show_offset_time_digitized: false,
            show_gps_latitude_ref: false,
            show_gps_longitude_ref: false,
            show_gps_altitude_ref: false,
            show_gps_timestamp: false,
            show_gps_satellites: false,
            show_gps_status: false,
            show_gps_measure_mode: false,
            show_gps_dop: false,
            show_gps_speed: false,
            show_gps_speed_ref: false,
            show_gps_track: false,
            show_gps_track_ref: false,
            show_gps_img_direction: false,
            show_gps_img_direction_ref: false,
            show_gps_map_datum: false,
            show_gps_date_stamp: false,
            show_gps_processing_method: false,
            show_gps_area_information: false,
            show_gps_differential: false,
            show_gps_h_positioning_error: false,
            show_resolution_unit: false,
            show_photometric_interpretation: false,
            show_compression: false,
            show_bits_per_sample: false,
            show_samples_per_pixel: false,
            show_planar_configuration: false,
            show_ycbcr_sub_sampling: false,
            show_ycbcr_positioning: false,
            show_reference_black_white: false,
            show_white_point: false,
            show_primary_chromaticities: false,
            show_ycbcr_coefficients: false,
            show_water_depth: false,
            show_acceleration: false,
            show_camera_elevation_angle: false,
            show_components_configuration: false,
            show_compressed_bits_per_pixel: false,
            show_file_source: false,
            show_scene_type: false,
            show_cfa_pattern: false,
            show_device_setting_description: false,
            show_subject_area: false,
            show_related_sound_file: false,
            show_spatial_frequency_response: false,
            show_spectral_sensitivity: false,
            show_oecf: false,
            show_width: false,
            show_height: false,
            show_raw_orientation: false,
            show_fuji_rotation_width: false,
            show_camera_mode: false,
            show_remark: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
enum SortColumn {
    Name,
    Count,
    Aperture,
    FocalLength,
    Camera,
    Lens,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
enum SortOrder {
    Ascending,
    Descending,
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
struct TableState {
    photo_sort: Option<(SortColumn, SortOrder)>,
    photo_camera_filter: std::collections::HashSet<String>,
    photo_lens_filter: std::collections::HashSet<String>,
    photo_aperture_filter: std::collections::HashSet<String>,
    photo_focal_filter: std::collections::HashSet<String>,
    photo_type_filter: std::collections::HashSet<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
enum ChartType {
    Bars,
    Lines,
    Points,
}

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
enum ApertureScale {
    Linear,
    Logarithmic,
    FStops,
}

struct CameraStatsApp {
    photos: Vec<PhotoMeta>,
    stats: Stats,
    selected_dir: Option<PathBuf>,
    status: String,
    scanning: bool,
    scan_rx: Option<mpsc::Receiver<Vec<PhotoMeta>>>,
    scan_start: Option<Instant>,
    log_guard: Option<tracing_appender::non_blocking::WorkerGuard>,
    show_export_dialog: bool,
    show_settings: bool,
    display_config: DisplayConfig,
    table_state: TableState,
}

impl Default for CameraStatsApp {
    fn default() -> Self {
        let log_dir = dirs::data_local_dir().map(|d| d.join("camera_stats").join("logs"));
        let log_guard = init_logging(log_dir.as_deref()).ok().flatten();

        info!("Camera Stats started");
        log_ui_event("Application initialized");

        Self {
            photos: Vec::new(),
            stats: Stats::default(),
            selected_dir: None,
            status: "Select a folder to scan".into(),
            scanning: false,
            scan_rx: None,
            scan_start: None,
            log_guard,
            show_export_dialog: false,
            show_settings: false,
            display_config: DisplayConfig::default(),
            table_state: TableState::default(),
        }
    }
}

impl CameraStatsApp {
    fn render_chart(&self, ui: &mut egui::Ui, id: &str, label: &str, points: PlotPoints) {
        match self.display_config.chart_type {
            ChartType::Bars => {
                let bars: Vec<Bar> = points.points().iter().map(|p| {
                    Bar::new(p.x, p.y).width(0.8)
                }).collect();
                let chart = BarChart::new(bars)
                    .name(label)
                    .width(0.8)
                    .element_formatter(Box::new(move |bar, _| format!("{}: {:.0}", bar.argument, bar.value)));
                Plot::new(id)
                    .height(200.0)
                    .show(ui, |plot_ui| plot_ui.bar_chart(chart));
            }
            ChartType::Lines => {
                let line = Line::new(points)
                    .name(label)
                    .fill(0.0_f32)
                    .highlight(true);
                Plot::new(id)
                    .height(200.0)
                    .show(ui, |plot_ui| plot_ui.line(line));
            }
            ChartType::Points => {
                let line = Line::new(points)
                    .name(label)
                    .highlight(true);
                Plot::new(id)
                    .height(200.0)
                    .show(ui, |plot_ui| plot_ui.line(line));
            }
        }
    }

    fn render_settings(&mut self, ctx: &egui::Context) {
        egui::Window::new("⚙️ Display Settings")
            .open(&mut self.show_settings)
            .default_width(400.0)
            .show(ctx, |ui| {
                ui.heading("Charts");
                ui.checkbox(&mut self.display_config.show_iso_chart, "ISO Distribution");
                ui.checkbox(&mut self.display_config.show_aperture_chart, "Aperture Distribution");
                ui.checkbox(&mut self.display_config.show_focal_chart, "Focal Length Distribution");

                ui.separator();
                ui.heading("Tables");
                ui.checkbox(&mut self.display_config.show_camera_table, "Camera Table");
                ui.checkbox(&mut self.display_config.show_lens_table, "Lens Table");
                ui.checkbox(&mut self.display_config.show_photo_table, "Photo Details Table");

                ui.separator();
                ui.heading("Chart Style");
                egui::ComboBox::from_label("Chart Type")
                    .selected_text(format!("{:?}", self.display_config.chart_type))
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.display_config.chart_type, ChartType::Bars, "Bars");
                    ui.selectable_value(&mut self.display_config.chart_type, ChartType::Lines, "Lines");
                    ui.selectable_value(&mut self.display_config.chart_type, ChartType::Points, "Points");
                });

                ui.separator();
                ui.heading("Aperture Scale");
                egui::ComboBox::from_label("Scale")
                    .selected_text(format!("{:?}", self.display_config.aperture_scale))
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut self.display_config.aperture_scale, ApertureScale::Linear, "Linear (f/1.0, f/2.0...)");
                        ui.selectable_value(&mut self.display_config.aperture_scale, ApertureScale::Logarithmic, "Logarithmic");
                        ui.selectable_value(&mut self.display_config.aperture_scale, ApertureScale::FStops, "Standard F-Stops");
                    });

                ui.separator();
                ui.heading("Table Limits");
                ui.add(egui::DragValue::new(&mut self.display_config.max_table_rows).range(5..=100).prefix("Max rows: "));

                ui.separator();
                ui.heading("Photo Table Columns");
                egui::ScrollArea::vertical().max_height(300.0).show(ui, |ui| {
                    // Default columns (always shown)
                    ui.group(|ui| {
                        ui.strong("Default Columns (Always Visible)");
                        ui.checkbox(&mut self.display_config.photo_table_columns.show_file, "File");
                        ui.checkbox(&mut self.display_config.photo_table_columns.show_date, "Date");
                        ui.checkbox(&mut self.display_config.photo_table_columns.show_aperture, "Aperture");
                        ui.checkbox(&mut self.display_config.photo_table_columns.show_focal_length, "Focal Length");
                        ui.checkbox(&mut self.display_config.photo_table_columns.show_camera, "Camera");
                        ui.checkbox(&mut self.display_config.photo_table_columns.show_lens, "Lens");
                        ui.checkbox(&mut self.display_config.photo_table_columns.show_type, "Type");
                    });

                    ui.separator();

                    // JPEG only columns
                    ui.group(|ui| {
                        ui.strong("JPEG Only");
                        ui.checkbox(&mut self.display_config.photo_table_columns.show_make, "Make");
                        ui.checkbox(&mut self.display_config.photo_table_columns.show_software, "Software");
                        ui.checkbox(&mut self.display_config.photo_table_columns.show_artist, "Artist");
                        ui.checkbox(&mut self.display_config.photo_table_columns.show_copyright, "Copyright");
                        ui.checkbox(&mut self.display_config.photo_table_columns.show_lens_make, "Lens Make");
                        ui.checkbox(&mut self.display_config.photo_table_columns.show_lens_specification, "Lens Specification");
                        ui.checkbox(&mut self.display_config.photo_table_columns.show_exposure_time, "Exposure Time");
                        ui.checkbox(&mut self.display_config.photo_table_columns.show_shutter_speed_value, "Shutter Speed Value");
                        ui.checkbox(&mut self.display_config.photo_table_columns.show_exposure_program, "Exposure Program");
                        ui.checkbox(&mut self.display_config.photo_table_columns.show_exposure_mode, "Exposure Mode");
                        ui.checkbox(&mut self.display_config.photo_table_columns.show_exposure_bias_value, "Exposure Bias");
                        ui.checkbox(&mut self.display_config.photo_table_columns.show_metering_mode, "Metering Mode");
                        ui.checkbox(&mut self.display_config.photo_table_columns.show_flash, "Flash");
                        ui.checkbox(&mut self.display_config.photo_table_columns.show_white_balance, "White Balance");
                        ui.checkbox(&mut self.display_config.photo_table_columns.show_color_space, "Color Space");
                        ui.checkbox(&mut self.display_config.photo_table_columns.show_scene_capture_type, "Scene Capture Type");
                        ui.checkbox(&mut self.display_config.photo_table_columns.show_gain_control, "Gain Control");
                        ui.checkbox(&mut self.display_config.photo_table_columns.show_contrast, "Contrast");
                        ui.checkbox(&mut self.display_config.photo_table_columns.show_saturation, "Saturation");
                        ui.checkbox(&mut self.display_config.photo_table_columns.show_sharpness, "Sharpness");
                        ui.checkbox(&mut self.display_config.photo_table_columns.show_body_serial_number, "Body Serial Number");
                        ui.checkbox(&mut self.display_config.photo_table_columns.show_camera_owner_name, "Camera Owner Name");
                        ui.checkbox(&mut self.display_config.photo_table_columns.show_gps_latitude, "GPS Latitude");
                        ui.checkbox(&mut self.display_config.photo_table_columns.show_gps_longitude, "GPS Longitude");
                        ui.checkbox(&mut self.display_config.photo_table_columns.show_gps_altitude, "GPS Altitude");
                        ui.checkbox(&mut self.display_config.photo_table_columns.show_orientation, "Orientation");
                        ui.checkbox(&mut self.display_config.photo_table_columns.show_x_resolution, "X Resolution");
                        ui.checkbox(&mut self.display_config.photo_table_columns.show_y_resolution, "Y Resolution");
                        ui.checkbox(&mut self.display_config.photo_table_columns.show_temperature, "Temperature");
                        ui.checkbox(&mut self.display_config.photo_table_columns.show_humidity, "Humidity");
                        ui.checkbox(&mut self.display_config.photo_table_columns.show_pressure, "Pressure");
                        ui.checkbox(&mut self.display_config.photo_table_columns.show_exif_version, "Exif Version");
                        ui.checkbox(&mut self.display_config.photo_table_columns.show_flashpix_version, "Flashpix Version");
                        ui.checkbox(&mut self.display_config.photo_table_columns.show_sensing_method, "Sensing Method");
                    });

                    ui.separator();

                    // RAW only columns
                    ui.group(|ui| {
                        ui.strong("RAW Only");
                        ui.checkbox(&mut self.display_config.photo_table_columns.show_raw_width, "Raw Width");
                        ui.checkbox(&mut self.display_config.photo_table_columns.show_raw_height, "Raw Height");
                        ui.checkbox(&mut self.display_config.photo_table_columns.show_raw_cpp, "Raw Components/Pixel");
                        ui.checkbox(&mut self.display_config.photo_table_columns.show_raw_bps, "Raw Bits/Sample");
                        ui.checkbox(&mut self.display_config.photo_table_columns.show_black_level, "Black Level");
                        ui.checkbox(&mut self.display_config.photo_table_columns.show_white_level, "White Level");
                        ui.checkbox(&mut self.display_config.photo_table_columns.show_active_area, "Active Area");
                        ui.checkbox(&mut self.display_config.photo_table_columns.show_crop_area, "Crop Area");
                        ui.checkbox(&mut self.display_config.photo_table_columns.show_raw_cfa_pattern, "CFA Pattern");
                        ui.checkbox(&mut self.display_config.photo_table_columns.show_color_matrix_1, "Color Matrix 1");
                        ui.checkbox(&mut self.display_config.photo_table_columns.show_color_matrix_2, "Color Matrix 2");
                        ui.checkbox(&mut self.display_config.photo_table_columns.show_wb_coeffs, "WB Coefficients");
                        ui.checkbox(&mut self.display_config.photo_table_columns.show_xyz_to_cam, "XYZ to Camera Matrix");
                        ui.checkbox(&mut self.display_config.photo_table_columns.show_clean_make, "Clean Make");
                        ui.checkbox(&mut self.display_config.photo_table_columns.show_clean_model, "Clean Model");
                        ui.checkbox(&mut self.display_config.photo_table_columns.show_default_scale, "Default Scale");
                        ui.checkbox(&mut self.display_config.photo_table_columns.show_best_quality_scale, "Best Quality Scale");
                    });

                    ui.separator();

                    // Both (available in both JPEG and RAW)
                    ui.group(|ui| {
                        ui.strong("Available in Both");
                        ui.checkbox(&mut self.display_config.photo_table_columns.show_photographic_sensitivity, "Photographic Sensitivity");
                        ui.checkbox(&mut self.display_config.photo_table_columns.show_iso_speed, "ISO Speed");
                        ui.checkbox(&mut self.display_config.photo_table_columns.show_recommended_exposure_index, "Recommended Exposure Index");
                        ui.checkbox(&mut self.display_config.photo_table_columns.show_standard_output_sensitivity, "Standard Output Sensitivity");
                        ui.checkbox(&mut self.display_config.photo_table_columns.show_sensitivity_type, "Sensitivity Type");
                        ui.checkbox(&mut self.display_config.photo_table_columns.show_aperture_value, "Aperture Value (EV)");
                        ui.checkbox(&mut self.display_config.photo_table_columns.show_max_aperture_value, "Max Aperture Value (EV)");
                        ui.checkbox(&mut self.display_config.photo_table_columns.show_focal_length_in_35mm_film, "Focal Length (35mm equiv)");
                        ui.checkbox(&mut self.display_config.photo_table_columns.show_focal_plane_x_resolution, "Focal Plane X Resolution");
                        ui.checkbox(&mut self.display_config.photo_table_columns.show_focal_plane_y_resolution, "Focal Plane Y Resolution");
                        ui.checkbox(&mut self.display_config.photo_table_columns.show_focal_plane_resolution_unit, "Focal Plane Resolution Unit");
                        ui.checkbox(&mut self.display_config.photo_table_columns.show_flash_energy, "Flash Energy");
                        ui.checkbox(&mut self.display_config.photo_table_columns.show_subject_distance, "Subject Distance");
                        ui.checkbox(&mut self.display_config.photo_table_columns.show_subject_distance_range, "Subject Distance Range");
                        ui.checkbox(&mut self.display_config.photo_table_columns.show_custom_rendered, "Custom Rendered");
                        ui.checkbox(&mut self.display_config.photo_table_columns.show_gamma, "Gamma");
                        ui.checkbox(&mut self.display_config.photo_table_columns.show_image_description, "Image Description");
                        ui.checkbox(&mut self.display_config.photo_table_columns.show_user_comment, "User Comment");
                        ui.checkbox(&mut self.display_config.photo_table_columns.show_maker_note, "Maker Note");
                        ui.checkbox(&mut self.display_config.photo_table_columns.show_image_unique_id, "Image Unique ID");
                        ui.checkbox(&mut self.display_config.photo_table_columns.show_lens_serial_number, "Lens Serial Number");
                        ui.checkbox(&mut self.display_config.photo_table_columns.show_date_time_original, "Date Time Original");
                        ui.checkbox(&mut self.display_config.photo_table_columns.show_date_time_digitized, "Date Time Digitized");
                        ui.checkbox(&mut self.display_config.photo_table_columns.show_sub_sec_time, "Sub-second Time");
                        ui.checkbox(&mut self.display_config.photo_table_columns.show_sub_sec_time_original, "Sub-second Time Original");
                        ui.checkbox(&mut self.display_config.photo_table_columns.show_sub_sec_time_digitized, "Sub-second Time Digitized");
                        ui.checkbox(&mut self.display_config.photo_table_columns.show_offset_time, "Offset Time");
                        ui.checkbox(&mut self.display_config.photo_table_columns.show_offset_time_original, "Offset Time Original");
                        ui.checkbox(&mut self.display_config.photo_table_columns.show_offset_time_digitized, "Offset Time Digitized");
                        ui.checkbox(&mut self.display_config.photo_table_columns.show_gps_latitude_ref, "GPS Latitude Ref");
                        ui.checkbox(&mut self.display_config.photo_table_columns.show_gps_longitude_ref, "GPS Longitude Ref");
                        ui.checkbox(&mut self.display_config.photo_table_columns.show_gps_altitude_ref, "GPS Altitude Ref");
                        ui.checkbox(&mut self.display_config.photo_table_columns.show_gps_timestamp, "GPS Timestamp");
                        ui.checkbox(&mut self.display_config.photo_table_columns.show_gps_satellites, "GPS Satellites");
                        ui.checkbox(&mut self.display_config.photo_table_columns.show_gps_status, "GPS Status");
                        ui.checkbox(&mut self.display_config.photo_table_columns.show_gps_measure_mode, "GPS Measure Mode");
                        ui.checkbox(&mut self.display_config.photo_table_columns.show_gps_dop, "GPS DOP");
                        ui.checkbox(&mut self.display_config.photo_table_columns.show_gps_speed, "GPS Speed");
                        ui.checkbox(&mut self.display_config.photo_table_columns.show_gps_speed_ref, "GPS Speed Ref");
                        ui.checkbox(&mut self.display_config.photo_table_columns.show_gps_track, "GPS Track");
                        ui.checkbox(&mut self.display_config.photo_table_columns.show_gps_track_ref, "GPS Track Ref");
                        ui.checkbox(&mut self.display_config.photo_table_columns.show_gps_img_direction, "GPS Img Direction");
                        ui.checkbox(&mut self.display_config.photo_table_columns.show_gps_img_direction_ref, "GPS Img Direction Ref");
                        ui.checkbox(&mut self.display_config.photo_table_columns.show_gps_map_datum, "GPS Map Datum");
                        ui.checkbox(&mut self.display_config.photo_table_columns.show_gps_date_stamp, "GPS Date Stamp");
                        ui.checkbox(&mut self.display_config.photo_table_columns.show_gps_processing_method, "GPS Processing Method");
                        ui.checkbox(&mut self.display_config.photo_table_columns.show_gps_area_information, "GPS Area Information");
                        ui.checkbox(&mut self.display_config.photo_table_columns.show_gps_differential, "GPS Differential");
                        ui.checkbox(&mut self.display_config.photo_table_columns.show_gps_h_positioning_error, "GPS H Positioning Error");
                        ui.checkbox(&mut self.display_config.photo_table_columns.show_resolution_unit, "Resolution Unit");
                        ui.checkbox(&mut self.display_config.photo_table_columns.show_photometric_interpretation, "Photometric Interpretation");
                        ui.checkbox(&mut self.display_config.photo_table_columns.show_compression, "Compression");
                        ui.checkbox(&mut self.display_config.photo_table_columns.show_bits_per_sample, "Bits Per Sample");
                        ui.checkbox(&mut self.display_config.photo_table_columns.show_samples_per_pixel, "Samples Per Pixel");
                        ui.checkbox(&mut self.display_config.photo_table_columns.show_planar_configuration, "Planar Configuration");
                        ui.checkbox(&mut self.display_config.photo_table_columns.show_ycbcr_sub_sampling, "YCbCr Sub Sampling");
                        ui.checkbox(&mut self.display_config.photo_table_columns.show_ycbcr_positioning, "YCbCr Positioning");
                        ui.checkbox(&mut self.display_config.photo_table_columns.show_reference_black_white, "Reference Black/White");
                        ui.checkbox(&mut self.display_config.photo_table_columns.show_white_point, "White Point");
                        ui.checkbox(&mut self.display_config.photo_table_columns.show_primary_chromaticities, "Primary Chromaticities");
                        ui.checkbox(&mut self.display_config.photo_table_columns.show_ycbcr_coefficients, "YCbCr Coefficients");
                        ui.checkbox(&mut self.display_config.photo_table_columns.show_water_depth, "Water Depth");
                        ui.checkbox(&mut self.display_config.photo_table_columns.show_acceleration, "Acceleration");
                        ui.checkbox(&mut self.display_config.photo_table_columns.show_camera_elevation_angle, "Camera Elevation Angle");
                        ui.checkbox(&mut self.display_config.photo_table_columns.show_components_configuration, "Components Configuration");
                        ui.checkbox(&mut self.display_config.photo_table_columns.show_compressed_bits_per_pixel, "Compressed Bits/Pixel");
                        ui.checkbox(&mut self.display_config.photo_table_columns.show_file_source, "File Source");
                        ui.checkbox(&mut self.display_config.photo_table_columns.show_scene_type, "Scene Type");
                        ui.checkbox(&mut self.display_config.photo_table_columns.show_cfa_pattern, "CFA Pattern");
                        ui.checkbox(&mut self.display_config.photo_table_columns.show_device_setting_description, "Device Setting Description");
                        ui.checkbox(&mut self.display_config.photo_table_columns.show_subject_area, "Subject Area");
                        ui.checkbox(&mut self.display_config.photo_table_columns.show_related_sound_file, "Related Sound File");
                        ui.checkbox(&mut self.display_config.photo_table_columns.show_spatial_frequency_response, "Spatial Frequency Response");
                        ui.checkbox(&mut self.display_config.photo_table_columns.show_spectral_sensitivity, "Spectral Sensitivity");
                        ui.checkbox(&mut self.display_config.photo_table_columns.show_oecf, "OECF");
                        ui.checkbox(&mut self.display_config.photo_table_columns.show_width, "Width");
                        ui.checkbox(&mut self.display_config.photo_table_columns.show_height, "Height");
                        ui.checkbox(&mut self.display_config.photo_table_columns.show_raw_orientation, "Raw Orientation");
                        ui.checkbox(&mut self.display_config.photo_table_columns.show_fuji_rotation_width, "Fuji Rotation Width");
                        ui.checkbox(&mut self.display_config.photo_table_columns.show_camera_mode, "Camera Mode");
                        ui.checkbox(&mut self.display_config.photo_table_columns.show_remark, "Remark");
                    });
                });

                ui.separator();
                if ui.button("Reset to Defaults").clicked() {
                    self.display_config = DisplayConfig::default();
                }
            });
    }

    fn sortable_header(
        ui: &mut egui::Ui,
        label: &str,
        column: SortColumn,
        sort_state: &mut Option<(SortColumn, SortOrder)>,
    ) {
        let arrow = if let Some((col, order)) = sort_state {
            if *col == column {
                match order {
                    SortOrder::Ascending => " ▲",
                    SortOrder::Descending => " ▼",
                }
            } else {
                ""
            }
        } else {
            ""
        };
        
        let button_text = format!("{}{}", label, arrow);
        let response = ui.add(egui::Button::new(button_text).frame(false));
        if response.clicked() {
            *sort_state = match sort_state {
                Some((col, SortOrder::Ascending)) if *col == column => Some((column, SortOrder::Descending)),
                Some((_, _)) => Some((column, SortOrder::Ascending)),
                None => Some((column, SortOrder::Ascending)),
            };
        }
    }

    fn dropdown_filter(
        ui: &mut egui::Ui,
        label: &str,
        options: &[String],
        selected: &mut std::collections::HashSet<String>,
    ) {
        let mut sorted_options = options.to_vec();
        // Natural sort: extract leading numbers for numeric comparison
        sorted_options.sort_by(|a, b| {
            // Extract numeric prefix for comparison
            let a_num = Self::extract_leading_number(a);
            let b_num = Self::extract_leading_number(b);
            match (a_num, b_num) {
                (Some(an), Some(bn)) => an.partial_cmp(&bn).unwrap_or(a.cmp(b)),
                (Some(_), None) => std::cmp::Ordering::Less,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                _ => a.cmp(b),
            }
        });
        let count = selected.len();
        egui::ComboBox::new(egui::Id::new(format!("filter_{}", label)), "filter")
            .selected_text(if count == 0 { "All".to_string() } else { format!("{} selected", count) })
            .show_ui(ui, |ui| {
                // "All" option
                let all_selected = selected.is_empty();
                if ui.selectable_label(all_selected, "All").clicked() {
                    selected.clear();
                }
                ui.separator();
                // Checkbox options
                let mut toggles = Vec::new();
                for opt in &sorted_options {
                    let is_selected = selected.contains(opt);
                    if ui.selectable_label(is_selected, opt).clicked() {
                        toggles.push(opt.clone());
                    }
                }
                for opt in toggles {
                    if selected.contains(&opt) {
                        selected.remove(&opt);
                    } else {
                        selected.insert(opt);
                    }
                }
            });
    }

    fn extract_leading_number(s: &str) -> Option<f64> {
        // Extract leading number from string (handles "13", "100", "f/2.8", "f/11")
        let num_str: String = s.chars()
            .take_while(|c| c.is_ascii_digit() || *c == '.')
            .collect();
        num_str.parse().ok()
    }

    fn get_filtered_photos<'a>(
        photos: &'a [PhotoMeta],
        camera_filter: &'a std::collections::HashSet<String>,
        lens_filter: &'a std::collections::HashSet<String>,
        aperture_filter: &'a std::collections::HashSet<String>,
        focal_filter: &'a std::collections::HashSet<String>,
        type_filter: &'a std::collections::HashSet<String>,
    ) -> Vec<&'a PhotoMeta> {
        let mut filtered: Vec<_> = photos.iter().collect();
        filtered.retain(|p| {
            let cam_match = camera_filter.is_empty() ||
                p.camera_model.as_deref().map_or(false, |c| camera_filter.contains(c));
            let lens_match = lens_filter.is_empty() ||
                p.lens_model.as_deref().map_or(false, |l| lens_filter.contains(l));
            let aperture_match = aperture_filter.is_empty() ||
                p.aperture.map_or(false, |v| aperture_filter.contains(&format!("{:.1}", v)));
            let focal_match = focal_filter.is_empty() ||
                p.focal_length.map_or(false, |v| focal_filter.contains(&format!("{:.0}", v)));
            let type_match = type_filter.is_empty() ||
                type_filter.contains(&format!("{:?}", p.photo_type));
            let date_match = true;
            cam_match && lens_match && aperture_match && focal_match && type_match && date_match
        });
        filtered
    }

    fn compute_iso_counts(&self, photos: &[&PhotoMeta]) -> std::collections::BTreeMap<u32, usize> {
        let mut counts = std::collections::BTreeMap::new();
        for p in photos {
            if let Some(iso) = p.iso {
                *counts.entry(iso).or_default() += 1;
            }
        }
        counts
    }

    fn compute_aperture_points(&self, photos: &[&PhotoMeta]) -> PlotPoints {
        let mut points: Vec<[f64; 2]> = Vec::new();

        match self.display_config.aperture_scale {
            ApertureScale::Linear => {
                let mut counts: std::collections::BTreeMap<i32, usize> = std::collections::BTreeMap::new();
                for p in photos {
                    if let Some(ap) = p.aperture {
                        let key = (ap * 10.0).round() as i32;
                        *counts.entry(key).or_default() += 1;
                    }
                }
                for (k, v) in counts {
                    points.push([k as f64 / 10.0, v as f64]);
                }
            }
            ApertureScale::Logarithmic => {
                let mut counts: std::collections::BTreeMap<i32, usize> = std::collections::BTreeMap::new();
                for p in photos {
                    if let Some(ap) = p.aperture {
                        let key = (ap * 10.0).round() as i32;
                        *counts.entry(key).or_default() += 1;
                    }
                }
                for (k, v) in counts {
                    let f = k as f64 / 10.0;
                    if f > 0.0 {
                        points.push([f.log10(), v as f64]);
                    }
                }
            }
            ApertureScale::FStops => {
                let f_stops = [1.0, 1.4, 2.0, 2.8, 4.0, 5.6, 8.0, 11.0, 16.0, 22.0, 32.0];
                let mut fstop_counts: std::collections::BTreeMap<usize, usize> = std::collections::BTreeMap::new();

                for p in photos {
                    if let Some(ap) = p.aperture {
                        let f = ap;
                        let closest_idx = f_stops.iter()
                            .enumerate()
                            .min_by(|(_, a), (_, b)| {
                                (*a - f).abs().partial_cmp(&(*b - f).abs()).unwrap()
                            })
                            .map(|(i, _)| i)
                            .unwrap_or(0);
                        *fstop_counts.entry(closest_idx).or_default() += 1;
                    }
                }

                for (idx, count) in fstop_counts {
                    points.push([f_stops[idx] as f64, count as f64]);
                }
            }
        }

        points.sort_by(|a, b| a[0].partial_cmp(&b[0]).unwrap());
        PlotPoints::from(points)
    }

    fn compute_focal_counts(&self, photos: &[&PhotoMeta]) -> std::collections::BTreeMap<i32, usize> {
        let mut counts = std::collections::BTreeMap::new();
        for p in photos {
            if let Some(fl) = p.focal_length {
                *counts.entry(fl.round() as i32).or_default() += 1;
            }
        }
        counts
    }
}

impl eframe::App for CameraStatsApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if let Some(rx) = &self.scan_rx {
            if let Ok(photos) = rx.try_recv() {
                let duration = self.scan_start.map(|s| s.elapsed().as_millis()).unwrap_or(0);
                if let Some(dir) = &self.selected_dir {
                    log_scan_complete(dir, photos.len(), duration);
                }
                self.photos = photos;
                self.stats = Stats::from_photos(&self.photos);
                self.status = format!("Found {} photos", self.photos.len());
                self.scanning = false;
                self.scan_rx = None;
                self.scan_start = None;
            } else {
                ctx.request_repaint_after(std::time::Duration::from_millis(100));
            }
        }

        egui::TopBottomPanel::top("toolbar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                if ui.button("📁 Select Folder").clicked() && !self.scanning {
                    if let Some(dir) = FileDialog::new().pick_folder() {
                        self.selected_dir = Some(dir.clone());
                        self.status = format!("Scanning {}...", dir.display());
                        self.scanning = true;
                        self.scan_start = Some(Instant::now());
                        let (tx, rx) = mpsc::channel();
                        self.scan_rx = Some(rx);
                        thread::spawn(move || scan_directory(dir, tx));
                        log_ui_event("Folder scan started");
                    }
                }

                ui.separator();

                if let Some(dir) = &self.selected_dir {
                    ui.label(format!("📂 {}", dir.display()));
                }

                ui.separator();

                if ui.button("📤 Export CSV").clicked() && !self.photos.is_empty() {
                    self.show_export_dialog = true;
                }

                ui.separator();

                if ui.button("⚙️ Settings").clicked() {
                    self.show_settings = true;
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(&self.status);
                });
            });
        });

        if self.show_settings {
            self.render_settings(ctx);
        }

        if self.show_export_dialog {
            if let Some(path) = FileDialog::new()
                .add_filter("CSV", &["csv"])
                .set_file_name(format!("camera_stats_{}.csv", Local::now().format("%Y%m%d_%H%M%S")))
                .save_file()
            {
                match export_csv(&self.photos, &path) {
                    Ok(_) => {
                        self.status = format!("Exported to {}", path.display());
                        log_ui_event(&format!("Exported CSV to {:?}", path));
                    }
                    Err(e) => {
                        self.status = format!("Export failed: {}", e);
                        error!("CSV export failed: {}", e);
                    }
                }
            }
            self.show_export_dialog = false;
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            if self.photos.is_empty() {
                ui.centered_and_justified(|ui| {
                    ui.heading("📸 Camera Stats");
                    ui.add_space(10.0);
                    ui.label("Select a folder containing photos to analyze EXIF data");
                });
                return;
            }

            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.heading("📊 Statistics");
                ui.add_space(10.0);

                if self.display_config.show_iso_chart && !self.stats.iso_counts.is_empty() {
                    ui.group(|ui| {
                        ui.label("ISO Distribution");
                        let filtered_photos = Self::get_filtered_photos(
                            &self.photos,
                            &self.table_state.photo_camera_filter,
                            &self.table_state.photo_lens_filter,
                            &self.table_state.photo_aperture_filter,
                            &self.table_state.photo_focal_filter,
                            &self.table_state.photo_type_filter,
                        );
                        let iso_counts = self.compute_iso_counts(&filtered_photos);
                        if !iso_counts.is_empty() {
                            let points: PlotPoints = iso_counts.iter()
                                .map(|(k, v)| [*k as f64, *v as f64])
                                .collect();
                            self.render_chart(ui, "iso_plot", "ISO", points);
                        }
                    });
                }

                if self.display_config.show_aperture_chart && !self.stats.aperture_counts.is_empty() {
                    ui.group(|ui| {
                        let label = match self.display_config.aperture_scale {
                            ApertureScale::Linear => "Aperture (f-stop)",
                            ApertureScale::Logarithmic => "Aperture (log scale)",
                            ApertureScale::FStops => "Aperture (standard f-stops)",
                        };
                        ui.label(label);
                        let filtered_photos = Self::get_filtered_photos(
                            &self.photos,
                            &self.table_state.photo_camera_filter,
                            &self.table_state.photo_lens_filter,
                            &self.table_state.photo_aperture_filter,
                            &self.table_state.photo_focal_filter,
                            &self.table_state.photo_type_filter,
                        );
                        let points = self.compute_aperture_points(&filtered_photos);
                        if !points.points().is_empty() {
                            self.render_chart(ui, "aperture_plot", label, points);
                        }
                    });
                }

                if self.display_config.show_focal_chart && !self.stats.focal_length_counts.is_empty() {
                    ui.group(|ui| {
                        ui.label("Focal Length Distribution (mm)");
                        let filtered_photos = Self::get_filtered_photos(
                            &self.photos,
                            &self.table_state.photo_camera_filter,
                            &self.table_state.photo_lens_filter,
                            &self.table_state.photo_aperture_filter,
                            &self.table_state.photo_focal_filter,
                            &self.table_state.photo_type_filter,
                        );
                        let focal_counts = self.compute_focal_counts(&filtered_photos);
                        if !focal_counts.is_empty() {
                            let points: PlotPoints = focal_counts.iter()
                                .map(|(k, v)| [*k as f64, *v as f64])
                                .collect();
                            self.render_chart(ui, "focal_plot", "Focal Length", points);
                        }
                    });
                }
if self.display_config.show_photo_table {
                    // Clone filter sets to avoid borrow conflicts with the closure
                    let camera_filter = self.table_state.photo_camera_filter.clone();
                    let lens_filter = self.table_state.photo_lens_filter.clone();
                    let aperture_filter = self.table_state.photo_aperture_filter.clone();
                    let focal_filter = self.table_state.photo_focal_filter.clone();
                    let type_filter = self.table_state.photo_type_filter.clone();

                    // Compute filtered photos and unique values before UI to avoid borrow conflicts
                    let mut photos: Vec<_> = Self::get_filtered_photos(
                        &self.photos,
                        &camera_filter,
                        &lens_filter,
                        &aperture_filter,
                        &focal_filter,
                        &type_filter,
                    );
                    if let Some((col, order)) = self.table_state.photo_sort {
                        photos.sort_by(|a, b| {
                            let cmp = match col {
                                SortColumn::Name => a.path.file_name().unwrap().to_string_lossy()
                                    .cmp(&b.path.file_name().unwrap().to_string_lossy()),
                                SortColumn::Aperture => a.aperture.partial_cmp(&b.aperture).unwrap_or(std::cmp::Ordering::Equal),
                                SortColumn::FocalLength => a.focal_length.partial_cmp(&b.focal_length).unwrap_or(std::cmp::Ordering::Equal),
                                SortColumn::Camera => a.camera_model.as_deref().unwrap_or("").cmp(b.camera_model.as_deref().unwrap_or("")),
                                SortColumn::Lens => a.lens_model.as_deref().unwrap_or("").cmp(b.lens_model.as_deref().unwrap_or("")),
                                _ => std::cmp::Ordering::Equal,
                            };
                            match order {
                                SortOrder::Ascending => cmp,
                                SortOrder::Descending => cmp.reverse(),
                            }
                        });
                    }
                    // Collect unique values for dropdown filters
                    let cameras: Vec<String> = self.photos.iter()
                        .filter_map(|p| p.camera_model.clone())
                        .collect::<std::collections::HashSet<_>>()
                        .into_iter()
                        .collect();
                    let lenses: Vec<String> = self.photos.iter()
                        .filter_map(|p| p.lens_model.clone())
                        .collect::<std::collections::HashSet<_>>()
                        .into_iter()
                        .collect();
                    let apertures: Vec<String> = self.photos.iter()
                        .filter_map(|p| p.aperture.map(|v| format!("{:.1}", v)))
                        .collect::<std::collections::HashSet<_>>()
                        .into_iter()
                        .collect();
                    let focals: Vec<String> = self.photos.iter()
                        .filter_map(|p| p.focal_length.map(|v| format!("{:.0}", v)))
                        .collect::<std::collections::HashSet<_>>()
                        .into_iter()
                        .collect();

                    let filtered_count = photos.len();
                    let total_count = self.photos.len();
                    ui.group(|ui| {
                        ui.label(format!("All Photos ({} of {})", filtered_count, total_count));
                        egui::ScrollArea::horizontal().max_width(ui.available_width()).show(ui, |ui| {
                            egui::Grid::new("photo_grid").striped(true).min_col_width(100.0).show(ui, |ui| {
                                Self::sortable_header(ui, "File", SortColumn::Name, &mut self.table_state.photo_sort);
                                Self::sortable_header(ui, "Date", SortColumn::Name, &mut self.table_state.photo_sort);
                                Self::sortable_header(ui, "Aperture", SortColumn::Aperture, &mut self.table_state.photo_sort);
                                Self::sortable_header(ui, "Focal (mm)", SortColumn::FocalLength, &mut self.table_state.photo_sort);
                                Self::sortable_header(ui, "Camera", SortColumn::Camera, &mut self.table_state.photo_sort);
                                Self::sortable_header(ui, "Lens", SortColumn::Lens, &mut self.table_state.photo_sort);
                                Self::sortable_header(ui, "Type", SortColumn::Name, &mut self.table_state.photo_sort);
                                ui.end_row();

                                // Filter row with dropdowns
                                ui.label(""); // File column - no filter
                                ui.label(""); // Date column
                                Self::dropdown_filter(ui, "Aperture", &apertures, &mut self.table_state.photo_aperture_filter);
                                Self::dropdown_filter(ui, "Focal", &focals, &mut self.table_state.photo_focal_filter);
                                Self::dropdown_filter(ui, "Camera", &cameras, &mut self.table_state.photo_camera_filter);
                                Self::dropdown_filter(ui, "Lens", &lenses, &mut self.table_state.photo_lens_filter);
                                let types: Vec<String> = vec!["Jpeg".to_string(), "Raw".to_string()];
                                Self::dropdown_filter(ui, "Type", &types, &mut self.table_state.photo_type_filter);
                                ui.end_row();

                                // Date range picker row removed
                                ui.end_row();

                                for p in photos {
                                    ui.label(p.path.file_name().unwrap().to_string_lossy().to_string());
                                    ui.label(p.date_time.map_or("—".into(), |dt| dt.format("%Y-%m-%d").to_string()));
                                    ui.label(p.aperture.map_or("—".into(), |v| format!("f/{:.1}", v)));
                                    ui.label(p.focal_length.map_or("—".into(), |v| format!("{:.0}", v)));
                                    ui.label(p.camera_model.as_deref().unwrap_or("—"));
                                    ui.label(p.lens_model.as_deref().unwrap_or("—"));
                                    ui.label(format!("{:?}", p.photo_type));
                                    ui.end_row();
                                }
                            });  // photo grid
                        });  // photo scroll horizontal
                    });  // ui.group for photo table
                }
            });  // scroll vertical
        });  // central panel
    }  // update fn
}  // impl

fn main() -> Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1000.0, 800.0])
            .with_min_inner_size([800.0, 600.0]),
        ..Default::default()
    };

    eframe::run_native(
        "Camera Stats",
        options,
        Box::new(|_cc| Ok(Box::new(CameraStatsApp::default()))),
    ).map_err(|e| anyhow::anyhow!("{:?}", e))
}
