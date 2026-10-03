use crate::error::{FlashError, Result};
use std::path::{Path, PathBuf};
use tracing::{error, warn};

pub mod memory_map;

use compact_str::CompactString;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ParsedDocument {
    pub path: String,
    pub content: String,
    pub title: Option<CompactString>,
    pub language: Option<CompactString>,
    pub keywords: Option<String>,
    pub layout: Option<String>,
    pub code_metadata: Option<String>,
    pub embeddings: Option<Vec<f32>>,
}

#[derive(Debug, Clone)]
pub struct PreviewElement {
    pub element_type: crate::models::ElementType,
    pub content: String,
}

/// Returns true for plaintext, code, data, and configuration file formats
/// that can be read with zero-copy UTF-8 validation bypassing heavy document parsing.
#[must_use]
pub fn is_plaintext_fast_path(path: &Path) -> bool {
    let Some(ext) = path.extension().and_then(|e| e.to_str()) else {
        return false;
    };
    matches!(
        ext.to_ascii_lowercase().as_str(),
        "txt"
            | "md"
            | "markdown"
            | "rs"
            | "py"
            | "js"
            | "jsx"
            | "ts"
            | "tsx"
            | "json"
            | "yaml"
            | "yml"
            | "toml"
            | "xml"
            | "csv"
            | "tsv"
            | "html"
            | "htm"
            | "css"
            | "scss"
            | "sass"
            | "less"
            | "c"
            | "cpp"
            | "cc"
            | "cxx"
            | "h"
            | "hpp"
            | "hh"
            | "hxx"
            | "cs"
            | "go"
            | "java"
            | "kt"
            | "kts"
            | "swift"
            | "php"
            | "rb"
            | "lua"
            | "sh"
            | "bash"
            | "zsh"
            | "bat"
            | "cmd"
            | "ps1"
            | "psm1"
            | "ini"
            | "conf"
            | "config"
            | "env"
            | "log"
            | "sql"
            | "graphql"
            | "proto"
            | "diff"
            | "patch"
            | "tex"
            | "r"
            | "dart"
            | "scala"
            | "zig"
            | "nim"
    )
}

/// True for extensions that should be previewed with syntax highlighting.
#[must_use]
pub fn is_code_extension(ext: &str) -> bool {
    matches!(
        ext.to_ascii_lowercase().as_str(),
        "rs" | "py"
            | "js"
            | "jsx"
            | "ts"
            | "tsx"
            | "c"
            | "cpp"
            | "h"
            | "hpp"
            | "cs"
            | "go"
            | "java"
            | "kt"
            | "swift"
            | "php"
            | "rb"
            | "lua"
            | "sh"
            | "sql"
            | "json"
            | "toml"
            | "yaml"
            | "yml"
            | "xml"
            | "html"
            | "css"
    )
}

/// Ensure Xberg built-in extractors and registries are pre-warmed during bootstrap.
pub fn ensure_initialized() {
    let _ = xberg::core::mime::list_supported_formats();
}

/// Streams a file through BLAKE3 without loading it into memory.
///
/// The heavy extraction path used to `read_file` the *entire* document a second
/// time purely to hash it, which for a 90 MB PDF meant another 90 MB of pages
/// faulted in and a transient allocation of the same size. Hashing in 256 KiB
/// chunks keeps this path's memory flat and lets the OS evict the previous
/// buffer before the next one is read.
fn hash_file_streaming(path: &Path) -> Result<[u8; 32]> {
    use std::io::Read;

    let file = std::fs::File::open(path).map_err(|e| FlashError::Io(std::sync::Arc::new(e)))?;

    let mut hasher = blake3::Hasher::new();
    let mut buffer = vec![0u8; 256 * 1024];
    let mut reader = std::io::BufReader::with_capacity(256 * 1024, file);

    loop {
        let read = reader
            .read(&mut buffer)
            .map_err(|e| FlashError::Io(std::sync::Arc::new(e)))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }

    Ok(*hasher.finalize().as_bytes())
}

/// Reads a plaintext/code file and returns its UTF-8 content plus BLAKE3 hash in
/// a single pass over the data.
fn parse_plaintext(path: &Path) -> Result<(String, [u8; 32])> {
    let file_data = memory_map::read_file(path)?;
    let hash = *blake3::hash(&file_data).as_bytes();
    let content = std::str::from_utf8(&file_data).map_or_else(
        |_| String::from_utf8_lossy(&file_data).into_owned(),
        std::string::ToString::to_string,
    );
    Ok((content, hash))
}

/// Builds a `ParsedDocument` for a plaintext file, using the file name as title.
fn plaintext_document(path: &Path, content: String) -> ParsedDocument {
    ParsedDocument {
        path: path.to_string_lossy().to_string(),
        content,
        title: path
            .file_name()
            .map(|n| CompactString::from(n.to_string_lossy().as_ref())),
        language: None,
        keywords: None,
        layout: None,
        code_metadata: None,
        embeddings: None,
    }
}

/// Detect file type, route to appropriate parser, and compute BLAKE3 hash in a single pass
pub async fn parse_file_with_hash(
    path: &Path,
    enable_ocr: bool,
) -> Result<(ParsedDocument, [u8; 32])> {
    // Zero-copy plaintext / code fast path (microsecond execution)
    if is_plaintext_fast_path(path) {
        let (content, hash) = parse_plaintext(path)?;
        return Ok((plaintext_document(path, content), hash));
    }

    // Heavy document extraction pipeline (PDF, DOCX, XLSX, etc.)
    let config = extraction_config(enable_ocr);

    let input = xberg::ExtractInput::from_uri(path.to_string_lossy().into_owned());

    let result = xberg::extract(input, &config).await.map_err(|e| {
        tracing::error!("Failed to extract file {}: {}", path.display(), e);
        FlashError::parse(path, format!("Extraction failed: {e}"))
    })?;

    let doc = result.results.into_iter().next().ok_or_else(|| {
        FlashError::parse(path, "Extraction returned empty results list".to_string())
    })?;

    Ok((
        map_extracted_document(path, doc),
        hash_file_streaming(path)?,
    ))
}

/// Builds the Xberg extraction config used for indexing.
fn extraction_config(enable_ocr: bool) -> xberg::ExtractionConfig {
    xberg::ExtractionConfig {
        use_cache: false,
        disable_ocr: !enable_ocr,
        output_format: xberg::OutputFormat::Plain,
        ..Default::default()
    }
}

/// Detect file type and route to appropriate parser
pub async fn parse_file(path: &Path, enable_ocr: bool) -> Result<ParsedDocument> {
    parse_file_with_hash(path, enable_ocr)
        .await
        .map(|(doc, _)| doc)
}

pub async fn parse_file_preview(path: &Path, enable_ocr: bool) -> Result<Vec<PreviewElement>> {
    // Fast path for code and text preview rendering
    if is_plaintext_fast_path(path) {
        let (content, _) = parse_plaintext(path)?;

        let extension = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        let element_type = if is_code_extension(extension) {
            crate::models::ElementType::CodeBlock
        } else {
            crate::models::ElementType::NarrativeText
        };

        return Ok(vec![PreviewElement {
            element_type,
            content,
        }]);
    }

    let config = xberg::ExtractionConfig {
        use_cache: false,
        disable_ocr: !enable_ocr,
        result_format: xberg::ResultFormat::ElementBased,
        ..Default::default()
    };

    let input = xberg::ExtractInput::from_uri(path.to_string_lossy().into_owned());

    let result = xberg::extract(input, &config)
        .await
        .map_err(|e| FlashError::parse(path, format!("Preview extraction failed: {e}")))?;

    let doc = result.results.into_iter().next().ok_or_else(|| {
        FlashError::parse(
            path,
            "Preview extraction returned empty results list".to_string(),
        )
    })?;

    let elements = doc
        .elements
        .unwrap_or_default()
        .into_iter()
        .map(|e| {
            let element_type = match e.element_type {
                xberg::types::ElementType::Title => crate::models::ElementType::Title,
                xberg::types::ElementType::Heading => crate::models::ElementType::Heading,
                xberg::types::ElementType::NarrativeText => {
                    crate::models::ElementType::NarrativeText
                }
                xberg::types::ElementType::ListItem => crate::models::ElementType::ListItem,
                xberg::types::ElementType::CodeBlock => crate::models::ElementType::CodeBlock,
                xberg::types::ElementType::Table => crate::models::ElementType::Table,
                xberg::types::ElementType::Image => crate::models::ElementType::Image,
                xberg::types::ElementType::PageBreak => crate::models::ElementType::PageBreak,
                _ => crate::models::ElementType::Unknown,
            };
            PreviewElement {
                element_type,
                content: e.text,
            }
        })
        .collect();

    Ok(elements)
}

/// One file's parse outcome: either the document plus its content hash, or the
/// reason it could not be parsed.
pub type ParseOutcome = Result<(ParsedDocument, [u8; 32])>;

/// Outcome for a whole batch, positionally matching the input `paths`.
pub type BatchParseResult = Vec<ParseOutcome>;

/// One file's outcome: the index position plus its parse result.
type ParsedSlot = (usize, ParseOutcome);

/// Splits `paths` into plaintext/code work and heavy document work.
///
/// Returns the Xberg inputs and the position each one corresponds to in
/// `paths`.
fn split_batch(paths: &[PathBuf]) -> (Vec<xberg::ExtractInput>, Vec<usize>) {
    let mut inputs = Vec::with_capacity(paths.len());
    let mut indices = Vec::with_capacity(paths.len());
    for (idx, path) in paths.iter().enumerate() {
        if is_plaintext_fast_path(path) {
            continue;
        }
        inputs.push(xberg::ExtractInput::from_uri(
            path.to_string_lossy().into_owned(),
        ));
        indices.push(idx);
    }
    (inputs, indices)
}

/// Parses every plaintext/code file in `paths` in parallel, off the async
/// runtime.
///
/// This is I/O plus UTF-8 validation bound. It used to run inline on the calling
/// Tokio worker for every file in the chunk, which stalled the entire runtime —
/// and therefore the UI's async tasks — for the duration of the batch.
async fn parse_plaintext_files(paths: &[PathBuf]) -> Result<Vec<ParsedSlot>> {
    use rayon::prelude::*;

    if paths.is_empty() {
        return Ok(Vec::new());
    }

    // Owned copies so the rayon closure is `'static` for `spawn_blocking`.
    let owned: Vec<(usize, PathBuf)> = paths
        .iter()
        .enumerate()
        .map(|(i, p)| (i, p.clone()))
        .collect();

    // `spawn_blocking` keeps the blocking reads off the runtime's async workers.
    tokio::task::spawn_blocking(move || {
        owned
            .par_iter()
            .map(|(idx, path)| {
                let out = parse_plaintext(path)
                    .map(|(content, hash)| (plaintext_document(path, content), hash));
                (*idx, out)
            })
            .collect()
    })
    .await
    .map_err(|e| {
        FlashError::Io(std::sync::Arc::new(std::io::Error::other(format!(
            "Plaintext parse pool panicked: {e}"
        ))))
    })
}

/// Parses a batch of files: plaintext/code on the rayon pool, everything else via
/// Xberg's concurrent batch extractor.
pub async fn parse_files_batch(
    paths: &[PathBuf],
    max_threads: u8,
    enable_ocr: bool,
) -> Result<BatchParseResult> {
    let mut slots: Vec<Option<ParseOutcome>> = vec![None; paths.len()];

    // Positions in `paths` that take the plaintext fast path.
    let plaintext_positions: Vec<usize> = paths
        .iter()
        .enumerate()
        .filter(|(_, p)| is_plaintext_fast_path(p))
        .map(|(i, _)| i)
        .collect();

    let (complex_inputs, complex_indices) = split_batch(paths);

    // `parse_plaintext_files` returns slots in the order of `plaintext`, and
    // `plaintext_positions` maps each of those back to a position in `paths`.
    let plaintext: Vec<PathBuf> = plaintext_positions
        .iter()
        .map(|&i| paths[i].clone())
        .collect();
    let plaintext_results = parse_plaintext_files(&plaintext).await?;
    for ((_, result), slot_idx) in plaintext_results.into_iter().zip(plaintext_positions) {
        slots[slot_idx] = Some(result);
    }

    if !complex_inputs.is_empty() {
        let config = xberg::ExtractionConfig {
            use_cache: false,
            max_concurrent_extractions: Some(usize::from(max_threads).max(1)),
            disable_ocr: !enable_ocr,
            output_format: xberg::OutputFormat::Plain,
            ..Default::default()
        };

        match xberg::extract_batch(complex_inputs, &config).await {
            Ok(batch_results) => {
                for result in batch_results.results {
                    let source_idx = result
                        .metadata
                        .additional
                        .get("source_index")
                        .and_then(serde_json::Value::as_u64)
                        .and_then(|v| usize::try_from(v).ok());

                    if let Some(sub_idx) = source_idx
                        && let Some(&actual_idx) = complex_indices.get(sub_idx)
                    {
                        let hash = hash_file_streaming(&paths[actual_idx])?;
                        slots[actual_idx] = Some(Ok((
                            map_extracted_document(&paths[actual_idx], result),
                            hash,
                        )));
                    } else {
                        warn!("Xberg returned a result with no usable source_index");
                    }
                }

                for error in batch_results.errors {
                    if let Some(&actual_idx) = complex_indices.get(error.index) {
                        slots[actual_idx] = Some(Err(FlashError::parse(
                            &paths[actual_idx],
                            format!("Extraction failed: {}", error.message),
                        )));
                    }
                }
            }
            Err(e) => {
                // `extract_batch` failing outright means every document in the
                // chunk is unparsed. Mark them individually so the caller can fall
                // back per-file instead of seeing one opaque batch error.
                error!("Xberg batch extraction failed: {e}");
                for &idx in &complex_indices {
                    slots[idx] = Some(Err(FlashError::parse(
                        &paths[idx],
                        format!("Batch extraction failed: {e}"),
                    )));
                }
            }
        }
    }

    Ok(slots
        .into_iter()
        .enumerate()
        .map(|(idx, slot)| {
            slot.unwrap_or_else(|| {
                Err(FlashError::parse(
                    &paths[idx],
                    "No output returned for file".to_string(),
                ))
            })
        })
        .collect())
}

/// Maps a `xberg::ExtractedDocument` into a `ParsedDocument`.
///
/// `layout`, `code_metadata`, and `embeddings` are intentionally left unset:
/// nothing downstream ever reads them (the writer only uses `content`,
/// `keywords`, `title`, and `path`), and populating them previously meant
/// `format!("{value:?}")`-ing entire structured outputs on every document just
/// to throw the string away.
fn map_extracted_document(path: &Path, doc: xberg::ExtractedDocument) -> ParsedDocument {
    let language = doc
        .detected_languages
        .as_ref()
        .and_then(|langs| langs.first().map(CompactString::from));

    let keywords = doc.metadata.keywords.as_ref().map(|kws| {
        kws.iter()
            .map(std::string::String::as_str)
            .collect::<Vec<_>>()
            .join(" ")
    });

    ParsedDocument {
        path: path.to_string_lossy().to_string(),
        content: doc.content,
        title: doc
            .metadata
            .title
            .as_ref()
            .map(|t| CompactString::from(t.as_str())),
        language,
        keywords,
        layout: None,
        code_metadata: None,
        embeddings: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsStr;

    fn extension_matches(extension: &OsStr, expected: &str) -> bool {
        extension
            .to_str()
            .is_some_and(|s| s.to_lowercase() == expected)
    }

    #[test]
    fn test_extension_matches() {
        assert!(extension_matches(OsStr::new("docx"), "docx"));
        assert!(extension_matches(OsStr::new("DOCX"), "docx"));
        assert!(extension_matches(OsStr::new("Docx"), "docx"));
        assert!(!extension_matches(OsStr::new("pdf"), "docx"));
    }

    #[tokio::test]
    async fn test_parse_file_txt() {
        use std::io::Write;
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("test.txt");
        let mut file = std::fs::File::create(&file_path).unwrap();
        writeln!(file, "Hello, world!").unwrap();

        let result = parse_file(&file_path, false).await;
        assert!(result.is_ok());
        let doc = result.unwrap();
        assert!(doc.content.contains("Hello, world!"));
    }

    #[tokio::test]
    async fn test_parse_file_unknown() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("test.unknown");
        std::fs::File::create(&file_path).unwrap();

        let result = parse_file(&file_path, false).await;
        assert!(result.is_err());
    }
}
