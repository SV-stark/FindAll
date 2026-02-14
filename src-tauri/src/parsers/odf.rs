use crate::error::{FlashError, Result};
use crate::parsers::ParsedDocument;
use litchi::odf::Document;
use std::path::Path;

const MAX_TEXT_LENGTH: usize = 100 * 1024 * 1024;

pub fn parse_odf(path: &Path) -> Result<ParsedDocument> {
    let doc = Document::open(path).map_err(|e| {
        FlashError::parse(path, format!(
            "Failed to open ODF document {}: {}",
            path.display(),
            e
        ))
    })?;

    let text = doc.text().map_err(|e| {
        FlashError::parse(path, format!(
            "Failed to extract text from {}: {}",
            path.display(),
            e
        ))
    })?;

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
    fn test_odf_parsing_placeholder() {}
}
