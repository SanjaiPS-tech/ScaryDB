#!/usr/bin/env bash
# ScaryDB Uninstallation Script for Linux/macOS
# Run with: sudo ./uninstall.sh

set -euo pipefail

INSTALL_DIR="/opt/scarydb"
SERVICE_USER="scarydb"

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

log_info() { echo -e "${BLUE}[INFO]${NC} $*"; }
log_success() { echo -e "${GREEN}[SUCCESS]${NC} $*"; }
log_warning() { echo -e "${YELLOW}[WARNING]${NC} $*"; }
log_error() { echo -e "${RED}[ERROR]${NC} $*"; }

if [[ $EUID -ne 0 ]]; then
    log_error "This script must be run as root (use sudo)"
    exit 1
fi

log_info "Uninstalling ScaryDB..."

# Stop and disable systemd service
if command -v systemctl &>/dev/null; then
    systemctl stop scarydb 2>/dev/null || true
    systemctl disable scarydb 2>/dev/null || true
    rm -f /etc/systemd/system/scarydb.service
    systemctl daemon-reload
    log_success "Systemd service removed."
fi

# Unload launchd service (macOS)
if [[ "$(uname -s)" == "Darwin" ]]; then
    launchctl unload /Library/LaunchDaemons/com.scarydb.server.plist 2>/dev/null || true
    rm -f /Library/LaunchDaemons/com.scarydb.server.plist
    log_success "Launchd service removed."
fi

# Remove service user
if id "$SERVICE_USER" &>/dev/null; then
    userdel "$SERVICE_USER" 2>/dev/null || true
    log_success "Service user removed."
fi

# Remove installation directory
if [[ -d "$INSTALL_DIR" ]]; then
    rm -rf "$INSTALL_DIR"
    log_success "Installation directory removed."
fi

log_success "Uninstallation complete!"
