use crate::commands::AppState;
use crate::indexer::filename_index::{FilenameIndexStats, FilenameSearchResult};
use crate::indexer::searcher::{SearchParams, SearchResult};
use crate::models::PreviewResult;
use crate::parsers::{PreviewElement, parse_file_preview};
use iced::widget::text::Highlighter as _;
use mini_moka::sync::Cache;
use std::sync::{Arc, OnceLock};
use std::time::Duration;

static PREVIEW_CACHE: OnceLock<Cache<(String, u64), Vec<PreviewElement>>> = OnceLock::new();

fn get_preview_cache() -> &'static Cache<(String, u64), Vec<PreviewElement>> {
    PREVIEW_CACHE.get_or_init(|| {
        Cache::builder()
            .max_capacity(100)
            .time_to_live(Duration::from_mins(10))
            .build()
    })
}

/// Performs a search query against the index, then fills in snippets.
///
/// Snippets are generated here rather than inside the searcher because `content`
/// is no longer `STORED` in the Tantivy index: the index knows *which*
/// documents matched but no longer holds their text. Each hit is re-extracted
/// from disk on a blocking thread, bounded to
/// [`crate::snippet::MAX_SNIPPET_FILES_PER_QUERY`] files and cached by
/// `(path, mtime, size, terms)`.
///
/// # Errors
///
/// Returns an error if the search query fails. A failure to extract an
/// individual snippet is *not* an error: the result is still returned without
/// one, matching the previous behaviour for a document with no locatable match.
pub async fn search_query_internal(
    params: SearchParams<'_>,
    state: &Arc<AppState>,
) -> Result<Vec<SearchResult>, String> {
    // Captured before `params` is consumed by `search`.
    let query = params.query.to_string();
    let case_sensitive = params.case_sensitive;

    let mut results = state
        .indexer
        .search(params)
        .await
        .map_err(|e| e.to_string())?;

    let enable_ocr = state.settings_cache.load().enable_ocr;

    // Concurrently, not sequentially: each extraction is I/O + CPU bound and
    // independent, so a page of 50 results should not cost 50 round trips.
    //
    // `JoinSet` is used rather than a `Vec<JoinHandle>` because it drops the
    // collection as soon as every task has been awaited, so no handles are left
    // dangling when an early error returns.
    let mut tasks = tokio::task::JoinSet::new();

    for result in results
        .iter()
        .take(crate::snippet::MAX_SNIPPET_FILES_PER_QUERY)
    {
        let path = result.file_path.clone();
        let query = query.clone();
        tasks.spawn(async move {
            let outcome =
                crate::snippet::generate_snippets(&path, None, &query, case_sensitive, enable_ocr)
                    .await;
            (path, outcome)
        });
    }

    let mut by_path: std::collections::HashMap<String, crate::snippet::SnippetOutcome> =
        std::collections::HashMap::new();
    while let Some(joined) = tasks.join_next().await {
        match joined {
            Ok((path, outcome)) => {
                by_path.insert(path, outcome);
            }
            Err(e) => tracing::warn!("Snippet task failed: {e}"),
        }
    }

    // A case-sensitive query drops hits whose text does not contain the exact
    // spelling. This is the only place casing can be enforced, because the index
    // stores lowercased terms only.
    let enforce_case = case_sensitive
        && crate::indexer::searcher::needs_case_post_filter(
            &crate::indexer::query_parser::extract_highlight_terms(&query, case_sensitive),
        );

    results.retain(|result| {
        by_path
            .get(&result.file_path)
            // Not examined (beyond the snippet cap): keep it, the indexer matched it.
            .is_none_or(|outcome| !enforce_case || outcome.case_match)
    });

    for result in &mut results {
        if let Some(outcome) = by_path.get(&result.file_path) {
            result.snippets = outcome.snippets.clone();
        }
    }

    Ok(results)
}

/// Gets a preview of the file content.
///
/// # Errors
///
/// Returns an error if the file cannot be read or parsed.
pub async fn get_file_preview_internal(
    path: String,
    enable_ocr: bool,
) -> Result<Vec<PreviewElement>, String> {
    let path_buf = std::path::PathBuf::from(&path);
    let modified = std::fs::metadata(&path_buf)
        .and_then(|m| m.modified())
        .map_or(0, |t| {
            t.duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs()
        });

    let cache = get_preview_cache();
    let cache_key = (path.clone(), modified);

    if let Some(cached) = cache.get(&cache_key) {
        return Ok(cached);
    }

    let result = parse_file_preview(&path_buf, enable_ocr).await;

    match result {
        Ok(elements) => {
            cache.insert(cache_key, elements.clone());
            Ok(elements)
        }
        Err(e) => Err(e.to_string()),
    }
}

fn highlight_search_matches(
    spans: Vec<(String, Option<[f32; 4]>)>,
    matched_terms: &[String],
    case_sensitive: bool,
) -> Vec<(String, Option<[f32; 4]>)> {
    if matched_terms.is_empty() {
        return spans;
    }

    let valid_terms: Vec<&str> = matched_terms
        .iter()
        .map(String::as_str)
        .filter(|t| !t.is_empty())
        .collect();

    if valid_terms.is_empty() {
        return spans;
    }

    // Use SIMD-accelerated Aho-Corasick multi-pattern search
    let Ok(matcher) = aho_corasick::AhoCorasick::builder()
        .ascii_case_insensitive(!case_sensitive)
        .match_kind(aho_corasick::MatchKind::LeftmostFirst)
        .build(&valid_terms)
    else {
        return spans;
    };

    let mut result = Vec::new();
    for (text, color) in spans {
        if text.is_empty() {
            continue;
        }

        let mut matches = Vec::new();
        for m in matcher.find_iter(&text) {
            matches.push((m.start(), m.end()));
        }

        if matches.is_empty() {
            result.push((text, color));
            continue;
        }

        matches.sort_by_key(|r| r.0);
        let mut merged: Vec<(usize, usize)> = Vec::new();
        for m in matches {
            if let Some(last) = merged.last_mut()
                && m.0 <= last.1
            {
                last.1 = last.1.max(m.1);
                continue;
            }
            merged.push(m);
        }

        let mut last_idx = 0;
        for (start, end) in merged {
            if start > last_idx {
                result.push((text[last_idx..start].to_string(), color));
            }
            result.push((text[start..end].to_string(), Some([1.0, 0.75, 0.0, 1.0])));
            last_idx = end;
        }
        if last_idx < text.len() {
            result.push((text[last_idx..].to_string(), color));
        }
    }
    result
}

/// Gets a highlighted preview of the file content.
///
/// # Errors
///
/// Returns an error if the preview generation fails.
pub async fn get_file_preview_highlighted_internal(
    path: String,
    query: String,
    state: &Arc<AppState>,
) -> Result<PreviewResult, String> {
    use crate::indexer::query_parser::extract_highlight_terms;
    let settings = state.settings_cache.load();
    let case_sensitive = settings.case_sensitive;
    let enable_ocr = settings.enable_ocr;
    let matched_terms = extract_highlight_terms(&query, case_sensitive);

    let elements = get_file_preview_internal(path.clone(), enable_ocr).await?;

    let elements_clone = elements.clone();
    let matched_terms_clone = matched_terms.clone();

    let highlighted_elements = tokio::task::spawn_blocking(move || {
        let mut final_elements = Vec::new();

        for element in elements_clone {
            let mut spans = Vec::new();
            let content = element.content;

            if element.element_type == crate::models::ElementType::CodeBlock {
                let extension = std::path::Path::new(&path)
                    .extension()
                    .and_then(|s| s.to_str())
                    .unwrap_or("txt");
                let mut highlighter =
                    iced_highlighter::Highlighter::new(&iced_highlighter::Settings {
                        theme: iced_highlighter::Theme::Base16Ocean,
                        token: extension.to_string(),
                    });
                for line in content.lines() {
                    for (range, highlight) in highlighter.highlight_line(line) {
                        let color = highlight.color().map(|c| [c.r, c.g, c.b, c.a]);
                        spans.push((line[range].to_string(), color));
                    }
                    spans.push(("\n".to_string(), None));
                }
            } else {
                spans.push((content, None));
            }

            // Overlay search matches if any
            let processed_spans =
                highlight_search_matches(spans, &matched_terms_clone, case_sensitive);

            final_elements.push(crate::models::DocumentElementHighlight {
                element_type: element.element_type,
                spans: processed_spans,
            });
        }
        final_elements
    })
    .await
    .unwrap_or_default();

    Ok(PreviewResult {
        elements: highlighted_elements,
        matched_terms,
    })
}

/// Searches for filenames in the filename index.
///
/// # Errors
///
/// Returns an error if the filename index is not initialized or the search fails.
pub async fn search_filenames_internal(
    query: String,
    limit: usize,
    state: &Arc<AppState>,
) -> Result<Vec<FilenameSearchResult>, String> {
    // Results are already the index's own type; an identical copy lived in
    // `models` and every hit was converted into it field by field.
    state
        .filename_index
        .as_ref()
        .ok_or_else(|| "Filename index not initialized".to_string())?
        .search(&query, limit)
        .map_err(|e| e.to_string())
}

/// Gets statistics for the filename index.
///
/// # Errors
///
/// Returns an error if the filename index is not initialized or stats cannot be retrieved.
pub async fn get_filename_index_stats_internal(
    state: &Arc<AppState>,
) -> Result<FilenameIndexStats, String> {
    state
        .filename_index
        .as_ref()
        .ok_or_else(|| "Filename index not initialized".to_string())?
        .get_stats()
        .map_err(|e| e.to_string())
}
