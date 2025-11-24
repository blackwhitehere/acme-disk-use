//! Cache management module for storing and retrieving disk usage statistics

use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fs, io,
    path::{Path, PathBuf},
};

use crate::error::DiskUseError;
use crate::scanner::DirStat;

/// Cache structure for storing multiple directory scan results
#[derive(Serialize, Deserialize, Debug, Default)]
pub(crate) struct Cache {
    pub(crate) roots: HashMap<PathBuf, DirStat>,
    pub(crate) version: u32,
}

/// Public interface for cache operations with lazy writing
pub struct CacheManager {
    cache: Cache,
    cache_path: PathBuf,
    dirty: bool, // Track if cache needs to be saved
}

impl CacheManager {
    /// Create a new cache manager with specified path
    pub fn new(cache_path: impl AsRef<Path>) -> Self {
        let cache_path = cache_path.as_ref().to_path_buf();
        let cache = Self::load_from_file(&cache_path);

        Self {
            cache,
            cache_path,
            dirty: false,
        }
    }

    /// Load cache from file using binary format (falls back to JSON for compatibility)
    fn load_from_file(cache_path: &Path) -> Cache {
        // Try binary format first (new format)
        match fs::read(cache_path) {
            Ok(bytes) => {
                if let Ok(cache) = bincode::deserialize::<Cache>(&bytes) {
                    return cache;
                }
                // Fall back to JSON for backward compatibility
                if let Ok(s) = String::from_utf8(bytes) {
                    if let Ok(cache) = serde_json::from_str(&s) {
                        return cache;
                    }
                }
                eprintln!(
                    "Warning: Cache file '{}' is corrupted, starting with empty cache",
                    cache_path.display()
                );
                Cache::default()
            }
            Err(err) => {
                // Only log if it's not a "not found" error (expected on first run)
                if err.kind() != io::ErrorKind::NotFound {
                    let disk_err = DiskUseError::CacheReadError {
                        path: cache_path.to_path_buf(),
                        source: err,
                    };
                    eprintln!("Warning: {}", disk_err);
                }
                Cache::default()
            }
        }
    }

    /// Save cache to file using binary format
    pub fn save(&mut self) -> io::Result<()> {
        if !self.dirty {
            return Ok(()); // Skip if nothing changed
        }

        // Ensure parent directory exists
        if let Some(parent) = self.cache_path.parent() {
            fs::create_dir_all(parent).map_err(|err| {
                io::Error::from(DiskUseError::CacheWriteError {
                    path: parent.to_path_buf(),
                    source: err,
                })
            })?;
        }

        // Serialize to binary format (much faster than JSON)
        let bytes = bincode::serialize(&self.cache).map_err(|e| {
            io::Error::from(DiskUseError::CacheSerializationError {
                path: self.cache_path.clone(),
                message: e.to_string(),
            })
        })?;

        fs::write(&self.cache_path, bytes).map_err(|err| {
            io::Error::from(DiskUseError::CacheWriteError {
                path: self.cache_path.clone(),
                source: err,
            })
        })?;

        self.dirty = false;
        Ok(())
    }

    /// Get a cached directory stat by path
    pub fn get(&self, path: &Path) -> Option<&DirStat> {
        // Normalize path for lookup
        let lookup_path = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());

        // 1. Try direct lookup in roots
        if let Some(stat) = self.cache.roots.get(&lookup_path) {
            return Some(stat);
        }

        // 2. Search inside roots
        // Find the root that is a parent of lookup_path with the longest path
        let mut best_root: Option<&DirStat> = None;

        for root_stat in self.cache.roots.values() {
            if lookup_path.starts_with(&root_stat.path) {
                match best_root {
                    None => best_root = Some(root_stat),
                    Some(current_best) => {
                        // Pick the more specific root (longer path)
                        if root_stat.path.components().count()
                            > current_best.path.components().count()
                        {
                            best_root = Some(root_stat);
                        }
                    }
                }
            }
        }

        // If we found a containing root, traverse down to find the target
        if let Some(mut current) = best_root {
            // We know lookup_path starts with current.path
            if let Ok(relative) = lookup_path.strip_prefix(&current.path) {
                let mut path_so_far = current.path.clone();

                for component in relative.components() {
                    path_so_far.push(component);

                    // Try to find the next child
                    if let Some(child) = current.children.get(&path_so_far) {
                        current = child;
                    } else {
                        // Path diverges from cache
                        return None;
                    }
                }

                // If we consumed all components, we found it
                return Some(current);
            }
        }

        None
    }

    /// Insert or update a directory stat in the cache
    /// Path is automatically canonicalized to ensure consistent lookups
    #[allow(dead_code)]
    pub fn insert(&mut self, path: PathBuf, stats: DirStat) {
        // Canonicalize the path before storing to ensure consistent lookups
        let canonical_path = path.canonicalize().unwrap_or(path);
        self.cache.roots.insert(canonical_path, stats);
        self.dirty = true;
    }

    /// Update an existing entry with new stats
    /// This is just a convenience wrapper around insert
    pub fn update(&mut self, path: &Path, new_stats: DirStat) {
        self.insert(path.to_path_buf(), new_stats);
    }

    /// Clear all cache contents
    pub fn clear(&mut self) -> io::Result<()> {
        self.cache = Cache::default();
        self.dirty = true;
        self.save()
    }

    /// Delete the cache file
    pub fn delete(&self) -> io::Result<()> {
        if self.cache_path.exists() {
            fs::remove_file(&self.cache_path)
        } else {
            Ok(())
        }
    }

    /// Get the cache file path
    pub fn path(&self) -> &Path {
        &self.cache_path
    }
}

// Implement Drop to auto-save on destruction
impl Drop for CacheManager {
    fn drop(&mut self) {
        if self.dirty {
            // Try to save, but don't panic if it fails
            let _ = self.save();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::SystemTime;
    use tempfile::TempDir;

    #[test]
    fn test_cache_manager_basic_operations() -> io::Result<()> {
        // This test verifies the basic operations of the CacheManager:
        // 1. Inserting a new entry into the cache.
        // 2. Retrieving an entry from the cache.
        // 3. Saving the cache to disk.
        // 4. Loading the cache from disk and verifying the data persists.
        let temp_dir = TempDir::new()?;
        let cache_file = temp_dir.path().join("test_cache.json");

        let mut cache_mgr = CacheManager::new(&cache_file);

        // Test insert
        let test_stat = DirStat {
            path: PathBuf::from("/test/path"),
            total_size: 1000,
            file_count: 10,
            last_scan: SystemTime::now(),
            children: HashMap::new(),
        };

        cache_mgr.insert(PathBuf::from("/test/path"), test_stat.clone());

        // Test get
        let retrieved = cache_mgr.get(Path::new("/test/path"));
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().total_size, 1000);
        assert_eq!(retrieved.unwrap().file_count, 10);

        // Test save
        cache_mgr.save()?;
        assert!(cache_file.exists());

        // Test loading from file
        let cache_mgr2 = CacheManager::new(&cache_file);
        let retrieved2 = cache_mgr2.get(Path::new("/test/path"));
        assert!(retrieved2.is_some());
        assert_eq!(retrieved2.unwrap().total_size, 1000);

        Ok(())
    }

    #[test]
    fn test_cache_clear_and_delete() -> io::Result<()> {
        // This test verifies the cache cleanup operations:
        // 1. `clear()`: Should remove all entries from the in-memory cache.
        // 2. `delete()`: Should remove the cache file from the disk.
        let temp_dir = TempDir::new()?;
        let cache_file = temp_dir.path().join("test_cache.json");

        let mut cache_mgr = CacheManager::new(&cache_file);

        let test_stat = DirStat {
            path: PathBuf::from("/test"),
            total_size: 500,
            file_count: 5,
            last_scan: SystemTime::now(),
            children: HashMap::new(),
        };

        cache_mgr.insert(PathBuf::from("/test"), test_stat);
        cache_mgr.save()?;

        // Test clear
        cache_mgr.clear()?;
        assert!(cache_mgr.get(Path::new("/test")).is_none());

        // Test delete
        cache_mgr.delete()?;
        assert!(!cache_file.exists());

        Ok(())
    }

    #[test]
    fn test_get_nested_path() -> io::Result<()> {
        // This test verifies the nested path retrieval logic.
        // It creates a cache with a root directory that contains nested children.
        // It then attempts to retrieve stats for the children directly using `get()`.
        // This ensures that `get()` can traverse the cached tree structure to find
        // subdirectories even if they are not top-level roots.
        let temp_dir = TempDir::new()?;
        let cache_file = temp_dir.path().join("test_cache.json");
        let mut cache_mgr = CacheManager::new(&cache_file);

        // Create a nested structure
        // /root
        //   /root/child
        //     /root/child/grandchild

        let grandchild_path = PathBuf::from("/root/child/grandchild");
        let grandchild_stat = DirStat {
            path: grandchild_path.clone(),
            total_size: 10,
            file_count: 1,
            last_scan: SystemTime::now(),
            children: HashMap::new(),
        };

        let child_path = PathBuf::from("/root/child");
        let mut child_stat = DirStat {
            path: child_path.clone(),
            total_size: 20,
            file_count: 2,
            last_scan: SystemTime::now(),
            children: HashMap::new(),
        };
        child_stat
            .children
            .insert(grandchild_path.clone(), grandchild_stat);

        let root_path = PathBuf::from("/root");
        let mut root_stat = DirStat {
            path: root_path.clone(),
            total_size: 30,
            file_count: 3,
            last_scan: SystemTime::now(),
            children: HashMap::new(),
        };
        root_stat.children.insert(child_path.clone(), child_stat);

        cache_mgr.insert(root_path, root_stat);

        // Test retrieving nested paths
        let retrieved_child = cache_mgr.get(Path::new("/root/child"));
        assert!(retrieved_child.is_some());
        assert_eq!(retrieved_child.unwrap().total_size, 20);

        let retrieved_grandchild = cache_mgr.get(Path::new("/root/child/grandchild"));
        assert!(retrieved_grandchild.is_some());
        assert_eq!(retrieved_grandchild.unwrap().total_size, 10);

        // Test non-existent path
        assert!(cache_mgr.get(Path::new("/root/nonexistent")).is_none());
        assert!(cache_mgr
            .get(Path::new("/root/child/nonexistent"))
            .is_none());

        Ok(())
    }

    #[test]
    fn test_cache_write_to_readonly_location() {
        // Test that writing cache to a readonly location fails gracefully
        // Note: This test may not work on all platforms or may require special setup
        let cache_file = PathBuf::from("/dev/null/cannot_write_here");
        let mut cache_mgr = CacheManager::new(&cache_file);

        let test_stat = DirStat {
            path: PathBuf::from("/test"),
            total_size: 100,
            file_count: 1,
            last_scan: SystemTime::now(),
            children: HashMap::new(),
        };

        cache_mgr.insert(PathBuf::from("/test"), test_stat);

        // Try to save - should fail gracefully
        let result = cache_mgr.save();
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.to_string().contains("Failed to write cache file")
                || err.to_string().contains("Failed to serialize"),
            "Error message should be descriptive: {}",
            err
        );
    }
}
