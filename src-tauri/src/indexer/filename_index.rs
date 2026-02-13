use crate::error::{FlashError, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::Arc;
use tantivy::collector::TopDocs;
use tantivy::query::RegexQuery;
use tantivy::schema::*;
use tantivy::{
    directory::MmapDirectory, Index, IndexReader, IndexWriter, ReloadPolicy, TantivyDocument,
};
use tokio::sync::Mutex;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct FilenameResult {
    pub file_path: String,
    pub file_name: String,
}

pub struct FilenameIndex {
    index: Index,
    reader: IndexReader,
    writer: Arc<Mutex<IndexWriter>>,
    schema: Schema,
    path_field: Field,
    name_field: Field,
    index_path: std::path::PathBuf,
}

impl FilenameIndex {
    pub fn open(index_path: &Path) -> Result<Self> {
        let mut schema_builder = Schema::builder();

        let path_field = schema_builder.add_text_field("path", STRING | STORED);
        let name_field = schema_builder.add_text_field("name", TEXT | STORED);

        let schema = schema_builder.build();

        if !index_path.exists() {
            std::fs::create_dir_all(index_path)?;
        }

        let index_path = index_path.join("filenames");

        let directory = MmapDirectory::open(&index_path)
            .map_err(|e| FlashError::Search(format!("Failed to open filename index: {}", e)))?;

        let index = Index::open_or_create(directory, schema.clone())
            .map_err(|e| FlashError::Search(format!("Failed to create filename index: {}", e)))?;

        let reader = index
            .reader_builder()
            .reload_policy(ReloadPolicy::OnCommitWithDelay)
            .try_into()
            .map_err(|e| FlashError::Search(e.to_string()))?;

        let writer = index
            .writer(50_000_000)
            .map_err(|e| FlashError::Search(e.to_string()))?;

        Ok(Self {
            index,
            reader,
            writer: Arc::new(Mutex::new(writer)),
            schema,
            path_field,
            name_field,
            index_path: index_path.to_path_buf(),
        })
    }

    pub fn add_file(&self, path: &str, name: &str) -> Result<()> {
        let writer = self.writer.blocking_lock();

        let mut doc = TantivyDocument::default();
        doc.add_text(self.path_field, path);
        doc.add_text(self.name_field, name);

        writer
            .add_document(doc)
            .map_err(|e| FlashError::Index(e.to_string()))?;

        Ok(())
    }

    pub fn commit(&self) -> Result<()> {
        let mut writer = self.writer.blocking_lock();
        writer
            .commit()
            .map_err(|e| FlashError::Index(e.to_string()))?;
        Ok(())
    }

    pub fn search(&self, query: &str, limit: usize) -> Result<Vec<FilenameResult>> {
        // Reload reader to see latest changes
        self.reader
            .reload()
            .map_err(|e| FlashError::Index(e.to_string()))?;

        let searcher = self.reader.searcher();

        // Use regex query for filename matching
        let regex_query =
            RegexQuery::from_pattern(&format!("(?i){}", regex::escape(query)), self.name_field)
                .map_err(|e| FlashError::Search(e.to_string()))?;

        let top_docs = searcher
            .search(&regex_query, &TopDocs::with_limit(limit))
            .map_err(|e| FlashError::Search(e.to_string()))?;

        let mut results = Vec::new();

        for (_score, doc_address) in top_docs {
            let doc: TantivyDocument = searcher
                .doc(doc_address)
                .map_err(|e| FlashError::Search(e.to_string()))?;

            let path = doc
                .get_first(self.path_field)
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
                .unwrap_or_default();

            let name = doc
                .get_first(self.name_field)
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
                .unwrap_or_default();

            results.push(FilenameResult {
                file_path: path,
                file_name: name,
            });
        }

        Ok(results)
    }

    pub fn clear(&self) -> Result<()> {
        let mut writer = self.writer.blocking_lock();
        writer
            .delete_all_documents()
            .map_err(|e| FlashError::Index(e.to_string()))?;
        writer
            .commit()
            .map_err(|e| FlashError::Index(e.to_string()))?;
        Ok(())
    }

    pub fn get_stats(&self) -> Result<(usize, u64)> {
        let searcher = self.reader.searcher();
        let num_docs = searcher.num_docs() as usize;

        // Estimate index size
        let size = if self.index_path.exists() {
            walkdir::WalkDir::new(&self.index_path)
                .into_iter()
                .filter_map(|e| e.ok())
                .filter_map(|e| e.metadata().ok())
                .map(|m| m.len())
                .sum()
        } else {
            0
        };

        Ok((num_docs, size))
    }
}
