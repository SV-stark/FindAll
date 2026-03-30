pub mod drive_scanner;

use crate::error::Result;
use crate::indexer::IndexManager;
use crate::metadata::MetadataDb;
use crate::parsers::{parse_file, ParsedDocument};
use blake3;
use drive_scanner::DriveScanner;
use rayon::prelude::*;
use std::path::PathBuf;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::mpsc;
use tracing::{info, instrument, warn};

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

const BATCH_SIZE: usize = 1000;

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
    settings: crate::settings::AppSettings,
    pool: Arc<rayon::ThreadPool>,
}

impl Scanner {
    pub fn new(
        indexer: Arc<IndexManager>,
        metadata_db: Arc<MetadataDb>,
        filename_index: Option<Arc<crate::indexer::filename_index::FilenameIndex>>,
        progress_tx: Option<mpsc::Sender<ProgressEvent>>,
        settings: crate::settings::AppSettings,
    ) -> Self {
        let threads = settings.indexing_threads as usize;
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .stack_size(8 * 1024 * 1024)
            .build()
            .unwrap_or_else(|_| {
                rayon::ThreadPoolBuilder::new()
                    .stack_size(8 * 1024 * 1024)
                    .build()
                    .expect("Failed to initialize Rayon thread pool fallback")
            });

        Self {
            indexer,
            metadata_db,
            filename_index,
            progress_tx,
            settings,
            pool: Arc::new(pool),
        }
    }

    fn get_scanner(&self) -> Box<dyn DriveScanner> {
        #[cfg(target_os = "windows")]
        {
            Box::new(drive_scanner::WindowsDriveScanner)
        }
        #[cfg(target_os = "macos")]
        {
            Box::new(drive_scanner::MacDriveScanner)
        }
        #[cfg(target_os = "linux")]
        {
            Box::new(drive_scanner::LinuxDriveScanner)
        }
        #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
        {
            Box::new(drive_scanner::DefaultDriveScanner)
        }
    }

    #[instrument(skip(self, tx))]
    pub fn watch_drive(
        &self,
        root: PathBuf,
        tx: mpsc::Sender<(PathBuf, crate::watcher::WatcherAction)>,
    ) -> Result<()> {
        let scanner = self.get_scanner();
        scanner.watch(root, tx)
    }

    #[instrument(skip(self, exclude_patterns), fields(root = %root.display()))]
    pub async fn scan_directory(&self, root: PathBuf, exclude_patterns: Vec<String>) -> Result<()> {
        info!("Starting directory scan for {}", root.display());

        let (path_tx, path_rx) = crossbeam_channel::unbounded::<PathBuf>();

        let root_clone = root.clone();
        let tx_clone = self.progress_tx.clone();
        let scanner = self.get_scanner();
        let total = Arc::new(AtomicUsize::new(0));
        let total_for_scan = total.clone();

        let walker_handle = tokio::task::spawn_blocking(move || {
            scanner.scan(
                root_clone,
                exclude_patterns,
                path_tx,
                tx_clone,
                total_for_scan,
            )
        });

        // --- Stage 2: Content Indexing (Batched) ---
        // Parse files in parallel via Rayon, collect results via mpsc,
        // then batch-write to the index and metadata DB.
        let (task_tx, task_rx) = crossbeam_channel::bounded::<IndexTask>(BATCH_SIZE * 8);
        let metadata_db_for_parser = self.metadata_db.clone();
        let metadata_db_for_writer = self.metadata_db.clone();
        let indexer_clone = self.indexer.clone();
        let filename_index_clone = self.filename_index.clone();
        let progress_tx_clone = self.progress_tx.clone();
        let total_files = total.clone();

        let file_size_limit_mb = self.settings.index_file_size_limit_mb;
        let allowed_extensions: Arc<std::collections::HashSet<String>> = Arc::new(
            self.settings
                .get_allowed_extensions()
                .iter()
                .map(|e| e.to_lowercase())
                .collect(),
        );
        let allowed_extensions_clone = allowed_extensions.clone();

        // --- Stage 2a: Parallel parsing (Rayon) → sends IndexTask into channel ---
        let pool = self.pool.clone();
        let parser_handle = tokio::task::spawn_blocking(move || {
            info!("Stage 2a: Parallel Parsing");

            pool.install(|| {
                let mut chunk = Vec::with_capacity(200);
                for path in path_rx {
                    chunk.push(path);
                    if chunk.len() >= 200 {
                        Self::process_chunk(
                            &chunk,
                            &metadata_db_for_parser,
                            file_size_limit_mb,
                            &allowed_extensions_clone,
                            &task_tx,
                        );
                        chunk.clear();
                    }
                }
                if !chunk.is_empty() {
                    Self::process_chunk(
                        &chunk,
                        &metadata_db_for_parser,
                        file_size_limit_mb,
                        &allowed_extensions_clone,
                        &task_tx,
                    );
                }
            });
            // task_tx is dropped here, closing the channel
        });

        // --- Stage 2b: Sequential batch writer (single thread) ---
        let writer_handle = tokio::task::spawn_blocking(move || {
            info!("Stage 2b: Batch Writing");
            let start = Instant::now();
            let mut doc_batch: Vec<(crate::parsers::ParsedDocument, u64, u64)> =
                Vec::with_capacity(BATCH_SIZE);
            let mut meta_batch: Vec<(String, u64, u64, [u8; 32])> = Vec::with_capacity(BATCH_SIZE);
            let mut processed: usize = 0;

            for task in &task_rx {
                // Add to filename index
                if let Some(f_index) = &filename_index_clone {
                    let path = std::path::Path::new(&task.doc.path);
                    if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                        let _ = f_index.add_file(&task.doc.path, name);
                    }
                }

                // Clone path before moving doc
                let doc_path = task.doc.path.clone();
                doc_batch.push((task.doc, task.modified, task.size));
                meta_batch.push((doc_path, task.modified, task.size, task.content_hash));
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
                    let rate = if elapsed > 0.0 {
                        processed as f64 / elapsed
                    } else {
                        0.0
                    };

                    // Batch summary every 1000 files
                    if processed % 1000 == 0 {
                        info!(
                            "Indexed {} / {} files ({:.1} files/s)",
                            processed, current_total, rate
                        );
                    }

                    if let Some(tx) = &progress_tx_clone {
                        let _ = tx.try_send(ProgressEvent {
                            ptype: ProgressType::Content,
                            current_file: String::new(),
                            current_folder: String::new(),
                            processed,
                            total: current_total,
                            status: "Indexing contents...".to_string(),
                            eta_seconds: if rate > 0.0 {
                                (current_total.saturating_sub(processed) as f64 / rate) as u64
                            } else {
                                0
                            },
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
                    current_file: String::new(),
                    current_folder: String::new(),
                    processed,
                    total: processed,
                    status: "All files indexed".to_string(),
                    eta_seconds: 0,
                    files_per_second: 0.0,
                });
            }

            info!(
                "Indexed {} files in {:.2}s",
                processed,
                start.elapsed().as_secs_f64()
            );
        });

        // Wait for all stages to complete
        let _ = walker_handle
            .await
            .map_err(|e| crate::error::FlashError::index(format!("Walk task failed: {e}")))?;
        parser_handle
            .await
            .map_err(|e| crate::error::FlashError::index(format!("Parse task failed: {e}")))?;
        writer_handle
            .await
            .map_err(|e| crate::error::FlashError::index(format!("Write task failed: {e}")))?;

        // Commit filename index to disk
        if let Some(f_index) = &self.filename_index {
            let _ = f_index.commit();
        }

        Ok(())
    }

    fn process_chunk(
        chunk: &[PathBuf],
        metadata_db: &Arc<MetadataDb>,
        file_size_limit_mb: u32,
        allowed_extensions: &std::collections::HashSet<String>,
        task_tx: &crossbeam_channel::Sender<IndexTask>,
    ) {
        let limit_bytes = u64::from(file_size_limit_mb) * 1024 * 1024;
        let mut batch_entries = Vec::with_capacity(chunk.len());
        let mut valid_paths = Vec::with_capacity(chunk.len());

        for path in chunk {
            if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                let ext_lower = ext.to_ascii_lowercase();
                if !allowed_extensions.contains(&ext_lower) {
                    continue;
                }
            } else {
                // If the file doesn't have an extension, we skip it by default
                continue;
            }
            let metadata = match std::fs::metadata(path) {
                Ok(m) => m,
                Err(_) => continue,
            };

            let size = metadata.len();
            if size > limit_bytes {
                warn!(
                    "Skipping large file: {} ({} bytes > limit of {} bytes)",
                    path.display(),
                    size,
                    limit_bytes
                );
                continue;
            }

            let modified = metadata
                .modified()
                .unwrap_or(std::time::SystemTime::UNIX_EPOCH)
                .duration_since(std::time::SystemTime::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();

            batch_entries.push((path.to_string_lossy().to_string(), modified, size));
            valid_paths.push((path.clone(), modified, size));
        }

        if batch_entries.is_empty() {
            return;
        }

        let needs_reindex = metadata_db
            .batch_needs_reindex(&batch_entries)
            .unwrap_or_else(|_| vec![true; batch_entries.len()]);

        let paths_to_parse: Vec<_> = valid_paths
            .into_iter()
            .enumerate()
            .filter(|(i, _)| needs_reindex[*i])
            .map(|(_, data)| data)
            .collect();

        let just_paths: Vec<PathBuf> = paths_to_parse.iter().map(|(p, _, _)| p.clone()).collect();

        // Stage 1: Attempt native batch computation
        if let Ok(results) = crate::parsers::parse_files_batch(just_paths) {
            for (parsed_res, (path, modified, size)) in
                results.into_iter().zip(paths_to_parse.into_iter())
            {
                match parsed_res {
                    Ok(parsed) => {
                        let content_hash = blake3::hash(parsed.content.as_bytes()).into();
                        let _ = task_tx.send(IndexTask {
                            doc: parsed,
                            modified,
                            size,
                            content_hash,
                        });
                    }
                    Err(e) => {
                        warn!("Failed to parse file {:?}: {}", path, e);
                    }
                }
            }
        } else {
            // Stage 2: Fallback to standalone parsing to isolate obscure file panic loops
            warn!("Batch parsing crashed, dropping to discrete fallback loops");
            paths_to_parse
                .into_par_iter()
                .for_each(|(path, modified, size)| {
                    if let Ok(parsed) = parse_file(&path) {
                        let content_hash = blake3::hash(parsed.content.as_bytes()).into();
                        let _ = task_tx.send(IndexTask {
                            doc: parsed,
                            modified,
                            size,
                            content_hash,
                        });
                    } else {
                        warn!("Failed to parse file {:?}", path);
                    }
                });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::indexer::IndexManager;
    use crate::metadata::MetadataDb;
    use crate::settings::AppSettings;
    use std::sync::Arc;
    use tempfile::tempdir;

    #[test]
    fn test_scanner_new() {
        let dir = tempdir().unwrap();
        let index_path = dir.path().join("index");
        let db_path = dir.path().join("metadata.redb");

        let settings = AppSettings::default();
        let indexer = Arc::new(IndexManager::open(&index_path, 100).unwrap());
        let metadata_db = Arc::new(MetadataDb::open(&db_path).unwrap());

        let scanner = Scanner::new(indexer, metadata_db, None, None, settings);

        assert!(scanner.filename_index.is_none());
    }

    #[test]
    fn test_progress_event_serialization() {
        let event = ProgressEvent {
            total: 100,
            processed: 50,
            current_file: "test.txt".to_string(),
            status: "Indexing...".to_string(),
            ptype: ProgressType::Content,
            files_per_second: 10.5,
            eta_seconds: 5,
            current_folder: "/home/user".to_string(),
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("test.txt"));
        assert!(json.contains("Content"));
    }
}
