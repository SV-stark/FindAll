use std::path::PathBuf;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum FlashError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Parse error in {path}: {cause}")]
    Parse { path: PathBuf, cause: String },

    #[error("Index error: {msg}")]
    Index { msg: String, field: Option<String> },

    #[error("Database error: {operation} on {key}: {cause}")]
    Database {
        operation: String,
        key: String,
        cause: String,
    },

    #[error("Unsupported file format: {format}")]
    UnsupportedFormat { format: String, extension: String },

    #[error("File corrupted or encrypted: {path}")]
    CorruptedFile { path: PathBuf, operation: String },

    #[error("Search error: query '{query}' failed: {cause}")]
    Search { query: String, cause: String },

    #[error("Config error: {key}: {cause}")]
    Config { key: String, cause: String },

    #[error("Archive error: {archive_type} - {operation}: {cause}")]
    Archive {
        archive_type: String,
        operation: String,
        cause: String,
    },

    #[error("Lock poisoned: {lock_name}")]
    PoisonedLock { lock_name: String },

    #[error("Validation error: {field} - {reason}")]
    Validation { field: String, reason: String },

    #[error("Timeout: {operation} did not complete within {timeout_ms}ms")]
    Timeout { operation: String, timeout_ms: u64 },

    #[error("Not found: {resource} '{identifier}'")]
    NotFound {
        resource: String,
        identifier: String,
    },

    #[error("Permission denied: {operation} on {path}")]
    Permission { operation: String, path: PathBuf },

    #[error("Network error: {status} {url}")]
    Network { status: u16, url: String },

    #[error("Concurrent modification: {resource} was modified by another operation")]
    ConcurrentModification { resource: String },
}

pub type Result<T> = std::result::Result<T, FlashError>;

// Helper constructors for common error patterns
impl FlashError {
    pub fn parse<P: Into<PathBuf>, S: Into<String>>(path: P, cause: S) -> Self {
        Self::Parse {
            path: path.into(),
            cause: cause.into(),
        }
    }

    pub fn index<S: Into<String>>(msg: S) -> Self {
        Self::Index {
            msg: msg.into(),
            field: None,
        }
    }

    pub fn index_field<S: Into<String>>(field: S, msg: S) -> Self {
        Self::Index {
            msg: msg.into(),
            field: Some(field.into()),
        }
    }

    pub fn database<S: Into<String>, S2: Into<String>>(operation: S, key: S, cause: S2) -> Self {
        Self::Database {
            operation: operation.into(),
            key: key.into(),
            cause: cause.into(),
        }
    }

    pub fn unsupported_format<S: Into<String>>(format: S, extension: S) -> Self {
        Self::UnsupportedFormat {
            format: format.into(),
            extension: extension.into(),
        }
    }

    pub fn corrupted_file<P: Into<PathBuf>, S: Into<String>>(path: P, operation: S) -> Self {
        Self::CorruptedFile {
            path: path.into(),
            operation: operation.into(),
        }
    }

    pub fn search<S1: Into<String>, S2: Into<String>>(query: S1, cause: S2) -> Self {
        Self::Search {
            query: query.into(),
            cause: cause.into(),
        }
    }

    pub fn config<S1: Into<String>, S2: Into<String>>(key: S1, cause: S2) -> Self {
        Self::Config {
            key: key.into(),
            cause: cause.into(),
        }
    }

    pub fn archive<S1: Into<String>, S2: Into<String>, S3: Into<String>>(
        archive_type: S1,
        operation: S2,
        cause: S3,
    ) -> Self {
        Self::Archive {
            archive_type: archive_type.into(),
            operation: operation.into(),
            cause: cause.into(),
        }
    }

    pub fn poisoned_lock<S: Into<String>>(lock_name: S) -> Self {
        Self::PoisonedLock {
            lock_name: lock_name.into(),
        }
    }

    pub fn validation<S1: Into<String>, S2: Into<String>>(field: S1, reason: S2) -> Self {
        Self::Validation {
            field: field.into(),
            reason: reason.into(),
        }
    }

    pub fn timeout<S: Into<String>>(operation: S, timeout_ms: u64) -> Self {
        Self::Timeout {
            operation: operation.into(),
            timeout_ms,
        }
    }

    pub fn not_found<S: Into<String>>(resource: S, identifier: S) -> Self {
        Self::NotFound {
            resource: resource.into(),
            identifier: identifier.into(),
        }
    }

    pub fn permission<S: Into<String>, P: Into<PathBuf>>(operation: S, path: P) -> Self {
        Self::Permission {
            operation: operation.into(),
            path: path.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = FlashError::parse("/test/file.pdf", "Invalid PDF structure");
        assert_eq!(
            err.to_string(),
            "Parse error in /test/file.pdf: Invalid PDF structure"
        );
    }

    #[test]
    fn test_index_error() {
        let err = FlashError::index_field("content", "Field not found in schema");
        assert_eq!(err.to_string(), "Index error: Field not found in schema");
    }

    #[test]
    fn test_database_error() {
        let err = FlashError::database("get_metadata", "/test/file.txt", "Key not found");
        assert_eq!(
            err.to_string(),
            "Database error: get_metadata on /test/file.txt: Key not found"
        );
    }

    #[test]
    fn test_helper_constructors() {
        let err = FlashError::unsupported_format("PDF", "pdf");
        matches!(err, FlashError::UnsupportedFormat { .. });
    }

    #[test]
    fn test_io_conversion() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "File not found");
        let flash_err: FlashError = io_err.into();
        matches!(flash_err, FlashError::Io(_));
    }
}
