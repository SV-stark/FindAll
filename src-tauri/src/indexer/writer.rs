use crate::error::{FlashError, Result};
use crate::parsers::ParsedDocument;
use std::sync::Mutex;
use tantivy::schema::{Field, Schema};
use tantivy::{Index, IndexWriter, TantivyDocument};

/// Manages writing documents to the Tantivy index with batch support
pub struct IndexWriterManager {
    writer: Mutex<IndexWriter>,
    schema: Schema,
    path_field: Field,
    content_field: Field,
    title_field: Field,
    modified_field: Field,
    size_field: Field,
}

impl IndexWriterManager {
    /// Calculate optimal heap size based on system resources
    /// Returns heap size in bytes (min 32MB, max 256MB)
    fn calculate_heap_size() -> usize {
        // Get system memory info (using sysinfo crate would be better, but this is lightweight)
        let available_memory = std::env::var("FLASH_SEARCH_MEMORY_MB")
            .ok()
            .and_then(|s| s.parse::<usize>().ok())
            .map(|mb| mb * 1_000_000)
            .unwrap_or_else(|| {
                // Default calculation: use 5% of system memory, capped at 256MB
                // This is a heuristic - in production, use sysinfo to get actual memory
                let system_memory = if cfg!(target_os = "linux") {
                    // Try to read from /proc/meminfo on Linux
                    std::fs::read_to_string("/proc/meminfo")
                        .ok()
                        .and_then(|content| {
                            content
                                .lines()
                                .find(|line| line.starts_with("MemTotal:"))
                                .and_then(|line| {
                                    line.split_whitespace()
                                        .nth(1)
                                        .and_then(|s| s.parse::<usize>().ok())
                                        .map(|kb| kb * 1024) // Convert KB to bytes
                                })
                        })
                        .unwrap_or(8_000_000_000) // Default to 8GB if unknown
                } else {
                    8_000_000_000 // Default 8GB for other platforms
                };

                let heap = (system_memory / 20).min(256_000_000).max(32_000_000);
                heap
            });

        available_memory.min(256_000_000).max(32_000_000)
    }

    pub fn new(index: &Index) -> Result<Self> {
        let schema = index.schema();

        // Configure with adaptive heap size based on system resources
        let heap_size = Self::calculate_heap_size();
        eprintln!(
            "Tantivy index writer heap size: {} MB",
            heap_size / 1_000_000
        );

        let writer = index
            .writer(heap_size)
            .map_err(|e| FlashError::index(format!("Failed to create index writer: {}", e)))?;

        let path_field = schema
            .get_field("file_path")
            .map_err(|_| FlashError::index_field("file_path", "Field not found in schema"))?;
        let content_field = schema
            .get_field("content")
            .map_err(|_| FlashError::index_field("content", "Field not found in schema"))?;
        let title_field = schema
            .get_field("title")
            .map_err(|_| FlashError::index_field("title", "Field not found in schema"))?;
        let modified_field = schema
            .get_field("modified")
            .map_err(|_| FlashError::index_field("modified", "Field not found in schema"))?;
        let size_field = schema
            .get_field("size")
            .map_err(|_| FlashError::index_field("size", "Field not found in schema"))?;

        Ok(Self {
            writer: Mutex::new(writer),
            schema,
            path_field,
            content_field,
            title_field,
            modified_field,
            size_field,
        })
    }

    /// Add a single document to the index
    /// Note: For better performance, use add_documents_batch for multiple docs
    pub fn add_document(&self, doc: &ParsedDocument, modified: u64, size: u64) -> Result<()> {
        let tantivy_doc = self.create_tantivy_document(doc, modified, size);

        let writer = self
            .writer
            .lock()
            .map_err(|_| FlashError::poisoned_lock("IndexWriter"))?;

        writer
            .add_document(tantivy_doc)
            .map_err(|e| FlashError::index(format!("Failed to add document: {}", e)))?;

        Ok(())
    }

    /// Add multiple documents in a single lock acquisition (much more efficient)
    pub fn add_documents_batch(&self, docs: &[(ParsedDocument, u64, u64)]) -> Result<()> {
        if docs.is_empty() {
            return Ok(());
        }

        let writer = self
            .writer
            .lock()
            .map_err(|_| FlashError::poisoned_lock("IndexWriter"))?;

        for (doc, modified, size) in docs {
            let tantivy_doc = self.create_tantivy_document(doc, *modified, *size);
            writer
                .add_document(tantivy_doc)
                .map_err(|e| FlashError::index(format!("Failed to add document: {}", e)))?;
        }

        Ok(())
    }

    /// Create a Tantivy document from ParsedDocument
    #[inline]
    fn create_tantivy_document(
        &self,
        doc: &ParsedDocument,
        modified: u64,
        size: u64,
    ) -> TantivyDocument {
        let mut document = TantivyDocument::default();

        document.add_text(self.path_field, &doc.path);
        document.add_text(self.content_field, &doc.content);

        if let Some(ref title) = doc.title {
            document.add_text(self.title_field, title);
        }

        let modified_date = tantivy::DateTime::from_timestamp_secs(modified as i64);
        document.add_date(self.modified_field, modified_date);
        document.add_u64(self.size_field, size);

        document
    }

    /// Commit pending changes to disk
    pub fn commit(&self) -> Result<()> {
        let mut writer = self
            .writer
            .lock()
            .map_err(|_| FlashError::poisoned_lock("IndexWriter"))?;

        writer
            .commit()
            .map_err(|e| FlashError::index(format!("Failed to commit index: {}", e)))?;

        Ok(())
    }
}
