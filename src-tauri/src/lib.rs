pub mod commands;
pub mod error;
pub mod indexer;
pub mod metadata;
pub mod models;
pub mod parsers;
pub mod scanner;
pub mod settings;
pub mod watcher;

use commands::{
    add_recent_search, add_search_history, clear_recent_searches, copy_to_clipboard, 
    export_results, filter_by_filename, get_file_preview, get_file_preview_highlighted, 
    get_filename_index_stats, get_home_dir, get_index_status, 
    get_index_statistics, get_recent_files, get_recent_searches, get_settings, 
    get_search_history, get_pinned_files, pin_file, unpin_file, open_folder, 
    save_settings, search_filenames, search_query, select_folder, 
    start_indexing, build_filename_index, AppState,
};
use scanner::Scanner;
use std::path::PathBuf;
use std::sync::Arc;
use tauri::{Manager, Emitter};
use tracing::{info, warn, error};

pub fn get_app_data_dir() -> PathBuf {
    #[cfg(target_os = "windows")]
    let mut path = dirs::config_dir().expect("Failed to get config directory");
    
    #[cfg(not(target_os = "windows"))]
    let mut path = dirs::data_dir().unwrap_or_else(|| {
        dirs::home_dir().map(|h| h.join(".flash-search")).unwrap_or_else(|| PathBuf::from("."))
    });
    
    path.push("com.hp.flash-search");
    path
}

fn truncate(s: &str, max_chars: usize) -> String {
    if s.len() > max_chars {
        format!("{}...", &s[..max_chars - 3])
    } else {
        s.to_string()
    }
}

pub async fn run_cli(query: Option<String>, index_path: Option<String>) -> crate::error::Result<()> {
    let app_data_dir = get_app_data_dir();
    if !app_data_dir.exists() {
        std::fs::create_dir_all(&app_data_dir)?;
    }

    if let Some(query_str) = query {
        let index_path = app_data_dir.join("index");
        let indexer = indexer::IndexManager::open(&index_path)?;
        
        let results = indexer.search(&query_str, 20, None, None, None).await?;
        
        if results.is_empty() {
            println!("No results found for: {}", query_str);
        } else {
            println!("\n🔍 Search results for: {}\n", query_str);
            println!("{:<8} | {:<30} | {}", "Score", "Title", "Path");
            println!("{:-<8}-+-{:-<30}-+-{:-<40}", "", "", "");
            
            for res in results {
                let title = res.title.as_deref().unwrap_or("Untitled");
                println!("{:<8.2} | {:<30} | {}", res.score, truncate(title, 30), res.file_path);
            }
            println!();
        }
    }

    if let Some(path_str) = index_path {
        println!("Indexing directory: {}", path_str);
        // We would need to initialize MetadataDb here too for a proper index
        // For now, let's focus on the search CLI
    }

    Ok(())
}

/// Get all available drives on Windows
#[cfg(target_os = "windows")]
fn get_available_drives() -> Vec<PathBuf> {
    let mut drives = Vec::new();
    
    // Check drive letters A-Z
    for letter in b'A'..=b'Z' {
        let drive_path = PathBuf::from(format!("{}:\\", letter as char));
        if drive_path.exists() {
            drives.push(drive_path);
        }
    }
    
    drives
}

/// Get default search paths (home directory on non-Windows)
#[cfg(not(target_os = "windows"))]
fn get_available_drives() -> Vec<PathBuf> {
    if let Some(home) = dirs::home_dir() {
        vec![home]
    } else {
        vec![PathBuf::from(".")]
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    run_with_args(None, None);
}

pub fn run_with_args(initial_search: Option<String>, index_dir: Option<String>) {
    // Initialize tracing subscriber for structured logging
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env()
            .add_directive(tracing::Level::INFO.into()))
        .with_target(true)
        .with_thread_ids(true)
        .init();
    
    info!("Flash Search starting up");
    
    use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut};

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .setup(|app| {
            // Register global shortcut Win+Shift+F (or Cmd+Shift+F on Mac)
            let shortcut = if cfg!(target_os = "macos") {
                "Cmd+Shift+F"
            } else {
                "CmdOrControl+Shift+F"
            };

            let handle = app.handle().clone();
            app.global_shortcut()
                .on_shortcut(shortcut.parse::<Shortcut>().unwrap(), move |handle, _shortcut, _event| {
                    // Get the main window and show/focus it
                    if let Some(window) = handle.get_webview_window("main") {
                        let _ = window.show();
                        let _ = window.set_focus();
                        let _ = window.unminimize();
                    }
                })
                .expect("Failed to register global shortcut");
            info!("Setting up Flash Search application");
            
            // Get app data directory for storing index and metadata
            let app_data_dir = app
                .path()
                .app_data_dir()
                .expect("Failed to get app data directory");
            
            info!(app_data_dir = %app_data_dir.display(), "Using app data directory");

            std::fs::create_dir_all(&app_data_dir).expect("Failed to create app data directory");

            // Initialize settings
            let settings_manager = settings::SettingsManager::new(&app_data_dir);
            info!("Settings manager initialized");

            // Initialize index
            let index_path = app_data_dir.join("index");
            info!(index_path = %index_path.display(), "Opening search index");
            let indexer =
                indexer::IndexManager::open(&index_path).expect("Failed to open search index");
            info!("Search index opened successfully");

            // Initialize metadata database
            let db_path = app_data_dir.join("metadata.redb");
            info!(db_path = %db_path.display(), "Opening metadata database");
            let metadata_db =
                metadata::MetadataDb::open(&db_path).expect("Failed to open metadata database");
            info!("Metadata database opened successfully");

            let metadata_db_shared = Arc::new(metadata_db);
            let indexer_shared = Arc::new(indexer);

            // Initialize watcher
            let mut watcher = watcher::WatcherManager::new(
                app.handle().clone(),
                indexer_shared.clone(),
                metadata_db_shared.clone(),
            );

            let initial_settings = settings_manager.load().unwrap_or_default();
            
            // Auto-index on startup: scan all drives if no index dirs configured
            // This gives a "works out of the box" experience like AnyTXT
            let should_auto_index = initial_settings.index_dirs.is_empty() && initial_settings.auto_index_on_startup;
            
            // Update watcher with index dirs (this moves index_dirs)
            watcher.update_watch_list(initial_settings.index_dirs.clone()).ok();
            
            // Initialize filename index (fast filename-only search) - enabled by default
            // The open method will create the index if it doesn't exist
            let filename_index = match indexer::filename_index::FilenameIndex::open(&app_data_dir.join("filename_index")) {
                Ok(idx) => {
                    info!("Filename index opened successfully");
                    Some(Arc::new(idx))
                }
                Err(e) => {
                    warn!("Failed to open/create filename index: {}", e);
                    None
                }
            };

            // Create and manage app state
            let state = Arc::new(AppState::new(
                indexer_shared,
                metadata_db_shared,
                settings_manager,
                watcher,
                filename_index,
            ));
            app.manage(state.clone());

            // Auto-index all drives on first startup
            if should_auto_index {
                let app_handle = app.handle().clone();
                let indexer = state.indexer.clone();
                let metadata_db = state.metadata_db.clone();
                let settings = state.settings_manager.load().unwrap_or_default();
                
                // Get all available drives on Windows
                tauri::async_runtime::spawn(async move {
                    let drives = get_available_drives();
                    info!(?drives, "Auto-indexing available drives");
                    
                    for drive in drives {
                        let scanner = Scanner::new(
                            indexer.clone(),
                            metadata_db.clone(),
                            app_handle.clone()
                        );
                        
                        // Combine exclude_patterns with exclude_folders
                        let mut exclude_patterns = settings.exclude_patterns.clone();
                        exclude_patterns.extend(settings.exclude_folders.clone());
                        
                        if let Err(e) = scanner.scan_directory(drive, exclude_patterns).await {
                            error!(error = %e, "Failed to index drive");
                        }
                    }
                });
            }

            // Handle command-line arguments
            if let Some(search) = initial_search {
                // Emit initial search query to frontend
            let _handle = app.handle().clone();
                let search_clone = search.clone();
                std::thread::spawn(move || {
                    std::thread::sleep(std::time::Duration::from_millis(500));
                    if let Some(window) = handle.get_webview_window("main") {
                        let _ = window.emit("initial-search", search_clone);
                    }
                });
            }

            if let Some(dir) = index_dir {
                // Start indexing the specified directory
                let app_handle = app.handle().clone();
                let dir_clone = dir.clone();
                let indexer = state.indexer.clone();
                let metadata_db = state.metadata_db.clone();
                let settings = state.settings_manager.load().unwrap_or_default();
                
                // Combine exclude_patterns with exclude_folders
                let mut exclude_patterns = settings.exclude_patterns;
                exclude_patterns.extend(settings.exclude_folders);
                
                tauri::async_runtime::spawn(async move {
                    let scanner = Scanner::new(indexer, metadata_db, app_handle);
                    let _ = scanner.scan_directory(std::path::PathBuf::from(dir_clone), exclude_patterns).await;
                });
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            search_query,
            start_indexing,
            get_index_status,
            get_file_preview,
            get_file_preview_highlighted,
            get_home_dir,
            get_settings,
            save_settings,
            open_folder,
            select_folder,
            copy_to_clipboard,
            export_results,
            get_recent_searches,
            add_recent_search,
            clear_recent_searches,
            get_index_statistics,
            get_recent_files,
            pin_file,
            unpin_file,
            get_pinned_files,
            search_filenames,
            get_filename_index_stats,
            build_filename_index,
            add_search_history,
            get_search_history,
            filter_by_filename,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
