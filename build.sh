#!/bin/bash
# Cross-platform build script for camera_stats

set -e

echo "🔧 Building camera_stats..."

# Detect OS
OS="$(uname -s)"

case "$OS" in
    Linux*)
        echo "📦 Linux detected - checking dependencies..."
        if command -v apt-get &> /dev/null; then
            echo "   Ubuntu/Debian - installing GTK deps..."
            sudo apt-get update && sudo apt-get install -y \
                libgtk-3-dev libatk1.0-dev libcairo2-dev libpango1.0-dev \
                libgdk-pixbuf2.0-dev libwebkit2gtk-4.1-dev \
                build-essential pkg-config libssl-dev
        elif command -v dnf &> /dev/null; then
            echo "   Fedora - installing GTK deps..."
            sudo dnf install -y \
                gtk3-devel atk-devel cairo-devel pango-devel \
                gdk-pixbuf2-devel webkit2gtk4.1-devel \
                gcc-c++ pkg-config openssl-devel
        elif command -v pacman &> /dev/null; then
            echo "   Arch - installing GTK deps..."
            sudo pacman -S --needed gtk3 atk cairo pango gdk-pixbuf2 webkit2gtk-4.1 base-devel pkg-config openssl
        else
            echo "   ⚠️  Unknown distro - please install GTK3 dev libraries manually"
        fi
        ;;
    Darwin*)
        echo "🍎 macOS detected - no extra deps needed"
        ;;
    *)
        echo "❌ Unsupported OS: $OS"
        exit 1
        ;;
esac

# Build
echo "🔨 Building release binary..."
cargo build --release

echo "✅ Build complete!"
echo "   Binary: target/release/camera_stats"

# Offer to create macOS .app bundle
if [[ "$OS" == "Darwin" ]]; then
    read -p "Create .app bundle? (y/n) " -n 1 -r
    echo
    if [[ $REPLY =~ ^[Yy]$ ]]; then
        if ! command -v cargo-bundle &> /dev/null; then
            cargo install cargo-bundle
        fi
        cargo bundle --release
        echo "📦 App bundle: target/release/bundle/macos/Camera Stats.app"
    fi
fi