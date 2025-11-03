# Contributing to acme-disk-use

Thank you for considering contributing to acme-disk-use! This document provides guidelines and instructions for contributing.

## Development Setup

### Prerequisites

- Rust 1.70 or later (install from [rustup.rs](https://rustup.rs/))
- Git
- A GitHub account (for submitting pull requests)

### Getting Started

1. **Fork and clone the repository:**
   ```bash
   git clone https://github.com/YOUR_USERNAME/acme-disk-use.git
   cd acme-disk-use
   ```

2. **Create a new branch:**
   ```bash
   git checkout -b feature/your-feature-name
   ```

3. **Build the project:**
   ```bash
   cargo build
   ```

4. **Run tests:**
   ```bash
   cargo test
   ```

## Development Workflow

### Before Committing

Always run these commands before committing:

```bash
# Format your code
cargo fmt

# Run clippy for linting
cargo clippy --all-targets --all-features -- -D warnings

# Run tests
cargo test --all-features

# Run benchmarks (optional, but recommended for performance changes)
cargo bench
```

### Code Style

- Follow the official [Rust Style Guide](https://github.com/rust-lang/style-team)
- Use `cargo fmt` to automatically format your code
- Use meaningful variable and function names
- Add documentation comments (`///`) for public APIs
- Keep functions focused and small
- Add tests for new functionality

### Commit Messages

Write clear, descriptive commit messages following this format:

```
<type>: <subject>

<body>

<footer>
```

**Types:**
- `feat`: New feature
- `fix`: Bug fix
- `docs`: Documentation changes
- `style`: Code style changes (formatting, etc.)
- `refactor`: Code refactoring
- `perf`: Performance improvements
- `test`: Adding or updating tests
- `chore`: Maintenance tasks

**Example:**
```
feat: add parallel directory scanning with rayon

Implement parallel processing for subdirectories to improve
performance on multi-core systems. Falls back to sequential
processing for single subdirectories to avoid overhead.

Closes #42
```

## Pull Request Process

1. **Update documentation:**
   - Update README.md if you're adding new features
   - Update CHANGELOG.md under the `[Unreleased]` section
   - Add/update code documentation

2. **Ensure CI passes:**
   - All tests must pass
   - Code must be formatted (`cargo fmt`)
   - No clippy warnings (`cargo clippy`)
   - Build must succeed on all platforms

3. **Create a pull request:**
   - Target the `develop` branch (not `main`)
   - Provide a clear description of changes
   - Reference any related issues
   - Request review from maintainers

4. **Respond to feedback:**
   - Address review comments
   - Push additional commits as needed
   - Keep the PR focused on a single feature/fix

## Testing

### Running Tests

```bash
# Run all tests
cargo test

# Run tests with output
cargo test -- --nocapture

# Run specific test
cargo test test_scan_directory

# Run tests in a specific module
cargo test scanner::tests
```

### Writing Tests

- Add unit tests in the same file as the code (in a `#[cfg(test)]` mod tests block)
- Add integration tests in the `tests/` directory
- Use `tempfile` crate for tests that need temporary directories
- Test both success and error cases
- Test edge cases (empty directories, non-existent paths, etc.)

### Benchmarks

```bash
# Run all benchmarks
cargo bench

# Run specific benchmark
cargo bench cold_cache

# View benchmark results
cat target/criterion/report/index.html
```

## Performance Considerations

When working on performance-sensitive code:

1. **Benchmark your changes:**
   ```bash
   cargo bench
   ```

2. **Profile the application:**
   ```bash
   cargo build --profile profiling
   samply record target/profiling/acme-disk-use benchmark_data
   ```

3. **Consider:**
   - Avoiding unnecessary allocations
   - Using parallel processing appropriately
   - Caching expensive computations
   - Minimizing I/O operations

## Documentation

### Code Documentation

- Add doc comments (`///`) to all public functions, structs, and modules
- Include examples in doc comments when helpful
- Explain the "why" not just the "what"

```rust
/// Scan a directory recursively and return statistics
/// 
/// # Arguments
/// * `path` - The directory path to scan
/// * `cache` - Optional cached statistics for this directory
/// 
/// # Returns
/// Directory statistics including size, file count, and child directories
/// 
/// # Examples
/// ```
/// use acme_disk_use::scanner::scan_directory;
/// let stats = scan_directory(Path::new("."), None)?;
/// println!("Total size: {}", stats.total_size());
/// ```
pub fn scan_directory(path: &Path, cache: Option<&DirStat>) -> io::Result<DirStat> {
    // Implementation
}
```

### Generate Documentation

```bash
# Generate and open documentation
cargo doc --open
```

## Release Process

Releases are automated via GitHub Actions:

1. **Update version in Cargo.toml:**
   ```toml
   version = "0.2.0"
   ```

2. **Update CHANGELOG.md:**
   - Move unreleased changes to new version section
   - Add release date
   - Update comparison links

3. **Merge to main:**
   ```bash
   git checkout main
   git merge develop
   ```

4. **Create and push tag:**
   ```bash
   git tag v0.2.0
   git push origin main --tags
   ```

5. **GitHub Actions will:**
   - Run all CI checks
   - Build binaries for all platforms
   - Publish to crates.io
   - Create GitHub Release

## Reporting Issues

### Before Opening an Issue

- Search existing issues to avoid duplicates
- Update to the latest version
- Collect relevant information (OS, Rust version, error messages)

### Opening an Issue

Include:
- Clear description of the problem
- Steps to reproduce
- Expected vs actual behavior
- Environment details (OS, Rust version)
- Relevant logs or error messages

### Issue Labels

- `bug`: Something isn't working
- `enhancement`: New feature or request
- `documentation`: Documentation improvements
- `good first issue`: Good for newcomers
- `help wanted`: Extra attention needed
- `performance`: Performance-related

## Questions?

- Open a [Discussion](https://github.com/blackwhitehere/acme-disk-use/discussions)
- Ask in the issue comments
- Check existing documentation

## License

By contributing, you agree that your contributions will be licensed under the Apache-2.0 License.
