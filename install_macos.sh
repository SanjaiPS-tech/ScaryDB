#!/usr/bin/env bash
# =============================================================================
# ScaryDB macOS Installation Script
# Installs ScaryDB as a launchd service on macOS
# =============================================================================

set -euo pipefail

# Configuration
PROJECT_NAME="scarydb"
INSTALL_DIR="/opt/${PROJECT_NAME}"
SERVICE_LABEL="com.${PROJECT_NAME}.server"
BINARY_NAME="${PROJECT_NAME}"
CONFIG_FILE="config.json"
PLIST_FILE="${SERVICE_LABEL}.plist"

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

show_help() {
    cat << EOF
ScaryDB macOS Installation Script

Usage: sudo $0 [OPTIONS]

OPTIONS:
    -h, --help              Show this help
    -d, --dir DIR           Installation directory (default: /opt/scarydb)
    --no-service            Don't install launchd service
    --no-start              Don't start service after installation
    --force                 Overwrite existing installation

EXAMPLES:
    sudo $0                 # Standard installation
    sudo $0 --no-service    # Install binary only, no service
    sudo $0 -d /usr/local/scarydb  # Custom install directory

EOF
}

INSTALL_DIR_OVERRIDE=""
NO_SERVICE=false
NO_START=false
FORCE=false

while [[ $# -gt 0 ]]; do
    case $1 in
        -h|--help) show_help; exit 0 ;;
        -d|--dir) INSTALL_DIR_OVERRIDE="$2"; shift 2 ;;
        --no-service) NO_SERVICE=true; shift ;;
        --no-start) NO_START=true; shift ;;
        --force) FORCE=true; shift ;;
        *) log_error "Unknown option: $1"; show_help; exit 1 ;;
    esac
done

if [[ -n "$INSTALL_DIR_OVERRIDE" ]]; then
    INSTALL_DIR="$INSTALL_DIR_OVERRIDE"
fi

# Check root
if [[ $EUID -ne 0 ]]; then
    log_error "This script must be run as root (use sudo)"
    exit 1
fi

# Check macOS
if [[ "$(uname -s)" != "Darwin" ]]; then
    log_error "This script is for macOS only"
    exit 1
fi

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

log_info "ScaryDB macOS Installation"
log_info "Install directory: $INSTALL_DIR"

# Check for existing installation
if [[ -d "$INSTALL_DIR" && "$FORCE" != true ]]; then
    log_error "Installation directory exists: $INSTALL_DIR"
    log_info "Use --force to overwrite or specify different directory with -d"
    exit 1
fi

# Create install directory
log_info "Creating installation directory..."
mkdir -p "$INSTALL_DIR/data"
mkdir -p "$INSTALL_DIR/logs"

# Copy binary
if [[ -f "$SCRIPT_DIR/$BINARY_NAME" ]]; then
    log_info "Copying binary..."
    cp "$SCRIPT_DIR/$BINARY_NAME" "$INSTALL_DIR/"
    chmod +x "$INSTALL_DIR/$BINARY_NAME"
else
    log_error "Binary not found: $SCRIPT_DIR/$BINARY_NAME"
    log_info "Run build.sh first or place binary in the same directory as this script"
    exit 1
fi

# Copy config
if [[ -f "$SCRIPT_DIR/$CONFIG_FILE" ]]; then
    log_info "Copying configuration..."
    cp "$SCRIPT_DIR/$CONFIG_FILE" "$INSTALL_DIR/"
else
    log_warning "Config file not found, creating default..."
    cat > "$INSTALL_DIR/$CONFIG_FILE" << EOFCONFIG
{
  "server": {
    "workers": 1
  },
  "storage": {
    "data_dir": "./data",
    "checkpoint_interval_ops": 10000
  },
  "memory": {
    "max_memory_kb": 0
  },
  "network": {
    "host": "127.0.0.1",
    "port": 6379
  },
  "metadata": {
    "version": "0.1.0",
    "startup_time": "$(date -u +%Y-%m-%dT%H:%M:%SZ)"
  }
}
EOFCONFIG
fi

# Set permissions
log_info "Setting permissions..."
chown -R root:wheel "$INSTALL_DIR"
chmod 755 "$INSTALL_DIR"
chmod 755 "$INSTALL_DIR/data"
chmod 755 "$INSTALL_DIR/logs"

# Install launchd service
if [[ "$NO_SERVICE" != true ]]; then
    if [[ -f "$SCRIPT_DIR/$PLIST_FILE" ]]; then
        log_info "Installing launchd service from package..."
        cp "$SCRIPT_DIR/$PLIST_FILE" /Library/LaunchDaemons/
    else
        log_info "Creating launchd service..."
        cat > /Library/LaunchDaemons/${PLIST_FILE} << EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>${SERVICE_LABEL}</string>
    <key>ProgramArguments</key>
    <array>
        <string>${INSTALL_DIR}/${BINARY_NAME}</string>
        <string>server</string>
    </array>
    <key>WorkingDirectory</key>
    <string>${INSTALL_DIR}</string>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <dict>
        <key>SuccessfulExit</key>
        <false/>
        <key>Crashed</key>
        <true/>
    </dict>
    <key>StandardOutPath</key>
    <string>${INSTALL_DIR}/logs/scarydb.log</string>
    <key>StandardErrorPath</key>
    <string>${INSTALL_DIR}/logs/scarydb.error.log</string>
    <key>ProcessType</key>
    <string>Background</string>
    <key>ThrottleInterval</key>
    <integer>10</integer>
</dict>
</plist>
EOF
    fi
    
    # Set correct permissions on plist
    chown root:wheel /Library/LaunchDaemons/${PLIST_FILE}
    chmod 644 /Library/LaunchDaemons/${PLIST_FILE}
    
    log_success "Launchd service installed"
    
    if [[ "$NO_START" != true ]]; then
        log_info "Loading service..."
        launchctl load /Library/LaunchDaemons/${PLIST_FILE}
        sleep 2
        
        if launchctl list | grep -q "${SERVICE_LABEL}"; then
            log_success "Service loaded and running!"
            log_info "Check status: launchctl list | grep ${SERVICE_LABEL}"
            log_info "View logs: tail -f ${INSTALL_DIR}/logs/scarydb.log"
        else
            log_error "Service failed to start"
            log_info "Check error log: ${INSTALL_DIR}/logs/scarydb.error.log"
            exit 1
        fi
    else
        log_info "Service installed but not started (--no-start flag)"
        log_info "Start manually: sudo launchctl load /Library/LaunchDaemons/${PLIST_FILE}"
    fi
else
    log_info "Skipping service installation (--no-service flag)"
fi

# Create convenience symlink
if [[ ! -f /usr/local/bin/${PROJECT_NAME} ]]; then
    ln -sf "$INSTALL_DIR/$BINARY_NAME" /usr/local/bin/${PROJECT_NAME}
    log_success "Created symlink: /usr/local/bin/${PROJECT_NAME}"
fi

log_success "Installation complete!"
log_info ""
log_info "Installation summary:"
log_info "  Binary:     $INSTALL_DIR/$BINARY_NAME"
log_info "  Config:     $INSTALL_DIR/$CONFIG_FILE"
log_info "  Data dir:   $INSTALL_DIR/data/"
log_info "  Logs dir:   $INSTALL_DIR/logs/"
log_info "  Service:    ${SERVICE_LABEL} (launchd)"
log_info ""
log_info "Quick start:"
log_info "  ${PROJECT_NAME} standalone    # Run server + client"
log_info "  ${PROJECT_NAME} server        # Run server only"
log_info "  ${PROJECT_NAME} client        # Run client only"
log_info ""
if [[ "$NO_SERVICE" != true && "$NO_START" != true ]]; then
    log_info "Service is running. Test connection:"
    log_info "  ${PROJECT_NAME} client"
fi