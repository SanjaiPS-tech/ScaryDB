# Changelog

All notable changes to ScaryDB will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0] - 2026-08-17

### Added
- Initial release of ScaryDB
- High-performance in-memory hierarchical database (Database → Bucket → Key → Value)
- Actor-based architecture with thread pool concurrency model
- TCP server-client architecture with JSON wire protocol
- Interactive REPL client with SQL-like query syntax
- Dual-path persistence: binary WAL + JSON checkpoints
- Automatic type detection (String, Int, Float, Bool) with explicit type tags
- Cross-platform support: Linux (x86_64, ARM64), macOS (Intel, Apple Silicon), Windows (x86_64)
- System service installers for systemd (Linux), launchd (macOS), Windows Services
- Universal build script with auto-detection
- Distribution packaging for all platforms
- Comprehensive documentation (README, QUICKSTART, SETUP)

### Commands
- **DDC**: CREATE/DROP/USE/LIST databases and buckets
- **DMC**: SET/DEL key-value pairs with type tags
- **DRC**: GET/EXISTS/LIST/COUNT keys
- **SCC**: PING/INFO/STATS/VERSION/HELP/EXIT
- **CCC**: LIST/GET/SET CONFIG at runtime

### Performance (Release Mode)
- GET: ~22,700 ops/sec (~44 µs latency)
- SET: ~13,200 ops/sec (~76 µs latency)

### Infrastructure
- GitHub Actions CI/CD pipeline for multi-platform builds
- Release automation with artifacts for all targets
- Homebrew, Chocolatey, Snap, APT, DNF packaging configs
- Docker image configuration

---

## [Unreleased]

### Planned
- Authentication and ACL system
- TLS/SSL support
- Cluster mode with replication
- Backup/restore utilities
- Metrics endpoint (Prometheus)
- Lua scripting support