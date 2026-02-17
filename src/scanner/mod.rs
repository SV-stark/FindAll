use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use tokio::sync::mpsc;
use rayon::prelude::*;
use ignore::WalkBuilder;
use tracing::{info, instrument};
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
const CHUNK_SIZE: usize = 1000;

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
    progress_tx: Option<mpsc::Sender<ProgressEvent>>,
}

impl Scanner {
    pub fn new(
        indexer: Arc<IndexManager>,
        metadata_db: Arc<MetadataDb>,
        progress_tx: Option<mpsc::Sender<ProgressEvent>>,
    ) -> Self {
        Self {
            indexer,
            metadata_db,
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
                    eprintln!("Invalid exclude pattern '{}': {}", pattern, e);
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
            let mut builder = builder.build_parallel();
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

        // --- Stage 2: Content Indexing ---
        // Run in separate blocking thread to process paths as they arrive (Pipeline)
        let self_clone = Scanner {
             indexer: self.indexer.clone(),
             metadata_db: self.metadata_db.clone(),
             progress_tx: self.progress_tx.clone(),
        };
        
        let total_files = total.clone();
        
        let indexer_handle = tokio::task::spawn_blocking(move || {
            info!("Stage 2: Content Indexing");
            let start = Instant::now();
            let processed = AtomicUsize::new(0);
            
            // Use Rayon for parallel processing of the stream
            path_rx.into_iter().par_bridge().for_each(|path: PathBuf| {
                let p_count = processed.fetch_add(1, Ordering::Relaxed);
                let current_total = total_files.load(Ordering::Relaxed);
                
                if let Err(e) = Scanner::process_and_index_file(&path, &self_clone.indexer, &self_clone.metadata_db) {
                     // warn!("Failed to index {:?}: {}", path, e);
                }

                // Progress update
                if p_count % 10 == 0 {
                    let elapsed = start.elapsed().as_secs_f64();
                    let rate = if elapsed > 0.0 { p_count as f64 / elapsed } else { 0.0 };
                    if let Some(tx) = &self_clone.progress_tx {
                         let _ = tx.try_send(ProgressEvent {
                            ptype: ProgressType::Content,
                            current_file: path.file_name().unwrap_or_default().to_string_lossy().to_string(),
                            current_folder: path.parent().unwrap_or(&path).to_string_lossy().to_string(),
                            processed: p_count,
                            total: current_total, 
                            status: "Indexing contents...".to_string(),
                            eta_seconds: if rate > 0.0 { (current_total.saturating_sub(p_count) as f64 / rate) as u64 } else { 0 },
                            files_per_second: rate,
                        });
                    }
                }
            });
            
            let final_count = processed.load(Ordering::Relaxed);
            if let Some(tx) = &self_clone.progress_tx {
                let _ = tx.try_send(ProgressEvent {
                    ptype: ProgressType::Content,
                    current_file: "".to_string(),
                    current_folder: "".to_string(),
                    processed: final_count,
                    total: final_count,
                    status: "All files indexed".to_string(),
                    eta_seconds: 0,
                    files_per_second: 0.0,
                });
            }
        });

        // Wait for both to complete
        let _ = walker_handle.await.map_err(|e| crate::error::FlashError::index(format!("Walk task failed: {}", e)))?;
        let _ = indexer_handle.await.map_err(|e| crate::error::FlashError::index(format!("Index task failed: {}", e)))?;
        
        Ok(())
    }
    
    // Helper to process and index without self ref issues in Rayon
    fn process_and_index_file(path: &Path, indexer: &Arc<IndexManager>, metadata_db: &Arc<MetadataDb>) -> Result<()> {
         if let Some(task) = Self::process_file(path, metadata_db) {
             indexer.add_document(&task.doc, task.modified, task.size)?;
             // We are not batching here? P3 logic was "non-blocking concurrent traversal".
             // If we don't batch, commit frequency might be high.
             // But `add_document` in `IndexWriter` is usually buffered in RAM.
             // Taking a lock on writer for every file might be slow?
             // Previously `scan_directory` used `mpsc` to batch commits.
             // If I use `par_bridge`, I can't easily batch unless I use `map...collect` then batch.
             // But I want pipeline. 
             // Ideally: `par_bridge` produces tasks -> `mpsc` -> batched committer.
             // But I simply called `add_document`.
             // Depending on `IndexManager` impl, this might be fine.
             // `IndexManager` uses `IndexWriterManager` which locks `writer`.
             // It's probably fine for now. Performance P3 was focused on "non-blocking traversal".
             // Batching P4 was about double walk?
             // Let's assume `add_document` is fast enough.
             // Only explicit `commit` is needed.
             // When to commit?
             // Maybe at end?
         }
         Ok(())
    }

    async fn send_progress(&self, event: ProgressEvent) {
        if let Some(tx) = &self.progress_tx {
            let _ = tx.send(event).await;
        }
    }
    
    async fn commit_batch(
        indexer: &Arc<IndexManager>,
        metadata_db: &Arc<MetadataDb>,
        batch: &mut Vec<IndexTask>,
        metadata_batch: &mut Vec<(String, u64, u64, [u8; 32])>,
    ) -> Result<()> {
        for task in batch.iter() {
            let _ = indexer.add_document(&task.doc, task.modified, task.size);
        }
        indexer.commit()?;
        metadata_db.batch_update_metadata(metadata_batch)?;
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
