use crate::error::{FlashError, Result};
use crate::parsers::ParsedDocument;
use parking_lot::Mutex;
use tantivy::schema::{Field, Schema};
use tantivy::{Index, IndexWriter, TantivyDocument};
use tracing::info;

/// Manages writing documents to the Tantivy index with batch support
pub struct IndexWriterManager {
    writer: Mutex<IndexWriter>,
    #[allow(dead_code)]
    schema: Schema,
    path_field: Field,
    content_field: Field,
    title_field: Field,
    modified_field: Field,
    size_field: Field,
    extension_field: Field,
}

impl IndexWriterManager {
    /// Calculate optimal heap size based on system resources
    /// Returns heap size in bytes (min 32MB, max 256MB)
    fn calculate_heap_size() -> usize {
        let available_memory = std::env::var("FLASH_SEARCH_MEMORY_MB")
            .ok()
            .and_then(|s| s.parse::<usize>().ok())
            .map_or_else(
                || {
                    let (total_mem, _, _) = crate::system::throttling::get_system_memory();
                    let system_memory = usize::try_from(total_mem).unwrap_or(usize::MAX);
                    (system_memory / 20).clamp(32_000_000, 256_000_000)
                },
                |mb| mb * 1_000_000,
            );

        available_memory.clamp(32_000_000, 256_000_000)
    }

    pub fn new(index: &Index, memory_limit_mb: u32) -> Result<Self> {
        let schema = index.schema();

        // Use user-provided memory limit if it's within reasonable bounds,
        // otherwise fall back to adaptive calculation.
        let heap_size = if (32..=2048).contains(&memory_limit_mb) {
            info!(
                "Using user-configured index writer heap size: {} MB",
                memory_limit_mb
            );
            (memory_limit_mb as usize) * 1_000_000
        } else {
            let calculated = Self::calculate_heap_size();
            info!(
                "Using calculated index writer heap size: {} MB (config: {} MB)",
                calculated / 1_000_000,
                memory_limit_mb
            );
            calculated
        };

        let writer = index
            .writer(heap_size)
            .map_err(|e| FlashError::index(format!("Failed to create index writer: {e}")))?;

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
        let extension_field = schema
            .get_field("extension")
            .map_err(|_| FlashError::index_field("extension", "Field not found in schema"))?;

        Ok(Self {
            writer: Mutex::new(writer),
            schema,
            path_field,
            content_field,
            title_field,
            modified_field,
            size_field,
            extension_field,
        })
    }

    /// Add a single document to the index
    /// Note: For better performance, use `add_documents_batch` for multiple docs
    pub fn add_document(&self, doc: &ParsedDocument, modified: u64, size: u64) -> Result<()> {
        let tantivy_doc = self.create_tantivy_document(doc, modified, size);

        self.writer
            .lock()
            .add_document(tantivy_doc)
            .map_err(|e| FlashError::index(format!("Failed to add document: {e}")))?;

        Ok(())
    }

    /// Add multiple documents in a single lock acquisition (much more efficient)
    pub fn add_documents_batch(&self, docs: &[(ParsedDocument, u64, u64)]) -> Result<()> {
        if docs.is_empty() {
            return Ok(());
        }

        // Chunk into 500 documents to yield writer lock periodically,
        // allowing concurrent watcher updates or search readers to make progress.
        for chunk in docs.chunks(500) {
            let writer = self.writer.lock();

            for (doc, modified, size) in chunk {
                let tantivy_doc = self.create_tantivy_document(doc, *modified, *size);
                writer
                    .add_document(tantivy_doc)
                    .map_err(|e| FlashError::index(format!("Failed to add document: {e}")))?;
            }
        }

        Ok(())
    }

    /// Create a Tantivy document from `ParsedDocument`
    #[inline]
    fn create_tantivy_document(
        &self,
        doc: &ParsedDocument,
        modified: u64,
        size: u64,
    ) -> TantivyDocument {
        let mut document = TantivyDocument::default();

        document.add_text(self.path_field, &doc.path);

        // Unify content and keywords into a single stream to keep posting list lean
        if let Some(ref keywords) = doc.keywords {
            let mut combined = String::with_capacity(doc.content.len() + keywords.len() + 1);
            combined.push_str(&doc.content);
            combined.push(' ');
            combined.push_str(keywords);
            document.add_text(self.content_field, &combined);
        } else {
            document.add_text(self.content_field, &doc.content);
        }

        if let Some(ref title) = doc.title {
            document.add_text(self.title_field, title);
        }

        let modified_date =
            tantivy::DateTime::from_timestamp_secs(i64::try_from(modified).unwrap_or(i64::MAX));
        document.add_date(self.modified_field, modified_date);
        document.add_u64(self.size_field, size);

        // Index file extension for fast filtering without allocating when already lowercase
        if let Some(ext) = std::path::Path::new(&doc.path)
            .extension()
            .and_then(|e| e.to_str())
        {
            if ext.bytes().all(|b| b.is_ascii_lowercase()) {
                document.add_text(self.extension_field, ext);
            } else {
                document.add_text(self.extension_field, ext.to_ascii_lowercase());
            }
        }

        document
    }

    /// Remove a document from the index
    pub fn remove_document(&self, path: &str) -> Result<()> {
        let term = tantivy::Term::from_field_text(self.path_field, path);
        self.writer.lock().delete_term(term);

        Ok(())
    }

    /// Remove many documents under a single lock acquisition.
    ///
    /// Dropping an indexed directory previously issued one
    /// `delete_term` + one redb write transaction *per file*. For a folder with
    /// 50 000 files that meant 50 000 round trips through the writer mutex.
    pub fn remove_documents_batch(&self, paths: &[String]) -> Result<usize> {
        if paths.is_empty() {
            return Ok(0);
        }

        let writer = self.writer.lock();
        for path in paths {
            let term = tantivy::Term::from_field_text(self.path_field, path);
            writer.delete_term(term);
        }
        // The guard releases here, before the caller commits.
        drop(writer);

        Ok(paths.len())
    }

    /// Delete all documents from the index
    pub fn delete_all_documents(&self) -> Result<()> {
        self.writer
            .lock()
            .delete_all_documents()
            .map_err(|e| FlashError::index(format!("Failed to delete all documents: {e}")))?;

        Ok(())
    }

    /// Commit pending changes to disk
    pub fn commit(&self) -> Result<()> {
        self.writer
            .lock()
            .commit()
            .map_err(|e| FlashError::index(format!("Failed to commit index: {e}")))?;

        Ok(())
    }
}
