#![allow(clippy::large_futures)]

use flash_search::error::Result;
use flash_search::indexer::searcher::SearchParams;
use flash_search::{indexer::IndexManager, metadata::MetadataDb};
use std::fs;
use std::sync::Arc;
use tempfile::tempdir;

#[tokio::test]
async fn test_end_to_end_search() -> Result<()> {
    let temp_workspace = tempdir()?;
    let index_dir = temp_workspace.path().join("index");
    let data_dir = temp_workspace.path().join("data");
    let settings_dir = temp_workspace.path().join("settings");

    fs::create_dir_all(&index_dir)?;
    fs::create_dir_all(&data_dir)?;
    fs::create_dir_all(&settings_dir)?;

    let txt_path = data_dir.join("hello.txt");
    fs::write(&txt_path, "This is a unique test string for searching.")?;

    let md_path = data_dir.join("notes.md");
    fs::write(
        &md_path,
        "# Notes\n\nSome markdown content with unique keyword: flashsearchintegrationtest",
    )?;

    let indexer = Arc::new(IndexManager::open(&index_dir, 100)?);
    let metadata_db_path = index_dir.join("metadata.redb");
    let _metadata_db = Arc::new(MetadataDb::open(&metadata_db_path)?.0);

    let txt_doc = flash_search::parsers::parse_file(&txt_path, false).await?;
    let md_doc = flash_search::parsers::parse_file(&md_path, false).await?;

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();

    indexer.add_document(&txt_doc, now, 100)?;
    indexer.add_document(&md_doc, now, 200)?;
    indexer.commit()?;

    // Commits are deterministic and immediate via searcher.reload() - no sleep needed!
    let results = indexer
        .search(
            SearchParams::builder()
                .query("unique test string")
                .limit(10)
                .case_sensitive(false)
                .build(),
        )
        .await?;
    assert_eq!(results.len(), 1);
    assert!(results[0].file_path.contains("hello.txt"));
    // Since schema 3.0.0 `content` is not STORED, so the indexer cannot produce a
    // snippet. Snippets are re-extracted from disk by `flash_search::snippet`,
    // which `commands::search_query_internal` drives. Asserting on the indexer's
    // empty snippets here pins that split so it cannot regress silently.
    assert!(
        results[0].snippets.is_empty(),
        "the indexer must not synthesize snippets now that content is not stored"
    );

    // The on-disk snippet path must still find the term and highlight it.
    let outcome = flash_search::snippet::generate_snippets(
        &results[0].file_path,
        None,
        "unique test string",
        false,
        false,
    )
    .await;
    assert!(
        !outcome.snippets.is_empty(),
        "snippets must be rebuilt from the file on disk"
    );
    assert!(
        outcome.snippets[0].contains("<b>"),
        "rebuilt snippet should highlight the match, got: {:?}",
        outcome.snippets[0]
    );

    let results = indexer
        .search(
            SearchParams::builder()
                .query("flashsearchintegrationtest")
                .limit(10)
                .case_sensitive(false)
                .build(),
        )
        .await?;
    assert_eq!(results.len(), 1);
    assert!(results[0].file_path.contains("notes.md"));

    Ok(())
}

#[tokio::test]
async fn test_corrupt_db_preservation() -> Result<()> {
    let temp_workspace = tempdir()?;
    let db_path = temp_workspace.path().join("metadata.redb");

    // Write arbitrary garbage bytes to simulate page corruption
    fs::write(&db_path, b"CORRUPTED_GARBAGE_BYTES_PAGE_HEADER_CORRUPT")?;

    let (db, corrupted) = MetadataDb::open(&db_path)?;
    assert!(corrupted, "Database must be detected as corrupted");

    // Verify the corrupt file was preserved to a timestamped backup instead of wiped
    let mut preserved_files = 0;
    for entry in fs::read_dir(temp_workspace.path())? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().to_string();
        if name.contains("corrupt") {
            preserved_files += 1;
        }
    }
    assert!(
        preserved_files >= 1,
        "Corrupt DB must be preserved with a timestamped backup"
    );

    // Verify the fresh DB works properly
    assert!(
        db.get_metadata(std::path::Path::new("dummy.txt"))?
            .is_none()
    );

    Ok(())
}

// ---------------------------------------------------------------------------
// Metadata database
// ---------------------------------------------------------------------------

#[test]
fn test_metadata_round_trip_and_staleness() -> Result<()> {
    let temp = tempdir()?;
    let (db, _) = MetadataDb::open(&temp.path().join("metadata.redb"))?;
    let path = temp.path().join("a.txt");
    fs::write(&path, "content")?;
    let hash = [7u8; 32];

    assert!(db.needs_reindex(&path, 100, 10)?, "unknown file is stale");

    db.update_metadata(&path, 100, 10, hash)?;
    assert!(
        !db.needs_reindex(&path, 100, 10)?,
        "unchanged file is fresh"
    );
    assert!(db.needs_reindex(&path, 200, 10)?, "new mtime is stale");
    assert!(db.needs_reindex(&path, 100, 11)?, "new size is stale");

    let stored = db.get_metadata(&path)?.expect("row must be readable");
    assert_eq!(stored.content_hash, hash);
    assert_eq!(stored.size, 10);

    assert!(db.remove_file(&path)?);
    assert!(
        !db.remove_file(&path)?,
        "second remove reports nothing removed"
    );
    assert!(db.get_metadata(&path)?.is_none());

    Ok(())
}

#[test]
fn test_metadata_batch_operations_agree_with_single_ops() -> Result<()> {
    let temp = tempdir()?;
    let (db, _) = MetadataDb::open(&temp.path().join("metadata.redb"))?;

    let entries: Vec<(std::path::PathBuf, u64, u64)> = (0..50)
        .map(|i| (temp.path().join(format!("f{i}.txt")), 1000 + i, 10 + i))
        .collect();
    for (path, _, _) in &entries {
        fs::write(path, "x")?;
    }

    let stamps: Vec<(u64, u64)> = entries.iter().map(|(_, m, s)| (*m, *s)).collect();
    let borrowed: Vec<&std::path::Path> = entries.iter().map(|(p, _, _)| p.as_path()).collect();
    let as_strings: Vec<(String, u64, u64)> = entries
        .iter()
        .map(|(p, m, s)| (p.to_string_lossy().to_string(), *m, *s))
        .collect();

    assert!(
        db.batch_needs_reindex(&as_strings)?
            .iter()
            .all(|stale| *stale)
    );
    assert!(
        db.batch_needs_reindex_paths_paths(&borrowed, &stamps)?
            .iter()
            .all(|stale| *stale)
    );

    db.batch_update_metadata(
        &as_strings
            .iter()
            .map(|(p, m, s)| (p.clone(), *m, *s, [1u8; 32]))
            .collect::<Vec<_>>(),
    )?;

    assert!(db.batch_needs_reindex(&as_strings)?.iter().all(|s| !s));
    assert!(db.batch_needs_reindex_paths(&entries)?.iter().all(|s| !s));

    // Bump one file's mtime; only that entry goes stale.
    let mut bumped = entries.clone();
    bumped[7].1 += 1;
    let stale = db.batch_needs_reindex_paths(&bumped)?;
    assert!(stale[7]);
    assert_eq!(stale.iter().filter(|s| **s).count(), 1);

    assert!(db.batch_needs_reindex_paths(&[])?.is_empty());

    Ok(())
}

#[test]
fn test_metadata_paths_under_and_remove_files() -> Result<()> {
    let temp = tempdir()?;
    let (db, _) = MetadataDb::open(&temp.path().join("metadata.redb"))?;

    let root = temp.path().join("docs");
    let other = temp.path().join("other");
    fs::create_dir_all(&root)?;
    fs::create_dir_all(&other)?;

    let mut rows = Vec::new();
    for i in 0..10 {
        let p = root.join(format!("f{i}.txt"));
        fs::write(&p, "x")?;
        rows.push((p.to_string_lossy().to_string(), 1_000u64, 1u64, [0u8; 32]));
    }
    let other_path = other.join("keep.txt");
    fs::write(&other_path, "x")?;
    rows.push((
        other_path.to_string_lossy().to_string(),
        1_000,
        1,
        [0u8; 32],
    ));
    // A sibling directory whose name shares the prefix must not be swept in.
    let sibling = temp.path().join("docs-archive");
    fs::create_dir_all(&sibling)?;
    let sibling_path = sibling.join("old.txt");
    fs::write(&sibling_path, "x")?;
    rows.push((
        sibling_path.to_string_lossy().to_string(),
        1_000,
        1,
        [0u8; 32],
    ));

    db.batch_update_metadata(&rows)?;

    let under = db.paths_under(&root)?;
    assert_eq!(under.len(), 10, "prefix match must not include siblings");
    assert!(
        under
            .iter()
            .all(|p| p.starts_with(&root.to_string_lossy().to_string()))
    );

    let borrowed: Vec<&std::path::Path> = under.iter().map(std::path::Path::new).collect();
    assert_eq!(db.remove_files(&borrowed)?, 10);
    assert_eq!(db.remove_files(&[])?, 0);

    assert!(db.paths_under(&root)?.is_empty());
    assert!(db.get_metadata(&sibling_path)?.is_some());
    assert!(db.get_metadata(&other_path)?.is_some());

    Ok(())
}

// ---------------------------------------------------------------------------
// Scanner: the indexing pipeline end to end
// ---------------------------------------------------------------------------

fn test_settings() -> flash_search::settings::AppSettings {
    flash_search::settings::AppSettings {
        index_file_size_limit_mb: 100,
        indexing_threads: 2,
        auto_index_on_startup: false,
        exclude_patterns: vec!["skipme/".to_string()],
        exclude_folders: vec![],
        ..Default::default()
    }
}

fn build_scanner(
    root: &std::path::Path,
    settings: flash_search::settings::AppSettings,
) -> Result<(
    Arc<flash_search::scanner::Scanner>,
    Arc<IndexManager>,
    Arc<MetadataDb>,
)> {
    let indexer = Arc::new(IndexManager::open(&root.join("index"), 64)?);
    let metadata_db = Arc::new(MetadataDb::open(&root.join("metadata.redb"))?.0);
    // The scanner reads a shared live settings cell, so a test that wants to
    // change settings mid-run must hold the same `Arc` and store into it.
    let settings_cache = Arc::new(arc_swap::ArcSwap::from_pointee(settings));
    let scanner = Arc::new(flash_search::scanner::Scanner::new(
        indexer.clone(),
        metadata_db.clone(),
        None,
        None,
        Arc::clone(&settings_cache),
    ));
    Ok((scanner, indexer, metadata_db))
}

#[tokio::test(flavor = "multi_thread")]
async fn test_scan_directory_indexes_and_is_idempotent() -> Result<()> {
    let temp = tempdir()?;
    let data = temp.path().join("data");
    fs::create_dir_all(data.join("nested"))?;
    fs::write(data.join("a.txt"), "alpha uniqueword")?;
    fs::write(data.join("nested/b.md"), "beta uniqueword")?;
    // Extensions outside the allow-list must be skipped.
    fs::write(data.join("c.bin"), "gamma uniqueword")?;

    let (scanner, indexer, metadata_db) = build_scanner(temp.path(), test_settings())?;
    let cancel = flash_search::scanner::cancel::CancelToken::never();

    let report = scanner
        .scan_directory(data.clone(), vec![], cancel.clone())
        .await?;

    assert!(!report.cancelled);
    assert_eq!(report.write_errors, 0, "scan must not report write errors");
    assert_eq!(
        report.documents_written, 2,
        "only supported types are indexed"
    );

    let results = indexer
        .search(
            SearchParams::builder()
                .query("uniqueword")
                .limit(10)
                .case_sensitive(false)
                .build(),
        )
        .await?;
    assert_eq!(results.len(), 2);
    assert!(
        results.iter().all(|r| r.extension.is_some()),
        "extension must round-trip through the schema"
    );

    // Metadata rows must exist for exactly the indexed files.
    assert!(metadata_db.get_metadata(&data.join("a.txt"))?.is_some());
    assert!(metadata_db.get_metadata(&data.join("c.bin"))?.is_none());

    // Second scan: nothing changed, so nothing is re-indexed.
    let second = scanner.scan_directory(data, vec![], cancel).await?;
    assert_eq!(
        second.documents_written, 0,
        "unchanged files must not be re-indexed"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_scan_honours_exclude_patterns() -> Result<()> {
    let temp = tempdir()?;
    let data = temp.path().join("data");
    fs::create_dir_all(data.join("skipme"))?;
    fs::create_dir_all(data.join("keep"))?;
    fs::write(data.join("skipme/hidden.txt"), "excludeme uniqueword")?;
    fs::write(data.join("keep/shown.txt"), "includeme uniqueword")?;

    let (scanner, indexer, _) = build_scanner(temp.path(), test_settings())?;
    let report = scanner
        .scan_directory(
            data,
            vec!["skipme/".to_string()],
            flash_search::scanner::cancel::CancelToken::never(),
        )
        .await?;

    assert_eq!(report.documents_written, 1);

    let results = indexer
        .search(
            SearchParams::builder()
                .query("uniqueword")
                .limit(10)
                .case_sensitive(false)
                .build(),
        )
        .await?;
    assert_eq!(results.len(), 1);
    assert!(results[0].file_path.contains("shown.txt"));

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_scan_skips_files_over_the_size_limit() -> Result<()> {
    let temp = tempdir()?;
    let data = temp.path().join("data");
    fs::create_dir_all(&data)?;
    fs::write(data.join("small.txt"), "tiny")?;
    fs::write(data.join("big.txt"), vec![b'x'; 4096])?;

    let mut settings = test_settings();
    settings.index_file_size_limit_mb = 0; // 0 MiB => nothing fits
    let (scanner, _, _) = build_scanner(temp.path(), settings)?;
    let report = scanner
        .scan_directory(
            data,
            vec![],
            flash_search::scanner::cancel::CancelToken::never(),
        )
        .await?;

    assert_eq!(report.documents_written, 0);

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_cancelled_scan_flushes_what_it_already_wrote() -> Result<()> {
    // Regression: the final flush was gated on `!doc_batch.is_empty()`, so
    // cancelling right after a batch boundary threw away the accumulated
    // metadata rows and never committed.
    let temp = tempdir()?;
    let data = temp.path().join("data");
    fs::create_dir_all(&data)?;
    for i in 0..5 {
        fs::write(
            data.join(format!("f{i}.txt")),
            format!("content uniqueword {i}"),
        )?;
    }

    let (scanner, indexer, metadata_db) = build_scanner(temp.path(), test_settings())?;

    let control = flash_search::scanner::cancel::CancellationController::new();
    let token = control.begin();
    // Cancel before the scan starts; the walker stops immediately.
    control.cancel();

    let probe = data.join("f0.txt");
    let report = scanner.scan_directory(data, vec![], token).await?;
    assert!(report.cancelled, "report must reflect cancellation");

    // Even with nothing written, the index must still be committed and usable.
    let results = indexer
        .search(
            SearchParams::builder()
                .query("uniqueword")
                .limit(10)
                .case_sensitive(false)
                .build(),
        )
        .await?;
    assert_eq!(results.len(), 0);
    assert!(metadata_db.get_metadata(&probe)?.is_none());

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_a_new_scan_supersedes_the_previous_one() -> Result<()> {
    // Regression: `JoinHandle::abort` cannot stop `spawn_blocking` stages, so the
    // old shared `AtomicBool` was reset to `false` by the new run while the old
    // run's stages were still alive and writing.
    let control = flash_search::scanner::cancel::CancellationController::new();
    let first = control.begin();
    let second = control.begin();
    assert!(first.is_cancelled());
    assert!(!second.is_cancelled());

    let data = tempdir()?;
    let (scanner, _, _) = build_scanner(data.path(), test_settings())?;
    fs::create_dir_all(data.path().join("src"))?;
    fs::write(data.path().join("src/a.txt"), "uniqueword")?;

    let report = scanner
        .scan_directory(data.path().join("src"), vec![], second.clone())
        .await?;
    assert!(!report.cancelled);
    assert_eq!(report.documents_written, 1);

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_watched_edits_reach_the_index() -> Result<()> {
    // Covers the incremental path: index a file, then push an update through the
    // watcher processor and confirm the new content is searchable.
    let temp = tempdir()?;
    let data = temp.path().join("data");
    fs::create_dir_all(&data)?;
    let file = data.join("note.txt");
    fs::write(&file, "original uniqueterm")?;

    let indexer = Arc::new(IndexManager::open(&temp.path().join("index"), 64)?);
    let metadata_db = Arc::new(MetadataDb::open(&temp.path().join("metadata.redb"))?.0);

    let watcher = flash_search::watcher::WatcherManager::new(
        indexer.clone(),
        metadata_db.clone(),
        std::collections::HashSet::from(["txt".to_string()]),
        false,
    );

    // Initial index.
    watcher
        .event_tx()
        .send((file.clone(), flash_search::watcher::WatcherAction::Index))
        .await
        .ok();
    // Debounce window is 500 ms; give the processor time to flush.
    tokio::time::sleep(std::time::Duration::from_millis(1_200)).await;

    let results = indexer
        .search(
            SearchParams::builder()
                .query("uniqueterm")
                .limit(10)
                .case_sensitive(false)
                .build(),
        )
        .await?;
    assert_eq!(results.len(), 1);

    // Modify, then confirm the new term appears and the old one does not.
    fs::write(&file, "replaced brandnewterm")?;
    watcher
        .event_tx()
        .send((file.clone(), flash_search::watcher::WatcherAction::Index))
        .await
        .ok();
    tokio::time::sleep(std::time::Duration::from_millis(1_200)).await;

    let results = indexer
        .search(
            SearchParams::builder()
                .query("brandnewterm")
                .limit(10)
                .case_sensitive(false)
                .build(),
        )
        .await?;
    assert_eq!(results.len(), 1, "updated content must be searchable");

    // Deleting the file removes it from the index.
    fs::remove_file(&file)?;
    watcher
        .event_tx()
        .send((file.clone(), flash_search::watcher::WatcherAction::Remove))
        .await
        .ok();
    tokio::time::sleep(std::time::Duration::from_millis(1_200)).await;

    let results = indexer
        .search(
            SearchParams::builder()
                .query("brandnewterm")
                .limit(10)
                .case_sensitive(false)
                .build(),
        )
        .await?;
    assert!(results.is_empty(), "deleted files must leave the index");

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_purge_directory_removes_only_that_subtree() -> Result<()> {
    let temp = tempdir()?;
    let data = temp.path().join("data");
    fs::create_dir_all(data.join("docs"))?;
    fs::create_dir_all(data.join("pics"))?;
    fs::write(data.join("docs/a.txt"), "docword uniqueword")?;
    fs::write(data.join("pics/b.txt"), "picword uniqueword")?;

    let (scanner, indexer, metadata_db) = build_scanner(temp.path(), test_settings())?;
    scanner
        .scan_directory(
            data.clone(),
            vec![],
            flash_search::scanner::cancel::CancelToken::never(),
        )
        .await?;

    let docs = data.join("docs");
    let indexer_bg = indexer.clone();
    let metadata_bg = metadata_db.clone();
    let purged = tokio::task::spawn_blocking(move || {
        let paths = metadata_bg.paths_under(&docs)?;
        let borrowed: Vec<&std::path::Path> = paths.iter().map(std::path::Path::new).collect();
        metadata_bg.remove_files(&borrowed)?;
        indexer_bg.remove_documents_batch(&paths)?;
        indexer_bg.commit()?;
        Ok::<usize, flash_search::error::FlashError>(paths.len())
    })
    .await
    .expect("purge task must not panic")?;
    assert_eq!(purged, 1);

    let results = indexer
        .search(
            SearchParams::builder()
                .query("uniqueword")
                .limit(10)
                .case_sensitive(false)
                .build(),
        )
        .await?;
    assert_eq!(results.len(), 1);
    assert!(results[0].file_path.contains("pics"));
    // Join one component at a time: the index stores the scanner's own path
    // formatting, so a hard-coded separator would not round-trip on Windows.
    let pic = data.join("pics").join("b.txt");
    assert!(metadata_db.get_metadata(&pic)?.is_some());

    Ok(())
}

/// `App::save_settings` used to re-implement the save and skip the
/// `settings_cache` update and the watcher reconfiguration. This asserts the
/// save path every UI toggle goes through actually reaches the shared cache
/// that the scanner and watcher read from.
#[tokio::test]
async fn test_save_settings_reaches_the_shared_cache() -> Result<()> {
    use flash_search::commands::AppState;
    use flash_search::scanner::{ProgressEvent, Scanner};
    use flash_search::settings::{AppSettings, SettingsManager};
    use flash_search::watcher::WatcherManager;

    let temp_workspace = tempdir()?;
    let index_dir = temp_workspace.path().join("index");
    // redb creates a *file*, so the metadata DB cannot live inside the
    // directory Tantivy owns.
    let metadata_db_path = temp_workspace.path().join("metadata.redb");
    let settings_dir = temp_workspace.path().join("settings");
    let data_dir = temp_workspace.path().join("data");

    fs::create_dir_all(&index_dir)?;
    fs::create_dir_all(&settings_dir)?;
    fs::create_dir_all(&data_dir)?;

    let indexer = Arc::new(IndexManager::open(&index_dir, 100)?);
    let metadata_db = Arc::new(MetadataDb::open(&metadata_db_path)?.0);

    let base_settings = AppSettings::default();
    let (progress_tx, _progress_rx) = flume::bounded::<ProgressEvent>(64);
    let watcher = WatcherManager::new(
        Arc::clone(&indexer),
        Arc::clone(&metadata_db),
        base_settings.get_allowed_extensions().clone(),
        base_settings.enable_ocr,
    );
    let settings_manager = SettingsManager::new(&settings_dir);
    // One live settings cell shared by the scanner and the app state, mirroring
    // how `setup_app` wires them.
    let settings_cache = Arc::new(arc_swap::ArcSwap::from_pointee(base_settings));
    let scanner = Arc::new(Scanner::new(
        Arc::clone(&indexer),
        Arc::clone(&metadata_db),
        None,
        Some(progress_tx.clone()),
        Arc::clone(&settings_cache),
    ));

    let state = Arc::new(
        AppState::builder()
            .indexer(indexer)
            .metadata_db(metadata_db)
            .settings_cache(Arc::clone(&settings_cache))
            .settings_manager(SettingsManager::new(&settings_dir))
            .watcher(watcher)
            .progress_tx(progress_tx)
            .scanner(scanner)
            .build(),
    );

    let mut updated = state.settings_cache.load().as_ref().clone();
    updated.exclude_patterns = vec!["node_modules".to_string()];
    updated.index_dirs = vec![data_dir.to_string_lossy().to_string()];

    flash_search::commands::save_settings_internal(&updated, &state)
        .map_err(|e| flash_search::error::FlashError::config("save_settings", e))?;

    // The scanner reads its configuration from this cache on every run, so a
    // save that skipped it left indexing using the startup values.
    assert_eq!(
        state.settings_cache.load().exclude_patterns,
        vec!["node_modules".to_string()],
        "saved exclude patterns must reach the shared cache"
    );
    assert_eq!(state.settings_cache.load().index_dirs.len(), 1);

    // And the change must survive a restart, not just live in memory.
    let reloaded = settings_manager.load()?;
    assert_eq!(reloaded.exclude_patterns, vec!["node_modules".to_string()]);

    Ok(())
}
