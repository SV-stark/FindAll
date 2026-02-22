# Changelog

All notable changes to this project will be documented in this file.

## [0.2.0] - 2024

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
