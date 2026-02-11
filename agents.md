# AGENTS.md - AI Coding Agent Instructions

## Project Overview

**Flash Search** is an ultrafast local full-text search application built with Rust (backend) and Svelte (frontend). This document provides comprehensive instructions for AI coding agents working on this codebase.

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────┐
│                     Flash Search                             │
├─────────────────────────────────────────────────────────────┤
│  Frontend (Svelte + TypeScript)                              │
│  ├── UI Components                                           │
│  ├── Search Interface                                        │
│  └── Settings Panel                                          │
├─────────────────────────────────────────────────────────────┤
│  Tauri Bridge                                                │
│  ├── Commands (invoke handlers)                              │
│  └── Events (async notifications)                            │
├─────────────────────────────────────────────────────────────┤
│  Rust Backend                                                │
│  ├── parsers/     - File format parsers                      │
│  ├── indexer/     - Tantivy search engine wrapper            │
│  ├── scanner/     - File system crawler & watcher            │
│  ├── metadata/    - redb database operations                 │
│  └── state/       - Application state management             │
└─────────────────────────────────────────────────────────────┘
```

## Project Structure

```
flash-search/
├── src-tauri/
│   ├── src/
│   │   ├── main.rs              # Application entry point
│   │   ├── lib.rs               # Library exports (if needed)
│   │   ├── commands.rs          # Tauri command handlers
│   │   ├── error.rs             # Error types and handling
│   │   ├── state.rs             # Tauri managed state
│   │   ├── parsers/
│   │   │   ├── mod.rs           # Parser dispatch
│   │   │   ├── docx.rs          # DOCX parser (zip + quick-xml)
│   │   │   ├── pdf.rs           # PDF parser
│   │   │   ├── text.rs          # Text file parser
│   │   │   └── excel.rs         # XLSX parser
│   │   ├── indexer/
│   │   │   ├── mod.rs           # Indexer module
│   │   │   ├── schema.rs        # Tantivy schema definition
│   │   │   ├── writer.rs        # Document addition/removal
│   │   │   └── searcher.rs      # Query execution
│   │   ├── scanner/
│   │   │   ├── mod.rs           # Scanner orchestration
│   │   │   ├── crawler.rs       # Directory traversal
│   │   │   ├── watcher.rs       # File system watcher
│   │   │   └── worker.rs        # Parallel processing pool
│   │   └── metadata/
│   │       ├── mod.rs           # Metadata DB interface
│   │       ├── db.rs            # redb table definitions
│   │       └── cache.rs         # File hash cache
│   ├── Cargo.toml
│   └── tauri.conf.json
├── src/
│   ├── App.svelte               # Main app component
│   ├── main.ts                  # Frontend entry
│   ├── components/
│   │   ├── SearchBar.svelte
│   │   ├── ResultList.svelte
│   │   ├── ResultItem.svelte
│   │   ├── PreviewPanel.svelte
│   │   └── SettingsPanel.svelte
│   ├── stores/
│   │   ├── search.ts            # Search state management
│   │   └── settings.ts          # User preferences
│   └── utils/
│       ├── api.ts               # Tauri invoke wrappers
│       └── format.ts            # Text formatting utilities
├── Cargo.toml
├── package.json
└── README.md
```

## Critical Implementation Guidelines

### 1. Performance First

**This project prioritizes performance over convenience.**

- **Memory**: Keep RAM usage under 50MB at idle
- **Speed**: Search results must return in <50ms
- **I/O**: Use memory-mapped files where possible
- **CPU**: Parallelize parsing with Rayon

**DON'Ts:**
- Don't load entire files into memory unless necessary
- Don't use DOM-based XML parsers (use quick-xml streaming)
- Don't block the main thread during indexing
- Don't use heavy serialization (prefer rkyv over serde_json for hot paths)

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

### 7. Tauri Commands

```rust
use tauri::State;
use std::sync::Arc;
use tokio::sync::Mutex;

// State type
pub type AppState = Arc<Mutex<AppStateInner>>;

pub struct AppStateInner {
    pub indexer: IndexManager,
    pub metadata_db: MetadataDb,
}

// Command implementations
#[tauri::command]
pub async fn search_query(
    query: String,
    state: State<'_, AppState>,
) -> Result<Vec<SearchResult>, String> {
    let state = state.lock().await;
    
    state.indexer
        .search(&query)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn start_indexing(
    path: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let state = state.inner().clone();
    
    // Spawn indexing in background
    tokio::spawn(async move {
        let scanner = Scanner::new(state);
        scanner.scan_directory(PathBuf::from(path)).await
    });
    
    Ok(())
}
```

### 8. Frontend Patterns (Svelte)

```svelte
<!-- SearchBar.svelte -->
<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { debounce } from '$lib/utils';
  
  let query = '';
  let results: SearchResult[] = [];
  let isLoading = false;
  
  const debouncedSearch = debounce(async (q: string) => {
    if (!q.trim()) {
      results = [];
      return;
    }
    
    isLoading = true;
    try {
      results = await invoke('search_query', { query: q });
    } catch (e) {
      console.error('Search failed:', e);
    } finally {
      isLoading = false;
    }
  }, 300);
  
  $: debouncedSearch(query);
</script>

<input
  type="text"
  bind:value={query}
  placeholder="Search files..."
  class="search-input"
/>

{#if isLoading}
  <div class="loading">Searching...</div>
{:else}
  <ResultList {results} />
{/if}
```

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

### Adding a Tauri Command

1. Define command in `src/commands.rs`
2. Add to `tauri::generate_handler![]` in `main.rs`
3. Create TypeScript type definition in `src/types.ts`
4. Add wrapper function in `src/utils/api.ts`
5. Update AGENTS.md command documentation

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
# Development
npm run tauri dev

# Build for production
npm run tauri build

# Run Rust tests
cd src-tauri && cargo test

# Run linter
cd src-tauri && cargo clippy -- -D warnings

# Format code
cd src-tauri && cargo fmt
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

- [Tantivy Documentation](https://docs.rs/tantivy/latest/tantivy/)
- [Tauri v2 Guide](https://v2.tauri.app/)
- [Redb Documentation](https://docs.rs/redb/latest/redb/)
- [Rayon Documentation](https://docs.rs/rayon/latest/rayon/)

## Contact

For questions about implementation details, refer to:
- GitHub Issues: https://github.com/yourusername/flash-search/issues
- Architecture Decisions: See `docs/adr/` directory

---

**Last Updated**: 2024
**Version**: 0.1.0
