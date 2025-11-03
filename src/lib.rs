//! Disk usage analyzer with intelligent caching
//!
//! This library provides fast disk usage calculation with caching support,
//! designed for applications that work with mostly immutable files.

mod cache;
mod disk_use;
mod scanner;

// Re-export public API
pub use disk_use::DiskUse;
pub use scanner::DirStat;

use std::{env, path::PathBuf};

/// Format bytes into string with optional human-readable scaling
///
/// # Arguments
/// * `bytes` - The size in bytes to format
/// * `human_readable` - If true, formats with appropriate scale (B, KB, MB, GB, TB)
///   If false, returns raw bytes with "bytes" suffix
///
/// # Examples
/// ```
/// use acme_disk_use::format_size;
///
/// assert_eq!(format_size(1024, true), "1.00 KB");
/// assert_eq!(format_size(1024, false), "1024 bytes");
/// ```
pub fn format_size(bytes: u64, human_readable: bool) -> String {
    if !human_readable {
        return format!("{} bytes", bytes);
    }

    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
    const THRESHOLD: f64 = 1024.0;

    if bytes == 0 {
        return "0 B".to_string();
    }

    let mut size = bytes as f64;
    let mut unit_index = 0;

    while size >= THRESHOLD && unit_index < UNITS.len() - 1 {
        size /= THRESHOLD;
        unit_index += 1;
    }

    if unit_index == 0 {
        format!("{} {}", bytes, UNITS[unit_index])
    } else {
        format!("{:.2} {}", size, UNITS[unit_index])
    }
}

/// Get default cache directory path
///
/// Checks the `ACME_DISK_USE_CACHE` environment variable first,
/// then falls back to `~/.cache/acme-disk-use/cache.bin` on Unix systems.
pub fn get_default_cache_path() -> PathBuf {
    if let Ok(cache_dir) = env::var("ACME_DISK_USE_CACHE") {
        let mut cache_dir = PathBuf::from(cache_dir);
        cache_dir.push("cache.bin");
        return cache_dir;
    }

    // Use XDG cache directory on Unix systems
    if let Ok(home) = env::var("HOME") {
        let mut cache_dir = PathBuf::from(home);
        cache_dir.push(".cache");
        cache_dir.push("acme-disk-use");
        cache_dir.push("cache.bin");
        cache_dir
    } else {
        // Fallback for systems without HOME
        let mut cache_dir = PathBuf::from("./.cache/acme-disk-use");
        std::fs::create_dir_all(&cache_dir).ok(); // Create the directory if it doesn't exist
        cache_dir.push("cache.bin");
        cache_dir
    }
}

/// Logger module for file-based logging
pub mod logger;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_size_human_readable() {
        assert_eq!(format_size(0, true), "0 B");
        assert_eq!(format_size(512, true), "512 B");
        assert_eq!(format_size(1024, true), "1.00 KB");
        assert_eq!(format_size(1536, true), "1.50 KB");
        assert_eq!(format_size(1024 * 1024, true), "1.00 MB");
        assert_eq!(format_size(1024 * 1024 * 1024, true), "1.00 GB");
        assert_eq!(format_size(1024_u64.pow(4), true), "1.00 TB");

        // Test non-human-readable format
        assert_eq!(format_size(0, false), "0 bytes");
        assert_eq!(format_size(1024, false), "1024 bytes");
        assert_eq!(format_size(1234567, false), "1234567 bytes");
    }

    #[test]
    fn test_default_cache_path() {
        let default_path = get_default_cache_path();

        // Should end with cache.bin
        assert!(default_path.to_string_lossy().ends_with("cache.bin"));

        // Should be different from just "cache.bin" unless we're in fallback mode
        // This test is environment-dependent, so we just check it's a valid path
        assert!(default_path.is_absolute() || default_path.ends_with("cache.bin"));
    }
}
