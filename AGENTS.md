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

```
src/
├── main.rs          # App entry point, UI, event loop
├── lib.rs           # Core logic (public API, tests)
│   ├── PhotoMeta    # Photo metadata struct
│   ├── Stats        # Aggregated statistics
│   ├── extract_exif()        # JPEG EXIF extraction
│   ├── extract_raw_exif()    # RAW EXIF extraction
│   ├── scan_directory()      # Parallel folder scanning
│   └── export_csv()          # CSV export
└── logging.rs       # Tracing setup + helper macros
```

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

### Logging
- Logs to `~/Library/Application Support/camera_stats/logs/` (macOS)
- Logs to `~/.local/share/camera_stats/logs/` (Linux)
- Daily rotating files: `camera_stats.log.YYYY-MM-DD`
- Control verbosity: `RUST_LOG=debug cargo run`
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
5. Add chart/table in `main.rs` UI

### New Export Format
1. Add function in `lib.rs` (e.g., `export_json`)
2. Add button in toolbar (`main.rs`)
3. Add file dialog with appropriate filter

### New RAW Format
1. Check `rawler` supports it (uses libraw)
2. Add extension to `scan_directory` extensions list
3. Test with real files

## Error Handling
- All fallible operations return `anyhow::Result<T>`
- Log errors with context: `error!("Failed to process {:?}: {}", path, err)`
- UI shows user-friendly status messages, not raw errors
- Failed EXIF extraction skips file (logs warning, continues)

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