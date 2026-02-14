use crate::error::{FlashError, Result};
use redb::{Database, ReadableTable, RedbValue, TableDefinition, TypeName};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::SystemTime;

const FILES_TABLE: TableDefinition<&str, FileMetadata> = TableDefinition::new("files");

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileMetadata {
    pub path: String,
    pub modified: u64,          // Unix timestamp
    pub size: u64,              // File size in bytes
    pub content_hash: [u8; 32], // Blake3 hash for content deduplication
    pub indexed_at: u64,        // When this file was last indexed
}

/// Connection metrics for monitoring
#[derive(Debug)]
pub struct ConnectionMetrics {
    pub read_operations: AtomicU64,
    pub write_operations: AtomicU64,
    pub bytes_read: AtomicU64,
    pub bytes_written: AtomicU64,
}

impl Default for ConnectionMetrics {
    fn default() -> Self {
        Self {
            read_operations: AtomicU64::new(0),
            write_operations: AtomicU64::new(0),
            bytes_read: AtomicU64::new(0),
            bytes_written: AtomicU64::new(0),
        }
    }
}

/// Snapshot of metrics for reporting
#[derive(Debug, Clone, Copy)]
pub struct ConnectionMetricsSnapshot {
    pub read_operations: u64,
    pub write_operations: u64,
    pub bytes_read: u64,
    pub bytes_written: u64,
}

/// Manages file metadata database using redb
/// Implements connection pooling pattern for redb (even though it's embedded)
/// to ensure proper resource management and monitoring
pub struct MetadataDb {
    db: Arc<Database>,
    metrics: Arc<ConnectionMetrics>,
}

impl MetadataDb {
    /// Open or create the metadata database
    pub fn open(db_path: &Path) -> Result<Self> {
        let db = Arc::new(
            Database::create(db_path).map_err(|e| FlashError::database("database_operation", "files_table", e.to_string()))?
        );

        // Create table if it doesn't exist
        let txn = db
            .begin_write()
            .map_err(|e| FlashError::database("database_operation", "files_table", e.to_string()))?;
        {
            let _table = txn
                .open_table(FILES_TABLE)
                .map_err(|e| FlashError::database("database_operation", "files_table", e.to_string()))?;
        }
        txn.commit()
            .map_err(|e| FlashError::database("database_operation", "files_table", e.to_string()))?;

        Ok(Self { 
            db,
            metrics: Arc::new(ConnectionMetrics::default()),
        })
    }

    /// Clone with shared state (for multi-threaded access)
    pub fn clone_for_thread(&self) -> Self {
        Self {
            db: Arc::clone(&self.db),
            metrics: Arc::clone(&self.metrics),
        }
    }

    /// Get current metrics snapshot
    pub fn get_metrics(&self) -> ConnectionMetricsSnapshot {
        ConnectionMetricsSnapshot {
            read_operations: self.metrics.read_operations.load(Ordering::Relaxed),
            write_operations: self.metrics.write_operations.load(Ordering::Relaxed),
            bytes_read: self.metrics.bytes_read.load(Ordering::Relaxed),
            bytes_written: self.metrics.bytes_written.load(Ordering::Relaxed),
        }
    }

    /// Check if file needs reindexing based on modification time and hash
    pub fn needs_reindex(&self, path: &Path, modified: u64, size: u64) -> Result<bool> {
        let txn = self
            .db
            .begin_read()
            .map_err(|e| FlashError::database("database_operation", "files_table", e.to_string()))?;

        let table = txn
            .open_table(FILES_TABLE)
            .map_err(|e| FlashError::database("database_operation", "files_table", e.to_string()))?;

        let path_str = path.to_str().unwrap_or("");

        let result = match table
            .get(path_str)
            .map_err(|e| FlashError::database("database_operation", "files_table", e.to_string()))?
        {
            Some(metadata) => {
                let meta = metadata.value();
                self.metrics.read_operations.fetch_add(1, Ordering::Relaxed);
                self.metrics.bytes_read.fetch_add(
                    std::mem::size_of::<FileMetadata>() as u64,
                    Ordering::Relaxed,
                );
                // Reindex if modification time or size changed
                meta.modified != modified || meta.size != size
            }
            None => true, // File not indexed yet
        };

        Ok(result)
    }

    /// Update file metadata after indexing
    pub fn update_metadata(
        &self,
        path: &Path,
        modified: u64,
        size: u64,
        content_hash: [u8; 32],
    ) -> Result<()> {
        let txn = self
            .db
            .begin_write()
            .map_err(|e| FlashError::database("database_operation", "files_table", e.to_string()))?;

        {
            let mut table = txn
                .open_table(FILES_TABLE)
                .map_err(|e| FlashError::database("database_operation", "files_table", e.to_string()))?;

            let metadata = FileMetadata {
                path: path.to_string_lossy().to_string(),
                modified,
                size,
                content_hash,
                indexed_at: SystemTime::now()
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs(),
            };

            table
                .insert(path.to_str().unwrap_or(""), metadata)
                .map_err(|e| FlashError::database("database_operation", "files_table", e.to_string()))?;
        }

        txn.commit()
            .map_err(|e| FlashError::database("database_operation", "files_table", e.to_string()))?;

        Ok(())
    }

    /// Get metadata for a specific file
    pub fn get_metadata(&self, path: &Path) -> Result<Option<FileMetadata>> {
        let txn = self
            .db
            .begin_read()
            .map_err(|e| FlashError::database("database_operation", "files_table", e.to_string()))?;

        let table = txn
            .open_table(FILES_TABLE)
            .map_err(|e| FlashError::database("database_operation", "files_table", e.to_string()))?;

        let result = match table
            .get(path.to_str().unwrap_or(""))
            .map_err(|e| FlashError::database("database_operation", "files_table", e.to_string()))?
        {
            Some(metadata) => Some(metadata.value()),
            None => None,
        };

        Ok(result)
    }

    /// Batch update metadata for multiple files (much more efficient)
    /// Updates all files in a single transaction to minimize I/O overhead
    pub fn batch_update_metadata(
        &self,
        entries: &[(String, u64, u64, [u8; 32])], // (path, modified, size, hash)
    ) -> Result<usize> {
        if entries.is_empty() {
            return Ok(0);
        }

        let txn = self
            .db
            .begin_write()
            .map_err(|e| FlashError::database("database_operation", "files_table", e.to_string()))?;

        let indexed_at = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let mut total_bytes_written = 0u64;

        {
            let mut table = txn
                .open_table(FILES_TABLE)
                .map_err(|e| FlashError::database("database_operation", "files_table", e.to_string()))?;

            for (path, modified, size, content_hash) in entries {
                let metadata = FileMetadata {
                    path: path.clone(),
                    modified: *modified,
                    size: *size,
                    content_hash: *content_hash,
                    indexed_at,
                };

                total_bytes_written += std::mem::size_of::<FileMetadata>() as u64;
                total_bytes_written += path.len() as u64;

                table
                    .insert(path.as_str(), metadata)
                    .map_err(|e| FlashError::database("database_operation", "files_table", e.to_string()))?;
            }
        }

        txn.commit()
            .map_err(|e| FlashError::database("database_operation", "files_table", e.to_string()))?;

        // Update metrics
        self.metrics.write_operations.fetch_add(1, Ordering::Relaxed);
        self.metrics.bytes_written.fetch_add(total_bytes_written, Ordering::Relaxed);

        Ok(entries.len())
    }

    /// Batch check which files need reindexing
    /// Returns a vector of booleans indicating if each file needs reindexing
    pub fn batch_needs_reindex(
        &self,
        entries: &[(String, u64, u64)], // (path, modified, size)
    ) -> Result<Vec<bool>> {
        if entries.is_empty() {
            return Ok(vec![]);
        }

        let txn = self
            .db
            .begin_read()
            .map_err(|e| FlashError::database("database_operation", "files_table", e.to_string()))?;

        let table = txn
            .open_table(FILES_TABLE)
            .map_err(|e| FlashError::database("database_operation", "files_table", e.to_string()))?;

        let results: Vec<bool> = entries
            .iter()
            .map(|(path, modified, size)| {
                match table.get(path.as_str()) {
                    Ok(Some(metadata)) => {
                        let meta = metadata.value();
                        meta.modified != *modified || meta.size != *size
                    }
                    Ok(None) => true, // File not indexed yet
                    Err(_) => true,   // Error reading, safer to reindex
                }
            })
            .collect();

        Ok(results)
    }

    /// Get recently modified files sorted by modification time
    pub fn get_recent_files(&self, limit: usize) -> Result<Vec<(String, Option<String>, u64, u64)>> {
        let txn = self
            .db
            .begin_read()
            .map_err(|e| FlashError::database("database_operation", "files_table", e.to_string()))?;

        let table = txn
            .open_table(FILES_TABLE)
            .map_err(|e| FlashError::database("database_operation", "files_table", e.to_string()))?;

        let mut files: Vec<(String, u64, u64)> = table
            .iter()
            .map_err(|e| FlashError::database("database_operation", "files_table", e.to_string()))?
            .filter_map(|entry| {
                entry.ok().map(|(k, v)| {
                    let metadata = v.value();
                    (k.value().to_string(), metadata.modified, metadata.size)
                })
            })
            .collect();

        // Sort by modification time descending
        files.sort_by(|a, b| b.1.cmp(&a.1));
        files.truncate(limit);

        // Convert to the expected format (without titles for now, can be enhanced)
        Ok(files
            .into_iter()
            .map(|(path, modified, size)| (path, None, modified, size))
            .collect())
    }
}

// Implement RedbValue for FileMetadata to store in redb
impl RedbValue for FileMetadata {
    type SelfType<'a> = FileMetadata;
    type AsBytes<'a> = Vec<u8>;

    fn fixed_width() -> Option<usize> {
        None
    }

    fn from_bytes<'a>(data: &'a [u8]) -> Self::SelfType<'a>
    where
        Self: 'a,
    {
        bincode::deserialize(data).expect("Failed to deserialize FileMetadata")
    }

    fn as_bytes<'a, 'b: 'a>(value: &Self::SelfType<'b>) -> Self::AsBytes<'a>
    where
        Self: 'a,
        Self: 'b,
    {
        bincode::serialize(value).expect("Failed to serialize FileMetadata")
    }

    fn type_name() -> TypeName {
        TypeName::new("FileMetadata")
    }
}
