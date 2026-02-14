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
use std::path::Path;
use tantivy::{directory::MmapDirectory, Index};

/// Central manager for the Tantivy search index
pub struct IndexManager {
    index: Index,
    writer: IndexWriterManager,
    searcher: IndexSearcher,
}

impl IndexManager {
    /// Open or create index at the specified path
    pub fn open(index_path: &Path) -> Result<Self> {
        let schema = create_schema();

        // Ensure directory exists
        if !index_path.exists() {
            std::fs::create_dir_all(index_path).map_err(|e| FlashError::Io(e))?;
        }

        // Use memory-mapped directory for efficient I/O
        let directory = MmapDirectory::open(index_path)
            .map_err(|e| FlashError::index(format!("Failed to open index directory: {}", e)))?;

        let index = match Index::open_or_create(directory, schema.clone()) {
            Ok(index) => index,
            Err(tantivy::TantivyError::SchemaError(msg))
                if msg.contains("schema does not match") =>
            {
                eprintln!("Schema mismatch detected. Recreating index...");
                // Close directory and remove files
                std::fs::remove_dir_all(index_path).map_err(|e| FlashError::Io(e))?;
                std::fs::create_dir_all(index_path).map_err(|e| FlashError::Io(e))?;

                let new_directory = MmapDirectory::open(index_path).map_err(|e| {
                    FlashError::index(format!("Failed to recreate index directory: {}", e))
                })?;
                Index::open_or_create(new_directory, schema)
                    .map_err(|e| FlashError::index(format!("Failed to create new index: {}", e)))?
            }
            Err(e) => return Err(FlashError::index(format!("Failed to open index: {}", e))),
        };

        let writer = IndexWriterManager::new(&index)?;
        let searcher = IndexSearcher::new(&index)?;

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
    pub async fn invalidate_cache(&self) {
        self.searcher.invalidate_cache().await;
    }

    /// Get index statistics
    pub fn get_statistics(&self) -> Result<IndexStatistics> {
        self.searcher.get_statistics()
    }
}
