# acme-disk-use

[![CI](https://github.com/blackwhitehere/acme-disk-use/workflows/CI/badge.svg)](https://github.com/blackwhitehere/acme-disk-use/actions/workflows/ci.yml)
[![Crates.io](https://img.shields.io/crates/v/acme-disk-use.svg)](https://crates.io/crates/acme-disk-use)
[![Documentation](https://docs.rs/acme-disk-use/badge.svg)](https://docs.rs/acme-disk-use)
[![License](https://img.shields.io/crates/l/acme-disk-use.svg)](LICENSE)
> Disclaimer: This is alpha software. Interfaces and cache formats may change without notice.

A replacement for `du` for applications that:
- Mostly write immutable files
- Occasionaly add a new sub-directory where incremental output directories are written
- Mostly output their data to incrementaly created directories
- Need fast repeated disk usage calculations

e.g. a directory of model outputs each writing its output to a new daily data directory

## Features

- **Caching**: Aggregates disk usage stats at directory level and caches results so they can be reused on next invocation if no change to underlying data is found
- **Cache Invalidation**: Scans directories that have changed since last scan based on dir's mtime or under which a new sub-directory was created (no matter how nested)
- **Smart Deletion Detection**: Prunes deleted directories from cache without full rescans
- **Human-Readable Output**: Automatically formats sizes in B, KB, MB, GB, or TB
- **Flexible Cache Location**: Configurable via environment variable or defaults to `~/.cache/acme-disk-use/`

## Installation

### From crates.io (Recommended)

Install the latest stable version from [crates.io](https://crates.io/crates/acme-disk-use):

```bash
cargo install acme-disk-use
```

### From GitHub Release

Download pre-built binaries for your platform from the [Releases page](https://github.com/yourusername/acme-disk-use/releases):

**Linux (x86_64):**
```bash
wget https://github.com/yourusername/acme-disk-use/releases/latest/download/acme-disk-use-linux-x86_64
chmod +x acme-disk-use-linux-x86_64
sudo mv acme-disk-use-linux-x86_64 /usr/local/bin/acme-disk-use
```

**macOS (Intel):**
```bash
curl -LO https://github.com/yourusername/acme-disk-use/releases/latest/download/acme-disk-use-macos-x86_64
chmod +x acme-disk-use-macos-x86_64
sudo mv acme-disk-use-macos-x86_64 /usr/local/bin/acme-disk-use
```

**macOS (Apple Silicon):**
```bash
curl -LO https://github.com/yourusername/acme-disk-use/releases/latest/download/acme-disk-use-macos-aarch64
chmod +x acme-disk-use-macos-aarch64
sudo mv acme-disk-use-macos-aarch64 /usr/local/bin/acme-disk-use
```

**Windows:**
Download `acme-disk-use-windows-x86_64.exe` from the releases page and add it to your PATH.

### From Source

Clone the repository and build from source:

```bash
git clone https://github.com/yourusername/acme-disk-use.git
cd acme-disk-use
cargo build --release
# Binary will be at target/release/acme-disk-use
```

### Verify Installation

```bash
acme-disk-use --version
acme-disk-use --help
```

## TODO

- Memory-mapped cache loading for instant startup
- Configurable parallel scanning threshold
- User picks to use logical file size or block size (like du does)

## Usage

### Basic Usage

Scan current directory:
```bash
acme-disk-use
```

Scan a specific directory:
```bash
acme-disk-use /path/to/directory
```

### Options

**Show raw bytes instead of human-readable sizes:**
```bash
acme-disk-use --non-human-readable /path/to/directory
```

**Ignore cache and scan fresh:**
```bash
acme-disk-use --ignore-cache /path/to/directory
```

**Clean the cache:**
```bash
acme-disk-use clean
```

**Show help:**
```bash
acme-disk-use --help
```

### Configuration

**Custom cache location:**
Set the `ACME_DISK_USE_CACHE` environment variable:
```bash
export ACME_DISK_USE_CACHE=/custom/path/to/cache/
acme-disk-use /path/to/directory
```

Or use it inline:
```bash
ACME_DISK_USE_CACHE=/tmp/path/to/cache/ acme-disk-use /path/to/directory
```

**Default cache location:**
- If `ACME_DISK_USE_CACHE` is not set, defaults to `~/.cache/acme-disk-use` on Unix systems
- Falls back to `./cache.bin` if home directory is not available

## Examples

```bash
# Scan data directory with human-readable output
$ acme-disk-use data
Scanned 42 files, total size: 1.25 GB

# Show exact byte count
$ acme-disk-use --non-human-readable data
Scanned 42 files, total size: 1342177280 bytes

# Force fresh scan without using cache
$ acme-disk-use --ignore-cache data
Scanned 42 files, total size: 1.25 GB

# Clear all cached data
$ acme-disk-use clean
Cache cleared successfully.
```

# Development

## Cargo commands

### Check for compile errors:

`cargo check`

### Format files

`cargo fmt`

### Build binaries

`cargo build`

### Run binary

`RUST_LOG=debug cargo run`

### Build documentation

`cargo doc --open`

### Run tests

`cargo test`

### Run benchmarks

Relies on `criterion` library

`cargo bench`

### Profile application

Install `samply`: https://github.com/mstange/samply

`cargo build --profile profiling`
`samply record target/profiling/acme-disk-use`

### Linting
Install `clippy`: `rustup component add clippy`
`cargo clippy --all-targets --all-features -- -D warnings`

## Contributing

We welcome contributions! Please see [CONTRIBUTING.md](CONTRIBUTING.md) for detailed guidelines.

**Quick Start:**
1. Fork the repository
2. Create a feature branch: `git checkout -b feature/your-feature`
3. Make your changes
4. Run tests: `cargo test`
5. Format code: `cargo fmt`
6. Check lints: `cargo clippy --all-targets --all-features -- -D warnings`
7. Commit and push
8. Open a pull request against the `develop` branch

## CI/CD

This project uses GitHub Actions for continuous integration and deployment:

- **CI Pipeline** (`ci.yml`): Runs on every push to `develop` and on pull requests
  - ✓ Code formatting check (`cargo fmt`)
  - ✓ Linting with clippy (`cargo clippy`)
  - ✓ Test suite on Linux, macOS, and Windows
  - ✓ Code coverage reporting

- **Release Pipeline** (`release.yml`): Triggered by version tags (e.g., `v0.1.0`) on `main` branch
  - ✓ Validates version matches Cargo.toml
  - ✓ Runs full CI checks
  - ✓ Publishes to crates.io
  - ✓ Builds binaries for multiple platforms
  - ✓ Creates GitHub Release with binaries

**Creating a Release:**
```bash
# Update version in Cargo.toml and CHANGELOG.md
git tag v0.2.0
git push origin main --tags
```

## License

Licensed under the Apache License, Version 2.0. See [LICENSE](LICENSE) for details.

## Acknowledgments

- Built with [Rust](https://www.rust-lang.org/)
- Uses [rayon](https://github.com/rayon-rs/rayon) for parallel processing
- Uses [bincode](https://github.com/bincode-org/bincode) for efficient serialization
- Benchmarking powered by [criterion](https://github.com/bheisler/criterion.rs)
