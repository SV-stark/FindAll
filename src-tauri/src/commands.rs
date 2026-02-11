use tauri::State;
use std::sync::Arc;
use tokio::sync::Mutex;
use serde::Serialize;
use crate::indexer::{IndexManager, searcher::SearchResult};
use crate::metadata::MetadataDb;
use crate::scanner::Scanner;
use crate::parsers::parse_file;

use crate::settings::{AppSettings, SettingsManager};

#[derive(Serialize)]
pub struct IndexStatus {
    pub status: String,
    pub files_indexed: usize,
}

/// Get user's home directory
#[tauri::command]
pub fn get_home_dir() -> Result<String, String> {
    dirs::home_dir()
        .map(|p| p.to_string_lossy().to_string())
        .ok_or_else(|| "Could not determine home directory".to_string())
}

/// Search command - queries the index and returns results
#[tauri::command]
pub async fn search_query(
    query: String,
    limit: usize,
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<SearchResult>, String> {
    let indexer = state.indexer.lock().await;

    indexer.search(&query, limit)
        .map_err(|e| e.to_string())
}

/// Start indexing a directory
#[tauri::command]
pub async fn start_indexing(
    path: String,
    state: State<'_, Arc<AppState>>,
    app: tauri::AppHandle,
) -> Result<(), String> {
    let path = std::path::PathBuf::from(path);

    // Clone state for the spawned task
    let indexer = state.indexer.clone();
    let metadata_db = state.metadata_db.clone();

    // Load exclusion patterns from settings
    let settings = state.settings_manager.load().unwrap_or_default();
    let exclude_patterns = settings.exclude_patterns;

    // Spawn indexing in background
    tokio::spawn(async move {
        let scanner = Scanner::new(indexer, metadata_db, app);

        if let Err(e) = scanner.scan_directory(path, exclude_patterns).await {
            eprintln!("Indexing error: {}", e);
        }
    });

    Ok(())
}

/// Get indexing status
#[tauri::command]
pub async fn get_index_status(
    state: State<'_, Arc<AppState>>,
) -> Result<IndexStatus, String> {
    // This is a placeholder - in production, track actual indexing progress
    Ok(IndexStatus {
        status: "idle".to_string(),
        files_indexed: 0,
    })
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

/// Get current settings
#[tauri::command]
pub fn get_settings(state: State<'_, Arc<AppState>>) -> Result<AppSettings, String> {
    state.settings_manager.load().map_err(|e| e.to_string())
}

/// Save settings
#[tauri::command]
pub fn save_settings(
    settings: AppSettings,
    state: State<'_, Arc<AppState>>,
) -> Result<(), String> {
    state.settings_manager.save_settings(&settings).map_err(|e| e.to_string())
}

/// Application state shared across commands
pub struct AppState {
    pub indexer: Arc<Mutex<IndexManager>>,
    pub metadata_db: Arc<MetadataDb>,
    pub settings_manager: Arc<SettingsManager>,
}

impl AppState {
    pub fn new(
        indexer: IndexManager, 
        metadata_db: MetadataDb,
        settings_manager: SettingsManager,
    ) -> Self {
        Self {
            indexer: Arc::new(Mutex::new(indexer)),
            metadata_db: Arc::new(metadata_db),
            settings_manager: Arc::new(settings_manager),
        }
    }
}
