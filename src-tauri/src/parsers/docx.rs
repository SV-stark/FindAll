use crate::error::{FlashError, Result};
use crate::parsers::ParsedDocument;
use litchi::Document;
use std::path::Path;

const MAX_TEXT_LENGTH: usize = 100 * 1024 * 1024;

pub fn parse_docx(path: &Path) -> Result<ParsedDocument> {
    let doc = Document::open(path)
        .map_err(|e| FlashError::parse(path, format!("Failed to open document: {}", e)))?;

    let text = doc
        .text()
        .map_err(|e| FlashError::parse(path, format!("Failed to extract text: {}", e)))?;

    let content = if text.len() > MAX_TEXT_LENGTH {
        text[..MAX_TEXT_LENGTH].to_string()
    } else {
        text
    };

    let title = path.file_stem().map(|s| s.to_string_lossy().to_string());

    Ok(ParsedDocument {
        path: path.to_string_lossy().to_string(),
        content,
        title,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_docx_parsing_placeholder() {}
}
