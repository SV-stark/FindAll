pub mod commands;
pub mod error;
pub mod iced_ui;
pub mod indexer;
pub mod metadata;
pub mod models;
pub mod parsers;
pub mod scanner;
pub mod settings;
pub mod system;
pub mod watcher;

use crate::error::{Context, FlashError, Result};
use commands::AppState;
use std::path::PathBuf;
use std::sync::Arc;
use tracing::{error, info};

pub fn get_app_data_dir() -> std::result::Result<PathBuf, FlashError> {
    #[cfg(target_os = "windows")]
    let path = dirs::config_dir()
        .ok_or_else(|| FlashError::config("config_dir", "Could not find config directory"))?;
    
    #[cfg(not(target_os = "windows"))]
    let path = dirs::data_dir().or_else(|| {
        dirs::home_dir().map(|h| h.join(".flash-search"))
    }).ok_or_else(|| FlashError::config("data_dir", "Could not find data directory"))?;
    
    let mut path = path;
    path.push("com.flashsearch");
    Ok(path)
}

pub fn setup_app() -> std::result::Result<(Arc<AppState>, tokio::sync::mpsc::Receiver<crate::scanner::ProgressEvent>), FlashError> {
    let app_data_dir = get_app_data_dir()?;
    
    if !app_data_dir.exists() {
        std::fs::create_dir_all(&app_data_dir)
            .context("Failed to create app data directory")?;
    }

    info!("App data directory: {:?}", app_data_dir);

    let settings_manager = settings::SettingsManager::new(&app_data_dir);
    let index_path = app_data_dir.join("index");
    let indexer = indexer::IndexManager::open(&index_path)
        .map_err(|e| FlashError::Index { msg: format!("Failed to open search index: {}", e), field: None })?;
    let db_path = app_data_dir.join("metadata.redb");
    let metadata_db = metadata::MetadataDb::open(&db_path)
        .map_err(|e| FlashError::database("open", "metadata.redb", e.to_string()))?;

    let metadata_db_shared = Arc::new(metadata_db);
    let indexer_shared = Arc::new(indexer);

    let filename_index = match indexer::filename_index::FilenameIndex::open(&app_data_dir.join("filename_index")) {
        Ok(idx) => Some(Arc::new(idx)),
        Err(e) => {
            error!("Failed to open filename index: {}", e);
            None
        }
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
    
    Ok((state, progress_rx))
}

pub fn run_ui() -> std::result::Result<(), FlashError> {
    let result = setup_app().map(|(state, rx)| {
        iced_ui::run_ui(Ok(state), rx);
    });
    
    if let Err(e) = result {
        iced_ui::run_ui(Err(e.to_string()), tokio::sync::mpsc::channel(1).1);
    }
    
    Ok(())
}

pub async fn run_cli(query: Option<String>, _index_path: Option<String>) -> crate::error::Result<()> {
    if let Some(query_str) = query {
        let (state, _) = setup_app()?;
        let results = state.indexer.search(&query_str, 20, None, None, None).await?;
        for res in results {
            println!("{} | {}", res.score, res.file_path);
        }
    }
    Ok(())
}
