use tauri::{State, Emitter};
use std::sync::{Arc, Mutex as StdMutex};
use tokio::sync::Mutex;
use serde::Serialize;
use crate::indexer::{IndexManager, searcher::SearchResult, searcher::IndexStatistics, filename_index::FilenameIndex};
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

/// Get index statistics
#[tauri::command]
pub async fn get_index_statistics(
    state: State<'_, Arc<AppState>>,
) -> Result<IndexStatistics, String> {
    let indexer = state.indexer.lock().await;
    indexer.get_statistics().map_err(|e| e.to_string())
}

/// Get recently modified files
#[tauri::command]
pub async fn get_recent_files(
    limit: usize,
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<RecentFile>, String> {
    let files = state.metadata_db.get_recent_files(limit)
        .map_err(|e| e.to_string())?;
    
    Ok(files.into_iter()
        .map(|(path, title, modified, size)| RecentFile {
            path,
            title,
            modified,
            size,
        })
        .collect())
}

/// Pin a file for quick access
#[tauri::command]
pub fn pin_file(
    path: String,
    state: State<'_, Arc<AppState>>,
) -> Result<(), String> {
    let mut settings = state.settings_manager.load().map_err(|e| e.to_string())?;
    
    if !settings.pinned_files.contains(&path) {
        settings.pinned_files.push(path);
        state.settings_manager.save_settings(&settings).map_err(|e| e.to_string())?;
    }
    
    Ok(())
}

/// Unpin a file
#[tauri::command]
pub fn unpin_file(
    path: String,
    state: State<'_, Arc<AppState>>,
) -> Result<(), String> {
    let mut settings = state.settings_manager.load().map_err(|e| e.to_string())?;
    settings.pinned_files.retain(|p| p != &path);
    state.settings_manager.save_settings(&settings).map_err(|e| e.to_string())?;
    Ok(())
}

/// Get pinned files
#[tauri::command]
pub fn get_pinned_files(state: State<'_, Arc<AppState>>) -> Result<Vec<String>, String> {
    let settings = state.settings_manager.load().map_err(|e| e.to_string())?;
    Ok(settings.pinned_files)
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
        let (total_files, index_size) = filename_index.get_stats()
            .map_err(|e| e.to_string())?;
        Ok(FilenameIndexStats {
            total_files,
            index_size_bytes: index_size,
        })
    } else {
        Err("Filename index not initialized".to_string())
    }
}

/// Start building filename index
#[tauri::command]
pub async fn build_filename_index(
    path: String,
    state: State<'_, Arc<AppState>>,
    app: tauri::AppHandle,
) -> Result<(), String> {
    if state.filename_index.is_none() {
        return Err("Filename index not initialized".to_string());
    }
    
    let filename_index = state.filename_index.clone();
    let app_handle = app;
    
    tokio::spawn(async move {
        if let Some(index) = filename_index.as_ref() {
            // Clear existing index
            index.clear().ok();
            
            // Walk directory and index filenames
            use ignore::WalkBuilder;
            use std::sync::atomic::{AtomicUsize, Ordering};
            
            let count = Arc::new(AtomicUsize::new(0));
            let total = Arc::new(AtomicUsize::new(0));
            
            // First pass: count files
            for entry in WalkBuilder::new(&path)
                .hidden(true)
                .ignore(true)
                .build()
            {
                if let Ok(entry) = entry {
                    if entry.file_type().map(|ft| ft.is_file()).unwrap_or(false) {
                        total.fetch_add(1, Ordering::Relaxed);
                    }
                }
            }
            
            // Second pass: index filenames
            let mut batch = Vec::new();
            let batch_size = 1000;
            
            for entry in WalkBuilder::new(&path)
                .hidden(true)
                .ignore(true)
                .build()
            {
                if let Ok(entry) = entry {
                    if entry.file_type().map(|ft| ft.is_file()).unwrap_or(false) {
                        if let Some(name) = entry.file_name().to_str() {
                            if let Some(path_str) = entry.path().to_str() {
                                batch.push((path_str.to_string(), name.to_string()));
                            }
                        }
                        
                        if batch.len() >= batch_size {
                            // Add batch to index
                            for (path, name) in batch.drain(..) {
                                if let Err(e) = index.add_file(&path, &name) {
                                    eprintln!("Failed to add file to filename index: {}", e);
                                }
                            }
                            index.commit().ok();
                            
                            let processed = count.fetch_add(batch_size, Ordering::Relaxed) + batch_size;
                            let _ = app_handle.emit("filename-index-progress", serde_json::json!({
                                "processed": processed,
                                "total": total.load(Ordering::Relaxed),
                                "status": "indexing"
                            }));
                        }
                    }
                }
            }
            
            // Add remaining batch
            for (path, name) in batch {
                index.add_file(&path, &name).ok();
            }
            index.commit().ok();
            
            let _ = app_handle.emit("filename-index-progress", serde_json::json!({
                "processed": total.load(Ordering::Relaxed),
                "total": total.load(Ordering::Relaxed),
                "status": "done"
            }));
        }
    });
    
    Ok(())
}

/// Add to search history with frequency tracking
#[tauri::command]
pub fn add_search_history(
    query: String,
    state: State<'_, Arc<AppState>>,
) -> Result<(), String> {
    let mut settings = state.settings_manager.load().map_err(|e| e.to_string())?;
    
    // Get or initialize search history with frequency
    let mut history = settings.search_history.unwrap_or_default();
    
    // Check if query exists
    let mut found = false;
    for item in &mut history {
        if item.query == query {
            item.frequency += 1;
            item.last_used = std::time::SystemTime::now()
                .duration_since(std::time::SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_secs();
            found = true;
            break;
        }
    }
    
    if !found {
        // Add new entry
        history.insert(0, crate::settings::SearchHistoryItem {
            query,
            frequency: 1,
            last_used: std::time::SystemTime::now()
                .duration_since(std::time::SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        });
    }
    
    // Sort by frequency
    history.sort_by(|a, b| b.frequency.cmp(&a.frequency));
    
    // Keep only top 50
    history.truncate(50);
    
    settings.search_history = Some(history);
    state.settings_manager.save_settings(&settings).map_err(|e| e.to_string())?;
    
    Ok(())
}

/// Get search history sorted by frequency
#[tauri::command]
pub fn get_search_history(
    limit: usize,
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<SearchHistoryItem>, String> {
    let settings = state.settings_manager.load().map_err(|e| e.to_string())?;
    let history = settings.search_history.unwrap_or_default();
    
    // Sort by frequency and return top N
    let mut sorted = history;
    sorted.sort_by(|a, b| b.frequency.cmp(&a.frequency));
    sorted.truncate(limit);
    
    Ok(sorted.into_iter().map(|item| SearchHistoryItem {
        query: item.query,
        frequency: item.frequency,
        last_used: item.last_used,
    }).collect())
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

/// Application state shared across commands
pub struct AppState {
    pub indexer: Arc<Mutex<IndexManager>>,
    pub metadata_db: Arc<MetadataDb>,
    pub settings_manager: Arc<SettingsManager>,
    pub watcher: std::sync::Mutex<WatcherManager>,
    pub filename_index: Option<Arc<FilenameIndex>>,
}

/// Search result with highlighted content
#[derive(Serialize)]
pub struct PreviewResult {
    pub content: String,
    pub matched_terms: Vec<String>,
}

/// File information for recent files
#[derive(Serialize)]
pub struct RecentFile {
    pub path: String,
    pub title: Option<String>,
    pub modified: u64,
    pub size: u64,
}

/// Filename search result
#[derive(Serialize)]
pub struct FilenameSearchResult {
    pub file_path: String,
    pub file_name: String,
}

/// Filename index statistics
#[derive(Serialize)]
pub struct FilenameIndexStats {
    pub total_files: usize,
    pub index_size_bytes: u64,
}

/// Search history with frequency
#[derive(Serialize)]
pub struct SearchHistoryItem {
    pub query: String,
    pub frequency: u32,
    pub last_used: u64,
}

impl AppState {
    pub fn new(
        indexer: Arc<Mutex<IndexManager>>, 
        metadata_db: Arc<MetadataDb>,
        settings_manager: SettingsManager,
        watcher: WatcherManager,
        filename_index: Option<Arc<FilenameIndex>>,
    ) -> Self {
        Self {
            indexer,
            metadata_db,
            settings_manager: Arc::new(settings_manager),
            watcher: std::sync::Mutex::new(watcher),
            filename_index,
        }
    }
}
