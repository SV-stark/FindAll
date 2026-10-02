mod autostart;
mod export;
mod indexing;
mod search;
mod settings;
mod system;

pub use autostart::{is_auto_start_enabled, set_auto_start};
pub use export::{export_results_csv, export_results_json};
pub use indexing::{
    cancel_indexing_internal, get_index_statistics_internal, get_index_status_internal,
    get_recent_files_internal, start_indexing_internal,
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
    /// Handle to the in-flight indexing run, if any.
    pub indexing_handle: Mutex<Option<tokio::task::JoinHandle<()>>>,
    /// Invalidates the current indexing run when a new one begins.
    ///
    /// A generation-based [`CancellationController`](crate::scanner::cancel::CancellationController)
    /// rather than a plain flag because `JoinHandle::abort` cannot stop the
    /// `spawn_blocking` stages inside `scan_directory`.
    pub indexing_control: crate::scanner::cancel::CancellationController,
    pub db_corrupted: bool,
}

impl AppState {
    pub fn builder() -> AppStateBuilder {
        AppStateBuilder::default()
    }

    /// Cancels any in-flight indexing run.
    pub fn cancel_indexing(&self) {
        self.indexing_control.cancel();
    }

    /// Starts a new indexing run for `path`, superseding any run already in
    /// flight.
    ///
    /// Returns a `Result` so the caller can surface a failure to spawn, rather
    /// than silently leaving the UI in "indexing" forever.
    ///
    /// # Errors
    ///
    /// Propagates a panic from the previous run's task and any error returned by
    /// the scanner.
    pub async fn start_indexing(self: &Arc<Self>, path: std::path::PathBuf) -> crate::error::Result<()> {
        // Supersede the previous run *first*, then wait for it to actually stop.
        let previous = self.indexing_handle.lock().take();
        let cancel = self.indexing_control.begin();

        if let Some(previous) = previous {
            // The token above already told every stage to stop. Give the run a
            // moment to flush and release the index writer so the new run does
            // not interleave with a still-draining one.
            if tokio::time::timeout(
                std::time::Duration::from_secs(5),
                previous,
            )
            .await
            .is_err()
            {
                tracing::warn!("Previous indexing run did not stop within 5s; continuing anyway");
            }
        }

        let settings = self.settings_cache.load();
        let mut exclude_patterns = settings.exclude_patterns.clone();
        exclude_patterns.extend(settings.exclude_folders.iter().cloned());
        // Deduplicate so a folder listed both as a pattern and as a folder is not
        // compiled twice.
        exclude_patterns.sort_unstable();
        exclude_patterns.dedup();

        let scanner = Arc::clone(&self.scanner);
        let handle = tokio::spawn(async move {
            match scanner.scan_directory(path, exclude_patterns, cancel).await {
                Ok(report) => {
                    if report.write_errors > 0 {
                        tracing::error!(
                            "Indexing completed with {} write errors ({} documents written)",
                            report.write_errors,
                            report.documents_written
                        );
                    } else {
                        tracing::info!(
                            "Indexing completed: {} documents written{}",
                            report.documents_written,
                            if report.cancelled { " (cancelled)" } else { "" }
                        );
                    }
                }
                Err(e) => tracing::error!("Indexing failed: {e}"),
            }
        });

        *self.indexing_handle.lock() = Some(handle);
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub fn new(
        indexer: Arc<IndexManager>,
        metadata_db: Arc<MetadataDb>,
        settings: AppSettings,
        settings_manager: SettingsManager,
        watcher: WatcherManager,
        filename_index: Option<Arc<FilenameIndex>>,
        progress_tx: flume::Sender<crate::scanner::ProgressEvent>,
        scanner: Arc<crate::scanner::Scanner>,
        db_corrupted: bool,
    ) -> Self {
        let mut watcher = watcher;
        let _ = watcher.update_watch_list(&settings.index_dirs);
        Self {
            indexer,
            metadata_db,
            settings_manager: Arc::new(settings_manager),
            settings_cache: ArcSwap::from_pointee(settings),
            watcher: Mutex::new(watcher),
            filename_index,
            progress_tx,
            scanner,
            indexing_handle: Mutex::new(None),
            indexing_control: crate::scanner::cancel::CancellationController::new(),
            db_corrupted,
        }
    }
}

#[derive(Default)]
pub struct AppStateBuilder {
    indexer: Option<Arc<IndexManager>>,
    metadata_db: Option<Arc<MetadataDb>>,
    settings: Option<AppSettings>,
    settings_manager: Option<SettingsManager>,
    watcher: Option<WatcherManager>,
    filename_index: Option<Arc<FilenameIndex>>,
    progress_tx: Option<flume::Sender<crate::scanner::ProgressEvent>>,
    scanner: Option<Arc<crate::scanner::Scanner>>,
    db_corrupted: Option<bool>,
}

impl AppStateBuilder {
    #[must_use]
    pub fn indexer(mut self, indexer: Arc<IndexManager>) -> Self {
        self.indexer = Some(indexer);
        self
    }

    #[must_use]
    pub fn metadata_db(mut self, metadata_db: Arc<MetadataDb>) -> Self {
        self.metadata_db = Some(metadata_db);
        self
    }

    /// Supplies the already-loaded settings.
    ///
    /// `AppState::new` used to re-read `settings.json` from disk, which meant the
    /// scan configuration could differ from the one `setup_app` had just used to
    /// open the index.
    #[must_use]
    pub fn settings(mut self, settings: AppSettings) -> Self {
        self.settings = Some(settings);
        self
    }

    #[must_use]
    pub fn settings_manager(mut self, settings_manager: SettingsManager) -> Self {
        self.settings_manager = Some(settings_manager);
        self
    }

    #[must_use]
    pub fn watcher(mut self, watcher: WatcherManager) -> Self {
        self.watcher = Some(watcher);
        self
    }

    #[must_use]
    pub fn filename_index(mut self, filename_index: Option<Arc<FilenameIndex>>) -> Self {
        self.filename_index = filename_index;
        self
    }

    #[must_use]
    pub fn maybe_filename_index(self, filename_index: Option<Arc<FilenameIndex>>) -> Self {
        self.filename_index(filename_index)
    }

    #[must_use]
    pub fn progress_tx(
        mut self,
        progress_tx: flume::Sender<crate::scanner::ProgressEvent>,
    ) -> Self {
        self.progress_tx = Some(progress_tx);
        self
    }

    #[must_use]
    pub fn scanner(mut self, scanner: Arc<crate::scanner::Scanner>) -> Self {
        self.scanner = Some(scanner);
        self
    }

    #[must_use]
    pub const fn db_corrupted(mut self, db_corrupted: bool) -> Self {
        self.db_corrupted = Some(db_corrupted);
        self
    }

    /// Builds the `AppState`.
    ///
    /// # Panics
    ///
    /// Panics if a required field was not supplied. All setters are used by
    /// `setup_app` in a single place, so a missing field is a wiring bug that
    /// should fail loudly at startup rather than degrade silently.
    pub fn build(self) -> AppState {
        let settings_manager = self
            .settings_manager
            .expect("settings_manager is required");
        let settings = self.settings.unwrap_or_else(|| {
            settings_manager.load().unwrap_or_else(|e| {
                tracing::warn!("Failed to load settings (using defaults): {e}");
                AppSettings::default()
            })
        });

        AppState::new(
            self.indexer.expect("indexer is required"),
            self.metadata_db.expect("metadata_db is required"),
            settings,
            settings_manager,
            self.watcher.expect("watcher is required"),
            self.filename_index,
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
