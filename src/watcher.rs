use crate::error::{FlashError, Result};
use crate::indexer::IndexManager;
use crate::metadata::MetadataDb;
use crate::parsers::parse_file;
use blake3;
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tracing::{error, info, warn};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WatcherAction {
    Index,
    Remove,
}

/// Manages active file system watching with debouncing
pub struct WatcherManager {
    watcher: Option<RecommendedWatcher>,
    _indexer: Arc<IndexManager>,
    _metadata_db: Arc<MetadataDb>,
    _runtime_handle: tokio::runtime::Handle,
    // Buffer for pending events: Map<Path, Action>
    event_buffer: Arc<Mutex<HashMap<PathBuf, WatcherAction>>>,
    external_tx: mpsc::Sender<(PathBuf, WatcherAction)>,
}

impl WatcherManager {
    pub fn new(indexer: Arc<IndexManager>, metadata_db: Arc<MetadataDb>) -> Self {
        let buffer: Arc<Mutex<HashMap<PathBuf, WatcherAction>>> =
            Arc::new(Mutex::new(HashMap::new()));
        let buffer_for_task = buffer.clone();
        let indexer_for_task = indexer.clone();
        let metadata_db_for_task = metadata_db.clone();
        let (external_tx, mut external_rx) = mpsc::channel::<(PathBuf, WatcherAction)>(1000);
        let runtime_handle = tokio::runtime::Handle::current();

        // Spawn background processor for debounced events
        runtime_handle.spawn(async move {
            loop {
                // Use select to handle both periodic flushes and incoming external events
                tokio::select! {
                    Some((path, action)) = external_rx.recv() => {
                        if let Ok(mut guard) = buffer_for_task.lock() {
                            guard.insert(path, action);
                        }
                    }
                    _ = tokio::time::sleep(Duration::from_millis(1000)) => {
                        let events = {
                            let mut guard = match buffer_for_task.lock() {
                                Ok(g) => g,
                                Err(e) => {
                                    error!("Watcher lock poisoned: {}", e);
                                    continue;
                                }
                            };
                            if guard.is_empty() {
                                continue;
                            }
                            // Take all events
                            std::mem::take(&mut *guard)
                        };

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
                            let _ = indexer_for_task.remove_document(&path_str);
                            match metadata_db_for_task.remove_file(&path) {
                                Ok(true) => {
                                    needs_commit = true;
                                    info!("Removed file (watcher): {:?}", path);
                                }
                                Err(e) => error!("Watcher error removing {:?}: {}", path, e),
                                _ => {}
                            }
                        }

                        // Then process indexes
                        let mut docs_to_add = Vec::new();
                        let mut meta_to_update = Vec::new();

                        for path in index_paths {
                            match Self::reindex_single_file(&path, &metadata_db_for_task).await {
                                Ok(Some((doc, modified, size, hash))) => {
                                    meta_to_update.push((doc.path.clone(), modified, size, hash));
                                    docs_to_add.push((doc, modified, size));
                                }
                                Ok(None) => {} // Skipped or does not exist
                                Err(e) => {
                                    error!("Watcher error indexing {:?}: {}", path, e);
                                }
                            }
                        }

                        if !docs_to_add.is_empty() {
                            if let Err(e) = indexer_for_task.add_documents_batch(&docs_to_add) {
                                error!("Watcher error batch adding to index: {}", e);
                            }
                            if let Err(e) = metadata_db_for_task.batch_update_metadata(&meta_to_update) {
                                error!("Watcher error batch updating DB: {}", e);
                            } else {
                                info!("Re-indexed {} files (watcher)", docs_to_add.len());
                            }
                            needs_commit = true;
                        }

                        if needs_commit {
                            if let Err(e) = indexer_for_task.commit() {
                                error!("Watcher failed to commit index: {}", e);
                            } else {
                                indexer_for_task.invalidate_cache();
                                info!("Watcher committed changes");
                            }
                        }
                    }
                }
            }
        });

        Self {
            watcher: None,
            _indexer: indexer,
            _metadata_db: metadata_db,
            _runtime_handle: runtime_handle,
            event_buffer: buffer,
            external_tx,
        }
    }

    /// Get a sender to push external events (like USN Journal) into the watcher
    pub fn event_tx(&self) -> mpsc::Sender<(PathBuf, WatcherAction)> {
        self.external_tx.clone()
    }

    /// Update the list of watched directories
    pub fn update_watch_list(&mut self, dirs: Vec<String>) -> Result<()> {
        self.watcher = None;

        if dirs.is_empty() {
            return Ok(());
        }

        let buffer_clone = self.event_buffer.clone();

        let mut watcher = notify::recommended_watcher(move |res: notify::Result<Event>| {
            if let Ok(event) = res {
                let mut guard = match buffer_clone.lock() {
                    Ok(g) => g,
                    Err(e) => {
                        error!("Watcher lock poisoned: {}", e);
                        return;
                    }
                };

                match event.kind {
                    EventKind::Modify(_) | EventKind::Create(_) | EventKind::Remove(_) => {
                        for path in &event.paths {
                            match event.kind {
                                EventKind::Remove(_) => {
                                    guard.insert(path.clone(), WatcherAction::Remove);
                                }
                                _ => {
                                    // Don't check if file exists here - let reindex_single_file handle missing files
                                    guard.insert(path.clone(), WatcherAction::Index);
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
        })
        .map_err(|e| FlashError::Io(std::io::Error::other(e)))?;

        for dir in dirs {
            let path = Path::new(&dir);
            if path.exists() {
                watcher
                    .watch(path, RecursiveMode::Recursive)
                    .map_err(|e| FlashError::Io(std::io::Error::other(e)))?;
            }
        }

        self.watcher = Some(watcher);
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

        let metadata = match std::fs::metadata(path) {
            Ok(m) => m,
            Err(_) => return Ok(None), // Ignore if cannot read metadata
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
            .map_err(|e| FlashError::parse(path, format!("Parse task failed: {}", e)))?;

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

        let mut watcher = WatcherManager::new(indexer, metadata);

        // Add a directory to watch
        let watch_dir = temp.path().join("watch_me");
        fs::create_dir(&watch_dir).unwrap();

        assert!(watcher
            .update_watch_list(vec![watch_dir.to_string_lossy().to_string()])
            .is_ok());
        assert!(watcher.watcher.is_some());

        // Empty list should remove watcher
        assert!(watcher.update_watch_list(vec![]).is_ok());
        assert!(watcher.watcher.is_none());
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
