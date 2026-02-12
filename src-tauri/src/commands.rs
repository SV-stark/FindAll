use tauri::State;
use std::sync::Arc;
use tokio::sync::Mutex;
use serde::Serialize;
use crate::indexer::{IndexManager, searcher::SearchResult};
use crate::metadata::MetadataDb;
use crate::scanner::Scanner;
use crate::parsers::parse_file;

use crate::settings::{AppSettings, SettingsManager};
use crate::watcher::WatcherManager;

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
    min_size: Option<u64>,
    max_size: Option<u64>,
    file_extensions: Option<Vec<String>>,
) -> Result<Vec<SearchResult>, String> {
    let indexer = state.indexer.lock().await;

    indexer.search(&query, limit, min_size, max_size, file_extensions.as_deref())
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
    _state: State<'_, Arc<AppState>>,
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

/// Open folder and select file
#[tauri::command]
pub fn open_folder(path: String) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        use std::process::Command;
        Command::new("explorer")
            .arg("/select,")
            .arg(path)
            .spawn()
            .map_err(|e| e.to_string())?;
        Ok(())
    }
    #[cfg(not(target_os = "windows"))]
    {
        let path = std::path::PathBuf::from(path);
        if let Some(parent) = path.parent() {
            opener::reveal(parent).map_err(|e| e.to_string())?;
        }
        Ok(())
    }
}

/// Pick a folder using native dialog
#[tauri::command]
pub async fn select_folder(app: tauri::AppHandle) -> Result<Option<String>, String> {
    use tauri_plugin_dialog::DialogExt;
    
    let (tx, rx) = tokio::sync::oneshot::channel();
    
    app.dialog().file().pick_folder(move |folder| {
        let _ = tx.send(folder.map(|f| f.to_string()));
    });
    
    rx.await.map_err(|e| e.to_string())
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
    state.settings_manager.save_settings(&settings).map_err(|e| e.to_string())?;
    
    // Update watcher
    let mut watcher = state.watcher.lock().unwrap();
    watcher.update_watch_list(settings.index_dirs).map_err(|e| e.to_string())?;
    
    Ok(())
}

/// Copy text to clipboard
#[tauri::command]
pub fn copy_to_clipboard(text: String) -> Result<(), String> {
    use arboard::Clipboard;
    let mut clipboard = Clipboard::new().map_err(|e| e.to_string())?;
    clipboard.set_text(text).map_err(|e| e.to_string())?;
    Ok(())
}

/// Export search results to CSV
#[tauri::command]
pub async fn export_results(
    results: Vec<SearchResult>,
    format: String,
    app: tauri::AppHandle,
) -> Result<(), String> {
    use tauri_plugin_dialog::DialogExt;
    
    let (tx, rx) = tokio::sync::oneshot::channel();
    
    let extension = match format.as_str() {
        "csv" => "csv",
        "json" => "json",
        _ => "txt",
    };
    
    app.dialog().file()
        .add_filter(format.to_uppercase(), &[extension])
        .save_file(move |file_path| {
            let _ = tx.send(file_path.map(|f| f.to_string()));
        });
    
    let file_path = rx.await.map_err(|e| e.to_string())?;
    
    if let Some(path) = file_path {
        let content = match format.as_str() {
            "csv" => {
                let mut csv = String::from("File Path,Title,Score\n");
                for result in results {
                    let title = result.title.unwrap_or_default().replace('"', "\"");
                    csv.push_str(&format!("\"{}\",\"{}\",{}\n", 
                        result.file_path.replace('"', "\""),
                        title,
                        result.score
                    ));
                }
                csv
            }
            "json" => serde_json::to_string_pretty(&results).map_err(|e| e.to_string())?,
            _ => {
                let mut text = String::new();
                for result in results {
                    text.push_str(&format!("{}\t{}\t{}\n", 
                        result.file_path,
                        result.title.unwrap_or_default(),
                        result.score
                    ));
                }
                text
            }
        };
        
        tokio::fs::write(path, content).await.map_err(|e| e.to_string())?;
    }
    
    Ok(())
}

/// Get recent searches
#[tauri::command]
pub fn get_recent_searches(state: State<'_, Arc<AppState>>) -> Result<Vec<String>, String> {
    let settings = state.settings_manager.load().map_err(|e| e.to_string())?;
    Ok(settings.recent_searches.unwrap_or_default())
}

/// Add a search to recent searches
#[tauri::command]
pub fn add_recent_search(
    query: String,
    state: State<'_, Arc<AppState>>,
) -> Result<(), String> {
    let mut settings = state.settings_manager.load().map_err(|e| e.to_string())?;
    
    // Get or initialize recent searches
    let mut recent = settings.recent_searches.unwrap_or_default();
    
    // Remove if already exists (to move to front)
    recent.retain(|q| q != &query);
    
    // Add to front
    recent.insert(0, query);
    
    // Keep only last 10
    recent.truncate(10);
    
    settings.recent_searches = Some(recent);
    state.settings_manager.save_settings(&settings).map_err(|e| e.to_string())?;
    
    Ok(())
}

/// Clear recent searches
#[tauri::command]
pub fn clear_recent_searches(state: State<'_, Arc<AppState>>) -> Result<(), String> {
    let mut settings = state.settings_manager.load().map_err(|e| e.to_string())?;
    settings.recent_searches = Some(vec![]);
    state.settings_manager.save_settings(&settings).map_err(|e| e.to_string())?;
    Ok(())
}

/// Application state shared across commands
pub struct AppState {
    pub indexer: Arc<Mutex<IndexManager>>,
    pub metadata_db: Arc<MetadataDb>,
    pub settings_manager: Arc<SettingsManager>,
    pub watcher: std::sync::Mutex<WatcherManager>,
}

impl AppState {
    pub fn new(
        indexer: Arc<Mutex<IndexManager>>, 
        metadata_db: Arc<MetadataDb>,
        settings_manager: SettingsManager,
        watcher: WatcherManager,
    ) -> Self {
        Self {
            indexer,
            metadata_db,
            settings_manager: Arc::new(settings_manager),
            watcher: std::sync::Mutex::new(watcher),
        }
    }
}
