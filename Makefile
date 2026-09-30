.PHONY: build run test test-lib test-ui clean install-deps fmt clippy check bundle

# Default target
all: build

# Build release binary
build:
	cargo build --release

# Build debug binary
build-debug:
	cargo build

# Run the app
run: build
	./target/release/camera_stats

# Run debug build
run-debug: build-debug
	./target/debug/camera_stats

# Run all tests
test:
	cargo test

# Run only library tests
test-lib:
	cargo test --lib

# Run tests with output
test-verbose:
	cargo test -- --nocapture

# Format code
fmt:
	cargo fmt

# Lint code
clippy:
	cargo clippy -- -D warnings

# Check code compiles (no build)
check:
	cargo check

# Clean build artifacts
clean:
	cargo clean

# Install Linux dependencies (Ubuntu/Debian/Fedora/Arch)
install-deps:
	@echo "Installing system dependencies..."
	@if command -v apt-get >/dev/null; then \
		sudo apt-get update && sudo apt-get install -y \
			libgtk-3-dev libatk1.0-dev libcairo2-dev libpango1.0-dev \
			libgdk-pixbuf2.0-dev libwebkit2gtk-4.1-dev \
			build-essential pkg-config libssl-dev; \
	elif command -v dnf >/dev/null; then \
		sudo dnf install -y \
			gtk3-devel atk-devel cairo-devel pango-devel \
			gdk-pixbuf2-devel webkit2gtk4.1-devel \
			gcc-c++ pkg-config openssl-devel; \
	elif command -v pacman >/dev/null; then \
		sudo pacman -S --needed gtk3 atk cairo pango gdk-pixbuf2 webkit2gtk-4.1 base-devel pkg-config openssl; \
	else \
		echo "Unknown package manager. Please install GTK3 dev libraries manually."; \
		exit 1; \
	fi

# Create macOS .app bundle
bundle:
	@if [ "$$(uname)" = "Darwin" ]; then \
		if ! command -v cargo-bundle >/dev/null; then \
			cargo install cargo-bundle; \
		fi; \
		cargo bundle --release; \
		echo "App bundle: target/release/bundle/macos/Camera Stats.app"; \
	else \
		echo "Bundle target only available on macOS"; \
		exit 1; \
	fi

# Cross-platform build using build.sh
build-all:
	./build.sh

# Run with debug logging
run-debug-log:
	RUST_LOG=debug,camera_stats=trace cargo run

# Run with specific log targets
run-scan-log:
	RUST_LOG=scan=debug cargo run

run-exif-log:
	RUST_LOG=exif=debug cargo run

run-ui-log:
	RUST_LOG=ui=debug cargo run

# Development workflow: fmt + clippy + test
dev-check: fmt clippy test

# Release workflow: clean + fmt + clippy + test + build
release: clean fmt clippy test build

# Help
help:
	@echo "Available targets:"
	@echo "  build         - Build release binary"
	@echo "  build-debug   - Build debug binary"
	@echo "  run           - Build and run release"
	@echo "  run-debug     - Build and run debug"
	@echo "  test          - Run all tests"
	@echo "  test-lib      - Run library tests only"
	@echo "  test-verbose  - Run tests with output"
	@echo "  fmt           - Format code"
	@echo "  clippy        - Lint code"
	@echo "  check         - Check compilation"
	@echo "  clean         - Clean build artifacts"
	@echo "  install-deps  - Install Linux GTK dependencies"
	@echo "  bundle        - Create macOS .app bundle"
	@echo "  build-all     - Cross-platform build via build.sh"
	@echo "  run-debug-log - Run with debug logging"
	@echo "  dev-check     - fmt + clippy + test"
	@echo "  release       - Clean + fmt + clippy + test + build"