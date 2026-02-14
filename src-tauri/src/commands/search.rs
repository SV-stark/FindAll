use tauri::State;
use std::sync::Arc;
use crate::indexer::searcher::SearchResult;
use crate::parsers::parse_file;
use crate::models::{PreviewResult, FilenameSearchResult, FilenameIndexStats};
use crate::commands::AppState;

/// Search command - queries the index and returns results
#[tauri::command]
pub async fn search_query(
    query: String,
    limit: usize,
    state: State<'_, Arc<AppState>>,
    min_size: Option<u64>,
    max_size: Option<u64>,
    file_extensions: Option<Vec<String>>,
) -> Result<Vec<SearchResult>, String> {
    state.indexer.search(&query, limit, min_size, max_size, file_extensions.as_deref())
        .await
        .map_err(|e| e.to_string())
}

/// Get file content for preview
#[tauri::command]
pub async fn get_file_preview(
    path: String,
) -> Result<String, String> {
    let path = std::path::PathBuf::from(path);

    match parse_file(&path) {
        Ok(doc) => Ok(doc.content[..std::cmp::min(doc.content.len(), 10000)].to_string()),
        Err(e) => Err(e.to_string()),
    }
}

/// Get file preview with search term highlighting
#[tauri::command]
pub async fn get_file_preview_highlighted(
    path: String,
    query: String,
) -> Result<PreviewResult, String> {
    use crate::indexer::query_parser::extract_highlight_terms;
    
    let path = std::path::PathBuf::from(path);
    let matched_terms = extract_highlight_terms(&query);

    match parse_file(&path) {
        Ok(doc) => Ok(PreviewResult {
            content: doc.content[..std::cmp::min(doc.content.len(), 10000)].to_string(),
            matched_terms,
        }),
        Err(e) => Err(e.to_string()),
    }
}

/// Search filenames only (fast mode)
#[tauri::command]
pub async fn search_filenames(
    query: String,
    limit: usize,
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<FilenameSearchResult>, String> {
    if let Some(ref filename_index) = state.filename_index {
        filename_index.search(&query, limit)
            .map(|results| results.into_iter().map(|r| FilenameSearchResult {
                file_path: r.file_path,
                file_name: r.file_name,
            }).collect())
            .map_err(|e| e.to_string())
    } else {
        Err("Filename index not initialized".to_string())
    }
}

/// Get filename index statistics
#[tauri::command]
pub async fn get_filename_index_stats(
    state: State<'_, Arc<AppState>>,
) -> Result<FilenameIndexStats, String> {
    if let Some(ref filename_index) = state.filename_index {
        filename_index.get_stats()
            .map_err(|e| e.to_string())
    } else {
        Err("Filename index not initialized".to_string())
    }
}

/// Filter results by filename pattern
#[tauri::command]
pub async fn filter_by_filename(
    results: Vec<SearchResult>,
    filename_pattern: String,
) -> Result<Vec<SearchResult>, String> {
    use regex::Regex;
    
    let regex = Regex::new(&filename_pattern)
        .map_err(|e| format!("Invalid regex: {}", e))?;
    
    let filtered: Vec<SearchResult> = results
        .into_iter()
        .filter(|r| {
            let filename = r.file_path.split(['\\', '/']).last().unwrap_or("");
            regex.is_match(filename)
        })
        .collect();
    
    Ok(filtered)
}
