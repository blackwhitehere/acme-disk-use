# GitHub Actions CI/CD Setup Guide

This document explains how to set up and configure GitHub Actions for the acme-disk-use project.

## Overview

The project uses two GitHub Actions workflows:

1. **CI Pipeline** (`.github/workflows/ci.yml`) - Continuous Integration
2. **Release Pipeline** (`.github/workflows/release.yml`) - Automated releases

## CI Pipeline

### Triggers

The CI pipeline runs automatically on:
- Push to `develop` branch
- Pull requests to `develop` or `main` branches

### Jobs

1. **Check** - Verifies the project compiles
   - Runs `cargo check --all-features --all-targets`
   - Uses caching to speed up builds

2. **Test** - Runs the test suite
   - Runs on Linux, macOS, and Windows
   - Executes `cargo test --all-features`
   - Matrix strategy ensures cross-platform compatibility

3. **Format** - Checks code formatting
   - Runs `cargo fmt --all -- --check`
   - Fails if code is not properly formatted

4. **Clippy** - Linting and static analysis
   - Runs `cargo clippy --all-targets --all-features -- -D warnings`
   - Treats all warnings as errors

5. **Coverage** (Optional) - Code coverage reporting
   - Uses `cargo-tarpaulin`
   - Uploads results to Codecov

### Before Pushing Code

Always run these commands locally before pushing:

```bash
# Format code
cargo fmt

# Run linter
cargo clippy --all-targets --all-features -- -D warnings

# Run tests
cargo test --all-features

# Build
cargo build
```

## Release Pipeline

### Triggers

The release pipeline runs when:
- A tag matching the pattern `v*.*.*` is pushed to the `main` branch
- Example: `v0.1.0`, `v1.2.3`, etc.

### Required Setup

#### 1. Create CRATES_IO_TOKEN Secret

To publish to crates.io, you need to add your crates.io API token as a GitHub secret:

1. **Get your crates.io token:**
   - Go to https://crates.io/settings/tokens
   - Create a new token with publish permissions
   - Copy the token (you won't be able to see it again)

2. **Add the secret to GitHub:**
   - Go to your repository on GitHub
   - Click **Settings** → **Secrets and variables** → **Actions**
   - Click **New repository secret**
   - Name: `CRATES_IO_TOKEN`
   - Value: Paste your crates.io token
   - Click **Add secret**

### Release Workflow

#### Jobs

1. **Validate** - Ensures tag version matches Cargo.toml
   - Extracts version from tag (e.g., `v0.2.0` → `0.2.0`)
   - Compares with version in `Cargo.toml`
   - Fails if versions don't match

2. **CI** - Runs all CI checks
   - Format check
   - Clippy linting
   - Test suite
   - Release build

3. **Publish** - Publishes to crates.io
   - Runs `cargo publish`
   - Uses `CRATES_IO_TOKEN` secret
   - Only runs after CI passes

4. **Build Binaries** - Builds for multiple platforms
   - Linux (x86_64, musl)
   - macOS (Intel, Apple Silicon)
   - Windows (x86_64)
   - Strips binaries to reduce size
   - Uploads artifacts

5. **Create Release** - Creates GitHub Release
   - Downloads all binary artifacts
   - Generates checksums
   - Creates release with description
   - Attaches binaries and checksums

### Creating a Release

#### Step 1: Update Version

Edit `Cargo.toml`:
```toml
[package]
version = "0.2.0"  # Update this
```

#### Step 2: Update CHANGELOG.md

Move unreleased changes to the new version:
```markdown
## [0.2.0] - 2025-11-03

### Added
- New feature X
- New feature Y

### Fixed
- Bug fix Z
```

#### Step 3: Commit Changes

```bash
git add Cargo.toml CHANGELOG.md
git commit -m "chore: bump version to 0.2.0"
```

#### Step 4: Merge to Main

```bash
# Ensure you're on develop
git checkout develop

# Merge develop into main
git checkout main
git merge develop
git push origin main
```

#### Step 5: Create and Push Tag

```bash
# Create tag
git tag v0.2.0

# Push tag
git push origin v0.2.0
```

#### Step 6: Monitor Release

1. Go to **Actions** tab on GitHub
2. Watch the **Release** workflow
3. Check for any failures
4. Once complete, check:
   - https://crates.io/crates/acme-disk-use (published)
   - https://github.com/yourusername/acme-disk-use/releases (release created)

## Troubleshooting

### CI Failures

**Format check fails:**
```bash
cargo fmt
git add .
git commit -m "fix: format code"
```

**Clippy fails:**
```bash
cargo clippy --all-targets --all-features -- -D warnings
# Fix reported issues
git add .
git commit -m "fix: address clippy warnings"
```

**Tests fail:**
```bash
cargo test --all-features
# Debug and fix failing tests
```

### Release Failures

**Version mismatch:**
- Ensure tag version (without 'v') matches `Cargo.toml` version exactly
- Example: Tag `v0.2.0` requires `version = "0.2.0"` in Cargo.toml

**CRATES_IO_TOKEN not found:**
- Verify the secret is added in repository settings
- Name must be exactly `CRATES_IO_TOKEN`

**Publish fails (crate already exists):**
- You can't republish the same version
- Increment version number and create new tag

**Binary build fails:**
- Check build logs for specific platform
- May need to add platform-specific dependencies

## Best Practices

### Branch Strategy

- **develop** - Main development branch
- **main** - Stable, release-ready code
- **feature/*** - Feature branches (merge to develop)
- **fix/*** - Bug fix branches (merge to develop)

### Commit Messages

Follow conventional commits:
- `feat:` - New features
- `fix:` - Bug fixes
- `docs:` - Documentation changes
- `chore:` - Maintenance tasks
- `test:` - Test additions/changes
- `refactor:` - Code refactoring

### Pull Requests

1. Create feature branch from `develop`
2. Make changes
3. Ensure CI passes locally
4. Open PR against `develop`
5. Wait for CI checks to pass
6. Request review
7. Merge after approval

### Versioning

Follow Semantic Versioning (SemVer):
- **MAJOR** (1.0.0) - Breaking changes
- **MINOR** (0.1.0) - New features (backward compatible)
- **PATCH** (0.0.1) - Bug fixes (backward compatible)

## Additional Configuration

### Codecov (Optional)

To enable code coverage reporting:

1. Sign up at https://codecov.io
2. Connect your GitHub repository
3. The CI pipeline will automatically upload coverage
4. Add badge to README:
   ```markdown
   [![codecov](https://codecov.io/gh/yourusername/acme-disk-use/branch/main/graph/badge.svg)](https://codecov.io/gh/yourusername/acme-disk-use)
   ```

### Dependabot (Recommended)

Create `.github/dependabot.yml`:
```yaml
version: 2
updates:
  - package-ecosystem: "cargo"
    directory: "/"
    schedule:
      interval: "weekly"
```

This automatically creates PRs for dependency updates.

## Monitoring

### GitHub Actions Dashboard

- **Actions** tab shows all workflow runs
- Click on a run to see detailed logs
- Failed jobs show error messages

### crates.io Dashboard

- View download statistics
- Monitor version history
- Check documentation builds

## Support

If you encounter issues:
1. Check workflow logs in GitHub Actions
2. Verify all secrets are correctly configured
3. Ensure local tests pass before pushing
4. Open an issue with workflow logs attached

## Reference Links

- [GitHub Actions Documentation](https://docs.github.com/en/actions)
- [cargo publish Documentation](https://doc.rust-lang.org/cargo/commands/cargo-publish.html)
- [Semantic Versioning](https://semver.org/)
- [Conventional Commits](https://www.conventionalcommits.org/)
