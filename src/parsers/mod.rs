use crate::error::{FlashError, Result};
use std::path::{Path, PathBuf};

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

/// Ensure Xberg built-in extractors and registries are pre-warmed during bootstrap.
pub fn ensure_initialized() {
    let _ = xberg::core::mime::list_supported_formats();
}

/// Returns all supported file extensions dynamically registered in the active build.
#[must_use]
pub fn list_supported_extensions() -> Vec<String> {
    xberg::core::mime::list_supported_formats()
        .into_iter()
        .map(|f| f.extension)
        .collect()
}

/// Check if a path corresponds to a supported document or plaintext file format.
#[must_use]
pub fn is_supported_file(path: &Path) -> bool {
    if is_plaintext_fast_path(path) {
        return true;
    }
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        let ext_lower = ext.to_lowercase();
        return xberg::core::mime::list_supported_formats()
            .iter()
            .any(|f| f.extension.eq_ignore_ascii_case(&ext_lower));
    }
    false
}

/// Detect file type and route to appropriate parser
pub async fn parse_file(path: &Path, enable_ocr: bool) -> Result<ParsedDocument> {
    // Zero-copy plaintext / code fast path (microsecond execution)
    if is_plaintext_fast_path(path) {
        let file_data = memory_map::read_file(path)?;
        let content = std::str::from_utf8(&file_data).map_or_else(
            |_| String::from_utf8_lossy(&file_data).into_owned(),
            std::string::ToString::to_string,
        );

        return Ok(ParsedDocument {
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
        });
    }

    // Heavy document extraction pipeline (PDF, DOCX, XLSX, etc.)
    // Uses OutputFormat::Plain for indexing to strip markdown syntax tokens
    let config = xberg::ExtractionConfig {
        use_cache: false,
        disable_ocr: !enable_ocr,
        output_format: xberg::OutputFormat::Plain,
        ..Default::default()
    };

    // Use URI input directly to avoid cloning file buffers in memory
    let input = xberg::ExtractInput::from_uri(path.to_string_lossy().into_owned());

    let result = xberg::extract(input, &config).await.map_err(|e| {
        tracing::error!("Failed to extract file {}: {}", path.display(), e);
        FlashError::parse(path, format!("Extraction failed: {e}"))
    })?;

    let doc = result.results.into_iter().next().ok_or_else(|| {
        FlashError::parse(path, "Extraction returned empty results list".to_string())
    })?;

    Ok(map_extracted_document(path, doc))
}

pub async fn parse_file_preview(path: &Path, enable_ocr: bool) -> Result<Vec<PreviewElement>> {
    // Fast path for code and text preview rendering
    if is_plaintext_fast_path(path) {
        let file_data = memory_map::read_file(path)?;
        let content = std::str::from_utf8(&file_data).map_or_else(
            |_| String::from_utf8_lossy(&file_data).into_owned(),
            std::string::ToString::to_string,
        );

        let extension = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        let is_code = matches!(
            extension.to_ascii_lowercase().as_str(),
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
        );

        let element_type = if is_code {
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

/// Process a batch of files using parallel plaintext parsing and async Xberg extraction
pub async fn parse_files_batch(
    paths: &[PathBuf],
    max_threads: u8,
    enable_ocr: bool,
) -> Result<Vec<Result<ParsedDocument>>> {
    let mut slots: Vec<Option<Result<ParsedDocument>>> = vec![None; paths.len()];
    let mut complex_inputs = Vec::new();
    let mut complex_indices = Vec::new();

    // Fast-path evaluation for plain text & source code files
    for (idx, path) in paths.iter().enumerate() {
        if is_plaintext_fast_path(path) {
            match memory_map::read_file(path) {
                Ok(file_data) => {
                    let content = std::str::from_utf8(&file_data).map_or_else(
                        |_| String::from_utf8_lossy(&file_data).into_owned(),
                        std::string::ToString::to_string,
                    );
                    slots[idx] = Some(Ok(ParsedDocument {
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
                    }));
                }
                Err(e) => {
                    slots[idx] = Some(Err(e));
                }
            }
        } else {
            complex_inputs.push(xberg::ExtractInput::from_uri(
                path.to_string_lossy().into_owned(),
            ));
            complex_indices.push(idx);
        }
    }

    // Process any remaining complex binary files via Xberg
    if !complex_inputs.is_empty() {
        let config = xberg::ExtractionConfig {
            use_cache: false,
            max_concurrent_extractions: Some(max_threads as usize),
            disable_ocr: !enable_ocr,
            output_format: xberg::OutputFormat::Plain,
            ..Default::default()
        };

        if let Ok(batch_results) = xberg::extract_batch(complex_inputs, &config).await {
            for result in batch_results.results {
                let source_idx = result
                    .metadata
                    .additional
                    .get("source_index")
                    .and_then(serde_json::Value::as_u64)
                    .and_then(|v| usize::try_from(v).ok());

                if let Some(sub_idx) = source_idx
                    && sub_idx < complex_indices.len()
                {
                    let actual_idx = complex_indices[sub_idx];
                    slots[actual_idx] =
                        Some(Ok(map_extracted_document(&paths[actual_idx], result)));
                }
            }

            for error in batch_results.errors {
                if error.index < complex_indices.len() {
                    let actual_idx = complex_indices[error.index];
                    slots[actual_idx] = Some(Err(FlashError::parse(
                        &paths[actual_idx],
                        format!("Extraction failed: {}", error.message),
                    )));
                }
            }
        }
    }

    let results = slots
        .into_iter()
        .enumerate()
        .map(|(idx, opt)| {
            opt.unwrap_or_else(|| {
                Err(FlashError::parse(
                    &paths[idx],
                    "No output returned for file".to_string(),
                ))
            })
        })
        .collect();

    Ok(results)
}

/// Maps a `xberg::ExtractedDocument` into a `ParsedDocument`.
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
        layout: doc.structured_output.map(|l| format!("{l:?}")),
        code_metadata: doc.annotations.map(|c| format!("{c:?}")),
        embeddings: doc
            .chunks
            .and_then(|c| c.into_iter().find_map(|chunk| chunk.embedding)),
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
