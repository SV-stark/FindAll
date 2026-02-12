use crate::error::{FlashError, Result};
use serde::{Deserialize, Serialize};
use std::ops::Bound;
use tantivy::collector::TopDocs;
use tantivy::query::{BooleanQuery, Occur, QueryParser, RangeQuery};
use tantivy::schema::{Field, Schema, Term, Value};
use tantivy::{Index, IndexReader, ReloadPolicy, TantivyDocument};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SearchResult {
    pub file_path: String,
    pub title: Option<String>,
    pub score: f32,
    /// Terms that matched for highlighting
    pub matched_terms: Vec<String>,
}

/// Statistics about the search index
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct IndexStatistics {
    pub total_documents: usize,
    pub total_size_bytes: u64,
    pub last_updated: Option<String>,
}

/// Manages searching the Tantivy index
pub struct IndexSearcher {
    reader: IndexReader,
    query_parser: QueryParser,
    schema: Schema,
    path_field: Field,
    title_field: Field,
    size_field: Field,
}

impl IndexSearcher {
    pub fn new(index: &Index) -> Result<Self> {
        let schema = index.schema();

        let reader = index
            .reader_builder()
            .reload_policy(ReloadPolicy::Manual)
            .try_into()
            .map_err(|e| FlashError::Search(e.to_string()))?;

        // Get field references once to avoid repeated lookups
        let path_field = schema
            .get_field("file_path")
            .map_err(|_| FlashError::Search("file_path field not found".to_string()))?;
        let title_field = schema
            .get_field("title")
            .map_err(|_| FlashError::Search("title field not found".to_string()))?;
        let size_field = schema
            .get_field("size")
            .map_err(|_| FlashError::Search("size field not found".to_string()))?;

        // Search across content, title, and file_path fields
        let default_fields: Vec<Field> = vec!["content", "title", "file_path"]
            .iter()
            .filter_map(|field_name| schema.get_field(field_name).ok())
            .collect();

        let query_parser = QueryParser::for_index(index, default_fields);

        Ok(Self {
            reader,
            query_parser,
            schema,
            path_field,
            title_field,
            size_field,
        })
    }

    /// Search the index and return top results with optional filters
    pub fn search(
        &self,
        query: &str,
        limit: usize,
        min_size: Option<u64>,
        max_size: Option<u64>,
        file_extensions: Option<&[String]>,
    ) -> Result<Vec<SearchResult>> {
        use super::query_parser::{ParsedQuery, extract_highlight_terms};
        
        let parsed = ParsedQuery::new(query);
        let highlight_terms = extract_highlight_terms(query);
        
        let searcher = self.reader.searcher();

        let text_query = self
            .query_parser
            .parse_query(&parsed.text_query)
            .map_err(|e| FlashError::Search(e.to_string()))?;

        // Build query with optional filters
        let mut combine: Vec<(Occur, Box<dyn tantivy::query::Query>)> =
            vec![(Occur::Must, text_query)];

        // Add size filters
        if min_size.is_some() || max_size.is_some() {
            let size_field_name = self.schema.get_field_name(self.size_field).to_string();
            let value_type = tantivy::schema::Type::U64;

            if let Some(min_val) = min_size {
                let lower = Term::from_field_u64(self.size_field, min_val);
                let upper = Term::from_field_u64(self.size_field, u64::MAX);
                let range = RangeQuery::new_term_bounds(
                    size_field_name.clone(),
                    value_type,
                    &Bound::Included(lower),
                    &Bound::Included(upper),
                );
                combine.push((Occur::Must, Box::new(range)));
            }

            if let Some(max_val) = max_size {
                let lower = Term::from_field_u64(self.size_field, 0);
                let upper = Term::from_field_u64(self.size_field, max_val);
                let range = RangeQuery::new_term_bounds(
                    size_field_name.clone(),
                    value_type,
                    &Bound::Included(lower),
                    &Bound::Included(upper),
                );
                combine.push((Occur::Must, Box::new(range)));
            }
        }

        // Build file extension filter as a boolean query clause
        if let Some(extensions) = file_extensions {
            if !extensions.is_empty() {
                let extension_queries: Vec<_> = extensions
                    .iter()
                    .filter_map(|ext| {
                        let ext_lower = ext.to_lowercase();
                        let ext_with_dot = if ext_lower.starts_with('.') {
                            ext_lower
                        } else {
                            format!(".{}", ext_lower)
                        };
                        // Use regex query for extension matching
                        Some(tantivy::query::RegexQuery::from_pattern(
                            &format!("{}$", regex::escape(&ext_with_dot)),
                            self.path_field,
                        ).ok()?)
                    })
                    .collect();

                if !extension_queries.is_empty() {
                    let extension_bool_query = tantivy::query::BooleanQuery::new(
                        extension_queries
                            .into_iter()
                            .map(|q| (Occur::Should, Box::new(q) as Box<dyn tantivy::query::Query>))
                            .collect(),
                    );
                    combine.push((Occur::Must, Box::new(extension_bool_query)));
                }
            }
        }

        let final_query: Box<dyn tantivy::query::Query> = if combine.len() == 1 {
            combine.remove(0).1
        } else {
            Box::new(BooleanQuery::new(combine))
        };

        let top_docs = searcher
            .search(&*final_query, &TopDocs::with_limit(limit))
            .map_err(|e| FlashError::Search(e.to_string()))?;

        let mut results = Vec::with_capacity(top_docs.len().min(limit));

        for (score, doc_address) in top_docs {
            let retrieved_doc: TantivyDocument = searcher
                .doc(doc_address)
                .map_err(|e| FlashError::Search(e.to_string()))?;

            let file_path = retrieved_doc
                .get_first(self.path_field)
                .and_then(|f| f.as_str())
                .map(|s: &str| s.to_string())
                .unwrap_or_default();

            let title = retrieved_doc
                .get_first(self.title_field)
                .and_then(|f| f.as_str())
                .map(|s: &str| s.to_string());

            results.push(SearchResult {
                file_path,
                title,
                score,
                matched_terms: highlight_terms.clone(),
            });

            if results.len() >= limit {
                break;
            }
        }

        Ok(results)
    }

    /// Get statistics about the index
    pub fn get_statistics(&self) -> Result<IndexStatistics> {
        let searcher = self.reader.searcher();
        let total_docs = searcher.num_docs() as usize;
        
        let mut total_size = 0u64;
        for segment_reader in searcher.segment_readers() {
            let store_reader = segment_reader.get_store_reader(1)?;
            for doc_id in 0..segment_reader.num_docs() {
                if let Ok(doc) = store_reader.get(doc_id) {
                    if let Some(value) = doc.get_first(self.size_field) {
                        if let Some(size) = value.as_u64() {
                            total_size += size;
                        }
                    }
                }
            }
        }
        
        Ok(IndexStatistics {
            total_documents: total_docs,
            total_size_bytes: total_size,
            last_updated: None,
        })
    }
}
