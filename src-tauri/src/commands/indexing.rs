use tauri::{State, Emitter};
use std::sync::Arc;
use crate::scanner::Scanner;
use crate::indexer::searcher::IndexStatistics;
use crate::models::{IndexStatus, RecentFile};
use crate::commands::AppState;

/// Start indexing a directory
#[tauri::command]
pub async fn start_indexing(
    path: String,
    state: State<'_, Arc<AppState>>,
    app: tauri::AppHandle,
) -> Result<(), String> {
    let path = std::path::PathBuf::from(path);

    let indexer = state.indexer.clone();
    let metadata_db = state.metadata_db.clone();

    let settings = state.settings_manager.load().unwrap_or_default();
    
    let mut exclude_patterns = settings.exclude_patterns;
    for folder in settings.exclude_folders {
        exclude_patterns.push(folder);
    }

    tauri::async_runtime::spawn(async move {
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
    Ok(IndexStatus {
        status: "idle".to_string(),
        files_indexed: 0,
    })
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
    
    tauri::async_runtime::spawn(async move {
        if let Some(index) = filename_index.as_ref() {
            index.clear().ok();
            
            use ignore::WalkBuilder;
            use std::sync::atomic::{AtomicUsize, Ordering};
            
            let count = Arc::new(AtomicUsize::new(0));
            let total = Arc::new(AtomicUsize::new(0));
            
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
