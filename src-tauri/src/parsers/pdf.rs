use crate::error::Result;
use crate::parsers::ParsedDocument;
use std::path::Path;

/// Parse PDF file using pdf-extract crate
/// Falls back to empty string on failure (don't crash on corrupted PDFs)
pub fn parse_pdf(path: &Path) -> Result<ParsedDocument> {
    let text = std::panic::catch_unwind(|| pdf_extract::extract_text(path));

    let content = match text {
        Ok(Ok(text)) => text,
        Ok(Err(e)) => {
            eprintln!("Warning: Failed to extract PDF text from {:?}: {}", path, e);
            String::new()
        }
        Err(_) => {
            eprintln!("Warning: PDF extraction panicked for {:?}", path);
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
fn extract_title_from_content(content: &str) -> Option<String> {
    content
        .lines()
        .find(|line| !line.trim().is_empty())
        .map(|line| line.trim().to_string())
        .filter(|line| line.len() < 200) // Don't use entire paragraphs as title
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pdf_parsing_placeholder() {
        // Placeholder for actual test
    }
}
