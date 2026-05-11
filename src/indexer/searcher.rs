use super::query_parser::{ParsedQuery, extract_highlight_terms};
use crate::error::{FlashError, Result};
use compact_str::CompactString;
use moka::sync::Cache;
use serde::{Deserialize, Serialize};
use std::ops::Bound;
use std::time::Duration;
use tantivy::collector::TopDocs;
use tantivy::query::{Occur, RangeQuery};
use tantivy::schema::{Field, IndexRecordOption, Term, Value};
use tantivy::{Index, IndexReader};

/// Search result containing file metadata and score
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub file_path: String,
    pub score: f32,
    pub title: Option<CompactString>,
    pub extension: Option<CompactString>,
    pub modified: Option<u64>,
    pub size: Option<u64>,
    pub matched_terms: Vec<String>,
    pub snippets: Vec<String>,
}

impl SearchResult {
    pub fn builder() -> SearchResultBuilder {
        SearchResultBuilder::default()
    }
}

#[derive(Default)]
pub struct SearchResultBuilder {
    file_path: Option<String>,
    score: Option<f32>,
    title: Option<CompactString>,
    extension: Option<CompactString>,
    modified: Option<u64>,
    size: Option<u64>,
    matched_terms: Option<Vec<String>>,
    snippets: Option<Vec<String>>,
}

impl SearchResultBuilder {
    pub fn file_path(mut self, file_path: String) -> Self {
        self.file_path = Some(file_path);
        self
    }

    pub const fn score(mut self, score: f32) -> Self {
        self.score = Some(score);
        self
    }

    pub fn title(mut self, title: Option<CompactString>) -> Self {
        self.title = title;
        self
    }

    pub fn maybe_title(self, title: Option<CompactString>) -> Self {
        self.title(title)
    }

    pub fn extension(mut self, extension: Option<CompactString>) -> Self {
        self.extension = extension;
        self
    }

    pub fn maybe_extension(self, extension: Option<CompactString>) -> Self {
        self.extension(extension)
    }

    pub const fn modified(mut self, modified: Option<u64>) -> Self {
        self.modified = modified;
        self
    }

    pub const fn size(mut self, size: Option<u64>) -> Self {
        self.size = size;
        self
    }

    pub fn matched_terms(mut self, matched_terms: Vec<String>) -> Self {
        self.matched_terms = Some(matched_terms);
        self
    }

    pub fn snippets(mut self, snippets: Vec<String>) -> Self {
        self.snippets = Some(snippets);
        self
    }

    pub fn build(self) -> SearchResult {
        SearchResult {
            file_path: self.file_path.expect("file_path is required"),
            score: self.score.expect("score is required"),
            title: self.title,
            extension: self.extension,
            modified: self.modified,
            size: self.size,
            matched_terms: self.matched_terms.expect("matched_terms is required"),
            snippets: self.snippets.expect("snippets is required"),
        }
    }
}

/// Statistics about the index
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct IndexStatistics {
    pub total_documents: usize,
    pub total_size_bytes: u64,
}

/// Cache key for search queries
#[derive(Clone, Debug, Hash, Eq, PartialEq)]
pub(crate) struct CacheKey {
    pub(crate) query: String,
    pub(crate) limit: usize,
    pub(crate) min_size: Option<u64>,
    pub(crate) max_size: Option<u64>,
    pub(crate) min_modified: Option<u64>,
    pub(crate) extensions: Option<smallvec::SmallVec<[CompactString; 8]>>,
    pub(crate) case_sensitive: bool,
}

#[derive(Debug, Clone)]
pub struct SearchParams<'a> {
    pub query: &'a str,
    pub limit: usize,
    pub min_size: Option<u64>,
    pub max_size: Option<u64>,
    pub min_modified: Option<u64>,
    pub file_extensions: Option<&'a [String]>,
    pub case_sensitive: bool,
}

impl<'a> SearchParams<'a> {
    pub fn builder() -> SearchParamsBuilder<'a> {
        SearchParamsBuilder::default()
    }
}

#[derive(Default)]
pub struct SearchParamsBuilder<'a> {
    query: Option<&'a str>,
    limit: Option<usize>,
    min_size: Option<u64>,
    max_size: Option<u64>,
    min_modified: Option<u64>,
    file_extensions: Option<&'a [String]>,
    case_sensitive: Option<bool>,
}

impl<'a> SearchParamsBuilder<'a> {
    pub const fn query(mut self, query: &'a str) -> Self {
        self.query = Some(query);
        self
    }

    pub const fn limit(mut self, limit: usize) -> Self {
        self.limit = Some(limit);
        self
    }

    pub const fn min_size(mut self, min_size: Option<u64>) -> Self {
        self.min_size = min_size;
        self
    }

    pub const fn maybe_min_size(self, min_size: Option<u64>) -> Self {
        self.min_size(min_size)
    }

    pub const fn max_size(mut self, max_size: Option<u64>) -> Self {
        self.max_size = max_size;
        self
    }

    pub const fn maybe_max_size(self, max_size: Option<u64>) -> Self {
        self.max_size(max_size)
    }

    pub const fn min_modified(mut self, min_modified: Option<u64>) -> Self {
        self.min_modified = min_modified;
        self
    }

    pub const fn maybe_min_modified(self, min_modified: Option<u64>) -> Self {
        self.min_modified(min_modified)
    }

    pub const fn file_extensions(mut self, extensions: &'a [String]) -> Self {
        self.file_extensions = Some(extensions);
        self
    }

    pub const fn maybe_file_extensions(self, extensions: Option<&'a [String]>) -> Self {
        if let Some(exts) = extensions {
            self.file_extensions(exts)
        } else {
            self
        }
    }

    pub const fn case_sensitive(mut self, case_sensitive: bool) -> Self {
        self.case_sensitive = Some(case_sensitive);
        self
    }

    pub const fn maybe_case_sensitive(self, case_sensitive: Option<bool>) -> Self {
        if let Some(cs) = case_sensitive {
            self.case_sensitive(cs)
        } else {
            self
        }
    }

    pub const fn build(self) -> SearchParams<'a> {
        SearchParams {
            query: self.query.expect("query is required"),
            limit: self.limit.expect("limit is required"),
            min_size: self.min_size,
            max_size: self.max_size,
            min_modified: self.min_modified,
            file_extensions: self.file_extensions,
            case_sensitive: self.case_sensitive.expect("case_sensitive is required"),
        }
    }
}

/// LRU-style query result cache using moka + ahash
#[derive(Clone)]
pub struct QueryCache {
    cache: Cache<CacheKey, Vec<SearchResult>>,
}

impl Default for QueryCache {
    fn default() -> Self {
        Self::new()
    }
}

impl QueryCache {
    pub fn new() -> Self {
        Self {
            cache: Cache::builder()
                .max_capacity(100)
                .time_to_live(Duration::from_mins(5)) // 5 minutes TTL
                .build(),
        }
    }

    pub(crate) fn get(&self, key: &CacheKey) -> Option<Vec<SearchResult>> {
        self.cache.get(key)
    }

    pub(crate) fn insert(&self, key: CacheKey, results: Vec<SearchResult>) {
        self.cache.insert(key, results);
    }

    pub fn invalidate(&self) {
        self.cache.invalidate_all();
    }
}

/// Handles search operations on the index
pub struct IndexSearcher {
    reader: IndexReader,
    index_path: std::path::PathBuf,
    cache: QueryCache,
    path_field: Field,
    content_field: Field,
    title_field: Field,
    modified_field: Field,
    size_field: Field,
    extension_field: Field,
}

impl IndexSearcher {
    pub fn new(index: &Index, index_path: std::path::PathBuf) -> Result<Self> {
        let reader = index
            .reader_builder()
            .reload_policy(tantivy::ReloadPolicy::OnCommitWithDelay)
            .try_into()
            .map_err(|e| FlashError::index(format!("Failed to create index reader: {e}")))?;

        let schema = index.schema();
        let path_field = schema
            .get_field("file_path")
            .map_err(|_| FlashError::index_field("file_path", "Field not found"))?;
        let content_field = schema
            .get_field("content")
            .map_err(|_| FlashError::index_field("content", "Field not found"))?;
        let title_field = schema
            .get_field("title")
            .map_err(|_| FlashError::index_field("title", "Field not found"))?;
        let modified_field = schema
            .get_field("modified")
            .map_err(|_| FlashError::index_field("modified", "Field not found"))?;
        let size_field = schema
            .get_field("size")
            .map_err(|_| FlashError::index_field("size", "Field not found"))?;
        let extension_field = schema
            .get_field("extension")
            .map_err(|_| FlashError::index_field("extension", "Field not found"))?;

        Ok(Self {
            reader,
            index_path,
            cache: QueryCache::new(),
            path_field,
            content_field,
            title_field,
            modified_field,
            size_field,
            extension_field,
        })
    }

    /// Search the index and return top results with optional filters
    pub async fn search(
        self: &std::sync::Arc<Self>,
        params: SearchParams<'_>,
    ) -> Result<Vec<SearchResult>> {
        let this = std::sync::Arc::clone(self);

        let query_owned = params.query.to_string();
        let extensions_owned: Option<Vec<String>> = params.file_extensions.map(<[String]>::to_vec);
        let limit = params.limit;
        let min_size = params.min_size;
        let max_size = params.max_size;
        let min_modified = params.min_modified;
        let case_sensitive = params.case_sensitive;

        tokio::task::spawn_blocking(move || {
            let params = SearchParams {
                query: &query_owned,
                limit,
                min_size,
                max_size,
                min_modified,
                file_extensions: extensions_owned.as_deref(),
                case_sensitive,
            };
            this.search_sync(&params)
        })
        .await
        .map_err(|e| FlashError::search(params.query, format!("Search task failed: {e}")))?
    }

    /// Synchronous search implementation
    ///
    /// # Panics
    ///
    /// Panics if the phrase search regex fails to compile.
    #[allow(clippy::too_many_lines)]
    pub fn search_sync(&self, params: &SearchParams<'_>) -> Result<Vec<SearchResult>> {
        let file_extensions = params.file_extensions.map(|e| {
            e.iter()
                .map(|s| CompactString::from(s.as_str()))
                .collect::<smallvec::SmallVec<[CompactString; 8]>>()
        });

        // Create cache key
        let cache_key = CacheKey {
            query: params.query.to_string(),
            limit: params.limit,
            min_size: params.min_size,
            max_size: params.max_size,
            min_modified: params.min_modified,
            extensions: file_extensions.clone(),
            case_sensitive: params.case_sensitive,
        };

        // Check cache first
        if let Some(cached) = self.cache.get(&cache_key) {
            return Ok(cached);
        }

        let parsed = ParsedQuery::new(params.query, params.case_sensitive);
        let highlight_terms = extract_highlight_terms(params.query, params.case_sensitive);

        let searcher = self.reader.searcher();

        // Helper to run query with all filters
        #[allow(clippy::type_complexity)]
        let run_query = |text_query: Box<dyn tantivy::query::Query>,
                         limit: usize,
                         query_str: &str|
         -> Result<(
            Box<dyn tantivy::query::Query>,
            Vec<(f32, tantivy::DocAddress)>,
        )> {
            let mut combine: Vec<(Occur, Box<dyn tantivy::query::Query>)> =
                vec![(Occur::Must, text_query)];

            if params.min_size.is_some() || params.max_size.is_some() {
                let lower = Term::from_field_u64(self.size_field, params.min_size.unwrap_or(0));
                let upper =
                    Term::from_field_u64(self.size_field, params.max_size.unwrap_or(u64::MAX));
                let range = RangeQuery::new(Bound::Included(lower), Bound::Included(upper));
                combine.push((Occur::Must, Box::new(range)));
            }

            if let Some(min_mod) = params.min_modified {
                let lower = Term::from_field_date(
                    self.modified_field,
                    tantivy::DateTime::from_timestamp_secs(
                        i64::try_from(min_mod).unwrap_or(i64::MAX),
                    ),
                );
                let upper = Term::from_field_date(
                    self.modified_field,
                    tantivy::DateTime::from_timestamp_secs(i64::MAX / 1000),
                );
                let range = RangeQuery::new(Bound::Included(lower), Bound::Included(upper));
                combine.push((Occur::Must, Box::new(range)));
            }

            if let Some(ref extensions) = file_extensions
                && !extensions.is_empty()
            {
                let extension_queries: Vec<_> = extensions
                    .iter()
                    .map(|ext| {
                        let ext_lower = ext.to_lowercase();
                        let term = tantivy::Term::from_field_text(self.extension_field, &ext_lower);
                        tantivy::query::TermQuery::new(term, IndexRecordOption::Basic)
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

            let final_query = tantivy::query::BooleanQuery::new(combine);
            let top_docs = searcher
                .search(&final_query, &TopDocs::with_limit(limit).order_by_score())
                .map_err(|e| FlashError::search(query_str, e.to_string()))?;

            Ok((
                Box::new(final_query) as Box<dyn tantivy::query::Query>,
                top_docs,
            ))
        };

        let (_final_query, top_docs) = if parsed.text_query == "*" {
            run_query(
                Box::new(tantivy::query::AllQuery),
                params.limit,
                params.query,
            )?
        } else {
            let mut query_parser =
                tantivy::query::QueryParser::for_index(searcher.index(), vec![self.content_field]);
            query_parser.set_conjunction_by_default();

            let query_result = query_parser.parse_query(&parsed.text_query);

            if let Ok(q) = query_result {
                run_query(q, params.limit, params.query)?
            } else {
                let fuzzy_query = tantivy::query::FuzzyTermQuery::new(
                    Term::from_field_text(self.content_field, &parsed.text_query),
                    1,
                    true,
                );
                run_query(Box::new(fuzzy_query), params.limit, params.query)?
            }
        };

        if top_docs.len() < params.limit
            && !parsed.text_query.contains(' ')
            && parsed.text_query != "*"
        {
            let phrase_regex =
                regex::Regex::new(r#""([^"]+)""#).expect("Invalid regex for phrase search");
            if !phrase_regex.is_match(&parsed.text_query) {
                let fuzzy_query = tantivy::query::FuzzyTermQuery::new(
                    Term::from_field_text(self.content_field, &parsed.text_query),
                    1,
                    true,
                );
                if let Ok((_, fuzzy_docs)) =
                    run_query(Box::new(fuzzy_query), params.limit, params.query)
                {
                    let mut combined = top_docs;
                    let existing_ids: std::collections::HashSet<_> =
                        combined.iter().map(|(_, addr)| *addr).collect();

                    for (score, addr) in fuzzy_docs {
                        if !existing_ids.contains(&addr) {
                            combined.push((score * 0.8, addr));
                        }
                    }
                    combined
                        .sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
                    return self.process_top_docs(
                        &searcher,
                        combined.into_iter().take(params.limit).collect(),
                        params.query,
                        &highlight_terms,
                        &cache_key,
                    );
                }
            }
        }

        self.process_top_docs(
            &searcher,
            top_docs,
            params.query,
            &highlight_terms,
            &cache_key,
        )
    }

    fn process_top_docs(
        &self,
        searcher: &tantivy::Searcher,
        top_docs: Vec<(f32, tantivy::DocAddress)>,
        query: &str,
        highlight_terms: &[String],
        cache_key: &CacheKey,
    ) -> Result<Vec<SearchResult>> {
        let mut results = Vec::with_capacity(top_docs.len().min(cache_key.limit));

        for (score, doc_address) in top_docs {
            let doc: tantivy::TantivyDocument = searcher
                .doc(doc_address)
                .map_err(|e| FlashError::search(query, e.to_string()))?;

            match self.retrieve_result_with_doc(
                searcher,
                query,
                score,
                doc_address,
                &doc,
                highlight_terms,
            ) {
                Ok(result) => {
                    results.push(result);
                }
                Err(e) => {
                    tracing::error!("Error retrieving result: {}", e);
                }
            }

            if results.len() >= cache_key.limit {
                break;
            }
        }

        self.cache.insert(cache_key.clone(), results.clone());
        Ok(results)
    }

    fn retrieve_result_with_doc(
        &self,
        searcher: &tantivy::Searcher,
        _query: &str,
        score: f32,
        doc_address: tantivy::DocAddress,
        tantivy_doc: &tantivy::TantivyDocument,
        highlight_terms: &[String],
    ) -> Result<SearchResult> {
        let file_path = tantivy_doc
            .get_first(self.path_field)
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();

        let title = tantivy_doc
            .get_first(self.title_field)
            .and_then(|v| v.as_str())
            .map(CompactString::from);

        let extension = tantivy_doc
            .get_first(self.extension_field)
            .and_then(|v| v.as_str())
            .map(CompactString::from);

        let size = tantivy_doc
            .get_first(self.size_field)
            .and_then(|v| v.as_u64());

        let modified = searcher
            .segment_reader(doc_address.segment_ord)
            .fast_fields()
            .date("modified")
            .ok()
            .map(|f| {
                let date = f.values.get_val(doc_address.doc_id);
                u64::try_from(date.into_timestamp_secs()).unwrap_or(0)
            });

        Ok(SearchResult {
            file_path,
            score,
            title,
            extension,
            modified,
            size,
            matched_terms: highlight_terms.to_vec(),
            snippets: Vec::new(),
        })
    }

    pub fn get_statistics(&self) -> Result<IndexStatistics> {
        let searcher = self.reader.searcher();
        let total_docs = usize::try_from(searcher.num_docs()).unwrap_or(usize::MAX);

        let mut total_size = 0;
        if let Ok(entries) = std::fs::read_dir(&self.index_path) {
            for entry in entries.flatten() {
                if let Ok(metadata) = entry.metadata()
                    && metadata.is_file()
                {
                    total_size += metadata.len();
                }
            }
        }

        Ok(IndexStatistics {
            total_documents: total_docs,
            total_size_bytes: total_size,
        })
    }

    pub fn get_recent_files(&self, limit: usize) -> Result<Vec<SearchResult>> {
        let searcher = self.reader.searcher();
        let query = tantivy::query::AllQuery;

        let top_docs = searcher
            .search(
                &query,
                &TopDocs::with_limit(limit)
                    .order_by_fast_field::<i64>("modified", tantivy::Order::Desc),
            )
            .map_err(|e| FlashError::index(format!("Failed to get recent files: {e}")))?;

        let mut results = Vec::new();
        for (_mod_time, doc_address) in top_docs {
            if let Ok(doc) = searcher.doc(doc_address)
                && let Ok(res) =
                    self.retrieve_result_with_doc(&searcher, "", 0.0, doc_address, &doc, &[])
            {
                results.push(res);
            }
        }

        Ok(results)
    }

    pub fn invalidate_cache(&self) {
        self.cache.invalidate();
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
            min_modified: None,
            extensions: None,
            case_sensitive: false,
        };
        let key2 = CacheKey {
            query: "test".to_string(),
            limit: 10,
            min_size: None,
            max_size: None,
            min_modified: None,
            extensions: None,
            case_sensitive: false,
        };
        assert_eq!(key1, key2);
    }
}
