# ScaryDB Complete Setup Guide

Comprehensive installation and configuration guide for all platforms.

## Table of Contents
1. [System Requirements](#system-requirements)
2. [Linux Installation](#linux-installation)
3. [macOS Installation](#macos-installation)
4. [Windows Installation](#windows-installation)
5. [Building from Source](#building-from-source)
6. [Configuration](#configuration)
7. [Running ScaryDB](#running-scarydb)
8. [Service Management](#service-management)
9. [Uninstallation](#uninstallation)
10. [Troubleshooting](#troubleshooting)

---

## System Requirements

### Minimum Requirements
- **CPU**: x86_64 or ARM64 (1 core)
- **RAM**: 512 MB available
- **Disk**: 100 MB for binary + data
- **Network**: Loopback (127.0.0.1) for local access

### Recommended for Production
- **CPU**: 4+ cores (matches `server.workers` config)
- **RAM**: 4+ GB (depends on dataset size)
- **Disk**: SSD with 10+ GB free
- **Network**: Dedicated interface for client connections

### Supported Platforms
| OS | Architectures | Service Manager |
|----|---------------|-----------------|
| Linux | x86_64, ARM64 (glibc/musl) | systemd |
| macOS | x86_64 (Intel), ARM64 (Apple Silicon) | launchd |
| Windows 10/11 | x86_64 (MinGW/MSVC) | Windows Service |
| Windows Server 2019+ | x86_64 | Windows Service |

### Dependencies
- **Runtime**: None (statically linked where possible)
- **Build**: Rust 1.70+, cargo
- **Linux**: glibc 2.31+ or musl
- **macOS**: 11.0+ (Big Sur)
- **Windows**: Visual C++ Redistributable (MSVC builds)

---

## Linux Installation

### Method 1: Pre-built Binary (Recommended)

```bash
# Download latest release
VERSION="0.1.0"
ARCH="x86_64"  # or aarch64
wget "https://github.com/SanjaiPS-tech/ScaryDB/releases/download/v${VERSION}/scarydb-${VERSION}-${ARCH}-unknown-linux-gnu.tar.gz"

# Extract
tar -xzf "scarydb-${VERSION}-${ARCH}-unknown-linux-gnu.tar.gz"
cd "scarydb-${VERSION}-${ARCH}-unknown-linux-gnu"

# Run directly (no install)
./scarydb.sh standalone

# Or install as service
sudo ./install_linux.sh
```

### Method 2: Install Script Options

```bash
# Standard installation to /opt/scarydb
sudo ./install_linux.sh

# Custom directory
sudo ./install_linux.sh -d /usr/local/scarydb

# Binary only (no systemd service)
sudo ./install_linux.sh --no-service

# Install but don't start service
sudo ./install_linux.sh --no-start

# Force overwrite existing
sudo ./install_linux.sh --force
```

### Method 3: Package Managers (Coming Soon)

```bash
# Homebrew (Linux)
brew install sanjaips-tech/tap/scarydb

# Snap
sudo snap install scarydb

# APT (Ubuntu/Debian)
curl -fsSL https://pkg.scarydb.io/gpg.key | sudo gpg --dearmor -o /usr/share/keyrings/scarydb.gpg
echo "deb [signed-by=/usr/share/keyrings/scarydb.gpg] https://pkg.scarydb.io/apt stable main" | sudo tee /etc/apt/sources.list.d/scarydb.list
sudo apt update && sudo apt install scarydb

# DNF (Fedora/RHEL)
sudo dnf config-manager --add-repo https://pkg.scarydb.io/rpm/scarydb.repo
sudo dnf install scarydb
```

### Post-Installation Verification

```bash
# Check service status
sudo systemctl status scarydb

# View logs
sudo journalctl -u scarydb -f

# Test connection
scarydb client
# Type: PING
# Should return: BOINK! 🐷

# Check config
cat /opt/scarydb/config.json
```

---

## macOS Installation

### Method 1: Pre-built Binary (Recommended)

```bash
# Download (choose Intel or Apple Silicon)
VERSION="0.1.0"
# For Intel Macs:
ARCH="x86_64"
# For Apple Silicon (M1/M2/M3):
ARCH="aarch64"

wget "https://github.com/SanjaiPS-tech/ScaryDB/releases/download/v${VERSION}/scarydb-${VERSION}-${ARCH}-apple-darwin.tar.gz"

# Extract and run
tar -xzf "scarydb-${VERSION}-${ARCH}-apple-darwin.tar.gz"
cd "scarydb-${VERSION}-${ARCH}-apple-darwin"
./scarydb.sh standalone

# Or install as service
sudo ./install_macos.sh
```

### Method 2: Install Script Options

```bash
# Standard installation to /opt/scarydb
sudo ./install_macos.sh

# Custom directory
sudo ./install_macos.sh -d /usr/local/scarydb

# Binary only (no launchd service)
sudo ./install_macos.sh --no-service

# Install but don't start service
sudo ./install_macos.sh --no-start
```

### Method 3: Homebrew (Coming Soon)

```bash
brew tap sanjaips-tech/scarydb
brew install scarydb
```

### Post-Installation Verification

```bash
# Check service status
launchctl list | grep scarydb

# View logs
tail -f /opt/scarydb/logs/scarydb.log
tail -f /opt/scarydb/logs/scarydb.error.log

# Test connection
scarydb client
```

---

## Windows Installation

### Method 1: Pre-built Binary (Recommended)

1. Download `scarydb-<version>-x86_64-pc-windows-gnu.zip` from [Releases](https://github.com/SanjaiPS-tech/ScaryDB/releases)
2. Extract to desired location (e.g., `C:\ScaryDB`)
3. Run from extracted folder:

**PowerShell (Recommended):**
```powershell
cd C:\ScaryDB
.\scarydb.ps1 standalone
```

**Command Prompt:**
```cmd
cd C:\ScaryDB
scarydb.bat standalone
```

### Method 2: Install as Windows Service (Requires Admin)

**PowerShell (Right-click -> Run as Administrator):**
```powershell
# Standard install
.\install_windows.ps1

# Custom path
.\install_windows.ps1 -InstallPath "D:\ScaryDB"

# Binary only (no service)
.\install_windows.ps1 -NoService

# Install service but don't start
.\install_windows.ps1 -NoStart

# Force overwrite
.\install_windows.ps1 -Force
```

**Command Prompt (Run as Administrator):**
```cmd
install_windows.bat
install_windows.bat --dir D:\ScaryDB
install_windows.bat --no-service
install_windows.bat --force
```

### Method 3: Chocolatey (Coming Soon)

```powershell
choco install scarydb
```

### Method 4: Scoop (Coming Soon)

```powershell
scoop bucket add scarydb https://github.com/SanjaiPS-tech/scoop-scarydb
scoop install scarydb
```

### Post-Installation Verification

```powershell
# Check service status
Get-Service ScaryDB

# View logs in Event Viewer
Get-WinEvent -LogName Application -ProviderName ScaryDB | Select -First 20

# Test connection
scarydb client
```

---

## Building from Source

### Prerequisites

```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env

# Verify
rustc --version  # Should be 1.70+
cargo --version
```

### Linux/macOS Build

```bash
git clone https://github.com/SanjaiPS-tech/ScaryDB.git
cd ScaryDB

# Quick build (release mode)
./build.sh

# Debug build
./build.sh -m debug

# Build for specific target (cross-compilation)
./build.sh -t x86_64-unknown-linux-musl
./build.sh -t aarch64-apple-darwin

# Build and create distribution package
./build.sh --package

# Clean build
./build.sh --clean -m release
```

### Windows Build

**PowerShell:**
```powershell
git clone https://github.com/SanjaiPS-tech/ScaryDB.git
cd ScaryDB

# Build using Cargo directly
cargo build --release --bin scarydb --bin benchmark

# Or use build script via WSL/Git Bash
bash build.sh
bash build.sh --package
```

### Cross-Compilation Targets

| Target | Platform | Notes |
|--------|----------|-------|
| `x86_64-unknown-linux-gnu` | Linux x86_64 (glibc) | Default on Linux |
| `aarch64-unknown-linux-gnu` | Linux ARM64 (glibc) | Requires cross toolchain |
| `x86_64-unknown-linux-musl` | Linux x86_64 (musl) | Fully static |
| `aarch64-unknown-linux-musl` | Linux ARM64 (musl) | Fully static |
| `x86_64-apple-darwin` | macOS Intel | Build on macOS |
| `aarch64-apple-darwin` | macOS Apple Silicon | Build on macOS |
| `x86_64-pc-windows-gnu` | Windows (MinGW) | Cross from Linux |
| `x86_64-pc-windows-msvc` | Windows (MSVC) | Build on Windows |

### Build Artifacts

After building:
```
target/
├── release/
│   ├── scarydb          # Main server binary
│   └── benchmark        # Benchmark tool
└── <target>/release/
    └── scarydb(.exe)    # Cross-compiled binaries
```

---

## Configuration

### config.json Structure

```json
{
  "server": {
    "workers": 4
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
    "startup_time": "2026-01-15T10:30:00Z"
  }
}
```

### Configuration Options

| Section | Property | Default | Description |
|---------|----------|---------|-------------|
| server | workers | 1 | Thread pool workers (match CPU cores) |
| storage | data_dir | "./data" | Data directory (relative to binary or absolute) |
| storage | checkpoint_interval_ops | 10000 | Ops between JSON checkpoints |
| memory | max_memory_kb | 0 | Memory limit (0 = unlimited) |
| network | host | "127.0.0.1" | Bind address (0.0.0.0 for all interfaces) |
| network | port | 6379 | TCP port |

### Runtime Configuration Changes

Connect via client and use:
```sql
-- View all config
LIST CONFIG;

-- Get specific value
GET CONFIG storage.checkpoint_interval_ops;

-- Set value (persists to config.json)
SET CONFIG storage.checkpoint_interval_ops 5000;
SET CONFIG server.workers 8;
SET CONFIG network.host 0.0.0.0;
SET CONFIG network.port 6380;
```

### Environment Variable Overrides

```bash
# Override config path
SCARYDB_CONFIG_PATH=/etc/scarydb/config.json scarydb server

# Override port (for testing)
SCARYDB_TEST_PORT=6380 scarydb server

# Override data directory
SCARYDB_TEST_DATA_DIR=/var/lib/scarydb scarydb server
```

---

## Running ScaryDB

### Mode 1: Standalone (Development/Testing)

Runs server + client in one process.

```bash
# Linux/macOS
./scarydb.sh standalone

# Windows PowerShell
.\scarydb.ps1 standalone

# Windows CMD
scarydb.bat standalone

# Direct binary
./scarydb standalone
```

### Mode 2: Server Only (Production)

```bash
# Linux/macOS
./scarydb.sh server
# or
./scarydb server

# Windows
.\scarydb.ps1 server
scarydb.bat server
```

### Mode 3: Client Only

Connects to running server.

```bash
# Linux/macOS
./scarydb.sh client
./scarydb client

# Windows
.\scarydb.ps1 client
scarydb.bat client
```

### Mode 4: Log Reader

Reads binary WAL log.

```bash
./scarydb.sh log-read ./data/operations.log
./scarydb log-read ./data/operations.log
```

### Running in Background (Linux/macOS)

```bash
# Using nohup
nohup ./scarydb server > scarydb.log 2>&1 &

# Using systemd (if installed)
sudo systemctl start scarydb

# Using launchd (if installed)
sudo launchctl load /Library/LaunchDaemons/com.scarydb.server.plist
```

### Running in Background (Windows)

```powershell
# As service (if installed)
Start-Service ScaryDB

# Direct background
Start-Process -FilePath "scarydb.exe" -ArgumentList "server" -WindowStyle Hidden
```

---

## Service Management

### Linux (systemd)

```bash
# Start
sudo systemctl start scarydb

# Stop
sudo systemctl stop scarydb

# Restart
sudo systemctl restart scarydb

# Enable auto-start
sudo systemctl enable scarydb

# Disable auto-start
sudo systemctl disable scarydb

# Status
sudo systemctl status scarydb

# Logs
sudo journalctl -u scarydb -f
sudo journalctl -u scarydb --since "1 hour ago"

# Reload config after changes
sudo systemctl reload scarydb
```

### macOS (launchd)

```bash
# Start
sudo launchctl load /Library/LaunchDaemons/com.scarydb.server.plist

# Stop
sudo launchctl unload /Library/LaunchDaemons/com.scarydb.server.plist

# Status
launchctl list | grep scarydb

# Logs
tail -f /opt/scarydb/logs/scarydb.log
tail -f /opt/scarydb/logs/scarydb.error.log
```

### Windows (Services)

```powershell
# Start
Start-Service ScaryDB
net start ScaryDB

# Stop
Stop-Service ScaryDB
net stop ScaryDB

# Restart
Restart-Service ScaryDB

# Status
Get-Service ScaryDB

# Set startup type
Set-Service ScaryDB -StartupType Automatic
Set-Service ScaryDB -StartupType Manual
Set-Service ScaryDB -StartupType Disabled

# Logs
Get-WinEvent -LogName Application -ProviderName ScaryDB | Select -First 50
```

---

## Uninstallation

### Linux

```bash
# Using uninstall script
sudo ./uninstall.sh

# Manual
sudo systemctl stop scarydb
sudo systemctl disable scarydb
sudo rm /etc/systemd/system/scarydb.service
sudo systemctl daemon-reload
sudo userdel scarydb 2>/dev/null || true
sudo rm -rf /opt/scarydb
sudo rm /usr/local/bin/scarydb
```

### macOS

```bash
# Using uninstall script
sudo ./uninstall.sh

# Manual
sudo launchctl unload /Library/LaunchDaemons/com.scarydb.server.plist
sudo rm /Library/LaunchDaemons/com.scarydb.server.plist
sudo rm -rf /opt/scarydb
sudo rm /usr/local/bin/scarydb
```

### Windows

**PowerShell (Admin):**
```powershell
# Using uninstall script
.\Uninstall-ScaryDBService.ps1

# Manual
Stop-Service ScaryDB -Force
sc.exe delete ScaryDB
Remove-Item -Path "C:\Program Files\ScaryDB" -Recurse -Force
# Remove from PATH manually if needed
```

**Command Prompt (Admin):**
```cmd
Uninstall-ScaryDBService.ps1
REM Or manually:
net stop ScaryDB
sc delete ScaryDB
rmdir /s /q "C:\Program Files\ScaryDB"
```

---

## Troubleshooting

### Common Issues

#### Port Already in Use
```
Error: Failed to bind to TCP address 127.0.0.1:6379: Address already in use
```
**Solution:**
```bash
# Find and kill process
lsof -i :6379
kill -9 <PID>

# Or change port
SET CONFIG network.port 6380;
```

#### Permission Denied (Linux/macOS)
```
Error: Failed to create config parent directories: Permission denied
```
**Solution:**
```bash
# Run installer with sudo
sudo ./install_linux.sh

# Or fix permissions
chmod +x scarydb.sh scarydb
```

#### Binary Not Found After Build
```
Error: Binary not found: target/release/scarydb
```
**Solution:**
```bash
# Clean and rebuild
./build.sh --clean -m release

# Check target directory
ls -la target/release/
```

#### Windows Service Won't Start
```
Service 'ScaryDB' failed to start
```
**Solution:**
```powershell
# Check Event Viewer
Get-WinEvent -LogName Application -ProviderName ScaryDB | Select -First 20

# Check binary runs directly
cd "C:\Program Files\ScaryDB"
.\scarydb.exe server

# Common: Missing VC++ Redistributable (MSVC builds)
# Install: https://aka.ms/vs/17/release/vc_redist.x64.exe
```

#### Connection Refused
```
Error: Connection failed: Connection refused
```
**Solution:**
```bash
# Check server is running
ps aux | grep scarydb
# or
sudo systemctl status scarydb

# Check port
netstat -tlnp | grep 6379
ss -tlnp | grep 6379

# Check firewall
sudo ufw status
sudo firewall-cmd --list-all
```

#### High Memory Usage
```
Warning: Memory usage exceeds limit
```
**Solution:**
```bash
# Set memory limit in config.json
SET CONFIG memory.max_memory_kb 2097152  # 2GB

# Or reduce checkpoint interval
SET CONFIG storage.checkpoint_interval_ops 5000
```

### Debug Mode

```bash
# Enable debug logging
RUST_LOG=debug scarydb server

# Specific modules
RUST_LOG=scarydb=debug,worker=info scarydb server

# Run client with timing
scarydb client  # Shows (X ms) after each command in standalone
```

### Performance Tuning

```bash
# Increase workers for multi-core
SET CONFIG server.workers 8

# Tune checkpoint interval (higher = less disk I/O, more WAL)
SET CONFIG storage.checkpoint_interval_ops 50000

# Use 0.0.0.0 for external access
SET CONFIG network.host 0.0.0.0

# Set memory limit to prevent OOM
SET CONFIG memory.max_memory_kb 4194304  # 4GB
```

### Getting Help

- **GitHub Issues**: https://github.com/SanjaiPS-tech/ScaryDB/issues
- **Documentation**: https://github.com/SanjaiPS-tech/ScaryDB#readme
- **Command Help**: Type `HELP` in client

---

## Security Considerations

1. **Network Binding**: Default binds to 127.0.0.1 only. Use `0.0.0.0` with firewall rules.
2. **Authentication**: Not implemented yet. Use network isolation (VPN, firewall).
3. **File Permissions**: Data directory should be owned by service user only.
4. **Updates**: Regularly update from GitHub releases for security patches.

---

## Next Steps

- Read [QUICKSTART.md](QUICKSTART.md) for 5-minute start
- Read [README.md](README.md) for full command reference
- Check [Performance Guide](docs/performance.md) for tuning
- Join [Discussions](https://github.com/SanjaiPS-tech/ScaryDB/discussions) for help