#!/usr/bin/env bash
# =============================================================================
# ScaryDB Universal Build Script
# Cross-platform build automation for Linux, macOS, and Windows (via WSL/Git Bash)
# =============================================================================

set -euo pipefail

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Configuration
PROJECT_NAME="scarydb"
VERSION="${VERSION:-$(grep '^version' Cargo.toml | head -1 | sed 's/.*= *"//;s/".*//')}"
BUILD_MODE="${BUILD_MODE:-release}"
TARGET_DIR="target/${BUILD_MODE}"
BINARY_NAME="${PROJECT_NAME}"

# Platform detection
OS="$(uname -s | tr '[:upper:]' '[:lower:]')"
ARCH="$(uname -m)"

# Logging functions
log_info() { echo -e "${BLUE}[INFO]${NC} $*"; }
log_success() { echo -e "${GREEN}[SUCCESS]${NC} $*"; }
log_warning() { echo -e "${YELLOW}[WARNING]${NC} $*"; }
log_error() { echo -e "${RED}[ERROR]${NC} $*"; }

# Help function
show_help() {
    cat << EOF
ScaryDB Universal Build Script v${VERSION}

Usage: $0 [OPTIONS]

OPTIONS:
    -h, --help              Show this help message
    -m, --mode MODE         Build mode: debug|release (default: release)
    -t, --target TARGET     Rust target triple (default: auto-detect)
    --features FEATURES     Comma-separated Cargo features to enable
    --no-run                Don't run tests after build
    --package               Create distribution package after build
    --clean                 Clean build artifacts before building

EXAMPLES:
    $0                      # Release build for current platform
    $0 -m debug             # Debug build
    $0 --package            # Build and create distribution package
    $0 --clean -m release   # Clean release build

SUPPORTED PLATFORMS:
    Linux   (x86_64, aarch64, armv7)
    macOS   (x86_64, arm64)
    Windows (x86_64 via WSL/Git Bash/MSYS2)

EOF
}

# Parse arguments
CLEAN_BUILD=false
RUN_TESTS=true
CREATE_PACKAGE=false
CUSTOM_TARGET=""
CARGO_FEATURES=""

while [[ $# -gt 0 ]]; do
    case $1 in
        -h|--help)
            show_help
            exit 0
            ;;
        -m|--mode)
            BUILD_MODE="$2"
            shift 2
            ;;
        -t|--target)
            CUSTOM_TARGET="$2"
            shift 2
            ;;
        --features)
            CARGO_FEATURES="$2"
            shift 2
            ;;
        --no-run)
            RUN_TESTS=false
            shift
            ;;
        --package)
            CREATE_PACKAGE=true
            shift
            ;;
        --clean)
            CLEAN_BUILD=true
            shift
            ;;
        *)
            log_error "Unknown option: $1"
            show_help
            exit 1
            ;;
    esac
done

# Validate build mode
if [[ "$BUILD_MODE" != "debug" && "$BUILD_MODE" != "release" ]]; then
    log_error "Invalid build mode: $BUILD_MODE (must be 'debug' or 'release')"
    exit 1
fi

# Determine target
if [[ -n "$CUSTOM_TARGET" ]]; then
    TARGET="$CUSTOM_TARGET"
else
    case "$OS" in
        linux)
            case "$ARCH" in
                x86_64) TARGET="x86_64-unknown-linux-gnu" ;;
                aarch64) TARGET="aarch64-unknown-linux-gnu" ;;
                armv7*) TARGET="armv7-unknown-linux-gnueabihf" ;;
                *) log_error "Unsupported Linux architecture: $ARCH"; exit 1 ;;
            esac
            ;;
        darwin)
            case "$ARCH" in
                x86_64) TARGET="x86_64-apple-darwin" ;;
                arm64) TARGET="aarch64-apple-darwin" ;;
                *) log_error "Unsupported macOS architecture: $ARCH"; exit 1 ;;
            esac
            ;;
        msys*|cygwin*|mingw*)
            TARGET="x86_64-pc-windows-gnu"
            BINARY_NAME="${PROJECT_NAME}.exe"
            ;;
        *)
            log_error "Unsupported OS: $OS"
            exit 1
            ;;
    esac
fi

log_info "Building ScaryDB v${VERSION}"
log_info "Platform: $OS ($ARCH)"
log_info "Target: $TARGET"
log_info "Build mode: $BUILD_MODE"

# Check for Rust toolchain
if ! command -v cargo &> /dev/null; then
    log_error "Rust toolchain not found. Please install from https://rustup.rs/"
    exit 1
fi

# Check if target is installed
if [[ -n "$CUSTOM_TARGET" || "$TARGET" != "$(rustc -vV | grep host | cut -d' ' -f2)" ]]; then
    if ! rustup target list --installed | grep -q "^$TARGET$"; then
        log_info "Installing target: $TARGET"
        rustup target add "$TARGET"
    fi
fi

# Clean if requested
if [[ "$CLEAN_BUILD" == true ]]; then
    log_info "Cleaning previous build artifacts..."
    cargo clean --target "$TARGET"
fi

# Build command
BUILD_ARGS=("--target" "$TARGET" "--bin" "$PROJECT_NAME" "--bin" "benchmark")
if [[ "$BUILD_MODE" == "release" ]]; then
    BUILD_ARGS+=("--release")
fi
if [[ -n "$CARGO_FEATURES" ]]; then
    BUILD_ARGS+=("--features" "$CARGO_FEATURES")
fi

log_info "Running: cargo build ${BUILD_ARGS[*]}"
cargo build "${BUILD_ARGS[@]}"

# Verify binary exists
BINARY_PATH="${TARGET_DIR}/${BINARY_NAME}"
if [[ ! -f "$BINARY_PATH" ]]; then
    log_error "Build failed: binary not found at $BINARY_PATH"
    exit 1
fi

log_success "Build completed successfully!"
log_info "Binary: $BINARY_PATH"
log_info "Size: $(du -h "$BINARY_PATH" | cut -f1)"

# Run tests if requested
if [[ "$RUN_TESTS" == true ]]; then
    log_info "Running tests..."
    cargo test --target "$TARGET" ${BUILD_MODE:+--release} ${CARGO_FEATURES:+--features "$CARGO_FEATURES"}
    log_success "All tests passed!"
fi

# Create package if requested
if [[ "$CREATE_PACKAGE" == true ]]; then
    log_info "Creating distribution package..."
    ./package.sh --target "$TARGET" --mode "$BUILD_MODE"
fi

log_success "Done!"