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

/// Detect file type and route to appropriate parser using Xberg
pub async fn parse_file(path: &Path, enable_ocr: bool) -> Result<ParsedDocument> {
    // Log the file extension for debugging
    let extension = path.extension().and_then(|e| e.to_str()).unwrap_or("none");
    tracing::debug!(
        "Parsing file: {} (extension: {})",
        path.display(),
        extension
    );

    let mime = xberg::detect_mime_type(path.to_string_lossy().into_owned(), true)
        .map_err(|e| FlashError::parse(path, format!("Mime detection failed: {e}")))?;

    // Disable cache to prevent unbounded memory growth during deep directory scans.
    let config = xberg::ExtractionConfig {
        use_cache: false,
        disable_ocr: !enable_ocr,
        ..Default::default()
    };

    let file_data = memory_map::read_file(path)?;
    let input = xberg::ExtractInput::from_bytes(
        file_data.to_vec(),
        mime,
        path.file_name().map(|n| n.to_string_lossy().into_owned()),
    );

    let result = xberg::extract(input, &config).await.map_err(|e| {
        tracing::error!("Failed to extract file {}: {}", path.display(), e);
        FlashError::parse(path, format!("Extraction failed: {e}"))
    })?;

    tracing::debug!("Successfully parsed file: {}", path.display());

    let doc = result.results.into_iter().next().ok_or_else(|| {
        FlashError::parse(path, "Extraction returned empty results list".to_string())
    })?;

    Ok(map_extracted_document(path, doc))
}

pub async fn parse_file_preview(path: &Path, enable_ocr: bool) -> Result<Vec<PreviewElement>> {
    let mime = xberg::detect_mime_type(path.to_string_lossy().into_owned(), true)
        .map_err(|e| FlashError::parse(path, format!("Mime detection failed: {e}")))?;

    let config = xberg::ExtractionConfig {
        use_cache: false,
        disable_ocr: !enable_ocr,
        result_format: xberg::ResultFormat::ElementBased,
        ..Default::default()
    };

    let file_data = memory_map::read_file(path)?;
    let input = xberg::ExtractInput::from_bytes(
        file_data.to_vec(),
        mime,
        path.file_name().map(|n| n.to_string_lossy().into_owned()),
    );

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

/// Process a batch of files using Xberg's native async concurrent batch extraction.
///
/// This is `async` — it must be called from within a Tokio async context. Xberg
/// manages its own semaphore-gated `JoinSet` internally.
pub async fn parse_files_batch(
    paths: &[PathBuf],
    max_threads: u8,
    enable_ocr: bool,
) -> Result<Vec<Result<ParsedDocument>>> {
    tracing::debug!(
        "Async batch parsing {} files via xberg (max_threads: {})",
        paths.len(),
        max_threads
    );

    let config = xberg::ExtractionConfig {
        use_cache: false,
        max_concurrent_extractions: Some(max_threads as usize),
        disable_ocr: !enable_ocr,
        ..Default::default()
    };

    let inputs: Vec<xberg::ExtractInput> = paths
        .iter()
        .map(|p| xberg::ExtractInput::from_uri(p.to_string_lossy().into_owned()))
        .collect();

    let batch_results = xberg::extract_batch(inputs, &config).await.map_err(|e| {
        tracing::error!("Xberg async batch extraction failed entirely: {}", e);
        FlashError::parse(Path::new("batch"), format!("Batch extraction crashed: {e}"))
    })?;

    let mut slots: Vec<Option<Result<ParsedDocument>>> = vec![None; paths.len()];

    for result in batch_results.results {
        let index = result
            .metadata
            .additional
            .get("source_index")
            .and_then(serde_json::Value::as_u64)
            .and_then(|v| usize::try_from(v).ok());

        if let Some(idx) = index
            && idx < paths.len()
        {
            slots[idx] = Some(Ok(map_extracted_document(&paths[idx], result)));
        }
    }

    for error in batch_results.errors {
        if error.index < paths.len() {
            slots[error.index] = Some(Err(FlashError::parse(
                &paths[error.index],
                format!("Extraction failed: {}", error.message),
            )));
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
