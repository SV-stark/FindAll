use crate::error::{FlashError, Result};
use crate::indexer::IndexManager;
use crate::metadata::MetadataDb;
use arc_swap::ArcSwap;
use globset::{Glob, GlobSet, GlobSetBuilder};
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WatcherAction {
    Index,
    Remove,
}

use std::sync::atomic::{AtomicU64, Ordering};

static DROPPED_WATCHER_EVENTS: AtomicU64 = AtomicU64::new(0);

/// Debounce window between bursts of filesystem events.
const DEBOUNCE_GAP: Duration = Duration::from_millis(500);
/// Hard cap on how long a burst of events is buffered before being flushed.
const MAX_DEBOUNCE_WAIT: Duration = Duration::from_secs(5);
/// Maximum number of files re-parsed concurrently for one debounced burst.
const REINDEX_CONCURRENCY: usize = 8;

/// Settings values the watcher needs at event time.
///
/// Previously these were snapshotted once at construction, so editing
/// `custom_extensions` or `exclude_patterns` in Settings had no effect until
/// restart, and the watcher and scanner disagreed about what to index.
#[derive(Debug, Clone)]
struct WatcherConfig {
    allowed_extensions: HashSet<String>,
    exclude_globs: Arc<GlobSet>,
    enable_ocr: bool,
}

/// Manages active file system watching with debouncing
pub struct WatcherManager {
    watchers: HashMap<String, RecommendedWatcher>,
    _indexer: Arc<IndexManager>,
    _metadata_db: Arc<MetadataDb>,
    _runtime_handle: tokio::runtime::Handle,
    external_tx: mpsc::Sender<(PathBuf, WatcherAction)>,
    /// Live configuration, swappable without restarting the watcher task.
    config: Arc<ArcSwap<WatcherConfig>>,
}

impl WatcherManager {
    /// Creates a new `WatcherManager`
    pub fn new(
        indexer: Arc<IndexManager>,
        metadata_db: Arc<MetadataDb>,
        allowed_extensions: HashSet<String>,
        enable_ocr: bool,
    ) -> Self {
        Self::new_with_excludes(indexer, metadata_db, allowed_extensions, &[], enable_ocr)
    }

    /// Creates a new `WatcherManager` with exclude patterns.
    pub fn new_with_excludes(
        indexer: Arc<IndexManager>,
        metadata_db: Arc<MetadataDb>,
        allowed_extensions: HashSet<String>,
        exclude_patterns: &[String],
        enable_ocr: bool,
    ) -> Self {
        let (external_tx, external_rx) = mpsc::channel::<(PathBuf, WatcherAction)>(10_000);
        let runtime_handle = tokio::runtime::Handle::current();

        let config = Arc::new(ArcSwap::from_pointee(WatcherConfig {
            allowed_extensions,
            exclude_globs: Arc::new(compile_globs(exclude_patterns)),
            enable_ocr,
        }));

        // Spawn background processor for debounced events
        Self::spawn_processor_task(
            &runtime_handle,
            external_rx,
            indexer.clone(),
            metadata_db.clone(),
            Arc::clone(&config),
        );

        Self {
            watchers: HashMap::new(),
            _indexer: indexer,
            _metadata_db: metadata_db,
            _runtime_handle: runtime_handle,
            external_tx,
            config,
        }
    }

    /// Updates the exclude globs without restarting the watcher task.
    pub fn update_exclude_patterns(&self, patterns: &[String]) {
        let current = self.config.load();
        self.config.store(Arc::new(WatcherConfig {
            allowed_extensions: current.allowed_extensions.clone(),
            exclude_globs: Arc::new(compile_globs(patterns)),
            enable_ocr: current.enable_ocr,
        }));
        info!(
            "Updated watcher exclude patterns ({} rules)",
            patterns.len()
        );
    }

    /// Updates the set of indexed extensions without restarting the watcher task.
    pub fn update_allowed_extensions(&self, extensions: &HashSet<String>) {
        let current = self.config.load();
        self.config.store(Arc::new(WatcherConfig {
            allowed_extensions: extensions.clone(),
            exclude_globs: Arc::clone(&current.exclude_globs),
            enable_ocr: current.enable_ocr,
        }));
        info!(
            "Updated watcher extensions ({} types)",
            extensions.len()
        );
    }

    /// Updates whether OCR is attempted for newly changed documents.
    pub fn set_enable_ocr(&self, enable_ocr: bool) {
        let current = self.config.load();
        self.config.store(Arc::new(WatcherConfig {
            allowed_extensions: current.allowed_extensions.clone(),
            exclude_globs: Arc::clone(&current.exclude_globs),
            enable_ocr,
        }));
    }

    fn spawn_processor_task(
        runtime_handle: &tokio::runtime::Handle,
        mut external_rx: mpsc::Receiver<(PathBuf, WatcherAction)>,
        indexer: Arc<IndexManager>,
        metadata_db: Arc<MetadataDb>,
        config: Arc<ArcSwap<WatcherConfig>>,
    ) {
        runtime_handle.spawn(async move {
            let mut ordered_buffer: Vec<(PathBuf, WatcherAction)> = Vec::new();
            let mut first_event_time: Option<std::time::Instant> = None;

            loop {
                let timeout_duration = first_event_time.map_or_else(
                    || Duration::from_secs(3600),
                    |first_time| {
                        let elapsed = first_time.elapsed();
                        if elapsed >= MAX_DEBOUNCE_WAIT {
                            Duration::ZERO // Force flush immediately
                        } else {
                            DEBOUNCE_GAP.min(
                                MAX_DEBOUNCE_WAIT
                                    .checked_sub(elapsed)
                                    .unwrap_or(Duration::ZERO),
                            )
                        }
                    },
                );

                tokio::select! {
                    res = external_rx.recv() => {
                        if let Some((path, action)) = res {
                            if ordered_buffer.is_empty() {
                                first_event_time = Some(std::time::Instant::now());
                            }
                            ordered_buffer.push((path, action));
                        } else {
                            break;
                        }
                    }
                    () = tokio::time::sleep(timeout_duration) => {
                        if ordered_buffer.is_empty() {
                            continue;
                        }
                        first_event_time = None;
                        let events = std::mem::take(&mut ordered_buffer);
                        let snapshot = config.load_full();
                        Self::process_events(events, &indexer, &metadata_db, &snapshot).await;
                    }
                }
            }
        });
    }

    /// Applies one debounced burst of filesystem events.
    ///
    /// Additions and modifications are collapsed per path and re-parsed
    /// concurrently. The previous implementation awaited a full document
    /// extraction *per event, serially*, so a single `git checkout` or
    /// `npm install` could block the watcher for minutes and overflow the
    /// 10 000-slot queue (whose drops were only counted, never repaired).
    async fn process_events(
        events: Vec<(PathBuf, WatcherAction)>,
        indexer: &Arc<IndexManager>,
        metadata_db: &Arc<MetadataDb>,
        config: &WatcherConfig,
    ) {
        // Filter excluded paths, then collapse to the last action per path so
        // Remove -> Create -> Modify sequences cost one re-index, not three.
        let mut latest: std::collections::HashMap<PathBuf, WatcherAction> =
            std::collections::HashMap::with_capacity(events.len());
        let mut order: Vec<PathBuf> = Vec::with_capacity(events.len());

        for (path, action) in events {
            if Self::is_excluded(&path, &config.exclude_globs) {
                continue;
            }
            if latest.insert(path.clone(), action).is_none() {
                order.push(path);
            }
        }

        let mut removals: Vec<PathBuf> = Vec::new();
        let mut additions: Vec<PathBuf> = Vec::new();

        for path in order {
            match latest[&path] {
                WatcherAction::Remove => removals.push(path),
                WatcherAction::Index => {
                    let extension_ok = path
                        .extension()
                        .and_then(|e| e.to_str())
                        .is_some_and(|ext| config.allowed_extensions.contains(&ext.to_lowercase()));
                    if extension_ok {
                        additions.push(path);
                    }
                }
            }
        }

        let mut needs_commit = false;

        for path in &removals {
            let path_str = path.to_string_lossy();
            if let Err(e) = indexer.remove_document(&path_str) {
                error!("Watcher failed to remove {} from index: {e}", path.display());
            }
            match metadata_db.remove_file(path) {
                Ok(true) => {
                    needs_commit = true;
                    info!("Removed file (watcher): {:?}", path);
                }
                Ok(false) => {}
                Err(e) => error!("Watcher failed to remove {} from metadata: {e}", path.display()),
            }
        }

        if !additions.is_empty() {
            let parsed = Self::reindex_files(&additions, metadata_db, config.enable_ocr).await;
            let indexer = Arc::clone(indexer);
            let metadata_db = Arc::clone(metadata_db);
            // Index writes are blocking, so keep them off the runtime's async workers.
            let write_result = tokio::task::spawn_blocking(move || {
                let mut written = 0usize;
                for (path, modified, size, hash, doc) in parsed {
                    if let Err(e) = indexer.add_document(&doc, modified, size) {
                        error!("Watcher failed to index {}: {e}", path.display());
                        continue;
                    }
                    if let Err(e) = metadata_db.update_metadata(&path, modified, size, hash) {
                        error!(
                            "Watcher failed to update metadata for {}: {e}",
                            path.display()
                        );
                        continue;
                    }
                    written += 1;
                }
                written
            })
            .await;

            match write_result {
                Ok(0) => {}
                Ok(n) => {
                    needs_commit = true;
                    debug!("Watcher indexed {n} files");
                }
                Err(e) => error!("Watcher write task panicked: {e}"),
            }
        }

        if needs_commit
            && let Err(e) = indexer.commit()
        {
            error!("Watcher failed to commit index: {e}");
        }
    }

    fn is_excluded(path: &Path, globs: &GlobSet) -> bool {
        if globs.is_empty() {
            return false;
        }
        let path_str = path.to_string_lossy();
        path.components().any(|c| {
            let comp = c.as_os_str().to_string_lossy();
            globs.is_match(comp.as_ref())
        }) || globs.is_match(path_str.as_ref())
    }

    /// Re-parses a batch of files concurrently, preserving input order.
    ///
    /// Returns only the files that actually changed and still need indexing.
    async fn reindex_files(
        paths: &[PathBuf],
        metadata_db: &Arc<MetadataDb>,
        enable_ocr: bool,
    ) -> Vec<(PathBuf, u64, u64, [u8; 32], crate::parsers::ParsedDocument)> {
        // Cheap pre-filter: files whose size and mtime are unchanged cannot have
        // changed content, so they never need a parse. This is the vast majority
        // of the events a watcher sees (touched mtimes, access-time updates).
        let candidates: Vec<(PathBuf, u64, u64)> = {
            let stats: Vec<(bool, u64, u64)> = paths
                .iter()
                .map(|path| {
                    let Ok(meta) = std::fs::metadata(path) else {
                        return (false, 0, 0);
                    };
                    let modified = meta
                        .modified()
                        .unwrap_or(std::time::SystemTime::UNIX_EPOCH)
                        .duration_since(std::time::SystemTime::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs();
                    (true, modified, meta.len())
                })
                .collect();

            let paths_to_check: Vec<&Path> = paths
                .iter()
                .zip(&stats)
                .filter_map(|(p, (ok, _, _))| ok.then_some(p.as_path()))
                .collect();

            let stale = if paths_to_check.is_empty() {
                Vec::new()
            } else {
                let stamps: Vec<(u64, u64)> = stats
                    .iter()
                    .filter_map(|(ok, m, s)| ok.then_some((*m, *s)))
                    .collect();
                metadata_db
                    .batch_needs_reindex_paths_paths(&paths_to_check, &stamps)
                    .unwrap_or_else(|e| {
                        warn!("Watcher staleness check failed, re-indexing burst: {e}");
                        vec![true; paths_to_check.len()]
                    })
            };

            let mut stale_iter = stale.into_iter();
            let mut out = Vec::with_capacity(paths.len());
            for (path, (ok, modified, size)) in paths.iter().zip(stats) {
                if !ok {
                    continue;
                }
                if stale_iter.next().unwrap_or(true) {
                    out.push((path.clone(), modified, size));
                }
            }
            out
        };

        if candidates.is_empty() {
            return Vec::new();
        }

        let semaphore = Arc::new(tokio::sync::Semaphore::new(REINDEX_CONCURRENCY));
        let metadata_db = Arc::clone(metadata_db);

        let mut tasks = Vec::with_capacity(candidates.len());
        for (path, _modified, _size) in candidates {
            let semaphore = Arc::clone(&semaphore);
            let metadata_db = Arc::clone(&metadata_db);
            tasks.push(tokio::spawn(async move {
                let _permit = semaphore.acquire().await.ok()?;
                match Self::reindex_single_file(&path, &metadata_db, enable_ocr).await {
                    Ok(Some((doc, modified, size, hash))) => {
                        Some((path, modified, size, hash, doc))
                    }
                    Ok(None) => None,
                    Err(e) => {
                        warn!("Watcher error indexing {:?}: {e}", path);
                        None
                    }
                }
            }));
        }

        let mut out = Vec::with_capacity(tasks.len());
        for task in tasks {
            match task.await {
                Ok(Some(hit)) => out.push(hit),
                Ok(None) => {}
                Err(e) => error!("Watcher re-index task panicked: {e}"),
            }
        }
        out
    }

    /// Get a sender to push external events (like USN Journal) into the watcher
    #[must_use]
    pub fn event_tx(&self) -> mpsc::Sender<(PathBuf, WatcherAction)> {
        self.external_tx.clone()
    }

    /// Update the list of watched directories
    pub fn update_watch_list(&mut self, dirs: &[String]) -> Result<()> {
        let current_dirs: HashSet<String> = dirs.iter().cloned().collect();
        let existing_dirs: HashSet<String> = self.watchers.keys().cloned().collect();

        // Remove watchers for directories no longer in the list
        for dir in existing_dirs.difference(&current_dirs) {
            self.watchers.remove(dir);
        }

        // Add watchers for new directories
        for dir in current_dirs.difference(&existing_dirs) {
            let tx = self.external_tx.clone();
            let mut watcher = notify::recommended_watcher(move |res: notify::Result<Event>| {
                if let Ok(event) = res {
                    let action = match event.kind {
                        EventKind::Remove(_) => WatcherAction::Remove,
                        EventKind::Modify(_) | EventKind::Create(_) => WatcherAction::Index,
                        _ => return,
                    };
                    for path in &event.paths {
                        if tx.try_send((path.clone(), action)).is_err() {
                            let count = DROPPED_WATCHER_EVENTS.fetch_add(1, Ordering::Relaxed) + 1;
                            if count % 1000 == 1 {
                                warn!(
                                    "Watcher event queue full! Dropped {} events so far (latest: {:?})",
                                    count, path
                                );
                            }
                        }
                    }
                }
            })
            .map_err(|e| FlashError::Io(std::sync::Arc::new(std::io::Error::other(e))))?;

            let path = Path::new(dir);
            if !path.exists() {
                warn!("Not watching {}: path does not exist", path.display());
                continue;
            }
            watcher
                .watch(path, RecursiveMode::Recursive)
                .map_err(|e| FlashError::Io(std::sync::Arc::new(std::io::Error::other(e))))?;
            info!("Watching {} for changes", path.display());
            self.watchers.insert(dir.clone(), watcher);
        }

        Ok(())
    }

    // Returns parsed document data if file needs re-indexing
    async fn reindex_single_file(
        path: &Path,
        metadata_db: &Arc<MetadataDb>,
        enable_ocr: bool,
    ) -> Result<Option<(crate::parsers::ParsedDocument, u64, u64, [u8; 32])>> {
        if !path.exists() {
            return Ok(None);
        }

        let Ok(metadata) = std::fs::metadata(path) else {
            return Ok(None);
        };

        let modified = metadata
            .modified()
            .unwrap_or(std::time::SystemTime::UNIX_EPOCH)
            .duration_since(std::time::SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let size = metadata.len();

        let path_buf = path.to_path_buf();
        let (parsed, content_hash) =
            match crate::parsers::parse_file_with_hash(&path_buf, enable_ocr).await {
                Ok(res) => res,
                Err(e) => {
                    warn!("Failed to parse file {:?}: {}", path, e);
                    return Ok(None);
                }
            };

        // A watcher burst routinely re-requests the same file many times. If the
        // content hash is unchanged, skip the index write entirely.
        if let Ok(Some(existing)) = metadata_db.get_metadata(path)
            && existing.content_hash == content_hash
        {
            let _ = metadata_db.update_metadata(path, modified, size, content_hash);
            return Ok(None);
        }

        Ok(Some((parsed, modified, size, content_hash)))
    }
}

/// Compiles exclude patterns into a [`GlobSet`], logging and skipping bad ones.
///
/// Trailing slashes are stripped first. The shipped defaults are written the way
/// `.gitignore` expects (`.git/`, `node_modules/`, `target/`) and the walker
/// consumes them through `ignore`'s override builder, which understands the
/// trailing slash as "this directory". `globset` does not: `Glob::new("target/")`
/// compiles to the literal string `"target/"`, which never matches the path
/// component `"target"`. The watcher was therefore re-indexing every file under
/// `node_modules` and `target` while the scanner skipped them.
fn compile_globs(patterns: &[String]) -> GlobSet {
    let mut builder = GlobSetBuilder::new();
    for pattern in patterns {
        let normalized = pattern.trim().trim_end_matches(['/', '\\']);
        if normalized.is_empty() {
            continue;
        }
        match Glob::new(normalized) {
            Ok(glob) => {
                builder.add(glob);
            }
            Err(e) => warn!("Invalid exclude glob '{pattern}': {e}"),
        }
    }
    builder.build().unwrap_or_else(|e| {
        warn!("Failed to build exclude glob set: {e}");
        GlobSet::empty()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metadata::MetadataDb;
    use std::fs;
    use std::io::Write;
    use tempfile::tempdir;

    fn test_manager(dir: &Path) -> (Arc<IndexManager>, Arc<MetadataDb>, WatcherManager) {
        let indexer = Arc::new(IndexManager::open(dir, 64).unwrap());
        let metadata = Arc::new(MetadataDb::open(&dir.join("metadata.db")).unwrap().0);
        let watcher = WatcherManager::new(indexer.clone(), metadata.clone(), HashSet::new(), false);
        (indexer, metadata, watcher)
    }

    #[tokio::test]
    async fn watch_list_adds_and_removes() {
        let temp = tempdir().unwrap();
        let (_, _, mut watcher) = test_manager(temp.path());

        let watch_dir = temp.path().join("watch_me");
        fs::create_dir(&watch_dir).unwrap();

        assert!(
            watcher
                .update_watch_list(&[watch_dir.to_string_lossy().to_string()])
                .is_ok()
        );
        assert_eq!(watcher.watchers.len(), 1);

        assert!(watcher.update_watch_list(&[]).is_ok());
        assert!(watcher.watchers.is_empty());
    }

    #[tokio::test]
    async fn watch_list_skips_missing_directories() {
        let temp = tempdir().unwrap();
        let (_, _, mut watcher) = test_manager(temp.path());

        let missing = temp.path().join("does_not_exist");
        assert!(watcher.update_watch_list(&[missing.to_string_lossy().to_string()]).is_ok());
        assert!(watcher.watchers.is_empty());
    }

    #[tokio::test]
    async fn reindex_single_file_is_idempotent_on_unchanged_content() {
        let temp = tempdir().unwrap();
        let (_, metadata, _) = test_manager(temp.path());

        let file_path = temp.path().join("test.txt");
        fs::File::create(&file_path)
            .unwrap()
            .write_all(b"Initial content\n")
            .unwrap();

        let first = WatcherManager::reindex_single_file(&file_path, &metadata, false)
            .await
            .unwrap();
        let (doc, modified, size, hash) = first.expect("first pass must produce a document");
        assert_eq!(doc.content.trim(), "Initial content");
        metadata
            .update_metadata(&file_path, modified, size, hash)
            .unwrap();

        let second = WatcherManager::reindex_single_file(&file_path, &metadata, false)
            .await
            .unwrap();
        assert!(second.is_none(), "unchanged file must not be re-indexed");
    }

    #[tokio::test]
    async fn reindex_detects_content_change() {
        let temp = tempdir().unwrap();
        let (_, metadata, _) = test_manager(temp.path());

        let file_path = temp.path().join("test.txt");
        fs::File::create(&file_path).unwrap().write_all(b"one\n").unwrap();
        let (_, modified, size, hash) = WatcherManager::reindex_single_file(&file_path, &metadata, false)
            .await
            .unwrap()
            .unwrap();
        metadata
            .update_metadata(&file_path, modified, size, hash)
            .unwrap();

        // Change content *and* size so the cheap mtime/size pre-filter cannot
        // short-circuit the test.
        fs::write(&file_path, b"one two three four\n").unwrap();
        let second = WatcherManager::reindex_single_file(&file_path, &metadata, false)
            .await
            .unwrap();
        assert!(second.is_some(), "changed content must be re-indexed");
    }

    #[tokio::test]
    async fn burst_of_events_indexes_each_file_once() {
        let temp = tempdir().unwrap();
        let (indexer, metadata, _) = test_manager(temp.path());

        let source_dir = temp.path().join("src");
        fs::create_dir(&source_dir).unwrap();
        let file_a = source_dir.join("alpha.txt");
        let file_b = source_dir.join("beta.txt");
        fs::write(&file_a, b"alpha content\n").unwrap();
        fs::write(&file_b, b"beta content\n").unwrap();

        let config = WatcherConfig {
            allowed_extensions: ["txt".to_string()].into_iter().collect(),
            exclude_globs: Arc::new(GlobSet::empty()),
            enable_ocr: false,
        };

        // Simulate a noisy burst: every file modified several times.
        let events: Vec<(PathBuf, WatcherAction)> = [file_a.clone(), file_b.clone()]
            .into_iter()
            .flat_map(|p| {
                (0..5).map(move |_| {
                    (
                        p.clone(),
                        WatcherAction::Index,
                    )
                })
            })
            .collect();

        WatcherManager::process_events(events, &indexer, &metadata, &config).await;

        let results = indexer
            .search_blocking(
                crate::indexer::searcher::SearchParams::builder()
                    .query("content")
                    .limit(10)
                    .case_sensitive(false)
                    .build(),
            )
            .unwrap();
        assert_eq!(results.len(), 2, "each file must appear exactly once");
        assert!(
            metadata.get_metadata(&file_a).unwrap().is_some(),
            "metadata must be written for indexed files"
        );
    }

    #[tokio::test]
    async fn removals_are_applied_to_index_and_metadata() {
        let temp = tempdir().unwrap();
        let (indexer, metadata, _) = test_manager(temp.path());

        let file_path = temp.path().join("gone.txt");
        fs::write(&file_path, b"temporary\n").unwrap();

        let (doc, modified, size, hash) =
            WatcherManager::reindex_single_file(&file_path, &metadata, false)
                .await
                .unwrap()
                .unwrap();
        indexer.add_document(&doc, modified, size).unwrap();
        metadata.update_metadata(&file_path, modified, size, hash).unwrap();
        indexer.commit().unwrap();

        assert!(metadata.get_metadata(&file_path).unwrap().is_some());

        let config = WatcherConfig {
            allowed_extensions: ["txt".to_string()].into_iter().collect(),
            exclude_globs: Arc::new(GlobSet::empty()),
            enable_ocr: false,
        };
        WatcherManager::process_events(
            vec![(file_path.clone(), WatcherAction::Remove)],
            &indexer,
            &metadata,
            &config,
        )
        .await;

        assert!(
            metadata.get_metadata(&file_path).unwrap().is_none(),
            "metadata row must be removed"
        );
        let remaining = indexer
            .search_blocking(
                crate::indexer::searcher::SearchParams::builder()
                    .query("temporary")
                    .limit(10)
                    .case_sensitive(false)
                    .build(),
            )
            .unwrap();
        assert!(remaining.is_empty(), "document must be removed from the index");
    }

    #[tokio::test]
    async fn excluded_paths_are_ignored() {
        let temp = tempdir().unwrap();
        let (indexer, metadata, _) = test_manager(temp.path());

        let ignored = temp.path().join("node_modules");
        fs::create_dir(&ignored).unwrap();
        let file = ignored.join("dep.txt");
        fs::write(&file, b"dependency\n").unwrap();

        let globs = compile_globs(&["node_modules/".to_string()]);
        let config = WatcherConfig {
            allowed_extensions: ["txt".to_string()].into_iter().collect(),
            exclude_globs: Arc::new(globs),
            enable_ocr: false,
        };

        WatcherManager::process_events(
            vec![(file.clone(), WatcherAction::Index)],
            &indexer,
            &metadata,
            &config,
        )
        .await;

        assert!(
            metadata.get_metadata(&file).unwrap().is_none(),
            "excluded paths must never be indexed"
        );
    }

    #[tokio::test]
    async fn config_updates_take_effect_without_restart() {
        let temp = tempdir().unwrap();
        let (_, _, watcher) = test_manager(temp.path());

        watcher.update_allowed_extensions(&HashSet::from(["txt".to_string()]));
        assert!(watcher.config.load().allowed_extensions.contains("txt"));

        watcher.update_exclude_patterns(&["target/".to_string()]);
        assert!(watcher.config.load().exclude_globs.is_match("target"));

        watcher.set_enable_ocr(true);
        assert!(watcher.config.load().enable_ocr);

        // Excluded patterns must survive an extension update.
        watcher.update_allowed_extensions(&HashSet::from(["pdf".to_string()]));
        assert!(watcher.config.load().exclude_globs.is_match("target"));
    }
}
