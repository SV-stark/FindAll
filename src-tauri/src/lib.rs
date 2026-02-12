pub mod commands;
pub mod error;
pub mod indexer;
pub mod metadata;
pub mod parsers;
pub mod scanner;
pub mod settings;
pub mod watcher;

use commands::{
    add_recent_search, clear_recent_searches, copy_to_clipboard, export_results,
    get_file_preview, get_file_preview_highlighted, get_home_dir, get_index_status, 
    get_index_statistics, get_recent_files, get_recent_searches, get_settings, 
    get_pinned_files, pin_file, unpin_file, open_folder, save_settings, 
    search_query, select_folder, start_indexing, AppState,
};
use std::sync::Arc;
use tauri::Manager;
use tracing::{info, warn};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
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
                .on_shortcut(shortcut.parse::<Shortcut>().unwrap(), move |_app, _shortcut| {
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
            let indexer_shared = Arc::new(tokio::sync::Mutex::new(indexer));

            // Initialize watcher
            let mut watcher = watcher::WatcherManager::new(
                app.handle().clone(),
                indexer_shared.clone(),
                metadata_db_shared.clone(),
            );

            let initial_settings = settings_manager.load().unwrap_or_default();
            watcher.update_watch_list(initial_settings.index_dirs).ok();

            // Create and manage app state
            let state = Arc::new(AppState::new(
                indexer_shared,
                metadata_db_shared,
                settings_manager,
                watcher,
            ));
            app.manage(state);

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
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
