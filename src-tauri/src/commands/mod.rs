mod indexing;
mod search;
mod settings;
mod system;

pub use indexing::*;
pub use search::*;
pub use settings::*;
pub use system::*;

use crate::indexer::{filename_index::FilenameIndex, IndexManager};
use crate::metadata::MetadataDb;
use crate::settings::SettingsManager;
use crate::watcher::WatcherManager;
use std::sync::Arc;
use tokio::sync::Mutex;

pub struct AppState {
    pub indexer: Arc<IndexManager>,
    pub metadata_db: Arc<MetadataDb>,
    pub settings_manager: Arc<SettingsManager>,
    pub watcher: std::sync::Mutex<WatcherManager>,
    pub filename_index: Option<Arc<FilenameIndex>>,
}

impl AppState {
    pub fn new(
        indexer: Arc<IndexManager>,
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
