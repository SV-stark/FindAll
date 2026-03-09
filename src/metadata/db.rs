use crate::error::{FlashError, Result};
use redb::{Database, ReadableTable, TableDefinition};
use rkyv;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::SystemTime;

const FILES_TABLE: TableDefinition<&str, &[u8]> = TableDefinition::new("files");

#[derive(Debug, Clone, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
pub struct FileMetadata {
    pub path: String,
    pub modified: u64,          // Unix timestamp
    pub size: u64,              // File size in bytes
    pub content_hash: [u8; 32], // Blake3 hash for content deduplication
    pub indexed_at: u64,        // When this file was last indexed
}

pub type RecentFileEntry = (String, Option<String>, u64, u64);

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
        let db = match Database::create(db_path) {
            Ok(db) => Arc::new(db),
            Err(e) => {
                tracing::warn!("Failed to open metadata database: {}. Forcing reset...", e);
                let _ = std::fs::remove_file(db_path);
                Arc::new(Database::create(db_path).map_err(|e| {
                    FlashError::database("database_operation", "files_table", e.to_string())
                })?)
            }
        };

        // Create table if it doesn't exist
        // Wrap this in a closure to easily catch errors and retry
        let init_table = |db: &Database| -> Result<()> {
            let txn = db.begin_write().map_err(|e| {
                FlashError::database("database_operation", "files_table", e.to_string())
            })?;
            {
                let _table = txn.open_table(FILES_TABLE).map_err(|e| {
                    FlashError::database("database_operation", "files_table", e.to_string())
                })?;
            }
            txn.commit().map_err(|e| {
                FlashError::database("database_operation", "files_table", e.to_string())
            })
        };

        if let Err(e) = init_table(&db) {
            tracing::warn!(
                "Failed to initialize database tables: {}. Wiping and recreating...",
                e
            );
            drop(db); // Ensure file is not locked
            let _ = std::fs::remove_file(db_path);

            let db = Arc::new(Database::create(db_path).map_err(|e| {
                FlashError::database("database_operation", "files_table", e.to_string())
            })?);

            init_table(&db).map_err(|e| {
                FlashError::database(
                    "database_operation",
                    "files_table",
                    format!("Retry failed: {}", e),
                )
            })?;

            return Ok(Self {
                db,
                metrics: Arc::new(ConnectionMetrics::default()),
            });
        }

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
        let txn = self.db.begin_read().map_err(|e| {
            FlashError::database("database_operation", "files_table", e.to_string())
        })?;

        let table = txn.open_table(FILES_TABLE).map_err(|e| {
            FlashError::database("database_operation", "files_table", e.to_string())
        })?;

        let path_str = path.to_str().unwrap_or("");

        let result = match table
            .get(path_str)
            .map_err(|e| FlashError::database("database_operation", "files_table", e.to_string()))?
        {
            Some(metadata) => {
                let bytes = metadata.value();
                self.metrics.read_operations.fetch_add(1, Ordering::Relaxed);
                self.metrics
                    .bytes_read
                    .fetch_add(bytes.len() as u64, Ordering::Relaxed);
                // Zero-copy read and validate
                if let Ok(meta) =
                    rkyv::access::<rkyv::Archived<FileMetadata>, rkyv::rancor::Error>(bytes)
                {
                    meta.modified != modified || meta.size != size
                } else {
                    true // Reindex if validation fails
                }
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
        let txn = self.db.begin_write().map_err(|e| {
            FlashError::database("database_operation", "files_table", e.to_string())
        })?;

        {
            let mut table = txn.open_table(FILES_TABLE).map_err(|e| {
                FlashError::database("database_operation", "files_table", e.to_string())
            })?;

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

            let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&metadata).map_err(|e| {
                FlashError::database(
                    "database_operation",
                    "files_table",
                    format!("Serialization error: {}", e),
                )
            })?;

            table
                .insert(path.to_str().unwrap_or(""), bytes.as_slice())
                .map_err(|e| {
                    FlashError::database("database_operation", "files_table", e.to_string())
                })?;
        }

        txn.commit().map_err(|e| {
            FlashError::database("database_operation", "files_table", e.to_string())
        })?;

        Ok(())
    }

    /// Remove a file from the metadata database
    pub fn remove_file(&self, path: &Path) -> Result<bool> {
        let txn = self.db.begin_write().map_err(|e| {
            FlashError::database("database_operation", "files_table", e.to_string())
        })?;

        let existed = {
            let mut table = txn.open_table(FILES_TABLE).map_err(|e| {
                FlashError::database("database_operation", "files_table", e.to_string())
            })?;

            let path_str = path.to_str().unwrap_or("");
            let removed = table.remove(path_str).map_err(|e| {
                FlashError::database("database_operation", "files_table", e.to_string())
            })?;
            removed.is_some()
        };

        txn.commit().map_err(|e| {
            FlashError::database("database_operation", "files_table", e.to_string())
        })?;

        Ok(existed)
    }

    /// Clear all metadata (nuke the table)
    pub fn clear(&self) -> Result<()> {
        let txn = self.db.begin_write().map_err(|e| {
            FlashError::database("database_operation", "files_table", e.to_string())
        })?;

        {
            // Deleting the table is the fastest way to clear it
            txn.delete_table(FILES_TABLE).map_err(|e| {
                FlashError::database("database_operation", "files_table", e.to_string())
            })?;
            // We must recreate it in the same transaction or next open?
            // Actually, open_table in next usage will recreate it if we create it here?
            // Safer to just open it again to ensure it exists empty.
            let _ = txn.open_table(FILES_TABLE).map_err(|e| {
                FlashError::database("database_operation", "files_table", e.to_string())
            })?;
        }

        txn.commit().map_err(|e| {
            FlashError::database("database_operation", "files_table", e.to_string())
        })?;

        Ok(())
    }

    /// Get metadata for a specific file
    pub fn get_metadata(&self, path: &Path) -> Result<Option<FileMetadata>> {
        let txn = self.db.begin_read().map_err(|e| {
            FlashError::database("database_operation", "files_table", e.to_string())
        })?;

        let table = txn.open_table(FILES_TABLE).map_err(|e| {
            FlashError::database("database_operation", "files_table", e.to_string())
        })?;

        let result = match table
            .get(path.to_str().unwrap_or(""))
            .map_err(|e| FlashError::database("database_operation", "files_table", e.to_string()))?
        {
            Some(metadata) => {
                let bytes = metadata.value();
                if let Ok(meta) =
                    rkyv::access::<rkyv::Archived<FileMetadata>, rkyv::rancor::Error>(bytes)
                {
                    rkyv::deserialize::<FileMetadata, rkyv::rancor::Error>(meta).ok()
                } else {
                    None
                }
            }
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

        let txn = self.db.begin_write().map_err(|e| {
            FlashError::database("database_operation", "files_table", e.to_string())
        })?;

        let indexed_at = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let mut total_bytes_written = 0u64;

        {
            let mut table = txn.open_table(FILES_TABLE).map_err(|e| {
                FlashError::database("database_operation", "files_table", e.to_string())
            })?;

            for (path, modified, size, content_hash) in entries {
                let metadata = FileMetadata {
                    path: path.clone(),
                    modified: *modified,
                    size: *size,
                    content_hash: *content_hash,
                    indexed_at,
                };

                let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&metadata).map_err(|e| {
                    FlashError::database(
                        "database_operation",
                        "files_table",
                        format!("Serialization error: {}", e),
                    )
                })?;

                total_bytes_written += bytes.len() as u64;
                total_bytes_written += path.len() as u64;

                table.insert(path.as_str(), bytes.as_slice()).map_err(|e| {
                    FlashError::database("database_operation", "files_table", e.to_string())
                })?;
            }
        }

        txn.commit().map_err(|e| {
            FlashError::database("database_operation", "files_table", e.to_string())
        })?;

        // Update metrics
        self.metrics
            .write_operations
            .fetch_add(1, Ordering::Relaxed);
        self.metrics
            .bytes_written
            .fetch_add(total_bytes_written, Ordering::Relaxed);

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

        let txn = self.db.begin_read().map_err(|e| {
            FlashError::database("database_operation", "files_table", e.to_string())
        })?;

        let table = txn.open_table(FILES_TABLE).map_err(|e| {
            FlashError::database("database_operation", "files_table", e.to_string())
        })?;

        let results: Vec<bool> = entries
            .iter()
            .map(|(path, modified, size)| {
                match table.get(path.as_str()) {
                    Ok(Some(metadata)) => {
                        let bytes = metadata.value();
                        if let Ok(meta) =
                            rkyv::access::<rkyv::Archived<FileMetadata>, rkyv::rancor::Error>(bytes)
                        {
                            meta.modified != *modified || meta.size != *size
                        } else {
                            true
                        }
                    }
                    Ok(None) => true, // File not indexed yet
                    Err(e) => {
                        // Log error but assume safe to reindex rather than silently reindexing
                        tracing::warn!("Error reading metadata for '{}': {}", path, e);
                        true
                    }
                }
            })
            .collect();

        Ok(results)
    }

    /// Get recently modified files sorted by modification time
    /// Note: This loads all files into memory. For large datasets, consider using a separate index.
    pub fn get_recent_files(&self, limit: usize) -> Result<Vec<RecentFileEntry>> {
        let txn = self.db.begin_read().map_err(|e| {
            FlashError::database("database_operation", "files_table", e.to_string())
        })?;

        let table = txn.open_table(FILES_TABLE).map_err(|e| {
            FlashError::database("database_operation", "files_table", e.to_string())
        })?;

        let mut files: Vec<(String, u64, u64)> = table
            .iter()
            .map_err(|e| FlashError::database("database_operation", "files_table", e.to_string()))?
            .filter_map(|entry| {
                entry.ok().map(|(k, v)| {
                    let bytes = v.value();
                    if let Ok(meta) =
                        rkyv::access::<rkyv::Archived<FileMetadata>, rkyv::rancor::Error>(bytes)
                    {
                        (
                            k.value().to_string(),
                            meta.modified.to_native(),
                            meta.size.to_native(),
                        )
                    } else {
                        (k.value().to_string(), 0, 0)
                    }
                })
            })
            .collect();

        // Use select_nth_unstable for O(n) partial sort instead of O(n log n) full sort
        if files.len() > limit {
            files.select_nth_unstable_by(limit, |a, b| b.1.cmp(&a.1)); // Reverse order for max-heap behavior
            files.truncate(limit);
        } else {
            files.sort_by(|a, b| b.1.cmp(&a.1));
        }

        // Convert to the expected format (without titles for now, can be enhanced)
        Ok(files
            .into_iter()
            .map(|(path, modified, size)| (path, None, modified, size))
            .collect())
    }
}
