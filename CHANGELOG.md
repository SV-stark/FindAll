# Changelog

All notable changes to this project will be documented in this file.

## [0.17.0] - 2026-10-03

Indexes are rebuilt automatically on upgrade: the Tantivy schema version moved to
`2.1.0`. Local integrations that scripted the search endpoint need the IPC token
(see Security).

### Added
- **Right-click menu on results.** Right-clicking a row emitted `ShowContextMenu`,
  which no handler matched, so the gesture did nothing. The menu offers Open,
  Show in folder, Copy full path, and Pin/Unpin.
- **Double-click to open a result.** Iced's `MouseArea` exposes only single- and
  right-click, so there was no pointer-driven way to open a file at all — only the
  Enter key worked. Double-clicks are now reconstructed by timing two presses on the
  same row, and the action is configurable (open / show in folder / preview only)
  via the previously inert `double_click_action` setting.
- **Recent searches on the start screen.** Search history was recorded and
  persisted but never surfaced anywhere, so the feature was invisible. Submitted
  queries are now ranked by frequency, listed on the welcome screen, clickable to
  re-run, and clearable. Turning the setting off also discards what was collected.
- **Working font-size setting.** `font_size` was persisted but never applied —
  every text size in the views is a literal. All 135 of them now route through
  `theme::fs`, so the Small/Medium/Large picker resizes the whole interface.
- **`Esc` key.** Dismisses the context menu, then the selection, then the query.
  Without it the context menu had no keyboard dismissal path.
- **`Ctrl+F` actually focuses the search box.** The shortcut was advertised on the
  welcome screen but no handler was bound to it.
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
- `system::startup::sync_auto_start` and `system::context_menu::sync_context_menu`,
  which reconcile the stored settings against the real OS registration at launch.
- 21 new tests (115 total), covering the operator DSL, end-to-end search and
  filtering, the scanner pipeline, cancellation, watcher bursts, metadata
  batching, directory purging, filename-index persistence, IPC token
  generation, settings migration, the settings-save path reaching the shared
  cache, and the new UI state transitions.

### Fixed
- **"Start automatically at boot" and "Add to the right-click menu" were lies.**
  Both checkboxes flipped a persisted setting and did nothing else — no registry
  write ever happened. They now perform the real registration on a blocking thread,
  revert the checkbox and surface the error if it fails, and are reconciled against
  the OS state at launch so the setting and reality cannot drift apart.
- **Settings text fields could not be cleared.** `max_results` was a `usize` written
  through on every keystroke, so an unparseable value (including empty) silently
  snapped the box back. `exclude_patterns` was worse: the view rendered
  `join(", ")` while the handler split on `,` and dropped empties, so typing a
  separator re-flowed the text under the cursor. Both now own their raw text and
  commit on submit, on Save, and on leaving the Settings tab. `max_results` is
  clamped to a usable band, and unparseable input keeps the previous value.
- **Settings could be written back over an unsaved edit.** `save_settings` took
  `&self`, so any unrelated toggle persisted the document while a typed-in value
  was still pending. Every write now folds the pending text in first.
- **UI settings saves never reached the running workers.** The UI's `save_settings`
  re-implemented persistence instead of calling `save_settings_internal`, and the
  partial copy skipped the `settings_cache` update and the watcher reconfiguration.
  Editing exclude patterns or custom extensions wrote them to disk while the
  scanner and watcher kept using their startup values, so the change appeared to do
  nothing until restart. Both now go through the single save path, and a failure is
  reported to the user rather than discarded.
- **Settings were not durable.** The save wrote a temp file and renamed it without
  flushing, so a power loss could leave an empty `settings.json`. It is now synced
  before the rename, and a failed rename cleans up the temp file.
- **Leaving the Settings tab discarded edits** made but not submitted.
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
- The Explorer context-menu registration silently wrote an empty `command` value if
  the executable path could not be resolved, leaving a broken verb in the shell. An
  unresolvable path is now an error.
- Selecting Filename mode while the filename index is disabled failed deep in the
  search layer with an opaque message. It is now refused up front, and the mode is
  left if the setting is turned off while it is active.

### Security
- The local search endpoint on `127.0.0.1:9095` was unauthenticated with no input
  length cap, so any local process could dump every indexed path. It now requires
  a per-user token from an owner-only file, caps query length, bounds concurrent
  connections, and backs off instead of spinning when `accept` fails.

### Changed
- **Removed dead settings that could not do anything.** `fuzzy_matching` and
  `show_file_extensions` were persisted but had no consumer and no unambiguous
  meaning; inventing semantics for them would have been worse than dropping them.
  `double_click_action`, `font_size`, `search_history_enabled`, and
  `filename_index_enabled` were equally inert but had obvious intent, so they are
  wired up instead. Existing `settings.json` files carrying the removed keys still
  load unchanged (covered by a test).
- **Collapsed duplicated persistence paths.** Search history and pinned files each
  had a dedicated command that re-implemented the ranking and wrote the whole
  settings document, racing the normal save path and double-counting entries. Both
  are now ordinary settings mutations.
- **Removed duplicate types.** `FilenameSearchResult` and `FilenameIndexStats` were
  declared twice — once in `models`, once in `indexer::filename_index` — with every
  hit converted field by field. The `models` copies are gone.
- **Removed dead code:** the orphan `commands/autostart.rs` (a second, unused
  registry implementation next to `system/startup`), `system/compression.rs` and
  its `zstd` dependency, the unreferenced `self_update` installer, the folder
  picker that `Message::PickFolder` had superseded, the unused
  `parsers::{list_supported_extensions, is_supported_file}`, `TermHighlighter`,
  `models::RecentFile`, and five message variants that were handled but never
  emitted.
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
- The welcome screen's shortcut and feature cards were extracted into
  `shortcut_and_feature_cards`, and the shortcut list now documents the
  double-click and `Esc` gestures rather than omitting them.
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
