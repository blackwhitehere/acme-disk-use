//! High-level disk usage analysis interface combining cache and scanner

use std::{io, path::Path};

use crate::cache::CacheManager;
use crate::error::DiskUseError;
use crate::scanner::{self, DirStat};

/// Main interface for disk usage analysis with caching support
pub struct DiskUse {
    cache_manager: CacheManager,
}

impl DiskUse {
    /// Create a new DiskUse instance with the specified cache file path
    pub fn new(cache_path: impl AsRef<Path>) -> Self {
        Self {
            cache_manager: CacheManager::new(cache_path),
        }
    }

    /// Create a new DiskUse instance using the default cache location
    pub fn new_with_default_cache() -> Self {
        Self::new(crate::get_default_cache_path())
    }

    /// Scan a directory and return its total size in bytes
    ///
    /// This method automatically:
    /// - Loads from cache
    /// - Scans only changed directories
    /// - Saves the updated cache
    pub fn scan(&mut self, path: impl AsRef<Path>) -> io::Result<u64> {
        self.scan_with_options(path, false)
    }

    /// Scan a directory with options for ignoring cache
    ///
    /// # Arguments
    /// * `path` - The directory path to scan
    /// * `ignore_cache` - If true, performs a fresh scan without using cache
    pub fn scan_with_options(
        &mut self,
        path: impl AsRef<Path>,
        ignore_cache: bool,
    ) -> io::Result<u64> {
        let path = path.as_ref();

        // Check if path exists first
        if !path.exists() {
            return Err(io::Error::from(DiskUseError::PathNotFound {
                path: path.to_path_buf(),
            }));
        }

        // Normalize path to avoid issues with symlinks and /private on macOS
        let path_buf = match path.canonicalize() {
            Ok(p) => p,
            Err(err) => {
                // If canonicalization fails (e.g., permission denied), use original path
                if err.kind() == io::ErrorKind::PermissionDenied {
                    return Err(io::Error::from(DiskUseError::PermissionDenied {
                        path: path.to_path_buf(),
                    }));
                }
                path.to_path_buf()
            }
        };

        // Get existing cache entry for this root (unless ignoring cache)
        let old_entry = if ignore_cache {
            None
        } else {
            self.cache_manager.get(&path_buf)
        };

        // Scan the directory (will use cache for unchanged subdirectories)
        let new_entry = scanner::scan_directory(&path_buf, old_entry)?;

        // Get the total size before potentially moving new_entry
        let total_size = new_entry.total_size();

        // Update the cache with new results (unless ignoring cache)
        if !ignore_cache {
            self.cache_manager.update(&path_buf, new_entry);
            // Cache will auto-save on drop
        }

        Ok(total_size)
    }

    /// Get detailed statistics for a previously scanned path
    pub fn get_stats(&self, path: impl AsRef<Path>) -> Option<&DirStat> {
        self.cache_manager.get(path.as_ref())
    }

    /// Get file count for a path
    ///
    /// # Arguments
    /// * `path` - The path to get file count for
    /// * `ignore_cache` - If true, counts files directly from filesystem instead of using cache
    pub fn get_file_count(&self, path: impl AsRef<Path>, ignore_cache: bool) -> io::Result<u64> {
        if ignore_cache {
            scanner::count_files(path.as_ref())
        } else {
            Ok(self
                .get_stats(path)
                .map(|stats| stats.file_count())
                .unwrap_or(0))
        }
    }

    /// Save the current cache to disk
    pub fn save_cache(&mut self) -> io::Result<()> {
        self.cache_manager.save()
    }

    /// Clear all cache contents
    pub fn clear_cache(&mut self) -> io::Result<()> {
        self.cache_manager.clear()
    }

    /// Delete the cache file
    pub fn delete_cache(&self) -> io::Result<()> {
        self.cache_manager.delete()
    }

    /// Get the cache file path
    pub fn cache_path(&self) -> &Path {
        self.cache_manager.path()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn create_test_directory_structure(base: &Path) -> io::Result<()> {
        fs::create_dir_all(base.join("subdir1"))?;
        fs::create_dir_all(base.join("subdir2/nested"))?;

        fs::write(base.join("file1.txt"), "Hello World")?;
        fs::write(base.join("file2.txt"), "Test content")?;
        fs::write(base.join("subdir1/nested_file.txt"), "Nested content here")?;
        fs::write(base.join("subdir2/another.txt"), "More content")?;
        fs::write(base.join("subdir2/nested/deep.txt"), "Deep file content")?;

        Ok(())
    }

    #[test]
    fn test_disk_use_with_cache() -> io::Result<()> {
        // This test verifies the full `DiskUse` workflow with caching enabled.
        // 1. It scans a directory and saves the cache.
        // 2. It creates a new `DiskUse` instance and scans again.
        // 3. It verifies that the second scan returns the correct size and file count
        //    (which should be retrieved from the cache).
        let temp_dir = TempDir::new()?;
        let test_dir = temp_dir.path().join("test");
        let cache_file = temp_dir.path().join("cache.bin");

        fs::create_dir(&test_dir)?;
        create_test_directory_structure(&test_dir)?;

        let canonical_test_dir = test_dir.canonicalize()?;

        {
            let mut disk_use = DiskUse::new(&cache_file);
            let size1 = disk_use.scan(&canonical_test_dir)?;
            assert!(size1 >= 71);

            // Force save by explicitly calling save_cache
            disk_use.save_cache()?;
        } // Drop happens here, ensuring save

        assert!(cache_file.exists());

        {
            let mut disk_use = DiskUse::new(&cache_file);
            let _size2 = disk_use.scan(&canonical_test_dir)?;
            assert!(_size2 >= 71);

            let file_count = disk_use.get_file_count(&canonical_test_dir, false)?;
            assert_eq!(file_count, 5);
        }

        Ok(())
    }

    #[test]
    fn test_disk_use_ignore_cache() -> io::Result<()> {
        // This test verifies the `ignore_cache` functionality.
        // 1. It scans a directory and populates the cache.
        // 2. It modifies the directory (adds a file).
        // 3. It scans again with `ignore_cache = true`.
        // 4. It verifies that the scan result reflects the change, ignoring the stale cache.
        let temp_dir = TempDir::new()?;
        let test_dir = temp_dir.path().join("test");
        let cache_file = temp_dir.path().join("cache.json");

        fs::create_dir(&test_dir)?;
        create_test_directory_structure(&test_dir)?;

        let mut disk_use = DiskUse::new(&cache_file);

        let size1 = disk_use.scan(&test_dir)?;
        assert!(size1 >= 71);

        fs::write(test_dir.join("new_file.txt"), "New content")?;

        let _size2 = disk_use.scan(&test_dir)?;

        let size3 = disk_use.scan_with_options(&test_dir, true)?;
        assert!(size3 >= 82);

        Ok(())
    }

    #[test]
    fn test_cache_management() -> io::Result<()> {
        // This test verifies the high-level cache management methods of `DiskUse`:
        // 1. `save_cache()`: Explicitly saving the cache.
        // 2. `clear_cache()`: Clearing the in-memory cache.
        // 3. `delete_cache()`: Deleting the cache file.
        let temp_dir = TempDir::new()?;
        let test_dir = temp_dir.path().join("test");
        let cache_file = temp_dir.path().join("cache.bin");

        fs::create_dir(&test_dir)?;
        create_test_directory_structure(&test_dir)?;

        {
            let mut disk_use = DiskUse::new(&cache_file);

            disk_use.scan(&test_dir)?;
            disk_use.save_cache()?; // Explicit save
        } // Drop saves too

        assert!(cache_file.exists());

        {
            let mut disk_use = DiskUse::new(&cache_file);
            disk_use.clear_cache()?;

            disk_use.delete_cache()?;
        }

        assert!(!cache_file.exists());

        Ok(())
    }

    #[test]
    fn test_get_file_count_subdirectory() -> io::Result<()> {
        // This test verifies that `get_file_count` correctly retrieves the file count
        // for a subdirectory from the cache, without needing to re-scan the filesystem.
        // It scans a parent directory, then requests the count for a child directory.
        let temp_dir = TempDir::new()?;
        let test_dir = temp_dir.path().join("test");
        let cache_file = temp_dir.path().join("cache.bin");

        fs::create_dir(&test_dir)?;
        fs::create_dir(test_dir.join("sub"))?;
        fs::write(test_dir.join("sub/file.txt"), "content")?;

        let mut disk_use = DiskUse::new(&cache_file);
        disk_use.scan(&test_dir)?; // Scans /test, should cache /test/sub

        // Try to get count for /test/sub from cache
        let count = disk_use.get_file_count(test_dir.join("sub"), false)?;
        assert_eq!(count, 1);

        Ok(())
    }

    #[test]
    #[cfg(unix)]
    fn test_compare_with_du() -> io::Result<()> {
        // This test compares the library output with the system `du` command.
        // It ensures that our block-based size calculation matches the system's.
        use std::process::Command;

        let temp_dir = TempDir::new()?;
        let test_dir = temp_dir.path().join("test_du");
        fs::create_dir(&test_dir)?;

        // Create some files with known content
        fs::write(test_dir.join("file1.txt"), "Hello World")?; // Small file
                                                               // Create a larger file to ensure multiple blocks
        let large_content = vec![0u8; 8192]; // 8KB
        fs::write(test_dir.join("file2.bin"), &large_content)?;

        let mut disk_use = DiskUse::new_with_default_cache();
        let lib_size = disk_use.scan_with_options(&test_dir, true)?;

        // Run `du -s -k` (kilobytes) and convert to bytes
        // Note: macOS du -s uses 512-byte blocks by default, but -k forces 1024-byte blocks.
        // However, our library uses 512-byte blocks.
        // Let's use `du -s` which returns 512-byte blocks on macOS/BSD and usually 1024 on GNU/Linux.
        // To be safe, let's use `du -k` and multiply by 1024, but precision might be lost.
        // Better: use `du -B1` on GNU or just check if it's close enough.

        // Actually, let's try to match exact block count if possible.
        // On macOS: `du -s` returns 512-byte blocks.
        // On Linux: `du -s` usually returns 1024-byte blocks (check BLOCK_SIZE env).

        let output = Command::new("du")
            .arg("-s")
            .arg("-k") // Force 1024-byte blocks for consistency across platforms
            .arg(&test_dir)
            .output()?;

        if !output.status.success() {
            // If du fails (e.g. not found), skip the test
            return Ok(());
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let du_kblocks: u64 = stdout.split_whitespace().next().unwrap().parse().unwrap();

        let du_bytes = du_kblocks * 1024;

        // Allow for some small difference due to block alignment/metadata
        // But ideally they should be very close.
        // Since `du -k` rounds up to nearest 1024, and we sum up 512-byte blocks,
        // our result might be slightly different but comparable.

        // Let's just print them for now and assert they are within a reasonable margin (e.g. 4KB)
        println!("Library size: {}, du size: {}", lib_size, du_bytes);

        let diff = lib_size.abs_diff(du_bytes);

        assert!(
            diff <= 4096,
            "Library size {} differs significantly from du size {}",
            lib_size,
            du_bytes
        );

        Ok(())
    }

    #[test]
    fn test_scan_nonexistent_directory() {
        // Test that scanning a nonexistent directory returns an appropriate error
        let temp_dir = TempDir::new().unwrap();
        let cache_file = temp_dir.path().join("cache.bin");
        let mut disk_use = DiskUse::new(&cache_file);

        let nonexistent = "/nonexistent/path/that/does/not/exist";
        let result = disk_use.scan(nonexistent);

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.to_string().contains("does not exist"),
            "Error should indicate path doesn't exist: {}",
            err
        );
    }
}
