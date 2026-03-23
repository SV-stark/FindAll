use crate::commands::AppState;
use crate::indexer::searcher::SearchResult;
use crate::models::{FilenameIndexStats, FilenameSearchResult, PreviewResult};
use crate::parsers::parse_file;
use moka::sync::Cache;
use std::sync::{Arc, OnceLock};
use std::time::Duration;

static PREVIEW_CACHE: OnceLock<Cache<(String, u64), String>> = OnceLock::new();

fn get_preview_cache() -> &'static Cache<(String, u64), String> {
    PREVIEW_CACHE.get_or_init(|| {
        Cache::builder()
            .max_capacity(100)
            .time_to_live(Duration::from_secs(600))
            .build()
    })
}

pub async fn search_query_internal(
    query: String,
    limit: usize,
    state: &Arc<AppState>,
    min_size: Option<u64>,
    max_size: Option<u64>,
    min_modified: Option<u64>,
    file_extensions: Option<Vec<String>>,
    case_sensitive: bool,
) -> Result<Vec<SearchResult>, String> {
    state
        .indexer
        .search(
            &query,
            limit,
            min_size,
            max_size,
            min_modified,
            file_extensions.as_deref(),
            case_sensitive,
        )
        .await
        .map_err(|e| e.to_string())
}

pub async fn get_file_preview_internal(path: String) -> Result<String, String> {
    let path_buf = std::path::PathBuf::from(&path);
    let modified = std::fs::metadata(&path_buf)
        .and_then(|m| m.modified())
        .map(|t| {
            t.duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs()
        })
        .unwrap_or(0);

    let cache = get_preview_cache();
    let cache_key = (path.clone(), modified);

    if let Some(cached) = cache.get(&cache_key) {
        return Ok(cached);
    }

    let result = tokio::task::spawn_blocking(move || parse_file(&path_buf))
        .await
        .map_err(|e| format!("Preview task failed: {}", e))?;

    match result {
        Ok(doc) => {
            let content = doc.content[..std::cmp::min(doc.content.len(), 10000)].to_string();
            cache.insert(cache_key, content.clone());
            Ok(content)
        }
        Err(e) => Err(e.to_string()),
    }
}

pub async fn get_file_preview_highlighted_internal(
    path: String,
    query: String,
    state: &Arc<AppState>,
) -> Result<PreviewResult, String> {
    use crate::indexer::query_parser::extract_highlight_terms;
    let case_sensitive = state.settings_cache.load().case_sensitive;
    let matched_terms = extract_highlight_terms(&query, case_sensitive);

    let content = get_file_preview_internal(path).await?;
    Ok(PreviewResult {
        content,
        matched_terms,
    })
}

pub async fn search_filenames_internal(
    query: String,
    limit: usize,
    state: &Arc<AppState>,
) -> Result<Vec<FilenameSearchResult>, String> {
    if let Some(ref filename_index) = state.filename_index {
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
    } else {
        Err("Filename index not initialized".to_string())
    }
}

pub async fn get_filename_index_stats_internal(
    state: &Arc<AppState>,
) -> Result<FilenameIndexStats, String> {
    if let Some(ref filename_index) = state.filename_index {
        let stats = filename_index.get_stats().map_err(|e| e.to_string())?;
        Ok(FilenameIndexStats {
            total_files: stats.total_files,
            index_size_bytes: stats.index_size_bytes,
        })
    } else {
        Err("Filename index not initialized".to_string())
    }
}
