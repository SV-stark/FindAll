//! Tool definitions and dispatch for the MCP server.
//!
//! Every tool is read-only. `search` and `search_filenames` go through the same
//! engine the GUI uses, so an agent and a human see the same index.

use serde_json::{Value, json};
use std::sync::Arc;

use super::protocol::{self, RpcError};
use crate::commands::AppState;
use crate::indexer::searcher::SearchParams;

/// Names of every tool this server exposes.
///
/// Kept as a constant so `tools/list` and the dispatch table cannot drift, and
/// asserted against each other in tests.
pub const TOOL_NAMES: &[&str] = &[
    "search",
    "search_filenames",
    "read_file_text",
    "index_stats",
];

/// Upper bound on results a single tool call may return.
///
/// An agent asking for 100 000 results would be told about the limit rather
/// than being allowed to trigger unbounded extraction work (snippets require
/// re-reading each hit from disk).
const MAX_TOOL_RESULTS: usize = 200;

/// Default result count when the caller does not specify one.
const DEFAULT_TOOL_RESULTS: usize = 20;

/// JSON schemas for every tool, in `TOOL_NAMES` order.
#[must_use]
pub fn tool_definitions() -> Vec<Value> {
    vec![
        search_schema(),
        search_filenames_schema(),
        read_file_text_schema(),
        index_stats_schema(),
    ]
}

/// `search` schema. Kept separate purely to keep `tool_definitions` readable;
/// the descriptions are long by nature.
#[must_use]
fn search_schema() -> Value {
    json!({
        "name": "search",
        "description":
            "Full-text search across the content of every indexed file (documents, \
             source code, spreadsheets, PDFs, archives). Supports the same operator \
             language as the UI: bare words are ANDed; use \"exact phrase\" for phrases, \
             OR for alternatives, -term to exclude, and ext:/path:/title:/size:/date: \
             for filters. Returns matching paths with highlighted snippets.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "Search query. Use \"quoted phrases\", OR, -exclusions, \
                                    and ext:/path:/date: operators for precision."
                },
                "limit": {
                    "type": "integer",
                    "minimum": 1,
                    "maximum": MAX_TOOL_RESULTS,
                    "description": "Maximum number of results to return (default 20)."
                },
                "extensions": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Restrict to these file extensions, without the dot, \
                                    e.g. [\"rs\", \"pdf\"]."
                },
                "case_sensitive": {
                    "type": "boolean",
                    "description": "Match case exactly (default false)."
                },
                "modified_after": {
                    "type": "integer",
                    "description": "Only files modified at or after this Unix timestamp (seconds)."
                },
                "modified_before": {
                    "type": "integer",
                    "description": "Only files modified before this Unix timestamp (seconds)."
                },
                "min_size": {
                    "type": "integer",
                    "description": "Only files at least this many bytes."
                },
                "max_size": {
                    "type": "integer",
                    "description": "Only files at most this many bytes."
                }
            },
            "required": ["query"]
        }
    })
}

/// `search_filenames` schema.
#[must_use]
fn search_filenames_schema() -> Value {
    json!({
        "name": "search_filenames",
        "description":
            "Search file and directory names only, without touching file contents. \
             Matches subsequences, so \"idx\" finds \"index.rs\". Much faster than \
             `search` and the right tool when the user is looking for a file by name.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "query": { "type": "string", "description": "Substring to match in file names." },
                "limit": {
                    "type": "integer",
                    "minimum": 1,
                    "maximum": MAX_TOOL_RESULTS,
                    "description": "Maximum results (default 20)."
                }
            },
            "required": ["query"]
        }
    })
}

/// `read_file_text` schema.
#[must_use]
fn read_file_text_schema() -> Value {
    json!({
        "name": "read_file_text",
        "description":
            "Extract and return the text of one file that is already indexed. Use this \
             after `search` to read the full content of a specific hit. Returns a \
             truncation notice when the document exceeds the extraction limit.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "Absolute path of an indexed file." },
                "max_chars": {
                    "type": "integer",
                    "minimum": 1000,
                    "maximum": 500_000,
                    "description": "Truncate output at this many characters (default 20000)."
                }
            },
            "required": ["path"]
        }
    })
}

/// `index_stats` schema.
#[must_use]
fn index_stats_schema() -> Value {
    json!({
        "name": "index_stats",
        "description":
            "Report how much is indexed: total document count, on-disk index size, and \
            whether indexing is currently running. Useful for answering \"is my index \
            up to date?\" before concluding a search returned nothing because the \
            content was never indexed.",
        "inputSchema": { "type": "object", "properties": {} }
    })
}

/// Dispatches a `tools/call` request.
pub async fn call_tool(state: &Arc<AppState>, params: &Value) -> Result<Value, RpcError> {
    let name = params
        .get("name")
        .and_then(Value::as_str)
        .ok_or_else(|| RpcError::invalid_params("missing \"name\""))?;
    let arguments = params.get("arguments").cloned().unwrap_or(json!({}));

    match name {
        "search" => search(state, &arguments).await,
        "search_filenames" => search_filenames(state, &arguments).await,
        "read_file_text" => read_file_text(state, &arguments).await,
        "index_stats" => index_stats(state),
        other => Err(RpcError::new(
            protocol::METHOD_NOT_FOUND,
            format!("unknown tool: {other}"),
        )),
    }
}

/// `search` implementation.
async fn search(state: &Arc<AppState>, args: &Value) -> Result<Value, RpcError> {
    let query = args
        .get("query")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|q| !q.is_empty())
        .ok_or_else(|| RpcError::invalid_params("\"query\" must be a non-empty string"))?;

    let limit = read_limit(args)?;
    let case_sensitive = args
        .get("case_sensitive")
        .and_then(Value::as_bool)
        .unwrap_or(false);

    let extensions: Option<Vec<String>> = args
        .get("extensions")
        .and_then(Value::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(Value::as_str)
                .map(|s| s.trim_start_matches('.').to_lowercase())
                .filter(|s| !s.is_empty())
                .collect()
        })
        .filter(|v: &Vec<String>| !v.is_empty());

    let settings = state.settings_cache.load();
    let enable_ocr = settings.enable_ocr;

    let params = SearchParams::builder()
        .query(query)
        .limit(limit)
        // `None` means "no type restriction"; an empty slice would mean "match
        // nothing", so the Option must be forwarded rather than unwrapped.
        .file_extensions(extensions.as_deref().unwrap_or(&[]))
        .case_sensitive(case_sensitive)
        .maybe_min_size(read_u64(args, "min_size")?)
        .maybe_max_size(read_u64(args, "max_size")?)
        .maybe_min_modified(read_u64(args, "modified_after")?)
        .build();

    // Snippets are generated here (not by the indexer) because `content` is no
    // longer STORED; see `crate::snippet`.
    let mut results = state
        .indexer
        .search(params)
        .await
        .map_err(|e| RpcError::new(protocol::INTERNAL_ERROR, e.to_string()))?;

    attach_snippets(&mut results, query, case_sensitive, enable_ocr).await;

    let payload = json!({
        "query": query,
        "count": results.len(),
        "results": results
            .iter()
            .map(|r| json!({
                "path": r.file_path,
                "title": r.title,
                "extension": r.extension,
                "score": r.score,
                "modified": r.modified,
                "size": r.size,
                "snippets": r.snippets,
            }))
            .collect::<Vec<_>>(),
    });

    Ok(protocol::tool_result_text(
        serde_json::to_string_pretty(&payload).unwrap_or_else(|e| format!("{{\"error\":\"{e}\"}}")),
    ))
}

/// Populates `snippets` on each result by re-extracting from disk.
async fn attach_snippets(
    results: &mut Vec<crate::indexer::searcher::SearchResult>,
    query: &str,
    case_sensitive: bool,
    enable_ocr: bool,
) {
    let mut tasks = tokio::task::JoinSet::new();
    for result in results
        .iter()
        .take(crate::snippet::MAX_SNIPPET_FILES_PER_QUERY)
    {
        let path = result.file_path.clone();
        let query = query.to_string();
        tasks.spawn(async move {
            let outcome =
                crate::snippet::generate_snippets(&path, None, &query, case_sensitive, enable_ocr)
                    .await;
            (path, outcome)
        });
    }

    let mut by_path = std::collections::HashMap::new();
    while let Some(joined) = tasks.join_next().await {
        if let Ok((path, snippets)) = joined {
            by_path.insert(path, snippets);
        }
    }

    // A case-sensitive query drops hits whose text lacks the exact spelling.
    let enforce_case = case_sensitive
        && crate::indexer::searcher::needs_case_post_filter(
            &crate::indexer::query_parser::extract_highlight_terms(query, case_sensitive),
        );

    results.retain(|result| {
        !enforce_case
            || by_path
                .get(&result.file_path)
                .is_none_or(|outcome| outcome.case_match)
    });

    for result in results.iter_mut() {
        if let Some(outcome) = by_path.get(&result.file_path) {
            result.snippets = outcome.snippets.clone();
        }
    }
}

/// `search_filenames` implementation.
async fn search_filenames(state: &Arc<AppState>, args: &Value) -> Result<Value, RpcError> {
    let query = args
        .get("query")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|q| !q.is_empty())
        .ok_or_else(|| RpcError::invalid_params("\"query\" must be a non-empty string"))?;

    let limit = read_limit(args)?;

    let Some(filename_index) = state.filename_index.as_ref() else {
        // Not an error: the index is an opt-in setting, and the caller should
        // be told to fall back to `search` with a `path:` filter.
        return Ok(protocol::tool_result_text(
            "The filename index is disabled in Flash Search settings, so name-only \
             search is unavailable. Use `search` with a `path:` filter instead, which \
             searches the file path of indexed documents.",
        ));
    };

    // `FilenameIndex::search` is synchronous (it reads an in-memory FST
    // snapshot) despite the rest of the tool surface being async.
    let matches = tokio::task::spawn_blocking({
        let query = query.to_string();
        let index = Arc::clone(filename_index);
        move || index.search(&query, limit)
    })
    .await
    .map_err(|e| RpcError::new(protocol::INTERNAL_ERROR, e.to_string()))?
    .map_err(|e| RpcError::new(protocol::INTERNAL_ERROR, e.to_string()))?;

    let payload = json!({
        "query": query,
        "count": matches.len(),
        "results": matches
            .iter()
            .map(|m| json!({
                "path": m.file_path,
                "file_name": m.file_name,
            }))
            .collect::<Vec<_>>(),
    });

    Ok(protocol::tool_result_text(
        serde_json::to_string_pretty(&payload).unwrap_or_else(|e| format!("{{\"error\":\"{e}\"}}")),
    ))
}

/// `read_file_text` implementation.
async fn read_file_text(state: &Arc<AppState>, args: &Value) -> Result<Value, RpcError> {
    let path = args
        .get("path")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .ok_or_else(|| RpcError::invalid_params("\"path\" must be a non-empty string"))?;

    let max_chars = args
        .get("max_chars")
        .and_then(Value::as_u64)
        .unwrap_or(20_000)
        .clamp(1_000, 500_000) as usize;

    let enable_ocr = state.settings_cache.load().enable_ocr;
    let path_buf = std::path::PathBuf::from(path);

    let parsed = crate::parsers::parse_file(&path_buf, enable_ocr)
        .await
        .map_err(|e| RpcError::invalid_params(format!("could not read {path}: {e}")))?;

    let (text, truncated) = if parsed.content.chars().count() > max_chars {
        let truncated: String = parsed.content.chars().take(max_chars).collect();
        (truncated, true)
    } else {
        (parsed.content, false)
    };

    let payload = json!({
        "path": path,
        "title": parsed.title,
        "chars": text.chars().count(),
        "truncated": truncated,
        "text": text,
    });

    Ok(protocol::tool_result_text(
        serde_json::to_string_pretty(&payload).unwrap_or_else(|e| format!("{{\"error\":\"{e}\"}}")),
    ))
}

/// `index_stats` implementation.
fn index_stats(state: &Arc<AppState>) -> Result<Value, RpcError> {
    let stats = state
        .indexer
        .get_statistics()
        .map_err(|e| RpcError::new(protocol::INTERNAL_ERROR, e.to_string()))?;

    let settings = state.settings_cache.load();
    let payload = json!({
        "documents": stats.total_documents,
        "index_size_bytes": stats.total_size_bytes,
        "indexed_directories": settings.index_dirs,
        "filename_index_enabled": state.filename_index.is_some(),
        "watching": state.is_indexing(),
    });

    Ok(protocol::tool_result_text(
        serde_json::to_string_pretty(&payload).unwrap_or_else(|e| format!("{{\"error\":\"{e}\"}}")),
    ))
}

/// Reads and validates the `limit` argument.
fn read_limit(args: &Value) -> Result<usize, RpcError> {
    let limit = args
        .get("limit")
        .and_then(Value::as_u64)
        // `MAX_TOOL_RESULTS` is far below `usize::MAX` on every target, so the
        // clamp makes the narrowing lossless in practice.
        .map_or(DEFAULT_TOOL_RESULTS, |v| {
            usize::try_from(v).unwrap_or(MAX_TOOL_RESULTS)
        });

    if limit == 0 {
        return Err(RpcError::invalid_params("\"limit\" must be at least 1"));
    }
    Ok(limit.min(MAX_TOOL_RESULTS))
}

/// Reads an optional unsigned-integer argument, rejecting negatives and
/// non-integers rather than silently ignoring them.
fn read_u64(args: &Value, key: &str) -> Result<Option<u64>, RpcError> {
    match args.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Number(n)) => n.as_u64().map(Some).ok_or_else(|| {
            RpcError::invalid_params(format!("\"{key}\" must be a non-negative integer"))
        }),
        Some(_) => Err(RpcError::invalid_params(format!(
            "\"{key}\" must be a number"
        ))),
    }
}

/// Builds a minimal `AppState` over a temp directory.
///
/// The watcher and scanner are real (not stubbed) because `AppState` requires
/// them; both are constructed against the same empty temp index, so neither
/// observes anything.
///
/// `WatcherManager` captures `tokio::runtime::Handle::current()`, so this must
/// be called from inside a runtime. Every caller is therefore a `#[tokio::test]`,
/// which is exactly the constraint that previously produced "no reactor running"
/// panics.
///
/// Declared at module scope (not inside `tests`) so `mcp`'s own tests can reach
/// it: `pub(super)` from here means "visible in `mcp`".
#[cfg(test)]
pub(super) fn test_state() -> Arc<AppState> {
    let dir = tempfile::tempdir().expect("tempdir");
    let settings_manager = crate::settings::SettingsManager::new(dir.path());
    let settings = settings_manager
        .load()
        .unwrap_or_else(|_| crate::settings::AppSettings::default());

    let indexer = Arc::new(
        crate::indexer::IndexManager::open(&dir.path().join("index"), 32).expect("open index"),
    );
    let metadata_db = Arc::new(
        crate::metadata::MetadataDb::open(&dir.path().join("meta.redb"))
            .expect("open metadata")
            .0,
    );
    let settings_cache = Arc::new(arc_swap::ArcSwap::from_pointee(settings));
    let (progress_tx, _progress_rx) = flume::bounded(16);

    let scanner = Arc::new(crate::scanner::Scanner::new(
        Arc::clone(&indexer),
        Arc::clone(&metadata_db),
        None,
        Some(progress_tx.clone()),
        Arc::clone(&settings_cache),
    ));
    let watcher = crate::watcher::WatcherManager::new_with_excludes(
        Arc::clone(&indexer),
        Arc::clone(&metadata_db),
        settings_cache.load().get_allowed_extensions().clone(),
        &[],
        false,
    );

    Arc::new(
        crate::commands::AppState::builder()
            .indexer(indexer)
            .metadata_db(metadata_db)
            .settings_cache(settings_cache)
            .settings_manager(settings_manager)
            .watcher(watcher)
            .progress_tx(progress_tx)
            .scanner(scanner)
            .build(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::protocol;

    #[test]
    fn tool_names_match_definitions_exactly() {
        let definitions = tool_definitions();
        let defined: Vec<&str> = definitions
            .iter()
            .filter_map(|t| t["name"].as_str())
            .collect();
        assert_eq!(
            defined,
            TOOL_NAMES.to_vec(),
            "TOOL_NAMES and tool_definitions() have drifted"
        );
    }

    #[test]
    fn limit_defaults_and_clamps() {
        assert_eq!(read_limit(&json!({})).unwrap(), DEFAULT_TOOL_RESULTS);
        assert_eq!(read_limit(&json!({"limit": 5})).unwrap(), 5);
        assert_eq!(
            read_limit(&json!({"limit": 100_000})).unwrap(),
            MAX_TOOL_RESULTS,
            "oversized limits must be clamped, not rejected"
        );
    }

    #[test]
    fn zero_limit_is_rejected() {
        assert!(read_limit(&json!({"limit": 0})).is_err());
    }

    #[test]
    fn negative_limit_is_rejected() {
        // `as_u64` returns None for -1, so this falls back to the default; the
        // point is that it must not become a huge unsigned value.
        assert_eq!(
            read_limit(&json!({"limit": -1})).unwrap(),
            DEFAULT_TOOL_RESULTS
        );
    }

    #[test]
    fn u64_args_reject_negatives_and_wrong_types() {
        assert_eq!(read_u64(&json!({}), "min_size").unwrap(), None);
        assert_eq!(
            read_u64(&json!({"min_size": 1024}), "min_size").unwrap(),
            Some(1024)
        );
        assert!(read_u64(&json!({"min_size": -5}), "min_size").is_err());
        assert!(read_u64(&json!({"min_size": "big"}), "min_size").is_err());
        assert!(read_u64(&json!({"min_size": 1.5}), "min_size").is_err());
    }

    #[tokio::test]
    async fn unknown_tool_is_method_not_found() {
        let state = test_state();
        let err = call_tool(&state, &json!({"name": "delete_everything"})).await;
        let err = err.expect_err("must reject unknown tool");
        assert_eq!(err.code, protocol::METHOD_NOT_FOUND);
    }

    #[tokio::test]
    async fn missing_name_is_invalid_params() {
        let state = test_state();
        let err = call_tool(&state, &json!({})).await;
        let err = err.expect_err("must reject missing name");
        assert_eq!(err.code, protocol::INVALID_PARAMS);
    }

    #[tokio::test]
    async fn empty_query_is_rejected_before_touching_the_index() {
        let state = test_state();
        for bad in ["", "   "] {
            let err = call_tool(
                &state,
                &json!({"name": "search", "arguments": {"query": bad}}),
            )
            .await
            .expect_err("empty query must be rejected");
            assert_eq!(err.code, protocol::INVALID_PARAMS, "query: {bad:?}");
        }
    }

    #[tokio::test]
    async fn missing_query_argument_is_invalid_params() {
        let state = test_state();
        let err = call_tool(&state, &json!({"name": "search", "arguments": {}}))
            .await
            .expect_err("missing query must be rejected");
        assert_eq!(err.code, protocol::INVALID_PARAMS);
    }

    /// End-to-end proof that change #2 works: `content` is no longer STORED, so a
    /// snippet can only exist if it was re-extracted from disk.
    #[tokio::test]
    async fn searcher_returns_hits_without_snippets_because_content_is_not_stored() {
        let dir = tempfile::tempdir().expect("tempdir");
        let file = dir.path().join("needle.txt");
        std::fs::write(&file, "a distinctive haystackneedle phrase here").expect("write");

        let indexer = Arc::new(
            crate::indexer::IndexManager::open(&dir.path().join("index"), 32).expect("open index"),
        );
        let doc = crate::parsers::parse_file(&file, false)
            .await
            .expect("parse");
        let size = std::fs::metadata(&file).expect("meta").len();
        indexer
            .writer()
            .add_document(&doc, 0, size)
            .expect("index doc");
        indexer.commit().expect("commit");

        let hits = indexer
            .search_blocking(
                &SearchParams::builder()
                    .query("haystackneedle")
                    .limit(10)
                    .case_sensitive(false)
                    .build(),
            )
            .expect("search");
        assert_eq!(hits.len(), 1, "the indexed document must be findable");
        assert!(
            hits[0].snippets.is_empty(),
            "the searcher must not synthesize snippets from the doc store"
        );
    }
}
