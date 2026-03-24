use crate::error::{FlashError, Result};
use redb::{Database, ReadableTable, TableDefinition};
use rkyv;
use std::cmp::Reverse;
use std::path::Path;
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

/// Manages file metadata database using redb
/// Implements connection pooling pattern for redb (even though it's embedded)
/// to ensure proper resource management and monitoring
pub struct MetadataDb {
    db: Arc<Database>,
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

            return Ok(Self { db });
        }

        Ok(Self { db })
    }

    /// Clone with shared state (for multi-threaded access)
    pub fn clone_for_thread(&self) -> Self {
        Self {
            db: Arc::clone(&self.db),
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
                // Zero-copy read and validate with byte alignment
                let mut aligned_bytes = rkyv::util::AlignedVec::<16>::new();
                aligned_bytes.extend_from_slice(bytes);

                if let Ok(meta) = rkyv::access::<rkyv::Archived<FileMetadata>, rkyv::rancor::Error>(
                    &aligned_bytes,
                ) {
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

                table.insert(path.as_str(), bytes.as_slice()).map_err(|e| {
                    FlashError::database("database_operation", "files_table", e.to_string())
                })?;
            }
        }

        txn.commit().map_err(|e| {
            FlashError::database("database_operation", "files_table", e.to_string())
        })?;

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
    /// Uses a bounded min-heap to avoid loading all files into memory.
    pub fn get_recent_files(&self, limit: usize) -> Result<Vec<RecentFileEntry>> {
        let txn = self.db.begin_read().map_err(|e| {
            FlashError::database("database_operation", "files_table", e.to_string())
        })?;

        let table = txn.open_table(FILES_TABLE).map_err(|e| {
            FlashError::database("database_operation", "files_table", e.to_string())
        })?;

        // Use a min-heap to keep the top `limit` most recent files.
        // We store (modified, path, size) and the heap is ordered by modified (smallest at top).
        use std::collections::BinaryHeap;
        // We define a wrapper struct to have a min-heap based on modified time.
        // Since BinaryHeap is a max-heap, we invert the order by using Reverse.
        let mut heap: BinaryHeap<Reverse<(u64, String, u64)>> = BinaryHeap::new();

        for entry in table
            .iter()
            .map_err(|e| FlashError::database("database_operation", "files_table", e.to_string()))?
        {
            let (k, v) = entry.map_err(|e| {
                FlashError::database("database_operation", "files_table", e.to_string())
            })?;
            let bytes = v.value();
            let (modified, size) = if let Ok(meta) =
                rkyv::access::<rkyv::Archived<FileMetadata>, rkyv::rancor::Error>(bytes)
            {
                (meta.modified.to_native(), meta.size.to_native())
            } else {
                (0, 0)
            };
            let path = k.value().to_string();

            // Push the entry into the heap
            heap.push(Reverse((modified, path, size)));

            // If heap exceeds limit, remove the least recent (smallest modified)
            if heap.len() > limit {
                heap.pop();
            }
        }

        // Extract the files from the heap.
        // They are in arbitrary order (heap order). We want them sorted by modified descending.
        let mut files: Vec<(String, u64, u64)> = heap
            .into_iter()
            .map(|Reverse(tuple)| {
                let (modified, path, size) = tuple;
                (path, modified, size)
            })
            .collect();

        // Sort by modified descending
        files.sort_by(|a, b| b.1.cmp(&a.1));

        // Convert to the expected format (without titles for now, can be enhanced)
        Ok(files
            .into_iter()
            .map(|(path, modified, size)| (path, None, modified, size))
            .collect())
    }
}
