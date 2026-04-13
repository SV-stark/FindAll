pub mod drive_scanner;

use crate::error::Result;
use crate::indexer::IndexManager;
use crate::metadata::MetadataDb;
use crate::parsers::{ParsedDocument, parse_file};
use blake3;
use drive_scanner::DriveScanner;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
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
}

impl Scanner {
    /// Creates a new Scanner instance.
    pub const fn new(
        indexer: Arc<IndexManager>,
        metadata_db: Arc<MetadataDb>,
        filename_index: Option<Arc<crate::indexer::filename_index::FilenameIndex>>,
        progress_tx: Option<mpsc::Sender<ProgressEvent>>,
        settings: crate::settings::AppSettings,
    ) -> Self {
        Self {
            indexer,
            metadata_db,
            filename_index,
            progress_tx,
            settings,
        }
    }

    fn get_scanner() -> Box<dyn DriveScanner> {
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
        let scanner = Self::get_scanner();
        scanner.watch(root, tx)
    }

    fn process_writer_loop(
        task_rx: &crossbeam_channel::Receiver<IndexTask>,
        filename_index: Option<&Arc<crate::indexer::filename_index::FilenameIndex>>,
        indexer: &Arc<IndexManager>,
        metadata_db: &Arc<MetadataDb>,
        progress_tx: Option<&mpsc::Sender<ProgressEvent>>,
        total_files: &Arc<AtomicUsize>,
    ) {
        info!("Stage 2b: Batch Writing");
        let start = Instant::now();
        let mut doc_batch: Vec<(crate::parsers::ParsedDocument, u64, u64)> =
            Vec::with_capacity(BATCH_SIZE);
        let mut meta_batch: Vec<(String, u64, u64, [u8; 32])> = Vec::with_capacity(BATCH_SIZE);
        let mut processed: usize = 0;

        for task in task_rx {
            // Add to filename index
            if let Some(f_index) = filename_index {
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
                let _ = indexer.add_documents_batch(&doc_batch);
                let _ = indexer.commit();
                indexer.invalidate_cache();
                let _ = metadata_db.batch_update_metadata(&meta_batch);
                doc_batch.clear();
                meta_batch.clear();
            }

            // Progress update
            if processed.is_multiple_of(10) {
                let current_total = total_files.load(Ordering::Relaxed);
                let elapsed = start.elapsed().as_secs_f64();
                let rate = if elapsed > 0.0 {
                    processed as f64 / elapsed
                } else {
                    0.0
                };

                // Batch summary every 1000 files
                if processed.is_multiple_of(1000) {
                    info!(
                        "Indexed {} / {} files ({:.1} files/s)",
                        processed, current_total, rate
                    );
                }

                if let Some(tx) = progress_tx {
                    let _ = tx.try_send(ProgressEvent {
                        ptype: ProgressType::Content,
                        current_file: String::new(),
                        current_folder: String::new(),
                        processed,
                        total: current_total,
                        status: "Indexing contents...".to_string(),
                        eta_seconds: if rate > 0.0 {
                            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                            {
                                (current_total.saturating_sub(processed) as f64 / rate).round()
                                    as u64
                            }
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
            let _ = indexer.add_documents_batch(&doc_batch);
            let _ = indexer.commit();
            indexer.invalidate_cache();
            let _ = metadata_db.batch_update_metadata(&meta_batch);
        }

        // Final progress
        if let Some(tx) = progress_tx {
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
    }

    #[allow(clippy::too_many_lines)]
    #[instrument(skip(self, exclude_patterns), fields(root = %root.display()))]
    pub async fn scan_directory(&self, root: PathBuf, exclude_patterns: Vec<String>) -> Result<()> {
        info!("Starting directory scan for {}", root.display());

        let (path_tx, path_rx) = crossbeam_channel::unbounded::<PathBuf>();

        let root_clone = root.clone();
        let tx_clone = self.progress_tx.clone();
        let scanner = Self::get_scanner();
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

        // --- Stage 2: Content Indexing (Async Batched) ---
        //
        // Architecture:
        //   - path_rx (crossbeam, sync) is drained in a spawn_blocking task.
        //   - Valid, filtered file paths are grouped into chunks of CHUNK_SIZE and
        //     sent through an async mpsc channel (chunk_tx -> chunk_rx).
        //   - An async Tokio task receives chunks and awaits parse_files_batch(),
        //     which uses kreuzberg's native JoinSet-based concurrency internally —
        //     no manual Rayon pool needed.
        //   - Parsed IndexTasks are forwarded to a sync writer via crossbeam.
        const CHUNK_SIZE: usize = 200;

        let (task_tx, task_rx) = crossbeam_channel::bounded::<IndexTask>(BATCH_SIZE * 8);
        // Async channel for sending path-chunks from the blocking walker to the async parser.
        let (chunk_tx, mut chunk_rx) = tokio::sync::mpsc::channel::<Vec<(PathBuf, u64, u64)>>(32);

        let metadata_db_for_filter = self.metadata_db.clone();
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

        // --- Stage 2a: Blocking path receiver + filter ---
        // Drains path_rx (crossbeam), applies extension/size/metadata filters,
        // checks the metadata DB for staleness, then sends chunks over chunk_tx.
        let filter_handle = tokio::task::spawn_blocking(move || {
            info!("Stage 2a: Path filtering and chunking");
            let limit_bytes = u64::from(file_size_limit_mb) * 1024 * 1024;
            let mut chunk: Vec<(PathBuf, u64, u64)> = Vec::with_capacity(CHUNK_SIZE);

            for path in path_rx {
                // Extension filter
                let Some(ext) = path.extension().and_then(|e| e.to_str()) else {
                    continue;
                };
                if !allowed_extensions.contains(&ext.to_ascii_lowercase()) {
                    continue;
                }

                // Stat the file
                let Ok(meta) = std::fs::metadata(&path) else {
                    continue;
                };
                let size = meta.len();
                if size > limit_bytes {
                    warn!(
                        "Skipping large file: {} ({} bytes > {} bytes limit)",
                        path.display(),
                        size,
                        limit_bytes
                    );
                    continue;
                }
                let modified = meta
                    .modified()
                    .unwrap_or(std::time::SystemTime::UNIX_EPOCH)
                    .duration_since(std::time::SystemTime::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();

                chunk.push((path, modified, size));

                if chunk.len() >= CHUNK_SIZE {
                    // Batch-check staleness against the metadata DB before sending.
                    let entries: Vec<_> = chunk
                        .iter()
                        .map(|(p, m, s)| (p.to_string_lossy().to_string(), *m, *s))
                        .collect();
                    let needs: Vec<bool> = metadata_db_for_filter
                        .batch_needs_reindex(&entries)
                        .unwrap_or_else(|_| vec![true; entries.len()]);
                    let current_chunk = std::mem::take(&mut chunk);
                    let stale: Vec<_> = current_chunk
                        .into_iter()
                        .zip(needs)
                        .filter_map(|(item, need)| need.then_some(item))
                        .collect();
                    if !stale.is_empty() {
                        let _ = chunk_tx.blocking_send(stale);
                    }
                    chunk.clear();
                }
            }

            // Flush remainder
            if !chunk.is_empty() {
                let entries: Vec<_> = chunk
                    .iter()
                    .map(|(p, m, s)| (p.to_string_lossy().to_string(), *m, *s))
                    .collect();
                let needs: Vec<bool> = metadata_db_for_filter
                    .batch_needs_reindex(&entries)
                    .unwrap_or_else(|_| vec![true; entries.len()]);
                let stale: Vec<_> = chunk
                    .into_iter()
                    .zip(needs)
                    .filter_map(|(item, need)| need.then_some(item))
                    .collect();
                if !stale.is_empty() {
                    let _ = chunk_tx.blocking_send(stale);
                }
            }
            // chunk_tx drops here, closing chunk_rx.
        });

        // --- Stage 2b: Async Kreuzberg batch parser ---
        // Receives chunks over the mpsc channel and awaits kreuzberg's native
        // concurrent JoinSet-based batch extractor directly on the Tokio runtime.
        let task_tx_for_parser = task_tx.clone();
        let parser_handle = tokio::spawn(async move {
            info!("Stage 2b: Async Kreuzberg batch parsing");
            while let Some(chunk) = chunk_rx.recv().await {
                let just_paths: Vec<PathBuf> = chunk.iter().map(|(p, _, _)| p.clone()).collect();

                match crate::parsers::parse_files_batch(&just_paths).await {
                    Ok(results) => {
                        for (parsed_res, (path, modified, size)) in
                            results.into_iter().zip(chunk.into_iter())
                        {
                            match parsed_res {
                                Ok(parsed) => {
                                    let content_hash =
                                        blake3::hash(parsed.content.as_bytes()).into();
                                    let _ = task_tx_for_parser.send(IndexTask {
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
                    }
                    Err(e) => {
                        // Batch-level crash: fall back to individual sync parsing.
                        warn!("Async batch crashed ({e}), falling back to per-file sync parsing");
                        for (path, modified, size) in chunk {
                            if let Ok(parsed) = parse_file(&path) {
                                let content_hash = blake3::hash(parsed.content.as_bytes()).into();
                                let _ = task_tx_for_parser.send(IndexTask {
                                    doc: parsed,
                                    modified,
                                    size,
                                    content_hash,
                                });
                            } else {
                                warn!("Failed to parse file {:?}", path);
                            }
                        }
                    }
                }
            }
            // Drop task_tx clone so the writer sees EOF once filter also drops.
            drop(task_tx_for_parser);
        });

        // --- Stage 2c: Sequential batch writer (sync) ---
        // Tantivy writes must be sequential; this separate thread drains task_rx.
        let writer_handle = tokio::task::spawn_blocking(move || {
            Self::process_writer_loop(
                &task_rx,
                filename_index_clone.as_ref(),
                &indexer_clone,
                &metadata_db_for_writer,
                progress_tx_clone.as_ref(),
                &total_files,
            );
        });

        // Wait for all stages to complete
        walker_handle
            .await
            .map_err(|e| crate::error::FlashError::index(format!("Walk task failed: {e}")))?
            .map_err(|e| crate::error::FlashError::index(format!("Walk logic failed: {e}")))?;
        filter_handle
            .await
            .map_err(|e| crate::error::FlashError::index(format!("Filter task failed: {e}")))?;
        parser_handle
            .await
            .map_err(|e| crate::error::FlashError::index(format!("Parse task failed: {e}")))?;
        // Drop the original task_tx so the writer sees the channel close.
        drop(task_tx);
        writer_handle
            .await
            .map_err(|e| crate::error::FlashError::index(format!("Write task failed: {e}")))?;

        // Commit filename index to disk
        if let Some(f_index) = &self.filename_index {
            let _ = f_index.commit();
        }

        Ok(())
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
