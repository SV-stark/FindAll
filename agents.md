# AGENTS.md - AI Coding Agent Instructions

## Project Overview

**Flash Search** is an ultrafast local full-text search application built purely with Rust. This document provides comprehensive instructions for AI coding agents working on this codebase.

## Architecture Overview

┌─────────────────────────────────────────────────────────────┐
│                     Flash Search                             │
├─────────────────────────────────────────────────────────────┤
│  UI (Iced)                                                  │
│  ├── iced_ui/       - High-performance GUI components       │
│  ├── search.rs      - Search view and logic bindings        │
│  └── settings.rs    - Settings view and logic bindings      │
├─────────────────────────────────────────────────────────────┤
│  Rust Backend (Glue & Logic)                                │
│  ├── commands/      - Business logic handlers               │
│  └── state/         - Application state management          │
├─────────────────────────────────────────────────────────────┤
│  Core Engine                                                │
│  ├── indexer/       - Tantivy search engine wrapper         │
│  ├── parsers/       - File format parsers                   │
│  ├── scanner/       - File system crawler                   │
│  └── metadata/      - redb database operations              │
└─────────────────────────────────────────────────────────────┘

## Project Structure

```
flash-search/
├── src/
│   ├── main.rs              # Application entry point
│   ├── lib.rs               # Library exports
│   ├── error.rs             # Error types
│   ├── models.rs            # Data models
│   ├── settings.rs          # Settings management
│   ├── commands/            # Business logic (AppState)
│   ├── iced_ui/             # Iced UI implementation
│   │   ├── mod.rs           # UI entry and core state
│   │   ├── search.rs        # Search screen UI
│   │   └── settings.rs      # Settings screen UI
│   ├── parsers/
│   │   ├── mod.rs           # Parser dispatch
│   │   ├── docx.rs          # DOCX parser
│   │   └── ...              # Other parsers
│   ├── indexer/
│   │   ├── mod.rs           # Indexer module
│   │   ├── schema.rs        # Tantivy schema
│   │   └── searcher.rs      # Query execution
│   └── metadata/
│       ├── mod.rs           # Metadata DB interface
│       └── db.rs            # redb definitions
├── Cargo.toml               # Project dependencies
└── README.md
```

## Critical Implementation Guidelines

### 1. Performance First

**This project prioritizes performance over convenience.**

- **Memory**: Keep RAM usage under 30MB at idle
- **Speed**: Search results must return in <50ms
- **I/O**: Use memory-mapped files for parsing
- **UI Responsiveness**: Never block the Iced event loop

**DON'Ts:**
- Don't perform heavy computations in Iced `update` methods (use `Command::perform` or `tokio::spawn`)
- Don't load large datasets into the UI at once (use pagination or lazy loading if needed)

### 2. Error Handling

Use `anyhow` for error propagation and `thiserror` for custom error types:

```rust
// Good
use anyhow::{Context, Result};

pub fn parse_file(path: &Path) -> Result<String> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read file: {}", path.display()))?;
    Ok(content)
}

// Custom errors for specific failure modes
#[derive(Error, Debug)]
pub enum ParseError {
    #[error("Unsupported file format: {0}")]
    UnsupportedFormat(String),
    #[error("File corrupted or encrypted")]
    CorruptedFile,
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}
```

### 3. Concurrency Model

```rust
// File watching (async I/O) -> Tokio
// File parsing (CPU-bound) -> Rayon
// Search queries -> Tantivy (already parallel)

// Example: Hybrid approach
use rayon::prelude::*;
use tokio::task;

pub async fn index_directory(path: PathBuf) -> Result<()> {
    // Walk directory (async I/O)
    let files = walk_directory(&path).await?;
    
    // Parse files in parallel (CPU-bound)
    let results: Vec<_> = files
        .par_iter()
        .map(|file| parse_file(file))
        .collect();
    
    Ok(())
}
```

### 4. Parser Implementation

#### DOCX Parser (Critical Performance Path)

```rust
use memmap2::Mmap;
use quick_xml::events::Event;
use quick_xml::Reader;
use zip::ZipArchive;

pub fn parse_docx(path: &Path) -> Result<ParsedDocument> {
    // Memory map the file
    let file = std::fs::File::open(path)?;
    let mmap = unsafe { Mmap::map(&file)? };
    
    // Stream from memory
    let cursor = std::io::Cursor::new(&mmap[..]);
    let mut archive = ZipArchive::new(cursor)?;
    
    // Extract document.xml
    let mut doc_xml = archive.by_name("word/document.xml")?;
    let mut xml_content = String::new();
    doc_xml.read_to_string(&mut xml_content)?;
    
    // Stream parse XML (NO DOM!)
    let mut reader = Reader::from_str(&xml_content);
    let mut buf = Vec::new();
    let mut text = String::new();
    
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) if e.name().as_ref() == b"w:t" => {
                // Extract text content
                if let Ok(txt) = reader.read_text(e.name()) {
                    text.push_str(&txt);
                    text.push(' ');
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(e.into()),
            _ => (),
        }
        buf.clear();
    }
    
    Ok(ParsedDocument {
        path: path.to_string_lossy().to_string(),
        content: text,
        title: extract_title(&mut archive)?,
    })
}
```

### 5. Tantivy Schema

```rust
use tantivy::schema::*;

pub fn create_schema() -> Schema {
    let mut schema_builder = Schema::builder();
    
    // Store file path for retrieval
    schema_builder.add_text_field(
        "file_path",
        STRING | STORED
    );
    
    // Index content but don't store (retrieved from disk on demand)
    let text_options = TextOptions::default()
        .set_indexing_options(
            TextFieldIndexing::default()
                .set_tokenizer("default")
                .set_index_option(IndexRecordOption::WithFreqsAndPositions)
        )
        .set_stored(false);
    schema_builder.add_text_field("content", text_options);
    
    // Store title for display
    schema_builder.add_text_field(
        "title",
        TEXT | STORED
    );
    
    // Index timestamp for sorting
    schema_builder.add_date_field(
        "modified",
        FAST | INDEXED
    );
    
    schema_builder.build()
}
```

### 6. Metadata Database (redb)

```rust
use redb::{Database, TableDefinition};

const FILES_TABLE: TableDefinition<&str, FileMetadata> = TableDefinition::new("files");

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FileMetadata {
    pub path: String,
    pub modified: u64,      // Unix timestamp
    pub size: u64,
    pub content_hash: [u8; 32], // blake3 hash
    pub indexed_at: u64,
}

pub struct MetadataDb {
    db: Database,
}

impl MetadataDb {
    pub fn needs_reindex(&self, path: &Path, modified: u64) -> Result<bool> {
        let txn = self.db.begin_read()?;
        let table = txn.open_table(FILES_TABLE)?;
        
        match table.get(path.to_str().unwrap())? {
            Some(metadata) => {
                let meta = metadata.value();
                Ok(meta.modified != modified)
            }
            None => Ok(true), // File not indexed yet
        }
    }
}
```

### 7. Iced Integration (UI Glue)

```rust
use iced::{Application, Command, Element};

pub struct SearchApp {
    state: Arc<AppState>,
    search_query: String,
    results: Vec<FileItem>,
}

#[derive(Debug, Clone)]
pub enum Message {
    SearchInputChanged(String),
    PerformSearch,
    SearchResultsReceived(Vec<FileItem>),
}

impl Application for SearchApp {
    type Message = Message;
    // ...

    fn update(&mut self, message: Message) -> Command<Message> {
        match message {
            Message::SearchInputChanged(query) => {
                self.search_query = query;
                Command::none()
            }
            Message::PerformSearch => {
                let state = self.state.clone();
                let query = self.search_query.clone();
                
                // Offload search to async command
                Command::perform(async move {
                    let results = state.indexer.search(&query).await.unwrap_or_default();
                    results.into_iter().map(FileItem::from).collect()
                }, Message::SearchResultsReceived)
            }
            Message::SearchResultsReceived(results) => {
                self.results = results;
                Command::none()
            }
        }
    }

    fn view(&self) -> Element<Message> {
        // ... Iced view layout construction ...
        iced::widget::text("Search View").into()
    }
}
```

### 8. Iced Component Patterns

Follow the standard Elm architecture when creating view components in Iced. Keep state isolated where appropriate and fire clear descriptive messages.

## Common Tasks

### Adding a New File Parser

1. Create `src/parsers/<format>.rs`
2. Implement `parse_<format>(path: &Path) -> Result<ParsedDocument>`
3. Register in `src/parsers/mod.rs` dispatch function
4. Add MIME type detection
5. Write unit tests with sample files

### Modifying the Search Schema

1. Update `src/indexer/schema.rs`
2. **WARNING**: Schema changes require reindexing all documents
3. Consider migration strategy for existing users
4. Bump index version in constants

### Adding a New Iced Message

1. Define the message variant in the relevant `Message` enum (e.g., in `src/iced_ui/mod.rs`)
2. Handle the message in the `update()` function
3. Return an async `Command::perform` for any IO/backend work
4. Ensure the view triggers the matching message on user interactions

## Testing Guidelines

### Unit Tests (Rust)

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_docx_parsing() {
        let doc = parse_docx(Path::new("tests/fixtures/sample.docx"))
            .expect("Should parse DOCX");
        
        assert!(doc.content.contains("Hello World"));
        assert_eq!(doc.title, Some("Sample Document".to_string()));
    }
}
```

### Integration Tests

- Test with large file collections (10K+ files)
- Measure memory usage with `valgrind` or `/usr/bin/time -v`
- Benchmark search latency with `cargo bench`

## Build Commands

```bash
# Build and Run
cargo run

# Build for production
cargo build --release

# Run Rust tests
cargo test
```

## Troubleshooting

### High Memory Usage

- Check for memory leaks in parsers (ensure files are closed)
- Verify Tantivy index writer buffer size
- Profile with `heaptrack` or `dhat`

### Slow Search

- Verify Tantivy reader is cached (don't recreate)
- Check if index is memory-mapped
- Profile with `perf` or `samply`

### Parser Failures

- Check file encoding detection
- Verify zip/xml parsing handles malformed files gracefully
- Add more comprehensive error contexts

## Resources

- [Iced Documentation](https://docs.rs/iced/latest/iced/)
- [Tantivy Documentation](https://docs.rs/tantivy/latest/tantivy/)
- [Redb Documentation](https://docs.rs/redb/latest/redb/)
- [Rayon Documentation](https://docs.rs/rayon/latest/rayon/)

## Contact

For questions about implementation details, refer to:
- GitHub Issues: https://github.com/yourusername/flash-search/issues
- Architecture Decisions: See `docs/adr/` directory

---

**Last Updated**: 2024
**Version**: 0.1.0
