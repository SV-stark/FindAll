pub mod error;
pub mod parsers;
pub mod indexer;
pub mod metadata;
pub mod scanner;
pub mod commands;
pub mod settings;

use tauri::Manager;
use std::sync::Arc;
use commands::{AppState, search_query, start_indexing, get_index_status, get_file_preview, get_home_dir, get_settings, save_settings};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            // Get app data directory for storing index and metadata
            let app_data_dir = app.path().app_data_dir()
                .expect("Failed to get app data directory");

            std::fs::create_dir_all(&app_data_dir)
                .expect("Failed to create app data directory");

            // Initialize settings
            let settings_manager = settings::SettingsManager::new(&app_data_dir);

            // Initialize index
            let index_path = app_data_dir.join("index");
            let indexer = indexer::IndexManager::open(&index_path)
                .expect("Failed to open search index");

            // Initialize metadata database
            let db_path = app_data_dir.join("metadata.redb");
            let metadata_db = metadata::MetadataDb::open(&db_path)
                .expect("Failed to open metadata database");

            // Create and manage app state
            let state = Arc::new(AppState::new(indexer, metadata_db, settings_manager));
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
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
