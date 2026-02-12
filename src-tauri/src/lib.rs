pub mod commands;
pub mod error;
pub mod indexer;
pub mod metadata;
pub mod parsers;
pub mod scanner;
pub mod settings;
pub mod watcher;

use commands::{
    get_file_preview, get_home_dir, get_index_status, get_settings, open_folder, save_settings,
    search_query, select_folder, start_indexing, AppState,
};
use std::sync::Arc;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            // Get app data directory for storing index and metadata
            let app_data_dir = app
                .path()
                .app_data_dir()
                .expect("Failed to get app data directory");

            std::fs::create_dir_all(&app_data_dir).expect("Failed to create app data directory");

            // Initialize settings
            let settings_manager = settings::SettingsManager::new(&app_data_dir);

            // Initialize index
            let index_path = app_data_dir.join("index");
            let indexer =
                indexer::IndexManager::open(&index_path).expect("Failed to open search index");

            // Initialize metadata database
            let db_path = app_data_dir.join("metadata.redb");
            let metadata_db =
                metadata::MetadataDb::open(&db_path).expect("Failed to open metadata database");

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
            get_home_dir,
            get_settings,
            save_settings,
            open_folder,
            select_folder,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
