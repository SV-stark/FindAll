use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use notify::{Watcher, RecursiveMode, Event, EventKind, RecommendedWatcher};
use crate::error::{FlashError, Result};
use crate::indexer::IndexManager;
use crate::metadata::MetadataDb;
use crate::parsers::parse_file;
use blake3;
use tracing::{info, warn, error};

#[derive(Debug, Clone, PartialEq, Eq)]
enum WatcherAction {
    Index,
    Remove,
}

/// Manages active file system watching with debouncing
pub struct WatcherManager {
    watcher: Option<RecommendedWatcher>,
    indexer: Arc<IndexManager>,
    metadata_db: Arc<MetadataDb>,
    runtime_handle: tokio::runtime::Handle,
    // Buffer for pending events: Map<Path, Action>
    event_buffer: Arc<Mutex<HashMap<PathBuf, WatcherAction>>>,
}

impl WatcherManager {
    pub fn new(
        indexer: Arc<IndexManager>,
        metadata_db: Arc<MetadataDb>,
    ) -> Self {
        let buffer: Arc<Mutex<HashMap<PathBuf, WatcherAction>>> = Arc::new(Mutex::new(HashMap::new()));
        let buffer_for_task = buffer.clone();
        let indexer_for_task = indexer.clone();
        let metadata_db_for_task = metadata_db.clone();
        let runtime_handle = tokio::runtime::Handle::current();
        
        // Spawn background processor for debounced events
        runtime_handle.spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_millis(1000)).await; // Check every 1s
                
                let events = {
                    let mut guard = buffer_for_task.lock().map_err(|e| {
                        error!("Watcher lock poisoned: {}", e);
                    })?;
                    if guard.is_empty() {
                        continue;
                    }
                    // Take all events
                    let events: HashMap<PathBuf, WatcherAction> = std::mem::take(&mut *guard);
                    events
                };
                
                let mut needs_commit = false;
                
                for (path, action) in events {
                    let res = match action {
                        WatcherAction::Index => Self::reindex_single_file(&path, &indexer_for_task, &metadata_db_for_task).await,
                        WatcherAction::Remove => Self::remove_single_file(&path, &indexer_for_task, &metadata_db_for_task).await,
                    };
                    
                    if let Ok(processed) = res {
                        if processed {
                            needs_commit = true;
                        }
                    } else if let Err(e) = res {
                        error!("Watcher error processing {:?}: {}", path, e);
                    }
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
        });

        Self {
            watcher: None,
            indexer,
            metadata_db,
            runtime_handle,
            event_buffer: buffer,
        }
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
                            if path.is_file() {
                                match event.kind {
                                    EventKind::Remove(_) => {
                                        guard.insert(path.clone(), WatcherAction::Remove);
                                    }
                                    _ => {
                                        guard.insert(path.clone(), WatcherAction::Remove);
                                        guard.insert(path.clone(), WatcherAction::Index);
                                    }
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
        }).map_err(|e| FlashError::Io(std::io::Error::new(std::io::ErrorKind::Other, e)))?;

        for dir in dirs {
            let path = Path::new(&dir);
            if path.exists() {
                watcher.watch(path, RecursiveMode::Recursive)
                    .map_err(|e| FlashError::Io(std::io::Error::new(std::io::ErrorKind::Other, e)))?;
            }
        }

        self.watcher = Some(watcher);
        Ok(())
    }
    
    // Returns true if changes were made (requiring commit)
    async fn reindex_single_file(
        path: &Path,
        indexer: &Arc<IndexManager>,
        metadata_db: &Arc<MetadataDb>,
    ) -> Result<bool> {
        if !path.exists() {
             return Self::remove_single_file(path, indexer, metadata_db).await;
        }

        let metadata = match std::fs::metadata(path) {
            Ok(m) => m,
            Err(_) => return Ok(false), // Ignore if cannot read metadata
        };
        
        let modified = metadata.modified()
            .unwrap_or(std::time::SystemTime::UNIX_EPOCH)
            .duration_since(std::time::SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let size = metadata.len();
        
        // Skip check? If watcher said it changed, it probably did.
        // But checking db saves re-hashing if it was a false alarm.
        if !metadata_db.needs_reindex(path, modified, size).unwrap_or(true) {
            return Ok(false);
        }
        
        let parsed = match parse_file(path) {
            Ok(p) => p,
            Err(e) => {
                warn!("Failed to parse file {:?}: {}", path, e);
                return Ok(false);
            }
        };
        
        let content_hash: [u8; 32] = blake3::hash(parsed.content.as_bytes()).into();
        
        indexer.add_document(&parsed, modified, size)?;
        // NO commit here
        
        metadata_db.update_metadata(path, modified, size, content_hash)?;
        
        info!("Re-indexed file (watcher): {:?}", path);
        
        Ok(true)
    }

    async fn remove_single_file(
        path: &Path,
        indexer: &Arc<IndexManager>,
        metadata_db: &Arc<MetadataDb>,
    ) -> Result<bool> {
        let path_str = path.to_string_lossy();
        indexer.remove_document(&path_str)?;
        // NO commit here
        metadata_db.remove_file(path)?;
        info!("Removed file (watcher): {:?}", path);
        Ok(true)
    }
}
