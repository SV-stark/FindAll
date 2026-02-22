use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use tokio::sync::mpsc;
use rayon::prelude::*;
use ignore::WalkBuilder;
use tracing::{info, instrument, warn};
use crate::error::Result;
use crate::indexer::IndexManager;
use crate::metadata::MetadataDb;
use crate::parsers::{parse_file, ParsedDocument};
use blake3;
use std::time::Instant;
use std::sync::atomic::Ordering;


#[derive(Clone, Debug, serde::Serialize)]
pub enum ProgressType {
    Content,
    Filename,
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct ProgressEvent {
    pub total: usize,
    pub processed: usize,
    pub current_file: String,
    pub status: String,
    pub ptype: ProgressType,
    pub files_per_second: f64,
    pub eta_seconds: u64,
    pub current_folder: String,
}

const BATCH_SIZE: usize = 50;

#[derive(Debug)]
struct IndexTask {
    doc: ParsedDocument,
    modified: u64,
    size: u64,
    content_hash: [u8; 32],
}

pub struct Scanner {
    indexer: Arc<IndexManager>,
    metadata_db: Arc<MetadataDb>,
    filename_index: Option<Arc<crate::indexer::filename_index::FilenameIndex>>,
    progress_tx: Option<mpsc::Sender<ProgressEvent>>,
}

impl Scanner {
    pub fn new(
        indexer: Arc<IndexManager>,
        metadata_db: Arc<MetadataDb>,
        filename_index: Option<Arc<crate::indexer::filename_index::FilenameIndex>>,
        progress_tx: Option<mpsc::Sender<ProgressEvent>>,
    ) -> Self {
        Self {
            indexer,
            metadata_db,
            filename_index,
            progress_tx,
        }
    }
    
    #[instrument(skip(self, exclude_patterns), fields(root = %root.display()))]
    pub async fn scan_directory(&self, root: PathBuf, exclude_patterns: Vec<String>) -> Result<()> {
        info!("Starting directory scan for {}", root.display());
        
        // P3/P4: Run blocking WalkBuilder in a separate thread to avoid blocking Tokio runtime
        // and allow pipelined consumption.
        let (path_tx, path_rx): (std::sync::mpsc::Sender<PathBuf>, std::sync::mpsc::Receiver<PathBuf>) = std::sync::mpsc::channel();
        let total = Arc::new(AtomicUsize::new(0));
        let root_clone = root.clone();
        let tx_clone = self.progress_tx.clone();
        let total_clone = total.clone();
        
        let walker_handle = tokio::task::spawn_blocking(move || {
            let mut builder = WalkBuilder::new(&root_clone);
            // ... (keep logic same) ...
            
            // Add overrides
            let mut override_builder = ignore::overrides::OverrideBuilder::new(&root_clone);
             for pattern in &exclude_patterns {
                let ignore_pattern = format!("!{}", pattern);
                 if let Err(e) = override_builder.add(&ignore_pattern) {
                    warn!("Invalid exclude pattern '{}': {}", pattern, e);
                }
            }
            if let Ok(overrides) = override_builder.build() {
                builder.overrides(overrides);
            }
    
            builder
                .follow_links(true)
                .standard_filters(false);
            
            // --- Stage 1: Filename Indexing ---
            info!("Stage 1: Filename Indexing");
            let builder = builder.build_parallel();
            builder.run(|| {
                let path_tx = path_tx.clone();
                let tx = tx_clone.clone();
                let total = total_clone.clone();
                Box::new(move |entry| {
                    if let Ok(entry) = entry {
                        if entry.file_type().map(|ft| ft.is_file()).unwrap_or(false) {
                            let path = entry.path().to_path_buf();
                            let _ = path_tx.send(path);
                            let count = total.fetch_add(1, Ordering::Relaxed);
                            
                            // Send progress update periodically
                            if count % 100 == 0 {
                                if let Some(tx) = &tx {
                                    let _ = tx.try_send(ProgressEvent {
                                        ptype: ProgressType::Filename,
                                        current_file: entry.file_name().to_string_lossy().to_string(),
                                        current_folder: "".to_string(),
                                        processed: count,
                                        total: 0, // Unknown
                                        status: "Scanning filenames...".to_string(),
                                        eta_seconds: 0,
                                        files_per_second: 0.0,
                                    });
                                }
                            }
                        }
                    }
                    ignore::WalkState::Continue
                })
            });
            
            // Final update for stage 1
            let final_count = total_clone.load(Ordering::Relaxed);
             if let Some(tx) = &tx_clone {
                 let _ = tx.try_send(ProgressEvent {
                    ptype: ProgressType::Filename,
                    current_file: "".to_string(),
                    current_folder: "".to_string(),
                    processed: final_count,
                    total: final_count,
                    status: "Filename scan complete".to_string(),
                    eta_seconds: 0,
                    files_per_second: 0.0,
                });
            }
        });

        // --- Stage 2: Content Indexing (Batched) ---
        // Parse files in parallel via Rayon, collect results via mpsc,
        // then batch-write to the index and metadata DB.
        let (task_tx, task_rx) = std::sync::mpsc::sync_channel::<IndexTask>(BATCH_SIZE * 2);
        let metadata_db_for_parser = self.metadata_db.clone();
        let metadata_db_for_writer = self.metadata_db.clone();
        let indexer_clone = self.indexer.clone();
        let filename_index_clone = self.filename_index.clone();
        let progress_tx_clone = self.progress_tx.clone();
        let total_files = total.clone();

        // --- Stage 2a: Parallel parsing (Rayon) → sends IndexTask into channel ---
        let parser_handle = tokio::task::spawn_blocking(move || {
            info!("Stage 2a: Parallel Parsing");
            path_rx.into_iter().par_bridge().for_each(|path: PathBuf| {
                if let Some(task) = Scanner::process_file(&path, &metadata_db_for_parser) {
                    let _ = task_tx.send(task);
                }
            });
            // task_tx is dropped here, closing the channel
        });

        // --- Stage 2b: Sequential batch writer (single thread) ---
        let writer_handle = tokio::task::spawn_blocking(move || {
            info!("Stage 2b: Batch Writing");
            let start = Instant::now();
            let mut doc_batch: Vec<(crate::parsers::ParsedDocument, u64, u64)> = Vec::with_capacity(BATCH_SIZE);
            let mut meta_batch: Vec<(String, u64, u64, [u8; 32])> = Vec::with_capacity(BATCH_SIZE);
            let mut processed: usize = 0;

            for task in task_rx.iter() {
                // Add to filename index
                if let Some(f_index) = &filename_index_clone {
                    let path = std::path::Path::new(&task.doc.path);
                    if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                        let _ = f_index.add_file(&task.doc.path, name);
                    }
                }

                // Take ownership instead of cloning
                doc_batch.push((task.doc, task.modified, task.size));
                meta_batch.push((task.doc.path, task.modified, task.size, task.content_hash));
                processed += 1;

                // Flush batch when full
                if doc_batch.len() >= BATCH_SIZE {
                    let _ = indexer_clone.add_documents_batch(&doc_batch);
                    let _ = indexer_clone.commit();
                    indexer_clone.invalidate_cache();
                    let _ = metadata_db_for_writer.batch_update_metadata(&meta_batch);
                    doc_batch.clear();
                    meta_batch.clear();
                }

                // Progress update
                if processed % 10 == 0 {
                    let current_total = total_files.load(Ordering::Relaxed);
                    let elapsed = start.elapsed().as_secs_f64();
                    let rate = if elapsed > 0.0 { processed as f64 / elapsed } else { 0.0 };
                    if let Some(tx) = &progress_tx_clone {
                         let _ = tx.try_send(ProgressEvent {
                            ptype: ProgressType::Content,
                            current_file: "".to_string(),
                            current_folder: "".to_string(),
                            processed,
                            total: current_total,
                            status: "Indexing contents...".to_string(),
                            eta_seconds: if rate > 0.0 { (current_total.saturating_sub(processed) as f64 / rate) as u64 } else { 0 },
                            files_per_second: rate,
                        });
                    }
                }
            }

            // Flush remaining items (B1: always commit at end)
            if !doc_batch.is_empty() {
                let _ = indexer_clone.add_documents_batch(&doc_batch);
                let _ = indexer_clone.commit();
                indexer_clone.invalidate_cache();
                let _ = metadata_db_for_writer.batch_update_metadata(&meta_batch);
            }

            // Final progress
            if let Some(tx) = &progress_tx_clone {
                let _ = tx.try_send(ProgressEvent {
                    ptype: ProgressType::Content,
                    current_file: "".to_string(),
                    current_folder: "".to_string(),
                    processed,
                    total: processed,
                    status: "All files indexed".to_string(),
                    eta_seconds: 0,
                    files_per_second: 0.0,
                });
            }

            info!("Indexed {} files in {:.2}s", processed, start.elapsed().as_secs_f64());
        });

        // Wait for all stages to complete
        let _ = walker_handle.await.map_err(|e| crate::error::FlashError::index(format!("Walk task failed: {}", e)))?;
        let _ = parser_handle.await.map_err(|e| crate::error::FlashError::index(format!("Parse task failed: {}", e)))?;
        let _ = writer_handle.await.map_err(|e| crate::error::FlashError::index(format!("Write task failed: {}", e)))?;

        // Commit filename index to disk
        if let Some(f_index) = &self.filename_index {
            let _ = f_index.commit();
        }

        Ok(())
    }

    
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
        
        if !metadata_db.needs_reindex(path, modified, size).unwrap_or(true) {
            return None;
        }
        
        let parsed = parse_file(path).ok()?;
        let content_hash = blake3::hash(parsed.content.as_bytes()).into();
        
        Some(IndexTask {
            doc: parsed,
            modified,
            size,
            content_hash,
        })
    }
}
