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

use crate::indexer::{IndexManager, filename_index::FilenameIndex};
use crate::metadata::MetadataDb;
use crate::settings::{AppSettings, SettingsManager};
use crate::watcher::WatcherManager;
use arc_swap::ArcSwap;
use parking_lot::Mutex;
use std::sync::Arc;

pub struct AppState {
    pub indexer: Arc<IndexManager>,
    pub metadata_db: Arc<MetadataDb>,
    pub settings_manager: Arc<SettingsManager>,
    pub settings_cache: ArcSwap<AppSettings>,
    pub watcher: Mutex<WatcherManager>,
    pub filename_index: Option<Arc<FilenameIndex>>,
    pub progress_tx: flume::Sender<crate::scanner::ProgressEvent>,
    pub scanner: Arc<crate::scanner::Scanner>,
    pub indexing_handle: Mutex<Option<tokio::task::JoinHandle<()>>>,
    pub indexing_cancel: Arc<std::sync::atomic::AtomicBool>,
    pub db_corrupted: bool,
}

impl AppState {
    pub fn builder() -> AppStateBuilder {
        AppStateBuilder::default()
    }

    pub fn new(
        indexer: Arc<IndexManager>,
        metadata_db: Arc<MetadataDb>,
        settings_manager: SettingsManager,
        watcher: WatcherManager,
        filename_index: Option<Arc<FilenameIndex>>,
        progress_tx: flume::Sender<crate::scanner::ProgressEvent>,
        scanner: Arc<crate::scanner::Scanner>,
        db_corrupted: bool,
    ) -> Self {
        let cache = settings_manager.load().unwrap_or_else(|e| {
            tracing::warn!("Failed to load settings (using defaults): {}", e);
            AppSettings::default()
        });
        let mut watcher = watcher;
        let _ = watcher.update_watch_list(&cache.index_dirs);
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
            indexing_cancel: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            db_corrupted,
        }
    }
}

#[derive(Default)]
pub struct AppStateBuilder {
    indexer: Option<Arc<IndexManager>>,
    metadata_db: Option<Arc<MetadataDb>>,
    settings_manager: Option<SettingsManager>,
    watcher: Option<WatcherManager>,
    filename_index: Option<Option<Arc<FilenameIndex>>>,
    progress_tx: Option<flume::Sender<crate::scanner::ProgressEvent>>,
    scanner: Option<Arc<crate::scanner::Scanner>>,
    db_corrupted: Option<bool>,
}

impl AppStateBuilder {
    pub fn indexer(mut self, indexer: Arc<IndexManager>) -> Self {
        self.indexer = Some(indexer);
        self
    }

    pub fn metadata_db(mut self, metadata_db: Arc<MetadataDb>) -> Self {
        self.metadata_db = Some(metadata_db);
        self
    }

    pub fn settings_manager(mut self, settings_manager: SettingsManager) -> Self {
        self.settings_manager = Some(settings_manager);
        self
    }

    pub fn watcher(mut self, watcher: WatcherManager) -> Self {
        self.watcher = Some(watcher);
        self
    }

    pub fn filename_index(mut self, filename_index: Option<Arc<FilenameIndex>>) -> Self {
        self.filename_index = Some(filename_index);
        self
    }

    pub fn maybe_filename_index(self, filename_index: Option<Arc<FilenameIndex>>) -> Self {
        self.filename_index(filename_index)
    }

    pub fn progress_tx(
        mut self,
        progress_tx: flume::Sender<crate::scanner::ProgressEvent>,
    ) -> Self {
        self.progress_tx = Some(progress_tx);
        self
    }

    pub fn scanner(mut self, scanner: Arc<crate::scanner::Scanner>) -> Self {
        self.scanner = Some(scanner);
        self
    }

    pub const fn db_corrupted(mut self, db_corrupted: bool) -> Self {
        self.db_corrupted = Some(db_corrupted);
        self
    }

    pub fn build(self) -> AppState {
        AppState::new(
            self.indexer.expect("indexer is required"),
            self.metadata_db.expect("metadata_db is required"),
            self.settings_manager.expect("settings_manager is required"),
            self.watcher.expect("watcher is required"),
            self.filename_index.flatten(),
            self.progress_tx.expect("progress_tx is required"),
            self.scanner.expect("scanner is required"),
            self.db_corrupted.unwrap_or(false),
        )
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
        let results = vec![
            SearchResult::builder()
                .file_path("test.txt".to_string())
                .score(1.0)
                .matched_terms(vec![])
                .snippets(vec![])
                .build(),
        ];

        export_results_csv(&results, csv_path.to_str().unwrap()).unwrap();
        let content = std::fs::read_to_string(csv_path).unwrap();
        assert!(content.contains("Score,File Path,Title"));
        assert!(content.contains("1,test.txt,"));
    }
}
