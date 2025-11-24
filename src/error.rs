//! Error handling module with contextual error messages

use std::{fmt, io, path::PathBuf};

/// Custom error type that wraps IO errors with additional context
#[derive(Debug)]
pub enum DiskUseError {
    /// Error occurred while scanning a directory
    ScanError { path: PathBuf, source: io::Error },
    /// Error occurred while reading metadata
    MetadataError { path: PathBuf, source: io::Error },
    /// Error occurred while reading cache file
    CacheReadError { path: PathBuf, source: io::Error },
    /// Error occurred while writing cache file
    CacheWriteError { path: PathBuf, source: io::Error },
    /// Error occurred while serializing/deserializing cache
    CacheSerializationError { path: PathBuf, message: String },
    /// The specified path does not exist
    PathNotFound { path: PathBuf },
    /// The specified path is not accessible due to permissions
    PermissionDenied { path: PathBuf },
}

impl fmt::Display for DiskUseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DiskUseError::ScanError { path, source } => {
                write!(
                    f,
                    "Failed to scan directory '{}': {}",
                    path.display(),
                    get_user_friendly_error(source)
                )
            }
            DiskUseError::MetadataError { path, source } => {
                write!(
                    f,
                    "Failed to read metadata for '{}': {}",
                    path.display(),
                    get_user_friendly_error(source)
                )
            }
            DiskUseError::CacheReadError { path, source } => {
                write!(
                    f,
                    "Failed to read cache file '{}': {}",
                    path.display(),
                    get_user_friendly_error(source)
                )
            }
            DiskUseError::CacheWriteError { path, source } => {
                write!(
                    f,
                    "Failed to write cache file '{}': {}",
                    path.display(),
                    get_user_friendly_error(source)
                )
            }
            DiskUseError::CacheSerializationError { path, message } => {
                write!(
                    f,
                    "Failed to serialize/deserialize cache file '{}': {}",
                    path.display(),
                    message
                )
            }
            DiskUseError::PathNotFound { path } => {
                write!(f, "Path '{}' does not exist", path.display())
            }
            DiskUseError::PermissionDenied { path } => {
                write!(f, "Permission denied when accessing '{}'", path.display())
            }
        }
    }
}

impl std::error::Error for DiskUseError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            DiskUseError::ScanError { source, .. }
            | DiskUseError::MetadataError { source, .. }
            | DiskUseError::CacheReadError { source, .. }
            | DiskUseError::CacheWriteError { source, .. } => Some(source),
            _ => None,
        }
    }
}

/// Convert DiskUseError to io::Error for backward compatibility
impl From<DiskUseError> for io::Error {
    fn from(err: DiskUseError) -> io::Error {
        let err_string = err.to_string();
        match err {
            DiskUseError::PathNotFound { .. } => {
                io::Error::new(io::ErrorKind::NotFound, err_string)
            }
            DiskUseError::PermissionDenied { .. } => {
                io::Error::new(io::ErrorKind::PermissionDenied, err_string)
            }
            DiskUseError::ScanError { source, .. }
            | DiskUseError::MetadataError { source, .. }
            | DiskUseError::CacheReadError { source, .. }
            | DiskUseError::CacheWriteError { source, .. } => {
                io::Error::new(source.kind(), err_string)
            }
            DiskUseError::CacheSerializationError { .. } => {
                io::Error::new(io::ErrorKind::InvalidData, err_string)
            }
        }
    }
}

/// Provide user-friendly error messages for common IO error kinds
fn get_user_friendly_error(err: &io::Error) -> String {
    match err.kind() {
        io::ErrorKind::NotFound => "The path does not exist".to_string(),
        io::ErrorKind::PermissionDenied => {
            "Permission denied. You may need elevated privileges to access this location."
                .to_string()
        }
        io::ErrorKind::InvalidInput => "Invalid path or filename".to_string(),
        io::ErrorKind::OutOfMemory => "Out of memory".to_string(),
        _ => {
            // Check for specific error messages in the error string
            let err_str = err.to_string().to_lowercase();
            if err_str.contains("quota") || err_str.contains("no space") {
                "Disk quota exceeded or insufficient disk space".to_string()
            } else if err_str.contains("read-only") {
                "The filesystem is read-only".to_string()
            } else if err_str.contains("device") {
                "Device is not available or not ready".to_string()
            } else if err_str.contains("stale") {
                "Stale file handle (remote filesystem may be unavailable)".to_string()
            } else {
                format!("{}", err)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = DiskUseError::PathNotFound {
            path: PathBuf::from("/nonexistent"),
        };
        assert_eq!(err.to_string(), "Path '/nonexistent' does not exist");

        let err = DiskUseError::PermissionDenied {
            path: PathBuf::from("/root/secret"),
        };
        assert_eq!(
            err.to_string(),
            "Permission denied when accessing '/root/secret'"
        );
    }

    #[test]
    fn test_user_friendly_errors() {
        let err = io::Error::new(io::ErrorKind::NotFound, "test");
        assert_eq!(get_user_friendly_error(&err), "The path does not exist");

        let err = io::Error::new(io::ErrorKind::PermissionDenied, "test");
        assert!(get_user_friendly_error(&err).contains("Permission denied"));
    }

    #[test]
    fn test_disk_quota_detection() {
        let err = io::Error::other("Disk quota exceeded");
        assert!(get_user_friendly_error(&err).contains("Disk quota exceeded"));

        let err = io::Error::other("No space left on device");
        assert!(get_user_friendly_error(&err).contains("quota"));
    }
}
