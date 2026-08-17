# GitHub Actions Workflows

This directory contains the CI/CD and maintenance pipelines for ScaryDB.

## Workflows

### 1. CI Pipeline (`ci.yml`)
Runs on every push and pull request to `main` and `develop` branches.

**Checks:**
- **Format Check** - `cargo fmt --all -- --check`
- **Clippy Lints** - `cargo clippy --all-targets --all-features -- -D warnings`
- **Unit & Integration Tests** - `cargo test --all-features`
- **Documentation Tests** - `cargo test --doc --all-features`
- **Build Release** - `cargo build --release --all-features`
- **Security Audit** - `cargo audit` + `cargo deny check advisories`
- **MSRV Check** - Build with Rust 1.70 (Minimum Supported Rust Version)
- **Dead Code Check** - `cargo machete`

### 2. Benchmark Pipeline (`benchmark.yml`)
Runs on pushes to `main`, PRs to `main`, and manual dispatch.

**Features:**
- Runs the benchmark binary multiple times (configurable, default 5)
- Calculates average SET/GET ops/sec
- Compares against baseline from `main` branch README
- Fails PR if regression > 10%
- Comments benchmark results on PRs
- Maintains benchmark history in `benchmark_history.csv` on main branch

### 3. Scheduled Maintenance (`maintenance.yml`)
Runs on schedule and manual dispatch.

**Schedules:**
- **Daily** (2 AM UTC): Format, clippy, tests, build, security audit, dependency check
- **Weekly** (Sunday 3 AM UTC): Full test suite, benchmarks (3 runs), dead code, MSRV, docs, doc coverage
- **Security** (manual): cargo-audit, yanked deps check, license compliance
- **Dependency Update** (manual): cargo-outdated, cargo-update dry-run, test with updated deps

### 4. Release Pipeline (`release.yml`)
Triggers on version tags (`v*`) or manual dispatch.

**Steps:**
- Runs full test suite
- Builds release binary
- Runs benchmarks (3 runs)
- Creates release archive with checksums
- Generates changelog from git history
- Creates GitHub Release with artifacts
- Optionally publishes to crates.io (for stable releases)

## Manual Triggers

All maintenance workflows and benchmark pipeline can be triggered manually via:
- GitHub Actions UI → "Run workflow"
- GitHub CLI: `gh workflow run <workflow-name>.yml`

## Artifacts

All workflows upload artifacts with retention:
- **CI/Build**: Release binary (7 days)
- **Benchmark**: Output logs (30 days), history CSV (permanent on main)
- **Maintenance**: Reports (90 days)
- **Release**: Release artifacts (365 days)

## Required Secrets

For crates.io publishing:
- `CARGO_REGISTRY_TOKEN` - Crates.io API token

## Local Development

To run checks locally:
```bash
# Format
cargo fmt --all -- --check

# Clippy
cargo clippy --all-targets --all-features -- -D warnings

# Tests
cargo test --all-features -- --test-threads=4

# Build release
cargo build --release --all-features

# Benchmark
./target/release/benchmark

# Security audit
cargo install cargo-audit
cargo audit

# Outdated deps
cargo install cargo-outdated
cargo outdated --root-deps-only
```