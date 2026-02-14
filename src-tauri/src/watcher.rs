use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use notify::{Watcher, RecursiveMode, Event, EventKind, RecommendedWatcher};
use tauri::{AppHandle, Emitter};
use crate::error::{FlashError, Result};
use crate::indexer::IndexManager;
use crate::metadata::MetadataDb;
use crate::parsers::parse_file;
use blake3;

/// Manages active file system watching
pub struct WatcherManager {
    watcher: Option<RecommendedWatcher>,
    app_handle: AppHandle,
    indexer: Arc<IndexManager>,
    metadata_db: Arc<MetadataDb>,
}

impl WatcherManager {
    pub fn new(
        app_handle: AppHandle,
        indexer: Arc<IndexManager>,
        metadata_db: Arc<MetadataDb>,
    ) -> Self {
        Self {
            watcher: None,
            app_handle,
            indexer,
            metadata_db,
        }
    }

    /// Update the list of watched directories
    pub fn update_watch_list(&mut self, dirs: Vec<String>) -> Result<()> {
        self.watcher = None;

        if dirs.is_empty() {
            return Ok(());
        }

        let app_handle = self.app_handle.clone();
        let indexer = self.indexer.clone();
        let metadata_db = self.metadata_db.clone();

        let mut watcher = notify::recommended_watcher(move |res: notify::Result<Event>| {
            if let Ok(event) = res {
                match event.kind {
                    EventKind::Modify(_) | EventKind::Create(_) => {
                        for path in event.paths {
                            if path.is_file() {
                                let app = app_handle.clone();
                                let idx = indexer.clone();
                                let db = metadata_db.clone();
                                
                                tauri::async_runtime::spawn(async move {
                                    tokio::time::sleep(Duration::from_millis(500)).await;
                                    
                                    if let Err(e) = Self::reindex_single_file(&path, &idx, &db).await {
                                        eprintln!("Failed to reindex file {:?}: {}", path, e);
                                    } else {
                                        let _ = app.emit("file-updated", path.to_string_lossy().to_string());
                                    }
                                });
                            }
                        }
                    }
                    EventKind::Remove(_) => {
                        for path in event.paths {
                            if path.is_file() {
                                println!("File removed: {:?}", path);
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
    
    async fn reindex_single_file(
        path: &Path,
        indexer: &Arc<IndexManager>,
        metadata_db: &Arc<MetadataDb>,
    ) -> Result<()> {
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
        
        let parsed = parse_file(path)
            .map_err(|e| FlashError::parse(path, format!("Failed to parse file: {}", e)))?;
        
        let content_hash: [u8; 32] = blake3::hash(parsed.content.as_bytes()).into();
        
        indexer.add_document(&parsed, modified, size)?;
        indexer.commit()?;
        
        metadata_db.update_metadata(path, modified, size, content_hash)?;
        
        println!("Re-indexed file: {:?}", path);
        
        Ok(())
    }
}
