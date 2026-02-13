mod search;
mod indexing;
mod settings;
mod system;

pub use search::*;
pub use indexing::*;
pub use settings::*;
pub use system::*;

use std::sync::Arc;
use tokio::sync::Mutex;
use crate::indexer::{IndexManager, filename_index::FilenameIndex};
use crate::metadata::MetadataDb;
use crate::settings::SettingsManager;
use crate::watcher::WatcherManager;

pub struct AppState {
    pub indexer: Arc<Mutex<IndexManager>>,
    pub metadata_db: Arc<MetadataDb>,
    pub settings_manager: Arc<SettingsManager>,
    pub watcher: std::sync::Mutex<WatcherManager>,
    pub filename_index: Option<Arc<FilenameIndex>>,
}

impl AppState {
    pub fn new(
        indexer: Arc<Mutex<IndexManager>>, 
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
