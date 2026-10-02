# Camera Stats

Cross-platform desktop app to analyze EXIF data from photos (JPEG + RAW) and show shooting statistics.

## Features

- 📁 Recursive folder scanning
- 📸 Supports JPEG, CR2/3, NEF, ARW, RAF, RW2, ORF, PEF, SRW, DNG
- ⚡ Parallel EXIF extraction (Rayon) on a background thread — the UI stays responsive during a scan
- 📊 Charts: ISO, Aperture, and Focal Length distributions
  - Chart style: bars, lines, or points
  - Aperture axis: linear, logarithmic, or snapped to standard f-stops
  - Read-only and auto-scaling: charts cannot be panned or zoomed, and the y axis
    always starts at 0 with 5 units of headroom above the tallest bar
- 🔽 Sortable photo table (click a header to sort, again to reverse)
- 🔎 Multi-select filters on Aperture, Focal Length, Camera, Lens, and Type — filters combine with AND
- 📄 CSV export of the loaded photos
- ⚙️ Settings window for chart/table visibility and row limits
- 🖥️ Native UI on macOS (Metal), Linux (GTK3), Windows (WGPU)

### Known limitations

These are real gaps, not oversights in the docs:

- **Camera and lens summary tables are not implemented.** The Settings window has
  "Camera Table" and "Lens Table" checkboxes, but nothing renders those tables.
- **The photo table shows 7 columns** (File, Date, Aperture, Focal, Camera, Lens,
  Type). The Settings window offers 139 optional EXIF columns; those toggles are
  stored but not yet rendered.
- **Display preferences are not saved between launches.** Changing a setting
  affects the current session only.
- **Charts show the filtered selection**, not the whole library — this is
  intentional, so the charts stay consistent with the table below them.

## Quick Start

### macOS
```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env

# Build
git clone <this-repo>
cd ShutterStats
./build.sh
# or: cargo build --release
```

### Ubuntu/Debian
```bash
sudo apt update && sudo apt install -y \
  libgtk-3-dev libatk1.0-dev libcairo2-dev libpango1.0-dev \
  libgdk-pixbuf2.0-dev libwebkit2gtk-4.1-dev \
  build-essential pkg-config libssl-dev

curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env

cd ShutterStats
cargo build --release
```

### Fedora
```bash
sudo dnf install -y \
  gtk3-devel atk-devel cairo-devel pango-devel \
  gdk-pixbuf2-devel webkit2gtk4.1-devel \
  gcc-c++ pkg-config openssl-devel

curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env

cd ShutterStats
cargo build --release
```

### Arch
```bash
sudo pacman -S --needed gtk3 atk cairo pango gdk-pixbuf2 webkit2gtk-4.1 base-devel pkg-config openssl
```

### Run
```bash
./target/release/camera_stats
```

Then click **📁 Select Folder** and choose a directory of photos.

### macOS App Bundle
```bash
cargo install cargo-bundle
cargo bundle --release
# Creates: target/release/bundle/macos/Camera Stats.app
```

## Make Targets

```bash
make build          # release build
make run            # build and run the release binary
make test           # run all tests
make fmt            # cargo fmt
make clippy         # cargo clippy -D warnings
make dev-check      # fmt + clippy + test
make bundle         # macOS .app bundle
```

### Logging

Logs rotate daily under the platform data directory:
`~/Library/Application Support/camera_stats/logs/` (macOS),
`~/.local/share/camera_stats/logs/` (Linux).

Nothing is logged unless `RUST_LOG` is set, because a desktop GUI app has no
console attached:

```bash
make run-debug          # presets RUST_LOG for the scan/exif/ui targets
make run-debug-log      # most verbose
make run-scan-log       # folder scanning only
make run-exif-log       # per-file EXIF extraction only
make run-ui-log         # user interactions only
```

To debug a photo count that looks wrong, the scan summary answers it directly:

```
Dropped 2 of 24 files (raw: 1, jpeg: 1). Per-file reasons are logged above.
Scan result: 22 photos kept, 2 dropped
```

Set `RUST_LOG=exif=debug` for the per-file reason each file was dropped. If the
counts are right but a table is empty, the cause is filtering rather than
scanning; the `ui` target logs every filter change with its resulting row count.

## Dependencies

- **GUI**: `eframe`/`egui` (immediate mode, native backends)
- **EXIF (JPEG and RAW)**: `kamadak-exif` (pure Rust) — `.ARW` and other RAW
  containers are TIFF-based, so their EXIF block is read the same way
- **RAW dimensions**: `rawler` (libraw bindings) — the only source of true sensor
  dimensions
- **Image dims (JPEG)**: `image` crate
- **File dialog**: `rfd`
- **Parallel**: `rayon`
- **Charts**: `egui_plot`
- **Logging**: `tracing` + `tracing-appender`
- **Export**: `csv`

## Project Structure

```
src/
  main.rs          # Entry point only: module declarations + main()
  lib.rs           # Core logic (public API, unit tested, no UI)
  logging.rs       # Tracing setup + log helpers
  app.rs           # App state + eframe update loop
  config.rs        # Display preferences
  table.rs         # Photo-table sorting
  filter.rs        # Filter predicate, option lists, dropdown widget
  analytics.rs     # Photos -> chart data
  ui/
    toolbar.rs     # Folder select, export, status
    charts.rs      # Distribution charts
    photo_table.rs # Sortable, filterable photo grid
    settings.rs    # Settings window

Cargo.toml
build.sh           # Cross-platform build helper
Makefile           # Common dev tasks
test_files/        # Real EXIF fixtures (not committed; see Testing)
```

The split is deliberate: `lib.rs` holds decoding and statistics with no UI
dependency, so it is unit tested without starting a window. The binary modules
separate **state** (`app.rs`) from **drawing** (`ui/`), and `filter.rs`,
`table.rs`, and `analytics.rs` are pure functions with no `egui` or I/O.

## Testing

```bash
cargo test              # all tests
cargo test -- --nocapture
```

Some tests need real EXIF fixtures, which are **not committed** — they are
actual camera captures rather than synthesised files. To exercise the RAW and
scan paths, drop your own samples in:

```
test_files/
  raw/     DSC00001.ARW  DSC00002.ARW ...
  jpeg/    DSC00001.JPG  DSC00002.JPG ...
```

Any supported RAW extension works (`.ARW`, `.CR2`, `.NEF`, `.DNG`, ...), and
pairing a RAW with its JPEG export lets the tests verify the two merge under one
camera. Without fixtures those tests print `skipping: test_files/ fixtures not
present` and pass vacuously — so if you are investigating a RAW bug, check that
line before trusting a green run.

## License

MIT
