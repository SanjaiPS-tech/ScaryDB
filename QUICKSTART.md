# ScaryDB Quick Start Guide

Get up and running with ScaryDB in under 5 minutes.

## 🚀 Quick Start (All Platforms)

### Option 1: Download Pre-built Binary (Recommended)

1. Go to [Releases](https://github.com/SanjaiPS-tech/ScaryDB/releases)
2. Download the package for your platform:
   - **Linux**: `scarydb-<version>-x86_64-unknown-linux-gnu.tar.gz` (or arm64)
   - **macOS**: `scarydb-<version>-x86_64-apple-darwin.tar.gz` (Intel) or `scarydb-<version>-aarch64-apple-darwin.tar.gz` (Apple Silicon)
   - **Windows**: `scarydb-<version>-x86_64-pc-windows-gnu.zip`

3. Extract and run:
   ```bash
   # Linux/macOS
   tar -xzf scarydb-*.tar.gz
   cd scarydb-*
   ./scarydb.sh standalone
   
   # Windows (PowerShell)
   Expand-Archive scarydb-*.zip
   cd scarydb-*
   .\scarydb.ps1 standalone
   
   # Windows (Command Prompt)
   scarydb.bat standalone
   ```

### Option 2: Build from Source

Requires Rust 1.70+.

```bash
# Clone and build
git clone https://github.com/SanjaiPS-tech/ScaryDB.git
cd ScaryDB
./build.sh --package

# Run standalone mode (server + client)
./scarydb.sh standalone
```

### Option 3: Install as System Service

**Linux:**
```bash
sudo ./install_linux.sh
# Service auto-starts. Connect with:
scarydb client
```

**macOS:**
```bash
sudo ./install_macos.sh
scarydb client
```

**Windows (Admin PowerShell):**
```powershell
.\install_windows.ps1
scarydb client
```

---

## 🎮 Using ScaryDB

Once running in standalone mode or connected via client:

```sql
-- Create a database
CREATE DB myapp;

-- Switch to it
USE myapp;

-- Create a bucket (namespace)
CREATE BUCKET users;

-- Set key-value pairs (auto-detects types)
SET users user1 "Alice" / user2 [INT] 42 / user3 [BOOL] true;

-- Get values
GET users user1 / user2 / user3;

-- List all keys
LIST users;

-- Check existence
EXISTS users user1;

-- Delete keys
DEL users user1;

-- System commands
PING        -- Returns "BOINK! 🐷"
INFO        -- Server info
STATS       -- Database stats
VERSION     -- Version info
HELP        -- Show all commands
EXIT        -- Quit client
```

---

## 📦 Distribution Packages

| Platform | Archive | Installer | Service Manager |
|----------|---------|-----------|-----------------|
| Linux x86_64 | `.tar.gz` | `install_linux.sh` | systemd |
| Linux ARM64 | `.tar.gz` | `install_linux.sh` | systemd |
| macOS Intel | `.tar.gz` | `install_macos.sh` | launchd |
| macOS Apple Silicon | `.tar.gz` | `install_macos.sh` | launchd |
| Windows (MinGW) | `.zip` | `install_windows.ps1/.bat` | Windows Service |
| Windows (MSVC) | `.zip` | `install_windows.ps1/.bat` | Windows Service |

---

## 🔧 Configuration

Edit `config.json`:

```json
{
  "server": { "workers": 4 },
  "storage": {
    "data_dir": "./data",
    "checkpoint_interval_ops": 10000
  },
  "memory": { "max_memory_kb": 0 },
  "network": { "host": "127.0.0.1", "port": 6379 }
}
```

Or change at runtime:
```sql
SET CONFIG storage.checkpoint_interval_ops 5000;
SET CONFIG server.workers 4;
LIST CONFIG;
```

---

## 🐳 Docker (Coming Soon)

```bash
docker run -d -p 6379:6379 -v scarydb-data:/opt/scarydb/data sanjaips/scarydb:latest
```

---

## 📚 Documentation

- [Full README](README.md) - Complete command reference
- [Architecture](docs/architecture.md) - Internal design
- [Performance](docs/performance.md) - Benchmarks & tuning
- [API Reference](docs/api.md) - TCP protocol spec

---

## 🆘 Troubleshooting

**Port already in use:**
```bash
# Change port in config.json or:
SET CONFIG network.port 6380;
```

**Permission denied (Linux/macOS):**
```bash
chmod +x scarydb.sh scarydb
```

**Binary not found after build:**
```bash
./build.sh --clean
```

**Windows service won't start:**
```powershell
# Check Event Viewer -> Windows Logs -> Application
Get-WinEvent -LogName Application -ProviderName ScaryDB | Select -First 20
```

---

## 🤝 Contributing

1. Fork the repo
2. Create feature branch
3. Run tests: `cargo test`
4. Submit PR

---

## 📄 License

MIT OR Apache-2.0