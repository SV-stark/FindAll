use crate::commands::AppState;
use crate::indexer::searcher::{SearchParams, SearchResult};
use crate::models::{FilenameIndexStats, FilenameSearchResult, PreviewResult};
use crate::parsers::parse_file;
use iced::widget::text::Highlighter as _;
use moka::sync::Cache;
use std::sync::{Arc, OnceLock};
use std::time::Duration;

static PREVIEW_CACHE: OnceLock<Cache<(String, u64), String>> = OnceLock::new();

fn get_preview_cache() -> &'static Cache<(String, u64), String> {
    PREVIEW_CACHE.get_or_init(|| {
        Cache::builder()
            .max_capacity(100)
            .time_to_live(Duration::from_mins(10))
            .build()
    })
}

/// Performs a search query against the index.
///
/// # Errors
///
/// Returns an error if the search query fails.
pub async fn search_query_internal(
    params: SearchParams<'_>,
    state: &Arc<AppState>,
) -> Result<Vec<SearchResult>, String> {
    state
        .indexer
        .search(params)
        .await
        .map_err(|e| e.to_string())
}

/// Gets a preview of the file content.
///
/// # Errors
///
/// Returns an error if the file cannot be read or parsed.
pub async fn get_file_preview_internal(path: String) -> Result<String, String> {
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

    let result = tokio::task::spawn_blocking(move || parse_file(&path_buf, false))
        .await
        .map_err(|e| format!("Preview task failed: {e}"))?;

    match result {
        Ok(doc) => {
            let content = doc.content[..std::cmp::min(doc.content.len(), 10000)].to_string();
            cache.insert(cache_key, content.clone());
            Ok(content)
        }
        Err(e) => Err(e.to_string()),
    }
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
    let case_sensitive = state.settings_cache.load().case_sensitive;
    let matched_terms = extract_highlight_terms(&query, case_sensitive);

    let content = get_file_preview_internal(path.clone()).await?;

    let content_clone = content.clone();
    let matched_terms_clone = matched_terms.clone();

    let cached_spans = tokio::task::spawn_blocking(move || {
        let mut spans = Vec::new();
        if matched_terms_clone.is_empty() {
            let extension = std::path::Path::new(&path)
                .extension()
                .and_then(|s| s.to_str())
                .unwrap_or("txt");
            let mut highlighter = iced_highlighter::Highlighter::new(&iced_highlighter::Settings {
                theme: iced_highlighter::Theme::Base16Ocean,
                token: extension.to_string(),
            });
            for line in content_clone.lines() {
                for (range, highlight) in highlighter.highlight_line(line) {
                    let color = highlight.color().map(|c| [c.r, c.g, c.b, c.a]);
                    spans.push((line[range].to_string(), color));
                }
                spans.push(("\n".to_string(), None));
            }
        } else {
            let lower_content = if case_sensitive {
                content_clone.clone()
            } else {
                content_clone.to_lowercase()
            };
            let mut matches = Vec::new();

            for term in &matched_terms_clone {
                if term.is_empty() {
                    continue;
                }
                let term_to_match = if case_sensitive {
                    term.clone()
                } else {
                    term.to_lowercase()
                };
                let mut start = 0;
                while let Some(idx) = lower_content[start..].find(&term_to_match) {
                    let abs_idx = start + idx;
                    matches.push((abs_idx, abs_idx + term.len()));
                    start = abs_idx + term.len();
                }
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
                    spans.push((content_clone[last_idx..start].to_string(), None));
                }
                // AMBER color mapping
                spans.push((
                    content_clone[start..end].to_string(),
                    Some([1.0, 0.75, 0.0, 1.0]),
                ));
                last_idx = end;
            }
            if last_idx < content_clone.len() {
                spans.push((content_clone[last_idx..].to_string(), None));
            }
        }
        spans
    })
    .await
    .unwrap_or_default();

    Ok(PreviewResult {
        content,
        matched_terms,
        cached_spans,
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
    state.filename_index.as_ref().map_or_else(
        || Err("Filename index not initialized".to_string()),
        |filename_index| {
            filename_index
                .search(&query, limit)
                .map(|results| {
                    results
                        .into_iter()
                        .map(|r| FilenameSearchResult {
                            file_path: r.file_path,
                            file_name: r.file_name,
                        })
                        .collect()
                })
                .map_err(|e| e.to_string())
        },
    )
}

/// Gets statistics for the filename index.
///
/// # Errors
///
/// Returns an error if the filename index is not initialized or stats cannot be retrieved.
pub async fn get_filename_index_stats_internal(
    state: &Arc<AppState>,
) -> Result<FilenameIndexStats, String> {
    if let Some(ref filename_index) = state.filename_index {
        let index_stats = filename_index.get_stats().map_err(|e| e.to_string())?;
        Ok(FilenameIndexStats {
            total_files: index_stats.total_files,
            index_size_bytes: index_stats.index_size_bytes,
        })
    } else {
        Err("Filename index not initialized".to_string())
    }
}
