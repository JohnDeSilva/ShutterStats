# Camera Stats

Cross-platform desktop app to analyze EXIF data from photos (JPEG + RAW) and show shooting statistics.

## Features

- 📁 Recursive folder scanning
- 📸 Supports JPEG, CR2/3, NEF, ARW, RAF, RW2, ORF, PEF, SRW, DNG
- ⚡ Parallel EXIF extraction (Rayon)
- 📊 Interactive charts: ISO, Aperture, Focal Length distributions
- 📋 Top cameras/lenses tables
- 📄 Full photo metadata table
- 🖥️ Native UI on macOS (Metal), Linux (GTK3), Windows (WGPU)

## Quick Start

### macOS
```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env

# Build
git clone <this-repo>
cd camera_stats
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

cd camera_stats
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

cd camera_stats
cargo build --release
```

### Run
```bash
./target/release/camera_stats
```

### macOS App Bundle
```bash
cargo install cargo-bundle
cargo bundle --release
# Creates: target/release/bundle/macos/Camera Stats.app
```

## Dependencies

- **GUI**: `eframe`/`egui` (immediate mode, native backends)
- **EXIF (JPEG)**: `kamadak-exif` (pure Rust)
- **EXIF (RAW)**: `rawler` (libraw bindings)
- **Image dims**: `image` crate
- **File dialog**: `rfd`
- **Parallel**: `rayon`
- **Charts**: `egui_plot`

## Project Structure

```
src/
  main.rs      # App entry, UI, scanning logic
Cargo.toml     # Dependencies
build.sh       # Cross-platform build helper
```

## License

MIT