use crate::error::{FlashError, Result};
use crate::indexer::IndexManager;
use crate::metadata::MetadataDb;
use crate::parsers::parse_file;
use blake3;
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tracing::{error, info, warn};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WatcherAction {
    Index,
    Remove,
}

/// Manages active file system watching with debouncing
pub struct WatcherManager {
    watchers: HashMap<String, RecommendedWatcher>,
    _indexer: Arc<IndexManager>,
    _metadata_db: Arc<MetadataDb>,
    _runtime_handle: tokio::runtime::Handle,
    external_tx: mpsc::Sender<(PathBuf, WatcherAction)>,
}

impl WatcherManager {
    /// Creates a new `WatcherManager`
    ///
    /// # Panics
    ///
    /// Panics if the background processor task fails to spawn.
    pub fn new(
        indexer: Arc<IndexManager>,
        metadata_db: Arc<MetadataDb>,
        allowed_extensions: std::collections::HashSet<String>,
    ) -> Self {
        let (external_tx, external_rx) = mpsc::channel::<(PathBuf, WatcherAction)>(1000);
        let runtime_handle = tokio::runtime::Handle::current();

        // Spawn background processor for debounced events
        Self::spawn_processor_task(
            &runtime_handle,
            external_rx,
            indexer.clone(),
            metadata_db.clone(),
            allowed_extensions,
        );

        Self {
            watchers: HashMap::new(),
            _indexer: indexer,
            _metadata_db: metadata_db,
            _runtime_handle: runtime_handle,
            external_tx,
        }
    }

    fn spawn_processor_task(
        runtime_handle: &tokio::runtime::Handle,
        mut external_rx: mpsc::Receiver<(PathBuf, WatcherAction)>,
        indexer: Arc<IndexManager>,
        metadata_db: Arc<MetadataDb>,
        allowed_extensions: std::collections::HashSet<String>,
    ) {
        const MAX_DEBOUNCE_WAIT: Duration = Duration::from_secs(5);
        const DEBOUNCE_GAP: Duration = Duration::from_millis(500);

        runtime_handle.spawn(async move {
            let mut buffer = HashMap::new();
            let mut first_event_time: Option<std::time::Instant> = None;

            loop {
                let timeout_duration = first_event_time.map_or_else(
                    || Duration::from_secs(3600),
                    |first_time| {
                        let elapsed = first_time.elapsed();
                        if elapsed >= MAX_DEBOUNCE_WAIT {
                            Duration::from_millis(0) // Force flush immediately
                        } else {
                            DEBOUNCE_GAP.min(MAX_DEBOUNCE_WAIT.checked_sub(elapsed).unwrap())
                        }
                    },
                );

                tokio::select! {
                    res = external_rx.recv() => {
                        if let Some((path, action)) = res {
                            if buffer.is_empty() {
                                first_event_time = Some(std::time::Instant::now());
                            }
                            buffer.insert(path, action);
                        } else {
                            break;
                        }
                    }
                    () = tokio::time::sleep(timeout_duration) => {
                        if buffer.is_empty() {
                            continue;
                        }
                        first_event_time = None;
                        let events = std::mem::take(&mut buffer);
                        Self::process_events(events, &indexer, &metadata_db, &allowed_extensions).await;
                    }
                }
            }
        });
    }

    async fn process_events(
        events: HashMap<PathBuf, WatcherAction>,
        indexer: &Arc<IndexManager>,
        metadata_db: &Arc<MetadataDb>,
        allowed_extensions: &std::collections::HashSet<String>,
    ) {
        let mut needs_commit = false;

        // First pass: collect all paths that need to be removed
        let remove_paths: Vec<PathBuf> = events
            .iter()
            .filter(|(_, action)| matches!(action, WatcherAction::Remove))
            .map(|(path, _)| path.clone())
            .collect();

        // Second pass: collect all paths that need to be indexed
        let index_paths: Vec<PathBuf> = events
            .iter()
            .filter(|(_, action)| matches!(action, WatcherAction::Index))
            .map(|(path, _)| path.clone())
            .collect();

        // Process removes first
        for path in remove_paths {
            let path_str = path.to_string_lossy();
            let _ = indexer.remove_document(&path_str);
            if matches!(metadata_db.remove_file(&path), Ok(true)) {
                needs_commit = true;
                info!("Removed file (watcher): {:?}", path);
            }
        }

        // Then process indexes
        let mut docs_to_add = Vec::new();
        let mut meta_to_update = Vec::new();

        for path in index_paths {
            if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                if !allowed_extensions.contains(&ext.to_lowercase()) {
                    continue;
                }
            } else {
                continue;
            }

            match Self::reindex_single_file(&path, metadata_db).await {
                Ok(Some((doc, modified, size, hash))) => {
                    meta_to_update.push((doc.path.clone(), modified, size, hash));
                    docs_to_add.push((doc, modified, size));
                }
                Ok(None) => {} // Skipped
                Err(e) => error!("Watcher error indexing {:?}: {}", path, e),
            }
        }

        if !docs_to_add.is_empty() {
            let _ = indexer.add_documents_batch(&docs_to_add);
            let _ = metadata_db.batch_update_metadata(&meta_to_update);
            needs_commit = true;
        }

        if needs_commit {
            if let Err(e) = indexer.commit() {
                error!("Watcher failed to commit index: {}", e);
            } else {
                indexer.invalidate_cache();
            }
        }
    }

    /// Get a sender to push external events (like USN Journal) into the watcher
    #[must_use]
    pub fn event_tx(&self) -> mpsc::Sender<(PathBuf, WatcherAction)> {
        self.external_tx.clone()
    }

    /// Update the list of watched directories
    pub fn update_watch_list(&mut self, dirs: &[String]) -> Result<()> {
        let current_dirs: std::collections::HashSet<String> = dirs.iter().cloned().collect();
        let existing_dirs: std::collections::HashSet<String> =
            self.watchers.keys().cloned().collect();

        // Remove watchers for directories no longer in the list
        for dir in existing_dirs.difference(&current_dirs) {
            self.watchers.remove(dir);
        }

        // Add watchers for new directories
        for dir in current_dirs.difference(&existing_dirs) {
            let tx = self.external_tx.clone();
            let mut watcher = notify::recommended_watcher(move |res: notify::Result<Event>| {
                if let Ok(event) = res {
                    match event.kind {
                        EventKind::Modify(_) | EventKind::Create(_) | EventKind::Remove(_) => {
                            for path in &event.paths {
                                let action = match event.kind {
                                    EventKind::Remove(_) => WatcherAction::Remove,
                                    _ => WatcherAction::Index,
                                };
                                let _ = tx.try_send((path.clone(), action));
                            }
                        }
                        _ => {}
                    }
                }
            })
            .map_err(|e| FlashError::Io(std::sync::Arc::new(std::io::Error::other(e))))?;

            let path = Path::new(dir);
            if path.exists() {
                watcher
                    .watch(path, RecursiveMode::Recursive)
                    .map_err(|e| FlashError::Io(std::sync::Arc::new(std::io::Error::other(e))))?;
                self.watchers.insert(dir.clone(), watcher);
            }
        }

        Ok(())
    }

    // Returns parsed document data if file needs re-indexing
    async fn reindex_single_file(
        path: &Path,
        metadata_db: &Arc<MetadataDb>,
    ) -> Result<Option<(crate::parsers::ParsedDocument, u64, u64, [u8; 32])>> {
        if !path.exists() {
            return Ok(None);
        }

        let Ok(metadata) = std::fs::metadata(path) else {
            return Ok(None); // Ignore if cannot read metadata
        };

        let modified = metadata
            .modified()
            .unwrap_or(std::time::SystemTime::UNIX_EPOCH)
            .duration_since(std::time::SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let size = metadata.len();

        // Skip check? If watcher said it changed, it probably did.
        // But checking db saves re-hashing if it was a false alarm.
        if !metadata_db
            .needs_reindex(path, modified, size)
            .unwrap_or(true)
        {
            return Ok(None);
        }

        let path_buf = path.to_path_buf();
        let parsed_res = tokio::task::spawn_blocking(move || parse_file(&path_buf))
            .await
            .map_err(|e| FlashError::parse(path, format!("Parse task failed: {e}")))?;

        let parsed = match parsed_res {
            Ok(p) => p,
            Err(e) => {
                warn!("Failed to parse file {:?}: {}", path, e);
                return Ok(None);
            }
        };

        let content_hash: [u8; 32] = blake3::hash(parsed.content.as_bytes()).into();

        Ok(Some((parsed, modified, size, content_hash)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::indexer::IndexManager;
    use crate::metadata::MetadataDb;
    use std::fs;
    use std::sync::Arc;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_watcher_manager_creation() {
        let temp = tempdir().unwrap();
        let indexer = Arc::new(IndexManager::open(temp.path(), 256).unwrap());
        let metadata = Arc::new(MetadataDb::open(&temp.path().join("metadata.db")).unwrap());

        let mut watcher = WatcherManager::new(indexer, metadata, std::collections::HashSet::new());

        // Add a directory to watch
        let watch_dir = temp.path().join("watch_me");
        fs::create_dir(&watch_dir).unwrap();

        assert!(watcher
            .update_watch_list(&[watch_dir.to_string_lossy().to_string()])
            .is_ok());
        assert!(!watcher.watchers.is_empty());

        // Empty list should remove watcher
        assert!(watcher.update_watch_list(&[]).is_ok());
        assert!(watcher.watchers.is_empty());
    }

    #[tokio::test]
    async fn test_reindex_single_file() {
        use std::io::Write;
        let temp = tempdir().unwrap();
        let metadata = Arc::new(MetadataDb::open(&temp.path().join("metadata.db")).unwrap());

        let file_path = temp.path().join("test.txt");
        let mut file = fs::File::create(&file_path).unwrap();
        writeln!(file, "Initial content").unwrap();

        // Should return Some on first index
        let result = WatcherManager::reindex_single_file(&file_path, &metadata).await;
        assert!(result.is_ok());
        let option = result.unwrap();
        assert!(option.is_some());
        let (doc, modified, size, _hash) = option.unwrap();
        assert_eq!(doc.content, "Initial content");

        metadata
            .update_metadata(&file_path, modified, size, [0; 32])
            .unwrap();

        // Should return None if no change
        let result = WatcherManager::reindex_single_file(&file_path, &metadata).await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }
}
