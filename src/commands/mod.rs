mod autostart;
mod export;
mod indexing;
mod search;
mod settings;
mod system;

pub use autostart::*;
pub use export::*;
pub use indexing::*;
pub use search::*;
pub use settings::*;
pub use system::*;

use crate::indexer::{filename_index::FilenameIndex, IndexManager};
use crate::metadata::MetadataDb;
use crate::settings::{AppSettings, SettingsManager};
use crate::watcher::WatcherManager;
use std::sync::{Arc, RwLock};
use tokio::sync::mpsc;

pub struct AppState {
    pub indexer: Arc<IndexManager>,
    pub metadata_db: Arc<MetadataDb>,
    pub settings_manager: Arc<SettingsManager>,
    pub settings_cache: Arc<RwLock<AppSettings>>,
    pub watcher: std::sync::Mutex<WatcherManager>,
    pub filename_index: Option<Arc<FilenameIndex>>,
    pub progress_tx: mpsc::Sender<crate::scanner::ProgressEvent>,
    pub scanner: Arc<crate::scanner::Scanner>,
}

impl AppState {
    pub fn new(
        indexer: Arc<IndexManager>,
        metadata_db: Arc<MetadataDb>,
        settings_manager: SettingsManager,
        watcher: WatcherManager,
        filename_index: Option<Arc<FilenameIndex>>,
        progress_tx: mpsc::Sender<crate::scanner::ProgressEvent>,
        scanner: Arc<crate::scanner::Scanner>,
    ) -> Self {
        let cache = settings_manager.load().unwrap_or_default();
        Self {
            indexer,
            metadata_db,
            settings_manager: Arc::new(settings_manager),
            settings_cache: Arc::new(RwLock::new(cache)),
            watcher: std::sync::Mutex::new(watcher),
            filename_index,
            progress_tx,
            scanner,
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_export_csv() {
        // Basic test placeholder
    }
}
