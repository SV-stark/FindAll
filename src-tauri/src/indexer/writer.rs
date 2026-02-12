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
    pub fn new(index: &Index) -> Result<Self> {
        let schema = index.schema();

        // Configure with 50MB heap size for low RAM usage
        let writer = index
            .writer(50_000_000)
            .map_err(|e| FlashError::Index(e.to_string()))?;

        let path_field = schema
            .get_field("file_path")
            .map_err(|_| FlashError::Index("file_path field not found".to_string()))?;
        let content_field = schema
            .get_field("content")
            .map_err(|_| FlashError::Index("content field not found".to_string()))?;
        let title_field = schema
            .get_field("title")
            .map_err(|_| FlashError::Index("title field not found".to_string()))?;
        let modified_field = schema
            .get_field("modified")
            .map_err(|_| FlashError::Index("modified field not found".to_string()))?;
        let size_field = schema
            .get_field("size")
            .map_err(|_| FlashError::Index("size field not found".to_string()))?;

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
            .map_err(|_| FlashError::Index("Failed to lock writer".to_string()))?;

        writer
            .add_document(tantivy_doc)
            .map_err(|e| FlashError::Index(e.to_string()))?;

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
            .map_err(|_| FlashError::Index("Failed to lock writer".to_string()))?;

        for (doc, modified, size) in docs {
            let tantivy_doc = self.create_tantivy_document(doc, *modified, *size);
            writer
                .add_document(tantivy_doc)
                .map_err(|e| FlashError::Index(e.to_string()))?;
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
            .map_err(|_| FlashError::Index("Failed to lock writer".to_string()))?;

        writer
            .commit()
            .map_err(|e| FlashError::Index(e.to_string()))?;

        Ok(())
    }
}
