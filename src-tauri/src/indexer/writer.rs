use tantivy::{Index, IndexWriter, Document};
use tantivy::schema::{Schema, Field};
use std::path::Path;
use std::sync::Mutex;
use crate::error::{FlashError, Result};
use crate::parsers::ParsedDocument;

/// Manages writing documents to the Tantivy index
pub struct IndexWriterManager {
    writer: Mutex<IndexWriter>,
    schema: Schema,
    path_field: Field,
    content_field: Field,
    title_field: Field,
    modified_field: Field,
}

impl IndexWriterManager {
    pub fn new(index: &Index) -> Result<Self> {
        let schema = index.schema();
        
        // Configure with 50MB heap size for low RAM usage
        let writer = index.writer(50_000_000)
            .map_err(|e| FlashError::Index(e.to_string()))?;
        
        let path_field = schema.get_field("file_path")
            .map_err(|_| FlashError::Index("file_path field not found".to_string()))?;
        let content_field = schema.get_field("content")
            .map_err(|_| FlashError::Index("content field not found".to_string()))?;
        let title_field = schema.get_field("title")
            .map_err(|_| FlashError::Index("title field not found".to_string()))?;
        let modified_field = schema.get_field("modified")
            .map_err(|_| FlashError::Index("modified field not found".to_string()))?;
        
        Ok(Self {
            writer: Mutex::new(writer),
            schema,
            path_field,
            content_field,
            title_field,
            modified_field,
        })
    }
    
    /// Add a document to the index
    pub fn add_document(&self, doc: ParsedDocument) -> Result<()> {
        let mut document = Document::new();
        
        document.add_text(self.path_field, &doc.path);
        document.add_text(self.content_field, &doc.content);
        
        if let Some(title) = doc.title {
            document.add_text(self.title_field, title);
        }
        
        // Get current timestamp
        let now = tantivy::DateTime::from_timestamp_secs(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs() as i64
        );
        document.add_date(self.modified_field, now);
        
        let mut writer = self.writer.lock()
            .map_err(|_| FlashError::Index("Failed to lock writer".to_string()))?;
        
        writer.add_document(document)
            .map_err(|e| FlashError::Index(e.to_string()))?;
        
        Ok(())
    }
    
    /// Commit pending changes to disk
    pub fn commit(&self) -> Result<()> {
        let mut writer = self.writer.lock()
            .map_err(|_| FlashError::Index("Failed to lock writer".to_string()))?;
        
        writer.commit()
            .map_err(|e| FlashError::Index(e.to_string()))?;
        
        Ok(())
    }
}
