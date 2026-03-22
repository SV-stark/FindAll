use crate::error::{FlashError, Result};
use compact_str::CompactString;
use itertools::Itertools;
use moka::sync::Cache;
use serde::{Deserialize, Serialize};
use std::ops::Bound;
use std::sync::OnceLock;
use std::time::{Duration, Instant};
use tantivy::collector::TopDocs;
use tantivy::query::{BooleanQuery, FuzzyTermQuery, Occur, QueryParser, RangeQuery};
use tantivy::schema::{Field, IndexRecordOption, Value};
use tantivy::snippet::SnippetGenerator;
use tantivy::Term;
use tantivy::{Index, IndexReader, ReloadPolicy, TantivyDocument};

/// Maximum number of cached query results
const MAX_CACHE_SIZE: usize = 100;
/// Cache TTL in seconds
const CACHE_TTL_SECS: u64 = 30;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SearchResult {
    pub file_path: String,
    pub title: Option<CompactString>,
    pub score: f32,
    pub modified: Option<u64>,
    pub size: Option<u64>,
    pub extension: Option<CompactString>,
    /// Terms that matched for highlighting
    pub matched_terms: Vec<String>,
    /// Context snippets with highlighting
    pub snippets: Vec<String>,
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
#[derive(Clone)]
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
        self.cache.insert(
            key,
            CachedResult {
                results,
                timestamp: Instant::now(),
            },
        );
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
    path_field: Field,
    title_field: Field,
    size_field: Field,
    content_field: Field,
    extension_field: Field,
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
        if let Err(e) = reader.reload() {
            tracing::warn!("Failed to pre-warm index reader: {}", e);
        }

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
        let extension_field = schema
            .get_field("extension")
            .map_err(|_| FlashError::index_field("extension", "Field not found in schema"))?;

        // Search across content, title, and file_path fields
        let default_fields: Vec<Field> = ["content", "title", "file_path"]
            .iter()
            .filter_map(|field_name| schema.get_field(field_name).ok())
            .collect();

        let mut query_parser = QueryParser::for_index(index, default_fields);
        query_parser.set_conjunction_by_default();

        Ok(Self {
            reader,
            query_parser,
            path_field,
            title_field,
            size_field,
            content_field,
            extension_field,
            cache: QueryCache::new(),
            index_path,
        })
    }

    /// Search the index and return top results with optional filters
    pub async fn search(
        self: &std::sync::Arc<Self>,
        query: &str,
        limit: usize,
        min_size: Option<u64>,
        max_size: Option<u64>,
        file_extensions: Option<&[String]>,
    ) -> Result<Vec<SearchResult>> {
        let query_owned = query.to_string();
        let extensions_owned = file_extensions.map(|e| e.to_vec());
        // Arc::clone is O(1) — no heap allocation, just an atomic refcount bump
        let this = std::sync::Arc::clone(self);

        tokio::task::spawn_blocking(move || {
            this.search_sync(
                &query_owned,
                limit,
                min_size,
                max_size,
                extensions_owned.as_deref(),
            )
        })
        .await
        .map_err(|e| FlashError::search(query, format!("Search task failed: {}", e)))?
    }

    /// Synchronous search implementation
    pub fn search_sync(
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

        // Helper to run query with all filters
        #[allow(clippy::type_complexity)]
        let run_query = |text_query: Box<dyn tantivy::query::Query>| -> Result<(
            Box<dyn tantivy::query::Query>,
            Vec<(f32, tantivy::DocAddress)>,
        )> {
            let mut combine: Vec<(Occur, Box<dyn tantivy::query::Query>)> =
                vec![(Occur::Must, text_query)];

            if min_size.is_some() || max_size.is_some() {
                let lower = Term::from_field_u64(self.size_field, min_size.unwrap_or(0));
                let upper = Term::from_field_u64(self.size_field, max_size.unwrap_or(u64::MAX));
                let range = RangeQuery::new(Bound::Included(lower), Bound::Included(upper));
                combine.push((Occur::Must, Box::new(range)));
            }

            if let Some(extensions) = file_extensions {
                if !extensions.is_empty() {
                    let extension_queries: Vec<_> = extensions
                        .iter()
                        .map(|ext| {
                            let ext_lower = ext.to_lowercase();
                            let term =
                                tantivy::Term::from_field_text(self.extension_field, &ext_lower);
                            tantivy::query::TermQuery::new(term, IndexRecordOption::Basic)
                        })
                        .collect();

                    if !extension_queries.is_empty() {
                        let extension_bool_query = tantivy::query::BooleanQuery::new(
                            extension_queries
                                .into_iter()
                                .map(|q| {
                                    (Occur::Should, Box::new(q) as Box<dyn tantivy::query::Query>)
                                })
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

            Ok((final_query, top_docs))
        };

        // TIER 1: Exact search
        let exact_query = self
            .query_parser
            .parse_query(&parsed.text_query)
            .unwrap_or_else(|_| Box::new(tantivy::query::AllQuery));
        let (mut final_query, mut top_docs) = run_query(exact_query)?;

        // TIER 2: Fuzzy Fallback
        static PHRASE_REGEX: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
        let phrase_regex = PHRASE_REGEX.get_or_init(|| regex::Regex::new(r#""([^"]+)""#).unwrap());

        if top_docs.len() < limit
            && !phrase_regex.is_match(&parsed.text_query)
            && !parsed.text_query.trim().is_empty()
        {
            let fuzzy_query = self.build_fuzzy_query(&parsed.text_query)?;
            let (fuzzy_final_query, fuzzy_docs) = run_query(fuzzy_query)?;

            // Prefer highlighting based on the more permissive fuzzy query
            final_query = fuzzy_final_query;

            let seen: std::collections::HashSet<tantivy::DocAddress> =
                top_docs.iter().map(|(_, addr)| *addr).collect();

            // Use itertools to chain and filter unique documents
            top_docs = top_docs
                .into_iter()
                .chain(
                    fuzzy_docs
                        .into_iter()
                        .filter(|(_, addr)| !seen.contains(addr)),
                )
                .take(limit)
                .sorted_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal))
                .collect();
        }

        let mut results = Vec::with_capacity(top_docs.len().min(limit));

        // Create snippet generator once outside the loop
        let snippet_generator =
            SnippetGenerator::create(&searcher, &*final_query, self.content_field)?;

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
                .map(|s: &str| CompactString::from(s));


            let extension = retrieved_doc
                .get_first(self.extension_field)
                .and_then(|f| f.as_str())
                .map(|s: &str| CompactString::from(s));


            // Get fast fields for size and modified
            let size = searcher
                .segment_reader(doc_address.segment_ord)
                .fast_fields()
                .u64("size")
                .ok()
                .map(|f| f.values.get_val(doc_address.doc_id));

            let modified = searcher
                .segment_reader(doc_address.segment_ord)
                .fast_fields()
                .date("modified")
                .ok()
                .map(|f| {
                    let date = f.values.get_val(doc_address.doc_id);
                    date.into_timestamp_secs() as u64
                });

            // Generates snippets and replaces HTML tags.
            // NOTE: Offloading full rendering to UI.
            let snippet = snippet_generator.snippet_from_doc(&retrieved_doc);
            let snippet_text = snippet
                .to_html()
                .replace("<b>", "")
                .replace("</b>", "")
                .replace("&lt;", "<")
                .replace("&gt;", ">")
                .replace("&amp;", "&");

            let snippets = if snippet_text.is_empty() {
                vec![]
            } else {
                vec![snippet_text]
            };

            results.push(SearchResult {
                file_path,
                title,
                score,
                modified,
                size,
                extension,
                matched_terms: highlight_terms.clone(),
                snippets,
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
        static PHRASE_REGEX: OnceLock<regex::Regex> = OnceLock::new();
        let phrase_regex = PHRASE_REGEX.get_or_init(|| regex::Regex::new(r#""([^"]+)""#).unwrap());

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
            return Ok(Box::new(tantivy::query::AllQuery));
        }

        if terms.len() == 1 {
            // Single term - try exact first, then fuzzy across all relevant fields
            let term_text = terms[0];
            let mut subqueries: Vec<(Occur, Box<dyn tantivy::query::Query>)> = Vec::new();

            for field in [self.content_field, self.title_field, self.path_field] {
                let term = Term::from_field_text(field, term_text);
                let exact = tantivy::query::TermQuery::new(
                    term.clone(),
                    tantivy::schema::IndexRecordOption::Basic,
                );
                let fuzzy = FuzzyTermQuery::new(term, 2, true);
                subqueries.push((Occur::Should, Box::new(exact)));
                subqueries.push((Occur::Should, Box::new(fuzzy)));
            }

            Ok(Box::new(BooleanQuery::new(subqueries)))
        } else {
            // Multiple terms - build fuzzy query for each with AND logic
            let mut and_subqueries: Vec<(Occur, Box<dyn tantivy::query::Query>)> = Vec::new();

            for term_text in terms {
                let mut term_subqueries: Vec<(Occur, Box<dyn tantivy::query::Query>)> = Vec::new();
                for field in [self.content_field, self.title_field, self.path_field] {
                    let term = Term::from_field_text(field, term_text);
                    let exact = tantivy::query::TermQuery::new(
                        term.clone(),
                        tantivy::schema::IndexRecordOption::Basic,
                    );
                    let fuzzy = FuzzyTermQuery::new(term, 2, true);
                    term_subqueries.push((Occur::Should, Box::new(exact)));
                    term_subqueries.push((Occur::Should, Box::new(fuzzy)));
                }
                and_subqueries.push((Occur::Must, Box::new(BooleanQuery::new(term_subqueries))));
            }

            Ok(Box::new(BooleanQuery::new(and_subqueries)))
        }
    }

    /// Invalidate the search cache (call after index updates)
    pub fn invalidate_cache(&self) {
        self.cache.invalidate();
    }

    /// Get recent files using Tantivy's fast fields
    pub fn get_recent_files(&self, limit: usize) -> Result<Vec<SearchResult>> {
        let searcher = self.reader.searcher();
        let top_docs = searcher
            .search(
                &tantivy::query::AllQuery,
                &TopDocs::with_limit(limit)
                    .order_by_fast_field::<u64>("modified", tantivy::Order::Desc),
            )
            .map_err(|e| FlashError::search("recent files", e.to_string()))?;

        let mut results = Vec::with_capacity(top_docs.len().min(limit));

        for (_date, doc_address) in top_docs {
            let retrieved_doc: TantivyDocument = searcher.doc(doc_address).map_err(|e| {
                FlashError::search(
                    "recent files",
                    format!("Failed to retrieve document: {}", e),
                )
            })?;

            let file_path = retrieved_doc
                .get_first(self.path_field)
                .and_then(|f| f.as_str())
                .map(|s: &str| s.to_string())
                .unwrap_or_default();

            let title = retrieved_doc
                .get_first(self.title_field)
                .and_then(|f| f.as_str())
                .map(|s: &str| CompactString::from(s));


            let extension = retrieved_doc
                .get_first(self.extension_field)
                .and_then(|f| f.as_str())
                .map(|s: &str| CompactString::from(s));


            // Get fast fields for size and modified
            let size = searcher
                .segment_reader(doc_address.segment_ord)
                .fast_fields()
                .u64("size")
                .ok()
                .map(|f| f.values.get_val(doc_address.doc_id));

            let modified = searcher
                .segment_reader(doc_address.segment_ord)
                .fast_fields()
                .date("modified")
                .ok()
                .map(|f| {
                    let date = f.values.get_val(doc_address.doc_id);
                    date.into_timestamp_secs() as u64
                });

            results.push(SearchResult {
                file_path,
                title,
                score: 1.0,
                modified,
                size,
                extension,
                matched_terms: vec![],
                snippets: vec![],
            });
        }

        // Reverse because order_by_fast_field might sort ascending? We'll test it.
        // Actually Tantivy sorts in ascending order by default.
        results.reverse();

        Ok(results)
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
            last_updated: Some(jiff::Zoned::now().strftime("%Y-%m-%d %H:%M:%S").to_string()),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_key_equality() {
        let key1 = CacheKey {
            query: "test".to_string(),
            limit: 10,
            min_size: None,
            max_size: None,
            extensions: None,
        };
        let key2 = CacheKey {
            query: "test".to_string(),
            limit: 10,
            min_size: None,
            max_size: None,
            extensions: None,
        };
        assert_eq!(key1, key2);

        let key3 = CacheKey {
            query: "diff".to_string(),
            ..key1.clone()
        };
        assert_ne!(key1, key3);
    }

    #[test]
    fn test_query_cache_insert_get() {
        let cache = QueryCache::new();
        let key = CacheKey {
            query: "test".to_string(),
            limit: 10,
            min_size: None,
            max_size: None,
            extensions: None,
        };
        let results = vec![SearchResult {
            file_path: "path".to_string(),
            title: None,
            score: 1.0,
            modified: None,
            size: None,
            extension: None,
            matched_terms: vec!["test".to_string()],
            snippets: vec!["snippet".to_string()],
        }];
        cache.insert(key.clone(), results.clone());
        let cached = cache.get(&key);
        assert!(cached.is_some());
        assert_eq!(cached.unwrap().len(), 1);
    }

    #[test]
    fn test_query_cache_invalidate() {
        let cache = QueryCache::new();
        let key = CacheKey {
            query: "test".to_string(),
            limit: 10,
            min_size: None,
            max_size: None,
            extensions: None,
        };
        cache.insert(key.clone(), vec![]);
        cache.invalidate();
        assert!(cache.get(&key).is_none());
    }
}
