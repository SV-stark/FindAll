use crate::commands::AppState;
use crate::indexer::searcher::IndexStatistics;
use crate::models::{IndexStatus, RecentFile};
use std::path::PathBuf;
use std::sync::Arc;
use tracing::error;

pub async fn start_indexing_internal(path: String, state: Arc<AppState>) -> Result<(), String> {
    let path = PathBuf::from(path);
    let mut handle_guard = state.indexing_handle.lock();

    // Abort previous indexing if still running
    if let Some(handle) = handle_guard.take() {
        handle.abort();
    }

    let state_clone = state.clone();
    let handle = tokio::spawn(async move {
        let settings = state_clone.settings_cache.load();
        let mut exclude_patterns = settings.exclude_patterns.clone();
        for folder in &settings.exclude_folders {
            exclude_patterns.push(folder.clone());
        }

        if let Err(e) = state_clone
            .scanner
            .scan_directory(path, exclude_patterns)
            .await
        {
            error!("Indexing error: {}", e);
        }
    });

    *handle_guard = Some(handle);
    Ok(())
}

pub async fn get_index_status_internal(state: &Arc<AppState>) -> Result<IndexStatus, String> {
    let is_running = {
        let mut handle_guard = state.indexing_handle.lock();
        if let Some(handle) = &*handle_guard {
            if handle.is_finished() {
                *handle_guard = None;
                false
            } else {
                true
            }
        } else {
            false
        }
    };

    let status = if is_running {
        "indexing".to_string()
    } else {
        "idle".to_string()
    };

    let index_stats = state.indexer.get_statistics().map_err(|e| e.to_string())?;

    Ok(IndexStatus {
        status,
        files_indexed: index_stats.total_documents,
    })
}

pub async fn get_index_statistics_internal(
    state: &Arc<AppState>,
) -> Result<IndexStatistics, String> {
    state.indexer.get_statistics().map_err(|e| e.to_string())
}

pub async fn get_recent_files_internal(
    limit: usize,
    state: &Arc<AppState>,
) -> Result<Vec<RecentFile>, String> {
    let files = state
        .indexer
        .get_recent_files(limit)
        .map_err(|e| e.to_string())?;
    Ok(files
        .into_iter()
        .map(|r| RecentFile {
            path: r.file_path,
            title: r.title,
            modified: r.modified.unwrap_or(0),
            size: r.size.unwrap_or(0),
        })
        .collect())
}
