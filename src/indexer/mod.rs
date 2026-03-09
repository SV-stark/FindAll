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
use tantivy::{directory::MmapDirectory, Index};
use tracing::{info, warn};

/// Current schema version - bump this when schema changes
pub const SCHEMA_VERSION: &str = "1.2.0";

fn get_schema_version_path(index_path: &Path) -> PathBuf {
    index_path.join(".schema_version")
}

fn read_schema_version(index_path: &Path) -> Option<String> {
    std::fs::read_to_string(get_schema_version_path(index_path))
        .ok()
        .map(|s| s.trim().to_string())
}

fn write_schema_version(index_path: &Path, version: &str) -> Result<()> {
    std::fs::write(get_schema_version_path(index_path), version).map_err(FlashError::Io)
}

/// Central manager for the Tantivy search index
pub struct IndexManager {
    #[allow(dead_code)]
    index: Index,
    writer: IndexWriterManager,
    searcher: IndexSearcher,
}

impl IndexManager {
    /// Open or create index at the specified path
    pub fn open(index_path: &Path, memory_limit_mb: u32) -> Result<Self> {
        let schema = create_schema();

        // Ensure directory exists
        if !index_path.exists() {
            std::fs::create_dir_all(index_path).map_err(FlashError::Io)?;
        }

        // Check schema version - if mismatch, rebuild index
        let stored_version = read_schema_version(index_path);
        if let Some(ref ver) = stored_version {
            if ver != SCHEMA_VERSION {
                warn!(
                    "Schema version mismatch: stored={}, current={}. Rebuilding index...",
                    ver, SCHEMA_VERSION
                );
                std::fs::remove_dir_all(index_path).map_err(FlashError::Io)?;
                std::fs::create_dir_all(index_path).map_err(FlashError::Io)?;
                write_schema_version(index_path, SCHEMA_VERSION)?;
            }
        } else if index_path.join("meta.json").exists() {
            // Old index without version - rebuild
            warn!("No schema version found. Rebuilding index...");
            std::fs::remove_dir_all(index_path).map_err(FlashError::Io)?;
            std::fs::create_dir_all(index_path).map_err(FlashError::Io)?;
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

                    // Close the directory/files if needed? MmapDirectory handles it.
                    // Wipe and start over
                    std::fs::remove_dir_all(index_path).map_err(FlashError::Io)?;
                    std::fs::create_dir_all(index_path).map_err(FlashError::Io)?;
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
            searcher,
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
        &self,
        query: &str,
        limit: usize,
        min_size: Option<u64>,
        max_size: Option<u64>,
        file_extensions: Option<&[String]>,
    ) -> Result<Vec<SearchResult>> {
        self.searcher
            .search(query, limit, min_size, max_size, file_extensions)
            .await
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
    pub fn get_searcher(&self) -> &IndexSearcher {
        &self.searcher
    }
}
