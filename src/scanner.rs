//! Directory scanning module for calculating disk usage statistics

use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fs, io,
    path::{Path, PathBuf},
    time::SystemTime,
};

/// Statistics for a directory and its contents
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DirStat {
    pub(crate) path: PathBuf,
    pub(crate) total_size: u64, // Logical sum of st_size of all files
    pub(crate) file_count: u64,
    pub(crate) last_scan: SystemTime, // When this subtree was last scanned
    pub(crate) children: HashMap<PathBuf, DirStat>,
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

/// Prune deleted directories from the cache recursively
///
/// Removes any child DirStat entries whose paths no longer exist on disk.
/// Returns true if any deletions were found and pruned.
fn prune_deleted_dirs(cached: &mut DirStat) -> bool {
    let mut found_deletions = false;

    // Check direct children for deletions
    cached.children.retain(|child_path, child_stat| {
        if !child_path.exists() {
            found_deletions = true;
            false // Remove this entry
        } else {
            // Recursively prune this child's children
            if prune_deleted_dirs(child_stat) {
                found_deletions = true;
            }
            true // Keep this entry
        }
    });

    found_deletions
}

/// Check if a directory or any of its subdirectories have been modified
///
/// Assumes deleted directories have already been pruned via prune_deleted_dirs.
/// Uses a recursive mtime comparison approach:
/// 1. Check if directory's own mtime > last_scan (files/dirs added/removed)
/// 2. Check if any subdirectory's mtime > last_scan (changes within subdirs)
/// 3. Recursively validate cached subdirectories
fn dir_changed_since_last_scan(path: &Path, cached: &DirStat) -> bool {
    // Check if the directory itself was modified
    match fs::metadata(path).and_then(|m| m.modified()) {
        Ok(mtime) => {
            if mtime > cached.last_scan {
                return true;
            }
        }
        Err(_) => return true,
    }

    // Check if nested subdirectories are added that do not update mtime
    match fs::read_dir(path) {
        Ok(entries) => {
            for entry in entries.flatten() {
                let entry_path = entry.path();

                if let Ok(meta) = entry.metadata() {
                    if meta.is_dir() {
                        // Check if this directory's mtime is newer than our last scan
                        if let Ok(dir_mtime) = meta.modified() {
                            if dir_mtime > cached.last_scan {
                                return true;
                            }
                        }

                        // Handle edge case that when nested subdirectories are added that do not update mtime
                        // only for cached children as uncached children would be caught above by mtime check
                        if let Some(child_cache) = cached.children.get(&entry_path) {
                            if dir_changed_since_last_scan(&entry_path, child_cache) {
                                return true;
                            }
                        }
                    }
                }
            }
        }
        Err(_) => return true,
    }

    false
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
    // If cache exists, first prune deleted directories, then check if rescan needed
    if let Some(cached) = cache {
        let mut pruned_cache = cached.clone();
        let had_deletions = prune_deleted_dirs(&mut pruned_cache);

        // If we found deletions, we need to recalculate totals from remaining children
        if had_deletions {
            // Recalculate total_size and file_count from remaining children
            let mut total_size = 0;
            let mut file_count = 0;

            for child in pruned_cache.children.values() {
                total_size += child.total_size;
                file_count += child.file_count;
            }

            // Count files at this level (not in subdirs)
            if let Ok(entries) = fs::read_dir(path) {
                for entry in entries.flatten() {
                    if let Ok(meta) = entry.metadata() {
                        if meta.is_file() {
                            total_size += meta.len();
                            file_count += 1;
                        }
                    }
                }
            }

            pruned_cache.total_size = total_size;
            pruned_cache.file_count = file_count;
            pruned_cache.last_scan = SystemTime::now();
        }

        // Now check if directory changed (excluding deletion checks)
        if !dir_changed_since_last_scan(path, &pruned_cache) {
            return Ok(pruned_cache);
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
                total_size += meta.len();
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
        let temp_dir = TempDir::new()?;
        let test_dir = temp_dir.path().join("test");
        fs::create_dir(&test_dir)?;

        create_test_structure(&test_dir)?;

        let result = scan_directory(&test_dir, None)?;

        // Expected total: 11 + 12 + 19 + 12 + 17 = 71 bytes
        assert_eq!(result.total_size(), 71);
        assert_eq!(result.file_count(), 5);
        assert_eq!(result.children.len(), 2); // subdir1 and subdir2

        Ok(())
    }

    #[test]
    fn test_count_files() -> io::Result<()> {
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
