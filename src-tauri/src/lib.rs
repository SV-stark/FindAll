pub mod commands;
pub mod error;
pub mod indexer;
pub mod metadata;
pub mod models;
pub mod parsers;
pub mod scanner;
pub mod settings;
pub mod slint_ui;
pub mod watcher;

use commands::AppState;
use std::path::PathBuf;
use std::sync::Arc;
use tracing::{info};

pub fn get_app_data_dir() -> PathBuf {
    #[cfg(target_os = "windows")]
    let path = dirs::config_dir().expect("Failed to get config directory");
    
    #[cfg(not(target_os = "windows"))]
    let path = dirs::data_dir().unwrap_or_else(|| {
        dirs::home_dir().map(|h| h.join(".flash-search")).unwrap_or_else(|| PathBuf::from("."))
    });
    
    let mut path = path;
    path.push("com.hp.flash-search");
    path
}

pub fn setup_app() -> (Arc<AppState>, tokio::sync::mpsc::Receiver<crate::scanner::ProgressEvent>) {
    let app_data_dir = get_app_data_dir();
    if !app_data_dir.exists() {
        std::fs::create_dir_all(&app_data_dir).expect("Failed to create app data directory");
    }

    let settings_manager = settings::SettingsManager::new(&app_data_dir);
    let index_path = app_data_dir.join("index");
    let indexer = indexer::IndexManager::open(&index_path).expect("Failed to open search index");
    let db_path = app_data_dir.join("metadata.redb");
    let metadata_db = metadata::MetadataDb::open(&db_path).expect("Failed to open metadata database");

    let metadata_db_shared = Arc::new(metadata_db);
    let indexer_shared = Arc::new(indexer);

    let filename_index = match indexer::filename_index::FilenameIndex::open(&app_data_dir.join("filename_index")) {
        Ok(idx) => Some(Arc::new(idx)),
        Err(_) => None,
    };

    // Initialize watcher
    let watcher = watcher::WatcherManager::new(
        indexer_shared.clone(),
        metadata_db_shared.clone(),
    );

    let (progress_tx, progress_rx) = tokio::sync::mpsc::channel(100);

    let state = Arc::new(AppState::new(
        indexer_shared,
        metadata_db_shared,
        settings_manager,
        watcher,
        filename_index,
        progress_tx,
    ));
    
    (state, progress_rx)
}

pub fn run_slint() {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env()
            .add_directive(tracing::Level::INFO.into()))
        .init();

    info!("Starting Slint UI...");
    let (state, rx) = setup_app();
    slint_ui::run_slint_ui(state, rx);
}

pub fn run_tauri() {
    // Initialize tracing if not already initialized
    let _ = tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env()
            .add_directive(tracing::Level::INFO.into()))
        .try_init();

    info!("Starting Tauri UI...");
    let (state, mut rx) = setup_app();
    
    // Create a clone of state for the event loop
    let state_clone = state.clone();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            search_query,
            get_preview,
            get_preview_highlighted,
            search_filenames,
            get_filename_index_stats,
            start_indexing,
            get_statistics,
            get_recent_files,
            get_settings,
            save_settings,
            open_folder,
            unpin_file,
            pin_file,
            get_pinned_files,
            get_recent_searches,
            add_recent_search,
            copy_to_clipboard,
            get_home_dir,
            build_filename_index,
            export_results
        ])
        .setup(|app| {
            let handle = app.handle().clone();
            use tauri::Emitter;
            tokio::spawn(async move {
                while let Some(event) = rx.recv().await {
                    let _ = handle.emit("indexing-progress", event);
                }
            });
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

// Tauri Command Wrappers
#[tauri::command]
async fn search_query(
    query: String,
    limit: usize,
    min_size: Option<u64>,
    max_size: Option<u64>,
    file_extensions: Option<Vec<String>>,
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<Vec<crate::indexer::searcher::SearchResult>, String> {
    commands::search_query_internal(query, limit, &state, min_size, max_size, file_extensions).await
}

#[tauri::command]
async fn get_preview(path: String) -> Result<String, String> {
    commands::get_file_preview_internal(path).await
}

#[tauri::command]
async fn get_preview_highlighted(path: String, query: String) -> Result<crate::models::PreviewResult, String> {
    commands::get_file_preview_highlighted_internal(path, query).await
}

#[tauri::command]
async fn search_filenames(query: String, limit: usize, state: tauri::State<'_, Arc<AppState>>) -> Result<Vec<crate::models::FilenameSearchResult>, String> {
    commands::search_filenames_internal(query, limit, &state).await
}

#[tauri::command]
async fn get_filename_index_stats(state: tauri::State<'_, Arc<AppState>>) -> Result<crate::models::FilenameIndexStats, String> {
    commands::get_filename_index_stats_internal(&state).await
}

#[tauri::command]
async fn start_indexing(path: String, state: tauri::State<'_, Arc<AppState>>) -> Result<(), String> {
    commands::start_indexing_internal(path, state.inner().clone()).await
}

#[tauri::command]
async fn get_statistics(state: tauri::State<'_, Arc<AppState>>) -> Result<crate::indexer::searcher::IndexStatistics, String> {
    commands::get_index_statistics_internal(&state).await
}

#[tauri::command]
async fn get_recent_files(limit: usize, state: tauri::State<'_, Arc<AppState>>) -> Result<Vec<crate::models::RecentFile>, String> {
    commands::get_recent_files_internal(limit, &state).await
}

#[tauri::command]
async fn get_settings(state: tauri::State<'_, Arc<AppState>>) -> Result<crate::settings::AppSettings, String> {
    commands::get_settings_internal(&state)
}

#[tauri::command]
async fn save_settings(settings: crate::settings::AppSettings, state: tauri::State<'_, Arc<AppState>>) -> Result<(), String> {
    commands::save_settings_internal(settings, &state)
}

#[tauri::command]
async fn open_folder(path: String) -> Result<(), String> {
    commands::open_folder_internal(path)
}

#[tauri::command]
async fn unpin_file(path: String, state: tauri::State<'_, Arc<AppState>>) -> Result<(), String> {
    commands::unpin_file_internal(path, &state)
}

#[tauri::command]
async fn pin_file(path: String, state: tauri::State<'_, Arc<AppState>>) -> Result<(), String> {
    commands::pin_file_internal(path, &state)
}

#[tauri::command]
async fn get_pinned_files(state: tauri::State<'_, Arc<AppState>>) -> Result<Vec<String>, String> {
    commands::get_pinned_files_internal(&state)
}

#[tauri::command]
async fn get_recent_searches(state: tauri::State<'_, Arc<AppState>>) -> Result<Vec<String>, String> {
    commands::get_recent_searches_internal(&state)
}

#[tauri::command]
async fn add_recent_search(query: String, state: tauri::State<'_, Arc<AppState>>) -> Result<(), String> {
    commands::add_recent_search_internal(query, &state)
}

#[tauri::command]
async fn copy_to_clipboard(text: String) -> Result<(), String> {
    commands::copy_to_clipboard_internal(text)
}

#[tauri::command]
async fn get_home_dir() -> Result<String, String> {
    commands::get_home_dir_internal()
}

#[tauri::command]
async fn build_filename_index(path: String, state: tauri::State<'_, Arc<AppState>>) -> Result<(), String> {
    // Re-use start_indexing for now as it builds both if enabled
    commands::start_indexing_internal(path, state.inner().clone()).await
}

#[tauri::command]
async fn export_results(results: Vec<crate::indexer::searcher::SearchResult>, format: String) -> Result<(), String> {
    commands::export_results_internal(results, format).await
}

// Keep the CLI for now
pub async fn run_cli(query: Option<String>, _index_path: Option<String>) -> crate::error::Result<()> {
    if let Some(query_str) = query {
        let (state, _) = setup_app();
        let results = state.indexer.search(&query_str, 20, None, None, None).await?;
        for res in results {
            println!("{} | {}", res.score, res.file_path);
        }
    }
    Ok(())
}
