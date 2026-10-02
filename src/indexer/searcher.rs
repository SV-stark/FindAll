use super::query_parser::{ParsedQuery, extract_highlight_terms};
use crate::error::{FlashError, Result};
use compact_str::CompactString;
use mini_moka::sync::Cache;
use serde::{Deserialize, Serialize};
use std::ops::Bound;
use std::sync::Arc;
use std::time::Duration;
use tantivy::collector::TopDocs;
use tantivy::query::{Occur, RangeQuery};
use tantivy::schema::{Field, IndexRecordOption, Term, Value};
use tantivy::{Index, IndexReader};

/// Per-segment columnar readers, resolved once per segment and reused for every
/// hit in that segment.
///
/// Previously `retrieve_result_with_doc` called `searcher.segment_reader(seg_ord)`
/// twice per hit (once for `size`, once for `modified`), which meant two segment
/// opens and two fast-field column lookups for every single result.
struct SegmentMeta {
    size: Option<tantivy::columnar::Column<u64>>,
    modified: Option<tantivy::columnar::Column<tantivy::DateTime>>,
}

impl SegmentMeta {
    fn for_doc(searcher: &tantivy::Searcher, seg_ord: u32) -> Self {
        let segment = searcher.segment_reader(seg_ord);
        let fast_fields = segment.fast_fields();
        Self {
            size: fast_fields.u64("size").ok(),
            modified: fast_fields.date("modified").ok(),
        }
    }
}

/// Returns true when the query contains a `"quoted phrase"` literal.
///
/// A quoted phrase means the user asked for an exact multi-word match, so the
/// single-term fuzzy fallback must not silently widen it.
/// Largest timestamp Tantivy can represent safely.
///
/// `DateTime::from_timestamp_secs` converts to nanoseconds internally, so feeding
/// it anything above ~9.2e9 seconds overflows and panics. Using `i64::MAX` (or
/// dividing it, as the old code did) as the "no upper bound" sentinel therefore
/// crashed the search thread for any query that only set a *lower* date bound —
/// e.g. the sidebar's "Modified: This week" or an inline `modified:today`.
///
/// 2100-01-01 is comfortably beyond any plausible file mtime.
const MAX_SAFE_TIMESTAMP_SECS: u64 = 4_102_444_800;

/// Clamps a user-facing timestamp into Tantivy's representable range.
#[inline]
fn safe_timestamp_secs(value: u64) -> i64 {
    i64::try_from(value)
        .unwrap_or(i64::MAX)
        .min(i64::try_from(MAX_SAFE_TIMESTAMP_SECS).unwrap_or(i64::MAX))
}

fn has_quoted_phrase(query: &str) -> bool {
    static PHRASE_REGEX: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    PHRASE_REGEX
        .get_or_init(|| regex::Regex::new(r#""[^"]*""#).expect("PHRASE_REGEX is a valid regex"))
        .is_match(query)
}

/// Sums the sizes of every file directly under `dir`.
///
/// Best-effort: unreadable entries are skipped rather than failing the whole
/// statistics call.
fn directory_size_sync(dir: &std::path::Path) -> u64 {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return 0;
    };
    entries
        .flatten()
        .filter_map(|entry| entry.metadata().ok())
        .filter(std::fs::Metadata::is_file)
        .map(|meta| meta.len())
        .sum()
}

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
    #[must_use]
    pub fn file_path(mut self, file_path: String) -> Self {
        self.file_path = Some(file_path);
        self
    }

    #[must_use]
    pub const fn score(mut self, score: f32) -> Self {
        self.score = Some(score);
        self
    }

    #[must_use]
    pub fn title(mut self, title: Option<CompactString>) -> Self {
        self.title = title;
        self
    }

    #[must_use]
    pub fn maybe_title(self, title: Option<CompactString>) -> Self {
        self.title(title)
    }

    #[must_use]
    pub fn extension(mut self, extension: Option<CompactString>) -> Self {
        self.extension = extension;
        self
    }

    #[must_use]
    pub fn maybe_extension(self, extension: Option<CompactString>) -> Self {
        self.extension(extension)
    }

    #[must_use]
    pub const fn modified(mut self, modified: Option<u64>) -> Self {
        self.modified = modified;
        self
    }

    #[must_use]
    pub const fn size(mut self, size: Option<u64>) -> Self {
        self.size = size;
        self
    }

    #[must_use]
    pub fn matched_terms(mut self, matched_terms: Vec<String>) -> Self {
        self.matched_terms = Some(matched_terms);
        self
    }

    #[must_use]
    pub fn snippets(mut self, snippets: Vec<String>) -> Self {
        self.snippets = Some(snippets);
        self
    }

    /// Builds the `SearchResult`.
    ///
    /// # Panics
    ///
    /// Panics if any required field is missing.
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
    pub(crate) max_modified: Option<u64>,
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
    #[must_use]
    pub const fn query(mut self, query: &'a str) -> Self {
        self.query = Some(query);
        self
    }

    #[must_use]
    pub const fn limit(mut self, limit: usize) -> Self {
        self.limit = Some(limit);
        self
    }

    #[must_use]
    pub const fn min_size(mut self, min_size: Option<u64>) -> Self {
        self.min_size = min_size;
        self
    }

    #[must_use]
    pub const fn maybe_min_size(self, min_size: Option<u64>) -> Self {
        self.min_size(min_size)
    }

    #[must_use]
    pub const fn max_size(mut self, max_size: Option<u64>) -> Self {
        self.max_size = max_size;
        self
    }

    #[must_use]
    pub const fn maybe_max_size(self, max_size: Option<u64>) -> Self {
        self.max_size(max_size)
    }

    #[must_use]
    pub const fn min_modified(mut self, min_modified: Option<u64>) -> Self {
        self.min_modified = min_modified;
        self
    }

    #[must_use]
    pub const fn maybe_min_modified(self, min_modified: Option<u64>) -> Self {
        self.min_modified(min_modified)
    }

    #[must_use]
    pub const fn file_extensions(mut self, extensions: &'a [String]) -> Self {
        self.file_extensions = Some(extensions);
        self
    }

    #[must_use]
    pub const fn maybe_file_extensions(self, extensions: Option<&'a [String]>) -> Self {
        if let Some(exts) = extensions {
            self.file_extensions(exts)
        } else {
            self
        }
    }

    #[must_use]
    pub const fn case_sensitive(mut self, case_sensitive: bool) -> Self {
        self.case_sensitive = Some(case_sensitive);
        self
    }

    #[must_use]
    pub const fn maybe_case_sensitive(self, case_sensitive: Option<bool>) -> Self {
        if let Some(cs) = case_sensitive {
            self.case_sensitive(cs)
        } else {
            self
        }
    }

    /// Builds the `SearchParams`.
    ///
    /// # Panics
    ///
    /// Panics if any required field is missing.
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

/// LRU-style query result cache using moka
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

    pub(crate) fn insert(&self, key: &CacheKey, results: Vec<SearchResult>) {
        self.cache.insert(key.clone(), results);
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
    /// Cached on-disk index size in bytes, refreshed after each commit.
    size_cache: Arc<parking_lot::Mutex<u64>>,
}

impl IndexSearcher {
    /// Opens a reader over `index`.
    ///
    /// The reader is warmed on a background thread, so this returns promptly
    /// even for a large index.
    pub fn new(index: &Index, index_path: &std::path::Path) -> Result<Self> {
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

        let reader_for_searcher = reader.clone();
        let searcher = Self {
            reader: reader_for_searcher,
            index_path: index_path.to_path_buf(),
            cache: QueryCache::new(),
            path_field,
            content_field,
            title_field,
            modified_field,
            size_field,
            extension_field,
            size_cache: Arc::new(parking_lot::Mutex::new(0)),
        };

        // Seed the cached size synchronously so the very first UI paint shows a
        // real number instead of `0`, then keep it fresh via `refresh_size_cache`.
        *searcher.size_cache.lock() = directory_size_sync(index_path);

        // Warm the reader off the startup thread. `IndexManager::open` runs on the
        // UI bootstrap path, and a synchronous `AllQuery` scan there delays the
        // window appearing for no benefit.
        let warm_reader = reader;
        std::thread::Builder::new()
            .name("flash-search-index-warm".to_string())
            .spawn(move || {
                let searcher = warm_reader.searcher();
                let _ = searcher.search(
                    &tantivy::query::AllQuery,
                    &tantivy::collector::TopDocs::with_limit(1).order_by_score(),
                );
            })
            .map_err(|e| FlashError::index(format!("Failed to spawn index warm thread: {e}")))?;

        Ok(searcher)
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
    #[allow(clippy::too_many_lines)]
    pub fn search_sync(&self, params: &SearchParams<'_>) -> Result<Vec<SearchResult>> {
        let parsed = ParsedQuery::new(params.query, params.case_sensitive);

        // Explicit filter arguments (from the sidebar/CLI) and inline `ext:`
        // operators are OR-ed together, so either source can select types.
        let mut file_extensions: smallvec::SmallVec<[CompactString; 8]> =
            smallvec::SmallVec::new();
        for ext in params.file_extensions.unwrap_or_default() {
            let lower = ext.to_ascii_lowercase();
            if !lower.is_empty() && !file_extensions.iter().any(|e| *e == lower) {
                file_extensions.push(CompactString::from(lower));
            }
        }
        for ext in &parsed.extensions {
            if !file_extensions.iter().any(|e| e.as_str() == ext) {
                file_extensions.push(CompactString::from(ext.as_str()));
            }
        }
        let file_extensions = if file_extensions.is_empty() {
            None
        } else {
            Some(file_extensions)
        };

        // Inline operators act as an additional floor/ceiling; the explicit
        // params win when both are present so the sidebar always feels exact.
        let min_size = params.min_size.or(parsed.min_size);
        let max_size = params.max_size.or(parsed.max_size);
        let min_modified = params.min_modified.or(parsed.min_modified);
        let max_modified = parsed.max_modified;

        let cache_key = CacheKey {
            query: params.query.to_string(),
            limit: params.limit,
            min_size,
            max_size,
            min_modified,
            max_modified,
            extensions: file_extensions.clone(),
            case_sensitive: params.case_sensitive,
        };

        if let Some(cached) = self.cache.get(&cache_key) {
            return Ok(cached);
        }

        let highlight_terms = extract_highlight_terms(params.query, params.case_sensitive);

        let searcher = self.reader.searcher();

        // Helper to run query with all filters
        #[allow(clippy::type_complexity)]
        let run_query = |text_query: Box<dyn tantivy::query::Query>,
                         limit: usize|
         -> Result<(Vec<(f32, tantivy::DocAddress)>, Box<dyn tantivy::query::Query>)> {
            let mut combine: Vec<(Occur, Box<dyn tantivy::query::Query>)> =
                vec![(Occur::Must, text_query)];

            if min_size.is_some() || max_size.is_some() {
                let lower = Term::from_field_u64(self.size_field, min_size.unwrap_or(0));
                let upper = Term::from_field_u64(self.size_field, max_size.unwrap_or(u64::MAX));
                let range = RangeQuery::new(Bound::Included(lower), Bound::Included(upper));
                combine.push((Occur::Must, Box::new(range)));
            }

            if min_modified.is_some() || max_modified.is_some() {
                let lower = Term::from_field_date(
                    self.modified_field,
                    tantivy::DateTime::from_timestamp_secs(safe_timestamp_secs(
                        min_modified.unwrap_or(0),
                    )),
                );
                let upper = Term::from_field_date(
                    self.modified_field,
                    tantivy::DateTime::from_timestamp_secs(safe_timestamp_secs(
                        max_modified.unwrap_or(MAX_SAFE_TIMESTAMP_SECS),
                    )),
                );
                let range = RangeQuery::new(Bound::Included(lower), Bound::Included(upper));
                combine.push((Occur::Must, Box::new(range)));
            }

            if let Some(ref extensions) = file_extensions
                && !extensions.is_empty()
            {
                combine.push((
                    Occur::Must,
                    Box::new(tantivy::query::BooleanQuery::new(
                        extensions
                            .iter()
                            .map(|ext| {
                                let term = tantivy::Term::from_field_text(self.extension_field, ext);
                                let q: Box<dyn tantivy::query::Query> =
                                    Box::new(tantivy::query::TermQuery::new(
                                        term,
                                        IndexRecordOption::Basic,
                                    ));
                                (Occur::Should, q)
                            })
                            .collect(),
                    )),
                ));
            }

            let final_query = tantivy::query::BooleanQuery::new(combine);
            let top_docs = searcher
                .search(&final_query, &TopDocs::with_limit(limit).order_by_score())
                .map_err(|e| FlashError::search(params.query, e.to_string()))?;

            Ok((top_docs, Box::new(final_query)))
        };

        // Build the parsed text query once and reuse it for the snippet
        // generator, instead of re-parsing per query twice.
        let (text_query, used_query_str): (Box<dyn tantivy::query::Query>, &str) =
            if parsed.text_query == "*" {
                (Box::new(tantivy::query::AllQuery), parsed.text_query.as_str())
            } else {
                let mut query_parser = tantivy::query::QueryParser::for_index(
                    searcher.index(),
                    vec![self.content_field],
                );
                query_parser.set_conjunction_by_default();

                match query_parser.parse_query(&parsed.text_query) {
                    Ok(q) => (q, parsed.text_query.as_str()),
                    Err(_) => (
                        Box::new(tantivy::query::FuzzyTermQuery::new(
                            Term::from_field_text(self.content_field, &parsed.text_query),
                            1,
                            true,
                        )),
                        parsed.text_query.as_str(),
                    ),
                }
            };

        // `path:`/`title:` are substring filters, which Tantivy cannot express as
        // a query, so they are applied per-document. Over-fetch so that filtering
        // a highly selective filter does not silently truncate the result list.
        let needs_post_filter = parsed.needs_post_filter();
        let fetch_limit = if needs_post_filter {
            params.limit.saturating_mul(8).max(200)
        } else {
            params.limit
        };

        let (top_docs, _) = run_query(text_query, fetch_limit)?;

        // Only run the expensive fuzzy fallback if the exact query returned zero
        // hits and the user did not ask for a phrase / multi-word query.
        if top_docs.is_empty()
            && !parsed.text_query.contains(' ')
            && parsed.text_query != "*"
            && parsed.text_query.len() >= 3
            && !has_quoted_phrase(&parsed.text_query)
        {
            let fuzzy_query = tantivy::query::FuzzyTermQuery::new(
                Term::from_field_text(self.content_field, &parsed.text_query),
                1,
                true,
            );
            if let Ok((fuzzy_docs, _)) =
                run_query(Box::new(fuzzy_query), params.limit)
            {
                return self.process_top_docs(
                    &searcher,
                    fuzzy_docs,
                    used_query_str,
                    &highlight_terms,
                    &parsed,
                    &cache_key,
                );
            }
        }

        self.process_top_docs(
            &searcher,
            top_docs,
            used_query_str,
            &highlight_terms,
            &parsed,
            &cache_key,
        )
    }

    fn process_top_docs(
        &self,
        searcher: &tantivy::Searcher,
        top_docs: Vec<(f32, tantivy::DocAddress)>,
        query: &str,
        highlight_terms: &[String],
        parsed: &ParsedQuery,
        cache_key: &CacheKey,
    ) -> Result<Vec<SearchResult>> {
        let mut results = Vec::with_capacity(top_docs.len().min(cache_key.limit));

        let snippet_generator = if query.is_empty() || query == "*" {
            None
        } else {
            let query_parser =
                tantivy::query::QueryParser::for_index(searcher.index(), vec![self.content_field]);
            query_parser
                .parse_query(query)
                .ok()
                .and_then(|q| {
                    tantivy::snippet::SnippetGenerator::create(
                        searcher,
                        &*q,
                        self.content_field,
                    )
                    .ok()
                })
        };

        // Cache segment readers / columnar fast-field readers per segment instead
        // of looking them up twice for every hit.
        let mut segment_cache: std::collections::HashMap<u32, SegmentMeta> =
            std::collections::HashMap::new();

        for (score, doc_address) in top_docs {
            if results.len() >= cache_key.limit {
                break;
            }

            let doc: tantivy::TantivyDocument = searcher
                .doc(doc_address)
                .map_err(|e| FlashError::search(query, e.to_string()))?;

            // Apply the substring filters here (see `needs_post_filter`).
            if parsed.needs_post_filter() {
                let path_str = doc
                    .get_first(self.path_field)
                    .and_then(|v| v.as_str())
                    .unwrap_or_default();
                if !parsed.matches_path(path_str) {
                    continue;
                }
                let title = doc.get_first(self.title_field).and_then(|v| v.as_str());
                if !parsed.matches_title(title) {
                    continue;
                }
            }

            let entry = segment_cache
                .entry(doc_address.segment_ord)
                .or_insert_with(|| SegmentMeta::for_doc(searcher, doc_address.segment_ord));

            let result =
                self.retrieve_result_with_doc(entry, score, doc_address, &doc, highlight_terms, snippet_generator.as_ref());
            results.push(result);
        }

        self.cache.insert(cache_key, results.clone());
        Ok(results)
    }

    fn retrieve_result_with_doc(
        &self,
        segment: &SegmentMeta,
        score: f32,
        doc_address: tantivy::DocAddress,
        doc: &tantivy::TantivyDocument,
        highlight_terms: &[String],
        snippet_generator: Option<&tantivy::snippet::SnippetGenerator>,
    ) -> SearchResult {
        let size = segment
            .size
            .as_ref()
            .map(|col| col.values.get_val(doc_address.doc_id))
            .filter(|&s| s > 0)
            .or_else(|| doc.get_first(self.size_field).and_then(|v| v.as_u64()));

        let modified = segment.modified.as_ref().map(|col| {
            u64::try_from(col.values.get_val(doc_address.doc_id).into_timestamp_secs()).unwrap_or(0)
        });

        let file_path = doc
            .get_first(self.path_field)
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();

        let title = doc
            .get_first(self.title_field)
            .and_then(|v| v.as_str())
            .map(CompactString::from);

        let extension = doc
            .get_first(self.extension_field)
            .and_then(|v| v.as_str())
            .map(CompactString::from);

        let snippets = snippet_generator
            .map(|sg| {
                let snip = sg.snippet_from_doc(doc);
                let html = snip.to_html();
                if html.trim().is_empty() {
                    Vec::new()
                } else {
                    vec![html]
                }
            })
            .unwrap_or_default();

        SearchResult {
            file_path,
            score,
            title,
            extension,
            modified,
            size,
            matched_terms: highlight_terms.to_vec(),
            snippets,
        }
    }

    pub fn get_statistics(&self) -> Result<IndexStatistics> {
        let searcher = self.reader.searcher();
        let total_docs = usize::try_from(searcher.num_docs()).unwrap_or(usize::MAX);

        // Walking the index directory is blocking I/O, and this is called from
        // the UI startup path and after every indexing run. The size only
        // changes on commit, so it is refreshed in the background by
        // `refresh_size_cache()` and read from the cache here.
        let total_size = *self.size_cache.lock();

        Ok(IndexStatistics {
            total_documents: total_docs,
            total_size_bytes: total_size,
        })
    }

    /// Re-measures the on-disk index size on a background thread.
    ///
    /// Called after every commit. A failed measurement leaves the previous value
    /// in place, which beats reporting a misleading `0`.
    pub fn refresh_size_cache(&self) {
        let index_path = self.index_path.clone();
        let cache_slot = Arc::clone(&self.size_cache);

        let measure = move || {
            let size = directory_size_sync(&index_path);
            if size > 0 {
                *cache_slot.lock() = size;
            }
        };

        match tokio::runtime::Handle::try_current() {
            Ok(handle) => {
                handle.spawn_blocking(measure);
            }
            Err(_) => {
                std::thread::Builder::new()
                    .name("flash-search-index-size".to_string())
                    .spawn(measure)
                    .ok();
            }
        }
    }

    pub fn get_recent_files(&self, limit: usize) -> Result<Vec<SearchResult>> {
        let searcher = self.reader.searcher();
        let query = tantivy::query::AllQuery;

        let top_docs = searcher
            .search(
                &query,
                &TopDocs::with_limit(limit)
                    .order_by_fast_field::<tantivy::DateTime>("modified", tantivy::Order::Desc),
            )
            .map_err(|e| FlashError::index(format!("Failed to get recent files: {e}")))?;

        let mut results = Vec::new();
        let mut segments: std::collections::HashMap<u32, SegmentMeta> =
            std::collections::HashMap::new();

        for (mod_time, doc_address) in top_docs {
            if let Ok(doc) = searcher.doc(doc_address) {
                let entry = segments
                    .entry(doc_address.segment_ord)
                    .or_insert_with(|| SegmentMeta::for_doc(&searcher, doc_address.segment_ord));
                let mut res = self.retrieve_result_with_doc(entry, 0.0, doc_address, &doc, &[], None);
                res.modified = mod_time.map(|t| {
                    u64::try_from(t.into_timestamp_secs()).unwrap_or(0)
                });
                results.push(res);
            }
        }

        Ok(results)
    }

    /// Reload the index reader immediately and invalidate query cache
    pub fn reload(&self) -> Result<()> {
        self.reader
            .reload()
            .map_err(|e| FlashError::index(format!("Failed to reload index reader: {e}")))?;
        self.invalidate_cache();
        Ok(())
    }

    pub fn invalidate_cache(&self) {
        self.cache.invalidate();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::writer::IndexWriterManager;
    use crate::parsers::ParsedDocument;
    use tempfile::tempdir;

    fn doc(path: &str, content: &str) -> ParsedDocument {
        ParsedDocument {
            path: path.to_string(),
            content: content.to_string(),
            title: Some(CompactString::from(
                std::path::Path::new(path)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or(path),
            )),
            language: None,
            keywords: None,
            layout: None,
            code_metadata: None,
            embeddings: None,
        }
    }

    /// Builds an index over a handful of representative documents.
    fn fixture() -> (tempfile::TempDir, tantivy::Index) {
        let dir = tempdir().unwrap();
        let index = Index::open_or_create(
            tantivy::directory::MmapDirectory::open(dir.path()).unwrap(),
            crate::indexer::schema::create_schema(),
        )
        .unwrap();

        let writer: IndexWriterManager = IndexWriterManager::new(&index, 64).unwrap();

        let now = 1_700_000_000u64;
        writer
            .add_document(
                &doc("C:/docs/annual-report.pdf", "quarterly revenue summary"),
                now,
                4_096,
            )
            .unwrap();
        writer
            .add_document(
                &doc("C:/notes/todo.md", "quarterly revenue checklist"),
                now - 86_400,
                512,
            )
            .unwrap();
        writer
            .add_document(
                &doc("D:/archive/old-notes.txt", "unrelated historical content"),
                now - 400 * 86_400,
                128,
            )
            .unwrap();
        writer.commit().unwrap();

        (dir, index)
    }

    fn search(
        index: &tantivy::Index,
        tweak: impl FnOnce(SearchParamsBuilder<'_>) -> SearchParamsBuilder<'_>,
    ) -> Vec<SearchResult> {
        let params = tweak(
            SearchParamsBuilder::default()
                .limit(50)
                .case_sensitive(false),
        )
        .build();
        let searcher = IndexSearcher::new(index, std::path::Path::new("unused")).unwrap();
        searcher.search_sync(&params).unwrap()
    }

    #[test]
    fn test_extension_is_readable_from_the_index() {
        // Regression: `extension` was indexed as `STRING` but never `STORED`, so
        // `retrieve_result_with_doc` always returned `None` and every result
        // rendered with the generic "FILE" badge.
        let (_dir, index) = fixture();
        let results = search(&index, |b| b.query("quarterly"));
        assert!(!results.is_empty());
        for result in &results {
            assert!(
                result.extension.is_some(),
                "extension must round-trip through the index, got None for {}",
                result.file_path
            );
        }
        assert!(results.iter().any(|r| r.extension.as_deref() == Some("pdf")));
        assert!(results.iter().any(|r| r.extension.as_deref() == Some("md")));
    }

    #[test]
    fn test_extension_filter_is_case_insensitive() {
        // The writer lowercases the extension before indexing, so a query must
        // match regardless of the case the user typed.
        let (_dir, index) = fixture();
        let upper = search(&index, |b| b.query("quarterly ext:PDF"));
        let mixed = search(&index, |b| b.query("quarterly ext:Pdf"));
        assert_eq!(upper.len(), 1);
        assert_eq!(mixed.len(), 1);
    }

    #[test]
    fn test_unknown_extension_filter_returns_nothing() {
        let (_dir, index) = fixture();
        assert!(search(&index, |b| b.query("quarterly ext:xyz")).is_empty());
    }

    #[test]
    fn test_ext_operator_filters_results() {
        let (_dir, index) = fixture();
        let results = search(&index, |b| b.query("quarterly ext:pdf"));
        assert_eq!(results.len(), 1);
        assert!(
            results[0]
                .file_path
                .to_ascii_lowercase()
                .ends_with(".pdf")
        );
    }

    #[test]
    fn test_repeated_ext_operator_is_an_or_set() {
        let (_dir, index) = fixture();
        let results = search(&index, |b| b.query("quarterly ext:pdf ext:md"));
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_path_operator_filters_results() {
        let (_dir, index) = fixture();
        let results = search(&index, |b| b.query("quarterly path:docs"));
        assert_eq!(results.len(), 1);
        assert!(results[0].file_path.contains("docs"));
    }

    #[test]
    fn test_title_operator_filters_results() {
        let (_dir, index) = fixture();
        let results = search(&index, |b| b.query("quarterly title:annual"));
        assert_eq!(results.len(), 1);
        assert!(
            results[0]
                .file_path
                .to_ascii_lowercase()
                .ends_with("annual-report.pdf")
        );

        let none = search(&index, |b| b.query("quarterly title:does-not-exist"));
        assert!(none.is_empty());
    }

    #[test]
    fn test_size_filter_narrows_results() {
        let (_dir, index) = fixture();
        let results = search(&index, |b| b.query("quarterly size:>1000"));
        assert_eq!(results.len(), 1, "only the 4096-byte PDF passes");
        assert!(
            results[0]
                .file_path
                .to_ascii_lowercase()
                .ends_with(".pdf")
        );
    }

    #[test]
    fn test_date_filter_narrows_results() {
        let (_dir, index) = fixture();
        let recent = search(&index, |b| b.query("quarterly").min_modified(Some(
            1_700_000_000 - 2 * 86_400,
        )));
        assert_eq!(recent.len(), 2);
    }

    #[test]
    fn test_match_all_returns_every_document_with_snippets_omitted() {
        let (_dir, index) = fixture();
        let results = search(&index, |b| b.query("*"));
        assert_eq!(results.len(), 3);
        assert!(results.iter().all(|r| r.snippets.is_empty()));
    }

    #[test]
    fn test_snippets_are_produced_for_term_queries() {
        let (_dir, index) = fixture();
        let results = search(&index, |b| b.query("quarterly"));
        assert!(
            results.iter().any(|r| !r.snippets.is_empty()),
            "term queries must produce snippets for the results panel"
        );
    }

    #[test]
    fn test_cache_key_distinguishes_every_filter() {
        let base = CacheKey {
            query: "test".to_string(),
            limit: 10,
            min_size: None,
            max_size: None,
            min_modified: None,
            max_modified: None,
            extensions: None,
            case_sensitive: false,
        };
        assert_eq!(base, base.clone());

        let mut other = base.clone();
        other.max_modified = Some(5);
        assert_ne!(base, other, "max_modified must be part of the key");

        let mut other = base.clone();
        other.min_size = Some(1);
        assert_ne!(base, other);

        let mut other = base.clone();
        other.case_sensitive = true;
        assert_ne!(base, other);
    }

    #[test]
    fn test_cache_returns_the_same_results_as_a_cold_search() {
        let (_dir, index) = fixture();
        let searcher = IndexSearcher::new(&index, std::path::Path::new("unused")).unwrap();

        let params = SearchParams::builder()
            .query("quarterly")
            .limit(10)
            .case_sensitive(false)
            .build();
        let cold = searcher.search_sync(&params).unwrap();

        // Second identical query must hit the cache and match exactly.
        let cached = searcher.search_sync(&params).unwrap();
        assert_eq!(cold.len(), cached.len());
        assert_eq!(
            cold.iter().map(|r| &r.file_path).collect::<Vec<_>>(),
            cached.iter().map(|r| &r.file_path).collect::<Vec<_>>()
        );
    }

    #[test]
    fn test_statistics_report_document_count_and_size() {
        let dir = tempdir().unwrap();
        let index = tantivy::Index::open_or_create(
            tantivy::directory::MmapDirectory::open(dir.path()).unwrap(),
            crate::indexer::schema::create_schema(),
        )
        .unwrap();
        let writer = IndexWriterManager::new(&index, 64).unwrap();
        writer
            .add_document(&doc("C:/a.txt", "content"), 1, 1)
            .unwrap();
        writer.commit().unwrap();

        let searcher = IndexSearcher::new(&index, dir.path()).unwrap();
        let stats = searcher.get_statistics().unwrap();
        assert_eq!(stats.total_documents, 1);
        assert!(stats.total_size_bytes > 0);
    }

    #[test]
    fn test_directory_size_helper_handles_missing_directory() {
        assert_eq!(directory_size_sync(std::path::Path::new("definitely-not-here")), 0);
    }

    #[test]
    fn test_phrase_query_is_not_fuzzily_widened() {
        assert!(has_quoted_phrase("\"exact phrase\""));
        assert!(!has_quoted_phrase("plain terms"));
    }

    #[test]
    fn test_timestamp_clamp_prevents_tantivy_overflow() {
        // Regression: the "no upper bound" sentinel used to be `i64::MAX / 1000`,
        // which overflowed inside `DateTime::from_timestamp_secs` and panicked the
        // search thread for any query with only a lower date bound.
        assert_eq!(safe_timestamp_secs(0), 0);
        assert_eq!(safe_timestamp_secs(1_700_000_000), 1_700_000_000);
        assert_eq!(safe_timestamp_secs(u64::MAX), 4_102_444_800);
        assert_eq!(safe_timestamp_secs(MAX_SAFE_TIMESTAMP_SECS + 1), 4_102_444_800);

        // And it must actually be constructible.
        let _ = tantivy::DateTime::from_timestamp_secs(safe_timestamp_secs(u64::MAX));
    }

    #[test]
    fn test_open_ended_date_filter_does_not_panic() {
        // The sidebar's "Modified" filters always set only a lower bound.
        let (_dir, index) = fixture();
        let results = search(&index, |b| {
            b.query("quarterly")
                .min_modified(Some(1_699_827_200))
        });
        assert_eq!(results.len(), 2);
    }
}

