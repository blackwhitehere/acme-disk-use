//! Directory scanning module for calculating disk usage statistics

use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fs, io,
    path::{Path, PathBuf},
    time::SystemTime,
};

#[cfg(unix)]
use std::os::unix::fs::MetadataExt;

/// Get the physical size of a file on disk in bytes
///
/// On Unix systems, this uses the `blocks` metadata field multiplied by 512
/// to get the actual disk usage, which accounts for sparse files and block alignment.
/// On non-Unix systems, it falls back to the logical file size.
fn get_block_size(meta: &fs::Metadata) -> u64 {
    #[cfg(unix)]
    {
        meta.blocks() * 512
    }
    #[cfg(not(unix))]
    {
        meta.len()
    }
}

/// Statistics for a directory and its contents
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DirStat {
    pub(crate) path: PathBuf,                       // Directory path
    pub(crate) total_size: u64,                     // Logical sum of st_size of all files
    pub(crate) file_count: u64, // Number of files in this directory and subdirectories
    pub(crate) last_scan: SystemTime, // When this subtree was last scanned
    pub(crate) children: HashMap<PathBuf, DirStat>, // Child directories' stats
}

impl DirStat {
    /// Get the total size of this directory
    pub fn total_size(&self) -> u64 {
        self.total_size
    }

    /// Get the file count in this directory
    pub fn file_count(&self) -> u64 {
        self.file_count
    }

    /// Get the last scan time
    pub fn last_scan(&self) -> SystemTime {
        self.last_scan
    }

    /// Get the path of this directory
    pub fn path(&self) -> &Path {
        &self.path
    }
}

/// Check if a directory or any of its subdirectories have been modified
///
/// Uses a recursive mtime comparison approach:
/// 1. Check if directory's own mtime > last_scan (files/dirs added/removed)
/// 2. Recursively validate cached subdirectories
///
/// If files were added or removed in subdirectories, the
/// directory mtime would have been updated by the OS.
fn dir_changed_since_last_scan(path: &Path, cached: &DirStat) -> bool {
    // Check if the directory itself was modified
    match fs::metadata(path).and_then(|m| m.modified()) {
        Ok(mtime) => {
            if mtime > cached.last_scan {
                return true;
            }
        }
        Err(_) => return true, // If we can't stat it, assume it changed (or is gone/inaccessible)
    }

    // If directory mtime hasn't changed, we assume no files were added/removed
    // at this level. However, subdirectories might have changed internally
    // without updating the parent's mtime.
    // Parallelize the check for children
    cached
        .children
        .par_iter()
        .any(|(child_path, child_stat)| dir_changed_since_last_scan(child_path, child_stat))
}

/// Scan a directory recursively and return statistics
///
/// # Arguments
/// * `path` - The directory path to scan
/// * `cache` - Optional cached statistics for this directory
///
/// # Returns
/// Directory statistics including size, file count, and child directories
pub fn scan_directory(path: &Path, cache: Option<&DirStat>) -> io::Result<DirStat> {
    // If cache exists, check if rescan needed BEFORE cloning
    if let Some(cached) = cache {
        // If directory hasn't changed, return the cached version
        // This avoids cloning if we are going to discard it anyway
        if !dir_changed_since_last_scan(path, cached) {
            return Ok(cached.clone());
        }
    }

    let mut total_size = 0;
    let mut file_count = 0;
    let mut children = HashMap::new();

    // Collect entries first for potential parallel processing
    let entries: Vec<_> = fs::read_dir(path)?.filter_map(|e| e.ok()).collect();

    // Process files and collect subdirectories
    let mut subdirs = Vec::new();

    for entry in entries {
        let entry_path = entry.path();
        if let Ok(meta) = entry.metadata() {
            if meta.is_file() {
                total_size += get_block_size(&meta);
                file_count += 1;
            } else if meta.is_dir() {
                subdirs.push(entry_path);
            }
        }
    }

    // Process subdirectories in parallel if we have multiple
    if subdirs.len() > 1 {
        let results: Vec<_> = subdirs
            .par_iter()
            .filter_map(|entry_path| {
                let child_cache = cache.and_then(|c| c.children.get(entry_path));
                scan_directory(entry_path, child_cache).ok()
            })
            .collect();

        for child_stat in results {
            total_size += child_stat.total_size;
            file_count += child_stat.file_count;
            children.insert(child_stat.path.clone(), child_stat);
        }
    } else {
        // Sequential processing for single subdirectory
        for entry_path in subdirs {
            let child_cache = cache.and_then(|c| c.children.get(&entry_path));
            if let Ok(child_stat) = scan_directory(&entry_path, child_cache) {
                total_size += child_stat.total_size;
                file_count += child_stat.file_count;
                children.insert(entry_path, child_stat);
            }
        }
    }

    Ok(DirStat {
        path: path.to_path_buf(),
        total_size,
        file_count,
        last_scan: SystemTime::now(),
        children,
    })
}

/// Count files in a directory recursively (without using cache)
pub fn count_files(path: &Path) -> io::Result<u64> {
    let mut count = 0;

    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let meta = entry.metadata()?;

        if meta.is_file() {
            count += 1;
        } else if meta.is_dir() {
            count += count_files(&entry.path())?;
        }
    }

    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_test_structure(base: &Path) -> io::Result<()> {
        fs::create_dir_all(base.join("subdir1"))?;
        fs::create_dir_all(base.join("subdir2/nested"))?;

        fs::write(base.join("file1.txt"), "Hello World")?; // 11 bytes
        fs::write(base.join("file2.txt"), "Test content")?; // 12 bytes
        fs::write(base.join("subdir1/nested_file.txt"), "Nested content here")?; // 19 bytes
        fs::write(base.join("subdir2/another.txt"), "More content")?; // 12 bytes
        fs::write(base.join("subdir2/nested/deep.txt"), "Deep file content")?; // 17 bytes

        Ok(())
    }

    #[test]
    fn test_scan_directory() -> io::Result<()> {
        // This test verifies that `scan_directory` correctly calculates the total size
        // and file count of a directory structure. It checks if the calculated size
        // is at least the logical size (accounting for block overhead) and if the
        // file count and subdirectory count match the expected values.
        let temp_dir = TempDir::new()?;
        let test_dir = temp_dir.path().join("test");
        fs::create_dir(&test_dir)?;

        create_test_structure(&test_dir)?;

        let result = scan_directory(&test_dir, None)?;

        // Expected total: 11 + 12 + 19 + 12 + 17 = 71 bytes (logical)
        // With block size, it will be larger.
        assert!(result.total_size() >= 71);
        assert_eq!(result.file_count(), 5);
        assert_eq!(result.children.len(), 2); // subdir1 and subdir2

        Ok(())
    }

    #[test]
    fn test_count_files() -> io::Result<()> {
        // This test verifies that `count_files` correctly counts the total number
        // of files in a directory tree recursively, without using any cache.
        let temp_dir = TempDir::new()?;
        let test_dir = temp_dir.path().join("test");
        fs::create_dir(&test_dir)?;

        create_test_structure(&test_dir)?;

        let count = count_files(&test_dir)?;
        assert_eq!(count, 5);

        Ok(())
    }

    #[test]
    fn test_scan_with_cache() -> io::Result<()> {
        // This test verifies that the caching mechanism works correctly.
        // It performs an initial scan, then a second scan with the cache.
        // It asserts that the second scan reuses the cached result (indicated by
        // the same `last_scan` timestamp) since the directory hasn't changed.
        let temp_dir = TempDir::new()?;
        let test_dir = temp_dir.path().join("test");
        fs::create_dir(&test_dir)?;

        create_test_structure(&test_dir)?;

        // First scan without cache
        let stats1 = scan_directory(&test_dir, None)?;
        let scan_time1 = stats1.last_scan();

        // Second scan with cache (should reuse if directory hasn't changed)
        let stats2 = scan_directory(&test_dir, Some(&stats1))?;
        let scan_time2 = stats2.last_scan();

        // Since directory hasn't changed, should return cached stats with same timestamp
        assert_eq!(scan_time1, scan_time2);

        Ok(())
    }

    #[test]
    fn test_detects_new_nested_subdirectory() -> io::Result<()> {
        // This test ensures that the scanner detects changes deep in the directory tree.
        // It creates a structure, scans it, then adds a new nested subdirectory and file.
        // It verifies that the subsequent scan detects the new file and updates the
        // `last_scan` timestamp, indicating a re-scan occurred.
        use std::thread::sleep;
        use std::time::Duration;

        let temp_dir = TempDir::new()?;
        let test_dir = temp_dir.path().join("test");
        fs::create_dir(&test_dir)?;

        // Create initial structure: test/a/
        fs::create_dir(test_dir.join("a"))?;
        fs::write(test_dir.join("a/file1.txt"), "content")?;

        // First scan
        let stats1 = scan_directory(&test_dir, None)?;
        assert_eq!(stats1.file_count(), 1);

        // Wait a moment to ensure time difference
        sleep(Duration::from_millis(10));

        // Now create test/a/b/ (this updates a's mtime but NOT test's mtime)
        fs::create_dir(test_dir.join("a/b"))?;
        fs::write(test_dir.join("a/b/file2.txt"), "new content")?;

        // Second scan with cache - should detect the new subdirectory
        let stats2 = scan_directory(&test_dir, Some(&stats1))?;

        // Should have scanned and found the new file
        assert_eq!(stats2.file_count(), 2);
        assert!(
            stats2.last_scan() > stats1.last_scan(),
            "Should have rescanned since new subdirectory was added"
        );

        Ok(())
    }

    #[test]
    fn test_detects_deleted_subdirectory() -> io::Result<()> {
        // This test ensures that the scanner detects when a subdirectory is deleted.
        // It creates a structure, scans it, then deletes a subdirectory.
        // It verifies that the subsequent scan correctly reports the reduced file count
        // and updates the `last_scan` timestamp.
        use std::thread::sleep;
        use std::time::Duration;

        let temp_dir = TempDir::new()?;
        let test_dir = temp_dir.path().join("test");
        fs::create_dir(&test_dir)?;

        // Create initial structure
        fs::create_dir(test_dir.join("a"))?;
        fs::create_dir(test_dir.join("b"))?;
        fs::write(test_dir.join("a/file1.txt"), "content")?;
        fs::write(test_dir.join("b/file2.txt"), "content")?;

        // First scan
        let stats1 = scan_directory(&test_dir, None)?;
        assert_eq!(stats1.file_count(), 2);

        // Wait a moment
        sleep(Duration::from_millis(10));

        // Delete subdirectory b
        fs::remove_file(test_dir.join("b/file2.txt"))?;
        fs::remove_dir(test_dir.join("b"))?;

        // Second scan with cache - should detect the deleted subdirectory
        let stats2 = scan_directory(&test_dir, Some(&stats1))?;

        // Should have rescanned and found only 1 file now
        assert_eq!(stats2.file_count(), 1);
        assert!(
            stats2.last_scan() > stats1.last_scan(),
            "Should have rescanned since subdirectory was deleted"
        );

        Ok(())
    }

    #[test]
    fn test_prunes_deeply_nested_deleted_directory() -> io::Result<()> {
        // This test verifies that the scanner correctly handles the deletion of
        // deeply nested directories. It creates a deep structure, scans it,
        // then deletes a middle part of the tree. It checks if the cache is
        // correctly updated to reflect the removal of the nested structure.
        use std::thread::sleep;
        use std::time::Duration;

        let temp_dir = TempDir::new()?;
        let test_dir = temp_dir.path().join("test");
        fs::create_dir(&test_dir)?;

        // Create deeply nested structure: test/a/b/c/d/
        fs::create_dir_all(test_dir.join("a/b/c/d"))?;
        fs::write(test_dir.join("a/file1.txt"), "content1")?;
        fs::write(test_dir.join("a/b/file2.txt"), "content2")?;
        fs::write(test_dir.join("a/b/c/file3.txt"), "content3")?;
        fs::write(test_dir.join("a/b/c/d/file4.txt"), "content4")?;

        // First scan
        let stats1 = scan_directory(&test_dir, None)?;
        assert_eq!(stats1.file_count(), 4);

        // Wait a moment
        sleep(Duration::from_millis(10));

        // Delete deeply nested directory c (and its child d)
        fs::remove_file(test_dir.join("a/b/c/d/file4.txt"))?;
        fs::remove_dir(test_dir.join("a/b/c/d"))?;
        fs::remove_file(test_dir.join("a/b/c/file3.txt"))?;
        fs::remove_dir(test_dir.join("a/b/c"))?;

        // Second scan with cache - should prune deleted dirs and update counts
        let stats2 = scan_directory(&test_dir, Some(&stats1))?;

        // Should have only 2 files now (file1.txt and file2.txt)
        assert_eq!(stats2.file_count(), 2);

        // Verify cache structure is updated (b should exist, but c and d should be gone)
        let a_stats = stats2.children.get(&test_dir.join("a")).unwrap();
        let b_stats = a_stats.children.get(&test_dir.join("a/b")).unwrap();
        assert!(
            !b_stats.children.contains_key(&test_dir.join("a/b/c")),
            "Deleted directory c should be pruned from cache"
        );

        Ok(())
    }
}
