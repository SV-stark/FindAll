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
    
    #[instrument(skip(self, _exclude_patterns), fields(root = %root.display()))]
    pub async fn scan_directory(&self, root: PathBuf, _exclude_patterns: Vec<String>) -> Result<()> {
        info!("Starting directory scan");
        
        let mut builder = WalkBuilder::new(&root);
        builder.hidden(false);
        builder.git_ignore(true);
        builder.require_git(false);
        
        // --- Stage 1: Filename Indexing ---
        info!("Stage 1: Filename Indexing");
        let walker = builder.build();
        let mut files: Vec<PathBuf> = Vec::new();
        
        for (i, entry) in walker.enumerate() {
            if let Ok(e) = entry {
                if e.file_type().map(|ft| ft.is_file()).unwrap_or(false) {
                    files.push(e.path().to_path_buf());
                    
                    if i % 100 == 0 {
                        self.send_progress(ProgressEvent {
                            total: 0, // Unknown total during walk
                            processed: i,
                            current_file: e.path().display().to_string(),
                            status: "Scanning filenames...".to_string(),
                            ptype: ProgressType::Filename,
                        }).await;
                    }
                }
            }
        }
        
        let total_files = files.len();
        self.send_progress(ProgressEvent {
            total: total_files,
            processed: total_files,
            current_file: "Scan complete".to_string(),
            status: "Filenames indexed".to_string(),
            ptype: ProgressType::Filename,
        }).await;

        if total_files == 0 {
            return Ok(());
        }
        
        // --- Stage 2: Content Indexing ---
        info!("Stage 2: Content Indexing");
        let _processed_count = Arc::new(AtomicUsize::new(0));
        let indexer = self.indexer.clone();
        let metadata_db = self.metadata_db.clone();
        
        let (tx, mut rx) = mpsc::channel::<IndexTask>(CHUNK_SIZE);
        
        let indexer_for_consumer = indexer.clone();
        let metadata_db_for_consumer = metadata_db.clone();
        
        let consumer = tokio::spawn(async move {
            let mut batch = Vec::with_capacity(BATCH_SIZE);
            let mut metadata_batch = Vec::with_capacity(BATCH_SIZE);
            
            while let Some(task) = rx.recv().await {
                metadata_batch.push((
                    task.doc.path.clone(),
                    task.modified,
                    task.size,
                    task.content_hash,
                ));
                batch.push(task);
                
                if batch.len() >= BATCH_SIZE {
                    let _ = Self::commit_batch(&indexer_for_consumer, &metadata_db_for_consumer, &mut batch, &mut metadata_batch).await;
                    batch.clear();
                    metadata_batch.clear();
                }
            }
            
            if !batch.is_empty() {
                let _ = Self::commit_batch(&indexer_for_consumer, &metadata_db_for_consumer, &mut batch, &mut metadata_batch).await;
            }
        });

        let mut current_processed = 0;
        for chunk in files.chunks(CHUNK_SIZE) {
            let metadata_db_for_chunk = metadata_db.clone();
            let chunk_tasks: Vec<Option<IndexTask>> = chunk
                .par_iter()
                .map(|path| {
                    Self::process_file(path, &metadata_db_for_chunk)
                })
                .collect();
            
            for (i, task) in chunk_tasks.into_iter().enumerate() {
                if let Some(t) = task {
                    if tx.send(t).await.is_err() { break; }
                }
                current_processed += 1;
                
                if current_processed % 50 == 0 {
                    self.send_progress(ProgressEvent {
                        total: total_files,
                        processed: current_processed,
                        current_file: chunk[i].display().to_string(),
                        status: "Indexing contents...".to_string(),
                        ptype: ProgressType::Content,
                    }).await;
                }
            }
        }
        
        drop(tx);
        let _ = consumer.await;
        
        self.send_progress(ProgressEvent {
            total: total_files,
            processed: total_files,
            current_file: "All files indexed".to_string(),
            status: "Idle".to_string(),
            ptype: ProgressType::Content,
        }).await;

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
