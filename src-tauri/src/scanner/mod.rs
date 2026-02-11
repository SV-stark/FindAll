use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::Mutex;
use walkdir::WalkDir;
use rayon::prelude::*;
use ignore::gitignore::GitignoreBuilder;
use crate::error::Result;
use crate::indexer::IndexManager;
use crate::metadata::MetadataDb;
use crate::parsers::{parse_file, ParsedDocument};
use blake3;

/// Scans directories and indexes files
pub struct Scanner {
    indexer: Arc<Mutex<IndexManager>>,
    metadata_db: Arc<MetadataDb>,
}

impl Scanner {
    pub fn new(
        indexer: Arc<Mutex<IndexManager>>,
        metadata_db: Arc<MetadataDb>,
    ) -> Self {
        Self {
            indexer,
            metadata_db,
        }
    }
    
    /// Scan a directory and index all supported files
    pub async fn scan_directory(&self, root: PathBuf) -> Result<()> {
        // Build gitignore matcher
        let mut gitignore_builder = GitignoreBuilder::new(&root);
        gitignore_builder.add_line(None, ".git/").ok();
        gitignore_builder.add_line(None, "node_modules/").ok();
        gitignore_builder.add_line(None, "target/").ok();
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
        
        // Process files in parallel using Rayon
        // We collect parsed documents that need indexing
        let documents_to_index: Vec<(ParsedDocument, u64, u64, [u8; 32])> = files
            .par_iter()
            .filter_map(|path| self.process_file(path))
            .collect();
        
        println!("Indexing {} documents...", documents_to_index.len());
        
        // Add documents to index in batches (serial access to IndexManager)
        let batch_size = 1000;
        let indexer = self.indexer.lock().await;
        
        for (i, (doc, modified, size, content_hash)) in documents_to_index.iter().enumerate() {
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
                println!("Committing batch...");
                if let Err(e) = indexer.commit() {
                    eprintln!("Failed to commit batch: {}", e);
                }
            }
        }
        
        // Final commit
        indexer.commit()?;
        
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
