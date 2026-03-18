use crate::error::{FlashError, Result};
use std::path::Path;

pub mod memory_map;

#[derive(Debug, Clone)]
pub struct ParsedDocument {
    pub path: String,
    pub content: String,
    pub title: Option<String>,
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

    // Disable cache to prevent unbounded memory growth during deep directory scans.
    let config = kreuzberg::ExtractionConfig {
        use_cache: false,
        ..Default::default()
    };

    let result = kreuzberg::extract_file_sync(path, None, &config).map_err(|e| {
        tracing::error!("Failed to extract file {}: {}", path.display(), e);
        FlashError::parse(path, format!("Extraction failed: {}", e))
    })?;

    tracing::debug!("Successfully parsed file: {}", path.display());

    Ok(ParsedDocument {
        path: path.to_string_lossy().to_string(),
        content: result.content,
        title: result.metadata.title,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsStr;

    fn extension_matches(extension: &OsStr, expected: &str) -> bool {
        extension
            .to_str()
            .map(|s| s.to_lowercase() == expected)
            .unwrap_or(false)
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
