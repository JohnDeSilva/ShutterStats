pub mod logging;

use chrono::TimeZone;
use rayon::prelude::*;
use std::path::{Path, PathBuf};
use tracing::{debug, warn};

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

/// Tag accessors shared by the JPEG and RAW paths.
///
/// Both containers are TIFF-based (JPEG stores an EXIF block, RAW files are TIFF
/// themselves), so one parser serves both and keeps values consistent.
struct ExifTags<'a>(&'a exif::Exif);

impl<'a> ExifTags<'a> {
    /// Read an ASCII tag, stripping the quoting that `display_value` adds.
    fn text(&self, tag: exif::Tag) -> Option<String> {
        let exif::Value::Ascii(chunks) = &self.0.get_field(tag, exif::In::PRIMARY)?.value else {
            return None;
        };
        let joined: Vec<u8> = chunks.iter().flatten().copied().collect();
        String::from_utf8(joined)
            .ok()
            .map(|s| s.trim().trim_matches('"').trim().to_string())
            .filter(|s| !s.is_empty())
    }

    fn uint(&self, tag: exif::Tag) -> Option<u32> {
        self.0
            .get_field(tag, exif::In::PRIMARY)
            .and_then(|f| f.value.get_uint(0))
    }

    /// Rationals are stored as `numerator/denominator`; plain decimals also appear.
    fn float(&self, tag: exif::Tag) -> Option<f32> {
        self.0.get_field(tag, exif::In::PRIMARY).and_then(|f| {
            let s = f.display_value().to_string();
            if let Some((num, den)) = s.split_once('/') {
                num.parse::<f32>()
                    .ok()
                    .and_then(|n| den.parse::<f32>().ok().map(|d| n / d))
            } else {
                s.parse::<f32>().ok()
            }
        })
    }

    fn datetime(&self, tag: exif::Tag) -> Option<chrono::NaiveDateTime> {
        self.0.get_field(tag, exif::In::PRIMARY).and_then(|f| {
            // EXIF writes "YYYY:MM:DD HH:MM:SS"; chrono expects "YYYY-MM-DD HH:MM:SS".
            let s = f.display_value().to_string();
            let (date, time) = match s.split_once(' ') {
                Some((d, t)) => (d, t),
                None => (s.as_str(), ""),
            };
            let date = date.replace(':', "-");
            let candidate = if time.is_empty() {
                format!("{date} 00:00:00")
            } else {
                format!("{date} {time}")
            };
            chrono::NaiveDateTime::parse_from_str(&candidate, "%Y-%m-%d %H:%M:%S")
                .or_else(|_| {
                    chrono::NaiveDateTime::parse_from_str(&candidate, "%Y-%m-%d %H:%M:%S%.f")
                })
                .ok()
        })
    }

    /// Cameras write ISO under several tags depending on model and age.
    fn iso(&self) -> Option<u32> {
        self.uint(exif::Tag::ISOSpeed)
            .or_else(|| self.uint(exif::Tag::PhotographicSensitivity))
            .or_else(|| self.uint(exif::Tag::RecommendedExposureIndex))
    }

    fn camera_model(&self) -> Option<String> {
        let model = self.text(exif::Tag::Model);
        match (self.text(exif::Tag::Make), model) {
            (Some(make), Some(model)) => Some(format!("{} {model}", normalize_make(&make))),
            (None, Some(model)) => Some(model),
            (Some(make), None) => Some(normalize_make(&make)),
            (None, None) => None,
        }
    }

    fn lens_model(&self) -> Option<String> {
        self.text(exif::Tag::LensModel)
            .or_else(|| self.text(exif::Tag::LensSpecification))
    }

    fn date_time(&self) -> Option<chrono::NaiveDateTime> {
        self.datetime(exif::Tag::DateTimeOriginal)
            .or_else(|| self.datetime(exif::Tag::DateTimeDigitized))
            .or_else(|| self.datetime(exif::Tag::DateTime))
    }

    /// Dimensions, used when no pixel decoder is available.
    fn dimensions(&self) -> (u32, u32) {
        let pick = |a: exif::Tag, b: exif::Tag| -> u32 {
            self.uint(a).or_else(|| self.uint(b)).unwrap_or(0)
        };
        (
            pick(exif::Tag::ImageWidth, exif::Tag::PixelXDimension),
            pick(exif::Tag::ImageLength, exif::Tag::PixelYDimension),
        )
    }
}

/// Normalize a camera make so JPEG and RAW spellings group together.
///
/// EXIF records the make as written by the camera ("SONY", "NIKON CORPORATION"),
/// while `rawler` reports a cleaned-up form ("Sony"). Folding to title case
/// keeps a single row per camera in the summary tables.
fn normalize_make(make: &str) -> String {
    make.split_whitespace()
        .map(|word| {
            let lower = word.to_lowercase();
            let mut chars = lower.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Open `path` and parse its EXIF block. Returns `None` when the container has
/// no readable EXIF, so callers can still fall back to file metadata.
fn read_exif(path: &Path) -> Option<exif::Exif> {
    let file = std::fs::File::open(path).ok()?;
    let mut buf = std::io::BufReader::new(file);
    exif::Reader::new().read_from_container(&mut buf).ok()
}

/// File modification time in local time, used when no capture date is recorded.
fn file_modified(path: &Path) -> Option<chrono::NaiveDateTime> {
    let modified = std::fs::metadata(path).and_then(|m| m.modified()).ok()?;
    let secs = modified
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_secs() as i64;
    chrono::Utc
        .timestamp_opt(secs, 0)
        .single()
        .map(|dt| dt.naive_utc())
}

/// Extract metadata from a JPEG (or other `image`-crate decodable) file.
pub fn extract_exif(path: &PathBuf) -> anyhow::Result<PhotoMeta> {
    let tags = read_exif(path)
        .ok_or_else(|| anyhow::anyhow!("No readable EXIF block in {}", path.display()))?;

    let (width, height) = {
        let img = image::ImageReader::open(path)?
            .with_guessed_format()?
            .decode()?;
        (img.width(), img.height())
    };

    let tags = ExifTags(&tags);
    Ok(PhotoMeta {
        path: path.clone(),
        iso: tags.iso(),
        aperture: tags.float(exif::Tag::FNumber),
        focal_length: tags.float(exif::Tag::FocalLength),
        camera_model: tags.camera_model(),
        lens_model: tags.lens_model(),
        date_time: tags.date_time().or_else(|| file_modified(path)),
        width,
        height,
        photo_type: "Jpeg".to_string(),
    })
}

/// Extract metadata from a RAW file (ARW, CR2, NEF, RAF, RW2, ORF, PEF, SRW, DNG).
///
/// The `image` crate cannot decode RAW payloads -- it guesses TIFF and then fails
/// with "required tag `ImageWidth` not found" -- so dimensions come from `rawler`,
/// which understands the actual sensor layout. Everything else is read from the
/// EXIF block, which RAW files carry natively.
pub fn extract_raw_exif(path: &PathBuf) -> anyhow::Result<PhotoMeta> {
    let (mut width, mut height, mut camera_model) = (0u32, 0u32, None);

    // `rawler` is the only source of true sensor dimensions.
    match rawler::decode_file(path) {
        Ok(raw) => {
            width = raw.width as u32;
            height = raw.height as u32;
            camera_model = match (raw.clean_make.as_str(), raw.clean_model.as_str()) {
                ("", "") => None,
                ("", m) => Some(m.to_string()),
                (mk, "") => Some(normalize_make(mk)),
                (mk, m) => Some(format!("{} {m}", normalize_make(mk))),
            };
        }
        // Non-fatal: the EXIF block below still yields usable metadata.
        Err(e) => tracing::warn!(target: "exif", "rawler could not decode {:?}: {}", path, e),
    }

    let parsed = read_exif(path);
    let tags = parsed.as_ref().map(ExifTags);

    if width == 0 || height == 0 {
        if let Some(tags) = &tags {
            let (w, h) = tags.dimensions();
            width = w;
            height = h;
        }
    }
    if camera_model.is_none() {
        if let Some(tags) = &tags {
            camera_model = tags.camera_model();
        }
    }

    // A file that yielded neither sensor dimensions nor an EXIF block is not a
    // photo we can report on — it is almost certainly corrupt or not really a
    // RAW file. Returning it would inflate the photo count with empty rows, so
    // fail instead and let `scan_directory` count and log the drop.
    if width == 0 || height == 0 {
        anyhow::bail!(
            "no sensor dimensions and no readable EXIF: {}",
            path.display()
        );
    }

    Ok(PhotoMeta {
        path: path.clone(),
        iso: tags.as_ref().and_then(ExifTags::iso),
        aperture: tags.as_ref().and_then(|t| t.float(exif::Tag::FNumber)),
        focal_length: tags.as_ref().and_then(|t| t.float(exif::Tag::FocalLength)),
        camera_model,
        lens_model: tags.as_ref().and_then(ExifTags::lens_model),
        date_time: tags
            .as_ref()
            .and_then(ExifTags::date_time)
            .or_else(|| file_modified(path)),
        width,
        height,
        photo_type: "Raw".to_string(),
    })
}

pub fn export_csv(photos: &[PhotoMeta], path: &PathBuf) -> anyhow::Result<()> {
    let mut wtr = csv::Writer::from_path(path)?;
    wtr.write_record([
        "File",
        "Date",
        "Aperture",
        "Focal Length (mm)",
        "Camera",
        "Lens",
        "Width",
        "Height",
    ])?;
    for p in photos {
        // Bound to locals so the record is a uniform `&str` array; an inline
        // `&format!(...)` would borrow a temporary.
        let date = p
            .date_time
            .map_or_else(String::new, |dt| dt.format("%Y-%m-%d").to_string());
        let aperture = p.aperture.map_or_else(String::new, |v| format!("{v:.1}"));
        let focal = p
            .focal_length
            .map_or_else(String::new, |v| format!("{v:.0}"));
        let width = p.width.to_string();
        let height = p.height.to_string();

        wtr.write_record([
            p.path.file_name().unwrap().to_string_lossy().as_ref(),
            date.as_str(),
            aperture.as_str(),
            focal.as_str(),
            p.camera_model.as_deref().unwrap_or_default(),
            p.lens_model.as_deref().unwrap_or_default(),
            width.as_str(),
            height.as_str(),
        ])?;
    }
    wtr.flush()?;
    Ok(())
}

pub fn scan_directory(dir: PathBuf, tx: std::sync::mpsc::Sender<Vec<PhotoMeta>>) {
    let extensions = [
        "jpg", "jpeg", "JPG", "JPEG", "cr2", "cr3", "CR2", "CR3", "nef", "NEF", "arw", "ARW",
        "raf", "RAF", "rw2", "RW2", "orf", "ORF", "pef", "PEF", "srw", "SRW", "dng", "DNG",
    ];

    let files: Vec<_> = walkdir::WalkDir::new(&dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter(|e| {
            e.path()
                .extension()
                .and_then(|s| s.to_str())
                .map(|ext| extensions.contains(&ext))
                .unwrap_or(false)
        })
        .map(|e| e.path().to_path_buf())
        .collect();

    crate::logging::log_scan_start(&dir, files.len());

    // Counters for files that could not be turned into a `PhotoMeta`. Without
    // these a dropped file is invisible: the scan simply reports fewer photos
    // than there were files, which looks like a filter or an unsupported format
    // rather than a decode failure.
    let failed = std::sync::atomic::AtomicUsize::new(0);
    let succeeded = std::sync::atomic::AtomicUsize::new(0);
    let failed_raw = std::sync::atomic::AtomicUsize::new(0);
    let failed_jpeg = std::sync::atomic::AtomicUsize::new(0);

    let photos: Vec<PhotoMeta> = files
        .par_iter()
        .filter_map(|path| {
            let ext = path.extension()?.to_str()?.to_lowercase();
            // `ext` is already lowercased above, so upper-case file names are
            // handled here without spelling out every case variant.
            let is_raw = matches!(
                ext.as_str(),
                "cr2" | "cr3" | "nef" | "arw" | "raf" | "rw2" | "orf" | "pef" | "srw" | "dng"
            );

            let result = if is_raw {
                extract_raw_exif(path)
            } else {
                extract_exif(path)
            };

            match &result {
                Ok(_) => {
                    crate::logging::log_exif_extract(path, true, None);
                    succeeded.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                }
                Err(e) => {
                    crate::logging::log_exif_extract(path, false, Some(e));
                    failed.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    if is_raw {
                        failed_raw.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    } else {
                        failed_jpeg.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    }
                }
            }

            result.ok()
        })
        .collect();

    let ok = succeeded.load(std::sync::atomic::Ordering::Relaxed);
    let bad = failed.load(std::sync::atomic::Ordering::Relaxed);
    let bad_raw = failed_raw.load(std::sync::atomic::Ordering::Relaxed);
    let bad_jpeg = failed_jpeg.load(std::sync::atomic::Ordering::Relaxed);

    if bad > 0 {
        // Warn rather than error: a few unsupported files in a large library is
        // expected, but the count is the fastest way to explain a photo count
        // that does not match the file count.
        warn!(
            target: "scan",
            "Dropped {} of {} files (raw: {}, jpeg: {}). Per-file reasons are logged above.",
            bad,
            ok + bad,
            bad_raw,
            bad_jpeg
        );
    } else {
        debug!(target: "scan", "All {ok} files decoded successfully");
    }

    debug!(
        target: "scan",
        "Scan result: {} photos kept, {} dropped",
        photos.len(),
        bad
    );

    let _ = tx.send(photos);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn create_test_jpeg(dir: &Path) -> PathBuf {
        let path = dir.join("test.jpg");
        let img = image::ImageBuffer::from_fn(100, 100, |_, _| image::Rgb([128u8, 128u8, 128u8]));
        img.save_with_format(&path, image::ImageFormat::Jpeg)
            .unwrap();
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
                e.path()
                    .extension()
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
    fn test_normalize_make_folds_case() {
        assert_eq!(normalize_make("SONY"), "Sony");
        assert_eq!(normalize_make("nikon"), "Nikon");
        assert_eq!(normalize_make("CANON INC"), "Canon Inc");
    }

    /// Path to the real-EXIF fixture directory, or `None` if it is absent.
    ///
    /// The fixtures are real camera captures (Sony ILME-FX30 `.ARW` plus the
    /// matching JPEGs), which cannot be synthesised in a test, so they are not
    /// committed. Tests that need them skip instead of failing when absent.
    /// Checks for *files*, not just the directory: an empty directory means the
    /// fixtures are missing, and proceeding would assert on zero photos.
    fn fixtures_dir() -> Option<PathBuf> {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("test_files");
        let has_files = fs::read_dir(dir.join("raw"))
            .map(|entries| entries.flatten().any(|e| e.path().is_file()))
            .unwrap_or(false);
        has_files.then_some(dir)
    }

    #[test]
    fn test_scan_directory_includes_raw_and_jpeg() {
        // Guards the regression where `image` was used to read RAW dimensions:
        // it fails on ARW, which silently dropped every RAW file from results.
        let Some(dir) = fixtures_dir() else {
            eprintln!("skipping: test_files/ fixtures not present");
            return;
        };

        let (tx, rx) = std::sync::mpsc::channel();
        scan_directory(dir, tx);
        let photos = rx.recv().unwrap();

        let raw_count = photos.iter().filter(|p| p.photo_type == "Raw").count();
        let jpeg_count = photos.iter().filter(|p| p.photo_type == "Jpeg").count();

        assert!(raw_count > 0, "no RAW files were extracted");
        assert!(jpeg_count > 0, "no JPEG files were extracted");

        for p in &photos {
            assert!(
                p.width > 0 && p.height > 0,
                "{} has no dimensions",
                p.path.display()
            );
            assert!(
                p.camera_model.is_some(),
                "{} has no camera",
                p.path.display()
            );
            assert!(p.iso.is_some(), "{} has no ISO", p.path.display());
            assert!(p.aperture.is_some(), "{} has no aperture", p.path.display());
            assert!(p.lens_model.is_some(), "{} has no lens", p.path.display());
            // Make/model must not carry the quoting `display_value` adds.
            let cam = p.camera_model.as_deref().unwrap();
            assert!(
                !cam.contains('"'),
                "{} camera has stray quotes: {cam}",
                p.path.display()
            );
        }

        // JPEG and RAW copies of the same scene must group under one camera.
        let stats = Stats::from_photos(&photos);
        assert_eq!(
            stats.camera_counts.len(),
            1,
            "cameras did not merge: {:?}",
            stats.camera_counts
        );
    }

    /// A corrupt file must be dropped, not surfaced as a row of empty data.
    ///
    /// The RAW path used to return `Ok` unconditionally, so a truncated `.ARW`
    /// produced a `PhotoMeta` with no ISO, no lens, and `0x0` dimensions. That
    /// silently inflated the photo count and skewed every chart, which is the
    /// mirror image of the earlier bug where valid RAW files were dropped.
    #[test]
    fn test_corrupt_files_are_dropped() {
        // Real fixtures: a synthetic JPEG has no EXIF and would itself be
        // dropped, which would not prove anything about the corrupt files.
        let Some(fixtures) = fixtures_dir() else {
            eprintln!("skipping: test_files/ fixtures not present");
            return;
        };
        let sample = fixtures
            .join("raw")
            .read_dir()
            .unwrap()
            .flatten()
            .map(|e| e.path())
            .find(|p| p.is_file())
            .expect("fixtures_dir() confirmed at least one RAW file");

        let dir = TempDir::new().unwrap();
        // Garbage with the right extension, for both paths.
        for name in ["broken.jpg", "broken.ARW", "also_broken.dng"] {
            fs::write(dir.path().join(name), b"this is not a photo").unwrap();
        }
        let good = dir.path().join("good.ARW");
        fs::copy(&sample, &good).unwrap();

        let (tx, rx) = std::sync::mpsc::channel();
        scan_directory(dir.path().to_path_buf(), tx);
        let photos = rx.recv().unwrap();

        assert_eq!(
            photos.len(),
            1,
            "expected only the valid RAW, got {:?}",
            photos.iter().map(|p| &p.path).collect::<Vec<_>>()
        );
        assert_eq!(photos[0].photo_type, "Raw");
        assert!(photos[0].width > 0 && photos[0].height > 0);
    }

    #[test]
    fn test_export_csv() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("test.csv");

        let photos = vec![PhotoMeta {
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
        }];

        export_csv(&photos, &path).unwrap();
        let content = fs::read_to_string(&path).unwrap();
        assert!(content.contains("a.jpg"));
        assert!(content.contains("2.8"));
        assert!(content.contains("35"));
        assert!(content.contains("Cam A"));
        assert!(content.contains("Lens X"));
    }
}
