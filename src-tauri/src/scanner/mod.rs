use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use tokio::sync::{mpsc, Mutex};
use rayon::prelude::*;
use ignore::WalkBuilder;
use tauri::{AppHandle, Emitter};
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

/// Message sent through channel for indexing
#[derive(Debug)]
struct IndexTask {
    doc: ParsedDocument,
    modified: u64,
    size: u64,
    content_hash: [u8; 32],
}

/// Scans directories and indexes files efficiently
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
    
    /// Scan a directory and index all supported files using streaming
    pub async fn scan_directory(&self, root: PathBuf, exclude_patterns: Vec<String>) -> Result<()> {
        // Build walker with default and custom exclusions
        let mut builder = WalkBuilder::new(&root);
        builder.hidden(false);
        builder.git_ignore(true);
        builder.require_git(false);
        
        // Add default system exclusions
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

        // First pass: collect files (this is fast and memory-light)
        let files: Vec<PathBuf> = builder.build()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().map(|ft| ft.is_file()).unwrap_or(false))
            .map(|e| e.path().to_path_buf())
            .collect();
        
        let total_files = files.len();
        println!("Found {} files to process", total_files);
        
        // Report initial progress
        let _ = self.app_handle.emit("indexing-progress", ProgressEvent {
            total: total_files,
            processed: 0,
            current_file: "".to_string(),
            status: "scanning".to_string(),
        });

        // Create channel for producer-consumer pattern
        let (tx, mut rx) = mpsc::channel::<IndexTask>(BATCH_SIZE * 4);
        let processed_count = Arc::new(AtomicUsize::new(0));
        let processed_clone = processed_count.clone();
        
        // Clone needed for the async task
        let indexer = self.indexer.clone();
        let metadata_db = self.metadata_db.clone();
        let app_handle = self.app_handle.clone();
        let total_clone = total_files;
        
        // Spawn consumer task that batches and commits documents
        let consumer = tokio::spawn(async move {
            let mut batch = Vec::with_capacity(BATCH_SIZE);
            
            while let Some(task) = rx.recv().await {
                batch.push(task);
                
                // Process batch when full
                if batch.len() >= BATCH_SIZE {
                    if let Err(e) = Self::commit_batch(&indexer, &metadata_db, &mut batch).await {
                        eprintln!("Failed to commit batch: {}", e);
                    }
                    
                    // Update progress
                    let processed = processed_clone.fetch_add(batch.len(), Ordering::Relaxed) + batch.len();
                    if processed % 50 == 0 {
                        let _ = app_handle.emit("indexing-progress", ProgressEvent {
                            total: total_clone,
                            processed,
                            current_file: batch.last().map(|t| t.doc.path.clone()).unwrap_or_default(),
                            status: "indexing".to_string(),
                        });
                    }
                    
                    batch.clear();
                }
            }
            
            // Process remaining documents
            if !batch.is_empty() {
                if let Err(e) = Self::commit_batch(&indexer, &metadata_db, &mut batch).await {
                    eprintln!("Failed to commit final batch: {}", e);
                }
                processed_clone.fetch_add(batch.len(), Ordering::Relaxed);
            }
            
            processed_clone.load(Ordering::Relaxed)
        });

        // Producer: Process files in parallel using Rayon
        let metadata_db = self.metadata_db.clone();
        
        files.par_iter().for_each(|path| {
            if let Some(task) = Self::process_file(path, &metadata_db) {
                // Send to consumer - blocking if channel is full
                if let Err(e) = tx.blocking_send(task) {
                    eprintln!("Failed to send task to channel: {}", e);
                }
            }
        });
        
        // Drop sender to signal completion
        drop(tx);
        
        // Wait for consumer to finish
        let indexed_count = consumer.await.unwrap_or(0);
        
        // Final progress report
        let _ = self.app_handle.emit("indexing-progress", ProgressEvent {
            total: total_files,
            processed: indexed_count,
            current_file: "Completed".to_string(),
            status: "done".to_string(),
        });
        
        println!("Successfully indexed {} files (skipped {})", indexed_count, total_files - indexed_count);
        
        Ok(())
    }
    
    /// Commit a batch of documents to the index
    async fn commit_batch(
        indexer: &Arc<Mutex<IndexManager>>,
        metadata_db: &Arc<MetadataDb>,
        batch: &mut Vec<IndexTask>,
    ) -> Result<()> {
        let indexer = indexer.lock().await;
        
        for task in batch.iter() {
            // Add to search index
            if let Err(e) = indexer.add_document(&task.doc, task.modified, task.size) {
                eprintln!("Failed to add document {}: {}", task.doc.path, e);
                continue;
            }
            
            // Update metadata
            let path = PathBuf::from(&task.doc.path);
            if let Err(e) = metadata_db.update_metadata(&path, task.modified, task.size, task.content_hash) {
                eprintln!("Failed to update metadata for {}: {}", task.doc.path, e);
            }
        }
        
        // Single commit for the entire batch
        indexer.commit()?;
        
        Ok(())
    }
    
    /// Process a single file - check if needs reindexing and parse
    fn process_file(
        path: &Path,
        metadata_db: &Arc<MetadataDb>,
    ) -> Option<IndexTask> {
        // Get file metadata
        let metadata = std::fs::metadata(path).ok()?;
        let modified = metadata.modified().ok()?
            .duration_since(std::time::SystemTime::UNIX_EPOCH)
            .ok()?
            .as_secs();
        let size = metadata.len();
        
        // Check if we need to reindex this file
        match metadata_db.needs_reindex(path, modified, size) {
            Ok(false) => return None, // No changes, skip
            Ok(true) => {} // Continue processing
            Err(e) => {
                eprintln!("Error checking metadata for {:?}: {}", path, e);
                return None;
            }
        }
        
        // Parse the file
        let parsed = match parse_file(path) {
            Ok(doc) => doc,
            Err(e) => {
                eprintln!("Failed to parse {:?}: {}", path, e);
                return None;
            }
        };
        
        // Compute content hash for deduplication
        let content_hash = blake3::hash(parsed.content.as_bytes()).into();
        
        Some(IndexTask {
            doc: parsed,
            modified,
            size,
            content_hash,
        })
    }
}
