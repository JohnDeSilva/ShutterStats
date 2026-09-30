pub mod logging;

use std::path::PathBuf;
use rayon::prelude::*;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PhotoMeta {
    pub path: PathBuf,
    pub iso: Option<u32>,
    pub aperture: Option<f32>,
    pub focal_length: Option<f32>,
    pub camera_model: Option<String>,
    pub lens_model: Option<String>,
    pub date_time: Option<chrono::NaiveDateTime>,
    pub width: u32,
    pub height: u32,
    pub photo_type: String,
}

#[derive(Debug, Default)]
pub struct Stats {
    pub iso_counts: std::collections::BTreeMap<u32, usize>,
    pub aperture_counts: std::collections::BTreeMap<i32, usize>,
    pub focal_length_counts: std::collections::BTreeMap<i32, usize>,
    pub camera_counts: std::collections::BTreeMap<String, usize>,
    pub lens_counts: std::collections::BTreeMap<String, usize>,
}

impl Stats {
    pub fn from_photos(photos: &[PhotoMeta]) -> Self {
        let mut s = Self::default();
        for p in photos {
            if let Some(iso) = p.iso {
                *s.iso_counts.entry(iso).or_default() += 1;
            }
            if let Some(ap) = p.aperture {
                let key = (ap * 10.0).round() as i32;
                *s.aperture_counts.entry(key).or_default() += 1;
            }
            if let Some(fl) = p.focal_length {
                *s.focal_length_counts.entry(fl.round() as i32).or_default() += 1;
            }
            if let Some(cam) = &p.camera_model {
                *s.camera_counts.entry(cam.clone()).or_default() += 1;
            }
            if let Some(lens) = &p.lens_model {
                *s.lens_counts.entry(lens.clone()).or_default() += 1;
            }
        }
        s
    }
}

pub fn extract_exif(path: &PathBuf) -> anyhow::Result<PhotoMeta> {
    let file = std::fs::File::open(path)?;
    let mut buf = std::io::BufReader::new(file);
    let exif = exif::Reader::new().read_from_container(&mut buf)?;

    let get_tag = |tag: exif::Tag| -> Option<String> {
        exif.get_field(tag, exif::In::PRIMARY)
            .and_then(|f| f.display_value().to_string().into())
    };

    let get_u32 = |tag: exif::Tag| -> Option<u32> {
        exif.get_field(tag, exif::In::PRIMARY)
            .and_then(|f| f.value.get_uint(0))
    };

    let get_f32 = |tag: exif::Tag| -> Option<f32> {
        exif.get_field(tag, exif::In::PRIMARY)
            .and_then(|f| {
                let s = f.display_value().to_string();
                if let Some((num, den)) = s.split_once('/') {
                    num.parse::<f32>().ok().and_then(|n| den.parse::<f32>().ok().map(|d| n / d))
                } else {
                    s.parse::<f32>().ok()
                }
            })
    };

    let get_datetime = |tag: exif::Tag| -> Option<chrono::NaiveDateTime> {
        exif.get_field(tag, exif::In::PRIMARY)
            .and_then(|f| {
                let s = f.display_value().to_string();
                // EXIF format: "YYYY:MM:DD HH:MM:SS" - convert to "YYYY-MM-DD HH:MM:SS"
                let parts: Vec<&str> = s.splitn(2, ' ').collect();
                if parts.len() == 2 {
                    let date_part = parts[0].replace(':', "-");
                    let datetime_str = format!("{} {}", date_part, parts[1]);
                    datetime_str.parse::<chrono::NaiveDateTime>().ok()
                } else {
                    // Try parsing as just date
                    parts[0].replace(':', "-").parse::<chrono::NaiveDateTime>().ok()
                }
            })
    };

    let (width, height) = {
        let img = image::ImageReader::open(path)?.with_guessed_format()?.decode()?;
        (img.width(), img.height())
    };

    // Fallback to file modification time if EXIF dates not found
    let file_datetime = std::fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| chrono::NaiveDateTime::from_timestamp(d.as_secs() as i64, 0));

    Ok(PhotoMeta {
        path: path.clone(),
        iso: get_u32(exif::Tag::ISOSpeed),
        aperture: get_f32(exif::Tag::FNumber),
        focal_length: get_f32(exif::Tag::FocalLength),
        camera_model: get_tag(exif::Tag::Model),
        lens_model: get_tag(exif::Tag::LensModel),
        date_time: get_datetime(exif::Tag::DateTimeOriginal)
            .or_else(|| get_datetime(exif::Tag::DateTimeDigitized))
            .or_else(|| get_datetime(exif::Tag::DateTime))
            .or(file_datetime),
        width,
        height,
        photo_type: "Jpeg".to_string(),
    })
}

pub fn extract_raw_exif(path: &PathBuf) -> anyhow::Result<PhotoMeta> {
    let raw = rawler::decode_file(path)?;
    
    // rawler provides some metadata directly on RawImage
    let camera_model = Some(raw.camera.make.clone()).or_else(|| Some(raw.camera.model.clone()));
    let lens_model = None; // rawler doesn't provide lens info
    
    // Try to get EXIF from the raw file using a different approach
    let (iso, aperture, focal_length) = (None, None, None);

    let (width, height) = {
        let img = image::ImageReader::open(path)?.with_guessed_format()?.decode()?;
        (img.width(), img.height())
    };

    // Fallback to file modification time
    let file_datetime = std::fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| chrono::NaiveDateTime::from_timestamp(d.as_secs() as i64, 0));

    Ok(PhotoMeta {
        path: path.clone(),
        iso,
        aperture,
        focal_length,
        camera_model,
        lens_model,
        date_time: file_datetime,
        width,
        height,
        photo_type: "Raw".to_string(),
    })
}

pub fn export_csv(photos: &[PhotoMeta], path: &PathBuf) -> anyhow::Result<()> {
    let mut wtr = csv::Writer::from_path(path)?;
    wtr.write_record(&["File", "Date", "Aperture", "Focal Length (mm)", "Camera", "Lens", "Width", "Height"])?;
    for p in photos {
        wtr.write_record(&[
            p.path.file_name().unwrap().to_string_lossy().as_ref(),
            &p.date_time.map_or("".to_string(), |dt| dt.format("%Y-%m-%d").to_string()),
            &p.aperture.map_or("".to_string(), |v| format!("{:.1}", v)),
            &p.focal_length.map_or("".to_string(), |v| format!("{:.0}", v)),
            p.camera_model.as_deref().unwrap_or(""),
            p.lens_model.as_deref().unwrap_or(""),
            &p.width.to_string(),
            &p.height.to_string(),
        ])?;
    }
    wtr.flush()?;
    Ok(())
}

pub fn scan_directory(dir: PathBuf, tx: std::sync::mpsc::Sender<Vec<PhotoMeta>>) {
    let extensions = ["jpg", "jpeg", "JPG", "JPEG", "cr2", "cr3", "CR2", "CR3",
                      "nef", "NEF", "arw", "ARW", "raf", "RAF", "rw2", "RW2",
                      "orf", "ORF", "pef", "PEF", "srw", "SRW", "dng", "DNG"];

    let files: Vec<_> = walkdir::WalkDir::new(&dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter(|e| {
            e.path().extension()
                .and_then(|s| s.to_str())
                .map(|ext| extensions.contains(&ext))
                .unwrap_or(false)
        })
        .map(|e| e.path().to_path_buf())
        .collect();

    crate::logging::log_scan_start(&dir, files.len());

    let photos: Vec<PhotoMeta> = files
        .par_iter()
        .filter_map(|path| {
            let ext = path.extension()?.to_str()?.to_lowercase();
            let is_raw = matches!(ext.as_str(),
                "cr2" | "cr3" | "nef" | "arw" | "raf" | "rw2" | "orf" | "pef" | "srw" | "dng"
            );

            let result = if is_raw {
                extract_raw_exif(path)
            } else {
                extract_exif(path)
            };

            match &result {
                Ok(_) => crate::logging::log_exif_extract(path, true, None),
                Err(e) => crate::logging::log_exif_extract(path, false, Some(e)),
            }

            result.ok()
        })
        .collect();

    let _ = tx.send(photos);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn create_test_jpeg(dir: &PathBuf) -> PathBuf {
        let path = dir.join("test.jpg");
        let img = image::ImageBuffer::from_fn(100, 100, |_, _| image::Rgb([128u8, 128u8, 128u8]));
        img.save_with_format(&path, image::ImageFormat::Jpeg).unwrap();
        path
    }

    #[test]
    fn test_photo_meta_creation() {
        let meta = PhotoMeta {
            path: PathBuf::from("test.jpg"),
            iso: Some(400),
            aperture: Some(2.8),
            focal_length: Some(50.0),
            camera_model: Some("Test Camera".into()),
            lens_model: Some("Test Lens".into()),
            date_time: None,
            width: 100,
            height: 100,
            photo_type: "Jpeg".to_string(),
        };
        assert_eq!(meta.iso, Some(400));
        assert_eq!(meta.aperture, Some(2.8));
        assert_eq!(meta.focal_length, Some(50.0));
    }

    #[test]
    fn test_stats_from_photos() {
        let photos = vec![
            PhotoMeta {
                path: PathBuf::from("a.jpg"),
                iso: Some(100),
                aperture: Some(2.8),
                focal_length: Some(35.0),
                camera_model: Some("Cam A".into()),
                lens_model: Some("Lens X".into()),
                date_time: None,
                width: 100,
                height: 100,
                photo_type: "Jpeg".to_string(),
            },
            PhotoMeta {
                path: PathBuf::from("b.jpg"),
                iso: Some(400),
                aperture: Some(2.8),
                focal_length: Some(50.0),
                camera_model: Some("Cam A".into()),
                lens_model: Some("Lens Y".into()),
                date_time: None,
                width: 100,
                height: 100,
                photo_type: "Jpeg".to_string(),
            },
            PhotoMeta {
                path: PathBuf::from("c.jpg"),
                iso: Some(100),
                aperture: Some(4.0),
                focal_length: Some(35.0),
                camera_model: Some("Cam B".into()),
                lens_model: Some("Lens X".into()),
                date_time: None,
                width: 100,
                height: 100,
                photo_type: "Jpeg".to_string(),
            },
        ];

        let stats = Stats::from_photos(&photos);

        assert_eq!(stats.iso_counts.get(&100), Some(&2));
        assert_eq!(stats.iso_counts.get(&400), Some(&1));
        assert_eq!(stats.aperture_counts.get(&28), Some(&2));
        assert_eq!(stats.aperture_counts.get(&40), Some(&1));
        assert_eq!(stats.focal_length_counts.get(&35), Some(&2));
        assert_eq!(stats.focal_length_counts.get(&50), Some(&1));
        assert_eq!(stats.camera_counts.get("Cam A"), Some(&2));
        assert_eq!(stats.camera_counts.get("Cam B"), Some(&1));
        assert_eq!(stats.lens_counts.get("Lens X"), Some(&2));
        assert_eq!(stats.lens_counts.get("Lens Y"), Some(&1));
    }

    #[test]
    fn test_stats_empty() {
        let stats = Stats::from_photos(&[]);
        assert!(stats.iso_counts.is_empty());
        assert!(stats.aperture_counts.is_empty());
        assert!(stats.focal_length_counts.is_empty());
        assert!(stats.camera_counts.is_empty());
        assert!(stats.lens_counts.is_empty());
    }

    #[test]
    fn test_scan_directory_finds_files() {
        let temp_dir = TempDir::new().unwrap();
        let dir = temp_dir.path().to_path_buf();

        create_test_jpeg(&dir);
        fs::write(dir.join("test.txt"), "not an image").unwrap();
        fs::write(dir.join("test.CR2"), "fake raw").unwrap();

        let extensions = ["jpg", "jpeg", "JPG", "JPEG", "cr2", "cr3", "CR2", "CR3"];
        let files: Vec<_> = walkdir::WalkDir::new(&dir)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
            .filter(|e| {
                e.path().extension()
                    .and_then(|s| s.to_str())
                    .map(|ext| extensions.contains(&ext))
                    .unwrap_or(false)
            })
            .map(|e| e.path().to_path_buf())
            .collect();

        assert_eq!(files.len(), 2);
        assert!(files.iter().any(|f| f.extension().unwrap() == "jpg"));
        assert!(files.iter().any(|f| f.extension().unwrap() == "CR2"));
    }

    #[test]
    fn test_aperture_rounding() {
        let photos = vec![
            PhotoMeta {
                path: PathBuf::from("a.jpg"),
                iso: Some(100),
                aperture: Some(2.8),
                focal_length: Some(35.0),
                camera_model: None,
                lens_model: None,
                date_time: None,
                width: 100,
                height: 100,
                photo_type: "Jpeg".to_string(),
            },
            PhotoMeta {
                path: PathBuf::from("b.jpg"),
                iso: Some(100),
                aperture: Some(2.82),
                focal_length: Some(35.0),
                camera_model: None,
                lens_model: None,
                date_time: None,
                width: 100,
                height: 100,
                photo_type: "Jpeg".to_string(),
            },
        ];

        let stats = Stats::from_photos(&photos);
        assert_eq!(stats.aperture_counts.get(&28), Some(&2));
    }

    #[test]
    fn test_focal_length_rounding() {
        let photos = vec![
            PhotoMeta {
                path: PathBuf::from("a.jpg"),
                iso: Some(100),
                aperture: Some(2.8),
                focal_length: Some(35.2),
                camera_model: None,
                lens_model: None,
                date_time: None,
                width: 100,
                height: 100,
                photo_type: "Jpeg".to_string(),
            },
            PhotoMeta {
                path: PathBuf::from("b.jpg"),
                iso: Some(100),
                aperture: Some(2.8),
                focal_length: Some(35.7),
                camera_model: None,
                lens_model: None,
                date_time: None,
                width: 100,
                height: 100,
                photo_type: "Jpeg".to_string(),
            },
        ];

        let stats = Stats::from_photos(&photos);
        assert_eq!(stats.focal_length_counts.get(&35), Some(&1));
        assert_eq!(stats.focal_length_counts.get(&36), Some(&1));
    }

    #[test]
    fn test_missing_exif_handled() {
        let meta = PhotoMeta {
            path: PathBuf::from("test.jpg"),
            iso: None,
            aperture: None,
            focal_length: None,
            camera_model: None,
            lens_model: None,
            date_time: None,
            width: 100,
            height: 100,
            photo_type: "Jpeg".to_string(),
        };
        let stats = Stats::from_photos(&[meta]);
        assert!(stats.iso_counts.is_empty());
        assert!(stats.aperture_counts.is_empty());
        assert!(stats.focal_length_counts.is_empty());
    }

    #[test]
    fn test_export_csv() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("test.csv");

        let photos = vec![
            PhotoMeta {
                path: PathBuf::from("a.jpg"),
                iso: Some(100),
                aperture: Some(2.8),
                focal_length: Some(35.0),
                camera_model: Some("Cam A".into()),
                lens_model: Some("Lens X".into()),
                date_time: None,
                width: 4000,
                height: 3000,
                photo_type: "Jpeg".to_string(),
            },
        ];

        export_csv(&photos, &path).unwrap();
        let content = fs::read_to_string(&path).unwrap();
        assert!(content.contains("a.jpg"));
        assert!(content.contains("2.8"));
        assert!(content.contains("35"));
        assert!(content.contains("Cam A"));
        assert!(content.contains("Lens X"));
    }
}