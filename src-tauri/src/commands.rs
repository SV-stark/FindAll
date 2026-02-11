use tauri::State;
use std::sync::Arc;
use tokio::sync::Mutex;
use serde::Serialize;
use crate::indexer::{IndexManager, searcher::SearchResult};
use crate::metadata::MetadataDb;
use crate::scanner::Scanner;
use crate::parsers::parse_file;

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
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<SearchResult>, String> {
    let indexer = state.indexer.lock().await;
    
    indexer.search(&query)
        .map_err(|e| e.to_string())
}

/// Start indexing a directory
#[tauri::command]
pub async fn start_indexing(
    path: String,
    state: State<'_, Arc<AppState>>,
) -> Result<(), String> {
    let path = std::path::PathBuf::from(path);
    
    // Clone state for the spawned task
    let indexer = state.indexer.clone();
    let metadata_db = state.metadata_db.clone();
    
    // Spawn indexing in background
    tokio::spawn(async move {
        let scanner = Scanner::new(indexer, metadata_db);
        
        if let Err(e) = scanner.scan_directory(path).await {
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

/// Application state shared across commands
pub struct AppState {
    pub indexer: Arc<Mutex<IndexManager>>,
    pub metadata_db: Arc<MetadataDb>,
}

impl AppState {
    pub fn new(indexer: IndexManager, metadata_db: MetadataDb) -> Self {
        Self {
            indexer: Arc::new(Mutex::new(indexer)),
            metadata_db: Arc::new(metadata_db),
        }
    }
}
