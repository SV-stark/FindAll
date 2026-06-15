# Changelog

All notable changes to this project will be documented in this file.

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
