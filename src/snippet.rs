//! On-demand snippet generation.
//!
//! The `content` field is no longer `STORED` in the Tantivy index (see
//! [`crate::indexer::schema`]), because duplicating every extracted document
//! body into the doc store made the index grow with the corpus and forced a
//! full decompress of each hit just to render one highlighted line.
//!
//! Snippets are therefore rebuilt from the file on disk. Tantivy is still used
//! to do the hard part: its [`SnippetGenerator`] tokenizes the freshly extracted
//! text with the *same* analyzer used at index time, locates the query terms,
//! scores candidate fragments, and picks the best window. Only the text source
//! changed — the output stays byte-for-byte what the old `snippet_from_doc`
//! path produced, so the `<b>`/`</b>` parsing in `iced_ui::search` is untouched.
//!
//! Cost is one file read + extraction per hit, so it is bounded by
//! [`MAX_SNIPPET_FILES_PER_QUERY`] and memoised by `(path, mtime, size, terms)`.

use crate::error::Result;
use crate::indexer::query_parser::extract_highlight_terms;
use crate::parsers::ParsedDocument;
use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;

use mini_moka::sync::Cache;

/// Hard cap on files re-extracted for a single query.
///
/// The default `max_results` is 50, so this only bites for deliberately large
/// limits. Without it, a 10,000-result query would mean 10,000 extractions.
pub const MAX_SNIPPET_FILES_PER_QUERY: usize = 50;

/// Snippet window length, matching Tantivy's default so output is unchanged.
const SNIPPET_MAX_NUM_CHARS: usize = 150;

/// Memoised re-extractions, keyed by file fingerprint and query.
///
/// Snippets are a pure function of (file contents, query terms), so a cache hit
/// is always correct. `modified`/`size` are part of the key so an edited file
/// cannot serve a stale snippet.
static SNIPPET_CACHE: std::sync::OnceLock<Cache<SnippetCacheKey, Arc<SnippetOutcome>>> =
    std::sync::OnceLock::new();

fn snippet_cache() -> &'static Cache<SnippetCacheKey, Arc<SnippetOutcome>> {
    SNIPPET_CACHE.get_or_init(|| {
        Cache::builder()
            .max_capacity(512)
            .time_to_live(std::time::Duration::from_mins(10))
            .build()
    })
}

#[derive(Clone, Debug, Hash, Eq, PartialEq)]
struct SnippetCacheKey {
    path: String,
    modified: u64,
    size: u64,
    terms: Vec<String>,
    case_sensitive: bool,
}

/// File fingerprint used for cache invalidation.
#[derive(Clone, Copy, Debug)]
pub struct FileStamp {
    pub modified: u64,
    pub size: u64,
}

impl FileStamp {
    /// Reads the current on-disk stamp, or `None` if the file is unreadable.
    #[must_use]
    pub fn read(path: &Path) -> Option<Self> {
        let meta = std::fs::metadata(path).ok()?;
        let modified = meta
            .modified()
            .ok()?
            .duration_since(std::time::SystemTime::UNIX_EPOCH)
            .ok()?
            .as_secs();
        Some(Self {
            modified,
            size: meta.len(),
        })
    }
}

/// A snippet plus whether the document should be kept under a case-sensitive
/// query.
#[derive(Debug, Clone, Default)]
pub struct SnippetOutcome {
    pub snippets: Vec<String>,
    /// `false` when a case-sensitive query asked for a spelling the document does
    /// not actually contain, meaning the hit must be dropped.
    pub case_match: bool,
}

impl SnippetOutcome {
    /// Outcome for a document that could not be examined at all.
    ///
    /// Treated as a pass: a file that has been deleted or cannot be parsed should
    /// not silently disappear from the result list, and the indexer's staleness
    /// check will remove it on the next scan.
    const fn unverifiable() -> Self {
        Self {
            snippets: Vec::new(),
            case_match: true,
        }
    }
}

/// Renders `<b>highlighted</b>` snippets for one search hit.
///
/// Returns an empty vector when the file has vanished, cannot be extracted, or
/// contains no locatable match. Callers treat that as "no snippet", which is
/// the same thing the previous implementation did for a document with no match.
pub async fn generate_snippets(
    path: &str,
    stamp: Option<FileStamp>,
    query: &str,
    case_sensitive: bool,
    enable_ocr: bool,
) -> SnippetOutcome {
    let terms = extract_highlight_terms(query, case_sensitive);
    if terms.is_empty() {
        return SnippetOutcome {
            snippets: Vec::new(),
            case_match: true,
        };
    }

    // Only terms the tokenizer would have lowercased differently can change the
    // outcome. If every term is already lowercase, the post-filter is a no-op and
    // the extraction result is used as-is.
    let enforce_case = case_sensitive && crate::indexer::searcher::needs_case_post_filter(&terms);

    let Some(stamp) = stamp.or_else(|| FileStamp::read(Path::new(path))) else {
        return SnippetOutcome::unverifiable();
    };

    let key = SnippetCacheKey {
        path: path.to_string(),
        modified: stamp.modified,
        size: stamp.size,
        terms: terms.clone(),
        case_sensitive,
    };
    if let Some(hit) = snippet_cache().get(&key) {
        return hit.as_ref().clone();
    }

    let path_owned = path.to_string();
    let terms_for_task = terms.clone();
    let rendered = tokio::task::spawn_blocking(move || {
        extract_text(Path::new(&path_owned), enable_ocr).map(|content| {
            let case_match = !enforce_case
                || crate::indexer::searcher::case_sensitive_post_filter(&content, &terms_for_task);
            let snippets = if case_match {
                render_snippet(&content, &terms, case_sensitive)
                    .map(|s| vec![s])
                    .unwrap_or_default()
            } else {
                // Wrong casing: the document is not a match, so it must not be
                // shown with a snippet either.
                Vec::new()
            };
            SnippetOutcome {
                snippets,
                case_match,
            }
        })
    })
    .await;

    let outcome = match rendered {
        Ok(Ok(outcome)) => outcome,
        Ok(Err(e)) => {
            tracing::debug!("Snippet extraction failed for {path}: {e}");
            SnippetOutcome::unverifiable()
        }
        Err(e) => {
            tracing::warn!("Snippet task panicked for {path}: {e}");
            SnippetOutcome::unverifiable()
        }
    };

    snippet_cache().insert(key, Arc::new(outcome.clone()));
    outcome
}

/// Extracts plain text from `path` using the same routing the indexer uses, so
/// a snippet reflects what was actually indexed.
fn extract_text(path: &Path, enable_ocr: bool) -> Result<String> {
    // The plaintext fast path avoids building a Tokio runtime for the common
    // case (source files, notes, configs), which is most of a typical corpus.
    if crate::parsers::is_plaintext_fast_path(path) {
        return std::fs::read_to_string(path)
            .map_err(|e| crate::error::FlashError::parse(path, format!("Read failed: {e}")));
    }

    let config = crate::parsers::extraction_config_for(enable_ocr);
    let doc = extract_document(path, &config)?;
    Ok(combine_content_and_keywords(&doc))
}

/// Mirrors [`crate::indexer::writer`]'s content assembly: content and keywords
/// are indexed as one stream, so the snippet must search the same text.
fn combine_content_and_keywords(doc: &ParsedDocument) -> String {
    doc.keywords.as_ref().map_or_else(
        || doc.content.clone(),
        |keywords| {
            let mut combined = String::with_capacity(doc.content.len() + keywords.len() + 1);
            combined.push_str(&doc.content);
            combined.push(' ');
            combined.push_str(keywords);
            combined
        },
    )
}

/// Calls the xberg extractor.
///
/// `xberg::extract` is async but the caller is a blocking thread that has no
/// runtime, so a current-thread runtime is built here rather than paying for a
/// multi-threaded one per snippet.
fn extract_document(path: &Path, config: &xberg::ExtractionConfig) -> Result<ParsedDocument> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| crate::error::FlashError::parse(path, format!("runtime: {e}")))?;

    runtime.block_on(async {
        let input = xberg::ExtractInput::from_uri(path.to_string_lossy().into_owned());
        let result = xberg::extract(input, config).await.map_err(|e| {
            crate::error::FlashError::parse(path, format!("Extraction failed: {e}"))
        })?;
        result
            .results
            .into_iter()
            .next()
            .map(|doc| crate::parsers::map_extracted_document(path, doc))
            .ok_or_else(|| {
                crate::error::FlashError::parse(
                    path,
                    "Extraction returned empty results list".to_string(),
                )
            })
    })
}

/// Renders one highlighted snippet using Tantivy's own fragment scorer.
///
/// This deliberately uses `SnippetGenerator::snippet(&str)` — the text-based
/// entry point — instead of `snippet_from_doc(&doc)`. Both share the same
/// tokenizer and fragment-selection logic, so the output is equivalent, but
/// `snippet` does not require the text to be present in the index's doc store.
fn render_snippet(content: &str, terms: &[String], case_sensitive: bool) -> Option<String> {
    use tantivy::Score;
    use tantivy::snippet::SnippetGenerator;
    use tantivy::tokenizer::{RemoveLongFilter, SimpleTokenizer, TextAnalyzer, TokenStream};

    // Must match `indexer::schema`'s `content` tokenizer ("default"), which Tantivy
    // defines as SimpleTokenizer + RemoveLong(40) + LowerCaser. Built explicitly
    // so the analyzer used for snippet terms cannot drift from the one used to
    // build the index.
    let mut analyzer = TextAnalyzer::builder(SimpleTokenizer::default())
        .filter(RemoveLongFilter::limit(40))
        .filter(tantivy::tokenizer::LowerCaser)
        .build();

    // Build the term -> score map the generator expects, tokenizing each term
    // exactly as the indexer tokenized the document.
    let mut terms_text: BTreeMap<String, Score> = BTreeMap::new();
    for term in terms {
        if term.is_empty() {
            continue;
        }
        // `token_stream` returns an owned `BoxTokenStream`, whose current token is
        // exposed through a method rather than a public field.
        let mut tokens = analyzer.token_stream(term);
        while tokens.advance() {
            let text = tokens.token().text.clone();
            // Under a case-sensitive search the caller already filtered the
            // query, but the document is tokenized lowercase, so the match is
            // made on the lowercased form either way.
            let text = if case_sensitive {
                text.to_lowercase()
            } else {
                text
            };
            if text.is_empty() {
                continue;
            }
            let entry = terms_text.entry(text).or_insert(0.0);
            // Longer literal terms are more specific and should win fragment
            // selection, mirroring how `create` weights parsed query terms.
            *entry = entry.max(term.len() as Score);
        }
    }

    if terms_text.is_empty() {
        return None;
    }

    // `SnippetGenerator::new` takes a `Field` only so `snippet_from_doc` knows
    // which stored field to read. We call `snippet(&str)` instead, which never
    // touches it, so any field will do. `Field`'s inner value is private, so it
    // is borrowed from the real schema rather than constructed.
    let placeholder_field = crate::indexer::schema::create_schema()
        .get_field("content")
        .expect("the index schema always has a `content` field");

    let mut generator = SnippetGenerator::new(
        terms_text,
        analyzer,
        placeholder_field,
        SNIPPET_MAX_NUM_CHARS,
    );
    generator.set_max_num_chars(SNIPPET_MAX_NUM_CHARS);

    let snippet = generator.snippet(content);
    let html = snippet.to_html();
    if html.trim().is_empty() {
        None
    } else {
        Some(html)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn terms(list: &[&str]) -> Vec<String> {
        list.iter().map(ToString::to_string).collect()
    }

    #[test]
    fn no_terms_yields_no_snippet() {
        assert!(render_snippet("hello world", &[], false).is_none());
    }

    #[test]
    fn highlights_a_match() {
        let out = render_snippet(
            "the quick brown fox jumps over the lazy dog",
            &terms(&["brown"]),
            false,
        )
        .expect("expected a snippet");
        assert!(out.contains("<b>brown</b>"), "got: {out}");
    }

    #[test]
    fn case_insensitive_by_default() {
        let out =
            render_snippet("Hello World", &terms(&["hello"]), false).expect("expected a snippet");
        assert!(out.contains("<b>Hello</b>"), "got: {out}");
    }

    #[test]
    fn term_absent_yields_none() {
        assert!(render_snippet("Hello World", &terms(&["zebra"]), false).is_none());
    }

    #[test]
    fn multibyte_text_produces_valid_utf8_html() {
        let out = render_snippet(
            "héllo wörld héllo wörld héllo wörld héllo wörld",
            &terms(&["wörld"]),
            false,
        )
        .expect("expected a snippet");
        assert!(std::str::from_utf8(out.as_bytes()).is_ok());
        assert!(out.contains("<b>"));
    }

    #[test]
    fn match_near_end_of_content_is_clamped() {
        let out = render_snippet("start padding tail end", &terms(&["end"]), false)
            .expect("expected a snippet");
        assert!(out.ends_with("</b>"), "unterminated tag: {out}");
    }

    #[test]
    fn keywords_are_searched_alongside_content() {
        // The indexer concatenates content + keywords into one stream, so a
        // keyword-only match must still produce a snippet.
        let doc = ParsedDocument {
            path: "x".to_string(),
            content: "body text".to_string(),
            title: None,
            language: None,
            keywords: Some("needle".to_string()),
            layout: None,
            code_metadata: None,
            embeddings: None,
        };
        let combined = combine_content_and_keywords(&doc);
        let out = render_snippet(&combined, &terms(&["needle"]), false)
            .expect("expected a snippet from keywords");
        assert!(out.contains("<b>needle</b>"), "got: {out}");
    }
}
