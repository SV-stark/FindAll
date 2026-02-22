use std::sync::Arc;
use crate::scanner::Scanner;
use crate::indexer::searcher::IndexStatistics;
use crate::models::{IndexStatus, RecentFile};
use crate::commands::AppState;
use tracing::error;

pub async fn start_indexing_internal(
    path: String,
    state: Arc<AppState>,
) -> Result<(), String> {
    let path = std::path::PathBuf::from(path);
    let indexer = state.indexer.clone();
    let metadata_db = state.metadata_db.clone();
    let settings = state.settings_manager.load().unwrap_or_default();
    
    let mut exclude_patterns = settings.exclude_patterns;
    for folder in settings.exclude_folders {
        exclude_patterns.push(folder);
    }

    let progress_tx = state.progress_tx.clone();

    tokio::spawn(async move {
        let scanner = Scanner::new(indexer, metadata_db, state.filename_index.clone(), Some(progress_tx));
        if let Err(e) = scanner.scan_directory(path, exclude_patterns).await {
            error!("Indexing error: {}", e);
        }
    });

    Ok(())
}

pub async fn get_index_status_internal() -> Result<IndexStatus, String> {
    Ok(IndexStatus { status: "idle".to_string(), files_indexed: 0 })
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
    let files = state.metadata_db.get_recent_files(limit).map_err(|e| e.to_string())?;
    Ok(files.into_iter().map(|(p, t, m, s)| RecentFile { path: p, title: t, modified: m, size: s }).collect())
}
