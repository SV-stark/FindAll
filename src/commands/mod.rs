mod autostart;
mod export;
mod indexing;
mod search;
mod settings;
mod system;

pub use autostart::{is_auto_start_enabled, set_auto_start};
pub use export::{export_results_csv, export_results_json};
pub use indexing::{
    get_index_statistics_internal, get_index_status_internal, get_recent_files_internal,
    start_indexing_internal,
};
pub use search::{
    get_file_preview_highlighted_internal, get_file_preview_internal,
    get_filename_index_stats_internal, search_filenames_internal, search_query_internal,
};
pub use settings::{
    add_recent_search_internal, add_search_history_internal, clear_recent_searches_internal,
    get_pinned_files_internal, get_recent_searches_internal, get_search_history_internal,
    get_settings_internal, pin_file_internal, save_settings_internal, unpin_file_internal,
};
pub use system::{
    copy_to_clipboard_internal, export_results_internal, get_home_dir_internal,
    open_folder_internal, select_folder_internal,
};

use crate::indexer::{filename_index::FilenameIndex, IndexManager};
use crate::metadata::MetadataDb;
use crate::settings::{AppSettings, SettingsManager};
use crate::watcher::WatcherManager;
use arc_swap::ArcSwap;
use parking_lot::Mutex;
use std::sync::Arc;
use tokio::sync::mpsc;

pub struct AppState {
    pub indexer: Arc<IndexManager>,
    pub metadata_db: Arc<MetadataDb>,
    pub settings_manager: Arc<SettingsManager>,
    pub settings_cache: ArcSwap<AppSettings>,
    pub watcher: Mutex<WatcherManager>,
    pub filename_index: Option<Arc<FilenameIndex>>,
    pub progress_tx: mpsc::Sender<crate::scanner::ProgressEvent>,
    pub scanner: Arc<crate::scanner::Scanner>,
    pub indexing_handle: Mutex<Option<tokio::task::JoinHandle<()>>>,
}

#[bon::bon]
impl AppState {
    #[builder]
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
        let mut watcher = watcher;
        let _ = watcher.update_watch_list(cache.index_dirs.clone());
        Self {
            indexer,
            metadata_db,
            settings_manager: Arc::new(settings_manager),
            settings_cache: ArcSwap::from_pointee(cache),
            watcher: Mutex::new(watcher),
            filename_index,
            progress_tx,
            scanner,
            indexing_handle: Mutex::new(None),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::indexer::searcher::SearchResult;
    use tempfile::tempdir;

    #[test]
    fn test_export_csv() {
        let temp_dir = tempdir().unwrap();
        let csv_path = temp_dir.path().join("test.csv");
        let results = vec![SearchResult::builder()
            .file_path("test.txt".to_string())
            .score(1.0)
            .matched_terms(vec![])
            .snippets(vec![])
            .build()];

        export_results_csv(&results, csv_path.to_str().unwrap()).unwrap();
        let content = std::fs::read_to_string(csv_path).unwrap();
        assert!(content.contains("Score,File Path,Title"));
        assert!(content.contains("1,test.txt,"));
    }
}
