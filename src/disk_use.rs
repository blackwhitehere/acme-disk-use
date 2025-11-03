//! High-level disk usage analysis interface combining cache and scanner

use std::{io, path::Path};

use crate::cache::CacheManager;
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

        // Normalize path to avoid issues with symlinks and /private on macOS
        let path_buf = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());

        // Get existing cache entry for this root (unless ignoring cache)
        let old_entry = if ignore_cache {
            None
        } else {
            self.cache_manager.get(&path_buf)
        };

        // Scan the directory (will use cache for unchanged subdirectories)
        let new_entry = scanner::scan_directory(path, old_entry)?;

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
        let temp_dir = TempDir::new()?;
        let test_dir = temp_dir.path().join("test");
        let cache_file = temp_dir.path().join("cache.bin");

        fs::create_dir(&test_dir)?;
        create_test_directory_structure(&test_dir)?;

        let canonical_test_dir = test_dir.canonicalize()?;

        {
            let mut disk_use = DiskUse::new(&cache_file);
            let size1 = disk_use.scan(&canonical_test_dir)?;
            assert_eq!(size1, 71);

            // Force save by explicitly calling save_cache
            disk_use.save_cache()?;
        } // Drop happens here, ensuring save

        assert!(cache_file.exists());

        {
            let mut disk_use = DiskUse::new(&cache_file);
            let _size2 = disk_use.scan(&canonical_test_dir)?;
            assert_eq!(_size2, 71);

            let file_count = disk_use.get_file_count(&canonical_test_dir, false)?;
            assert_eq!(file_count, 5);
        }

        Ok(())
    }

    #[test]
    fn test_disk_use_ignore_cache() -> io::Result<()> {
        let temp_dir = TempDir::new()?;
        let test_dir = temp_dir.path().join("test");
        let cache_file = temp_dir.path().join("cache.json");

        fs::create_dir(&test_dir)?;
        create_test_directory_structure(&test_dir)?;

        let mut disk_use = DiskUse::new(&cache_file);

        let size1 = disk_use.scan(&test_dir)?;
        assert_eq!(size1, 71);

        fs::write(test_dir.join("new_file.txt"), "New content")?;

        let _size2 = disk_use.scan(&test_dir)?;

        let size3 = disk_use.scan_with_options(&test_dir, true)?;
        assert_eq!(size3, 82);

        Ok(())
    }

    #[test]
    fn test_cache_management() -> io::Result<()> {
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
}
