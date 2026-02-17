use crate::error::{FlashError, Result};
use moka::sync::Cache;
use serde::{Deserialize, Serialize};
use std::ops::Bound;
use std::time::{Duration, Instant};
use tantivy::collector::TopDocs;
use tantivy::query::{BooleanQuery, FuzzyTermQuery, Occur, QueryParser, RangeQuery, Query};
use tantivy::schema::{Field, IndexRecordOption, TextOptions, TEXT, STORED, STRING, Schema, Value};
use tantivy::Term;
use std::sync::Arc;
use tantivy::{Index, IndexReader, ReloadPolicy, TantivyDocument};

/// Maximum number of cached query results
const MAX_CACHE_SIZE: usize = 100;
/// Cache TTL in seconds
const CACHE_TTL_SECS: u64 = 30;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SearchResult {
    pub file_path: String,
    pub title: Option<String>,
    pub score: f32,
    /// Terms that matched for highlighting
    pub matched_terms: Vec<String>,
}

/// Statistics about the search index
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct IndexStatistics {
    pub total_documents: usize,
    pub total_size_bytes: u64,
    pub last_updated: Option<String>,
}

/// Cached search result with timestamp
#[derive(Clone)]
struct CachedResult {
    results: Vec<SearchResult>,
    #[allow(dead_code)]
    timestamp: Instant,
}

/// Cache key for search queries
#[derive(Clone, Debug, Hash, Eq, PartialEq)]
pub(crate) struct CacheKey {
    query: String,
    limit: usize,
    min_size: Option<u64>,
    max_size: Option<u64>,
    extensions: Option<Vec<String>>,
}

/// LRU-style query result cache using moka + ahash
pub struct QueryCache {
    cache: Cache<CacheKey, CachedResult>,
}

impl QueryCache {
    pub fn new() -> Self {
        Self {
            cache: Cache::builder()
                .max_capacity(MAX_CACHE_SIZE as u64)
                .time_to_live(Duration::from_secs(CACHE_TTL_SECS))
                .build(),
        }
    }

    pub(crate) fn get(&self, key: &CacheKey) -> Option<Vec<SearchResult>> {
        self.cache.get(key).map(|cached| cached.results)
    }

    pub(crate) fn insert(&self, key: CacheKey, results: Vec<SearchResult>) {
        self.cache.insert(key, CachedResult {
            results,
            timestamp: Instant::now(),
        });
    }

    pub fn invalidate(&self) {
        self.cache.invalidate_all();
    }
}

impl Default for QueryCache {
    fn default() -> Self {
        Self::new()
    }
}

/// Manages searching the Tantivy index
pub struct IndexSearcher {
    reader: IndexReader,
    query_parser: QueryParser,
    schema: Schema,
    path_field: Field,
    title_field: Field,
    size_field: Field,
    content_field: Field,
    cache: QueryCache,
    index_path: std::path::PathBuf,
}

impl IndexSearcher {
    pub fn new(index: &Index, index_path: std::path::PathBuf) -> Result<Self> {
        let schema = index.schema();

        // Pre-warm the reader by loading index into memory on startup
        // This ensures first search is fast (no initial load latency)
        let reader = index
            .reader_builder()
            .reload_policy(ReloadPolicy::OnCommitWithDelay)
            .try_into()
            .map_err(|e| FlashError::search("create_index_reader", e.to_string()))?;

        // Pre-warm: load the index reader
        reader.reload().ok();

        // Get field references once to avoid repeated lookups
        let path_field = schema
            .get_field("file_path")
            .map_err(|_| FlashError::index_field("file_path", "Field not found in schema"))?;
        let title_field = schema
            .get_field("title")
            .map_err(|_| FlashError::index_field("title", "Field not found in schema"))?;
        let size_field = schema
            .get_field("size")
            .map_err(|_| FlashError::index_field("size", "Field not found in schema"))?;
        let content_field = schema
            .get_field("content")
            .map_err(|_| FlashError::index_field("content", "Field not found in schema"))?;

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
            content_field,
            cache: QueryCache::new(),
            index_path,
        })
    }



    /// Search the index and return top results with optional filters
    pub async fn search(
        &self,
        query: &str,
        limit: usize,
        min_size: Option<u64>,
        max_size: Option<u64>,
        file_extensions: Option<&[String]>,
    ) -> Result<Vec<SearchResult>> {
        use super::query_parser::{extract_highlight_terms, ParsedQuery};

        // Create cache key
        let cache_key = CacheKey {
            query: query.to_string(),
            limit,
            min_size,
            max_size,
            extensions: file_extensions.map(|e| e.to_vec()),
        };

        // Check cache first
        if let Some(cached) = self.cache.get(&cache_key) {
            return Ok(cached);
        }

        let parsed = ParsedQuery::new(query);
        let highlight_terms = extract_highlight_terms(query);

        let searcher = self.reader.searcher();

        // Build the main query - use fuzzy search for better typo tolerance
        let text_query = self.build_fuzzy_query(&parsed.text_query)?;

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
                        Some(
                            tantivy::query::RegexQuery::from_pattern(
                                &format!("{}$", regex::escape(&ext_with_dot)),
                                self.path_field,
                            )
                            .ok()?,
                        )
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
            .map_err(|e| FlashError::search(query, e.to_string()))?;

        let mut results = Vec::with_capacity(top_docs.len().min(limit));

        for (score, doc_address) in top_docs {
            let retrieved_doc: TantivyDocument = searcher.doc(doc_address).map_err(|e| {
                FlashError::search(query, format!("Failed to retrieve document: {}", e))
            })?;

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

        // Cache the results
        self.cache.insert(cache_key, results.clone());

        Ok(results)
    }

    /// Build a fuzzy query for better typo tolerance
    fn build_fuzzy_query(&self, text_query: &str) -> Result<Box<dyn tantivy::query::Query>> {
        // Check if it's a phrase query (contains quoted strings)
        let phrase_regex = regex::Regex::new(r#""([^"]+)""#).unwrap();
        
        if phrase_regex.is_match(text_query) {
            // For phrase queries, use the query parser with phrase support
            return Ok(Box::new(
                self.query_parser
                    .parse_query(text_query)
                    .map_err(|e| FlashError::search(text_query, e.to_string()))?,
            ));
        }

        // For regular queries, build a fuzzy query with OR for each term
        let terms: Vec<&str> = text_query.split_whitespace().collect();
        
        if terms.is_empty() || (terms.len() == 1 && terms[0] == "*") {
            // Match all
            return Ok(Box::new(
                tantivy::query::AllQuery
            ));
        }

        if terms.len() == 1 {
            // Single term - try exact first, then fuzzy
            let term_text = terms[0];
            let term = Term::from_field_text(self.content_field, term_text);
            
            // Try exact match first (higher priority)
            let exact = tantivy::query::TermQuery::new(
                term,
                tantivy::schema::IndexRecordOption::Basic,
            );
            
            // Add fuzzy variant with edit distance of 2
            let fuzzy_term = Term::from_field_text(self.content_field, term_text);
            let fuzzy = FuzzyTermQuery::new(fuzzy_term, 2, true);
            
            // Combine with OR (exact first)
            let combined = BooleanQuery::new(vec![
                (Occur::Should, Box::new(exact)),
                (Occur::Should, Box::new(fuzzy)),
            ]);
            
            Ok(Box::new(combined))
        } else {
            // Multiple terms - build fuzzy query for each with AND logic
            let mut subqueries: Vec<(Occur, Box<dyn tantivy::query::Query>)> = Vec::new();
            
            for term_text in terms {
                let term = Term::from_field_text(self.content_field, term_text);
                
                // Exact term query
                let exact = tantivy::query::TermQuery::new(
                    term.clone(),
                    tantivy::schema::IndexRecordOption::Basic,
                );
                
                // Fuzzy variant
                let fuzzy = FuzzyTermQuery::new(term, 2, true);
                
                // Combine exact and fuzzy for this term
                let term_query = BooleanQuery::new(vec![
                    (Occur::Should, Box::new(exact)),
                    (Occur::Should, Box::new(fuzzy)),
                ]);
                
                subqueries.push((Occur::Must, Box::new(term_query)));
            }
            
            Ok(Box::new(BooleanQuery::new(subqueries)))
        }
    }

    /// Invalidate the search cache (call after index updates)
    pub fn invalidate_cache(&self) {
        self.cache.invalidate();
    }

    /// Get statistics about the index
    pub fn get_statistics(&self) -> Result<IndexStatistics> {
        let searcher = self.reader.searcher();
        let total_docs = searcher.num_docs() as usize;

        let mut total_size = 0;
        if let Ok(entries) = std::fs::read_dir(&self.index_path) {
            for entry in entries.flatten() {
                if let Ok(metadata) = entry.metadata() {
                    if metadata.is_file() {
                        total_size += metadata.len();
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
