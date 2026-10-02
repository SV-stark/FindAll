use crate::error::{FlashError, Result};
use redb::{Database, ReadableTable, TableDefinition};
use rkyv;
use std::cmp::Reverse;
use std::collections::BinaryHeap;
use std::path::Path;
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

impl FileMetadata {
    pub fn builder() -> FileMetadataBuilder {
        FileMetadataBuilder::default()
    }
}

#[derive(Default)]
pub struct FileMetadataBuilder {
    path: Option<String>,
    modified: Option<u64>,
    size: Option<u64>,
    content_hash: Option<[u8; 32]>,
    indexed_at: Option<u64>,
}

impl FileMetadataBuilder {
    #[must_use]
    pub fn path(mut self, path: String) -> Self {
        self.path = Some(path);
        self
    }

    #[must_use]
    pub const fn modified(mut self, modified: u64) -> Self {
        self.modified = Some(modified);
        self
    }

    #[must_use]
    pub const fn size(mut self, size: u64) -> Self {
        self.size = Some(size);
        self
    }

    #[must_use]
    pub const fn content_hash(mut self, content_hash: [u8; 32]) -> Self {
        self.content_hash = Some(content_hash);
        self
    }

    #[must_use]
    pub const fn indexed_at(mut self, indexed_at: u64) -> Self {
        self.indexed_at = Some(indexed_at);
        self
    }

    /// Builds the `FileMetadata`.
    ///
    /// # Panics
    ///
    /// Panics if any required field is missing.
    pub fn build(self) -> FileMetadata {
        FileMetadata {
            path: self.path.expect("path is required"),
            modified: self.modified.expect("modified is required"),
            size: self.size.expect("size is required"),
            content_hash: self.content_hash.expect("content_hash is required"),
            indexed_at: self.indexed_at.expect("indexed_at is required"),
        }
    }
}

pub type RecentFileEntry = (String, Option<String>, u64, u64);

/// Current wall-clock time in whole seconds since the Unix epoch.
fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// Renames the current database file aside and creates a fresh one.
///
/// A failed rename is only a warning: some filesystems (and Windows while a
/// handle is still open) refuse it, and recreating in place is still better than
/// refusing to start.
fn preserve_and_recreate(db_path: &Path, corrupt_path: &Path) -> Result<Database> {
    if let Err(rename_err) = std::fs::rename(db_path, corrupt_path) {
        tracing::warn!(
            "Could not preserve {:?} to {:?} ({rename_err}); recreating in place",
            db_path,
            corrupt_path
        );
    }

    Database::create(db_path)
        .map_err(|e| FlashError::database("create", db_path.display().to_string(), e.to_string()))
}

/// Manages file metadata database using redb
/// Implements connection pooling pattern for redb (even though it's embedded)
/// to ensure proper resource management and monitoring
pub struct MetadataDb {
    db: Database,
}

/// Helper to safely access archived metadata with 16-byte alignment
#[inline]
fn decode_metadata<R>(
    bytes: &[u8],
    f: impl FnOnce(&rkyv::Archived<FileMetadata>) -> R,
) -> Option<R> {
    let mut aligned = rkyv::util::AlignedVec::<16>::new();
    aligned.extend_from_slice(bytes);
    rkyv::access::<rkyv::Archived<FileMetadata>, rkyv::rancor::Error>(&aligned)
        .ok()
        .map(f)
}

impl MetadataDb {
    /// Open or create the metadata database.
    ///
    /// Returns `(db, was_reset)` where `was_reset` is true when a corrupt or
    /// unusable database file was preserved and replaced.
    pub fn open(db_path: &Path) -> Result<(Self, bool)> {
        let mut reset_occurred = false;
        let db = match Database::create(db_path) {
            Ok(db) => db,
            Err(e) => {
                reset_occurred = true;
                let now = unix_now();
                let corrupt_path = db_path.with_extension(format!("redb.corrupt.{now}"));
                tracing::warn!(
                    "Failed to open metadata database: {}. Preserving corrupt database to {:?}...",
                    e,
                    corrupt_path
                );
                preserve_and_recreate(db_path, &corrupt_path)?
            }
        };

        match Self::init_table(&db) {
            Ok(()) => Ok((Self { db }, reset_occurred)),
            Err(e) => {
                reset_occurred = true;
                let now = unix_now();
                let corrupt_path = db_path.with_extension(format!("redb.table_err.{now}"));
                tracing::warn!(
                    "Failed to initialize database tables: {}. Preserving to {:?} and recreating...",
                    e,
                    corrupt_path
                );

                // The `Database` handle must actually be released before the file
                // can be renamed. The previous code did `drop(db)` inside this
                // scope, which dropped a *shadowing* binding and left the real
                // `Arc` alive — on Windows the rename then failed with
                // "Access is denied" and the error was discarded.
                drop(db);
                let db = preserve_and_recreate(db_path, &corrupt_path)?;

                Self::init_table(&db).map_err(|e| {
                    FlashError::database(
                        "init_table_retry",
                        "files_table",
                        format!("Retry failed: {e}"),
                    )
                })?;

                Ok((Self { db }, reset_occurred))
            }
        }
    }

    /// Opens the files table, creating it when absent.
    fn init_table(db: &Database) -> Result<()> {
        let txn = db
            .begin_write()
            .map_err(|e| FlashError::database("begin_write", "files_table", e.to_string()))?;
        {
            let _table = txn
                .open_table(FILES_TABLE)
                .map_err(|e| FlashError::database("open_table", "files_table", e.to_string()))?;
        }
        txn.commit()
            .map_err(|e| FlashError::database("commit", "files_table", e.to_string()))
    }

    /// Check if file needs reindexing based on modification time and hash
    pub fn needs_reindex(&self, path: &Path, modified: u64, size: u64) -> Result<bool> {
        let txn = self
            .db
            .begin_read()
            .map_err(|e| FlashError::database("begin_read", "files_table", e.to_string()))?;

        let table = txn
            .open_table(FILES_TABLE)
            .map_err(|e| FlashError::database("open_table", "files_table", e.to_string()))?;

        let path_cow = path.to_string_lossy();
        let path_str = path_cow.as_ref();

        let result = table
            .get(path_str)
            .map_err(|e| FlashError::database("get", path_str, e.to_string()))?
            .is_none_or(|metadata| {
                let bytes = metadata.value();
                decode_metadata(bytes, |meta| meta.modified != modified || meta.size != size)
                    .unwrap_or(true)
            });

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
        let path_cow = path.to_string_lossy();
        let path_str = path_cow.as_ref();

        let txn = self
            .db
            .begin_write()
            .map_err(|e| FlashError::database("begin_write", "files_table", e.to_string()))?;

        {
            let mut table = txn
                .open_table(FILES_TABLE)
                .map_err(|e| FlashError::database("open_table", "files_table", e.to_string()))?;

            let metadata = FileMetadata::builder()
                .path(path_str.to_string())
                .modified(modified)
                .size(size)
                .content_hash(content_hash)
                .indexed_at(unix_now())
                .build();

            let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&metadata).map_err(|e| {
                FlashError::database("serialize", path_str, format!("Serialization error: {e}"))
            })?;

            table
                .insert(path_str, bytes.as_slice())
                .map_err(|e| FlashError::database("insert", path_str, e.to_string()))?;
        }

        txn.commit()
            .map_err(|e| FlashError::database("commit", "files_table", e.to_string()))?;

        Ok(())
    }

    /// Remove a file from the metadata database
    pub fn remove_file(&self, path: &Path) -> Result<bool> {
        let path_cow = path.to_string_lossy();
        let path_str = path_cow.as_ref();

        let txn = self
            .db
            .begin_write()
            .map_err(|e| FlashError::database("begin_write", "files_table", e.to_string()))?;

        let existed = {
            let mut table = txn
                .open_table(FILES_TABLE)
                .map_err(|e| FlashError::database("open_table", "files_table", e.to_string()))?;

            let removed = table
                .remove(path_str)
                .map_err(|e| FlashError::database("remove", path_str, e.to_string()))?;
            removed.is_some()
        };

        txn.commit()
            .map_err(|e| FlashError::database("commit", "files_table", e.to_string()))?;

        Ok(existed)
    }

    /// Clear all metadata (nuke the table)
    pub fn clear(&self) -> Result<()> {
        let txn = self
            .db
            .begin_write()
            .map_err(|e| FlashError::database("begin_write", "files_table", e.to_string()))?;

        {
            txn.delete_table(FILES_TABLE)
                .map_err(|e| FlashError::database("delete_table", "files_table", e.to_string()))?;
            let _ = txn
                .open_table(FILES_TABLE)
                .map_err(|e| FlashError::database("open_table", "files_table", e.to_string()))?;
        }

        txn.commit()
            .map_err(|e| FlashError::database("commit", "files_table", e.to_string()))?;

        Ok(())
    }

    /// Get all file paths currently stored in the metadata database
    pub fn get_all_file_paths(&self) -> Result<Vec<String>> {
        let txn = self
            .db
            .begin_read()
            .map_err(|e| FlashError::database("begin_read", "files_table", e.to_string()))?;

        let table = txn
            .open_table(FILES_TABLE)
            .map_err(|e| FlashError::database("open_table", "files_table", e.to_string()))?;

        let mut paths = Vec::new();
        for entry in table
            .iter()
            .map_err(|e| FlashError::database("iter", "files_table", e.to_string()))?
        {
            let (k, _) = entry
                .map_err(|e| FlashError::database("iter_entry", "files_table", e.to_string()))?;
            paths.push(k.value().to_string());
        }

        Ok(paths)
    }

    /// Get metadata for a specific file
    pub fn get_metadata(&self, path: &Path) -> Result<Option<FileMetadata>> {
        let path_cow = path.to_string_lossy();
        let path_str = path_cow.as_ref();

        let txn = self
            .db
            .begin_read()
            .map_err(|e| FlashError::database("begin_read", "files_table", e.to_string()))?;

        let table = txn
            .open_table(FILES_TABLE)
            .map_err(|e| FlashError::database("open_table", "files_table", e.to_string()))?;

        let result = table
            .get(path_str)
            .map_err(|e| FlashError::database("get", path_str, e.to_string()))?
            .and_then(|metadata| {
                let bytes = metadata.value();
                decode_metadata(bytes, |meta| FileMetadata {
                    path: meta.path.as_str().to_string(),
                    modified: meta.modified.to_native(),
                    size: meta.size.to_native(),
                    content_hash: meta.content_hash,
                    indexed_at: meta.indexed_at.to_native(),
                })
            });

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
            .map_err(|e| FlashError::database("begin_write", "files_table", e.to_string()))?;

        let indexed_at = unix_now();

        {
            let mut table = txn
                .open_table(FILES_TABLE)
                .map_err(|e| FlashError::database("open_table", "files_table", e.to_string()))?;

            for (path, modified, size, content_hash) in entries {
                let metadata = FileMetadata::builder()
                    .path(path.clone())
                    .modified(*modified)
                    .size(*size)
                    .content_hash(*content_hash)
                    .indexed_at(indexed_at)
                    .build();

                let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&metadata).map_err(|e| {
                    FlashError::database(
                        "serialize",
                        path.as_str(),
                        format!("Serialization error: {e}"),
                    )
                })?;

                table
                    .insert(path.as_str(), bytes.as_slice())
                    .map_err(|e| FlashError::database("insert", path.as_str(), e.to_string()))?;
            }
        }

        txn.commit()
            .map_err(|e| FlashError::database("commit", "files_table", e.to_string()))?;

        Ok(entries.len())
    }

    /// Batch check which files need reindexing.
    ///
    /// Returns a vector of booleans, positionally matching `entries`.
    pub fn batch_needs_reindex(
        &self,
        entries: &[(String, u64, u64)], // (path, modified, size)
    ) -> Result<Vec<bool>> {
        let keys: Vec<&str> = entries.iter().map(|(p, _, _)| p.as_str()).collect();
        let stamps: Vec<(u64, u64)> = entries.iter().map(|(_, m, s)| (*m, *s)).collect();
        self.staleness_for(&keys, &stamps)
    }

    /// Batch check which files need reindexing, without allocating `String`s for
    /// the paths.
    pub fn batch_needs_reindex_paths(
        &self,
        entries: &[(std::path::PathBuf, u64, u64)], // (path, modified, size)
    ) -> Result<Vec<bool>> {
        // `Path::to_str` yields a borrowed `&str` in the overwhelmingly common
        // UTF-8 case; only fall back to an owned lossy conversion otherwise.
        let owned: Vec<std::borrow::Cow<'_, str>> = entries
            .iter()
            .map(|(path, _, _)| {
                path.to_str()
                    .map_or_else(|| path.to_string_lossy(), std::borrow::Cow::Borrowed)
            })
            .collect();
        let keys: Vec<&str> = owned.iter().map(std::borrow::Cow::as_ref).collect();
        let stamps: Vec<(u64, u64)> = entries.iter().map(|(_, m, s)| (*m, *s)).collect();
        self.staleness_for(&keys, &stamps)
    }

    /// Batch check which files need reindexing from borrowed paths and explicit
    /// `(modified, size)` stamps.
    ///
    /// Lets callers that already stat'd a file (the watcher) reuse those stats
    /// instead of re-`stat`ing, and avoids allocating a `PathBuf` per entry.
    pub fn batch_needs_reindex_paths_paths(
        &self,
        paths: &[&Path],
        stamps: &[(u64, u64)],
    ) -> Result<Vec<bool>> {
        debug_assert_eq!(paths.len(), stamps.len());
        if paths.is_empty() {
            return Ok(vec![]);
        }

        let owned: Vec<std::borrow::Cow<'_, str>> = paths
            .iter()
            .map(|path| {
                path.to_str()
                    .map_or_else(|| path.to_string_lossy(), std::borrow::Cow::Borrowed)
            })
            .collect();
        let keys: Vec<&str> = owned.iter().map(std::borrow::Cow::as_ref).collect();
        self.staleness_for(&keys, stamps)
    }

    /// Shared staleness implementation over borrowed keys.
    ///
    /// A read error is logged and reported as "needs reindex" for that entry:
    /// re-parsing wastes work, while silently skipping a file makes it
    /// unsearchable, so failing open is the correct direction for a search tool.
    fn staleness_for(&self, keys: &[&str], stamps: &[(u64, u64)]) -> Result<Vec<bool>> {
        if keys.is_empty() {
            return Ok(vec![]);
        }

        let txn = self
            .db
            .begin_read()
            .map_err(|e| FlashError::database("begin_read", "files_table", e.to_string()))?;

        let table = txn
            .open_table(FILES_TABLE)
            .map_err(|e| FlashError::database("open_table", "files_table", e.to_string()))?;

        let mut read_errors = 0usize;
        let results: Vec<bool> = keys
            .iter()
            .zip(stamps)
            .map(|(key, &(modified, size))| match table.get(*key) {
                Err(e) => {
                    read_errors += 1;
                    tracing::warn!("Metadata read failed for {key}: {e}");
                    true
                }
                Ok(None) => true,
                Ok(Some(metadata)) => {
                    let bytes = metadata.value();
                    decode_metadata(bytes, |meta| {
                        meta.modified != modified || meta.size != size
                    })
                    // A row that fails to decode is treated as stale so the file
                    // gets rewritten with a valid record.
                    .unwrap_or(true)
                }
            })
            .collect();

        if read_errors > 0 {
            tracing::warn!(
                "{read_errors} metadata reads failed; those files will be re-indexed"
            );
        }

        Ok(results)
    }

    /// Removes many files from the metadata database in a single transaction.
    ///
    /// Returns the number of rows actually removed. The previous callers
    /// (removing a whole indexed folder) issued one write transaction per file,
    /// which for a large directory meant tens of thousands of commits.
    pub fn remove_files(&self, paths: &[&Path]) -> Result<usize> {
        if paths.is_empty() {
            return Ok(0);
        }

        let txn = self
            .db
            .begin_write()
            .map_err(|e| FlashError::database("begin_write", "files_table", e.to_string()))?;

        let mut removed = 0usize;
        {
            let mut table = txn
                .open_table(FILES_TABLE)
                .map_err(|e| FlashError::database("open_table", "files_table", e.to_string()))?;

            for path in paths {
                let path_cow = path.to_string_lossy();
                match table.remove(path_cow.as_ref()) {
                    Ok(_) => removed += 1,
                    Err(e) => {
                        tracing::warn!("Failed to remove metadata for {}: {e}", path.display());
                    }
                }
            }
        }

        txn.commit()
            .map_err(|e| FlashError::database("commit", "files_table", e.to_string()))?;

        Ok(removed)
    }

    /// Returns every stored path inside `prefix`.
    ///
    /// Matching is component-aware: a plain string `starts_with` would treat
    /// `C:\docs-archive\old.txt` as living under `C:\docs`, so removing an indexed
    /// `docs` folder would silently delete a sibling `docs-archive` folder's
    /// entire index.
    pub fn paths_under(&self, prefix: &Path) -> Result<Vec<String>> {
        let prefix_str = prefix.to_string_lossy().into_owned();
        // Require a separator (or the end of the string) right after the prefix.
        let boundary = |path: &str| -> bool {
            path.len() == prefix_str.len() || {
                path.as_bytes()
                    .get(prefix_str.len())
                    .is_some_and(|b| matches!(b, b'/' | b'\\'))
            }
        };

        let txn = self
            .db
            .begin_read()
            .map_err(|e| FlashError::database("begin_read", "files_table", e.to_string()))?;

        let table = txn
            .open_table(FILES_TABLE)
            .map_err(|e| FlashError::database("open_table", "files_table", e.to_string()))?;

        let mut paths = Vec::new();
        for entry in table
            .iter()
            .map_err(|e| FlashError::database("iter", "files_table", e.to_string()))?
        {
            let (k, _) = entry
                .map_err(|e| FlashError::database("iter_entry", "files_table", e.to_string()))?;
            let path = k.value();
            if path.starts_with(&prefix_str) && boundary(path) {
                paths.push(path.to_string());
            }
        }

        Ok(paths)
    }

    /// Get recently modified files sorted by modification time
    /// Uses a bounded min-heap to avoid loading all files into memory.
    pub fn get_recent_files(&self, limit: usize) -> Result<Vec<RecentFileEntry>> {
        let txn = self
            .db
            .begin_read()
            .map_err(|e| FlashError::database("begin_read", "files_table", e.to_string()))?;

        let table = txn
            .open_table(FILES_TABLE)
            .map_err(|e| FlashError::database("open_table", "files_table", e.to_string()))?;

        let mut heap: BinaryHeap<Reverse<(u64, String, u64)>> = BinaryHeap::new();

        for entry in table
            .iter()
            .map_err(|e| FlashError::database("iter", "files_table", e.to_string()))?
        {
            let (k, v) = entry
                .map_err(|e| FlashError::database("iter_entry", "files_table", e.to_string()))?;
            let bytes = v.value();
            let (modified, size) = decode_metadata(bytes, |meta| {
                (meta.modified.to_native(), meta.size.to_native())
            })
            .unwrap_or((0, 0));
            let path = k.value().to_string();

            heap.push(Reverse((modified, path, size)));

            if heap.len() > limit {
                heap.pop();
            }
        }

        let mut files: Vec<(String, u64, u64)> = heap
            .into_iter()
            .map(|Reverse(tuple)| {
                let (modified, path, size) = tuple;
                (path, modified, size)
            })
            .collect();

        files.sort_by_key(|b| std::cmp::Reverse(b.1));

        Ok(files
            .into_iter()
            .map(|(path, modified, size)| (path, None, modified, size))
            .collect())
    }
}
