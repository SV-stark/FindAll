use crate::commands::AppState;
use crate::indexer::searcher::IndexStatistics;
use crate::models::{IndexStatus, RecentFile};
use std::path::PathBuf;
use std::sync::Arc;

/// Starts (or restarts) indexing for `path`.
///
/// Supersedes any run already in flight and merges the user's exclude patterns
/// and folders. The UI used to call `Scanner::scan_directory` directly with an
/// empty exclude list, which silently ignored the user's settings and left the
/// run uncancellable and untracked.
///
/// # Errors
///
/// Returns an error if the indexing task cannot be started.
pub async fn start_indexing_internal(path: String, state: Arc<AppState>) -> Result<(), String> {
    state
        .start_indexing(PathBuf::from(path))
        .await
        .map_err(|e| e.to_string())
}

/// Cancels the current indexing run, if any.
pub fn cancel_indexing_internal(state: &Arc<AppState>) {
    state.cancel_indexing();
}

/// Gets the current status of the indexer.
///
/// # Errors
///
/// Returns an error if the index statistics cannot be retrieved.
pub async fn get_index_status_internal(state: &Arc<AppState>) -> Result<IndexStatus, String> {
    let is_running = state.indexing_handle.lock().is_some();

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

/// Gets the current index statistics.
///
/// # Errors
///
/// Returns an error if the indexer statistics are unavailable.
pub async fn get_index_statistics_internal(
    state: &Arc<AppState>,
) -> Result<IndexStatistics, String> {
    state.indexer.get_statistics().map_err(|e| e.to_string())
}

/// Gets a list of recently indexed files.
///
/// # Errors
///
/// Returns an error if the database query fails.
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
