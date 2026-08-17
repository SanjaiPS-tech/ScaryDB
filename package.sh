#!/usr/bin/env bash
# =============================================================================
# ScaryDB Package Script
# Creates distribution packages for Linux, macOS, and Windows
# =============================================================================

set -euo pipefail

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

PROJECT_NAME="scarydb"
VERSION="${VERSION:-$(grep '^version' Cargo.toml | head -1 | sed 's/.*= *"//;s/".*//')}"
BUILD_MODE="${BUILD_MODE:-release}"
TARGET="${TARGET:-$(rustc -vV | grep host | cut -d' ' -f2)}"
BINARY_NAME="${PROJECT_NAME}"

# Platform-specific binary name
case "$TARGET" in
    *windows*) BINARY_NAME="${PROJECT_NAME}.exe" ;;
esac

BINARY_PATH="target/${TARGET}/${BUILD_MODE}/${BINARY_NAME}"
DIST_DIR="dist/${PROJECT_NAME}-${VERSION}-${TARGET}"

log_info() { echo -e "${BLUE}[INFO]${NC} $*"; }
log_success() { echo -e "${GREEN}[SUCCESS]${NC} $*"; }
log_warning() { echo -e "${YELLOW}[WARNING]${NC} $*"; }
log_error() { echo -e "${RED}[ERROR]${NC} $*"; }

show_help() {
    cat << EOF
ScaryDB Package Script v${VERSION}

Usage: $0 [OPTIONS]

OPTIONS:
    -h, --help              Show this help
    -t, --target TARGET     Rust target triple (default: host target)
    -m, --mode MODE         Build mode: debug|release (default: release)
    -o, --output DIR        Output directory (default: dist/)
    --no-compress           Don't create compressed archive

EXAMPLES:
    $0                      # Package for current platform
    $0 -t x86_64-unknown-linux-gnu  # Package for Linux
    $0 --no-compress        # Create directory only, no archive

EOF
}

COMPRESS=true
OUTPUT_DIR="dist"

while [[ $# -gt 0 ]]; do
    case $1 in
        -h|--help) show_help; exit 0 ;;
        -t|--target) TARGET="$2"; shift 2 ;;
        -m|--mode) BUILD_MODE="$2"; shift 2 ;;
        -o|--output) OUTPUT_DIR="$2"; shift 2 ;;
        --no-compress) COMPRESS=false; shift ;;
        *) log_error "Unknown option: $1"; show_help; exit 1 ;;
    esac
done

# Update paths based on target
case "$TARGET" in
    *windows*) BINARY_NAME="${PROJECT_NAME}.exe" ;;
    *) BINARY_NAME="${PROJECT_NAME}" ;;
esac

BINARY_PATH="target/${TARGET}/${BUILD_MODE}/${BINARY_NAME}"
DIST_DIR="${OUTPUT_DIR}/${PROJECT_NAME}-${VERSION}-${TARGET}"

log_info "Packaging ScaryDB v${VERSION} for ${TARGET}"

# Verify binary exists
if [[ ! -f "$BINARY_PATH" ]]; then
    log_error "Binary not found: $BINARY_PATH"
    log_info "Run ./build.sh first to build the project"
    exit 1
fi

# Clean and create dist directory
rm -rf "$DIST_DIR"
mkdir -p "$DIST_DIR"

# Copy binary
log_info "Copying binary..."
cp "$BINARY_PATH" "$DIST_DIR/"

# Copy config template
log_info "Copying configuration template..."
cat > "$DIST_DIR/config.json" << 'EOFCONFIG'
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
    "version": "VERSION_PLACEHOLDER",
    "startup_time": "RUNTIME_PLACEHOLDER"
  }
}
EOFCONFIG

# Replace version placeholder
sed -i.bak "s/VERSION_PLACEHOLDER/${VERSION}/" "$DIST_DIR/config.json" && rm -f "$DIST_DIR/config.json.bak"

# Copy README
if [[ -f "README.md" ]]; then
    cp README.md "$DIST_DIR/"
fi

# Copy CHANGELOG
if [[ -f "CHANGELOG.md" ]]; then
    cp CHANGELOG.md "$DIST_DIR/"
fi

# Copy LICENSE
if [[ -f "LICENSE" ]]; then
    cp LICENSE "$DIST_DIR/"
fi

# Create platform-specific launch scripts
log_info "Creating launch scripts..."

# Linux/macOS launch script
cat > "$DIST_DIR/scarydb.sh" << 'EOF'
#!/usr/bin/env bash
# ScaryDB Launcher for Linux/macOS

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

# Ensure data directory exists
mkdir -p data

# Detect binary name
BINARY="./scarydb"
if [[ ! -f "$BINARY" ]]; then
    BINARY="./scarydb.exe"
fi

if [[ ! -f "$BINARY" ]]; then
    echo "Error: ScaryDB binary not found in $SCRIPT_DIR"
    exit 1
fi

# Make binary executable
chmod +x "$BINARY"

# Parse arguments
MODE="${1:-standalone}"

case "$MODE" in
    standalone|--standalone)
        echo "Starting ScaryDB in standalone mode (server + client)..."
        exec "$BINARY" standalone
        ;;
    server|--server)
        echo "Starting ScaryDB server..."
        exec "$BINARY" server
        ;;
    client|--client)
        echo "Starting ScaryDB client..."
        exec "$BINARY" client
        ;;
    log-read|--log-read)
        if [[ -z "${2:-}" ]]; then
            echo "Usage: $0 log-read <path_to_operations.log>"
            exit 1
        fi
        exec "$BINARY" log-read "$2"
        ;;
    *)
        echo "Usage: $0 [standalone|server|client|log-read <path>]"
        echo ""
        echo "Modes:"
        echo "  standalone  - Run server and client together (default)"
        echo "  server      - Run only the database server"
        echo "  client      - Run only the REPL client"
        echo "  log-read    - Read binary WAL log file"
        exit 1
        ;;
esac
EOF

chmod +x "$DIST_DIR/scarydb.sh"

# Windows launch script (batch)
cat > "$DIST_DIR/scarydb.bat" << 'EOF'
@echo off
REM ScaryDB Launcher for Windows

set SCRIPT_DIR=%~dp0
cd /d "%SCRIPT_DIR%"

REM Ensure data directory exists
if not exist data mkdir data

set BINARY=scarydb.exe
if not exist %BINARY% (
    echo Error: ScaryDB binary not found in %SCRIPT_DIR%
    exit /b 1
)

set MODE=%1
if "%MODE%"=="" set MODE=standalone

if /i "%MODE%"=="standalone" (
    echo Starting ScaryDB in standalone mode (server + client)...
    %BINARY% standalone
) else if /i "%MODE%"=="server" (
    echo Starting ScaryDB server...
    %BINARY% server
) else if /i "%MODE%"=="client" (
    echo Starting ScaryDB client...
    %BINARY% client
) else if /i "%MODE%"=="log-read" (
    if "%~2"=="" (
        echo Usage: %0 log-read ^<path_to_operations.log^>
        exit /b 1
    )
    %BINARY% log-read %2
) else (
    echo Usage: %0 [standalone^|server^|client^|log-read ^<path^>]
    echo.
    echo Modes:
    echo   standalone  - Run server and client together (default)
    echo   server      - Run only the database server
    echo   client      - Run only the REPL client
    echo   log-read    - Read binary WAL log file
    exit /b 1
)
EOF

# Windows launch script (PowerShell)
cat > "$DIST_DIR/scarydb.ps1" << 'EOF'
# ScaryDB Launcher for Windows (PowerShell)

$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Definition
Set-Location $ScriptDir

# Ensure data directory exists
if (-not (Test-Path "data")) {
    New-Item -ItemType Directory -Path "data" | Out-Null
}

$Binary = "scarydb.exe"
if (-not (Test-Path $Binary)) {
    Write-Error "ScaryDB binary not found in $ScriptDir"
    exit 1
}

$Mode = $args[0] ?? "standalone"

switch -Wildcard ($Mode.ToLower()) {
    "standalone" {
        Write-Host "Starting ScaryDB in standalone mode (server + client)..."
        & $Binary standalone
    }
    "server" {
        Write-Host "Starting ScaryDB server..."
        & $Binary server
    }
    "client" {
        Write-Host "Starting ScaryDB client..."
        & $Binary client
    }
    "log-read" {
        if ($args.Count -lt 2) {
            Write-Host "Usage: .\scarydb.ps1 log-read <path_to_operations.log>"
            exit 1
        }
        & $Binary log-read $args[1]
    }
    default {
        Write-Host "Usage: .\scarydb.ps1 [standalone|server|client|log-read <path>]"
        Write-Host ""
        Write-Host "Modes:"
        Write-Host "  standalone  - Run server and client together (default)"
        Write-Host "  server      - Run only the database server"
        Write-Host "  client      - Run only the REPL client"
        Write-Host "  log-read    - Read binary WAL log file"
        exit 1
    }
}
EOF

# Create systemd service file for Linux
cat > "$DIST_DIR/scarydb.service" << 'EOF'
[Unit]
Description=ScaryDB High-Performance In-Memory Database
After=network.target
Wants=network.target

[Service]
Type=simple
User=scarydb
Group=scarydb
WorkingDirectory=/opt/scarydb
ExecStart=/opt/scarydb/scarydb server
Restart=on-failure
RestartSec=5
StandardOutput=journal
StandardError=journal
SyslogIdentifier=scarydb

# Security hardening
NoNewPrivileges=true
PrivateTmp=true
ProtectSystem=strict
ProtectHome=true
ReadWritePaths=/opt/scarydb/data
ProtectKernelTunables=true
ProtectKernelModules=true
ProtectControlGroups=true

# Resource limits
LimitNOFILE=65536
LimitNPROC=4096

[Install]
WantedBy=multi-user.target
EOF

# Create launchd plist for macOS
cat > "$DIST_DIR/com.scarydb.server.plist" << 'EOF'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>com.scarydb.server</string>
    <key>ProgramArguments</key>
    <array>
        <string>/opt/scarydb/scarydb</string>
        <string>server</string>
    </array>
    <key>WorkingDirectory</key>
    <string>/opt/scarydb</string>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <true/>
    <key>StandardOutPath</key>
    <string>/var/log/scarydb.log</string>
    <key>StandardErrorPath</key>
    <string>/var/log/scarydb.error.log</string>
    <key>ProcessType</key>
    <string>Background</string>
</dict>
</plist>
EOF

# Create Windows service install script (PowerShell)
cat > "$DIST_DIR/Install-ScaryDBService.ps1" << 'EOF'
<#
.SYNOPSIS
    Installs ScaryDB as a Windows Service

.DESCRIPTION
    Creates and starts a Windows Service for ScaryDB server mode.
    Requires Administrator privileges.

.NOTES
    Run as Administrator: PowerShell -ExecutionPolicy Bypass -File Install-ScaryDBService.ps1
#>

param (
    [string]$InstallPath = "C:\Program Files\ScaryDB",
    [string]$ServiceName = "ScaryDB",
    [string]$DisplayName = "ScaryDB Database Server",
    [string]$Description = "High-performance in-memory hierarchical database"
)

# Check for admin rights
if (-not ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
    Write-Error "This script requires Administrator privileges. Please run as Administrator."
    exit 1
}

# Create installation directory
if (-not (Test-Path $InstallPath)) {
    New-Item -ItemType Directory -Path $InstallPath -Force | Out-Null
}

# Copy files (assumes running from package directory)
$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Definition
Copy-Item "$ScriptDir\scarydb.exe" "$InstallPath\" -Force
Copy-Item "$ScriptDir\config.json" "$InstallPath\" -Force
if (-not (Test-Path "$InstallPath\data")) {
    New-Item -ItemType Directory -Path "$InstallPath\data" | Out-Null
}

# Create service
$ServiceParams = @{
    Name        = $ServiceName
    DisplayName = $DisplayName
    Description = $Description
    BinaryPathName = "$InstallPath\scarydb.exe server"
    StartupType = 'Automatic'
    Credential  = (Get-Credential -Message "Enter service account credentials (or cancel for LocalSystem)")
}

try {
    New-Service @ServiceParams -ErrorAction Stop
    Write-Host "Service '$ServiceName' created successfully." -ForegroundColor Green
    
    # Start service
    Start-Service -Name $ServiceName -ErrorAction Stop
    Write-Host "Service '$ServiceName' started successfully." -ForegroundColor Green
} catch {
    Write-Error "Failed to create/start service: $_"
    exit 1
}
EOF

# Create uninstall script for Windows
cat > "$DIST_DIR/Uninstall-ScaryDBService.ps1" << 'EOF'
<#
.SYNOPSIS
    Uninstalls ScaryDB Windows Service

.NOTES
    Run as Administrator
#>

param (
    [string]$ServiceName = "ScaryDB",
    [string]$InstallPath = "C:\Program Files\ScaryDB"
)

if (-not ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
    Write-Error "This script requires Administrator privileges."
    exit 1
}

# Stop and remove service
if (Get-Service -Name $ServiceName -ErrorAction SilentlyContinue) {
    Stop-Service -Name $ServiceName -Force -ErrorAction SilentlyContinue
    sc.exe delete $ServiceName
    Write-Host "Service '$ServiceName' removed." -ForegroundColor Green
}

# Remove installation directory
if (Test-Path $InstallPath) {
    Remove-Item -Path $InstallPath -Recurse -Force
    Write-Host "Installation directory removed." -ForegroundColor Green
}
EOF

# Create install script for Linux/macOS
cat > "$DIST_DIR/install.sh" << 'EOF'
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
EOF

chmod +x "$DIST_DIR/install.sh"

# Create uninstall script for Linux/macOS
cat > "$DIST_DIR/uninstall.sh" << 'EOF'
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
EOF

chmod +x "$DIST_DIR/uninstall.sh"

# Create archive if requested
if [[ "$COMPRESS" == true ]]; then
    log_info "Creating compressed archive..."
    cd "$OUTPUT_DIR"
    
    case "$TARGET" in
        *windows*)
            if command -v zip &>/dev/null; then
                zip -r "${PROJECT_NAME}-${VERSION}-${TARGET}.zip" "${PROJECT_NAME}-${VERSION}-${TARGET}"
                log_success "Created: ${OUTPUT_DIR}/${PROJECT_NAME}-${VERSION}-${TARGET}.zip"
            else
                log_warning "zip not found, skipping archive creation"
            fi
            ;;
        *)
            tar -czf "${PROJECT_NAME}-${VERSION}-${TARGET}.tar.gz" "${PROJECT_NAME}-${VERSION}-${TARGET}"
            log_success "Created: ${OUTPUT_DIR}/${PROJECT_NAME}-${VERSION}-${TARGET}.tar.gz"
            ;;
    esac
    
    cd - >/dev/null
fi

log_success "Packaging complete!"
log_info "Distribution directory: $DIST_DIR"
log_info "Contents:"
ls -la "$DIST_DIR"