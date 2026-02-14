use crate::error::{FlashError, Result};
use crate::parsers::ParsedDocument;
use std::path::Path;

/// Maximum PDF content size to prevent memory issues
const MAX_PDF_SIZE: u64 = 500 * 1024 * 1024; // 500MB limit

/// Parse PDF file using pdf-extract crate
/// Implements proper resource management and error handling
pub fn parse_pdf(path: &Path) -> Result<ParsedDocument> {
    // Check file size first to prevent OOM
    let metadata = std::fs::metadata(path)
        .map_err(|e| FlashError::parse(path, format!("Failed to read PDF metadata: {}", e)))?;

    if metadata.len() > MAX_PDF_SIZE {
        return Err(FlashError::parse(
            path,
            format!(
                "PDF file too large: {} bytes (max: {})",
                metadata.len(),
                MAX_PDF_SIZE
            ),
        ));
    }

    // Use catch_unwind to prevent panics from crashing the application
    let text = std::panic::catch_unwind(|| pdf_extract::extract_text(path));

    let content = match text {
        Ok(Ok(text)) => text,
        Ok(Err(e)) => {
            eprintln!(
                "Warning: Failed to extract PDF text from {}: {}",
                path.display(),
                e
            );
            String::new()
        }
        Err(_) => {
            eprintln!(
                "Warning: PDF extraction panicked for {}. File may be corrupted or encrypted.",
                path.display()
            );
            String::new()
        }
    };

    // Try to extract title from first line or filename
    let title = extract_title_from_content(&content)
        .or_else(|| path.file_stem().map(|s| s.to_string_lossy().to_string()));

    Ok(ParsedDocument {
        path: path.to_string_lossy().to_string(),
        content,
        title,
    })
}

/// Extract title from first non-empty line of content
/// Filters out lines that are too long (likely not titles)
fn extract_title_from_content(content: &str) -> Option<String> {
    content
        .lines()
        .find(|line| {
            let trimmed = line.trim();
            !trimmed.is_empty() && trimmed.len() < 200
        })
        .map(|line| {
            let trimmed = line.trim();
            // Clean up common PDF artifacts
            trimmed
                .replace("\u{0000}", "") // Remove null bytes
                .replace("\r", "") // Remove carriage returns
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_title_from_content() {
        let content = "\n\n  Document Title  \n\nBody content here\n";
        assert_eq!(
            extract_title_from_content(content),
            Some("Document Title".to_string())
        );
    }

    #[test]
    fn test_extract_title_empty() {
        let content = "   \n\n   \n";
        assert_eq!(extract_title_from_content(content), None);
    }

    #[test]
    fn test_extract_title_too_long() {
        let content = "a".repeat(300);
        assert_eq!(extract_title_from_content(&content), None);
    }
}
