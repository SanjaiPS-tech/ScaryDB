# ScaryDB 🎃

A high-performance, in-memory, actor-based hierarchical database written in Rust. It utilizes a nested key-value store structured as `Database → Bucket → Key → Value` with automatic/explicit value types, an internal ID catalog mapping, a TCP server-client architecture with a thread pool/request queue concurrency model, and dual-path binary logging/JSON checkpoints persistence.

---

## 🚀 Quick Start

### Option 1: Download Pre-built Binary (Recommended)

1. Go to [Releases](https://github.com/SanjaiPS-tech/ScaryDB/releases)
2. Download for your platform:
   - **Linux**: `scarydb-<version>-x86_64-unknown-linux-gnu.tar.gz` (or ARM64)
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
```

### Option 2: Package Managers

```bash
# Homebrew (macOS/Linux)
brew tap SanjaiPS-tech/scarydb
brew install scarydb

# Chocolatey (Windows)
choco install scarydb

# APT (Debian/Ubuntu)
curl -fsSL https://apt.scarydb.io/scarydb.gpg | sudo gpg --dearmor -o /usr/share/keyrings/scarydb-archive-keyring.gpg
echo "deb [arch=$(dpkg --print-architecture) signed-by=/usr/share/keyrings/scarydb-archive-keyring.gpg] https://apt.scarydb.io stable main" | sudo tee /etc/apt/sources.list.d/scarydb.list
sudo apt update && sudo apt install scarydb

# DNF (Fedora/RHEL)
sudo rpm --import https://dnf.scarydb.io/scarydb.gpg
sudo dnf config-manager --add-repo https://dnf.scarydb.io/stable/x86_64/
sudo dnf install scarydb
```

### Option 3: Install as System Service

```bash
# Linux (systemd)
sudo ./install_linux.sh

# macOS (launchd)
sudo ./install_macos.sh

# Windows (Admin PowerShell)
.\install_windows.ps1
```

Then connect:
```bash
scarydb client
```

### Option 4: Build from Source

Requires Rust 1.70+.

```bash
git clone https://github.com/SanjaiPS-tech/ScaryDB.git
cd ScaryDB
./build.sh --package

# Run standalone mode (server + client)
./scarydb.sh standalone
```

---

## 🐳 Docker (Multi-arch)

```bash
# Run with Docker
docker run -d -p 6379:6379 -v scarydb-data:/opt/scarydb/data sanjaips/scarydb:latest

# Or build locally
docker buildx build --platform linux/amd64,linux/arm64 -t scarydb:local .
```

---

## 🎮 Running Modes

| Mode | Description | Command |
|------|-------------|---------|
| **Standalone** | Server + REPL client together (dev) | `scarydb standalone` |
| **Server** | TCP server only (production) | `scarydb server` |
| **Client** | REPL client only | `scarydb client` |
| **Log Reader** | Read binary WAL log | `scarydb log-read <path>` |
| **Version** | Show version | `scarydb --version` |

---

## 🎮 Database Query Syntax (Interactive REPL)

Once connected via the client, you can execute case-insensitive SQL-like queries. Multiple operations can be chained using the `/` separator, and statements optionally terminate with a semicolon `;`.

### 1. Database Definition Commands (DDC)
Used to structure database context and bucket namespaces:
*   `CREATE DB <db_name>;` - Create a new database.
*   `DROP DB <db_name>;` - Delete a database.
*   `USE <db_name>;` - Switch the active connection to a specific database context.
*   `CREATE BUCKET <bucket_name>;` - Create a bucket under the current active database.
*   `DROP BUCKET <bucket_name>;` - Drop a bucket and all its keys under the current active database.
*   `LIST DBS;` (or `LIST DATABASES;`) - List all databases.
*   `LIST BUCKETS;` (or `LIST BUCK;`) - List all buckets in the active database.

### 2. Data Manipulation Commands (DMC)
Used to mutate key-value pairs:
*   `SET <bucket> <key> [TYPE_TAG] <value> [ / <key2> [TYPE_TAG] <value2> ... ];`
    *   Set one or more key-value pairs in a bucket.
    *   **Type Tags (Optional):** `[STRING]`, `[INT]`, `[FLOAT]`, or `[BOOL]`. If omitted, type is automatically detected (e.g. `42` becomes `Int`, `true` becomes `Bool`, `"hello"` becomes `String`).
    *   *Example:* `SET users user1 "Alice" / user2 [INT] 42 / user3 [BOOL] true;`
*   `DEL <bucket> <key> [ / <key2> ... ];` - Delete one or more keys from a bucket.

### 3. Data Retrieval Commands (DRC)
Used to query key-value pairs:
*   `GET <bucket> <key> [ / <key2> ... ];` - Retrieve the value of one or more keys. Missing keys return `(nil)`.
*   `EXISTS <bucket> <key> [ / <key2> ... ];` - Check if one or more keys exist (returns `true` or `false`).
*   `LIST <bucket>;` - List all key names inside a bucket.
*   `COUNT <bucket>;` - Get the count of keys inside a bucket.

### 4. System Control Commands (SCC)
Used for checking server status, latency, and versions:
*   `PING` (or `BOINK`) - Test connectivity. Returns `BOINK! 🐷`.
*   `INFO` - Returns server startup timestamps, data directory paths, thread worker configurations, and memory limits.
*   `STATS` - Returns count metrics for databases, buckets, and keys.
*   `VERSION` - Returns current version of ScaryDB.
*   `HELP` (or `MAN`) - Displays command syntax instructions inside the REPL.
*   `EXIT` (or `QUIT`) - Disconnects from the server and exits the REPL.

### 5. Configuration Control Commands (CCC)
Used to view and update server configurations at runtime:
*   `LIST CONFIG;` - Lists all server configurations.
*   `GET CONFIG <property>;` - Get the value of a configuration property.
*   `SET CONFIG <property> <value>;` - Set a configuration property (saves updates to `config.json` automatically).
    *   *Example:* `SET CONFIG storage.checkpoint_interval_ops 100`

### 6. Resource Management Commands (RMC)
Used to monitor and manage resource limits, quotas, and pressure:
*   `SHOW RESOURCE USAGE;` - Display current resource usage (memory, connections, databases, buckets, keys, disk, pressure levels).
*   `SHOW RESOURCE LIMITS;` - Display configured resource limits.

### 7. Backup & Recovery Commands
*   `BACKUP <path>;` - Create a full backup (catalog, databases, WAL) to the specified path.
*   `RESTORE <path>;` - Restore database from a backup directory.

### 8. Circuit Breaker Commands
*   `CIRCUIT BREAKER STATUS;` - Show circuit breaker state.
*   `CIRCUIT BREAKER RESET;` - Reset circuit breaker to CLOSED state.

### 9. Authentication Commands
*   `AUTH <api_key>;` - Authenticate with API key.
*   `AUTH TOKEN <jwt_token>;` - Authenticate with JWT token.

---

## ⚙️ Configuration Properties (`config.json`)

The following settings are managed in `config.json`:

| Property | Description | Default |
|----------|-------------|---------|
| `server.workers` | Number of thread pool worker threads | `1` |
| `storage.data_dir` | Data directory for snapshots and WAL | `./data` |
| `storage.checkpoint_interval_ops` | Operations between JSON checkpoints | `10000` |
| `memory.max_memory_kb` | Memory limit (0 = unlimited) | `0` |
| `network.host` | Bind address | `127.0.0.1` |
| `network.port` | TCP port | `6379` |

### TLS Configuration
| Property | Description | Default |
|----------|-------------|---------|
| `tls.enabled` | Enable TLS | `false` |
| `tls.cert_file` | Certificate file path | `""` |
| `tls.key_file` | Private key file path | `""` |
| `tls.ca_file` | CA certificate file (for mTLS) | `""` |
| `tls.require_client_cert` | Require client certificate | `false` |

### Authentication Configuration
| Property | Description | Default |
|----------|-------------|---------|
| `auth.enabled` | Enable authentication | `false` |
| `auth.jwt_secret` | JWT signing secret | (required if enabled) |
| `auth.token_expiry_hours` | JWT token expiry | `24` |
| `auth.api_keys` | List of valid API keys | `[]` |

### Runtime Resource Limits (via Resource Manager)

| Property | Description | Default |
|----------|-------------|---------|
| `max_memory_kb` | Max memory (KB), 0 = unlimited | `0` |
| `max_connections` | Max concurrent connections | `10000` |
| `max_databases` | Max databases | `1000` |
| `max_buckets_per_db` | Max buckets per database | `10000` |
| `max_keys_per_bucket` | Max keys per bucket | `1000000` |
| `max_key_size_bytes` | Max key size | `65536` (64KB) |
| `max_value_size_bytes` | Max value size | `1048576` (1MB) |
| `max_disk_mb` | Max disk usage (MB), 0 = unlimited | `0` |
| `memory_pressure_threshold` | Memory pressure threshold (0.0-1.0) | `0.85` |
| `disk_pressure_threshold` | Disk pressure threshold (0.0-1.0) | `0.90` |

---

## 📊 Health & Monitoring

### HTTP Endpoints (when health server enabled)

| Endpoint | Description |
|----------|-------------|
| `GET /health` | Liveness probe - returns version, uptime, status |
| `GET /ready` | Readiness probe - checks database/storage accessibility, disk space, memory |
| `GET /metrics` | Prometheus metrics exposition format |

### Structured Logging

ScaryDB uses structured JSON logging with correlation IDs:

```json
{
  "timestamp": "2026-08-19T09:36:36.619Z",
  "level": "INFO",
  "fields": {
    "message": "ScaryDB logging initialized",
    "version": "0.1.0"
  },
  "target": "scarydb::logging",
  "threadName": "main",
  "threadId": "ThreadId(1)"
}
```

Set log level: `RUST_LOG=debug scarydb server`

### Prometheus Metrics

Key metrics exposed:
- `scarydb_requests_total{type="total|success|error"}`
- `scarydb_uptime_seconds`
- `scarydb_request_duration_seconds`
- Resource usage gauges (memory, connections, disk, pressure levels)

### Enhanced Health Checks (`/ready`)

The readiness endpoint performs dependency checks:
- **Disk Space**: Verifies sufficient disk space for WAL and checkpoints
- **Memory**: Checks memory pressure threshold
- **Network**: Validates TCP listener is accepting connections
- **Storage**: Confirms database files are readable/writable

---

## 🛡️ Resource Management

ScaryDB includes a built-in resource manager with:

- **Limits**: Configurable limits for memory, connections, databases, buckets, keys, disk
- **Per-database quotas**: Per-database resource quotas
- **Pressure detection**: Automatic memory/disk pressure detection with graceful degradation
- **Connection pooling**: RAII connection guards with automatic cleanup
- **Background monitoring**: Periodic resource usage snapshots with pressure alerts

### CLI Commands

```sql
-- Show current resource usage
SHOW RESOURCE USAGE;

-- Show configured limits
SHOW RESOURCE LIMITS;
```

---

## 🔧 Installation Scripts

| Platform | Script | Service Manager |
|----------|--------|-----------------|
| Linux | `./install_linux.sh` | systemd |
| macOS | `./install_macos.sh` | launchd |
| Windows | `.\install_windows.ps1` | Windows Service |

### Linux Service Management
```bash
sudo systemctl start scarydb
sudo systemctl status scarydb
sudo journalctl -u scarydb -f
```

### macOS Service Management
```bash
sudo launchctl load /Library/LaunchDaemons/com.scarydb.server.plist
launchctl list | grep scarydb
tail -f /opt/scarydb/logs/scarydb.log
```

### Windows Service Management
```powershell
Start-Service ScaryDB
Get-Service ScaryDB
Get-WinEvent -LogName Application -ProviderName ScaryDB
```

---

## 🔨 Building from Source

### Prerequisites
- Rust 1.70+ (`curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`)

### Build Commands

```bash
git clone https://github.com/SanjaiPS-tech/ScaryDB.git
cd ScaryDB

# Quick build (release mode)
./build.sh

# Debug build
./build.sh -m debug

# Cross-compile for target
./build.sh -t x86_64-unknown-linux-musl
./build.sh -t aarch64-apple-darwin

# Build and create distribution package
./build.sh --package

# Clean build
./build.sh --clean -m release
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

---

## ⚡ Performance Benchmarks

ScaryDB is optimized as a blazingly fast in-memory key-value store with cached WAL descriptor streaming.

### Latency Profiles (Optimized Release Mode)

| Operation | Throughput | Avg Latency |
|-----------|------------|-------------|
| **GET (Reads)** | **~22,700 ops/sec** | **~44 µs** |
| **SET (Writes)** | **~13,200 ops/sec** | **~76 µs** |

### Running Benchmarks

```bash
# Terminal 1: Start server
cargo run --release --bin scarydb -- server

# Terminal 2: Run benchmark
cargo run --release --bin benchmark
```

---

## 🏗️ Architecture

```
┌─────────────────────────────────────────────────────┐
│                    ScaryDB                          │
├─────────────────────────────────────────────────────┤
│  TCP Server (mio) ←→ Thread Pool (parking_lot)     │
│         ↓                    ↓                      │
│  Worker Threads ←→  Database Engine (DashMap)      │
│         ↓                    ↓                      │
│  WAL Writer ←→  Persistence Manager (JSON + WAL)   │
│         ↓                    ↓                      │
│  Resource Manager ←→  Health/Metrics Server (Axum) │
│         ↓                    ↓                      │
│  TLS/Auth Layer ←→  Circuit Breaker                │
└─────────────────────────────────────────────────────┘
```

### Key Technologies
- **Concurrency**: `parking_lot` (RwLock/Mutex), `flume` (bounded channels), `crossbeam`
- **Networking**: `mio` (epoll/kqueue/IOCP), `tokio` (health server)
- **Storage**: `dashmap` (concurrent HashMap), `memmap2` (mmap), `serde_json`
- **Observability**: `tracing` (structured logging), `metrics` + `metrics-exporter-prometheus`
- **Health**: `axum` + `tower-http` (HTTP endpoints)
- **TLS**: `rustls` + `tokio-rustls`
- **Auth**: `jsonwebtoken` (JWT), API keys
- **Reliability**: Circuit breaker pattern, PITR from WAL

---

## 🐛 Troubleshooting

### Port Already in Use
```bash
# Find and kill process
lsof -i :6379
kill -9 <PID>

# Or change port
SET CONFIG network.port 6380;
```

### Permission Denied (Linux/macOS)
```bash
# Run installer with sudo
sudo ./install_linux.sh

# Or fix permissions
chmod +x scarydb.sh scarydb
```

### Binary Not Found After Build
```bash
# Clean and rebuild
./build.sh --clean -m release

# Check target directory
ls -la target/release/
```

### Windows Service Won't Start
```powershell
# Check Event Viewer
Get-WinEvent -LogName Application -ProviderName ScaryDB | Select -First 20

# Check binary runs directly
cd "C:\Program Files\ScaryDB"
.\scarydb.exe server
```

### Connection Refused
```bash
# Check server is running
ps aux | grep scarydb
# or
sudo systemctl status scarydb

# Check port
netstat -tlnp | grep 6379
ss -tlnp | grep 6379
```

### TLS Certificate Issues
```bash
# Verify certificate
openssl x509 -in cert.pem -text -noout

# Check key matches cert
openssl rsa -in key.pem -pubout | openssl sha256
openssl x509 -in cert.pem -pubkey -noout | openssl sha256
```

---

## 🔒 Security Considerations

1. **Network Binding**: Default binds to `127.0.0.1` only. Use `0.0.0.0` with firewall rules for external access.
2. **Authentication**: Enable API key or JWT authentication for production. Use TLS for transport encryption.
3. **File Permissions**: Data directory should be owned by service user only.
4. **Updates**: Regularly update from GitHub releases for security patches.
5. **Secrets**: Store JWT secret and TLS keys securely, not in version control.

---

## 📦 Distribution Packaging

ScaryDB provides packaging for major package managers:

| Format | Location | Status |
|--------|----------|--------|
| **Homebrew** | `packaging/homebrew/scarydb.rb` | ✅ Ready |
| **Chocolatey** | `packaging/chocolatey/` | ✅ Ready |
| **Debian/APT** | `packaging/debian/` | ✅ Ready |
| **RPM/DNF** | `packaging/fedora/` | ✅ Ready |

### Building Packages

```bash
# Homebrew
brew tap-new SanjaiPS-tech/scarydb
cp packaging/homebrew/scarydb.rb $(brew --repository SanjaiPS-tech/scarydb)/Formula/

# Chocolatey
cd packaging/chocolatey
# Edit .nuspec with actual version/checksums
choco pack

# Debian
cd packaging/debian
chmod +x build-deb.sh
./build-deb.sh

# RPM
cd packaging/fedora
rpmbuild -ba scarydb.spec
```

---

## 📚 Documentation

- [QUICKSTART.md](QUICKSTART.md) - 5-minute getting started guide
- [SETUP.md](SETUP.md) - Comprehensive installation guide for all platforms
- [CHANGELOG.md](CHANGELOG.md) - Version history

---

## 🤝 Contributing

1. Fork the repo
2. Create feature branch
3. Run tests: `cargo test`
4. Submit PR

---

## 📄 License

MIT OR Apache-2.0

---

## 🙋 Support

- **GitHub Issues**: https://github.com/SanjaiPS-tech/ScaryDB/issues
- **Discussions**: https://github.com/SanjaiPS-tech/ScaryDB/discussions