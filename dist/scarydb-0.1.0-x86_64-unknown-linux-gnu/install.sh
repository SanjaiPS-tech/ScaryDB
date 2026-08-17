#!/usr/bin/env bash
# ScaryDB Installation Script for Linux/macOS
# Run with: sudo ./install.sh

set -euo pipefail

INSTALL_DIR="/opt/scarydb"
SERVICE_USER="scarydb"
SERVICE_GROUP="scarydb"
BINARY_NAME="scarydb"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

log_info() { echo -e "${BLUE}[INFO]${NC} $*"; }
log_success() { echo -e "${GREEN}[SUCCESS]${NC} $*"; }
log_warning() { echo -e "${YELLOW}[WARNING]${NC} $*"; }
log_error() { echo -e "${RED}[ERROR]${NC} $*"; }

# Check root
if [[ $EUID -ne 0 ]]; then
    log_error "This script must be run as root (use sudo)"
    exit 1
fi

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

log_info "Installing ScaryDB to $INSTALL_DIR"

# Create service user
if ! id "$SERVICE_USER" &>/dev/null; then
    log_info "Creating service user: $SERVICE_USER"
    useradd --system --no-create-home --shell /bin/false "$SERVICE_USER" || true
fi

# Create install directory
mkdir -p "$INSTALL_DIR/data"

# Copy files
log_info "Copying binaries and config..."
cp "$SCRIPT_DIR/$BINARY_NAME" "$INSTALL_DIR/"
cp "$SCRIPT_DIR/config.json" "$INSTALL_DIR/"
chmod +x "$INSTALL_DIR/$BINARY_NAME"

# Set ownership
chown -R "$SERVICE_USER:$SERVICE_GROUP" "$INSTALL_DIR"

# Install systemd service (Linux)
if [[ -f /etc/os-release ]] && command -v systemctl &>/dev/null; then
    log_info "Installing systemd service..."
    cp "$SCRIPT_DIR/scarydb.service" /etc/systemd/system/
    systemctl daemon-reload
    systemctl enable scarydb
    log_success "Systemd service installed. Start with: sudo systemctl start scarydb"
fi

# Install launchd service (macOS)
if [[ "$(uname -s)" == "Darwin" ]]; then
    log_info "Installing launchd service..."
    cp "$SCRIPT_DIR/com.scarydb.server.plist" /Library/LaunchDaemons/
    launchctl load /Library/LaunchDaemons/com.scarydb.server.plist
    log_success "Launchd service installed and loaded."
fi

log_success "Installation complete!"
log_info "Binary: $INSTALL_DIR/$BINARY_NAME"
log_info "Config: $INSTALL_DIR/config.json"
log_info "Data:   $INSTALL_DIR/data/"
