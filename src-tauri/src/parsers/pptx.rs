use crate::error::{FlashError, Result};
use crate::parsers::ParsedDocument;
use litchi::Presentation;
use std::path::Path;

const MAX_TEXT_LENGTH: usize = 50_000_000;

pub fn parse_pptx(path: &Path) -> Result<ParsedDocument> {
    let pres = Presentation::open(path).map_err(|e| {
        FlashError::parse(path, format!(
            "Failed to open presentation {}: {}",
            path.display(),
            e
        ))
    })?;

    let text = pres.text().map_err(|e| {
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

    let slide_count = pres.slide_count().unwrap_or(0);
    let title = path
        .file_stem()
        .map(|s| format!("{} ({} slides)", s.to_string_lossy(), slide_count));

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
    fn test_pptx_parsing_placeholder() {}
}
