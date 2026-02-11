use tantivy::{Index, IndexReader, ReloadPolicy, Searcher};
use tantivy::query::QueryParser;
use tantivy::collector::TopDocs;
use tantivy::schema::Schema;
use serde::Serialize;
use crate::error::{FlashError, Result};

#[derive(Serialize, Debug, Clone)]
pub struct SearchResult {
    pub file_path: String,
    pub title: Option<String>,
    pub score: f32,
}

/// Manages searching the Tantivy index
pub struct IndexSearcher {
    reader: IndexReader,
    query_parser: QueryParser,
}

impl IndexSearcher {
    pub fn new(index: &Index) -> Result<Self> {
        let schema = index.schema();
        
        let reader = index
            .reader_builder()
            .reload_policy(ReloadPolicy::OnCommit)
            .try_into()
            .map_err(|e| FlashError::Index(e.to_string()))?;
        
        // Search across content and title fields
        let default_fields: Vec<_> = vec!["content", "title", "file_path"]
            .iter()
            .filter_map(|field_name| schema.get_field(field_name).ok())
            .collect();
        
        let query_parser = QueryParser::for_index(index, default_fields);
        
        Ok(Self {
            reader,
            query_parser,
        })
    }
    
    /// Search the index and return top results
    pub fn search(&self, query: &str, limit: usize) -> Result<Vec<SearchResult>> {
        let searcher = self.reader.searcher();
        
        let query = self.query_parser.parse_query(query)
            .map_err(|e| FlashError::Search(e.to_string()))?;
        
        let top_docs = searcher.search(&query, &TopDocs::with_limit(limit))
            .map_err(|e| FlashError::Search(e.to_string()))?;
        
        let schema = searcher.schema();
        let path_field = schema.get_field("file_path")
            .map_err(|_| FlashError::Index("file_path field not found".to_string()))?;
        let title_field = schema.get_field("title")
            .map_err(|_| FlashError::Index("title field not found".to_string()))?;
        
        let mut results = Vec::new();
        
        for (score, doc_address) in top_docs {
            let retrieved_doc = searcher.doc(doc_address)
                .map_err(|e| FlashError::Search(e.to_string()))?;
            
            let file_path = retrieved_doc.get_first(path_field)
                .and_then(|f| f.as_text())
                .map(|s| s.to_string())
                .unwrap_or_default();
            
            let title = retrieved_doc.get_first(title_field)
                .and_then(|f| f.as_text())
                .map(|s| s.to_string());
            
            results.push(SearchResult {
                file_path,
                title,
                score,
            });
        }
        
        Ok(results)
    }
}
