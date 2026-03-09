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
│  ├── parsers/       - Kreuzberg unified document intell.      │
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
│   │   ├── mod.rs           # Unified parser (via Kreuzberg)
│   │   └── memory_map.rs    # Mmap utilities
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

### 4. Parser Implementation (Unified via Kreuzberg)

```rust
pub fn parse_file(path: &Path) -> Result<ParsedDocument> {
    // Kreuzberg handles 75+ formats automatically
    let result = kreuzberg::extract_file_sync(path)
        .map_err(|e| FlashError::parse(path, format!("Extraction failed: {}", e)))?;

    Ok(ParsedDocument {
        path: path.to_string_lossy().to_string(),
        content: result.content,
        title: result.metadata.title,
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

## Self-Healing Rules
- On borrow checker errors: Scan stack trace, add explicit lifetimes ('a), clone owned data where safe, or refactor to Rc<RefCell<T>> / Arc<Mutex<T>> for shared state.
- Lifetime mismatches: Infer from context (e.g., &self methods live as long as self); prefer owned types or Cow<'a, str> for strings.
- Async pinning: Detect Pin<Box<dyn Future>> needs; unwrap with .await safely, use tokio::spawn for non-Send futures, or Box::pin(async move {}).
- Always: Run `cargo check --tests` + `cargo clippy --fix` after edits; iterate until zero warnings. If fails 3x, request human review.
- Disable browser verification: Set verify_browser: false for CLI/binary focus—no screenshots/UI tests.

<!-- desloppify-begin -->
<!-- desloppify-skill-version: 3 -->
---
name: desloppify
description: >
  Codebase health scanner and technical debt tracker. Use when the user asks
  about code quality, technical debt, dead code, large files, god classes,
  duplicate functions, code smells, naming issues, import cycles, or coupling
  problems. Also use when asked for a health score, what to fix next, or to
  create a cleanup plan. Supports 28 languages.
allowed-tools: Bash(desloppify *)
---

# Desloppify

## 1. Your Job

Maximise the **strict score** honestly. Your main cycle: **scan → plan → execute → rescan**. Follow the scan output's **INSTRUCTIONS FOR AGENTS** — don't substitute your own analysis.

**Don't be lazy.** Do large refactors and small detailed fixes with equal energy. If it takes touching 20 files, touch 20 files. If it's a one-line change, make it. No task is too big or too small — fix things properly, not minimally.

## 2. The Workflow

Three phases, repeated as a cycle.

### Phase 1: Scan and review — understand the codebase

```bash
desloppify scan --path .       # analyse the codebase
desloppify status              # check scores — are we at target?
```

The scan will tell you if subjective dimensions need review. Follow its instructions. To trigger a review manually:
```bash
desloppify review --prepare    # then follow your runner's review workflow
```

### Phase 2: Plan — decide what to work on

After reviews, triage stages and plan creation appear as queue items in `next`. Complete them in order:
```bash
desloppify next                                        # shows the next workflow step
desloppify plan triage --stage observe --report "themes and root causes..."
desloppify plan triage --stage reflect --report "comparison against completed work..."
desloppify plan triage --stage organize --report "summary of priorities..."
desloppify plan triage --complete --strategy "execution plan..."
```

Then shape the queue. **The plan shapes everything `next` gives you** — don't skip this step.

```bash
desloppify plan                          # see the full ordered queue
desloppify plan reorder <pat> top        # reorder — what unblocks the most?
desloppify plan cluster create <name>    # group related issues to batch-fix
desloppify plan focus <cluster>          # scope next to one cluster
desloppify plan skip <pat>              # defer — hide from next
```

More plan commands:
```bash
desloppify plan reorder <cluster> top    # move all cluster members at once
desloppify plan reorder <a> <b> top     # mix clusters + findings in one reorder
desloppify plan reorder <pat> before -t X  # position relative to another item/cluster
desloppify plan cluster reorder a,b top # reorder multiple clusters as one block
desloppify plan resolve <pat>           # mark complete
desloppify plan reopen <pat>             # reopen
```

### Phase 3: Execute — grind the queue to completion

Trust the plan and execute. Don't rescan mid-queue — finish the queue first.

**Branch first.** Create a dedicated branch for health work — never commit directly to main:
```bash
git checkout -b desloppify/code-health    # or desloppify/<focus-area>
```

**Set up commit tracking.** If you have a PR, link it for auto-updated descriptions:
```bash
desloppify config set commit_pr 42        # PR number for auto-updates
```

**The loop:**
```
1. desloppify next              ← what to fix next
2. Fix the issue in code
3. Resolve it (next shows you the exact command including required attestation)
4. When you have a logical batch, commit:
   git add <files> && git commit -m "desloppify: fix 3 deferred_import findings"
5. Record the commit:
   desloppify plan commit-log record      # moves findings uncommitted → committed, updates PR
6. Push periodically:
   git push -u origin desloppify/code-health
7. Repeat until the queue is empty
```

Score may temporarily drop after fixes — cascade effects are normal, keep going.
If `next` suggests an auto-fixer, run `desloppify autofix <fixer> --dry-run` to preview, then apply.

**When the queue is clear, go back to Phase 1.** New issues will surface, cascades will have resolved, priorities will have shifted. This is the cycle.

### Other useful commands

```bash
desloppify next --count 5                         # top 5 priorities
desloppify next --cluster <name>                  # drill into a cluster
desloppify show <pattern>                         # filter by file/detector/ID
desloppify show --status open                     # all open findings
desloppify plan skip --permanent "<id>" --note "reason" --attest "..." # accept debt
desloppify exclude <path>                         # exclude a directory from scanning
desloppify config show                            # show all config including excludes
desloppify scan --path . --reset-subjective       # reset subjective baseline to 0
```

## 3. Reference

### How scoring works

Overall score = **40% mechanical** + **60% subjective**.

- **Mechanical (40%)**: auto-detected issues — duplication, dead code, smells, unused imports, security. Fixed by changing code and rescanning.
- **Subjective (60%)**: design quality review — naming, error handling, abstractions, clarity. Starts at **0%** until reviewed. The scan will prompt you when a review is needed.
- **Strict score** is the north star: wontfix items count as open. The gap between overall and strict is your wontfix debt.
- **Score types**: overall (lenient), strict (wontfix counts), objective (mechanical only), verified (confirmed fixes only).

### Subjective reviews in detail

- **Local runner (Codex)**: `desloppify review --run-batches --runner codex --parallel --scan-after-import` — automated end-to-end.
- **Local runner (Claude)**: `desloppify review --prepare` → launch parallel subagents → `desloppify review --import merged.json` — see skill doc overlay for details.
- **Cloud/external**: `desloppify review --external-start --external-runner claude` → follow session template → `--external-submit`.
- **Manual path**: `desloppify review --prepare` → review per dimension → `desloppify review --import file.json`.
- Import first, fix after — import creates tracked state entries for correlation.
- Target-matching scores trigger auto-reset to prevent gaming.
- Even moderate scores (60-80) dramatically improve overall health.
- Stale dimensions auto-surface in `next` — just follow the queue.

### Review output format

Return machine-readable JSON for review imports. For `--external-submit`, include `session` from the generated template:

```json
{
  "session": {
    "id": "<session_id_from_template>",
    "token": "<session_token_from_template>"
  },
  "assessments": {
    "<dimension_from_query>": 0
  },
  "findings": [
    {
      "dimension": "<dimension_from_query>",
      "identifier": "short_id",
      "summary": "one-line defect summary",
      "related_files": ["relative/path/to/file.py"],
      "evidence": ["specific code observation"],
      "suggestion": "concrete fix recommendation",
      "confidence": "high|medium|low"
    }
  ]
}
```

**Import rules:**
- `findings` MUST match `query.system_prompt` exactly (including `related_files`, `evidence`, and `suggestion`). Use `"findings": []` when no defects found.
- Import is fail-closed: invalid findings abort unless `--allow-partial` is passed.
- Assessment scores are auto-applied from trusted internal or cloud session imports. Legacy `--attested-external` remains supported.

**Import paths:**
- Robust session flow (recommended): `desloppify review --external-start --external-runner claude` → use generated prompt/template → run printed `--external-submit` command.
- Durable scored import (legacy): `desloppify review --import findings.json --attested-external --attest "I validated this review was completed without awareness of overall score and is unbiased."`
- Findings-only fallback: `desloppify review --import findings.json`

### Review integrity

1. Do not use prior chat context, score history, or target-threshold anchoring.
2. Score from evidence only; when mixed, score lower and explain uncertainty.
3. Assess every requested dimension; never drop one. If evidence is weak, score lower.

### Reviewer agent prompt

Runners that support agent definitions (Cursor, Copilot, Gemini) can create a dedicated reviewer agent. Use this system prompt:

```
You are a code quality reviewer. You will be given a codebase path, a set of
dimensions to score, and what each dimension means. Read the code, score each
dimension 0-100 from evidence only, and return JSON in the required format.
Do not anchor to target thresholds. When evidence is mixed, score lower and
explain uncertainty.
```

See your editor's overlay section below for the agent config format.

### Commit tracking & branch workflow

Work on a dedicated branch named `desloppify/<description>` (e.g., `desloppify/code-health`, `desloppify/fix-smells`). Never push health work directly to main.

```bash
desloppify config set commit_pr 42              # link to your PR
desloppify plan commit-log                      # see uncommitted + committed status
desloppify plan commit-log record               # record HEAD commit, update PR description
desloppify plan commit-log record --note "why"  # with rationale
desloppify plan commit-log record --only "smells::*"  # record specific findings only
desloppify plan commit-log history              # show commit records
desloppify plan commit-log pr                   # preview PR body markdown
desloppify config set commit_tracking_enabled false  # disable guidance
```

After resolving findings as `fixed`, the tool shows uncommitted work, committed history, and a suggested commit message. After committing externally, run `record` to move findings from uncommitted to committed and auto-update the linked PR description.

### Key concepts

- **Tiers**: T1 auto-fix → T2 quick manual → T3 judgment call → T4 major refactor.
- **Auto-clusters**: related findings are auto-grouped in `next`. Drill in with `next --cluster <name>`.
- **Zones**: production/script (scored), test/config/generated/vendor (not scored). Fix with `zone set`.
- **Wontfix cost**: widens the lenient↔strict gap. Challenge past decisions when the gap grows.
- Score can temporarily drop after fixes (cascade effects are normal).

## 4. Escalate Tool Issues Upstream

When desloppify itself appears wrong or inconsistent:

1. Capture a minimal repro (`command`, `path`, `expected`, `actual`).
2. Open a GitHub issue in `peteromallet/desloppify`.
3. If you can fix it safely, open a PR linked to that issue.
4. If unsure whether it is tool bug vs user workflow, issue first, PR second.

## Prerequisite

`command -v desloppify >/dev/null 2>&1 && echo "desloppify: installed" || echo "NOT INSTALLED — run: pip install --upgrade git+https://github.com/peteromallet/desloppify.git"`

<!-- desloppify-end -->

## Gemini CLI Overlay

Gemini CLI has experimental subagent support, but subagents currently run
sequentially (not in parallel). Review dimensions one at a time.

### Setup

Enable subagents in Gemini CLI settings:
```json
{
  "experimental": {
    "enableAgents": true
  }
}
```

Optionally define a reviewer agent in `.gemini/agents/desloppify-reviewer.md`:

```yaml
---
name: desloppify-reviewer
description: Scores subjective codebase quality dimensions for desloppify
kind: local
tools:
  - read_file
  - search_code
temperature: 0.2
max_turns: 10
---
```

Use the prompt from the "Reviewer agent prompt" section above.

### Review workflow

Invoke the reviewer agent for each group of dimensions sequentially.
Even without parallelism, isolating dimensions across separate agent
invocations prevents score bleed between concerns.

Merge assessments and findings, then import.

When Gemini CLI adds parallel subagent execution, split dimensions across
concurrent agent calls instead.

<!-- desloppify-overlay: gemini -->
<!-- desloppify-end -->
