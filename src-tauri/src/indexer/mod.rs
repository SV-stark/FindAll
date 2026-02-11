pub mod schema;
pub mod writer;
pub mod searcher;

use tantivy::{Index, directory::MmapDirectory};
use std::path::Path;
use crate::error::{FlashError, Result};
use crate::parsers::ParsedDocument;
use self::schema::create_schema;
use self::writer::IndexWriterManager;
use self::searcher::{IndexSearcher, SearchResult};

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
        
        // Use memory-mapped directory for efficient I/O
        let directory = MmapDirectory::open(index_path)
            .map_err(|e| FlashError::Index(format!("Failed to open index directory: {}", e)))?;
        
        let index = Index::open_or_create(directory, schema)
            .map_err(|e| FlashError::Index(format!("Failed to create/open index: {}", e)))?;
        
        let writer = IndexWriterManager::new(&index)?;
        let searcher = IndexSearcher::new(&index)?;
        
        Ok(Self {
            index,
            writer,
            searcher,
        })
    }
    
    /// Add a document to the index
    pub fn add_document(&self, doc: ParsedDocument) -> Result<()> {
        self.writer.add_document(doc)
    }
    
    /// Commit pending changes
    pub fn commit(&self) -> Result<()> {
        self.writer.commit()
    }
    
    /// Search the index
    pub fn search(&self, query: &str) -> Result<Vec<SearchResult>> {
        self.searcher.search(query, 50) // Return top 50 results
    }
}
