pub mod filename_index;
pub mod query_parser;
pub mod schema;
pub mod searcher;
pub mod writer;

use self::schema::create_schema;
use self::searcher::{IndexSearcher, IndexStatistics, SearchResult};
use self::writer::IndexWriterManager;
use crate::error::{FlashError, Result};
use crate::parsers::ParsedDocument;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tantivy::{directory::MmapDirectory, Index};
use tracing::{info, warn};

/// Current schema version - bump this when schema changes
pub const SCHEMA_VERSION: &str = "1.3.0";

fn get_schema_version_path(index_path: &Path) -> PathBuf {
    index_path.join(".schema_version")
}

fn read_schema_version(index_path: &Path) -> Option<String> {
    std::fs::read_to_string(get_schema_version_path(index_path))
        .ok()
        .map(|s| s.trim().to_string())
}

fn write_schema_version(index_path: &Path, version: &str) -> Result<()> {
    std::fs::write(get_schema_version_path(index_path), version)
        .map_err(|e| FlashError::Io(std::sync::Arc::new(e)))
}

/// Central manager for the Tantivy search index
pub struct IndexManager {
    #[allow(dead_code)]
    index: Index,
    writer: IndexWriterManager,
    searcher: Arc<IndexSearcher>,
}

impl IndexManager {
    /// Open or create index at the specified path
    pub fn open(index_path: &Path, memory_limit_mb: u32) -> Result<Self> {
        let schema = create_schema();

        // Ensure directory exists
        if !index_path.exists() {
            std::fs::create_dir_all(index_path)
                .map_err(|e| FlashError::Io(std::sync::Arc::new(e)))?;
        }

        // Check schema version - if mismatch, rebuild index
        let stored_version = read_schema_version(index_path);
        if let Some(ref ver) = stored_version {
            if ver != SCHEMA_VERSION {
                warn!(
                    "Schema version mismatch: stored={}, current={}. Rebuilding index...",
                    ver, SCHEMA_VERSION
                );
                // Try to backup the index before destroying it
                let backup_path = index_path.with_extension("backup");
                if let Err(e) = std::fs::remove_dir_all(&backup_path) {
                    if e.kind() != std::io::ErrorKind::NotFound {
                        warn!("Failed to remove old backup at {:?}: {}", backup_path, e);
                    }
                }
                if let Err(e) = copy_dir(index_path, &backup_path) {
                    warn!("Failed to backup index to {:?}: {}", backup_path, e);
                }

                if let Err(e) = std::fs::remove_dir_all(index_path) {
                    error!("Failed to remove corrupted index at {:?}: {}", index_path, e);
                    return Err(FlashError::Io(std::sync::Arc::new(e)));
                }

                if let Err(e) = std::fs::create_dir_all(index_path) {
                    error!("Failed to re-create index directory at {:?}: {}", index_path, e);
                    return Err(FlashError::Io(std::sync::Arc::new(e)));
                }
                write_schema_version(index_path, SCHEMA_VERSION)?;
            }
        } else if index_path.join("meta.json").exists() {
            // Old index without version - rebuild
            warn!("No schema version found. Rebuilding index...");
            // Try to backup the index before destroying it
            let backup_path = index_path.with_extension("backup");
            if let Err(e) = std::fs::remove_dir_all(&backup_path) {
                if e.kind() != std::io::ErrorKind::NotFound {
                    warn!("Failed to remove old backup at {:?}: {}", backup_path, e);
                }
            }
            if let Err(e) = copy_dir(index_path, &backup_path) {
                warn!("Failed to backup index to {:?}: {}", backup_path, e);
            }

            if let Err(e) = std::fs::remove_dir_all(index_path) {
                error!("Failed to remove old index at {:?}: {}", index_path, e);
                return Err(FlashError::Io(std::sync::Arc::new(e)));
            }
            if let Err(e) = std::fs::create_dir_all(index_path) {
                error!("Failed to re-create index directory at {:?}: {}", index_path, e);
                return Err(FlashError::Io(std::sync::Arc::new(e)));
            }
            write_schema_version(index_path, SCHEMA_VERSION)?;
        } else {
            // New index - write version
            write_schema_version(index_path, SCHEMA_VERSION)?;
        }

        let directory = MmapDirectory::open(index_path)
            .map_err(|e| FlashError::index(format!("Failed to open index directory: {}", e)))?;

        let index = match Index::open_or_create(directory, schema.clone()) {
            Ok(idx) => idx,
            Err(e) => {
                // Check if it's a schema mismatch error
                let err_str = e.to_string();
                if err_str.contains("Schema error") || err_str.contains("Inconsistent") {
                    warn!(
                        "Tantivy detected schema mismatch: {}. Forcing index rebuild...",
                        err_str
                    );

                    // Try to backup the index before destroying it
                    let backup_path = index_path.with_extension("backup");
                    let _ = std::fs::remove_dir_all(&backup_path); // Remove old backup if exists
                    let _ = copy_dir(index_path, &backup_path); // Try to backup

                    // Close the directory/files if needed? MmapDirectory handles it.
                    // Wipe and start over
                    std::fs::remove_dir_all(index_path)
                        .map_err(|e| FlashError::Io(std::sync::Arc::new(e)))?;
                    std::fs::create_dir_all(index_path)
                        .map_err(|e| FlashError::Io(std::sync::Arc::new(e)))?;
                    write_schema_version(index_path, SCHEMA_VERSION)?;

                    let new_directory = MmapDirectory::open(index_path).map_err(|e| {
                        FlashError::index(format!("Failed to re-open index directory: {}", e))
                    })?;
                    Index::open_or_create(new_directory, schema).map_err(|e| {
                        FlashError::index(format!("Failed to create new index after reset: {}", e))
                    })?
                } else {
                    return Err(FlashError::index(format!(
                        "Failed to open or create index: {}",
                        e
                    )));
                }
            }
        };

        info!(
            "Opened index at {} with schema version {}",
            index_path.display(),
            SCHEMA_VERSION
        );

        let writer = IndexWriterManager::new(&index, memory_limit_mb)?;
        let searcher = IndexSearcher::new(&index, index_path.to_path_buf())?;

        Ok(Self {
            index,
            writer,
            searcher: Arc::new(searcher),
        })
    }

    /// Add a document to the index
    pub fn add_document(&self, doc: &ParsedDocument, modified: u64, size: u64) -> Result<()> {
        self.writer.add_document(doc, modified, size)
    }

    /// Add multiple documents in a single lock acquisition (much more efficient)
    pub fn add_documents_batch(&self, docs: &[(ParsedDocument, u64, u64)]) -> Result<()> {
        self.writer.add_documents_batch(docs)
    }

    /// Remove a document from the index
    pub fn remove_document(&self, path: &str) -> Result<()> {
        self.writer.remove_document(path)
    }

    /// Clear all documents from the index
    pub fn clear(&self) -> Result<()> {
        self.writer.delete_all_documents()
    }

    /// Commit pending changes
    pub fn commit(&self) -> Result<()> {
        self.writer.commit()
    }

    /// Search the index (async with caching)
    pub async fn search(
        self: &Arc<Self>,
        query: &str,
        limit: usize,
        min_size: Option<u64>,
        max_size: Option<u64>,
        min_modified: Option<u64>,
        file_extensions: Option<&[String]>,
        case_sensitive: bool,
    ) -> Result<Vec<SearchResult>> {
        self.searcher
            .search(
                query,
                limit,
                min_size,
                max_size,
                min_modified,
                file_extensions,
                case_sensitive,
            )
            .await
    }

    /// Get recent files
    pub fn get_recent_files(&self, limit: usize) -> Result<Vec<SearchResult>> {
        self.searcher.get_recent_files(limit)
    }

    /// Invalidate search cache (call after index updates)
    pub fn invalidate_cache(&self) {
        self.searcher.invalidate_cache();
    }

    /// Get index statistics
    pub fn get_statistics(&self) -> Result<IndexStatistics> {
        self.searcher.get_statistics()
    }

    /// Get the searcher for direct document access
    pub fn get_searcher(&self) -> &Arc<IndexSearcher> {
        &self.searcher
    }
}

fn copy_dir(src: &Path, dst: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in ignore::WalkBuilder::new(src)
        .hidden(false)
        .git_ignore(false)
        .ignore(false)
        .parents(false)
        .build()
        .skip(1)
    {
        let entry = entry.map_err(std::io::Error::other)?;
        let ty = entry.file_type().ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::NotFound, "Could not get file type")
        })?;
        let path = entry.path();
        let relative = path.strip_prefix(src).map_err(std::io::Error::other)?;
        let target = dst.join(relative);
        if ty.is_dir() {
            std::fs::create_dir_all(target)?;
        } else {
            std::fs::copy(path, target)?;
        }
    }
    Ok(())
}
