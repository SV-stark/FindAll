use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use notify::{Watcher, RecursiveMode, Event, EventKind, RecommendedWatcher};
use crate::error::{FlashError, Result};
use crate::indexer::IndexManager;
use crate::metadata::MetadataDb;
use crate::parsers::parse_file;
use blake3;

/// Manages active file system watching
pub struct WatcherManager {
    watcher: Option<RecommendedWatcher>,
    indexer: Arc<IndexManager>,
    metadata_db: Arc<MetadataDb>,
    runtime_handle: tokio::runtime::Handle,
}

impl WatcherManager {
    pub fn new(
        indexer: Arc<IndexManager>,
        metadata_db: Arc<MetadataDb>,
    ) -> Self {
        Self {
            watcher: None,
            indexer,
            metadata_db,
            runtime_handle: tokio::runtime::Handle::current(),
        }
    }

    /// Update the list of watched directories
    pub fn update_watch_list(&mut self, dirs: Vec<String>) -> Result<()> {
        self.watcher = None;

        if dirs.is_empty() {
            return Ok(());
        }

        let indexer = self.indexer.clone();
        let metadata_db = self.metadata_db.clone();
        let rt_handle = self.runtime_handle.clone();

        let mut watcher = notify::recommended_watcher(move |res: notify::Result<Event>| {
            if let Ok(event) = res {
                let idx = indexer.clone();
                let db = metadata_db.clone();
                let rt = rt_handle.clone();
                
                // Spawn on the captured runtime handle to avoid "no reactor running" panic
                rt.spawn(async move {
                    match event.kind {
                        EventKind::Modify(_) | EventKind::Create(_) => {
                            for path in event.paths {
                                if path.is_file() {
                                    // Small delay to let file writes finish
                                    tokio::time::sleep(Duration::from_millis(500)).await;
                                    
                                    if let Err(e) = Self::reindex_single_file(&path, &idx, &db).await {
                                        eprintln!("Failed to reindex file {:?}: {}", path, e);
                                    }
                                }
                            }
                        }
                        EventKind::Remove(_) => {
                            for path in event.paths {
                                if let Err(e) = Self::remove_single_file(&path, &idx, &db).await {
                                    eprintln!("Failed to remove file {:?}: {}", path, e);
                                }
                            }
                        }
                        _ => {}
                    }
                });
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
    
    async fn reindex_single_file(
        path: &Path,
        indexer: &Arc<IndexManager>,
        metadata_db: &Arc<MetadataDb>,
    ) -> Result<()> {
        // ... (rest of reindex logic is fine, but re-implementing to be safe/clean)
        // Check if file still exists
        if !path.exists() {
             return Self::remove_single_file(path, indexer, metadata_db).await;
        }

        let metadata = std::fs::metadata(path)
            .map_err(|e| FlashError::Io(e))?;
        
        let modified = metadata.modified()
            .map_err(|e| FlashError::Io(e))?
            .duration_since(std::time::SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let size = metadata.len();
        
        if !metadata_db.needs_reindex(path, modified, size)? {
            return Ok(());
        }
        
        // Handle parsing errors gracefully (don't crash watcher)
        let parsed = match parse_file(path) {
            Ok(p) => p,
            Err(e) => {
                eprintln!("Failed to parse file {:?}: {}", path, e);
                return Ok(());
            }
        };
        
        let content_hash: [u8; 32] = blake3::hash(parsed.content.as_bytes()).into();
        
        indexer.add_document(&parsed, modified, size)?;
        indexer.commit()?;
        
        metadata_db.update_metadata(path, modified, size, content_hash)?;
        
        println!("Re-indexed file: {:?}", path);
        
        Ok(())
    }

    async fn remove_single_file(
        path: &Path,
        indexer: &Arc<IndexManager>,
        metadata_db: &Arc<MetadataDb>,
    ) -> Result<()> {
        let path_str = path.to_string_lossy();
        indexer.remove_document(&path_str)?;
        indexer.commit()?;
        metadata_db.remove_file(path)?;
        println!("Removed file from index: {:?}", path);
        Ok(())
    }
}
