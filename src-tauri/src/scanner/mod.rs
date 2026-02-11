use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::Mutex;
use walkdir::WalkDir;
use rayon::prelude::*;
use ignore::gitignore::GitignoreBuilder;
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

/// Scans directories and indexes files
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
    
    /// Scan a directory and index all supported files
    pub async fn scan_directory(&self, root: PathBuf) -> Result<()> {
        // Build gitignore matcher with AnyTXT-style default exclusions
        let mut gitignore_builder = GitignoreBuilder::new(&root);
        
        // Version Control
        gitignore_builder.add_line(None, ".git/").ok();
        gitignore_builder.add_line(None, ".svn/").ok();
        gitignore_builder.add_line(None, ".hg/").ok();
        
        // Development / Build Folders
        gitignore_builder.add_line(None, "node_modules/").ok();
        gitignore_builder.add_line(None, "target/").ok();
        gitignore_builder.add_line(None, "bin/").ok();
        gitignore_builder.add_line(None, "obj/").ok();
        gitignore_builder.add_line(None, "build/").ok();
        gitignore_builder.add_line(None, "dist/").ok();
        gitignore_builder.add_line(None, "__pycache__/").ok();
        
        // System / Temp Folders
        gitignore_builder.add_line(None, "AppData/").ok();
        gitignore_builder.add_line(None, "Local Settings/").ok();
        gitignore_builder.add_line(None, "Application Data/").ok();
        gitignore_builder.add_line(None, "Program Files/").ok();
        gitignore_builder.add_line(None, "Program Files (x86)/").ok();
        gitignore_builder.add_line(None, "Windows/").ok();
        gitignore_builder.add_line(None, "$RECYCLE.BIN/").ok();
        gitignore_builder.add_line(None, "System Volume Information/").ok();
        gitignore_builder.add_line(None, "temp/").ok();
        gitignore_builder.add_line(None, "tmp/").ok();
        
        // IDEs
        gitignore_builder.add_line(None, ".vscode/").ok();
        gitignore_builder.add_line(None, ".idea/").ok();
        
        let gitignore = gitignore_builder.build().expect("Failed to build gitignore");
        
        // Collect all files first
        let files: Vec<_> = WalkDir::new(&root)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
            .filter(|e| {
                let path = e.path();
                let is_ignored = gitignore.matched(path, false).is_ignore();
                !is_ignored
            })
            .map(|e| e.path().to_path_buf())
            .collect();
        
        println!("Found {} files to process", files.len());
        
        // Report initial progress
        let _ = self.app_handle.emit("indexing-progress", ProgressEvent {
            total: files.len(),
            processed: 0,
            current_file: "".to_string(),
            status: "scanning".to_string(),
        });
        
        // Process files in parallel using Rayon
        // We collect parsed documents that need indexing
        let documents_to_index: Vec<(ParsedDocument, u64, u64, [u8; 32])> = files
            .par_iter()
            .filter_map(|path| self.process_file(path))
            .collect();
        
        println!("Indexing {} documents...", documents_to_index.len());
        
        // Add documents to index in batches (serial access to IndexManager)
        let batch_size = 100; // Smaller batch size for smoother UI updates
        let indexer = self.indexer.lock().await;
        
        for (i, (doc, modified, size, content_hash)) in documents_to_index.iter().enumerate() {
            // Update UI more frequently
            if i % 10 == 0 {
                let _ = self.app_handle.emit("indexing-progress", ProgressEvent {
                    total: documents_to_index.len(),
                    processed: i,
                    current_file: doc.path.clone(),
                    status: "indexing".to_string(),
                });
            }

            // Add to search index
            if let Err(e) = indexer.add_document(doc.clone()) {
                eprintln!("Failed to add document {}: {}", doc.path, e);
                continue;
            }
            
            // Update metadata
            let path = PathBuf::from(&doc.path);
            if let Err(e) = self.metadata_db.update_metadata(&path, *modified, *size, *content_hash) {
                eprintln!("Failed to update metadata for {}: {}", doc.path, e);
            }
            
            // Commit every batch
            if (i + 1) % batch_size == 0 {
                if let Err(e) = indexer.commit() {
                    eprintln!("Failed to commit batch: {}", e);
                }
            }
        }
        
        // Final commit
        indexer.commit()?;
        
        // Final progress report
        let _ = self.app_handle.emit("indexing-progress", ProgressEvent {
            total: documents_to_index.len(),
            processed: documents_to_index.len(),
            current_file: "Completed".to_string(),
            status: "done".to_string(),
        });
        
        println!("Successfully indexed {} files", documents_to_index.len());
        
        Ok(())
    }
    
    /// Process a single file - check if needs reindexing and parse
    fn process_file(&self, path: &Path) -> Option<(ParsedDocument, u64, u64, [u8; 32])> {
        // Get file metadata
        let metadata = std::fs::metadata(path).ok()?;
        let modified = metadata.modified().ok()?
            .duration_since(std::time::SystemTime::UNIX_EPOCH)
            .ok()?
            .as_secs();
        let size = metadata.len();
        
        // Check if we need to reindex this file
        match self.metadata_db.needs_reindex(path, modified, size) {
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
        
        // Compute content hash
        let content_hash = blake3::hash(parsed.content.as_bytes()).into();
        
        Some((parsed, modified, size, content_hash))
    }
}
