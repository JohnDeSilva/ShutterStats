# Camera Stats - Agent Guidelines

## Project Overview
Cross-platform desktop app for analyzing EXIF data from photos (JPEG + RAW) with statistics visualization.

## Tech Stack
- **Language**: Rust 2021 edition
- **GUI**: `eframe`/`egui` (immediate mode, native backends: Metal on macOS, GTK3 on Linux, WGPU on Windows)
- **EXIF**: `kamadak-exif` (JPEG), `rawler` (RAW via libraw)
- **Parallel**: `rayon`
- **Charts**: `egui_plot`
- **Logging**: `tracing` + `tracing-subscriber` + `tracing-appender`
- **Serialization**: `serde` + `csv`

## Architecture

The crate is split in two halves: `lib.rs` is the library (no UI, unit tested),
and the binary modules hold all GUI code.

```
src/
├── main.rs          # Entry point only: `mod` declarations + main()
├── lib.rs           # Core logic (public API, tests)
│   ├── PhotoMeta              # Photo metadata struct
│   ├── Stats                  # Aggregated statistics
│   ├── extract_exif()         # JPEG EXIF extraction
│   ├── extract_raw_exif()     # RAW EXIF extraction
│   ├── scan_directory()       # Parallel folder scanning
│   └── export_csv()           # CSV export
├── logging.rs       # Tracing setup + log helpers
│
├── app.rs           # CameraStatsApp state + eframe::App update loop
├── config.rs        # DisplayConfig, PhotoTableColumns (user preferences)
├── table.rs         # SortColumn/SortOrder/TableState + sort_photos()
├── filter.rs        # Filter predicate, option lists, dropdown widget
├── analytics.rs     # Photos -> chart data (pure functions)
└── ui/              # All egui drawing code
    ├── mod.rs
    ├── toolbar.rs      # Folder select, export, settings, status
    ├── charts.rs       # ISO / aperture / focal charts
    ├── photo_table.rs  # Sortable, filterable photo grid
    └── settings.rs     # Display settings window
```

### Module rules
- `app.rs` holds **state**; `ui/` holds **drawing**. A view function takes
  `&mut CameraStatsApp` (or narrower fields) and draws — it never owns state.
- `filter.rs`, `analytics.rs`, and `table.rs` are **pure**: no `egui`, no I/O.
  That is what makes them directly unit testable.
- `ui/settings.rs` declares its 139 column checkboxes as data via the
  `column_group!` macro, naming each `PhotoTableColumns` field as a literal
  identifier. A test asserts the declared groups cover every struct field, so a
  new field cannot be added without either a checkbox or a test failure.
- Do not reintroduce a 1000-line `main.rs`; add a module instead.

## Development Workflow

### Building
```bash
# All platforms
./build.sh

# Or manually:
cargo build --release
```

### Testing
```bash
# Unit tests (in lib.rs)
cargo test

# With logging output
cargo test -- --nocapture

# Specific test
cargo test test_stats_from_photos
```

Tests live next to the code they cover (`#[cfg(test)] mod tests` in each module).
Pure modules (`filter`, `table`, `analytics`) are fully covered without fixtures.

**RAW/scan tests need real fixtures in `test_files/raw/` and `test_files/jpeg/`,
which are gitignored because they are actual camera captures.** Without them those
tests print `skipping: test_files/ fixtures not present` and pass vacuously — a
green run does **not** mean the RAW path was exercised. The `fixtures_dir()`
helper checks for files, not just the directory, so an empty dir also skips.

### Logging
- Logs to `~/Library/Application Support/camera_stats/logs/` (macOS)
- Logs to `~/.local/share/camera_stats/logs/` (Linux)
- Daily rotating files: `camera_stats.log.YYYY-MM-DD`
- Nothing is logged unless `RUST_LOG` is set — there is no console to attach to.
  Use `make run-debug` (presets `RUST_LOG`) rather than `cargo run`.
- Targets: `scan`, `exif`, `ui`

### Code Style
- Run `cargo fmt` before commits
- Run `cargo clippy` - fix all warnings
- No `unwrap()`/`expect()` in production code - use `?` and `anyhow::Result`
- Public API in `lib.rs` with doc comments
- Tests co-located in `lib.rs` under `#[cfg(test)]`

## Cross-Platform Notes

| Platform | GUI Backend | System Deps |
|----------|-------------|-------------|
| macOS | Metal | None |
| Ubuntu/Debian | GTK3 | `libgtk-3-dev libwebkit2gtk-4.1-dev ...` |
| Fedora | GTK3 | `gtk3-devel webkit2gtk4.1-devel ...` |
| Arch | GTK3 | `gtk3 webkit2gtk-4.1` |
| Windows | WGPU/DX12 | MSVC toolchain |

The `build.sh` script auto-installs Linux deps.

## Adding Features

### New Statistics
1. Add field to `PhotoMeta` (if new EXIF tag)
2. Update `extract_exif`/`extract_raw_exif`
3. Add counter to `Stats` struct
4. Update `Stats::from_photos`
5. Add the aggregation function to `analytics.rs`
6. Draw it from `ui/charts.rs`

### New Photo Table Column
1. Add `show_*` field to `PhotoTableColumns` (`config.rs`) with a `Default`
2. Add one `column_group!` entry in `ui/settings.rs` — the test will fail if
   you skip this
3. Add one `Column` entry to `COLUMNS` in `ui/photo_table.rs`, giving it a
   `visible`, `value`, and optionally `filter` + `filter_slot`

### New Export Format
1. Add function in `lib.rs` (e.g., `export_json`)
2. Add button in `ui/toolbar.rs`
3. Add file dialog with appropriate filter

### New RAW Format
1. Check `rawler` supports it (uses libraw)
2. Add the extension to `PHOTO_EXTENSIONS` in `lib.rs` (lowercase only —
   matching is case-insensitive) so the file gets scanned, and to
   `is_raw_extension` so it routes to the RAW extractor instead of the
   JPEG one
3. Add it to both tests in the `lib.rs` test module; they cover the two
   lists separately, so a missing entry shows up as a failure
4. Test with real files, and confirm the count in the scan log matches

## Error Handling
- All fallible operations return `anyhow::Result<T>`
- Log errors with context: `error!("Failed to process {:?}: {}", path, err)`
- UI shows user-friendly status messages, not raw errors
- Failed EXIF extraction skips file (logs warning, continues)
- **A file that yields no usable data must return `Err`, not a half-filled
  `PhotoMeta`.** A row with no ISO, no lens, and `0x0` dimensions is worse than
  a dropped file: it inflates the count and skews every chart. `extract_exif`
  fails when there is no EXIF block; `extract_raw_exif` fails when there are
  neither sensor dimensions nor readable EXIF.

## Performance
- Scanning runs on background thread (channel + `thread::spawn`)
- EXIF extraction parallelized with `rayon::par_iter`
- Large folders: consider adding progress indicator
- Charts use `egui_plot` - efficient for <10k points

## Logging Best Practices
```rust
// Structured logging with targets
info!(target: "scan", "Scanning {:?} ({} files)", dir, count);
warn!(target: "exif", "Failed {:?}: {}", path, err);

// Use helpers from logging.rs
log_scan_start(&dir, files.len());
log_exif_extract(path, success, err.as_ref());
log_filter_result(shown, total, filters);
log_stats_summary(iso_n, ap_n, fl_n, cam_n, lens_n);
```

Targets: `scan` (folder walking + summary), `exif` (per-file extraction),
`ui` (user interactions, filter changes, truncation).

### Debugging a wrong photo count
The log answers this directly. `scan_directory` emits a summary on every run:
```
Dropped 2 of 24 files (raw: 1, jpeg: 1). Per-file reasons are logged above.
Scan result: 22 photos kept, 2 dropped
```
Per-file `warn!` lines with `RUST_LOG=exif=debug` give the reason. If the counts
match but a table is empty, the cause is filtering, not scanning — `ui` target
logs every filter change with the row count it produced.

Set `RUST_LOG` before running, otherwise nothing is written:
```bash
make run-debug        # presets RUST_LOG for scan/exif/ui
make run-exif-log     # per-file extraction detail only
```

## Dependencies
- Keep `Cargo.toml` minimal
- Prefer pure-Rust crates over system deps
- Pin versions in `Cargo.lock` (committed)
- Audit with `cargo audit` periodically

## Release Checklist
- [ ] `cargo test` passes
- [ ] `cargo clippy` clean
- [ ] `cargo fmt` applied
- [ ] Version bump in `Cargo.toml`
- [ ] `cargo build --release` works on all targets
- [ ] macOS: `cargo bundle --release` creates `.app`
- [ ] Update CHANGELOG.md

## Common Issues

**"Package atk not found" (Linux)**
→ Install GTK dev packages (see `build.sh`)

**"No such file" for RAW**
→ `rawler` needs libraw; ensure RAW file isn't corrupted

**UI freezes during scan**
→ Scan runs on background thread; ensure channel recv is non-blocking

**Charts empty**
→ Check EXIF extraction logs; some cameras write non-standard tags