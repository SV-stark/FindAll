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
}

/// Detect file type and route to appropriate parser using Kreuzberg
pub fn parse_file(path: &Path) -> Result<ParsedDocument> {
    // Log the file extension for debugging
    let extension = path.extension().and_then(|e| e.to_str()).unwrap_or("none");
    tracing::debug!(
        "Parsing file: {} (extension: {})",
        path.display(),
        extension
    );

    let mime = kreuzberg::detect_mime_type(path, true).ok();

    // Disable cache to prevent unbounded memory growth during deep directory scans.
    let config = kreuzberg::ExtractionConfig {
        use_cache: false,
        ..Default::default()
    };

    let result = kreuzberg::extract_file_sync(path, mime.as_deref(), &config).map_err(|e| {
        tracing::error!("Failed to extract file {}: {}", path.display(), e);
        FlashError::parse(path, format!("Extraction failed: {e}"))
    })?;

    tracing::debug!("Successfully parsed file: {}", path.display());

    let language = result
        .detected_languages
        .as_ref()
        .and_then(|langs| langs.first().map(CompactString::from));

    let keywords = result.extracted_keywords.as_ref().map(|kws| {
        kws.iter()
            .map(|k| k.text.as_str())
            .collect::<Vec<_>>()
            .join(" ")
    });

    Ok(ParsedDocument {
        path: path.to_string_lossy().to_string(),
        content: result.content,
        title: result.metadata.title.map(CompactString::from),
        language,
        keywords,
    })
}

/// Process a batch of files using Kreuzberg's native asynchronous batch extraction
pub fn parse_files_batch(paths: &[PathBuf]) -> Result<Vec<Result<ParsedDocument>>> {
    tracing::debug!("Batch parsing {} files natively", paths.len());

    let config = kreuzberg::ExtractionConfig {
        use_cache: false,
        ..Default::default()
    };

    // Dispatch to the native batching execution pool
    let batched_paths: Vec<(PathBuf, Option<kreuzberg::FileExtractionConfig>)> =
        paths.iter().map(|p| (p.clone(), None)).collect();

    let batch_results =
        kreuzberg::batch_extract_file_sync(batched_paths, &config).map_err(|e| {
            tracing::error!("Kreuzberg batch extraction failed entirely: {}", e);
            FlashError::parse(Path::new("batch"), format!("Batch extraction crashed: {e}"))
        })?;

    let mut final_results = Vec::with_capacity(batch_results.len());

    for (i, result) in batch_results.into_iter().enumerate() {
        if let Some(path) = paths.get(i) {
            let language = result
                .detected_languages
                .as_ref()
                .and_then(|langs| langs.first().map(CompactString::from));

            let keywords = result.extracted_keywords.as_ref().map(|kws| {
                kws.iter()
                    .map(|k| k.text.as_str())
                    .collect::<Vec<_>>()
                    .join(" ")
            });

            final_results.push(Ok(ParsedDocument {
                path: path.to_string_lossy().to_string(),
                content: result.content,
                title: result.metadata.title.map(CompactString::from),
                language,
                keywords,
            }));
        }
    }

    Ok(final_results)
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

    #[test]
    fn test_parse_file_txt() {
        use std::io::Write;
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("test.txt");
        let mut file = std::fs::File::create(&file_path).unwrap();
        writeln!(file, "Hello, world!").unwrap();

        let result = parse_file(&file_path);
        assert!(result.is_ok());
        let doc = result.unwrap();
        assert!(doc.content.contains("Hello, world!"));
    }

    #[test]
    fn test_parse_file_unknown() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("test.unknown");
        std::fs::File::create(&file_path).unwrap();

        let result = parse_file(&file_path);
        assert!(result.is_err());
    }
}
