//! Cache management module for storing and retrieving disk usage statistics

use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fs, io,
    path::{Path, PathBuf},
};

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
        if let Ok(bytes) = fs::read(cache_path) {
            if let Ok(cache) = bincode::deserialize::<Cache>(&bytes) {
                return cache;
            }
            // Fall back to JSON for backward compatibility
            if let Ok(s) = String::from_utf8(bytes) {
                if let Ok(cache) = serde_json::from_str(&s) {
                    return cache;
                }
            }
        }
        Cache::default()
    }

    /// Save cache to file using binary format
    pub fn save(&mut self) -> io::Result<()> {
        if !self.dirty {
            return Ok(()); // Skip if nothing changed
        }

        // Ensure parent directory exists
        if let Some(parent) = self.cache_path.parent() {
            fs::create_dir_all(parent)?;
        }

        // Serialize to binary format (much faster than JSON)
        let bytes = bincode::serialize(&self.cache)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

        fs::write(&self.cache_path, bytes)?;
        self.dirty = false;
        Ok(())
    }

    /// Get a cached directory stat by path
    pub fn get(&self, path: &Path) -> Option<&DirStat> {
        // Normalize path for lookup
        let lookup_path = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
        self.cache.roots.get(&lookup_path)
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
}
