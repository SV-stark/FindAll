pub mod cancel;
pub mod drive_scanner;

use crate::error::Result;
use crate::indexer::IndexManager;
use crate::metadata::MetadataDb;
use crate::parsers::ParsedDocument;
use cancel::CancelToken;
use drive_scanner::DriveScanner;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::time::Instant;
use tokio::sync::mpsc;
use tracing::{error, info, instrument, warn};

/// Drops already-indexed files from `chunk` and forwards the stale remainder.
///
/// A metadata read failure is logged once and treated as "everything is stale",
/// which is the safe direction: re-parsing a file is wasted work, silently
/// *skipping* one leaves the user unable to find it.
fn send_stale_chunk(
    metadata_db: &MetadataDb,
    chunk_tx: &mpsc::Sender<Vec<(PathBuf, u64, u64)>>,
    chunk: &mut Vec<(PathBuf, u64, u64)>,
) -> bool {
    let needs = match metadata_db.batch_needs_reindex_paths(chunk) {
        Ok(needs) => needs,
        Err(e) => {
            warn!("Metadata staleness check failed, re-indexing chunk: {e}");
            vec![true; chunk.len()]
        }
    };

    let stale: Vec<_> = std::mem::take(chunk)
        .into_iter()
        .zip(needs)
        .filter_map(|(item, need)| need.then_some(item))
        .collect();

    if stale.is_empty() {
        return true;
    }

    // `blocking_send` returns false once the receiver is gone, which means the
    // parser stage was cancelled or failed.
    chunk_tx.blocking_send(stale).is_ok()
}

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

const BATCH_SIZE: usize = 5000;

/// Number of batches between Tantivy commits during a scan.
///
/// Committing only at the very end means a crash, kill, or power loss during a
/// multi-hour index run discards *everything* written so far. Committing every
/// `COMMIT_EVERY_BATCHES` batches bounds that loss window while keeping segment
/// churn low.
const COMMIT_EVERY_BATCHES: usize = 4;

/// Counters describing what a scan actually managed to persist.
///
/// Indexing errors used to be discarded with `let _ =`, which meant a failing
/// Tantivy writer or a corrupt metadata row produced a silently incomplete
/// index with a cheerful "All files indexed" message.
#[derive(Debug, Default)]
struct WriteStats {
    documents_written: usize,
    metadata_written: usize,
    filename_entries_written: usize,
    write_errors: usize,
}

impl WriteStats {
    const fn has_errors(&self) -> bool {
        self.write_errors > 0
    }
}

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
    progress_tx: Option<flume::Sender<ProgressEvent>>,
    settings: crate::settings::AppSettings,
}

impl Scanner {
    /// Creates a new Scanner instance.
    pub const fn new(
        indexer: Arc<IndexManager>,
        metadata_db: Arc<MetadataDb>,
        filename_index: Option<Arc<crate::indexer::filename_index::FilenameIndex>>,
        progress_tx: Option<flume::Sender<ProgressEvent>>,
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

    /// Consumes parsed documents from `task_rx`, flushing them to Tantivy, the
    /// metadata DB, and the filename index.
    ///
    /// Returns the write statistics, or the first hard error that made the index
    /// unreliable. Individual per-batch failures are logged and counted rather
    /// than aborted, so one bad batch cannot throw away hours of work.
    #[allow(clippy::too_many_lines)]
    fn process_writer_loop(
        task_rx: &flume::Receiver<IndexTask>,
        filename_index: Option<&Arc<crate::indexer::filename_index::FilenameIndex>>,
        indexer: &Arc<IndexManager>,
        metadata_db: &Arc<MetadataDb>,
        progress_tx: Option<&flume::Sender<ProgressEvent>>,
        total_files: &Arc<AtomicUsize>,
        cancel: &CancelToken,
    ) -> WriteStats {
        info!("Stage 2c: Batch Writing");
        let start = Instant::now();
        let mut stats = WriteStats::default();
        let mut doc_batch: Vec<(crate::parsers::ParsedDocument, u64, u64)> =
            Vec::with_capacity(BATCH_SIZE);
        let mut meta_batch: Vec<(String, u64, u64, [u8; 32])> = Vec::with_capacity(BATCH_SIZE);
        let mut filename_batch: Vec<crate::indexer::filename_index::FilenameEntry> =
            Vec::with_capacity(BATCH_SIZE);
        let mut processed: usize = 0;
        let mut batches_since_commit: usize = 0;
        let mut cancelled = false;

        for task in task_rx {
            if cancel.is_cancelled() {
                warn!("Indexing cancelled. Flushing batches...");
                cancelled = true;
                break;
            }

            // Prepare for filename index
            if filename_index.is_some() {
                let path = std::path::Path::new(&task.doc.path);
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    filename_batch.push(crate::indexer::filename_index::FilenameEntry {
                        path: task.doc.path.clone(),
                        name: compact_str::CompactString::from(name),
                    });
                }
            }

            let current_file = std::path::Path::new(&task.doc.path)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("")
                .to_string();

            // Clone path before moving doc
            let doc_path = task.doc.path.clone();
            doc_batch.push((task.doc, task.modified, task.size));
            meta_batch.push((doc_path, task.modified, task.size, task.content_hash));
            processed += 1;

            // Flush batch when full
            if doc_batch.len() >= BATCH_SIZE {
                batches_since_commit += 1;
                let should_commit = batches_since_commit >= COMMIT_EVERY_BATCHES;
                Self::flush_batches(
                    &mut doc_batch,
                    &mut meta_batch,
                    &mut filename_batch,
                    filename_index,
                    indexer,
                    metadata_db,
                    &mut stats,
                );
                if should_commit {
                    batches_since_commit = 0;
                    if let Err(e) = indexer.commit() {
                        error!("Failed to commit index mid-scan: {e}");
                        stats.write_errors += 1;
                    } else {
                        indexer.invalidate_cache();
                    }
                }
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

                if let Some(tx) = progress_tx {
                    let _ = tx.try_send(ProgressEvent {
                        ptype: ProgressType::Content,
                        current_file,
                        current_folder: String::new(),
                        processed,
                        total: current_total,
                        status: format!("Indexing: {processed} / {current_total}"),
                        eta_seconds: if rate > 0.0 && current_total > processed {
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

        // Always flush the remainder, regardless of which batch happens to be
        // non-empty. The old code gated the whole tail flush (metadata *and*
        // filename index *and* the final commit) on `!doc_batch.is_empty()`, so
        // cancelling a scan right after a batch boundary silently dropped every
        // metadata row and filename entry that had been accumulated.
        Self::flush_batches(
            &mut doc_batch,
            &mut meta_batch,
            &mut filename_batch,
            filename_index,
            indexer,
            metadata_db,
            &mut stats,
        );

        if let Err(e) = indexer.commit() {
            error!("Failed to commit index at end of scan: {e}");
            stats.write_errors += 1;
        }
        indexer.invalidate_cache();

        if let Some(f_index) = filename_index
            && let Err(e) = f_index.commit()
        {
            error!("Failed to commit filename index: {e}");
            stats.write_errors += 1;
        }

        // Final progress
        if let Some(tx) = progress_tx {
            let _ = tx.try_send(ProgressEvent {
                ptype: ProgressType::Content,
                current_file: String::new(),
                current_folder: String::new(),
                processed,
                total: processed,
                status: if cancelled {
                    format!("Indexing cancelled after {processed} files")
                } else {
                    "All files indexed".to_string()
                },
                eta_seconds: 0,
                files_per_second: 0.0,
            });
        }

        info!(
            "Indexed {} files in {:.2}s ({} docs, {} metadata rows, {} filename entries, {} write errors)",
            processed,
            start.elapsed().as_secs_f64(),
            stats.documents_written,
            stats.metadata_written,
            stats.filename_entries_written,
            stats.write_errors
        );

        stats
    }

    /// Writes the currently accumulated batches and clears them.
    fn flush_batches(
        doc_batch: &mut Vec<(crate::parsers::ParsedDocument, u64, u64)>,
        meta_batch: &mut Vec<(String, u64, u64, [u8; 32])>,
        filename_batch: &mut Vec<crate::indexer::filename_index::FilenameEntry>,
        filename_index: Option<&Arc<crate::indexer::filename_index::FilenameIndex>>,
        indexer: &Arc<IndexManager>,
        metadata_db: &Arc<MetadataDb>,
        stats: &mut WriteStats,
    ) {
        if !doc_batch.is_empty() {
            match indexer.add_documents_batch(doc_batch) {
                Ok(()) => stats.documents_written += doc_batch.len(),
                Err(e) => {
                    error!(
                        "Failed to write {} documents to the index: {e}",
                        doc_batch.len()
                    );
                    stats.write_errors += doc_batch.len();
                }
            }
        }

        if !meta_batch.is_empty() {
            match metadata_db.batch_update_metadata(meta_batch) {
                Ok(written) => stats.metadata_written += written,
                Err(e) => {
                    error!("Failed to write {} metadata rows: {e}", meta_batch.len());
                    stats.write_errors += meta_batch.len();
                }
            }
        }

        if let Some(f_index) = filename_index
            && !filename_batch.is_empty()
        {
            stats.filename_entries_written +=
                f_index.add_files_batch(std::mem::take(filename_batch));
        }

        doc_batch.clear();
        meta_batch.clear();
    }

    /// Scans `root` and indexes every supported, non-excluded file.
    ///
    /// Pass a [`CancelToken`] from [`cancel::CancellationController::begin`] so
    /// that starting a new run reliably stops this one, including its
    /// `spawn_blocking` stages which `JoinHandle::abort` cannot reach.
    ///
    /// Returns an error when a stage failed outright. Partial per-batch write
    /// failures are logged and counted (and surfaced through the returned
    /// [`IndexingReport`]) rather than aborting the run.
    #[allow(clippy::too_many_lines)]
    #[instrument(skip(self, exclude_patterns, cancel), fields(root = %root.display()))]
    pub async fn scan_directory(
        &self,
        root: PathBuf,
        exclude_patterns: Vec<String>,
        cancel: CancelToken,
    ) -> Result<IndexingReport> {
        info!("Starting directory scan for {}", root.display());

        // Bounded channel with backpressure to prevent unbounded RAM explosion on large filesystems
        let (path_tx, path_rx) = flume::bounded::<PathBuf>(10_000);

        let root_clone = root.clone();
        let tx_clone = self.progress_tx.clone();
        let scanner = Self::get_scanner();
        let total = Arc::new(AtomicUsize::new(0));
        let total_for_scan = total.clone();

        let use_gitignore = self.settings.use_gitignore;
        let cancel_for_scan = cancel.clone();
        let walker_handle = tokio::task::spawn_blocking(move || {
            scanner.scan(
                root_clone,
                exclude_patterns,
                use_gitignore,
                path_tx,
                tx_clone,
                total_for_scan,
                cancel_for_scan,
            )
        });

        // --- Stage 2: Content Indexing (Async Batched) ---
        const CHUNK_SIZE: usize = 200;

        // Bounded tasks channel with backpressure
        let (task_tx, task_rx) = flume::bounded::<IndexTask>(BATCH_SIZE);
        // Async channel for sending path-chunks from the blocking walker to the async parser.
        let (chunk_tx, mut chunk_rx) = tokio::sync::mpsc::channel::<Vec<(PathBuf, u64, u64)>>(32);

        let metadata_db_for_filter = self.metadata_db.clone();
        let metadata_db_for_writer = self.metadata_db.clone();
        let indexer_clone = self.indexer.clone();
        let filename_index_clone = self.filename_index.clone();
        let progress_tx_clone = self.progress_tx.clone();
        let total_files = total.clone();

        let indexing_threads = self.settings.indexing_threads;
        let enable_ocr = self.settings.enable_ocr;
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
        let cancel_for_filter = cancel.clone();
        let filter_handle = tokio::task::spawn_blocking(move || {
            info!("Stage 2a: Path filtering and chunking");
            let limit_bytes = u64::from(file_size_limit_mb) * 1024 * 1024;
            let mut chunk: Vec<(PathBuf, u64, u64)> = Vec::with_capacity(CHUNK_SIZE);

            for path in path_rx {
                if cancel_for_filter.is_cancelled() {
                    break;
                }

                // Extension filter (zero-allocation stack check via SmallVec)
                let Some(ext) = path.extension().and_then(|e| e.to_str()) else {
                    continue;
                };
                let mut ext_buf = smallvec::SmallVec::<[u8; 16]>::new();
                ext_buf.extend_from_slice(ext.as_bytes());
                ext_buf.make_ascii_lowercase();
                let is_allowed = std::str::from_utf8(&ext_buf)
                    .is_ok_and(|ext_lower| allowed_extensions.contains(ext_lower));
                if !is_allowed {
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

                if chunk.len() >= CHUNK_SIZE
                    && send_stale_chunk(&metadata_db_for_filter, &chunk_tx, &mut chunk)
                {
                    // Downstream is gone; stop feeding it.
                    return;
                }
            }

            // Flush remainder
            if !chunk.is_empty() {
                send_stale_chunk(&metadata_db_for_filter, &chunk_tx, &mut chunk);
            }
            // chunk_tx drops here, closing chunk_rx.
        });

        // --- Stage 2b: Async Xberg batch parser ---
        // Receives chunks over the mpsc channel and awaits xberg's native
        // concurrent JoinSet-based batch extractor directly on the Tokio runtime.
        let task_tx_for_parser = task_tx.clone();
        let progress_tx_for_parser = self.progress_tx.clone();
        let total_files_for_parser = total.clone();

        let cancel_for_parser = cancel.clone();

        let parser_handle = tokio::spawn(async move {
            info!("Stage 2b: Async Xberg batch parsing");

            while let Some(chunk) = chunk_rx.recv().await {
                if cancel_for_parser.is_cancelled() {
                    break;
                }

                if chunk.is_empty() {
                    continue;
                }

                let mut paths_to_parse = Vec::with_capacity(chunk.len());
                let mut meta_to_parse = Vec::with_capacity(chunk.len());

                for (path, modified, size) in chunk {
                    paths_to_parse.push(path);
                    meta_to_parse.push((modified, size));
                }

                if let Some(tx) = &progress_tx_for_parser {
                    let current_total = total_files_for_parser.load(Ordering::Relaxed);
                    let first_file = paths_to_parse
                        .first()
                        .and_then(|p| p.file_name())
                        .map_or_else(String::new, |n| n.to_string_lossy().to_string());

                    let _ = tx.try_send(ProgressEvent {
                        ptype: ProgressType::Content,
                        current_file: first_file,
                        current_folder: String::new(),
                        processed: 0,
                        total: current_total,
                        status: format!("Parsing batch of {} files...", paths_to_parse.len()),
                        eta_seconds: 0,
                        files_per_second: 0.0,
                    });
                }

                let load_metrics =
                    crate::system::throttling::get_adaptive_system_load(indexing_threads);
                if load_metrics.throttle_delay_ms > 0 {
                    tokio::time::sleep(std::time::Duration::from_millis(
                        load_metrics.throttle_delay_ms,
                    ))
                    .await;
                }

                match crate::parsers::parse_files_batch(
                    &paths_to_parse,
                    load_metrics.active_threads,
                    enable_ocr,
                )
                .await
                {
                    Ok(results) => {
                        for (parsed_res, (modified, size)) in
                            results.into_iter().zip(meta_to_parse.into_iter())
                        {
                            match parsed_res {
                                Ok((parsed, hash)) => {
                                    if task_tx_for_parser
                                        .send(IndexTask {
                                            doc: parsed,
                                            modified,
                                            size,
                                            content_hash: hash,
                                        })
                                        .is_err()
                                    {
                                        break;
                                    }
                                }
                                Err(e) => {
                                    warn!("Failed to parse file: {}", e);
                                }
                            }
                        }
                    }
                    Err(e) => {
                        warn!("Async batch crashed ({e}), falling back to per-file parsing");
                        for (path, (modified, size)) in
                            paths_to_parse.into_iter().zip(meta_to_parse.into_iter())
                        {
                            if let Ok((parsed, hash)) =
                                crate::parsers::parse_file_with_hash(&path, enable_ocr).await
                            {
                                if task_tx_for_parser
                                    .send(IndexTask {
                                        doc: parsed,
                                        modified,
                                        size,
                                        content_hash: hash,
                                    })
                                    .is_err()
                                {
                                    break;
                                }
                            } else {
                                warn!("Failed to parse file {:?}", path);
                            }
                        }
                    }
                }
            }
            drop(task_tx_for_parser);
        });

        // --- Stage 2c: Sequential batch writer (sync) ---
        // Tantivy writes must be sequential; this separate thread drains task_rx.
        let cancel_for_writer = cancel.clone();
        let writer_handle = tokio::task::spawn_blocking(move || {
            Self::process_writer_loop(
                &task_rx,
                filename_index_clone.as_ref(),
                &indexer_clone,
                &metadata_db_for_writer,
                progress_tx_clone.as_ref(),
                &total_files,
                &cancel_for_writer,
            )
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
        let stats = writer_handle
            .await
            .map_err(|e| crate::error::FlashError::index(format!("Write task failed: {e}")))?;

        if stats.has_errors() {
            warn!(
                "Indexing finished with {} write errors ({} docs, {} metadata rows, {} filename entries written)",
                stats.write_errors,
                stats.documents_written,
                stats.metadata_written,
                stats.filename_entries_written
            );
        }

        Ok(IndexingReport {
            documents_written: stats.documents_written,
            metadata_written: stats.metadata_written,
            filename_entries_written: stats.filename_entries_written,
            write_errors: stats.write_errors,
            cancelled: cancel.is_cancelled(),
        })
    }
}

/// Summary of what a single [`Scanner::scan_directory`] run persisted.
#[derive(Debug, Clone, Copy, Default, serde::Serialize)]
pub struct IndexingReport {
    pub documents_written: usize,
    pub metadata_written: usize,
    pub filename_entries_written: usize,
    pub write_errors: usize,
    pub cancelled: bool,
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
        let metadata_db = Arc::new(MetadataDb::open(&db_path).unwrap().0);

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
