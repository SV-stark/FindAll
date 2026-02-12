use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, Mutex};
use rayon::prelude::*;
use ignore::WalkBuilder;
use tauri::{AppHandle, Emitter};
use tracing::{error, info, instrument, trace, warn};
use crate::error::Result;
use crate::indexer::IndexManager;
use crate::metadata::MetadataDb;
use crate::parsers::{parse_file, ParsedDocument};
use blake3;

#[derive(Clone, serde::Serialize)]
pub struct ProgressEvent {
    pub total: usize,
    pub processed: usize,
    pub current_file: String,
    pub status: String,
}

/// Document batch for efficient indexing
const BATCH_SIZE: usize = 50;
/// Chunk size for processing files (process this many at a time)
const CHUNK_SIZE: usize = 1000;
/// Maximum time to wait before committing a partial batch
const BATCH_TIMEOUT_MS: u64 = 5000;
/// Progress update frequency (update every N files)
const PROGRESS_UPDATE_INTERVAL: usize = 10;

/// Message sent through channel for indexing
#[derive(Debug)]
struct IndexTask {
    doc: ParsedDocument,
    modified: u64,
    size: u64,
    content_hash: [u8; 32],
}

/// Scans directories and indexes files efficiently using chunked processing
pub struct Scanner {
    indexer: Arc<Mutex<IndexManager>>,
    metadata_db: Arc<MetadataDb>,
    app_handle: AppHandle,
}

impl Scanner {
    pub fn new(
        indexer: Arc<Mutex<IndexManager>>,
        metadata_db: Arc<MetadataDb>,
        app_handle: AppHandle,
    ) -> Self {
        Self {
            indexer,
            metadata_db,
            app_handle,
        }
    }
    
    /// Scan a directory and index all supported files using chunked processing
    /// Prevents deadlocks by processing files in discrete chunks with timeout-based commits
    #[instrument(skip(self, exclude_patterns), fields(root = %root.display()))]
    pub async fn scan_directory(&self, root: PathBuf, exclude_patterns: Vec<String>) -> Result<()> {
        info!("Starting directory scan");
        
        // Build walker with default and custom exclusions
        let mut builder = WalkBuilder::new(&root);
        builder.hidden(false);
        builder.git_ignore(true);
        builder.require_git(false);
        
        let system_excludes = vec![
            ".git", ".svn", ".hg", "node_modules", "target", "bin", "obj", 
            "build", "dist", "__pycache__", "AppData", "Local Settings", 
            "Application Data", "Program Files", "Windows", "$RECYCLE.BIN",
            "System Volume Information", "temp", "tmp", ".vscode", ".idea", ".next"
        ];

        let mut override_builder = ignore::overrides::OverrideBuilder::new(&root);
        for pattern in system_excludes {
            override_builder.add(&format!("!**/{}", pattern)).ok();
        }
        for pattern in exclude_patterns {
            override_builder.add(&format!("!**/{}", pattern)).ok();
        }
        
        let overrides = override_builder.build().expect("Failed to build overrides");
        builder.overrides(overrides);

        // Collect all files first
        let walker = builder.build();
        let files: Vec<PathBuf> = walker
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().map(|ft| ft.is_file()).unwrap_or(false))
            .map(|e| e.path().to_path_buf())
            .collect();
        
        let total_files = files.len();
        info!(total_files = total_files, "Found files to process");
        
        if total_files == 0 {
            warn!("No files found to index");
            let _ = self.app_handle.emit("indexing-progress", ProgressEvent {
                total: 0,
                processed: 0,
                current_file: "No files found".to_string(),
                status: "done".to_string(),
            });
            return Ok(());
        }
        
        let _ = self.app_handle.emit("indexing-progress", ProgressEvent {
            total: total_files,
            processed: 0,
            current_file: "Starting...".to_string(),
            status: "indexing".to_string(),
        });

        let processed_count = Arc::new(AtomicUsize::new(0));
        let skipped_count = Arc::new(AtomicUsize::new(0));
        let last_emitted_progress = Arc::new(AtomicUsize::new(0));
        
        let indexer = self.indexer.clone();
        let metadata_db = self.metadata_db.clone();
        let app_handle = self.app_handle.clone();
        let total_clone = total_files;
        let processed_clone = processed_count.clone();
        let skipped_clone = skipped_count.clone();
        let last_progress_clone = last_emitted_progress.clone();
        
        let (tx, mut rx) = mpsc::channel::<IndexTask>(CHUNK_SIZE);
        
        let consumer = tokio::spawn(async move {
            let mut batch = Vec::with_capacity(BATCH_SIZE);
            let mut metadata_batch = Vec::with_capacity(BATCH_SIZE);
            let mut last_commit = Instant::now();
            let mut total_indexed = 0usize;
            
            loop {
                match tokio::time::timeout(
                    Duration::from_millis(100), 
                    rx.recv()
                ).await {
                    Ok(Some(task)) => {
                        metadata_batch.push((
                            task.doc.path.clone(),
                            task.modified,
                            task.size,
                            task.content_hash,
                        ));
                        batch.push(task);
                        
                        if batch.len() >= BATCH_SIZE {
                            if let Err(e) = Self::commit_batch(&indexer, &metadata_db, &mut batch, &mut metadata_batch).await {
                                eprintln!("Failed to commit batch: {}", e);
                            }
                            total_indexed += batch.len();
                            last_commit = Instant::now();
                            batch.clear();
                            metadata_batch.clear();
                        }
                    }
                    Ok(None) => {
                        if !batch.is_empty() {
                            if let Err(e) = Self::commit_batch(&indexer, &metadata_db, &mut batch, &mut metadata_batch).await {
                                eprintln!("Failed to commit final batch: {}", e);
                            }
                            total_indexed += batch.len();
                        }
                        break;
                    }
                    Err(_) => {
                        if !batch.is_empty() && last_commit.elapsed().as_millis() > BATCH_TIMEOUT_MS as u128 {
                            if let Err(e) = Self::commit_batch(&indexer, &metadata_db, &mut batch, &mut metadata_batch).await {
                                eprintln!("Failed to commit timed batch: {}", e);
                            }
                            total_indexed += batch.len();
                            last_commit = Instant::now();
                            batch.clear();
                            metadata_batch.clear();
                        }
                        
                        let processed = processed_clone.load(Ordering::Relaxed);
                        let skipped = skipped_clone.load(Ordering::Relaxed);
                        let last_emitted = last_progress_clone.load(Ordering::Relaxed);
                        
                        if processed + skipped >= last_emitted + PROGRESS_UPDATE_INTERVAL {
                            let _ = app_handle.emit("indexing-progress", ProgressEvent {
                                total: total_clone,
                                processed: processed + skipped,
                                current_file: format!("{} files processed", processed + skipped),
                                status: "indexing".to_string(),
                            });
                            last_progress_clone.store(processed + skipped, Ordering::Relaxed);
                        }
                    }
                }
            }
            
            (total_indexed, skipped_clone.load(Ordering::Relaxed))
        });

        let metadata_db = self.metadata_db.clone();
        let processed_for_producer = processed_count.clone();
        let skipped_for_producer = skipped_count.clone();
        
        for chunk in files.chunks(CHUNK_SIZE) {
            let chunk_tasks: Vec<Option<IndexTask>> = chunk
                .par_iter()
                .map(|path| {
                    Self::process_file(path, &metadata_db)
                })
                .collect();
            
            for task in chunk_tasks {
                match task {
                    Some(t) => {
                        if let Err(e) = tx.send(t).await {
                            eprintln!("Failed to send task to channel: {}", e);
                            break;
                        }
                        processed_for_producer.fetch_add(1, Ordering::Relaxed);
                    }
                    None => {
                        skipped_for_producer.fetch_add(1, Ordering::Relaxed);
                    }
                }
            }
            
            tokio::task::yield_now().await;
        }
        
        drop(tx);
        
        let (indexed_count, skipped) = match tokio::time::timeout(
            Duration::from_secs(300),
            consumer
        ).await {
            Ok(result) => result.unwrap_or((0, 0)),
            Err(_) => {
                error!("Consumer task timed out after 5 minutes");
                (processed_count.load(Ordering::Relaxed), skipped_count.load(Ordering::Relaxed))
            }
        };
        
        let _ = self.app_handle.emit("indexing-progress", ProgressEvent {
            total: total_files,
            processed: indexed_count + skipped,
            current_file: "Completed".to_string(),
            status: "done".to_string(),
        });
        
        info!(
            indexed = indexed_count,
            skipped = skipped,
            total = total_files,
            "Indexing completed"
        );
        
        Ok(())
    }
    
    #[instrument(skip(indexer, metadata_db, batch, metadata_batch), fields(batch_size = batch.len()))]
    async fn commit_batch(
        indexer: &Arc<Mutex<IndexManager>>,
        metadata_db: &Arc<MetadataDb>,
        batch: &mut Vec<IndexTask>,
        metadata_batch: &mut Vec<(String, u64, u64, [u8; 32])>,
    ) -> Result<()> {
        if batch.is_empty() {
            return Ok(());
        }
        
        let batch_len = batch.len();
        let indexer = indexer.lock().await;
        
        for task in batch.iter() {
            if let Err(e) = indexer.add_document(&task.doc, task.modified, task.size) {
                error!(path = %task.doc.path, error = %e, "Failed to add document");
            }
        }
        
        indexer.commit()?;
        
        if let Err(e) = metadata_db.batch_update_metadata(metadata_batch) {
            error!(error = %e, "Failed to batch update metadata");
        }
        
        info!(batch_size = batch_len, "Batch committed successfully");
        
        Ok(())
    }
    
    #[instrument(skip(metadata_db), fields(path = %path.display()))]
    fn process_file(
        path: &Path,
        metadata_db: &Arc<MetadataDb>,
    ) -> Option<IndexTask> {
        let metadata = std::fs::metadata(path).ok()?;
        let modified = metadata.modified().ok()?
            .duration_since(std::time::SystemTime::UNIX_EPOCH)
            .ok()?
            .as_secs();
        let size = metadata.len();
        
        match metadata_db.needs_reindex(path, modified, size) {
            Ok(false) => return None,
            Ok(true) => {}
            Err(e) => {
                error!(error = %e, "Error checking metadata");
                return None;
            }
        }
        
        let parsed = match parse_file(path) {
            Ok(doc) => doc,
            Err(e) => {
                warn!(error = %e, "Failed to parse file");
                return None;
            }
        };
        
        let content_hash = blake3::hash(parsed.content.as_bytes()).into();
        
        Some(IndexTask {
            doc: parsed,
            modified,
            size,
            content_hash,
        })
    }
}
