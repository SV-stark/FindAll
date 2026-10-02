# Changelog

All notable changes to this project will be documented in this file.

## [0.17.0] - 2026-10-02

Indexes are rebuilt automatically on upgrade: the Tantivy schema version moved to
`2.1.0`. Local integrations that scripted the search endpoint need the IPC token
(see Security).

### Added
- Optional `ocr` cargo feature for scanned-PDF/image text extraction. Off by
  default because `xberg-tesseract` downloads and CMake-builds Leptonica and
  Tesseract from source, which would otherwise require network access, CMake, and
  a C/C++ toolchain on every machine that builds the project.
- Generation-based cancellation (`scanner::cancel`) that reliably stops indexing
  runs, including the blocking stages that `JoinHandle::abort` cannot reach.
- `IndexManager::search_blocking` and `IndexManager::remove_documents_batch` for
  callers already on a worker thread.
- `MetadataDb::paths_under` and `MetadataDb::remove_files` for bulk, single
  transaction directory removal.
- `CancelToken::check`, `IndexingReport`, and `WatchList` helpers for tooling.
- 62 new tests (94 total), covering the operator DSL, end-to-end search and
  filtering, the scanner pipeline, cancellation, watcher bursts, metadata
  batching, directory purging, filename-index persistence, and IPC token
  generation.

### Fixed
- **Silent index corruption.** The scanner's final flush was gated on
  `!doc_batch.is_empty()`, so cancelling an index run just after a batch
  boundary discarded every accumulated metadata row and filename entry and
  skipped the commit entirely.
- **Silent partial indexes.** All indexing write errors were discarded with
  `let _ =`; a failing Tantivy writer produced an incomplete index alongside an
  "All files indexed" status message. Errors are now logged, counted, and
  returned to the caller.
- **Lost index runs.** The index was only committed at the very end of a scan, so
  a crash, kill, or power loss during a multi-hour run discarded everything. It
  now commits every 4 batches.
- **Orphaned indexing runs.** Cancelling a run called `abort()` on its async task
  and immediately reset the shared cancel flag to `false`. The run's
  `spawn_blocking` stages cannot be aborted, so they kept writing into the index
  alongside the newly started run.
- **Every result showed a generic "FILE" badge.** The `extension` field was
  indexed as `STRING` but not `STORED`, so it always read back as `None` and the
  file-type badge and icon were never correct.
- **`path:` and `title:` operators did nothing.** They were parsed and stripped
  from the query text but never applied, so they silently returned unfiltered
  results. They are now applied per-document, with the query cache key extended
  to cover them.
- **Inconsistent query syntax.** Two divergent parsers handled the operator DSL,
  so the same query returned different results depending on whether it came from
  the window or from `--cli`. The UI copy's `modified:today` meant "last 24
  hours" rather than "since midnight". `ParsedQuery` is now the single source of
  truth.
- **Search crash on open-ended date filters.** The "no upper bound" timestamp
  sentinel overflowed inside Tantivy's `DateTime::from_timestamp_secs`, panicking
  the search thread for any query with only a lower date bound — which includes
  the sidebar's Modified filters. The release profile uses `panic = "abort"`, so
  this terminated the app.
- **Query corruption.** The parser removed operators with a per-operator
  `String::replace`, which was O(n²) and stripped the first *textual* match, so
  repeated operators ate unrelated words.
- **Repeated `ext:` operators overwrote each other** instead of forming an OR
  set.
- **Two instances could share one index.** When the lock was held but the
  recorded PID looked stale, the app logged "Continuing anyway" and started
  anyway. A stale lock is now reported and the app declines to start.
- **Ctrl-C did nothing.** It set a shutdown flag whose log message promised
  "committing index..." but which nothing read. Background threads are now signalled
  and the runtime is drained before exit.
- **Watcher excluded nothing.** The shipped exclude defaults carry a trailing
  slash (`.git/`, `target/`), which `globset` compiles as a literal and never
  matches. The watcher was re-indexing everything under `node_modules` and
  `target` while the scanner skipped it.
- **Watcher rebuilt wrong paths.** The USN journal watcher reconstructed every
  event as `C:\<name>`, correct only for files in the drive root, so events for
  subdirectory files were dropped or indexed under a bogus path.
- **Settings changes did not apply.** `custom_extensions` and
  `exclude_patterns` edits had no effect until restart, and the watcher disagreed
  with the scanner about which files to index. Both are now pushed to the live
  watcher on save.
- **Metadata database recovery failed on Windows.** `drop(db)` ran on a shadowing
  binding, leaving the real handle open, so renaming a corrupt database failed
  with "Access is denied" and the error was discarded.
- **Removing a folder could delete a sibling's index.** Prefix matching treated
  `docs-archive` as living under `docs`. Matching is now component-aware.
- **Selection of a stale result could panic.** The results list can shrink between
  a click being emitted and the message being handled.
- `format_date` no longer unwraps on the render path, and `run_ui` returns an
  error instead of panicking.

### Security
- The local search endpoint on `127.0.0.1:9095` was unauthenticated with no input
  length cap, so any local process could dump every indexed path. It now requires
  a per-user token from an owner-only file, caps query length, bounds concurrent
  connections, and backs off instead of spinning when `accept` fails.

### Changed
- **Symlinks are no longer followed** when scanning. `follow_links(true)` with no
  loop detection let a self-referential junction make a scan descend until the
  disk filled.
- **The walk depth limit of 20 was removed.** It silently omitted deep files —
  common inside `node_modules` — with no error anywhere.
- The filename index now publishes a single atomic snapshot. Previously the entry
  list and its FST lived in separate `ArcSwap`s updated in different orders, so a
  search could pair a fresh list with a stale FST.
- Filename-index persistence uses write-to-temp plus rename; an interrupted write
  can no longer leave an index that fails to load on the next start.
- Watcher bursts are collapsed per path and re-parsed with bounded concurrency,
  instead of one blocking document extraction per event, serially.
- Removing an indexed directory issues one batched delete and one transaction
  instead of one per file.
- Index size is measured in the background after each commit instead of walking
  the index directory on the UI thread during startup.
- Heavy documents are hashed by streaming through BLAKE3 rather than reading the
  whole file a second time.
- Removed dead code: the unused `fast_walker` module, an uncompiled test file, a
  cache that was written but never read, and structured outputs that were
  debug-formatted and discarded.
- `cargo clippy --all-targets` is clean under the configured `pedantic`, `nursery`,
  and `all` lint groups.

## [0.13.0] - 2026-06-15

### Added
- Pinned `kreuzberg` dependency to `=4.9.8` and `html-to-markdown-rs` to `3.5.7` to ensure stable compilation and resolve upstream type mismatch issues.

### Fixed
- Fixed all `cargo clippy` compiler warnings and errors under `-D warnings`.
- Resolved stack overflow risk by reducing large stack-allocated array buffers from 64KB to 16KB in file scanner and directory watcher.
- Eliminated redundant code structures, collapsed nested `if` statements, and simplified match patterns across the codebase.

### Changed
- Refactored `iced_ui` subscription event loop and hotkey registration to use idiomatic let-chains.
- Updated all other package dependencies to their latest safe/compatible versions.

## [0.2.0] - 2024-03-01

### Added
- Structured logging with tracing and log file rotation
- CLI `--version` flag
- Proper error handling instead of panics on startup
- CI test and clippy checks before build

### Fixed
- Replaced all `expect()`/`unwrap()` with proper error handling
- Standardized logging with tracing (replaced eprintln!/println!)
- Pinned litchi git dependency for reproducible builds
- NSIS uninstall hook now properly removes PATH entry

### Changed
- Updated version from 0.1.0 to 0.2.0
- Added tracing-appender for log rotation

## [0.1.0] - 2024-01-01

### Added
- Initial release
- Full-text search with Tantivy
- File metadata database with redb
- Iced-based UI
- Multiple file format parsers (PDF, DOCX, EPUB, etc.)
- File system watcher for live indexing
