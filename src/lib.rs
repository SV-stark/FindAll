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
use tracing::info;

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

    let scanner = Arc::new(crate::scanner::Scanner::new(
        indexer_shared.clone(),
        metadata_db_shared.clone(),
        filename_index.clone(),
        Some(progress_tx.clone()),
    ));

    let state = Arc::new(AppState::new(
        indexer_shared,
        metadata_db_shared,
        settings_manager,
        watcher,
        filename_index,
        progress_tx,
        scanner,
    ));
    
    (state, progress_rx)
}

pub fn run_ui() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env()
            .add_directive(tracing::Level::INFO.into()))
        .init();

    info!("Starting Flash Search UI...");
    let (state, rx) = setup_app();
    
    slint_ui::run_slint_ui(state, rx);
}

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
