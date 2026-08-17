#!/usr/bin/env bash
# =============================================================================
# ScaryDB Linux Installation Script
# Installs ScaryDB as a system service on Linux
# =============================================================================

set -euo pipefail

# Configuration
PROJECT_NAME="scarydb"
INSTALL_DIR="/opt/${PROJECT_NAME}"
SERVICE_USER="${PROJECT_NAME}"
SERVICE_GROUP="${PROJECT_NAME}"
BINARY_NAME="${PROJECT_NAME}"
CONFIG_FILE="config.json"
SERVICE_FILE="${PROJECT_NAME}.service"

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

# Help
show_help() {
    cat << EOF
ScaryDB Linux Installation Script

Usage: sudo $0 [OPTIONS]

OPTIONS:
    -h, --help              Show this help
    -d, --dir DIR           Installation directory (default: /opt/scarydb)
    -u, --user USER         Service user (default: scarydb)
    --no-service            Don't install systemd service
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
        -u|--user) SERVICE_USER="$2"; shift 2 ;;
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

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

log_info "ScaryDB Linux Installation"
log_info "Install directory: $INSTALL_DIR"
log_info "Service user: $SERVICE_USER"

# Check for existing installation
if [[ -d "$INSTALL_DIR" && "$FORCE" != true ]]; then
    log_error "Installation directory exists: $INSTALL_DIR"
    log_info "Use --force to overwrite or specify different directory with -d"
    exit 1
fi

# Create service user
if ! id "$SERVICE_USER" &>/dev/null; then
    log_info "Creating service user: $SERVICE_USER"
    useradd --system --no-create-home --shell /bin/false --comment "ScaryDB Database Server" "$SERVICE_USER"
    log_success "Service user created"
else
    log_info "Service user already exists: $SERVICE_USER"
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

# Set ownership
log_info "Setting permissions..."
chown -R "$SERVICE_USER:$SERVICE_GROUP" "$INSTALL_DIR"

# Install systemd service
if [[ "$NO_SERVICE" != true ]]; then
    if command -v systemctl &>/dev/null; then
        if [[ -f "$SCRIPT_DIR/$SERVICE_FILE" ]]; then
            log_info "Installing systemd service..."
            cp "$SCRIPT_DIR/$SERVICE_FILE" /etc/systemd/system/
        else
            log_info "Creating systemd service..."
            cat > /etc/systemd/system/${PROJECT_NAME}.service << EOF
[Unit]
Description=ScaryDB High-Performance In-Memory Database
Documentation=https://github.com/yourusername/scarydb
After=network.target
Wants=network.target

[Service]
Type=simple
User=${SERVICE_USER}
Group=${SERVICE_GROUP}
WorkingDirectory=${INSTALL_DIR}
ExecStart=${INSTALL_DIR}/${BINARY_NAME} server
Restart=on-failure
RestartSec=5
StandardOutput=journal
StandardError=journal
SyslogIdentifier=${PROJECT_NAME}

# Security hardening
NoNewPrivileges=true
PrivateTmp=true
ProtectSystem=strict
ProtectHome=true
ReadWritePaths=${INSTALL_DIR}/data ${INSTALL_DIR}/logs
ProtectKernelTunables=true
ProtectKernelModules=true
ProtectControlGroups=true

# Resource limits
LimitNOFILE=65536
LimitNPROC=4096

[Install]
WantedBy=multi-user.target
EOF
        fi
        
        systemctl daemon-reload
        systemctl enable ${PROJECT_NAME}
        log_success "Systemd service installed and enabled"
        
        if [[ "$NO_START" != true ]]; then
            log_info "Starting service..."
            systemctl start ${PROJECT_NAME}
            sleep 2
            if systemctl is-active --quiet ${PROJECT_NAME}; then
                log_success "Service started successfully!"
                log_info "Check status: sudo systemctl status ${PROJECT_NAME}"
                log_info "View logs: sudo journalctl -u ${PROJECT_NAME} -f"
            else
                log_error "Service failed to start"
                systemctl status ${PROJECT_NAME}
                exit 1
            fi
        else
            log_info "Service installed but not started (--no-start flag)"
            log_info "Start manually: sudo systemctl start ${PROJECT_NAME}"
        fi
    else
        log_warning "systemd not found, skipping service installation"
        log_info "You can run ScaryDB manually: $INSTALL_DIR/$BINARY_NAME server"
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
log_info "  Service:    ${PROJECT_NAME} (systemd)"
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