# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- Comprehensive benchmarks for different directory sizes (tiny, small, medium, large)
- CI/CD pipeline with GitHub Actions
- Automated release pipeline with multi-platform binary builds
- Installation instructions in README

### Changed
- Improved benchmark suite with cold cache, warm cache, and cache invalidation tests

## [0.1.0] - 2025-11-03

### Added
- Initial release
- Fast disk usage calculation using logical file sizes
- Intelligent caching with mtime-based change detection
- Binary cache format (bincode) for 10x faster serialization
- Lazy cache writing with dirty flag tracking
- Parallel directory scanning with rayon
- Auto-save on drop functionality
- Smart deletion detection and cache pruning
- Human-readable output formatting (B, KB, MB, GB, TB)
- Command-line interface with `--non-human-readable` and `--ignore-cache` flags
- Configurable cache location via `ACME_DISK_USE_CACHE` environment variable

### Performance
- Warm cache: 1.2x faster than `du` on 400-file datasets
- Cold cache: 1.17x relative to `du` (includes cache write overhead)
- Binary cache: 35% smaller than JSON format
- 260x faster than traditional shell scripting approaches (find + awk)

[Unreleased]: https://github.com/blackwhitehere/acme-disk-use/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/blackwhitehere/acme-disk-use/releases/tag/v0.1.0
